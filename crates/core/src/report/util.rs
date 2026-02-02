use crate::types::{DuplicateGroup, DuplicateSpanGroup};

pub(super) fn preview_from_lines(
    text: &str,
    start_line: u32,
    end_line: u32,
    max_chars: usize,
) -> String {
    let mut out = String::new();
    for (idx, line) in text.lines().enumerate() {
        let line_no = (idx as u32) + 1;
        if line_no < start_line {
            continue;
        }
        if line_no > end_line {
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
        if out.len() >= max_chars {
            out.truncate(max_chars);
            break;
        }
    }
    out
}

pub(super) fn sort_duplicate_groups_for_report(groups: &mut [DuplicateGroup]) {
    groups.sort_by(|a, b| {
        b.files
            .len()
            .cmp(&a.files.len())
            .then_with(|| b.normalized_len.cmp(&a.normalized_len))
            .then_with(|| a.content_hash.cmp(&b.content_hash))
    });
}

pub(super) fn sort_span_groups_for_report(groups: &mut [DuplicateSpanGroup]) {
    groups.sort_by(|a, b| {
        b.occurrences
            .len()
            .cmp(&a.occurrences.len())
            .then_with(|| b.normalized_len.cmp(&a.normalized_len))
            .then_with(|| a.content_hash.cmp(&b.content_hash))
    });
}
