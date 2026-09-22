//! What can stop a command.

use std::io;
use std::path::PathBuf;

/// The result of a command.
pub(crate) type Result<T> = std::result::Result<T, Error>;

/// Why a command failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The build itself.
    #[error(transparent)]
    Build(#[from] mf2_build::Error),

    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: io::Error,
    },

    /// The corpus has errors; they are already on stderr, so the command
    /// only has to exit non-zero.
    #[error("the corpus has errors")]
    Corpus,

    /// A catalog could not be read.
    #[error("{path}: {source}")]
    Catalog {
        /// The catalog.
        path: PathBuf,
        /// What the reader said.
        #[source]
        source: mf2_catalog::CatalogError,
    },

    /// A catalog could not be decoded.
    #[error("{path}: message {id}: {source}")]
    Decode {
        /// The catalog.
        path: PathBuf,
        /// Which message.
        id: u32,
        /// What the decoder said.
        #[source]
        source: mf2_catalog::DecodeError,
    },

    /// An argument names something that is not there.
    #[error("{0}")]
    Usage(String),
}

impl Error {
    /// An I/O error that names its file.
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Error {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

/// Reads a file, naming it on failure.
pub(crate) fn read(path: &std::path::Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| Error::io(path, source))
}

/// Reads a file's bytes, naming it on failure.
pub(crate) fn read_bytes(path: &std::path::Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::io(path, source))
}

/// Writes a file, naming it on failure, creating its directory.
pub(crate) fn write(path: &std::path::Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
    }
    std::fs::write(path, bytes).map_err(|source| Error::io(path, source))
}
