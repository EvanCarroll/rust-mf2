//! Errors of the case generator.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{src} ({locale}): {source}")]
    Compile {
        src: String,
        locale: String,
        #[source]
        source: mf2::CompileError,
    },
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}
