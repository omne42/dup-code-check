use std::io;
use std::path::PathBuf;

use crate::dedupe::{FileDuplicateGrouper, detect_duplicate_code_spans_winnowing};
use crate::scan::{
    Repo, make_rel_path, read_repo_file_bytes, repo_label, validate_roots, visit_repo_files,
};
use crate::types::{
    DuplicateFile, DuplicateGroup, DuplicateSpanGroup, ScanOptions, ScanOutcome, ScanStats,
};
use crate::util::{NormalizedFile, NormalizedFileView, normalize_for_code_spans};

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

                let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);
                let file = DuplicateFile {
                    repo_id: repo_file.repo_id,
                    repo_label: repo_file.repo_label.clone(),
                    path: rel_path,
                };

                groups.push_bytes(&bytes, file);

                Ok(std::ops::ControlFlow::Continue(()))
            })?
        {
            break;
        }
    }

    let mut out = groups.into_groups(options.cross_repo_only);

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

    let min_match_len = options.min_match_len.max(1);

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

    let mut stats = ScanStats::default();
    let mut files = Vec::new();

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

                let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);
                files.push(NormalizedFile {
                    repo_id: repo_file.repo_id,
                    repo_label: repo_file.repo_label,
                    rel_path,
                    normalized: normalized.chars,
                    line_map: normalized.line_map,
                });

                Ok(std::ops::ControlFlow::Continue(()))
            })?
        {
            break;
        }
    }

    let views: Vec<NormalizedFileView<'_>> = files
        .iter()
        .map(|file| NormalizedFileView {
            repo_id: file.repo_id,
            repo_label: &file.repo_label,
            rel_path: &file.rel_path,
            normalized: &file.normalized,
            line_map: &file.line_map,
        })
        .collect();

    let out = detect_duplicate_code_spans_winnowing(&views, options, &mut stats);
    Ok(ScanOutcome { result: out, stats })
}
