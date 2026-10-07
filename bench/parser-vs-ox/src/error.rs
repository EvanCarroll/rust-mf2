//! Errors of the parser gate.

use std::path::PathBuf;

/// Everything that can stop a gate run (a failed gate is not an error: it is
/// a result, reported with a non-zero exit status by the binary).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Writing a report failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Reading an input file failed.
    #[error("{}: {source}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// A JSON input could not be parsed, or the JSON report not written.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Loading the vendored WG suite (for the expected errors) failed.
    #[error("WG suite: {0}")]
    Conformance(#[from] mf2_conformance::Error),

    /// A committed corpus is malformed or out of date.
    #[error("corpus {}: {message}", path.display())]
    Corpus {
        /// The corpus file.
        path: PathBuf,
        /// What is wrong with it.
        message: String,
    },

    /// A setting the harness cannot honour.
    #[error("invalid settings: {0}")]
    Settings(String),
}

/// Result alias of this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
