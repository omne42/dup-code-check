use std::collections::{HashMap, HashSet};
use std::io;

use crate::types::{DuplicateFile, DuplicateGroup, DuplicateSpanGroup, ScanOptions, ScanStats};
use crate::util::{NormalizedFileView, fnv1a64, make_preview, whitespace_insensitive_fingerprint};
use crate::winnowing::{WinnowingParams, detect_duplicate_span_groups_winnowing};

type FileDuplicateKey = (u64, usize, u64, [u8; 16], [u8; 16]);

#[derive(Debug)]
struct FileGroupBuilder {
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
                        files: vec![file],
                        repo_ids,
                    },
                );
            }
        }
    }

    pub(crate) fn into_groups_verified<R>(
        self,
        cross_repo_only: bool,
        mut read_bytes: R,
    ) -> io::Result<Vec<DuplicateGroup>>
    where
        R: FnMut(&DuplicateFile) -> io::Result<Option<Vec<u8>>>,
    {
        fn normalize_ascii_whitespace(bytes: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(bytes.len());
            for &b in bytes {
                if !b.is_ascii_whitespace() {
                    out.push(b);
                }
            }
            out
        }

        let mut out = Vec::new();
        for builder in self.groups.into_values() {
            if builder.files.len() <= 1 {
                continue;
            }
            if cross_repo_only && builder.repo_ids.len() < 2 {
                continue;
            }

            let mut verified: Vec<(Vec<u8>, Vec<DuplicateFile>, HashSet<usize>)> = Vec::new();
            for file in builder.files {
                let Some(bytes) = read_bytes(&file)? else {
                    continue;
                };
                if bytes.contains(&0) {
                    continue;
                }
                let normalized = normalize_ascii_whitespace(&bytes);

                let Some((_, files, repo_ids)) =
                    verified.iter_mut().find(|(n, _, _)| *n == normalized)
                else {
                    let mut repo_ids = HashSet::new();
                    repo_ids.insert(file.repo_id);
                    verified.push((normalized, vec![file], repo_ids));
                    continue;
                };

                repo_ids.insert(file.repo_id);
                files.push(file);
            }

            for (normalized, mut files, repo_ids) in verified {
                if files.len() <= 1 {
                    continue;
                }
                if cross_repo_only && repo_ids.len() < 2 {
                    continue;
                }

                let content_hash = fnv1a64(&normalized);
                let normalized_len = normalized.len();

                files.sort_by(|a, b| (a.repo_id, &a.path).cmp(&(b.repo_id, &b.path)));
                out.push(DuplicateGroup {
                    content_hash,
                    normalized_len,
                    files,
                });
            }
        }

        Ok(out)
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
        WinnowingParams {
            min_len: min_match_len,
            fingerprint_len,
            window_size,
            cross_repo_only: options.cross_repo_only,
        },
        |_file_id, _start, _len| true,
        |_file_id, _start_line, _end_line, sample| make_preview(sample, 80),
        stats,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn file_duplicates_are_verified_against_bytes() {
        let mut groups = FileDuplicateGrouper::default();
        groups.push_bytes(
            b"abc",
            DuplicateFile {
                repo_id: 0,
                repo_label: "repo0".to_string(),
                path: "a.txt".to_string(),
            },
        );
        groups.push_bytes(
            b"abc",
            DuplicateFile {
                repo_id: 0,
                repo_label: "repo0".to_string(),
                path: "b.txt".to_string(),
            },
        );

        let mut content = HashMap::new();
        content.insert("a.txt".to_string(), b"abc".to_vec());
        // Simulate a file changing between scan and verification.
        content.insert("b.txt".to_string(), b"xyz".to_vec());

        let verified = groups
            .into_groups_verified(false, |file| Ok(content.get(&file.path).cloned()))
            .expect("verification should not fail");

        assert!(verified.is_empty());
    }
}
