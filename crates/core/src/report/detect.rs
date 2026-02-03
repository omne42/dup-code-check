use std::collections::{HashMap, HashSet};

use crate::dedupe::detect_duplicate_code_spans_winnowing;
use crate::types::{DuplicateSpanGroup, DuplicateSpanOccurrence, ScanOptions, SimilarityPair};
use crate::util::{
    NormalizedFileView, SpanGroupBuilder, add_occurrence_view, canonicalize_match, fnv1a64_u32,
    fold_u64_to_u32, maximal_match, winnowed_fingerprints,
};

use super::ScannedTextFile;
use super::util::{preview_from_lines, sort_span_groups_for_report};

pub(super) fn detect_duplicate_code_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_match_len = options.min_match_len.max(1);

    let mut normalized = Vec::new();
    for file in files {
        if file.code_chars.len() < min_match_len {
            continue;
        }
        normalized.push(NormalizedFileView {
            repo_id: file.repo_id,
            repo_label: &file.repo_label,
            rel_path: &file.path,
            normalized: &file.code_chars,
            line_map: &file.code_char_lines,
        });
    }

    if normalized.is_empty() {
        return Vec::new();
    }

    let mut out = detect_duplicate_code_spans_winnowing(&normalized, options);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    out
}

pub(super) fn detect_duplicate_line_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_char_len = options.min_match_len.max(1);

    let mut normalized = Vec::new();
    let mut file_line_lens = Vec::new();
    let mut file_texts = Vec::new();

    for file in files {
        if file.line_tokens.is_empty() {
            continue;
        }
        normalized.push(NormalizedFileView {
            repo_id: file.repo_id,
            repo_label: &file.repo_label,
            rel_path: &file.path,
            normalized: &file.line_tokens,
            line_map: &file.line_token_lines,
        });
        file_line_lens.push(file.line_token_char_lens.as_slice());
        file_texts.push(file.text.as_str());
    }

    detect_duplicate_span_groups_with_len_filter(
        &normalized,
        2,
        2,
        8,
        options.cross_repo_only,
        |file_id, start, len| {
            let lens = file_line_lens[file_id];
            let mut total = 0usize;
            for &l in &lens[start..start + len] {
                total += l;
                if total >= min_char_len {
                    return true;
                }
            }
            false
        },
        |file_id, start_line, end_line| {
            preview_from_lines(file_texts[file_id], start_line, end_line, 120)
        },
        options.max_report_items,
    )
}

pub(super) fn detect_duplicate_token_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);
    let fingerprint_len = min_token_len.clamp(1, 25);
    let window_size = min_token_len
        .saturating_sub(fingerprint_len)
        .saturating_add(1);

    let mut normalized = Vec::new();
    let mut file_texts = Vec::new();

    for file in files {
        if file.tokens.len() < min_token_len {
            continue;
        }
        normalized.push(NormalizedFileView {
            repo_id: file.repo_id,
            repo_label: &file.repo_label,
            rel_path: &file.path,
            normalized: &file.tokens,
            line_map: &file.token_lines,
        });
        file_texts.push(file.text.as_str());
    }

    detect_duplicate_span_groups_with_len_filter(
        &normalized,
        min_token_len,
        fingerprint_len,
        window_size,
        options.cross_repo_only,
        |_file_id, _start, _len| true,
        |file_id, start_line, end_line| {
            preview_from_lines(file_texts[file_id], start_line, end_line, 120)
        },
        options.max_report_items,
    )
}

