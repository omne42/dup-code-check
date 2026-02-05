use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crate::dedupe::{FileDuplicateGrouper, detect_duplicate_code_spans_winnowing};
use crate::scan::{
    Repo, make_rel_path, read_repo_file_bytes, read_repo_file_bytes_for_verification, repo_label,
    validate_roots, visit_repo_files,
};
use crate::types::{DuplicateGroup, DuplicateSpanGroup, ScanOptions, ScanOutcome, ScanStats};
use crate::util::{NormalizedCodeFile, NormalizedCodeFileView, normalize_for_code_spans};

pub fn find_duplicate_files(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<Vec<DuplicateGroup>> {
    Ok(find_duplicate_files_with_stats(roots, options)?.result)
}

pub fn find_duplicate_files_with_stats(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<ScanOutcome<Vec<DuplicateGroup>>> {
    if roots.is_empty() {
        return Ok(ScanOutcome {
            result: Vec::new(),
            stats: ScanStats::default(),
        });
    }

    validate_roots(roots)?;
    options.validate_for_file_duplicates()?;

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: Arc::from(repo_label(root, id)),
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

    let mut stats = ScanStats::default();
    let mut groups = FileDuplicateGrouper::default();

    for repo in &repos {
        let canonical_root = canonical_roots
            .as_ref()
            .map(|roots| roots[repo.id].as_path());

        if let std::ops::ControlFlow::Break(()) =
            visit_repo_files(repo, options, &mut stats, |stats, repo_file| {
                let Some(bytes) = read_repo_file_bytes(&repo_file, canonical_root, options, stats)?
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
                groups.push_bytes(&bytes, repo.id, rel_path_for_verification, rel_path);

                Ok(std::ops::ControlFlow::Continue(()))
            })?
        {
            break;
        }
    }

    let mut out = groups.into_groups_verified(
        options.cross_repo_only,
        |repo_id, path| {
            let repo = &repos[repo_id];
            let canonical_root = canonical_roots
                .as_ref()
                .map(|roots| roots[repo_id].as_path());

            read_repo_file_bytes_for_verification(
                &repo.root,
                path.as_path(),
                canonical_root,
                options.follow_symlinks,
                options.max_file_size,
            )
        },
        |repo_id| Arc::clone(&repos[repo_id].label),
    )?;

    out.sort_by(|a, b| {
        (a.content_hash, a.normalized_len, a.files.len()).cmp(&(
            b.content_hash,
            b.normalized_len,
            b.files.len(),
        ))
    });
    Ok(ScanOutcome { result: out, stats })
}

pub fn find_duplicate_code_spans(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<Vec<DuplicateSpanGroup>> {
    Ok(find_duplicate_code_spans_with_stats(roots, options)?.result)
}

pub fn find_duplicate_code_spans_with_stats(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<ScanOutcome<Vec<DuplicateSpanGroup>>> {
    if roots.is_empty() {
        return Ok(ScanOutcome {
            result: Vec::new(),
            stats: ScanStats::default(),
        });
    }

    validate_roots(roots)?;
    options.validate_for_code_spans()?;

    let min_match_len = options.min_match_len.max(1);

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: Arc::from(repo_label(root, id)),
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

    let mut stats = ScanStats::default();
    let mut files = Vec::new();
    let mut total_normalized_chars: usize = 0;

    for repo in &repos {
        let canonical_root = canonical_roots
            .as_ref()
            .map(|roots| roots[repo.id].as_path());

        if let std::ops::ControlFlow::Break(()) =
            visit_repo_files(repo, options, &mut stats, |stats, repo_file| {
                let Some(bytes) = read_repo_file_bytes(&repo_file, canonical_root, options, stats)?
                else {
                    return Ok(std::ops::ControlFlow::Continue(()));
                };

                let normalized = normalize_for_code_spans(&bytes);
                if normalized.chars.len() < min_match_len {
                    return Ok(std::ops::ControlFlow::Continue(()));
                }
                if let Some(max_normalized_chars) = options.max_normalized_chars {
                    let next_total = total_normalized_chars.saturating_add(normalized.chars.len());
                    if next_total > max_normalized_chars {
                        stats.skipped_budget_max_normalized_chars =
                            stats.skipped_budget_max_normalized_chars.saturating_add(1);
                        return Ok(std::ops::ControlFlow::Break(()));
                    }
                    total_normalized_chars = next_total;
                }

                let rel_path = make_rel_path(&repo.root, &repo_file.abs_path);
                files.push(NormalizedCodeFile {
                    repo_id: repo.id,
                    repo_label: Arc::clone(&repo.label),
                    rel_path: Arc::from(rel_path),
                    normalized: normalized.chars,
                    line_starts: normalized.line_starts,
                });

                Ok(std::ops::ControlFlow::Continue(()))
            })?
        {
            break;
        }
    }

    let views: Vec<NormalizedCodeFileView<'_>> = files
        .iter()
        .map(|file| {
            debug_assert!(
                file.repo_id < repos.len(),
                "repo_id must be valid for all scanned files"
            );
            NormalizedCodeFileView {
                repo_id: file.repo_id,
                repo_label: Arc::clone(&file.repo_label),
                rel_path: Arc::clone(&file.rel_path),
                normalized: &file.normalized,
                line_starts: &file.line_starts,
            }
        })
        .collect();

    let out = detect_duplicate_code_spans_winnowing(&views, options, &mut stats);
    Ok(ScanOutcome { result: out, stats })
}
