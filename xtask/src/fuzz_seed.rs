//! `cargo xtask fuzz-seed`: writes the seed corpus of the `parse` fuzz target
//! (`fuzz/corpus/parse/`, git-ignored): every `src` of the vendored WG suite
//! and every message of the committed reference workload, one file each.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use mf2_conformance::{SUITE_DIR, Suite};

use crate::error::{Error, Result};

pub(crate) fn run(root: &Path) -> Result<()> {
    let dir = root.join("fuzz/corpus/parse");
    fs::create_dir_all(&dir).map_err(|source| Error::IoAt {
        path: dir.clone(),
        source,
    })?;
    let suite = Suite::load(&root.join(SUITE_DIR))?;
    let workload_path = root.join("bench/corpora/workload-1600.json");
    let text = fs::read_to_string(&workload_path).map_err(|source| Error::IoAt {
        path: workload_path.clone(),
        source,
    })?;
    let workload: BTreeMap<String, String> =
        serde_json::from_str(&text).map_err(|e| Error::Json {
            path: workload_path.clone(),
            message: e.to_string(),
        })?;
    let sources = suite
        .tests()
        .iter()
        .map(|t| t.src.as_str())
        .chain(workload.values().map(String::as_str));
    let mut n = 0usize;
    for (i, src) in sources.enumerate() {
        let path = dir.join(format!("seed-{i:05}"));
        fs::write(&path, src).map_err(|source| Error::IoAt { path, source })?;
        n += 1;
    }
    eprintln!("fuzz-seed: wrote {n} files to {}", dir.display());
    Ok(())
}
