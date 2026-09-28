use std::path::PathBuf;

use thiserror::Error;

/// Failure while loading or selecting a native application's catalog.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NativeError {
    /// A requested locale is not one the corpus was built for.
    #[error("locale {0:?} is not supported")]
    UnknownLocale(String),
    /// The corpus has no catalog entry for one of its locales.
    #[error("no catalog was built for locale {0:?}")]
    MissingCatalog(String),
    /// The corpus was built with `Emit::NativeFiles`, which does not embed
    /// catalogs: load them with `from_directory`.
    #[error("the catalog for locale {0:?} is not embedded; load it with from_directory")]
    NotEmbedded(String),
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
        source: mf2::CatalogError,
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
