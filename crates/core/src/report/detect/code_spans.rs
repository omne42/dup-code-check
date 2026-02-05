use std::sync::Arc;

use crate::dedupe::detect_duplicate_code_spans_winnowing;
use crate::types::{DuplicateSpanGroup, ScanOptions, ScanStats};
use crate::util::NormalizedCodeFileView;

use super::super::ScannedTextFile;
use super::super::util::sort_span_groups_for_report;
use super::repo_label_arc;

pub(in crate::report) fn detect_duplicate_code_spans(
    repo_labels: &[Arc<str>],
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
        normalized.push(NormalizedCodeFileView {
            repo_id: file.repo_id,
            repo_label: repo_label_arc(repo_labels, file.repo_id),
            rel_path: Arc::clone(&file.path),
            normalized: &file.code_chars,
            line_starts: &file.code_line_starts,
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
