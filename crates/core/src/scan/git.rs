use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::ops::ControlFlow;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use crate::types::{ScanOptions, ScanStats};

use super::{Repo, RepoFile, ignore_dirs_contains, should_stop_due_to_max_files};

#[cfg(not(test))]
const ENV_GIT_BIN: &str = "DUP_CODE_CHECK_GIT_BIN";
#[cfg(not(test))]
const ENV_ALLOW_CUSTOM_GIT: &str = "DUP_CODE_CHECK_ALLOW_CUSTOM_GIT";

pub(super) fn allow_custom_git_override(raw: Option<&OsStr>) -> bool {
    raw == Some(OsStr::new("1"))
}

pub(super) fn git_bin_override_from_env(
    allow_custom_git: bool,
    raw_git_bin: Option<OsString>,
) -> Option<OsString> {
    if !allow_custom_git {
        return None;
    }
    raw_git_bin.and_then(validate_git_bin_override)
}

fn git_exe() -> OsString {
    #[cfg(test)]
    if let Some(exe) = TEST_GIT_EXE_OVERRIDE.with(|exe| exe.borrow().clone()) {
        return exe;
    }

    // `DUP_CODE_CHECK_GIT_BIN` is a sharp edge.
    // Ignore it unless `DUP_CODE_CHECK_ALLOW_CUSTOM_GIT=1` is set, and validate strictly.
    // Keep tests hermetic via `TEST_GIT_EXE_OVERRIDE`.
    #[cfg(not(test))]
    {
        let allow_custom_git =
            allow_custom_git_override(std::env::var_os(ENV_ALLOW_CUSTOM_GIT).as_deref());
        let raw_git_bin = allow_custom_git
            .then(|| std::env::var_os(ENV_GIT_BIN))
            .flatten();
        if let Some(exe) = git_bin_override_from_env(allow_custom_git, raw_git_bin) {
            return exe;
        }
    }

    OsString::from("git")
}

pub(super) fn validate_git_bin_override(raw: OsString) -> Option<OsString> {
    if raw.to_string_lossy().is_empty() {
        return None;
    }

    let path = Path::new(&raw);
    if !path.is_absolute() {
        return None;
    }

    let meta = fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() {
        return None;
    }
    if !meta.is_file() {
        return None;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = meta.permissions().mode();
        if mode & 0o111 == 0 {
            return None;
        }
        // Never allow a group/world-writable executable as an override.
        if mode & 0o022 != 0 {
            return None;
        }
    }

    Some(raw)
}

#[cfg(test)]
thread_local! {
    static TEST_GIT_EXE_OVERRIDE: std::cell::RefCell<Option<OsString>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn with_test_git_exe<R>(exe: &Path, f: impl FnOnce() -> R) -> R {
    TEST_GIT_EXE_OVERRIDE.with(|slot| {
        let prev = slot.replace(Some(exe.as_os_str().to_os_string()));
        let out = f();
        slot.replace(prev);
        out
    })
}

pub(super) fn try_visit_repo_files_via_git<F>(
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

    // Stream `git ls-files` in small batches to avoid collecting the full file list in memory.
    let out = visit_repo_files_via_git_streaming(repo, options, stats, on_file)?;
    if out.is_none() {
        stats.git_fast_path_fallbacks = stats.git_fast_path_fallbacks.saturating_add(1);
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

    // Read paths in small batches to avoid collecting a full file list in memory, and stop early
    // when scan budgets are hit (e.g. `maxFiles`).
    const BATCH_SIZE: usize = 256;
    let mut batch: Vec<PathBuf> = Vec::with_capacity(BATCH_SIZE);
    let mut reader = BufReader::new(stdout);
    let mut bytes: Vec<u8> = Vec::new();

    let mut started = false;

    loop {
        bytes.clear();
        let n = match reader.read_until(0, &mut bytes) {
            Ok(n) => n,
            Err(_) => {
                // Fail closed: fall back to the walker so scans keep working under transient
                // process/stdout errors.
                //
                // If we already started scanning, record a walk error so strict mode can report
                // the scan as incomplete.
                if started {
                    stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                }
                let _ = child.kill();
                let _ = child.wait();
                return Ok(None);
            }
        };
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
            Ok(s) => PathBuf::from(s),
            Err(_) => {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStringExt;

                    PathBuf::from(OsString::from_vec(bytes.clone()))
                }

                #[cfg(not(unix))]
                {
                    if !started {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Ok(None);
                    }
                    stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(None);
                }
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

    let status = match child.wait() {
        Ok(status) => status,
        Err(_) => {
            if started {
                stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
    };
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
    rel_paths: &[PathBuf],
    started: &mut bool,
) -> io::Result<ControlFlow<()>>
where
    F: FnMut(&mut ScanStats, RepoFile) -> io::Result<ControlFlow<()>>,
{
    if rel_paths.is_empty() {
        return Ok(ControlFlow::Continue(()));
    }

    for rel in rel_paths {
        if !super::is_safe_relative_path_buf(rel) {
            stats.skipped_outside_root = stats.skipped_outside_root.saturating_add(1);
            continue;
        }

        let mut ignored = false;
        if let Some(parent) = rel.parent() {
            for component in parent.components() {
                let Component::Normal(name) = component else {
                    continue;
                };
                let Some(name) = name.to_str() else {
                    continue;
                };
                if ignore_dirs_contains(&options.ignore_dirs, name) {
                    ignored = true;
                    break;
                }
            }
        }
        if ignored {
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
            Err(_) => {
                stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                continue;
            }
        };

        if meta.file_type().is_symlink() && !options.follow_symlinks {
            continue;
        }
        if !meta.is_file() {
            continue;
        }

        *started = true;
        stats.candidate_files = stats.candidate_files.saturating_add(1);
        let file = RepoFile { abs_path };

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