pub(super) fn detect_duplicate_blocks(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);

    let mut groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>> = HashMap::new();

    for (file_id, file) in files.iter().enumerate() {
        for node in &file.blocks {
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }
            let slice = &file.tokens[start..node.end_token];
            if slice.len() < min_token_len {
                continue;
            }
            let content_hash = fnv1a64_u32(slice);
            let key = (content_hash, slice.len());
            let bucket = groups.entry(key).or_default();

            let builder = match bucket.iter_mut().find(|g| g.sample == slice) {
                Some(existing) => existing,
                None => {
                    let preview =
                        preview_from_lines(&file.text, node.start_line, node.end_line, 120);
                    bucket.push(SpanGroupBuilder {
                        content_hash,
                        normalized_len: slice.len(),
                        sample: slice.to_vec(),
                        preview: String::new(),
                        occurrences: Vec::new(),
                        occurrence_keys: HashSet::new(),
                        repo_ids: HashSet::new(),
                    });
                    let b = bucket.last_mut().expect("just pushed");
                    b.occurrences.push(DuplicateSpanOccurrence {
                        repo_id: file.repo_id,
                        repo_label: file.repo_label.clone(),
                        path: file.path.clone(),
                        start_line: node.start_line,
                        end_line: node.end_line,
                    });
                    b.repo_ids.insert(file.repo_id);
                    b.occurrence_keys.insert((file_id, node.start_token));
                    b.preview = preview;
                    continue;
                }
            };

            if !builder.occurrence_keys.insert((file_id, node.start_token)) {
                continue;
            }
            builder.repo_ids.insert(file.repo_id);
            builder.occurrences.push(DuplicateSpanOccurrence {
                repo_id: file.repo_id,
                repo_label: file.repo_label.clone(),
                path: file.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
            });
        }
    }

    let mut out = finalize_span_groups(groups, options);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    out
}

pub(super) fn detect_duplicate_ast_subtrees(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);

    #[derive(Debug, Clone)]
    struct NodeRepr {
        hash: u64,
        repr: Vec<u32>,
    }

    let mut groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>> = HashMap::new();

    for (file_id, file) in files.iter().enumerate() {
        let mut reprs: Vec<Option<NodeRepr>> = vec![None; file.blocks.len()];
        let mut by_depth: Vec<usize> = (0..file.blocks.len()).collect();
        by_depth.sort_by_key(|&i| std::cmp::Reverse(file.blocks[i].depth));

        for node_id in by_depth {
            let node = &file.blocks[node_id];
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }

            let mut children: Vec<(usize, usize, usize)> = node
                .children
                .iter()
                .map(|&cid| {
                    let c = &file.blocks[cid];
                    (c.start_token, c.end_token, cid)
                })
                .collect();
            children.sort_by_key(|c| c.0);

            let mut repr = Vec::new();
            let mut idx = start;
            for (c_start, c_end, cid) in children {
                while idx < c_start && idx < node.end_token {
                    repr.push(file.tokens[idx]);
                    idx += 1;
                }
                if idx == c_start {
                    let child_hash = reprs[cid].as_ref().map(|r| r.hash).unwrap_or(0);
                    repr.push(50_000);
                    repr.push(fold_u64_to_u32(child_hash));
                    idx = c_end.saturating_add(1);
                }
            }
            while idx < node.end_token {
                repr.push(file.tokens[idx]);
                idx += 1;
            }

            let hash = fnv1a64_u32(&repr);
            reprs[node_id] = Some(NodeRepr { hash, repr });

            let repr_len = reprs[node_id].as_ref().map(|r| r.repr.len()).unwrap_or(0);
            if repr_len < min_token_len {
                continue;
            }

            let content_hash = hash;
            let key = (content_hash, repr_len);
            let bucket = groups.entry(key).or_default();

            let builder = match bucket.iter_mut().find(|g| {
                reprs[node_id].as_ref().map(|r| r.repr.as_slice()) == Some(g.sample.as_slice())
            }) {
                Some(existing) => existing,
                None => {
                    let preview =
                        preview_from_lines(&file.text, node.start_line, node.end_line, 120);
                    bucket.push(SpanGroupBuilder {
                        content_hash,
                        normalized_len: repr_len,
                        sample: reprs[node_id]
                            .as_ref()
                            .map(|r| r.repr.clone())
                            .unwrap_or_default(),
                        preview: String::new(),
                        occurrences: Vec::new(),
                        occurrence_keys: HashSet::new(),
                        repo_ids: HashSet::new(),
                    });
                    let b = bucket.last_mut().expect("just pushed");
                    b.occurrences.push(DuplicateSpanOccurrence {
                        repo_id: file.repo_id,
                        repo_label: file.repo_label.clone(),
                        path: file.path.clone(),
                        start_line: node.start_line,
                        end_line: node.end_line,
                    });
                    b.repo_ids.insert(file.repo_id);
                    b.occurrence_keys.insert((file_id, node.start_token));
                    b.preview = preview;
                    continue;
                }
            };

            if !builder.occurrence_keys.insert((file_id, node.start_token)) {
                continue;
            }
            builder.repo_ids.insert(file.repo_id);
            builder.occurrences.push(DuplicateSpanOccurrence {
                repo_id: file.repo_id,
                repo_label: file.repo_label.clone(),
                path: file.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
            });
        }
    }

    let mut out = finalize_span_groups(groups, options);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    out
}

