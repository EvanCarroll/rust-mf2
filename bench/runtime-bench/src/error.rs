//! Errors of `runtime-bench`.

use std::path::PathBuf;

/// What can go wrong.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// A measurement could not be made (message says why).
    #[error("{0}")]
    Bench(String),
    /// Writing a report.
    #[error("{path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Serializing a report.
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// This crate's `Result`.
pub(crate) type Result<T> = std::result::Result<T, Error>;
