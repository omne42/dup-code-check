use std::io;
use std::path::PathBuf;

use crate::dedupe::FileDuplicateGrouper;
use crate::scan::{Repo, make_rel_path, read_repo_file_bytes, repo_label, visit_repo_files};
use crate::tokenize::{parse_brace_blocks, tokenize_for_dup_detection};
use crate::types::{DuplicateFile, DuplicateGroup, ScanOptions, ScanStats};
use crate::util::{fnv1a64_u32, fold_u64_to_u32, normalize_for_code_spans, normalize_whitespace};

use super::ScannedTextFile;
use super::util::sort_duplicate_groups_for_report;

pub(super) fn scan_text_files_for_report(
    roots: &[PathBuf],
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<(Vec<ScannedTextFile>, Vec<DuplicateGroup>)> {
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
            visit_repo_files(repo, options, stats, |stats, repo_file| {
                let Some(bytes) = read_repo_file_bytes(&repo_file, canonical_root, options, stats)?
                else {
                    return Ok(std::ops::ControlFlow::Continue(()));
                };

                let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);

                // 1) File duplicates (whitespace-insensitive)
                let normalized_ws = normalize_whitespace(&bytes);
                let file = DuplicateFile {
                    repo_id: repo_file.repo_id,
                    repo_label: repo_file.repo_label.clone(),
                    path: rel_path.clone(),
                };
                file_groups.push(normalized_ws, file);

                // 2) Text-based detectors
                let text = String::from_utf8_lossy(&bytes).to_string();
                let code_norm = normalize_for_code_spans(&bytes);
                let line_norm = normalize_lines_for_dup_detection(&text);
                let tokenized = tokenize_for_dup_detection(&text);
                let blocks = parse_brace_blocks(&tokenized.tokens, &tokenized.token_lines);

                files.push(ScannedTextFile {
                    repo_id: repo_file.repo_id,
                    repo_label: repo_file.repo_label,
                    path: rel_path,
                    text,
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

    let mut file_duplicates = file_groups.into_groups(options.cross_repo_only);

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

fn normalize_lines_for_dup_detection(text: &str) -> LineNormalizedText {
    let mut line: u32 = 1;
    let mut current: Vec<u32> = Vec::new();

    let mut line_tokens = Vec::new();
    let mut line_lines = Vec::new();
    let mut line_lens = Vec::new();

    for ch in text.chars() {
        if ch == '\n' {
            if !current.is_empty() {
                line_lens.push(current.len());
                line_tokens.push(fold_u64_to_u32(fnv1a64_u32(&current)));
                line_lines.push(line);
            }
            current.clear();
            line = line.saturating_add(1);
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch as u32);
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
