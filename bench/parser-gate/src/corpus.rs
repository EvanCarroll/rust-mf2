//! The committed corpora (plans/05-tooling.md §1): `bench/corpora/suite.json`,
//! `bench/corpora/workload-1600.json`, and the placeholder-free subset of the
//! latter. Nothing is generated at bench time.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// File name of the suite corpus inside the corpora directory.
pub const SUITE_FILE: &str = "suite.json";
/// File name of the workload corpus inside the corpora directory.
pub const WORKLOAD_FILE: &str = "workload-1600.json";

/// Which corpus a row measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum CorpusId {
    /// The `src` of every WG suite test (`suite.json`).
    Suite,
    /// The 1,600-message reference workload (`workload-1600.json`).
    Workload,
    /// The workload's messages without `{` that do not start with `.`.
    PlaceholderFree,
}

impl CorpusId {
    /// All corpora, in report order.
    pub const ALL: [Self; 3] = [Self::Suite, Self::Workload, Self::PlaceholderFree];

    /// Stable key (JSON, command line).
    pub fn key(self) -> &'static str {
        match self {
            Self::Suite => "suite",
            Self::Workload => "workload",
            Self::PlaceholderFree => "placeholder-free",
        }
    }
}

/// One corpus: its messages, in file order.
#[derive(Debug, Clone)]
pub struct Corpus {
    /// Which corpus.
    pub id: CorpusId,
    /// The message sources.
    pub messages: Vec<String>,
}

impl Corpus {
    /// Total UTF-8 bytes of all messages.
    pub fn bytes(&self) -> usize {
        self.messages.iter().map(String::len).sum()
    }

    /// The row label used in the tables, e.g. `462 suite messages (14.6 KB)`.
    pub fn label(&self) -> String {
        let n = crate::report::thousands(self.messages.len() as u64);
        let kb = self.bytes() as f64 / 1000.0;
        match self.id {
            CorpusId::Suite => format!("{n} suite messages ({kb:.1} KB)"),
            CorpusId::Workload => format!("{n}-message workload ({kb:.1} KB)"),
            CorpusId::PlaceholderFree => format!("its {n} placeholder-free messages ({kb:.1} KB)"),
        }
    }
}

/// One entry of `suite.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SuiteEntry {
    /// Suite file, relative to `test/tests`, `/`-separated.
    pub file: String,
    /// Index of the test in that file's `tests` array.
    pub index: usize,
    /// The message source.
    pub src: String,
}

/// The loaded corpora.
#[derive(Debug, Clone)]
pub struct Corpora {
    /// `suite.json` as committed (file and index kept for the correctness join).
    pub suite_entries: Vec<SuiteEntry>,
    /// [`CorpusId::Suite`].
    pub suite: Corpus,
    /// [`CorpusId::Workload`].
    pub workload: Corpus,
    /// [`CorpusId::PlaceholderFree`].
    pub placeholder_free: Corpus,
}

impl Corpora {
    /// Loads `suite.json` and `workload-1600.json` from `dir` and derives the
    /// placeholder-free subset.
    pub fn load(dir: &Path) -> Result<Self> {
        let suite_path = dir.join(SUITE_FILE);
        let suite_entries: Vec<SuiteEntry> = parse(&suite_path)?;
        if suite_entries.is_empty() {
            return Err(corpus_error(&suite_path, "no entries"));
        }
        let workload_path = dir.join(WORKLOAD_FILE);
        // `BTreeMap<String, _>` iterates in bytewise id order, which is the
        // order the generator writes (= `MsgId` order).
        let workload: BTreeMap<String, String> = parse(&workload_path)?;
        if workload.is_empty() {
            return Err(corpus_error(&workload_path, "no messages"));
        }
        let workload: Vec<String> = workload.into_values().collect();
        let placeholder_free: Vec<String> = workload
            .iter()
            .filter(|m| is_placeholder_free(m))
            .cloned()
            .collect();
        let suite = suite_entries.iter().map(|e| e.src.clone()).collect();
        Ok(Self {
            suite_entries,
            suite: Corpus {
                id: CorpusId::Suite,
                messages: suite,
            },
            workload: Corpus {
                id: CorpusId::Workload,
                messages: workload,
            },
            placeholder_free: Corpus {
                id: CorpusId::PlaceholderFree,
                messages: placeholder_free,
            },
        })
    }

    /// The corpus `id`.
    pub fn get(&self, id: CorpusId) -> &Corpus {
        match id {
            CorpusId::Suite => &self.suite,
            CorpusId::Workload => &self.workload,
            CorpusId::PlaceholderFree => &self.placeholder_free,
        }
    }
}

/// The placeholder-free subset's definition (`bench/workload-gen/README.md`):
/// no `{` anywhere (no variable, no markup) and not starting with `.` (not a
/// complex message).
pub fn is_placeholder_free(src: &str) -> bool {
    !src.contains('{') && !src.starts_with('.')
}

fn parse<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|e| corpus_error(path, &e.to_string()))
}

fn corpus_error(path: &Path, message: &str) -> Error {
    Error::Corpus {
        path: PathBuf::from(path),
        message: message.to_owned(),
    }
}

/// The committed corpora directory, `<repo>/bench/corpora`.
pub fn default_dir() -> PathBuf {
    crate::repo_root().join("bench").join("corpora")
}

#[cfg(test)]
mod tests {
    use super::{Corpora, CorpusId, default_dir, is_placeholder_free};

    #[test]
    fn placeholder_free_definition() {
        assert!(is_placeholder_free("Send"));
        assert!(is_placeholder_free("Can’t reach the server."));
        assert!(!is_placeholder_free("{$count} members"));
        assert!(!is_placeholder_free("Press {#kbd}Esc{/kbd}"));
        assert!(!is_placeholder_free(
            ".input {$n :integer} .match $n * {{x}}"
        ));
        assert!(!is_placeholder_free(".x"));
    }

    #[test]
    fn committed_corpora_have_the_documented_sizes() {
        let c = Corpora::load(&default_dir()).expect("committed corpora load");
        assert_eq!(c.get(CorpusId::Suite).messages.len(), 462);
        assert_eq!(c.suite_entries.len(), 462);
        assert_eq!(c.get(CorpusId::Workload).messages.len(), 1600);
        // bench/workload-gen/README.md: "1,256 of 1,600".
        assert_eq!(c.get(CorpusId::PlaceholderFree).messages.len(), 1256);
        for id in CorpusId::ALL {
            assert_eq!(c.get(id).id, id);
        }
    }
}
