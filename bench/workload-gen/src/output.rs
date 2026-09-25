//! An in-memory set of generated files, written to disk in one step.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::error::Error;

/// Marker file at the root of every output directory written by this tool.
pub const MARKER: &str = ".workload-gen";

/// Generated files by relative path (`/`-separated), in path order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Files {
    map: BTreeMap<String, Vec<u8>>,
}

impl Files {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces one file.
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.map.insert(path.into(), bytes.into());
    }

    /// Adds every file of `other` under `prefix/`.
    pub fn extend_under(&mut self, prefix: &str, other: Self) {
        for (path, bytes) in other.map {
            self.map.insert(format!("{prefix}/{path}"), bytes);
        }
    }

    /// Adds every file of `other` at its own path.
    pub fn extend(&mut self, other: Self) {
        self.map.extend(other.map);
    }

    /// One file's contents.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.map.get(path).map(Vec::as_slice)
    }

    /// All files in path order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.map.iter().map(|(p, b)| (p.as_str(), b.as_slice()))
    }

    /// Number of files.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Directories this set owns and replaces wholesale on write: `locales/`,
    /// `ftl/`, `json/`, and `src/` / `style/` of every generated crate. Build output
    /// (`<app>/target/`) is never touched.
    fn owned_dirs(&self) -> BTreeSet<String> {
        let mut dirs = BTreeSet::new();
        for path in self.map.keys() {
            let mut parts = path.split('/');
            match (parts.next(), parts.next(), parts.next()) {
                (Some(top @ ("locales" | "ftl" | "json")), Some(_), _) => {
                    dirs.insert(top.to_owned());
                }
                (Some(krate), Some(sub @ ("src" | "style")), Some(_)) => {
                    dirs.insert(format!("{krate}/{sub}"));
                }
                _ => {}
            }
        }
        dirs
    }

    /// Writes the set under `dir`. Refuses a non-empty directory that this
    /// tool did not create; otherwise replaces the directories it owns.
    pub fn write_to(&self, dir: &Path, marker: &str) -> Result<(), Error> {
        if dir.exists() {
            let non_empty = std::fs::read_dir(dir)?.next().is_some();
            if non_empty && !dir.join(MARKER).is_file() {
                return Err(Error::Knobs(format!(
                    "refusing to write into {}: not empty and not created by workload-gen",
                    dir.display()
                )));
            }
            for owned in self.owned_dirs() {
                let path = dir.join(&owned);
                if path.exists() {
                    std::fs::remove_dir_all(&path)?;
                }
            }
        }
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(MARKER), marker)?;
        for (path, bytes) in &self.map {
            let full = dir.join(path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full, bytes)?;
        }
        Ok(())
    }
}
