//! Errors of the catalog bench.

use std::path::PathBuf;

/// Everything that can stop a run. A failed budget is not an error: it is a
/// result, reported with exit status 1 by the binary.
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

    /// The JSON report could not be written.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The workload generator failed.
    #[error("workload generator: {0}")]
    Workload(#[from] workload_gen::Error),

    /// The catalog writer refused the workload.
    #[error("catalog writer: {0}")]
    Write(#[from] mf2_catalog::WriteError),

    /// `Catalog::new` rejected a catalog the writer produced.
    #[error("catalog `{tag}`: Catalog::new rejected it: {source}")]
    Load {
        /// The locale.
        tag: String,
        /// Why.
        #[source]
        source: mf2_catalog::CatalogError,
    },

    /// The inputs are inconsistent (stale corpus, parse error, wrong hash).
    #[error("corpus: {0}")]
    Corpus(String),

    /// A compressor failed.
    #[error("compressor `{command}`: {message}")]
    Compressor {
        /// What was run.
        command: String,
        /// What went wrong.
        message: String,
    },

    /// A NAMES re-encoding estimate could not parse a catalog.
    #[error("NAMES estimate: {0}")]
    Names(&'static str),

    /// A setting the harness cannot honour.
    #[error("invalid settings: {0}")]
    Settings(String),
}

/// Result alias of this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
