use std::collections::HashSet;
use std::io;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ignore::WalkBuilder;

use crate::types::{ScanOptions, ScanStats};

use super::{Repo, RepoFile, ignore_dirs_contains, should_stop_due_to_max_files};

pub(crate) fn visit_repo_files<F>(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
    mut on_file_cb: F,
) -> io::Result<ControlFlow<()>>
where
    F: FnMut(&mut ScanStats, RepoFile) -> io::Result<ControlFlow<()>>,
{
    if options.max_files == Some(0) {
        stats.skipped_budget_max_files = stats.skipped_budget_max_files.saturating_add(1);
        return Ok(ControlFlow::Break(()));
    }

    let mut visited: Vec<PathBuf> = Vec::new();

    if options.respect_gitignore
        && !options.follow_symlinks
        && let Some(flow) = {
            let mut on_git_file = |stats: &mut ScanStats, file: RepoFile| {
                visited.push(file.abs_path.clone());
                on_file_cb(stats, file)
            };
            super::git::try_visit_repo_files_via_git(repo, options, stats, &mut on_git_file)?
        }
    {
        return Ok(flow);
    }

    let visited: Option<HashSet<PathBuf>> = if visited.is_empty() {
        None
    } else {
        Some(visited.into_iter().collect())
    };

    let ignore_dirs = options.ignore_dirs.clone();
    let follow_symlinks = options.follow_symlinks;
    let respect_gitignore = options.respect_gitignore;
    let is_git_repo = repo.root.join(".git").exists();

    let canonical_root = if follow_symlinks {
        Some(repo.root.canonicalize()?)
    } else {
        None
    };
    let skipped_outside_root = Arc::new(AtomicU64::new(0));
    let skipped_not_found = Arc::new(AtomicU64::new(0));
    let skipped_permission_denied = Arc::new(AtomicU64::new(0));
    let skipped_walk_errors = Arc::new(AtomicU64::new(0));
    let skipped_outside_root_cloned = Arc::clone(&skipped_outside_root);
    let skipped_not_found_cloned = Arc::clone(&skipped_not_found);
    let skipped_permission_denied_cloned = Arc::clone(&skipped_permission_denied);
    let skipped_walk_errors_cloned = Arc::clone(&skipped_walk_errors);

    let mut builder = WalkBuilder::new(&repo.root);
    builder
        .hidden(false)
        .follow_links(follow_symlinks)
        .ignore(false)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore && is_git_repo)
        .git_exclude(respect_gitignore && is_git_repo)
        .parents(false)
        .require_git(false);

    let walker = builder
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            if !follow_symlinks && entry.path_is_symlink() {
                return false;
            }

            let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
            if !is_dir {
                return true;
            }

            if let Some(name) = entry.file_name().to_str()
                && ignore_dirs_contains(&ignore_dirs, name)
            {
                return false;
            }

            if follow_symlinks && entry.path_is_symlink() {
                let Some(canonical_root) = canonical_root.as_ref() else {
                    return false;
                };
                match entry.path().canonicalize() {
                    Ok(resolved) => {
                        if !resolved.starts_with(canonical_root) {
                            skipped_outside_root_cloned.fetch_add(1, Ordering::Relaxed);
                            return false;
                        }
                    }
                    Err(err) => {
                        match err.kind() {
                            io::ErrorKind::NotFound => {
                                skipped_not_found_cloned.fetch_add(1, Ordering::Relaxed);
                            }
                            io::ErrorKind::PermissionDenied => {
                                skipped_permission_denied_cloned.fetch_add(1, Ordering::Relaxed);
                            }
                            _ => {
                                skipped_walk_errors_cloned.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        return false;
                    }
                }
            }

            true
        })
        .build();

    let flush_filter_skips = |stats: &mut ScanStats| {
        stats.skipped_outside_root = stats
            .skipped_outside_root
            .saturating_add(skipped_outside_root.load(Ordering::Relaxed));
        stats.skipped_not_found = stats
            .skipped_not_found
            .saturating_add(skipped_not_found.load(Ordering::Relaxed));
        stats.skipped_permission_denied = stats
            .skipped_permission_denied
            .saturating_add(skipped_permission_denied.load(Ordering::Relaxed));
        stats.skipped_walk_errors = stats
            .skipped_walk_errors
            .saturating_add(skipped_walk_errors.load(Ordering::Relaxed));
    };

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(err) => {
                if let Some(io_err) = err.io_error() {
                    match io_err.kind() {
                        io::ErrorKind::NotFound => {
                            stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                            continue;
                        }
                        io::ErrorKind::PermissionDenied => {
                            stats.skipped_permission_denied =
                                stats.skipped_permission_denied.saturating_add(1);
                            continue;
                        }
                        _ => {}
                    }
                }
                stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                continue;
            }
        };

        if entry.depth() == 0 {
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }

        let abs_path = entry.into_path();
        if let Some(set) = visited.as_ref()
            && set.contains(&abs_path)
        {
            continue;
        }

        stats.candidate_files = stats.candidate_files.saturating_add(1);
        let file = RepoFile { abs_path };

        match on_file_cb(stats, file)? {
            ControlFlow::Continue(()) => {}
            ControlFlow::Break(()) => {
                flush_filter_skips(stats);
                return Ok(ControlFlow::Break(()));
            }
        }

        if should_stop_due_to_max_files(options, stats) {
            flush_filter_skips(stats);
            return Ok(ControlFlow::Break(()));
        }
    }

    flush_filter_skips(stats);

    Ok(ControlFlow::Continue(()))
}
