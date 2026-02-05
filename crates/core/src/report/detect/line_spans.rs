use std::sync::Arc;

use crate::types::{DuplicateSpanGroup, ScanOptions, ScanStats};
use crate::util::NormalizedFileView;
use crate::winnowing::WinnowingParams;

use super::super::ScannedTextFile;
use super::super::util::fill_missing_previews_from_files;
use super::repo_label_arc;
use super::span_groups::detect_duplicate_span_groups_with_len_filter;

pub(in crate::report) fn detect_duplicate_line_spans(
    repo_labels: &[Arc<str>],
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
            repo_label: repo_label_arc(repo_labels, file.repo_id),
            rel_path: Arc::clone(&file.path),
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
