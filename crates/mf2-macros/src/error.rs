//! Why an expansion could not use the manifest.
//!
//! Every variant becomes one `compile_error!` spanned at the call site's id
//! literal, so the message an application sees names a file it can act on.

use std::path::PathBuf;

/// The manifest could not be read, or is not the one the call site was
/// generated against.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ManifestError {
    /// The baked path does not exist and no relocated copy was found.
    #[error(
        "cannot read the message manifest at {path}: {source}\n\
         the i18n crate's build script writes it — build that crate, or, if its \
         target directory moved, rebuild it there"
    )]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The file is not a well-formed `manifest.mf2m`.
    #[error("the message manifest at {path} is not readable: {source}")]
    Invalid {
        path: PathBuf,
        #[source]
        source: mf2_catalog::ManifestError,
    },
    /// The manifest's hash is not the one baked into the wrapper: the
    /// generated module and the manifest on disk are from different builds,
    /// and using either would check the call site against the wrong corpus.
    #[error(
        "the message manifest at {path} is stale: it hashes to {found:#018x}, but this \
         `tr!` was generated for {expected:#018x}\n\
         the generated module and the manifest are from different builds — rebuild the \
         i18n crate (in an editor: restart the proc-macro server)"
    )]
    Stale {
        path: String,
        found: u64,
        expected: u64,
    },
    /// The inline manifest literal (`bytes b"…"`) is not a byte string.
    #[error("the inline manifest is not a byte-string literal: {0}")]
    Literal(String),
}