#[allow(clippy::too_many_arguments)]
fn detect_duplicate_span_groups_with_len_filter<'a>(
    files: &[NormalizedFileView<'a>],
    min_len: usize,
    fingerprint_len: usize,
    window_size: usize,
    cross_repo_only: bool,
    accept_match: impl Fn(usize, usize, usize) -> bool,
    preview_from_occurrence: impl Fn(usize, u32, u32) -> String,
    max_items: usize,
) -> Vec<DuplicateSpanGroup> {
    if max_items == 0 {
        return Vec::new();
    }
    if files.is_empty() {
        return Vec::new();
    }

    #[derive(Debug, Clone, Copy)]
    struct FingerprintOcc {
        file_id: usize,
        pos: usize,
    }

    let mut fingerprints: HashMap<u64, Vec<FingerprintOcc>> = HashMap::new();
    for (file_id, file) in files.iter().enumerate() {
        if file.normalized.len() < fingerprint_len {
            continue;
        }
        for (hash, pos) in winnowed_fingerprints(file.normalized, fingerprint_len, window_size) {
            fingerprints
                .entry(hash)
                .or_default()
                .push(FingerprintOcc { file_id, pos });
        }
    }

    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    struct MatchKey {
        file_a: usize,
        start_a: usize,
        file_b: usize,
        start_b: usize,
        len: usize,
    }

    const MAX_BUCKET: usize = 512;

    fn truncate_bucket_by_repo<'a>(
        mut occs: Vec<FingerprintOcc>,
        files: &[NormalizedFileView<'a>],
        max_bucket: usize,
    ) -> Vec<FingerprintOcc> {
        if occs.len() <= max_bucket {
            return occs;
        }

        occs.sort_by_key(|o| (files[o.file_id].repo_id, o.file_id, o.pos));

        let mut by_repo: Vec<(usize, Vec<FingerprintOcc>)> = Vec::new();
        for occ in occs {
            let repo_id = files[occ.file_id].repo_id;
            if let Some((last_repo, bucket)) = by_repo.last_mut()
                && *last_repo == repo_id
            {
                bucket.push(occ);
            } else {
                by_repo.push((repo_id, vec![occ]));
            }
        }

        let mut idxs = vec![0usize; by_repo.len()];
        let mut out = Vec::with_capacity(max_bucket);
        while out.len() < max_bucket {
            let mut progressed = false;
            for (i, (_, bucket)) in by_repo.iter().enumerate() {
                if idxs[i] < bucket.len() {
                    out.push(bucket[idxs[i]]);
                    idxs[i] += 1;
                    progressed = true;
                    if out.len() == max_bucket {
                        break;
                    }
                }
            }
            if !progressed {
                break;
            }
        }

        out
    }

    let mut seen_matches: HashSet<MatchKey> = HashSet::new();
    let mut groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>> = HashMap::new();

    for mut occs in fingerprints.into_values() {
        if occs.len() <= 1 {
            continue;
        }
        if occs.len() > MAX_BUCKET {
            occs = truncate_bucket_by_repo(occs, files, MAX_BUCKET);
        }

        for i in 0..occs.len() {
            for j in (i + 1)..occs.len() {
                let a = occs[i];
                let b = occs[j];
                if a.file_id == b.file_id && a.pos == b.pos {
                    continue;
                }
                if cross_repo_only && files[a.file_id].repo_id == files[b.file_id].repo_id {
                    continue;
                }

                let (start_a, start_b, len) = match maximal_match(
                    files[a.file_id].normalized,
                    a.pos,
                    files[b.file_id].normalized,
                    b.pos,
                    fingerprint_len,
                ) {
                    Some(v) => v,
                    None => continue,
                };

                if len < min_len {
                    continue;
                }
                if !accept_match(a.file_id, start_a, len) || !accept_match(b.file_id, start_b, len)
                {
                    continue;
                }

                if a.file_id == b.file_id {
                    let a_end = start_a + len;
                    let b_end = start_b + len;
                    if start_a < b_end && start_b < a_end {
                        continue;
                    }
                }

                let (file_a, file_b, start_a, start_b) =
                    canonicalize_match(a.file_id, b.file_id, start_a, start_b);
                let key = MatchKey {
                    file_a,
                    start_a,
                    file_b,
                    start_b,
                    len,
                };
                if !seen_matches.insert(key) {
                    continue;
                }

                let sample = files[file_a].normalized[start_a..start_a + len].to_vec();
                let content_hash = fnv1a64_u32(&sample);
                let bucket = groups.entry((content_hash, len)).or_default();
                let builder = match bucket.iter_mut().find(|g| g.sample == sample) {
                    Some(existing) => existing,
                    None => {
                        bucket.push(SpanGroupBuilder {
                            content_hash,
                            normalized_len: len,
                            sample,
                            preview: String::new(),
                            occurrences: Vec::new(),
                            occurrence_keys: HashSet::new(),
                            repo_ids: HashSet::new(),
                        });
                        bucket.last_mut().expect("just pushed")
                    }
                };

                if builder.occurrences.is_empty()
                    && let (Some(&start_line), Some(&end_line)) = (
                        files[file_a].line_map.get(start_a),
                        files[file_a].line_map.get(start_a + len - 1),
                    )
                {
                    builder.preview = preview_from_occurrence(file_a, start_line, end_line);
                }

                add_occurrence_view(builder, &files[file_a], file_a, start_a, len);
                add_occurrence_view(builder, &files[file_b], file_b, start_b, len);
            }
        }
    }

    let mut out = finalize_span_groups(
        groups,
        &ScanOptions {
            cross_repo_only,
            ..ScanOptions::default()
        },
    );
    sort_span_groups_for_report(&mut out);
    out.truncate(max_items);
    out
}

