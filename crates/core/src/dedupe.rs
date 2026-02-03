use std::collections::{HashMap, HashSet};

use crate::types::{DuplicateFile, DuplicateGroup, DuplicateSpanGroup, ScanOptions, ScanStats};
use crate::util::{NormalizedFileView, make_preview, whitespace_insensitive_fingerprint};
use crate::winnowing::detect_duplicate_span_groups_winnowing;

type FileDuplicateKey = (u64, usize, u64, [u8; 16], [u8; 16]);

#[derive(Debug)]
struct FileGroupBuilder {
    content_hash: u64,
    normalized_len: usize,
    files: Vec<DuplicateFile>,
    repo_ids: HashSet<usize>,
}

#[derive(Debug, Default)]
pub(crate) struct FileDuplicateGrouper {
    groups: HashMap<FileDuplicateKey, FileGroupBuilder>,
}

impl FileDuplicateGrouper {
    pub(crate) fn push_bytes(&mut self, bytes: &[u8], file: DuplicateFile) {
        let fp = whitespace_insensitive_fingerprint(bytes);
        let key = (
            fp.content_hash,
            fp.normalized_len,
            fp.content_hash2,
            fp.prefix,
            fp.suffix,
        );

        match self.groups.get_mut(&key) {
            Some(existing) => {
                existing.repo_ids.insert(file.repo_id);
                existing.files.push(file);
            }
            None => {
                let mut repo_ids = HashSet::new();
                repo_ids.insert(file.repo_id);
                self.groups.insert(
                    key,
                    FileGroupBuilder {
                        content_hash: fp.content_hash,
                        normalized_len: fp.normalized_len,
                        files: vec![file],
                        repo_ids,
                    },
                );
            }
        }
    }

    pub(crate) fn into_groups(self, cross_repo_only: bool) -> Vec<DuplicateGroup> {
        let mut out = Vec::new();
        for builder in self.groups.into_values() {
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
        out
    }
}

pub(crate) fn detect_duplicate_code_spans_winnowing<'a>(
    files: &[NormalizedFileView<'a>],
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> Vec<DuplicateSpanGroup> {
    let min_match_len = options.min_match_len.max(1);
    let fingerprint_len = min_match_len.clamp(1, 25);
    let window_size = min_match_len
        .saturating_sub(fingerprint_len)
        .saturating_add(1);

    detect_duplicate_span_groups_winnowing(
        files,
        min_match_len,
        fingerprint_len,
        window_size,
        options.cross_repo_only,
        |_file_id, _start, _len| true,
        |_file_id, _start_line, _end_line, sample| make_preview(sample, 80),
        stats,
    )
}
