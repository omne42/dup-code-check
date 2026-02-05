use std::sync::Arc;

use crate::types::{DuplicateSpanGroup, ScanOptions, ScanStats};
use crate::util::NormalizedFileView;
use crate::winnowing::WinnowingParams;

use super::super::ScannedTextFile;
use super::super::util::fill_missing_previews_from_files;
use super::repo_label_arc;
use super::span_groups::detect_duplicate_span_groups_with_len_filter;

pub(in crate::report) fn detect_duplicate_token_spans(
    repo_labels: &[Arc<str>],
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
            repo_label: repo_label_arc(repo_labels, file.repo_id),
            rel_path: Arc::clone(&file.path),
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
