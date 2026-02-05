use crate::types::{DuplicateSpanGroup, ScanStats};
use crate::util::NormalizedFileView;
use crate::winnowing::{WinnowingParams, detect_duplicate_span_groups_winnowing};

use super::super::util::sort_span_groups_for_report;

pub(super) fn detect_duplicate_span_groups_with_len_filter<'a>(
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
