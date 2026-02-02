use std::collections::{HashMap, HashSet};
use std::io;
use std::path::PathBuf;

use crate::scan::{
    Repo, make_rel_path, read_repo_file_bytes, repo_label, validate_roots, visit_repo_files,
};
use crate::types::{
    DuplicateFile, DuplicateGroup, DuplicateSpanGroup, ScanOptions, ScanOutcome, ScanStats,
};
use crate::util::{
    NormalizedFile, SpanGroupBuilder, add_occurrence, canonicalize_match, fnv1a64, fnv1a64_u32,
    make_preview, maximal_match, normalize_for_code_spans, normalize_whitespace,
    winnowed_fingerprints,
};

pub fn find_duplicate_files(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<Vec<DuplicateGroup>> {
    Ok(find_duplicate_files_with_stats(roots, options)?.result)
}

pub fn find_duplicate_files_with_stats(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<ScanOutcome<Vec<DuplicateGroup>>> {
    if roots.is_empty() {
        return Ok(ScanOutcome {
            result: Vec::new(),
            stats: ScanStats::default(),
        });
    }

    validate_roots(roots)?;

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: repo_label(root, id),
        })
        .collect();

    let canonical_roots = if options.follow_symlinks {
        Some(
            repos
                .iter()
                .map(|repo| repo.root.canonicalize())
                .collect::<io::Result<Vec<_>>>()?,
        )
    } else {
        None
    };

    let mut stats = ScanStats::default();

    #[derive(Debug)]
    struct GroupBuilder {
        content_hash: u64,
        normalized_len: usize,
        sample: Vec<u8>,
        files: Vec<DuplicateFile>,
        repo_ids: HashSet<usize>,
    }

    let mut groups: HashMap<(u64, usize), Vec<GroupBuilder>> = HashMap::new();

    for repo in &repos {
        let canonical_root = canonical_roots
            .as_ref()
            .map(|roots| roots[repo.id].as_path());

        if let std::ops::ControlFlow::Break(()) =
            visit_repo_files(repo, options, &mut stats, |stats, repo_file| {
                let Some(bytes) = read_repo_file_bytes(&repo_file, canonical_root, options, stats)?
                else {
                    return Ok(std::ops::ControlFlow::Continue(()));
                };

                let normalized = normalize_whitespace(&bytes);
                let content_hash = fnv1a64(&normalized);

                let key = (content_hash, normalized.len());
                let bucket = groups.entry(key).or_default();

                let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);
                let file = DuplicateFile {
                    repo_id: repo_file.repo_id,
                    repo_label: repo_file.repo_label.clone(),
                    path: rel_path,
                };

                if let Some(existing) = bucket.iter_mut().find(|g| g.sample == normalized) {
                    existing.repo_ids.insert(file.repo_id);
                    existing.files.push(file);
                    return Ok(std::ops::ControlFlow::Continue(()));
                }

                let mut repo_ids = HashSet::new();
                repo_ids.insert(file.repo_id);
                bucket.push(GroupBuilder {
                    content_hash,
                    normalized_len: normalized.len(),
                    sample: normalized,
                    files: vec![file],
                    repo_ids,
                });

                Ok(std::ops::ControlFlow::Continue(()))
            })?
        {
            break;
        }
    }

    let mut out = Vec::new();
    for builders in groups.into_values() {
        for mut builder in builders {
            if builder.files.len() <= 1 {
                continue;
            }
            if options.cross_repo_only && builder.repo_ids.len() < 2 {
                continue;
            }

            builder
                .files
                .sort_by(|a, b| (a.repo_id, &a.path).cmp(&(b.repo_id, &b.path)));
            out.push(DuplicateGroup {
                content_hash: builder.content_hash,
                normalized_len: builder.normalized_len,
                files: builder.files,
            });
        }
    }

    out.sort_by(|a, b| {
        (a.content_hash, a.normalized_len, a.files.len()).cmp(&(
            b.content_hash,
            b.normalized_len,
            b.files.len(),
        ))
    });
    Ok(ScanOutcome { result: out, stats })
}

pub fn find_duplicate_code_spans(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<Vec<DuplicateSpanGroup>> {
    Ok(find_duplicate_code_spans_with_stats(roots, options)?.result)
}

pub fn find_duplicate_code_spans_with_stats(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<ScanOutcome<Vec<DuplicateSpanGroup>>> {
    if roots.is_empty() {
        return Ok(ScanOutcome {
            result: Vec::new(),
            stats: ScanStats::default(),
        });
    }

    validate_roots(roots)?;

    let min_match_len = options.min_match_len.max(1);
    let fingerprint_len = min_match_len.clamp(1, 25);
    let window_size = min_match_len
        .saturating_sub(fingerprint_len)
        .saturating_add(1);

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: repo_label(root, id),
        })
        .collect();

    let canonical_roots = if options.follow_symlinks {
        Some(
            repos
                .iter()
                .map(|repo| repo.root.canonicalize())
                .collect::<io::Result<Vec<_>>>()?,
        )
    } else {
        None
    };

    let mut stats = ScanStats::default();
    let mut files = Vec::new();

    for repo in &repos {
        let canonical_root = canonical_roots
            .as_ref()
            .map(|roots| roots[repo.id].as_path());

        if let std::ops::ControlFlow::Break(()) =
            visit_repo_files(repo, options, &mut stats, |stats, repo_file| {
                let Some(bytes) = read_repo_file_bytes(&repo_file, canonical_root, options, stats)?
                else {
                    return Ok(std::ops::ControlFlow::Continue(()));
                };

                let normalized = normalize_for_code_spans(&bytes);
                if normalized.chars.len() < min_match_len {
                    return Ok(std::ops::ControlFlow::Continue(()));
                }

                let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);
                files.push(NormalizedFile {
                    repo_id: repo_file.repo_id,
                    repo_label: repo_file.repo_label,
                    rel_path,
                    normalized: normalized.chars,
                    line_map: normalized.line_map,
                });

                Ok(std::ops::ControlFlow::Continue(()))
            })?
        {
            break;
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct FingerprintOcc {
        file_id: usize,
        pos: usize,
    }

    let mut fingerprints: HashMap<u64, Vec<FingerprintOcc>> = HashMap::new();
    for (file_id, file) in files.iter().enumerate() {
        for (hash, pos) in winnowed_fingerprints(&file.normalized, fingerprint_len, window_size) {
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

    fn truncate_bucket_by_repo(
        mut occs: Vec<FingerprintOcc>,
        files: &[NormalizedFile],
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
            occs = truncate_bucket_by_repo(occs, &files, MAX_BUCKET);
        }

        for i in 0..occs.len() {
            for j in (i + 1)..occs.len() {
                let a = occs[i];
                let b = occs[j];
                if a.file_id == b.file_id && a.pos == b.pos {
                    continue;
                }
                if options.cross_repo_only && files[a.file_id].repo_id == files[b.file_id].repo_id {
                    continue;
                }

                let (start_a, start_b, len) = match maximal_match(
                    &files[a.file_id].normalized,
                    a.pos,
                    &files[b.file_id].normalized,
                    b.pos,
                    fingerprint_len,
                ) {
                    Some(v) => v,
                    None => continue,
                };

                if len < min_match_len {
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

                add_occurrence(builder, &files[file_a], file_a, start_a, len);
                add_occurrence(builder, &files[file_b], file_b, start_b, len);
            }
        }
    }

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
                preview: make_preview(&builder.sample, 80),
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
    Ok(ScanOutcome { result: out, stats })
}
