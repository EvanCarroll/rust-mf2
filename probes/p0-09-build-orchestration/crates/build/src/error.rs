//! Build errors.

use std::path::PathBuf;

/// Why the i18n build failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// File system.
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    /// A required environment variable (cargo sets these for build scripts).
    #[error("environment: {0}")]
    Env(#[from] std::env::VarError),
    /// Resource syntax (plans/05-tooling.md §2 working grammar).
    #[error("{file}:{line}: {message}")]
    Resource {
        /// File.
        file: PathBuf,
        /// 1-based line.
        line: usize,
        /// What is wrong.
        message: String,
    },
    /// MF2 syntax in one message.
    #[error("{file}: message `{id}`: {message}")]
    Message {
        /// File.
        file: PathBuf,
        /// Message id.
        id: String,
        /// What is wrong.
        message: String,
    },
    /// A translation disagrees with the source (plans/05 §3 rules).
    #[error("locale `{locale}`, message `{id}`: {message}")]
    Translation {
        /// Locale.
        locale: String,
        /// Message id.
        id: String,
        /// What is wrong.
        message: String,
    },
    /// A locale directory is missing or broken.
    #[error("locale `{0}`: {1}")]
    Locale(String, String),
}
