use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::types::{ScanOptions, ScanStats};

mod git;
mod read;
mod walker;

#[cfg(test)]
mod tests;

pub(crate) use read::{
    make_rel_path, read_repo_file_bytes, read_repo_file_bytes_for_verification,
    read_repo_file_bytes_with_path,
};
pub(crate) use walker::visit_repo_files;

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
    pub(crate) label: Arc<str>,
}

#[derive(Debug, Clone)]
pub(crate) struct RepoFile {
    pub(crate) abs_path: PathBuf,
}

pub(crate) fn repo_label(root: &Path, id: usize) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("repo{id}"))
}

fn ignore_dirs_contains(ignore_dirs: &HashSet<String>, name: &str) -> bool {
    if ignore_dirs.contains(name) {
        return true;
    }
    #[cfg(windows)]
    {
        ignore_dirs.iter().any(|d| d.eq_ignore_ascii_case(name))
    }
    #[cfg(not(windows))]
    {
        false
    }
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
            | Component::Prefix(_) => {
                return false;
            }
        }
    }
    true
}

fn is_safe_relative_path_buf(path: &Path) -> bool {
    if path.as_os_str().is_empty() {
        return false;
    }
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return false;
            }
        }
    }
    true
}
