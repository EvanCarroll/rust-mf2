//! Where the specification's files are — the one path helper every consumer
//! of `spec/` goes through.
//!
//! Upstream #1112 restricts redistribution of the spec text, so it is not in
//! the tree: `cargo xtask spec-sync`
//! fetches it at the pinned commit, checks it against the digests the `PIN`
//! records, and writes it to [`SPEC_DIR`] with the commit in [`SPEC_STAMP`].
//! [`spec_dir`] refuses a missing cache, or one from another pin, naming the
//! command — a consumer fails, it never skips.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The cached specification directory, relative to the repository root.
pub const SPEC_DIR: &str = "target/xtask-cache/message-format-wg-spec/spec";

/// The file holding the commit [`SPEC_DIR`] was fetched at.
pub const SPEC_STAMP: &str = "target/xtask-cache/message-format-wg-spec/COMMIT";

/// The pin the cache must match.
pub const PIN: &str = "third_party/message-format-wg/PIN";

/// The specification directory of the repository at `root`, once the cache
/// is there and at the pinned commit.
pub fn spec_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(SPEC_DIR);
    let missing = |reason: String| Error::SpecMissing {
        dir: dir.clone(),
        reason,
    };
    let pin_path = root.join(PIN);
    let pin = fs::read_to_string(&pin_path).map_err(|source| Error::IoAt {
        path: pin_path.clone(),
        source,
    })?;
    let pinned = pin
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "commit").then(|| v.trim().to_owned())
        })
        .ok_or_else(|| Error::Spec {
            path: pin_path,
            message: "no `commit` field".to_owned(),
        })?;
    let stamp =
        fs::read_to_string(root.join(SPEC_STAMP)).map_err(|_| missing("no cache".to_owned()))?;
    let stamp = stamp.trim();
    if !stamp.eq_ignore_ascii_case(&pinned) {
        return Err(missing(format!(
            "the cache is at {stamp}, the PIN at {pinned}"
        )));
    }
    if !dir.is_dir() {
        return Err(missing("no cache".to_owned()));
    }
    Ok(dir)
}

/// The path of `file` (e.g. `"message.abnf"`, `"data-model/message.json"`)
/// inside the specification directory of the repository at `root`.
pub fn spec_path(root: &Path, file: &str) -> Result<PathBuf> {
    Ok(spec_dir(root)?.join(file))
}

/// The text of `file` inside the specification directory.
pub fn read_spec(root: &Path, file: &str) -> Result<String> {
    let path = spec_path(root, file)?;
    fs::read_to_string(&path).map_err(|source| Error::IoAt { path, source })
}

/// `spec/message.abnf`.
pub const ABNF: &str = "message.abnf";

/// `spec/data-model/message.json` (the data model's JSON Schema).
pub const DATA_MODEL_SCHEMA: &str = "data-model/message.json";

#[cfg(test)]
mod tests {
    use super::{PIN, SPEC_DIR, SPEC_STAMP, spec_dir};
    use crate::error::Error;

    #[test]
    fn a_missing_or_stale_cache_names_the_command() {
        let root = std::env::temp_dir().join(format!("mf2-spec-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(PIN, "upstream = x\ncommit   = aaaa\n");
        let refused = |why: &str| {
            let e = spec_dir(&root).unwrap_err();
            assert!(matches!(e, Error::SpecMissing { .. }), "{why}: {e}");
            assert!(
                e.to_string().contains("cargo xtask spec-sync"),
                "{why}: {e}"
            );
        };
        refused("no cache");
        write(&format!("{SPEC_DIR}/message.abnf"), "");
        refused("no stamp");
        write(SPEC_STAMP, "bbbb\n");
        refused("another pin");
        write(SPEC_STAMP, "AAAA\n");
        assert_eq!(spec_dir(&root).unwrap(), root.join(SPEC_DIR));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
