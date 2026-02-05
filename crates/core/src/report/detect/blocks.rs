use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::types::{DuplicateSpanGroup, DuplicateSpanOccurrence, ScanOptions};
use crate::util::fnv1a64_u32;

use super::super::ScannedTextFile;
use super::super::util::{fill_missing_previews_from_files, sort_span_groups_for_report};
use super::repo_label_arc;

#[derive(Debug, Clone, Copy)]
struct SampleRef {
    file_id: usize,
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct ReportSpanGroupBuilder {
    content_hash: u64,
    normalized_len: usize,
    preview: String,
    occurrences: Vec<DuplicateSpanOccurrence>,
    occurrence_keys: HashSet<(usize, usize)>,
    repo_ids: HashSet<usize>,
    sample_ref: Option<SampleRef>,
}

fn finalize_report_span_groups(
    groups: impl IntoIterator<Item = ReportSpanGroupBuilder>,
    cross_repo_only: bool,
) -> Vec<DuplicateSpanGroup> {
    let mut out = Vec::new();
    for mut builder in groups {
        if builder.occurrences.len() <= 1 {
            continue;
        }
        if cross_repo_only && builder.repo_ids.len() < 2 {
            continue;
        }

        builder.occurrences.sort_by(|a, b| {
            (
                a.repo_id,
                a.repo_label.as_ref(),
                a.path.as_ref(),
                a.start_line,
                a.end_line,
            )
                .cmp(&(
                    b.repo_id,
                    b.repo_label.as_ref(),
                    b.path.as_ref(),
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
    out
}

pub(in crate::report) fn detect_duplicate_blocks(
    repo_labels: &[Arc<str>],
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);

    let mut groups: HashMap<(u64, usize), Vec<ReportSpanGroupBuilder>> = HashMap::new();

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

            let builder = match bucket.iter_mut().find(|g| {
                let Some(sample_ref) = g.sample_ref else {
                    return false;
                };
                let repr_file = &files[sample_ref.file_id];
                let repr = &repr_file.tokens[sample_ref.start..sample_ref.end];
                repr == slice
            }) {
                Some(existing) => existing,
                None => {
                    bucket.push(ReportSpanGroupBuilder {
                        content_hash,
                        normalized_len: slice.len(),
                        preview: String::new(),
                        occurrences: vec![DuplicateSpanOccurrence {
                            repo_id: file.repo_id,
                            repo_label: repo_label_arc(repo_labels, file.repo_id),
                            path: Arc::clone(&file.path),
                            start_line: node.start_line,
                            end_line: node.end_line,
                        }],
                        occurrence_keys: HashSet::from([(file_id, node.start_token)]),
                        repo_ids: HashSet::from([file.repo_id]),
                        sample_ref: Some(SampleRef {
                            file_id,
                            start,
                            end: node.end_token,
                        }),
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
                repo_label: repo_label_arc(repo_labels, file.repo_id),
                path: Arc::clone(&file.path),
                start_line: node.start_line,
                end_line: node.end_line,
            });
        }
    }

    let mut out =
        finalize_report_span_groups(groups.into_values().flatten(), options.cross_repo_only);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    fill_missing_previews_from_files(files, &mut out, 120);
    out
}

pub(in crate::report) fn detect_duplicate_ast_subtrees(
    repo_labels: &[Arc<str>],
    files: &[ScannedTextFile],
    options: &ScanOptions,
) -> Vec<DuplicateSpanGroup> {
    let min_token_len = options.min_token_len.max(1);

    let mut groups: HashMap<(u64, usize, u64), ReportSpanGroupBuilder> = HashMap::new();

    for (file_id, file) in files.iter().enumerate() {
        let mut hashes: Vec<Option<u64>> = vec![None; file.blocks.len()];
        let mut by_depth: Vec<usize> = (0..file.blocks.len()).collect();
        by_depth.sort_by_key(|&i| std::cmp::Reverse(file.blocks[i].depth));

        for node_id in by_depth {
            let node = &file.blocks[node_id];
            let start = node.start_token.saturating_add(1);
            if node.end_token <= start {
                continue;
            }

            const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
            const FNV_PRIME: u64 = 0x100000001b3;
            const BASE: u64 = 911382323;
            // Marker inserted before each child hash in the subtree signature.
            // Keep it outside the tokenizer's normal token range to avoid ambiguity.
            // Today tokens are < `TOK_PUNCT_BASE + 255` (see `tokenize.rs`), so 50_000 is safe.
            const CHILD_MARKER: u32 = 50_000;

            let mut hash1 = FNV_OFFSET_BASIS;
            let mut hash2 = 0u64;
            let mut repr_len = 0usize;

            fn push_u32(hash1: &mut u64, hash2: &mut u64, repr_len: &mut usize, v: u32) {
                for b in v.to_le_bytes() {
                    *hash1 ^= u64::from(b);
                    *hash1 = hash1.wrapping_mul(FNV_PRIME);
                }
                *hash2 = hash2
                    .wrapping_mul(BASE)
                    .wrapping_add(u64::from(v).wrapping_add(1));
                *repr_len = repr_len.saturating_add(1);
            }

            fn push_u64(hash1: &mut u64, hash2: &mut u64, repr_len: &mut usize, v: u64) {
                for b in v.to_le_bytes() {
                    *hash1 ^= u64::from(b);
                    *hash1 = hash1.wrapping_mul(FNV_PRIME);
                }
                *hash2 = hash2.wrapping_mul(BASE).wrapping_add(v.wrapping_add(1));
                *repr_len = repr_len.saturating_add(1);
            }

            let mut idx = start;
            // `parse_brace_blocks()` records children in token order, so `node.children` is already
            // sorted by `start_token`.
            let mut prev_child_start: Option<usize> = None;
            for &cid in &node.children {
                let c = &file.blocks[cid];
                let c_start = c.start_token;
                let c_end = c.end_token;
                if let Some(prev) = prev_child_start {
                    debug_assert!(c_start >= prev, "children must be in token order");
                }
                prev_child_start = Some(c_start);

                while idx < c_start && idx < node.end_token {
                    push_u32(&mut hash1, &mut hash2, &mut repr_len, file.tokens[idx]);
                    idx += 1;
                }
                if idx == c_start {
                    let child_hash = hashes[cid].unwrap_or(0);
                    push_u32(&mut hash1, &mut hash2, &mut repr_len, CHILD_MARKER);
                    push_u64(&mut hash1, &mut hash2, &mut repr_len, child_hash);
                    idx = c_end.saturating_add(1);
                }
            }
            while idx < node.end_token {
                push_u32(&mut hash1, &mut hash2, &mut repr_len, file.tokens[idx]);
                idx += 1;
            }

            hashes[node_id] = Some(hash1);

            if repr_len < min_token_len {
                continue;
            }

            // NOTE: We use (hash1, len, hash2) as an approximate equivalence key to avoid
            // materializing the full subtree representation. Collisions are theoretically
            // possible, but should be vanishingly unlikely with two independent 64-bit hashes
            // plus length and full 64-bit child hashes included in the parent representation.
            let content_hash = hash1;
            let key = (content_hash, repr_len, hash2);
            let builder = groups.entry(key).or_insert_with(|| ReportSpanGroupBuilder {
                content_hash,
                normalized_len: repr_len,
                preview: String::new(),
                occurrences: vec![DuplicateSpanOccurrence {
                    repo_id: file.repo_id,
                    repo_label: repo_label_arc(repo_labels, file.repo_id),
                    path: Arc::clone(&file.path),
                    start_line: node.start_line,
                    end_line: node.end_line,
                }],
                occurrence_keys: HashSet::from([(file_id, node.start_token)]),
                repo_ids: HashSet::from([file.repo_id]),
                sample_ref: None,
            });

            if !builder.occurrence_keys.insert((file_id, node.start_token)) {
                continue;
            }
            builder.repo_ids.insert(file.repo_id);
            builder.occurrences.push(DuplicateSpanOccurrence {
                repo_id: file.repo_id,
                repo_label: repo_label_arc(repo_labels, file.repo_id),
                path: Arc::clone(&file.path),
                start_line: node.start_line,
                end_line: node.end_line,
            });
        }
    }

    let mut out = finalize_report_span_groups(groups.into_values(), options.cross_repo_only);
    sort_span_groups_for_report(&mut out);
    out.truncate(options.max_report_items);
    fill_missing_previews_from_files(files, &mut out, 120);
    out
}
