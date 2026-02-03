use std::env;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub(crate) fn resolve_path(p: &Path) -> io::Result<PathBuf> {
    let base = if p.is_absolute() {
        PathBuf::new()
    } else {
        env::current_dir()?
    };
    let normalized = normalize_path(&base.join(p));
    match fs::canonicalize(&normalized) {
        Ok(canonical) => Ok(canonical),
        Err(_) => Ok(normalized),
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    use std::ffi::OsString;

    let mut parts: Vec<OsString> = Vec::new();
    let mut min_len = 0usize;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                parts.clear();
                parts.push(prefix.as_os_str().to_owned());
                min_len = parts.len();
            }
            Component::RootDir => {
                parts.push(component.as_os_str().to_owned());
                min_len = parts.len();
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.len() > min_len {
                    parts.pop();
                } else if min_len == 0 {
                    parts.push(component.as_os_str().to_owned());
                }
            }
            Component::Normal(part) => parts.push(part.to_owned()),
        }
    }

    let mut out = PathBuf::new();
    for part in parts {
        out.push(part);
    }
    out
}
