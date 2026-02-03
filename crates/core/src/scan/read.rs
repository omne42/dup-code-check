use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::types::{ScanOptions, ScanStats};
use crate::util::fnv1a64;

use super::RepoFile;

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
        Err(err) => return Err(err),
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
            Err(err) => return Err(err),
        };
        if (metadata.dev(), metadata.ino()) != (opened.dev(), opened.ino()) {
            stats.skipped_walk_errors = stats.skipped_walk_errors.saturating_add(1);
            return Ok(None);
        }
    }

    use std::io::Read;

    let mut bytes: Vec<u8> = Vec::with_capacity(metadata.len().min(1024 * 1024) as usize);
    let mut total_read = 0usize;
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total_read = total_read.saturating_add(n);
        if buf[..n].contains(&0) {
            stats.scanned_files = stats.scanned_files.saturating_add(1);
            stats.scanned_bytes = stats.scanned_bytes.saturating_add(total_read as u64);
            stats.skipped_binary = stats.skipped_binary.saturating_add(1);
            return Ok(None);
        }
        bytes.extend_from_slice(&buf[..n]);
    }

    stats.scanned_files = stats.scanned_files.saturating_add(1);
    stats.scanned_bytes = stats.scanned_bytes.saturating_add(total_read as u64);

    Ok(Some((bytes, read_path)))
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

pub(crate) fn read_repo_file_bytes_for_verification(
    repo_root: &Path,
    rel_path: &str,
    canonical_root: Option<&Path>,
    follow_symlinks: bool,
    max_file_size: Option<u64>,
) -> io::Result<Option<Vec<u8>>> {
    if !is_safe_relative_path(rel_path) {
        return Ok(None);
    }

    let candidate = repo_root.join(rel_path);
    let read_path = if follow_symlinks {
        let Some(canonical_root) = canonical_root else {
            return Err(io::Error::other(
                "read_repo_file_bytes_for_verification requires canonical_root when follow_symlinks=true",
            ));
        };

        let resolved = match candidate.canonicalize() {
            Ok(p) => p,
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) =>
            {
                return Ok(None);
            }
            Err(_) => return Ok(None),
        };

        if !resolved.starts_with(canonical_root) {
            return Ok(None);
        }

        resolved
    } else {
        candidate
    };

    let metadata = match fs::symlink_metadata(&read_path) {
        Ok(m) => {
            if m.file_type().is_symlink() {
                return Ok(None);
            }
            m
        }
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(None);
        }
        Err(_) => return Ok(None),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    if let Some(max_file_size) = max_file_size
        && metadata.len() > max_file_size
    {
        return Ok(None);
    }

    let mut file = match fs::File::open(&read_path) {
        Ok(f) => f,
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(None);
        }
        Err(err) => return Err(err),
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let opened = match file.metadata() {
            Ok(m) => m,
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) =>
            {
                return Ok(None);
            }
            Err(err) => return Err(err),
        };
        if (metadata.dev(), metadata.ino()) != (opened.dev(), opened.ino()) {
            return Ok(None);
        }
    }

    use std::io::Read;

    let mut bytes: Vec<u8> = Vec::with_capacity(metadata.len().min(1024 * 1024) as usize);
    file.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}