fn finalize_span_groups(
    groups: HashMap<(u64, usize), Vec<SpanGroupBuilder>>,
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let mut out = Vec::new();
    for builders in groups.into_values() {
        for mut builder in builders {
            if builder.occurrences.len() <= 1 {
                continue;
            }
            if options.cross_repo_only && builder.repo_ids.len() < 2 {
                continue;
            }

            builder.occurrences.sort_by(|a, b| {
                (a.repo_id, &a.repo_label, &a.path, a.start_line, a.end_line).cmp(&(
                    b.repo_id,
                    &b.repo_label,
                    &b.path,
                    b.start_line,
                    b.end_line,
                ))
            });

            out.push(DuplicateSpanGroup {
                content_hash: builder.content_hash,
                normalized_len: builder.normalized_len,
                preview: builder.preview,
                occurrences: builder.occurrences,
            });
        }
    }

    out.sort_by(|a, b| {
        (a.content_hash, a.normalized_len, a.occurrences.len()).cmp(&(
            b.content_hash,
            b.normalized_len,
            b.occurrences.len(),
        ))
    });
    out
}

pub(super) fn find_similar_blocks_minhash(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<SimilarityPair> {
    const SHINGLE: usize = 5;
    const SIG_SIZE: usize = 32;
    const BAND_SIZE: usize = 4;
    const BANDS: usize = SIG_SIZE / BAND_SIZE;

    fn splitmix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    let seeds: [u64; SIG_SIZE] = {
        let mut out = [0u64; SIG_SIZE];
        let mut s = 0x1234_5678_9abc_def0u64;
        for v in &mut out {
            s = splitmix64(s);
            *v = s;
        }
        out
    };

    #[derive(Debug)]
    struct BlockSig {
        occ: DuplicateSpanOccurrence,
        signature: [u32; SIG_SIZE],
    }

    let mut blocks = Vec::new();
    for file in files {
        for node in &file.blocks {
            if node.depth > 2 {
                continue;
            }
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }
            let slice = &file.tokens[start..node.end_token];
            if slice.len() < options.min_token_len || slice.len() < SHINGLE {
                continue;
            }

            let mut mins = [u32::MAX; SIG_SIZE];
            for shingle in slice.windows(SHINGLE) {
                let base = fnv1a64_u32(shingle);
                for i in 0..SIG_SIZE {
                    let h = splitmix64(base ^ seeds[i]) as u32;
                    if h < mins[i] {
                        mins[i] = h;
                    }
                }
            }

            blocks.push(BlockSig {
                occ: DuplicateSpanOccurrence {
                    repo_id: file.repo_id,
                    repo_label: file.repo_label.clone(),
                    path: file.path.clone(),
                    start_line: node.start_line,
                    end_line: node.end_line,
                },
                signature: mins,
            });
        }
    }

    let mut buckets: HashMap<(usize, u64), Vec<usize>> = HashMap::new();
    for (idx, blk) in blocks.iter().enumerate() {
        for band in 0..BANDS {
            let start = band * BAND_SIZE;
            let key_hash = fnv1a64_u32(&blk.signature[start..start + BAND_SIZE]);
            buckets.entry((band, key_hash)).or_default().push(idx);
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ids in buckets.into_values() {
        if ids.len() <= 1 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let key = if a < b { (a, b) } else { (b, a) };
                if !seen.insert(key) {
                    continue;
                }
                let sig_a = &blocks[key.0].signature;
                let sig_b = &blocks[key.1].signature;
                let eq = sig_a.iter().zip(sig_b).filter(|(x, y)| x == y).count();
                let score = eq as f64 / SIG_SIZE as f64;
                if score < options.similarity_threshold {
                    continue;
                }
                if options.cross_repo_only && blocks[key.0].occ.repo_id == blocks[key.1].occ.repo_id
                {
                    continue;
                }
                out.push(SimilarityPair {
                    a: blocks[key.0].occ.clone(),
                    b: blocks[key.1].occ.clone(),
                    score,
                    distance: None,
                });
            }
        }
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(options.max_report_items);
    out
}

pub(super) fn find_similar_blocks_simhash(
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<SimilarityPair> {
    const SHINGLE: usize = 5;
    const BANDS: usize = 4;
    const BAND_BITS: u32 = 16;

    fn splitmix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    #[derive(Debug)]
    struct BlockHash {
        occ: DuplicateSpanOccurrence,
        hash: u64,
    }

    let mut blocks = Vec::new();
    for file in files {
        for node in &file.blocks {
            if node.depth > 2 {
                continue;
            }
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }
            let slice = &file.tokens[start..node.end_token];
            if slice.len() < options.min_token_len || slice.len() < SHINGLE {
                continue;
            }

            let mut sums = [0i32; 64];
            for shingle in slice.windows(SHINGLE) {
                let base = fnv1a64_u32(shingle);
                let h = splitmix64(base);
                for (bit, sum) in sums.iter_mut().enumerate() {
                    if (h >> bit) & 1 == 1 {
                        *sum += 1;
                    } else {
                        *sum -= 1;
                    }
                }
            }

            let mut hash = 0u64;
            for (bit, sum) in sums.iter().enumerate() {
                if *sum > 0 {
                    hash |= 1u64 << bit;
                }
            }

            blocks.push(BlockHash {
                occ: DuplicateSpanOccurrence {
                    repo_id: file.repo_id,
                    repo_label: file.repo_label.clone(),
                    path: file.path.clone(),
                    start_line: node.start_line,
                    end_line: node.end_line,
                },
                hash,
            });
        }
    }

    let mut buckets: HashMap<(u32, u64), Vec<usize>> = HashMap::new();
    for (idx, blk) in blocks.iter().enumerate() {
        for band in 0..BANDS {
            let shift = (band as u32) * BAND_BITS;
            let band_value = (blk.hash >> shift) & 0xffff;
            buckets
                .entry((band as u32, band_value))
                .or_default()
                .push(idx);
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ids in buckets.into_values() {
        if ids.len() <= 1 {
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let key = if a < b { (a, b) } else { (b, a) };
                if !seen.insert(key) {
                    continue;
                }
                let hamming = (blocks[key.0].hash ^ blocks[key.1].hash).count_ones();
                if hamming > options.simhash_max_distance {
                    continue;
                }
                if options.cross_repo_only && blocks[key.0].occ.repo_id == blocks[key.1].occ.repo_id
                {
                    continue;
                }
                let score = 1.0 - (hamming as f64 / 64.0);
                out.push(SimilarityPair {
                    a: blocks[key.0].occ.clone(),
                    b: blocks[key.1].occ.clone(),
                    score,
                    distance: Some(hamming),
                });
            }
        }
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(options.max_report_items);
    out
}
