use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub ignore_dirs: HashSet<String>,
    pub max_file_size: Option<u64>,
    pub cross_repo_only: bool,
    pub follow_symlinks: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            ignore_dirs: default_ignore_dirs(),
            max_file_size: None,
            cross_repo_only: false,
            follow_symlinks: false,
        }
    }
}

pub fn default_ignore_dirs() -> HashSet<String> {
    [
        ".git",
        ".hg",
        ".svn",
        "node_modules",
        "target",
        "dist",
        "build",
        "out",
        ".next",
        ".turbo",
        ".cache",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFile {
    pub repo_id: usize,
    pub repo_label: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub content_hash: u64,
    pub normalized_len: usize,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone)]
struct Repo {
    id: usize,
    root: PathBuf,
    label: String,
}

#[derive(Debug, Clone)]
struct RepoFile {
    repo_id: usize,
    repo_label: String,
    root: PathBuf,
    abs_path: PathBuf,
}

pub fn find_duplicate_files(
    roots: &[PathBuf],
    options: &ScanOptions,
) -> io::Result<Vec<DuplicateGroup>> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }

    let repos: Vec<Repo> = roots
        .iter()
        .enumerate()
        .map(|(id, root)| Repo {
            id,
            root: root.clone(),
            label: repo_label(root, id),
        })
        .collect();

    let mut all_files = Vec::new();
    for repo in &repos {
        let mut visited_dirs = HashSet::new();
        walk_repo(&repo.root, repo, options, &mut all_files, &mut visited_dirs)?;
    }

    #[derive(Debug)]
    struct GroupBuilder {
        content_hash: u64,
        normalized_len: usize,
        sample: Vec<u8>,
        files: Vec<DuplicateFile>,
        repo_ids: HashSet<usize>,
    }

    let mut groups: HashMap<(u64, usize), Vec<GroupBuilder>> = HashMap::new();

    for repo_file in all_files {
        let metadata = fs::metadata(&repo_file.abs_path)?;
        if let Some(max_file_size) = options.max_file_size {
            if metadata.len() > max_file_size {
                continue;
            }
        }

        let bytes = fs::read(&repo_file.abs_path)?;
        if bytes.contains(&0) {
            continue;
        }

        let normalized = normalize_whitespace(&bytes);
        let content_hash = fnv1a64(&normalized);

        let key = (content_hash, normalized.len());
        let bucket = groups.entry(key).or_default();

        let rel_path = make_rel_path(&repo_file.root, &repo_file.abs_path);
        let file = DuplicateFile {
            repo_id: repo_file.repo_id,
            repo_label: repo_file.repo_label.clone(),
            path: rel_path,
        };

        if let Some(existing) = bucket.iter_mut().find(|g| g.sample == normalized) {
            existing.repo_ids.insert(file.repo_id);
            existing.files.push(file);
            continue;
        }

        let mut repo_ids = HashSet::new();
        repo_ids.insert(file.repo_id);
        bucket.push(GroupBuilder {
            content_hash,
            normalized_len: normalized.len(),
            sample: normalized,
            files: vec![file],
            repo_ids,
        });
    }

    let mut out = Vec::new();
    for builders in groups.into_values() {
        for mut builder in builders {
            if builder.files.len() <= 1 {
                continue;
            }
            if options.cross_repo_only && builder.repo_ids.len() < 2 {
                continue;
            }

            builder
                .files
                .sort_by(|a, b| (a.repo_id, &a.path).cmp(&(b.repo_id, &b.path)));
            out.push(DuplicateGroup {
                content_hash: builder.content_hash,
                normalized_len: builder.normalized_len,
                files: builder.files,
            });
        }
    }

    out.sort_by(|a, b| {
        (a.content_hash, a.normalized_len, a.files.len()).cmp(&(
            b.content_hash,
            b.normalized_len,
            b.files.len(),
        ))
    });
    Ok(out)
}

fn repo_label(root: &Path, id: usize) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("repo{id}"))
}

