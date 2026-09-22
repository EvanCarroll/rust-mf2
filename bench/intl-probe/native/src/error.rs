//! Errors of the native tool.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("message {src:?} does not compile: {kinds:?}")]
    Invalid { src: String, kinds: Vec<String> },
    #[error("catalog writer: {0}")]
    Write(#[from] mf2_catalog::WriteError),
    #[error("catalog reader: {0}")]
    Load(#[from] mf2_catalog::CatalogError),
    #[error("locale data: {0}")]
    LocaleData(#[from] mf2_locale_data::Error),
    #[error("compile: {0}")]
    Compile(#[from] mf2::CompileError),
    #[error("{0}")]
    Input(String),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// Attaches a path to an I/O error.
pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.into();
    move |source| Error::Io { path, source }
}

/// Attaches a path to a JSON error.
pub(crate) fn json(path: impl Into<PathBuf>) -> impl FnOnce(serde_json::Error) -> Error {
    let path = path.into();
    move |source| Error::Json { path, source }
}
