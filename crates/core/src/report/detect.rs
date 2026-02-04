use std::collections::{HashMap, HashSet};

use crate::dedupe::detect_duplicate_code_spans_winnowing;
use crate::types::{
    DuplicateSpanGroup, DuplicateSpanOccurrence, ScanOptions, ScanStats, SimilarityPair,
};
use crate::util::{NormalizedFileView, SpanGroupBuilder, fnv1a64_u32, fold_u64_to_u32};
use crate::winnowing::{
    WinnowingParams, detect_duplicate_span_groups_winnowing, finalize_span_groups,
};

use super::ScannedTextFile;
use super::util::{fill_missing_previews_from_files, sort_span_groups_for_report};

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

pub(super) fn detect_duplicate_code_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
    stats: &mut ScanStats,
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

    let mut out = detect_duplicate_code_spans_winnowing(&normalized, options, stats);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    out
}

pub(super) fn detect_duplicate_line_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> Vec<DuplicateSpanGroup> {
    let min_char_len = options.min_match_len.max(1);

    let mut normalized = Vec::new();
    let mut file_line_lens = Vec::new();

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
    }

    let mut out = detect_duplicate_span_groups_with_len_filter(
        &normalized,
        WinnowingParams {
            min_len: 2,
            fingerprint_len: 2,
            window_size: 8,
            cross_repo_only: options.cross_repo_only,
        },
        options.max_report_items,
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
        |_file_id, _start_line, _end_line| String::new(),
        stats,
    );
    fill_missing_previews_from_files(files, &mut out, 120);
    out
}

pub(super) fn detect_duplicate_token_spans(
    files: &[ScannedTextFile],
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);
    let fingerprint_len = min_token_len.clamp(1, 25);
    let window_size = min_token_len
        .saturating_sub(fingerprint_len)
        .saturating_add(1);

    let mut normalized = Vec::new();

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
    }

    let mut out = detect_duplicate_span_groups_with_len_filter(
        &normalized,
        WinnowingParams {
            min_len: min_token_len,
            fingerprint_len,
            window_size,
            cross_repo_only: options.cross_repo_only,
        },
        options.max_report_items,
        |_file_id, _start, _len| true,
        |_file_id, _start_line, _end_line| String::new(),
        stats,
    );
    fill_missing_previews_from_files(files, &mut out, 120);
    out
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
                    bucket.push(SpanGroupBuilder {
                        content_hash,
                        normalized_len: slice.len(),
                        sample: slice.to_vec(),
                        preview: String::new(),
                        occurrences: vec![DuplicateSpanOccurrence {
                            repo_id: file.repo_id,
                            repo_label: file.repo_label.clone(),
                            path: file.path.clone(),
                            start_line: node.start_line,
                            end_line: node.end_line,
                        }],
                        occurrence_keys: HashSet::from([(file_id, node.start_token)]),
                        repo_ids: HashSet::from([file.repo_id]),
                    });
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

    let mut out = finalize_span_groups(groups, options.cross_repo_only);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    fill_missing_previews_from_files(files, &mut out, 120);
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
                    bucket.push(SpanGroupBuilder {
                        content_hash,
                        normalized_len: repr_len,
                        sample: reprs[node_id]
                            .as_ref()
                            .map(|r| r.repr.clone())
                            .unwrap_or_default(),
                        preview: String::new(),
                        occurrences: vec![DuplicateSpanOccurrence {
                            repo_id: file.repo_id,
                            repo_label: file.repo_label.clone(),
                            path: file.path.clone(),
                            start_line: node.start_line,
                            end_line: node.end_line,
                        }],
                        occurrence_keys: HashSet::from([(file_id, node.start_token)]),
                        repo_ids: HashSet::from([file.repo_id]),
                    });
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

    let mut out = finalize_span_groups(groups, options.cross_repo_only);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    fill_missing_previews_from_files(files, &mut out, 120);
    out
}

fn detect_duplicate_span_groups_with_len_filter<'a>(
    files: &[NormalizedFileView<'a>],
    winnowing: WinnowingParams,
    max_items: usize,
    accept_match: impl Fn(usize, usize, usize) -> bool,
    preview_from_occurrence: impl Fn(usize, u32, u32) -> String,
    stats: &mut ScanStats,
) -> Vec<DuplicateSpanGroup> {
    if max_items == 0 || files.is_empty() {
        return Vec::new();
    }

    let mut out = detect_duplicate_span_groups_winnowing(
        files,
        winnowing,
        accept_match,
        |file_id, start_line, end_line, _sample| {
            preview_from_occurrence(file_id, start_line, end_line)
        },
        stats,
    );
    sort_span_groups_for_report(&mut out);
    out.truncate(max_items);
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
