use std::io;
use std::path::PathBuf;

use crate::dedupe::FileDuplicateGrouper;
use crate::scan::{
    Repo, make_rel_path, read_repo_file_bytes_for_verification, read_repo_file_bytes_with_path,
    repo_label, visit_repo_files,
};
use crate::tokenize::{parse_brace_blocks, tokenize_for_dup_detection};
use crate::types::{DuplicateFile, DuplicateGroup, ScanOptions, ScanStats};
use crate::util::{fnv1a64_u32, fold_u64_to_u32, normalize_for_code_spans};

use super::ScannedTextFile;
use super::util::sort_duplicate_groups_for_report;

const DEFAULT_REPORT_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

pub(super) fn scan_text_files_for_report(
    roots: &[PathBuf],
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<(Vec<ScannedTextFile>, Vec<DuplicateGroup>)> {
    let mut scan_options = options.clone();
    scan_options.max_total_bytes = Some(
        scan_options
            .max_total_bytes
            .unwrap_or(DEFAULT_REPORT_MAX_TOTAL_BYTES),
    );

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: repo_label(root, id),
        })
        .collect();

    let canonical_roots = if options.follow_symlinks {
        Some(
            repos
                .iter()
                .map(|repo| repo.root.canonicalize())
                .collect::<io::Result<Vec<_>>>()?,
        )
    } else {
        None
    };

    let mut file_groups = FileDuplicateGrouper::default();
    let mut files = Vec::new();

    for repo in &repos {
        let canonical_root = canonical_roots
            .as_ref()
            .map(|roots| roots[repo.id].as_path());

        if let std::ops::ControlFlow::Break(()) =
            visit_repo_files(repo, &scan_options, stats, |stats, repo_file| {
                let Some((bytes, read_path)) = read_repo_file_bytes_with_path(
                    &repo_file,
                    canonical_root,
                    &scan_options,
                    stats,
                )?
                else {
                    return Ok(std::ops::ControlFlow::Continue(()));
                };

                let rel_path = make_rel_path(&repo.root, &repo_file.abs_path);

                // 1) File duplicates (whitespace-insensitive)
                let file = DuplicateFile {
                    repo_id: repo.id,
                    repo_label: repo.label.clone(),
                    path: rel_path.clone(),
                };
                file_groups.push_bytes(&bytes, file);

                // 2) Text-based detectors
                let text = String::from_utf8_lossy(&bytes);
                let code_norm = normalize_for_code_spans(&bytes);
                let line_norm = normalize_lines_for_dup_detection(&bytes);
                let tokenized = tokenize_for_dup_detection(&text);
                let blocks = parse_brace_blocks(&tokenized.tokens, &tokenized.token_lines);

                files.push(ScannedTextFile {
                    repo_id: repo.id,
                    repo_label: repo.label.clone(),
                    path: rel_path,
                    abs_path: read_path,
                    code_chars: code_norm.chars,
                    code_char_lines: code_norm.line_map,
                    line_tokens: line_norm.line_tokens,
                    line_token_lines: line_norm.line_lines,
                    line_token_char_lens: line_norm.line_lens,
                    tokens: tokenized.tokens,
                    token_lines: tokenized.token_lines,
                    blocks,
                });

                Ok(std::ops::ControlFlow::Continue(()))
            })?
        {
            break;
        }
    }

    let follow_symlinks = scan_options.follow_symlinks;
    let max_file_size = scan_options.max_file_size;
    let canonical_roots = canonical_roots.as_deref();
    let mut file_duplicates =
        file_groups.into_groups_verified(options.cross_repo_only, |file| {
            let Some(repo) = repos.get(file.repo_id) else {
                return Ok(None);
            };
            let canonical_root = canonical_roots
                .and_then(|roots| roots.get(file.repo_id))
                .map(|p| p.as_path());

            read_repo_file_bytes_for_verification(
                &repo.root,
                &file.path,
                canonical_root,
                follow_symlinks,
                max_file_size,
            )
        })?;

    sort_duplicate_groups_for_report(&mut file_duplicates);
    file_duplicates.truncate(options.max_report_items);

    Ok((files, file_duplicates))
}

#[derive(Debug)]
struct LineNormalizedText {
    line_tokens: Vec<u32>,
    line_lines: Vec<u32>,
    line_lens: Vec<usize>,
}

fn normalize_lines_for_dup_detection(bytes: &[u8]) -> LineNormalizedText {
    let mut line: u32 = 1;
    let mut current: Vec<u32> = Vec::new();

    let mut line_tokens = Vec::new();
    let mut line_lines = Vec::new();
    let mut line_lens = Vec::new();

    for &b in bytes {
        if b == b'\n' {
            if !current.is_empty() {
                line_lens.push(current.len());
                line_tokens.push(fold_u64_to_u32(fnv1a64_u32(&current)));
                line_lines.push(line);
            }
            current.clear();
            line = line.saturating_add(1);
            continue;
        }
        if b.is_ascii_alphanumeric() || b == b'_' {
            current.push(u32::from(b));
        }
    }

    if !current.is_empty() {
        line_lens.push(current.len());
        line_tokens.push(fold_u64_to_u32(fnv1a64_u32(&current)));
        line_lines.push(line);
    }

    LineNormalizedText {
        line_tokens,
        line_lines,
        line_lens,
    }
}
