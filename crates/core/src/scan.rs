use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ignore::WalkBuilder;

use crate::types::{ScanOptions, ScanStats};
use crate::util::fnv1a64;

fn git_exe() -> OsString {
    std::env::var_os("DUP_CODE_CHECK_GIT_BIN").unwrap_or_else(|| OsString::from("git"))
}

fn should_stop_due_to_max_files(options: &ScanOptions, stats: &mut ScanStats) -> bool {
    let Some(max_files) = options.max_files else {
        return false;
    };
    if stats.scanned_files < max_files as u64 {
        return false;
    }
    stats.skipped_budget_max_files = stats.skipped_budget_max_files.saturating_add(1);
    true
}

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

pub(crate) fn visit_repo_files<F>(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
    mut on_file: F,
) -> io::Result<ControlFlow<()>>
where
    F: FnMut(&mut ScanStats, RepoFile) -> io::Result<ControlFlow<()>>,
{
    if options.max_files == Some(0) {
        stats.skipped_budget_max_files = stats.skipped_budget_max_files.saturating_add(1);
        return Ok(ControlFlow::Break(()));
    }

    if options.respect_gitignore
        && !options.follow_symlinks
        && let Some(flow) = try_visit_repo_files_via_git(repo, options, stats, &mut on_file)?
    {
        return Ok(flow);
    }

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
                && ignore_dirs.contains(name)
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

        stats.candidate_files = stats.candidate_files.saturating_add(1);
        let file = RepoFile {
            repo_id: repo.id,
            repo_label: repo.label.clone(),
            root: repo.root.clone(),
            abs_path: entry.into_path(),
        };

        match on_file(stats, file)? {
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

fn try_visit_repo_files_via_git<F>(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
    on_file: &mut F,
) -> io::Result<Option<ControlFlow<()>>>
where
    F: FnMut(&mut ScanStats, RepoFile) -> io::Result<ControlFlow<()>>,
{
    if !repo.root.join(".git").exists() {
        return Ok(None);
    }

    if options.max_files.is_some() {
        return visit_repo_files_via_git_streaming(repo, options, stats, on_file);
    }

    let output = match Command::new(git_exe())
        .arg("-C")
        .arg(&repo.root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .stderr(Stdio::null())
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
        let Ok(path) = std::str::from_utf8(part) else {
            // Fall back to the walker on repositories containing non-UTF-8 paths.
            return Ok(None);
        };
        rel_paths.push(path.to_string());
    }

    if rel_paths.is_empty() {
        return Ok(Some(ControlFlow::Continue(())));
    }

    let ignored = match git_check_ignore(&repo.root, &rel_paths) {
        Ok(set) => set,
        Err(_) => return Ok(None),
    };

    for rel in rel_paths {
        if ignored.contains(&rel) {
            continue;
        }
        if !is_safe_relative_path(&rel) {
            stats.skipped_outside_root = stats.skipped_outside_root.saturating_add(1);
            continue;
        }

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

        stats.candidate_files = stats.candidate_files.saturating_add(1);
        let file = RepoFile {
            repo_id: repo.id,
            repo_label: repo.label.clone(),
            root: repo.root.clone(),
            abs_path,
        };

        match on_file(stats, file)? {
            ControlFlow::Continue(()) => {}
            ControlFlow::Break(()) => return Ok(Some(ControlFlow::Break(()))),
        }

        if should_stop_due_to_max_files(options, stats) {
            return Ok(Some(ControlFlow::Break(())));
        }
    }

    Ok(Some(ControlFlow::Continue(())))
}

fn is_safe_relative_path(raw: &str) -> bool {
    if raw.is_empty() {
        return false;
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => return false,
        }
    }
    true
}

fn git_check_ignore(root: &Path, rel_paths: &[String]) -> io::Result<HashSet<String>> {
    let mut child = Command::new(git_exe())
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

fn visit_repo_files_via_git_streaming<F>(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
    on_file: &mut F,
) -> io::Result<Option<ControlFlow<()>>>
where
    F: FnMut(&mut ScanStats, RepoFile) -> io::Result<ControlFlow<()>>,
{
    let mut child = match Command::new(git_exe())
        .arg("-C")
        .arg(&repo.root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };

    let Some(stdout) = child.stdout.take() else {
        return Ok(None);
    };

    // With `maxFiles`, avoid collecting the entire file list. We read paths in small batches,
    // run a batched `git check-ignore`, and stop as soon as the scan budget is hit.
    const BATCH_SIZE: usize = 256;
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);
    let mut reader = BufReader::new(stdout);

    let mut started = false;

    loop {
        let mut bytes: Vec<u8> = Vec::new();
        let n = reader.read_until(0, &mut bytes)?;
        if n == 0 {
            break;
        }
        if bytes.last() == Some(&0) {
            bytes.pop();
        }
        if bytes.is_empty() {
            continue;
        }

        let rel = match std::str::from_utf8(&bytes) {
            Ok(s) => s.to_string(),
            Err(_) => {
                stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                continue;
            }
        };

        batch.push(rel);
        if batch.len() < BATCH_SIZE {
            continue;
        }

        let flow = match visit_repo_files_via_git_batch(
            repo,
            options,
            stats,
            on_file,
            &batch,
            &mut started,
        ) {
            Ok(flow) => flow,
            Err(err) => {
                if !started {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(None);
                }
                return Err(err);
            }
        };
        batch.clear();
        match flow {
            ControlFlow::Continue(()) => {}
            ControlFlow::Break(()) => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(Some(ControlFlow::Break(())));
            }
        }
    }

    if !batch.is_empty() {
        let flow = match visit_repo_files_via_git_batch(
            repo,
            options,
            stats,
            on_file,
            &batch,
            &mut started,
        ) {
            Ok(flow) => flow,
            Err(err) => {
                if !started {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(None);
                }
                return Err(err);
            }
        };
        match flow {
            ControlFlow::Continue(()) => {}
            ControlFlow::Break(()) => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(Some(ControlFlow::Break(())));
            }
        }
    }

    let status = child.wait()?;
    if !status.success() {
        return Ok(None);
    }

    Ok(Some(ControlFlow::Continue(())))
}

