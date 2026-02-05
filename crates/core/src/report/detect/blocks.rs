use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::types::{DuplicateSpanGroup, DuplicateSpanOccurrence, ScanOptions};
use crate::util::{SpanGroupBuilder, fnv1a64_u32, fold_u64_to_u32};
use crate::winnowing::finalize_span_groups;

use super::super::ScannedTextFile;
use super::super::util::{fill_missing_previews_from_files, sort_span_groups_for_report};
use super::repo_label_arc;

pub(in crate::report) fn detect_duplicate_blocks(
    repo_labels: &[Arc<str>],
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
                            repo_label: repo_label_arc(repo_labels, file.repo_id),
                            path: Arc::clone(&file.path),
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
                repo_label: repo_label_arc(repo_labels, file.repo_id),
                path: Arc::clone(&file.path),
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

pub(in crate::report) fn detect_duplicate_ast_subtrees(
    repo_labels: &[Arc<str>],
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
                            repo_label: repo_label_arc(repo_labels, file.repo_id),
                            path: Arc::clone(&file.path),
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
                repo_label: repo_label_arc(repo_labels, file.repo_id),
                path: Arc::clone(&file.path),
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
