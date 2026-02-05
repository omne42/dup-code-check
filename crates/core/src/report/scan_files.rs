use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crate::dedupe::FileDuplicateGrouper;
use crate::scan::{
    Repo, read_repo_file_bytes_for_verification, read_repo_file_bytes_with_path, repo_label,
    visit_repo_files,
};
use crate::tokenize::{parse_brace_blocks, tokenize_for_dup_detection};
use crate::types::{DuplicateGroup, ScanOptions, ScanStats};
use crate::util::{fnv1a64_u32, fold_u64_to_u32, normalize_for_code_spans};

use super::ScannedTextFile;
use super::util::sort_duplicate_groups_for_report;

const DEFAULT_REPORT_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_REPORT_MAX_NORMALIZED_CHARS_DIVISOR: u64 = 1;
const DEFAULT_REPORT_MAX_TOKENS_DIVISOR: u64 = 4;

type ReportScanOutput = (Vec<Arc<str>>, Vec<ScannedTextFile>, Vec<DuplicateGroup>);

pub(super) fn scan_text_files_for_report(
    roots: &[PathBuf],
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<ReportScanOutput> {
    let mut scan_options = options.clone();
    let max_total_bytes = scan_options
        .max_total_bytes
        .unwrap_or(DEFAULT_REPORT_MAX_TOTAL_BYTES);
    scan_options.max_total_bytes = Some(max_total_bytes);

    if scan_options.max_normalized_chars.is_none() {
        scan_options.max_normalized_chars = Some(
            usize::try_from(max_total_bytes / DEFAULT_REPORT_MAX_NORMALIZED_CHARS_DIVISOR)
                .unwrap_or(usize::MAX),
        );
    }
    if scan_options.max_tokens.is_none() {
        scan_options.max_tokens = Some(
            usize::try_from(max_total_bytes / DEFAULT_REPORT_MAX_TOKENS_DIVISOR)
                .unwrap_or(usize::MAX),
        );
    }

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: Arc::from(repo_label(root, id)),
        })
        .collect();
    let repo_labels: Vec<Arc<str>> = repos.iter().map(|repo| Arc::clone(&repo.label)).collect();

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
    let mut total_normalized_chars: usize = 0;
    let mut total_tokens: usize = 0;
    let max_normalized_chars = scan_options.max_normalized_chars;
    let max_tokens = scan_options.max_tokens;

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

                let rel_path_for_verification = match repo_file.abs_path.strip_prefix(&repo.root) {
                    Ok(rel) => rel.to_path_buf(),
                    Err(_) => {
                        stats.skipped_relativize_failed =
                            stats.skipped_relativize_failed.saturating_add(1);
                        return Ok(std::ops::ControlFlow::Continue(()));
                    }
                };

                let rel_path = Arc::<str>::from(
                    rel_path_for_verification
                        .to_string_lossy()
                        .replace('\\', "/"),
                );

                // Text-based detectors
                let text = String::from_utf8_lossy(&bytes);
                let code_norm = normalize_for_code_spans(&bytes);
                let line_norm = normalize_lines_for_dup_detection(&bytes);
                let tokenized = tokenize_for_dup_detection(&text);
                let blocks = parse_brace_blocks(&tokenized.tokens, &tokenized.token_lines);

                if let Some(max_normalized_chars) = max_normalized_chars {
                    let next_total = total_normalized_chars.saturating_add(code_norm.chars.len());
                    if next_total > max_normalized_chars {
                        stats.skipped_budget_max_normalized_chars =
                            stats.skipped_budget_max_normalized_chars.saturating_add(1);
                        return Ok(std::ops::ControlFlow::Break(()));
                    }
                    total_normalized_chars = next_total;
                }
                if let Some(max_tokens) = max_tokens {
                    let next_total = total_tokens.saturating_add(tokenized.tokens.len());
                    if next_total > max_tokens {
                        stats.skipped_budget_max_tokens =
                            stats.skipped_budget_max_tokens.saturating_add(1);
                        return Ok(std::ops::ControlFlow::Break(()));
                    }
                    total_tokens = next_total;
                }

                // File duplicates (whitespace-insensitive)
                file_groups.push_bytes(
                    &bytes,
                    repo.id,
                    rel_path_for_verification,
                    Arc::clone(&rel_path),
                );

                files.push(ScannedTextFile {
                    repo_id: repo.id,
                    path: rel_path,
                    abs_path: read_path,
                    code_chars: code_norm.chars,
                    code_line_starts: code_norm.line_starts,
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
    let mut file_duplicates = file_groups.into_groups_verified(
        options.cross_repo_only,
        |repo_id, path| {
            let repo = &repos[repo_id];
            let canonical_root = canonical_roots.map(|roots| roots[repo_id].as_path());

            read_repo_file_bytes_for_verification(
                &repo.root,
                path.as_path(),
                canonical_root,
                follow_symlinks,
                max_file_size,
            )
        },
        |repo_id| Arc::clone(&repos[repo_id].label),
    )?;

    sort_duplicate_groups_for_report(&mut file_duplicates);
    file_duplicates.truncate(options.max_report_items);

    Ok((repo_labels, files, file_duplicates))
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
