use std::collections::HashSet;
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ignore::WalkBuilder;

use crate::types::{ScanOptions, ScanStats};
use crate::util::fnv1a64;

pub(crate) fn validate_roots(roots: &[PathBuf]) -> io::Result<()> {
    for root in roots {
        let meta = fs::metadata(root)
            .map_err(|err| io::Error::new(err.kind(), format!("root {}: {err}", root.display())))?;
        if !meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("root {} is not a directory", root.display()),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct Repo {
    pub(crate) id: usize,
    pub(crate) root: PathBuf,
    pub(crate) label: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RepoFile {
    pub(crate) repo_id: usize,
    pub(crate) repo_label: String,
    pub(crate) root: PathBuf,
    pub(crate) abs_path: PathBuf,
}

pub(crate) fn repo_label(root: &Path, id: usize) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("repo{id}"))
}

pub(crate) fn collect_repo_files(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<Vec<RepoFile>> {
    if options.respect_gitignore
        && !options.follow_symlinks
        && let Some(files) = try_collect_repo_files_via_git(repo, options, stats)?
    {
        return Ok(files);
    }

    collect_repo_files_via_walk(repo, options, stats)
}

fn collect_repo_files_via_walk(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<Vec<RepoFile>> {
    let ignore_dirs = options.ignore_dirs.clone();
    let follow_symlinks = options.follow_symlinks;
    let respect_gitignore = options.respect_gitignore;
    let is_git_repo = repo.root.join(".git").exists();

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

            match entry.file_name().to_str() {
                Some(name) => !ignore_dirs.contains(name),
                None => true,
            }
        })
        .build();

    let mut out = Vec::new();
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

        out.push(RepoFile {
            repo_id: repo.id,
            repo_label: repo.label.clone(),
            root: repo.root.clone(),
            abs_path: entry.into_path(),
        });
    }

    Ok(out)
}

fn try_collect_repo_files_via_git(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<Option<Vec<RepoFile>>> {
    if !repo.root.join(".git").exists() {
        return Ok(None);
    }

    let output = match Command::new("git")
        .arg("-C")
        .arg(&repo.root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
    {
        Ok(out) => out,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };

    if !output.status.success() {
        return Ok(None);
    }

    let mut rel_paths = Vec::new();
    for part in output.stdout.split(|b| *b == 0) {
        if part.is_empty() {
            continue;
        }
        rel_paths.push(String::from_utf8_lossy(part).to_string());
    }

    if rel_paths.is_empty() {
        return Ok(Some(Vec::new()));
    }

    let ignored = match git_check_ignore(&repo.root, &rel_paths) {
        Ok(set) => set,
        Err(_) => return Ok(None),
    };

    let mut out = Vec::new();
    for rel in rel_paths {
        if ignored.contains(&rel) {
            continue;
        }

        let rel = rel.replace('\\', "/");
        let mut segs = rel.split('/');
        segs.next_back();
        if segs.any(|seg| options.ignore_dirs.contains(seg)) {
            continue;
        }

        let abs_path = repo.root.join(&rel);
        let meta = match fs::symlink_metadata(&abs_path) {
            Ok(m) => m,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
                continue;
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
                continue;
            }
            Err(err) => return Err(err),
        };

        if meta.file_type().is_symlink() && !options.follow_symlinks {
            continue;
        }
        if !meta.is_file() {
            continue;
        }

        out.push(RepoFile {
            repo_id: repo.id,
            repo_label: repo.label.clone(),
            root: repo.root.clone(),
            abs_path,
        });
    }

    Ok(Some(out))
}

fn git_check_ignore(root: &Path, rel_paths: &[String]) -> io::Result<HashSet<String>> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    {
        let Some(mut stdin) = child.stdin.take() else {
            return Err(io::Error::other("git check-ignore stdin not available"));
        };
        for rel in rel_paths {
            stdin.write_all(rel.as_bytes())?;
            stdin.write_all(&[0])?;
        }
    }

    let output = child.wait_with_output()?;
    if output.status.code() == Some(1) {
        return Ok(HashSet::new());
    }
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git check-ignore failed (status={:?})",
            output.status.code()
        )));
    }

    let mut out = HashSet::new();
    for part in output.stdout.split(|b| *b == 0) {
        if part.is_empty() {
            continue;
        }
        out.insert(String::from_utf8_lossy(part).to_string());
    }
    Ok(out)
}

pub(crate) fn make_rel_path(root: &Path, abs_path: &Path) -> String {
    match abs_path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => {
            let name = abs_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>");
            let hash = fnv1a64(abs_path.to_string_lossy().as_bytes());
            format!("<external:{hash:016x}>/{name}")
        }
    }
}

pub(crate) fn resolve_read_path(
    repo_file: &RepoFile,
    canonical_root: Option<&Path>,
    follow_symlinks: bool,
    stats: &mut ScanStats,
) -> io::Result<Option<PathBuf>> {
    if !follow_symlinks {
        return Ok(Some(repo_file.abs_path.clone()));
    }

    let Some(canonical_root) = canonical_root else {
        return Err(io::Error::other(
            "resolve_read_path requires canonical_root when follow_symlinks=true",
        ));
    };

    let resolved = match repo_file.abs_path.canonicalize() {
        Ok(p) => p,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
            return Ok(None);
        }
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
            return Ok(None);
        }
        Err(_) => {
            stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
            return Ok(None);
        }
    };

    if !resolved.starts_with(canonical_root) {
        stats.skipped_outside_root = stats.skipped_outside_root.saturating_add(1);
        return Ok(None);
    }

    Ok(Some(resolved))
}