fn visit_repo_files_via_git_batch<F>(
    repo: &Repo,
    options: &ScanOptions,
    stats: &mut ScanStats,
    on_file: &mut F,
    rel_paths: &[String],
    started: &mut bool,
) -> io::Result<ControlFlow<()>>
where
    F: FnMut(&mut ScanStats, RepoFile) -> io::Result<ControlFlow<()>>,
{
    if rel_paths.is_empty() {
        return Ok(ControlFlow::Continue(()));
    }

    let ignored = git_check_ignore(&repo.root, rel_paths)?;

    for rel in rel_paths {
        if ignored.contains(rel) {
            continue;
        }
        if !is_safe_relative_path(rel) {
            stats.skipped_outside_root = stats.skipped_outside_root.saturating_add(1);
            continue;
        }

        let mut segs = rel.split('/');
        segs.next_back();
        if segs.any(|seg| options.ignore_dirs.contains(seg)) {
            continue;
        }

        let abs_path = repo.root.join(rel);
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

        *started = true;
        stats.candidate_files = stats.candidate_files.saturating_add(1);
        let file = RepoFile {
            repo_id: repo.id,
            repo_label: repo.label.clone(),
            root: repo.root.clone(),
            abs_path,
        };

        match on_file(stats, file)? {
            ControlFlow::Continue(()) => {}
            ControlFlow::Break(()) => return Ok(ControlFlow::Break(())),
        }

        if should_stop_due_to_max_files(options, stats) {
            return Ok(ControlFlow::Break(()));
        }
    }

    Ok(ControlFlow::Continue(()))
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

pub(crate) fn read_repo_file_bytes(
    repo_file: &RepoFile,
    canonical_root: Option<&Path>,
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<Option<Vec<u8>>> {
    if let Some(max_files) = options.max_files
        && stats.scanned_files >= max_files as u64
    {
        return Ok(None);
    }

    let Some(read_path) =
        resolve_read_path(repo_file, canonical_root, options.follow_symlinks, stats)?
    else {
        return Ok(None);
    };

    let metadata = match fs::metadata(&read_path) {
        Ok(m) => m,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
            return Ok(None);
        }
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
            return Ok(None);
        }
        Err(err) => return Err(err),
    };

    if let Some(max_file_size) = options.max_file_size
        && metadata.len() > max_file_size
    {
        stats.skipped_too_large = stats.skipped_too_large.saturating_add(1);
        return Ok(None);
    }

    if let Some(max_total_bytes) = options.max_total_bytes
        && stats.scanned_bytes.saturating_add(metadata.len()) > max_total_bytes
    {
        stats.skipped_budget_max_total_bytes =
            stats.skipped_budget_max_total_bytes.saturating_add(1);
        return Ok(None);
    }

    let bytes = match fs::read(&read_path) {
        Ok(b) => b,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            stats.skipped_not_found = stats.skipped_not_found.saturating_add(1);
            return Ok(None);
        }
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            stats.skipped_permission_denied = stats.skipped_permission_denied.saturating_add(1);
            return Ok(None);
        }
        Err(err) => return Err(err),
    };

    if bytes.contains(&0) {
        stats.skipped_binary = stats.skipped_binary.saturating_add(1);
        return Ok(None);
    }

    stats.scanned_files = stats.scanned_files.saturating_add(1);
    stats.scanned_bytes = stats.scanned_bytes.saturating_add(bytes.len() as u64);

    Ok(Some(bytes))
}
