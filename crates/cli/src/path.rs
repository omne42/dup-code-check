use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn resolve_path(p: &Path) -> io::Result<PathBuf> {
    let base = if p.is_absolute() {
        PathBuf::new()
    } else {
        env::current_dir()?
    };
    fs::canonicalize(base.join(p))
}
