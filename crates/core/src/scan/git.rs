use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::Path;
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
        // Never allow a world-writable executable as an override.
        if mode & 0o002 != 0 {
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
    visit_repo_files_via_git_streaming(repo, options, stats, on_file)
}

fn git_check_ignore(root: &Path, rel_paths: &[String]) -> io::Result<HashSet<String>> {
    let mut child = Command::new(git_exe())
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
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
        let path = std::str::from_utf8(part)
            .map_err(|_| io::Error::other("git check-ignore returned a non-UTF-8 path"))?;
        out.insert(path.to_string());
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

    // Read paths in small batches, run a batched `git check-ignore`, and stop early when scan
    // budgets are hit (e.g. `maxFiles`).
    const BATCH_SIZE: usize = 256;
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);
    let mut reader = BufReader::new(stdout);
    let mut bytes: Vec<u8> = Vec::new();

    let mut started = false;
    let mut check_ignore_ok = true;

    loop {
        bytes.clear();
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
            &mut check_ignore_ok,
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
                if !check_ignore_ok {
                    return Ok(None);
                }
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
            &mut check_ignore_ok,
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
                if !check_ignore_ok {
                    return Ok(None);
                }
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
    check_ignore_ok: &mut bool,
) -> io::Result<ControlFlow<()>>
where
    F: FnMut(&mut ScanStats, RepoFile) -> io::Result<ControlFlow<()>>,
{
    if rel_paths.is_empty() {
        return Ok(ControlFlow::Continue(()));
    }

    let ignored = if *check_ignore_ok {
        match git_check_ignore(&repo.root, rel_paths) {
            Ok(ignored) => ignored,
            Err(_) => {
                if !*started {
                    return Err(io::Error::other("git check-ignore failed"));
                }
                stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                *check_ignore_ok = false;
                return Ok(ControlFlow::Break(()));
            }
        }
    } else {
        HashSet::new()
    };

    for rel in rel_paths {
        if ignored.contains(rel) {
            continue;
        }
        if !super::is_safe_relative_path(rel) {
            stats.skipped_outside_root = stats.skipped_outside_root.saturating_add(1);
            continue;
        }

        let mut segs = rel.split('/');
        segs.next_back();
        if segs.any(|seg| ignore_dirs_contains(&options.ignore_dirs, seg)) {
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
