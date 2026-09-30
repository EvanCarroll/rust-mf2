//! The crate's errors: what `compile_str` refuses (`compile`), what can go
//! wrong between a catalog's bytes and a rendered message (the Leptos
//! layer), and between a native application's catalogs and its locale
//! (`native`).

#[cfg(feature = "native")]
use alloc::string::String;
#[cfg(feature = "compile")]
use alloc::vec::Vec;
#[cfg(feature = "native")]
use std::path::PathBuf;

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

/// Failure while loading or selecting a native application's catalog:
/// `mf2::native::Error` (1.x's name for it was `NativeError`).
#[cfg(feature = "native")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum NativeError {
    /// A requested locale is not one the corpus was built for (or, from a
    /// directory, one whose catalog was not shipped).
    #[error("locale {0:?} is not supported")]
    UnknownLocale(String),
    /// The corpus has no catalog entry for one of its locales.
    #[error("no catalog was built for locale {0:?}")]
    MissingCatalog(String),
    /// The corpus was built with `Emit::NativeFiles`, which does not embed
    /// catalogs: load them from the directory they ship in
    /// (`install_from_directory`, `Catalogs::from_directory`).
    #[error(
        "the catalog for locale {0:?} is not embedded: the build wrote it beside the executable, to load from a directory"
    )]
    NotEmbedded(String),
    /// The app-wide store already holds another corpus: it holds one per
    /// process. `Catalogs` formats any other, explicitly.
    #[error(
        "another corpus is already installed: the store holds one message set per process (format another with mf2::native::Catalogs)"
    )]
    AnotherCorpus,
    /// A catalog file holds a catalog for a different locale.
    #[error("catalog for locale {expected:?} contains locale {actual:?}")]
    LocaleMismatch {
        /// The locale the corpus names.
        expected: String,
        /// The locale in the catalog's header.
        actual: String,
    },
    /// A compiled catalog is invalid or was built for another manifest.
    #[error("invalid catalog for locale {locale:?}: {source}")]
    Catalog {
        /// The locale the corpus names.
        locale: String,
        /// The catalog reader's error.
        source: mf2_catalog::CatalogError,
    },
    /// A catalog file's bytes do not give the content hash in its name
    /// (`<locale>.<hash>.mf2b`): it is from another build, renamed or
    /// copied over, or it was damaged. A rebuild that changes only a
    /// message's text keeps the manifest hash, so this is the check that
    /// tells its catalogs from the old ones.
    #[error(
        "catalog {path:?} is not the one its name promises (its content hashes to {actual}): it is from another build"
    )]
    ContentMismatch {
        /// The catalog's path; its name carries the expected hash.
        path: PathBuf,
        /// The content hash of the bytes read.
        actual: String,
    },
    /// Reading an external catalog file failed.
    #[error("could not read catalog {path:?}: {source}")]
    Io {
        /// The catalog's path.
        path: PathBuf,
        /// The file-system error.
        source: std::io::Error,
    },
}

/// Why a generated `Locale::from_str` refused a tag: none of the
/// application's languages serves it (plans/19-native-and-terminal.md §9,
/// §10). Its text lists the languages there are — "no language of this
/// application matches; it has en, fr" — which is what clap shows for a
/// refused `--lang`. It allocates nothing: it holds the build's own table.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
#[error("no language of this application matches; it has {}", Tags(self.supported))]
pub struct UnknownLocale {
    supported: &'static [(&'static str, mf2_catalog::Dir)],
}

impl UnknownLocale {
    /// What the generated module calls, with its `LOCALES`.
    #[doc(hidden)]
    #[must_use]
    pub const fn new(supported: &'static [(&'static str, mf2_catalog::Dir)]) -> UnknownLocale {
        UnknownLocale { supported }
    }

    /// The application's languages, in the build's order.
    pub fn supported(&self) -> impl ExactSizeIterator<Item = &'static str> + use<> {
        let supported = self.supported;
        supported.iter().map(|(tag, _)| *tag)
    }
}

/// The tags, comma-separated: [`UnknownLocale`]'s text.
struct Tags(&'static [(&'static str, mf2_catalog::Dir)]);

impl core::fmt::Display for Tags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, (tag, _)) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_str(tag)?;
        }
        Ok(())
    }
}
