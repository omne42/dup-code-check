use std::path::PathBuf;

use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi(object)]
pub struct ScanOptions {
    pub ignore_dirs: Option<Vec<String>>,
    pub max_file_size: Option<u32>,
    pub cross_repo_only: Option<bool>,
    pub follow_symlinks: Option<bool>,
}

#[napi(object)]
pub struct DuplicateFile {
    pub repo_id: u32,
    pub repo_label: String,
    pub path: String,
}

#[napi(object)]
pub struct DuplicateGroup {
    pub hash: String,
    pub normalized_len: u32,
    pub files: Vec<DuplicateFile>,
}

#[napi(js_name = "findDuplicateFiles")]
pub fn find_duplicate_files(
    roots: Vec<String>,
    options: Option<ScanOptions>,
) -> Result<Vec<DuplicateGroup>> {
    if roots.is_empty() {
        return Err(Error::from_reason("roots must not be empty"));
    }

    let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
    let options = to_core_options(options);

    let groups = code_checker_core::find_duplicate_files(&roots, &options)
        .map_err(|e| Error::from_reason(format!("scan failed: {e}")))?;

    Ok(groups
        .into_iter()
        .map(|g| DuplicateGroup {
            hash: format!("{:016x}", g.content_hash),
            normalized_len: g.normalized_len as u32,
            files: g
                .files
                .into_iter()
                .map(|f| DuplicateFile {
                    repo_id: f.repo_id as u32,
                    repo_label: f.repo_label,
                    path: f.path,
                })
                .collect(),
        })
        .collect())
}

fn to_core_options(options: Option<ScanOptions>) -> code_checker_core::ScanOptions {
    let mut out = code_checker_core::ScanOptions::default();

    if let Some(options) = options {
        if let Some(ignore_dirs) = options.ignore_dirs {
            out.ignore_dirs.extend(ignore_dirs);
        }
        out.max_file_size = options.max_file_size.map(u64::from);
        out.cross_repo_only = options.cross_repo_only.unwrap_or(out.cross_repo_only);
        out.follow_symlinks = options.follow_symlinks.unwrap_or(out.follow_symlinks);
    }

    out
}