fn walk_repo(
    dir: &Path,
    repo: &Repo,
    options: &ScanOptions,
    out: &mut Vec<RepoFile>,
    visited_dirs: &mut HashSet<PathBuf>,
) -> io::Result<()> {
    match classify_path(dir, options.follow_symlinks)? {
        PathKind::Dir => {}
        PathKind::Skip => return Ok(()),
        PathKind::File => {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("expected directory: {}", dir.display()),
            ));
        }
    }

    let Some(dir_name) = dir.file_name().and_then(|s| s.to_str()) else {
        return Ok(());
    };
    if options.ignore_dirs.contains(dir_name) {
        return Ok(());
    }

    if options.follow_symlinks {
        let canonical = fs::canonicalize(dir)?;
        if !visited_dirs.insert(canonical) {
            return Ok(());
        }
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        match classify_path(&path, options.follow_symlinks)? {
            PathKind::Dir => {
                let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if options.ignore_dirs.contains(dir_name) {
                    continue;
                }
                walk_repo(&path, repo, options, out, visited_dirs)?;
            }
            PathKind::File => {
                out.push(RepoFile {
                    repo_id: repo.id,
                    repo_label: repo.label.clone(),
                    root: repo.root.clone(),
                    abs_path: path,
                });
            }
            PathKind::Skip => {}
        }
    }

    Ok(())
}

enum PathKind {
    Dir,
    File,
    Skip,
}

fn classify_path(path: &Path, follow_symlinks: bool) -> io::Result<PathKind> {
    let meta = fs::symlink_metadata(path)?;
    let file_type = meta.file_type();
    if file_type.is_symlink() {
        if !follow_symlinks {
            return Ok(PathKind::Skip);
        }
        match fs::metadata(path) {
            Ok(target_meta) => {
                if target_meta.is_dir() {
                    Ok(PathKind::Dir)
                } else if target_meta.is_file() {
                    Ok(PathKind::File)
                } else {
                    Ok(PathKind::Skip)
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(PathKind::Skip),
            Err(err) => Err(err),
        }
    } else if meta.is_dir() {
        Ok(PathKind::Dir)
    } else if meta.is_file() {
        Ok(PathKind::File)
    } else {
        Ok(PathKind::Skip)
    }
}

fn make_rel_path(root: &Path, abs_path: &Path) -> String {
    abs_path
        .strip_prefix(root)
        .unwrap_or(abs_path)
        .to_string_lossy()
        .to_string()
}

fn normalize_whitespace(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if !b.is_ascii_whitespace() {
            out.push(b);
        }
    }
    out
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn normalize_whitespace_removes_ascii_whitespace() {
        let input = b"a \n\tb\r\nc";
        assert_eq!(normalize_whitespace(input), b"abc");
    }

    #[test]
    fn finds_duplicates_within_single_repo() -> io::Result<()> {
        let root = temp_dir("single");
        fs::create_dir_all(&root)?;
        fs::write(root.join("a.txt"), "a b\nc")?;
        fs::write(root.join("b.txt"), "ab\tc")?;
        fs::write(root.join("c.txt"), "different")?;

        let options = ScanOptions::default();
        let groups = find_duplicate_files(&[root], &options)?;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        Ok(())
    }

    #[test]
    fn finds_cross_repo_duplicates_when_enabled() -> io::Result<()> {
        let repo_a = temp_dir("repo_a");
        let repo_b = temp_dir("repo_b");
        fs::create_dir_all(&repo_a)?;
        fs::create_dir_all(&repo_b)?;

        fs::write(repo_a.join("same.txt"), "a b\nc")?;
        fs::write(repo_b.join("same.txt"), "ab\tc")?;
        fs::write(repo_b.join("diff.txt"), "different")?;

        let mut options = ScanOptions::default();
        options.cross_repo_only = true;

        let groups = find_duplicate_files(&[repo_a, repo_b], &options)?;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        Ok(())
    }

    fn temp_dir(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("code-checker-core-{suffix}-{nanos}"))
    }
}
