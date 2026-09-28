use thiserror::Error;

/// Failure while installing a corpus or choosing a locale.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A requested locale is not one the corpus was built for.
    #[error("locale {0:?} is not supported")]
    UnknownLocale(String),
    /// The corpus was built with `Emit::NativeFiles`, which embeds nothing.
    #[error("the catalog for locale {0:?} is not embedded")]
    NotEmbedded(String),
    /// A compiled catalog is invalid or was built for another manifest.
    #[error("invalid catalog for locale {locale:?}: {source}")]
    Catalog {
        /// The locale the corpus names.
        locale: String,
        /// The reader's error.
        #[source]
        source: mf2::CatalogError,
    },
    /// `install` was already called with another corpus.
    #[error("another corpus is already installed")]
    AnotherCorpus,
}
