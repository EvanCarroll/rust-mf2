//! What a native build (`mf2_build::Emit::Native`, `Emit::NativeFiles`)
//! generates as one value: the corpus's identity, locales, registry and
//! catalogs, so that a native application passes one thing around instead
//! of six.

use mf2_catalog::Dir;
use mf2_runtime::Registry;

use crate::LanguageMatching;

/// A corpus as a native build generates it: the generated module's
/// `CORPUS`. Never written by hand; read it through its methods.
#[derive(Clone, Copy, Debug)]
pub struct Corpus {
    source_locale: &'static str,
    manifest_hash: u64,
    locales: &'static [(&'static str, Dir)],
    registry: &'static Registry,
    catalogs: &'static [CatalogFile],
    /// The build's cut of CLDR's language-matching data for these locales
    /// (the generated `LANGUAGE_MATCHING`): what a native application
    /// matches a language with, as CLDR's whole table would.
    #[cfg_attr(
        not(feature = "native"),
        allow(dead_code, reason = "read by `mf2::native`")
    )]
    language_matching: Option<&'static LanguageMatching>,
}

/// One locale's compiled catalog: its file name and, when the build
/// embedded it (`Emit::Native`), its bytes.
#[derive(Clone, Copy, Debug)]
pub struct CatalogFile {
    tag: &'static str,
    file_name: &'static str,
    bytes: Option<&'static [u8]>,
}

impl Corpus {
    /// What the generated module calls.
    #[doc(hidden)]
    #[must_use]
    pub const fn new(
        source_locale: &'static str,
        manifest_hash: u64,
        locales: &'static [(&'static str, Dir)],
        registry: &'static Registry,
        catalogs: &'static [CatalogFile],
    ) -> Corpus {
        Corpus {
            source_locale,
            manifest_hash,
            locales,
            registry,
            catalogs,
            language_matching: None,
        }
    }

    /// The same, with the build's cut of CLDR's language-matching data for
    /// its locales: what the generated module calls. A corpus without one
    /// matches with no data.
    #[doc(hidden)]
    #[must_use]
    pub const fn with_language_matching(mut self, matching: &'static LanguageMatching) -> Corpus {
        self.language_matching = Some(matching);
        self
    }

    /// What a language is matched with among these locales
    /// (plans/19-native-and-terminal.md §9).
    #[cfg(feature = "native")]
    pub(crate) fn language_matching(&self) -> &'static LanguageMatching {
        self.language_matching.unwrap_or(&LanguageMatching::EMPTY)
    }

    /// The locale the manifest was built from, the final fallback.
    #[must_use]
    pub const fn source_locale(&self) -> &'static str {
        self.source_locale
    }

    /// The hash every catalog of this corpus carries.
    #[must_use]
    pub const fn manifest_hash(&self) -> u64 {
        self.manifest_hash
    }

    /// Every locale, with its base direction, in the build's order.
    #[must_use]
    pub const fn locales(&self) -> &'static [(&'static str, Dir)] {
        self.locales
    }

    /// The function registry the catalogs were built against.
    #[must_use]
    pub const fn registry(&self) -> &'static Registry {
        self.registry
    }

    /// One entry per locale.
    #[must_use]
    pub const fn catalogs(&self) -> &'static [CatalogFile] {
        self.catalogs
    }
}

impl CatalogFile {
    /// What the generated module calls.
    #[doc(hidden)]
    #[must_use]
    pub const fn new(
        tag: &'static str,
        file_name: &'static str,
        bytes: Option<&'static [u8]>,
    ) -> CatalogFile {
        CatalogFile {
            tag,
            file_name,
            bytes,
        }
    }

    /// The locale's BCP 47 tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        self.tag
    }

    /// The content-hashed file name the build wrote.
    #[must_use]
    pub const fn file_name(&self) -> &'static str {
        self.file_name
    }

    /// The catalog's bytes, when the build embedded them.
    #[must_use]
    pub const fn bytes(&self) -> Option<&'static [u8]> {
        self.bytes
    }
}
