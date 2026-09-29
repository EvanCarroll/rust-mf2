//! The crate's errors: what `compile_str` refuses (`compile`), and what can
//! go wrong between a catalog's bytes and a rendered message (the Leptos
//! layer).

#[cfg(feature = "compile")]
use alloc::vec::Vec;

#[cfg(feature = "compile")]
use mf2_model::{Diagnostic, ErrorKind};

/// Why [`crate::compile_str`] refused a message.
#[cfg(feature = "compile")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CompileError {
    /// The source is not well-formed (syntax errors) or not valid
    /// (data-model errors): the build refuses to ship it, as the suite's
    /// error tests expect.
    #[error("the message has {} syntax or data-model error(s)", .0.len())]
    Invalid(Vec<Diagnostic>),
    /// The catalog writer refused the model (e.g. a U+0000).
    #[error("catalog writer: {0}")]
    Write(#[from] mf2_catalog::WriteError),
    /// The written catalog did not load (a bug: the writer and the reader
    /// disagree).
    #[error("catalog reader: {0}")]
    Load(#[from] mf2_catalog::CatalogError),
    /// The locale data could not be read.
    #[error("locale data: {0}")]
    LocaleData(#[from] mf2_locale_data::Error),
}

#[cfg(feature = "compile")]
impl CompileError {
    /// The MF2 error kinds of an [`CompileError::Invalid`] message, in the
    /// order reported (empty for the other variants).
    pub fn kinds(&self) -> Vec<ErrorKind> {
        match self {
            CompileError::Invalid(d) => d.iter().map(|d| d.kind).collect(),
            _ => Vec::new(),
        }
    }
}

/// Why a catalog could not be installed, or a locale could not be switched
/// to.
///
/// None of these is a panic: the client path is panic-free, and a missing or
/// skewed catalog degrades to the source locale rather than taking the page
/// down.
#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// [`install`](crate::leptos::install) has not run, so there is no
    /// manifest hash to check the catalog against.
    #[error("mf2: install() has not run")]
    NotInstalled,
    /// The catalog is not this build's: its header carries another
    /// `manifest_hash`. A deploy skew — reload, never misread.
    #[error("mf2: the catalog was compiled against a different manifest")]
    ManifestMismatch,
    /// The bytes are not a catalog this reader accepts.
    #[error("mf2: {0}")]
    Malformed(#[from] mf2_catalog::CatalogError),
    /// No locale of that tag was built into this application.
    #[error("mf2: no such locale")]
    UnknownLocale,
    /// The catalog could not be fetched.
    #[error("mf2: the catalog could not be fetched")]
    Fetch,
}

#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
impl LoadError {
    /// A reader error, with the one case that is not "malformed" kept
    /// distinct: a skewed deploy is recoverable by reloading, and the client
    /// says so.
    pub(crate) fn from_reader(error: mf2_catalog::CatalogError) -> LoadError {
        match error {
            mf2_catalog::CatalogError::ManifestMismatch => LoadError::ManifestMismatch,
            other => LoadError::Malformed(other),
        }
    }
}
