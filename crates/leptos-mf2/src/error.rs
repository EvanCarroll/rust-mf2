//! What can go wrong between a catalog's bytes and a rendered message.
//!
//! None of these is a panic: the client path is panic-free (B12), and a
//! missing or skewed catalog degrades to the source locale rather than
//! taking the page down.

use mf2_catalog::CatalogError;

/// Why a catalog could not be installed, or a locale could not be switched
/// to.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// [`install`](crate::install) has not run, so there is no manifest hash
    /// to check the catalog against.
    #[error("mf2: install() has not run")]
    NotInstalled,
    /// The catalog is not this build's: its header carries another
    /// `manifest_hash` (F6). A deploy skew — reload, never misread.
    #[error("mf2: the catalog was compiled against a different manifest")]
    ManifestMismatch,
    /// The bytes are not a catalog this reader accepts.
    #[error("mf2: {0}")]
    Malformed(#[from] CatalogError),
    /// No locale of that tag was built into this application.
    #[error("mf2: no such locale")]
    UnknownLocale,
    /// The catalog could not be fetched.
    #[error("mf2: the catalog could not be fetched")]
    Fetch,
}

impl LoadError {
    /// A reader error, with the one case that is not "malformed" kept
    /// distinct: a skewed deploy is recoverable by reloading, and the client
    /// says so.
    pub(crate) fn from_reader(error: CatalogError) -> LoadError {
        match error {
            CatalogError::ManifestMismatch => LoadError::ManifestMismatch,
            other => LoadError::Malformed(other),
        }
    }
}
