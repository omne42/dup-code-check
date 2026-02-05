use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::types::{ScanOptions, ScanStats};
use crate::util::fnv1a64;

use super::RepoFile;

#[cfg(test)]
type BeforeOpenHook = std::cell::RefCell<Option<Box<dyn FnMut(&Path)>>>;

#[cfg(test)]
thread_local! {
    static TEST_BEFORE_OPEN_HOOK: BeforeOpenHook = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn with_test_before_open_hook<R>(
    hook: impl FnMut(&Path) + 'static,
    f: impl FnOnce() -> R,
) -> R {
    TEST_BEFORE_OPEN_HOOK.with(|slot| {
        let prev = slot.replace(Some(Box::new(hook)));
        let out = f();
        slot.replace(prev);
        out
    })
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

fn resolve_read_path(
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
    Ok(
        read_repo_file_bytes_with_path(repo_file, canonical_root, options, stats)?
            .map(|(bytes, _path)| bytes),
    )
}

pub(crate) fn read_repo_file_bytes_with_path(
    repo_file: &RepoFile,
    canonical_root: Option<&Path>,
    options: &ScanOptions,
    stats: &mut ScanStats,
) -> io::Result<Option<(Vec<u8>, PathBuf)>> {
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

    // Harden against TOCTOU when following symlinks: avoid reading from a different file than the
    // one we just resolved/validated (especially if a path is replaced with a symlink concurrently).
    let metadata = match fs::symlink_metadata(&read_path) {
        Ok(m) => {
            if m.file_type().is_symlink() {
                stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                return Ok(None);
            }
            m
        }
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

    #[cfg(test)]
    TEST_BEFORE_OPEN_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().as_mut() {
            hook(&read_path);
        }
    });

    let mut file = match fs::File::open(&read_path) {
        Ok(f) => f,
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

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let opened = match file.metadata() {
            Ok(m) => m,
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
        if (metadata.dev(), metadata.ino()) != (opened.dev(), opened.ino()) {
            stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
            return Ok(None);
        }
    }

    use std::io::Read;

    let metadata_len = metadata.len();
    let max_file_size = options.max_file_size;
    let max_total_bytes = options.max_total_bytes;

    let mut bytes: Vec<u8> = Vec::with_capacity(metadata_len.min(1024 * 1024) as usize);
    let mut total_read: u64 = 0;
    let mut buf = [0u8; 16 * 1024];
    loop {
        let mut limit = buf.len() as u64;

        if let Some(max_file_size) = max_file_size {
            let cap = max_file_size.saturating_add(1);
            let remaining = cap.saturating_sub(total_read);
            if remaining == 0 {
                stats.scanned_files = stats.scanned_files.saturating_add(1);
                stats.scanned_bytes = stats.scanned_bytes.saturating_add(total_read);
                stats.skipped_too_large = stats.skipped_too_large.saturating_add(1);
                return Ok(None);
            }
            limit = limit.min(remaining);
        }

        if let Some(max_total_bytes) = max_total_bytes {
            let remaining_budget =
                max_total_bytes.saturating_sub(stats.scanned_bytes.saturating_add(total_read));
            if remaining_budget == 0 {
                if total_read == metadata_len {
                    break;
                }
                stats.scanned_files = stats.scanned_files.saturating_add(1);
                stats.scanned_bytes = stats.scanned_bytes.saturating_add(total_read);
                stats.skipped_budget_max_total_bytes =
                    stats.skipped_budget_max_total_bytes.saturating_add(1);
                return Ok(None);
            }
            limit = limit.min(remaining_budget);
        }

        let n = match file.read(&mut buf[..limit as usize]) {
            Ok(n) => n,
            Err(_) => {
                stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
                if total_read > 0 {
                    stats.scanned_files = stats.scanned_files.saturating_add(1);
                    stats.scanned_bytes = stats.scanned_bytes.saturating_add(total_read);
                }
                return Ok(None);
            }
        };
        if n == 0 {
            break;
        }

        let new_total_read = total_read.saturating_add(n as u64);
        if buf[..n].contains(&0) {
            stats.scanned_files = stats.scanned_files.saturating_add(1);
            stats.scanned_bytes = stats.scanned_bytes.saturating_add(new_total_read);
            stats.skipped_binary = stats.skipped_binary.saturating_add(1);
            return Ok(None);
        }

        if let Some(max_file_size) = max_file_size
            && new_total_read > max_file_size
        {
            stats.scanned_files = stats.scanned_files.saturating_add(1);
            stats.scanned_bytes = stats.scanned_bytes.saturating_add(new_total_read);
            stats.skipped_too_large = stats.skipped_too_large.saturating_add(1);
            return Ok(None);
        }

        bytes.extend_from_slice(&buf[..n]);
        total_read = new_total_read;
    }

    stats.scanned_files = stats.scanned_files.saturating_add(1);
    stats.scanned_bytes = stats.scanned_bytes.saturating_add(total_read);

    Ok(Some((bytes, read_path)))
}

pub(crate) fn read_repo_file_bytes_for_verification(
    repo_root: &Path,
    rel_path: &Path,
    canonical_root: Option<&Path>,
    follow_symlinks: bool,
    max_file_size: Option<u64>,
) -> io::Result<Option<Vec<u8>>> {
    if !super::is_safe_relative_path_buf(rel_path) {
        return Ok(None);
    }

    if follow_symlinks && canonical_root.is_none() {
        return Err(io::Error::other(
            "read_repo_file_bytes_for_verification requires canonical_root when follow_symlinks=true",
        ));
    }

    let repo_file = RepoFile {
        abs_path: repo_root.join(rel_path),
    };
    let options = ScanOptions {
        follow_symlinks,
        max_file_size,
        max_files: None,
        max_total_bytes: None,
        ..ScanOptions::default()
    };
    let mut stats = ScanStats::default();
    read_repo_file_bytes(&repo_file, canonical_root, &options, &mut stats)
}
