use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;

use crate::types::{DuplicateGroup, DuplicateSpanGroup};

use super::ScannedTextFile;

fn preview_from_file_lines(
    path: &Path,
    start_line: u32,
    end_line: u32,
    max_chars: usize,
) -> String {
    if start_line == 0 || end_line == 0 || start_line > end_line || max_chars == 0 {
        return String::new();
    }

    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut reader = BufReader::new(file);

    let mut out = String::new();
    let mut line_no: u32 = 1;
    let mut buf: Vec<u8> = Vec::new();

    loop {
        buf.clear();
        let n = match reader.read_until(b'\n', &mut buf) {
            Ok(n) => n,
            Err(_) => break,
        };
        if n == 0 {
            break;
        }

        if line_no >= start_line && line_no <= end_line {
            let mut slice = buf.as_slice();
            if slice.ends_with(b"\n") {
                slice = &slice[..slice.len() - 1];
            }
            if slice.ends_with(b"\r") {
                slice = &slice[..slice.len() - 1];
            }

            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(std::string::String::from_utf8_lossy(slice).as_ref());
            if out.len() >= max_chars {
                out.truncate(max_chars);
                break;
            }
        }

        if line_no >= end_line {
            break;
        }
        line_no = line_no.saturating_add(1);
    }

    out
}

pub(super) fn fill_missing_previews_from_files(
    files: &[ScannedTextFile],
    groups: &mut [DuplicateSpanGroup],
    max_chars: usize,
) {
    if groups.is_empty() || max_chars == 0 {
        return;
    }

    let mut by_path: HashMap<(usize, &str), &Path> = HashMap::new();
    for file in files {
        by_path.insert((file.repo_id, file.path.as_str()), file.abs_path.as_path());
    }

    for group in groups {
        if !group.preview.is_empty() {
            continue;
        }
        let Some(occ) = group.occurrences.first() else {
            continue;
        };
        let Some(path) = by_path.get(&(occ.repo_id, occ.path.as_str())) else {
            continue;
        };

        group.preview = preview_from_file_lines(path, occ.start_line, occ.end_line, max_chars);
    }
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
