//! Errors for the conformance harness.

use std::io;
use std::path::PathBuf;

/// Everything that can go wrong loading the suite or the ledger.
///
/// Semantic problems with a well-formed ledger (missing entries, stale `until`,
/// skip-by-tag, …) are not errors but [`crate::Violation`]s, so that all of them
/// can be reported at once.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("{path}: {source}")]
    IoAt {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("ledger is not valid TOML: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("{file}: invalid JSON: {source}")]
    Json {
        file: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("{file}: {message}")]
    SuiteFile { file: String, message: String },

    #[error("{file} test #{index}: {message}")]
    SuiteTest {
        file: String,
        index: usize,
        message: String,
    },

    #[error("ledger: {context}: {message}")]
    Ledger { context: String, message: String },

    #[error("{path}: {message}")]
    Spec { path: PathBuf, message: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
