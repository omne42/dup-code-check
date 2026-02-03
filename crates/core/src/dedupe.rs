use std::collections::{HashMap, HashSet};

use crate::types::{DuplicateFile, DuplicateGroup, DuplicateSpanGroup, ScanOptions};
use crate::util::{
    NormalizedFileView, SpanGroupBuilder, add_occurrence_view, canonicalize_match, fnv1a64,
    fnv1a64_u32, make_preview, maximal_match, winnowed_fingerprints,
};

#[derive(Debug)]
struct FileGroupBuilder {
    content_hash: u64,
    normalized_len: usize,
    sample: Vec<u8>,
    files: Vec<DuplicateFile>,
    repo_ids: HashSet<usize>,
}

#[derive(Debug, Default)]
pub(crate) struct FileDuplicateGrouper {
    groups: HashMap<(u64, usize), Vec<FileGroupBuilder>>,
}

impl FileDuplicateGrouper {
    pub(crate) fn push(&mut self, normalized: Vec<u8>, file: DuplicateFile) {
        let content_hash = fnv1a64(&normalized);
        let key = (content_hash, normalized.len());
        let bucket = self.groups.entry(key).or_default();

        if let Some(existing) = bucket.iter_mut().find(|g| g.sample == normalized) {
            existing.repo_ids.insert(file.repo_id);
            existing.files.push(file);
            return;
        }

        let mut repo_ids = HashSet::new();
        repo_ids.insert(file.repo_id);
        bucket.push(FileGroupBuilder {
            content_hash,
            normalized_len: normalized.len(),
            sample: normalized,
            files: vec![file],
            repo_ids,
        });
    }

    pub(crate) fn into_groups(self, cross_repo_only: bool) -> Vec<DuplicateGroup> {
        let mut out = Vec::new();
        for builders in self.groups.into_values() {
            for builder in builders {
                if builder.files.len() <= 1 {
                    continue;
                }
                if cross_repo_only && builder.repo_ids.len() < 2 {
                    continue;
                }

                let mut files = builder.files;
                files.sort_by(|a, b| (a.repo_id, &a.path).cmp(&(b.repo_id, &b.path)));
                out.push(DuplicateGroup {
                    content_hash: builder.content_hash,
                    normalized_len: builder.normalized_len,
                    files,
                });
            }
        }
        out
    }
}

pub(crate) fn detect_duplicate_code_spans_winnowing<'a>(
    files: &[NormalizedFileView<'a>],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    if files.is_empty() {
        return Vec::new();
    }

    let min_match_len = options.min_match_len.max(1);
    let fingerprint_len = min_match_len.clamp(1, 25);
    let window_size = min_match_len
        .saturating_sub(fingerprint_len)
        .saturating_add(1);

    #[derive(Debug, Clone, Copy)]
    struct FingerprintOcc {
        file_id: usize,
        pos: usize,
    }

    let mut fingerprints: HashMap<u64, Vec<FingerprintOcc>> = HashMap::new();
    for (file_id, file) in files.iter().enumerate() {
        if file.normalized.len() < min_match_len {
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
                if options.cross_repo_only && files[a.file_id].repo_id == files[b.file_id].repo_id {
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

                let sample_slice = &files[file_a].normalized[start_a..start_a + len];
                let content_hash = fnv1a64_u32(sample_slice);

                let bucket = groups.entry((content_hash, len)).or_default();
                let builder = match bucket
                    .iter_mut()
                    .find(|g| g.sample.as_slice() == sample_slice)
                {
                    Some(existing) => existing,
                    None => {
                        bucket.push(SpanGroupBuilder {
                            content_hash,
                            normalized_len: len,
                            sample: sample_slice.to_vec(),
                            preview: make_preview(sample_slice, 80),
                            occurrences: Vec::new(),
                            occurrence_keys: HashSet::new(),
                            repo_ids: HashSet::new(),
                        });
                        bucket.last_mut().expect("just pushed")
                    }
                };

                add_occurrence_view(builder, &files[file_a], file_a, start_a, len);
                add_occurrence_view(builder, &files[file_b], file_b, start_b, len);
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
