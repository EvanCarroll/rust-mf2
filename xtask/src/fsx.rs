//! File-system helpers with path-carrying errors.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

fn at(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    }
}

/// The repository root (the parent of `xtask/`).
pub(crate) fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

/// Reads every regular file below `base/<sub>` (a file or a directory) into a
/// map keyed by its `/`-separated path relative to `base`. Missing ⇒ nothing.
pub(crate) fn read_tree(base: &Path, sub: &str) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut out = BTreeMap::new();
    let start = base.join(sub);
    if !start.exists() {
        return Ok(out);
    }
    let mut stack = vec![(start, sub.to_owned())];
    while let Some((path, rel)) = stack.pop() {
        let meta = fs::symlink_metadata(&path).map_err(at(&path))?;
        if meta.is_dir() {
            for entry in fs::read_dir(&path).map_err(at(&path))? {
                let entry = entry.map_err(at(&path))?;
                let name = entry.file_name().to_string_lossy().into_owned();
                stack.push((entry.path(), format!("{rel}/{name}")));
            }
        } else {
            let bytes = fs::read(&path).map_err(at(&path))?;
            out.insert(rel, bytes);
        }
    }
    Ok(out)
}

/// Reads `path` as UTF-8.
pub(crate) fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(at(path))
}

/// Writes `bytes` to `path`, creating parent directories.
pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(at(parent))?;
    }
    fs::write(path, bytes).map_err(at(path))
}

/// Removes a file or a directory tree if it exists.
pub(crate) fn remove(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path).map_err(at(path)),
        Ok(_) => fs::remove_file(path).map_err(at(path)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(at(path)(e)),
    }
}
