//! What a native build (`mf2_build::Emit::Native`, `Emit::NativeFiles`)
//! generates as one value: the corpus's identity, locales, registry and
//! catalogs, so that a native application passes one thing around instead
//! of six.

use mf2_catalog::Dir;
use mf2_runtime::Registry;

use crate::LanguageMatching;

/// A corpus as a native build generates it: the generated module's
/// `CORPUS`. Never written by hand; read it through its methods.
#[derive(Clone, Copy)]
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
    /// The host this corpus formats through (the generated `host::HOST`):
    /// with dates the one that resolves a named time zone, else the plain
    /// one, so that a corpus no date can reach links no time-zone database
    /// (`plan/01` §4.1). `None` in a corpus built by hand, which then
    /// formats through the plain native host.
    #[cfg_attr(
        not(all(
            feature = "host-std",
            any(feature = "native", feature = "axum", feature = "ssr")
        )),
        allow(dead_code, reason = "read where a corpus is formatted from")
    )]
    host: Option<&'static dyn mf2_runtime::Host>,
}

/// Everything but the host, which is a `&dyn` with nothing to show.
impl core::fmt::Debug for Corpus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Corpus")
            .field("source_locale", &self.source_locale)
            .field("manifest_hash", &self.manifest_hash)
            .field("locales", &self.locales)
            .field("registry", &self.registry)
            .field("catalogs", &self.catalogs)
            .field("language_matching", &self.language_matching)
            .finish_non_exhaustive()
    }
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
            host: None,
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

    /// The same, formatting through `host`: what the generated module
    /// calls, with its own `host::HOST`. A corpus without one formats
    /// through the plain native host, whose named zones are *Bad Option*.
    #[doc(hidden)]
    #[must_use]
    pub const fn with_host(mut self, host: &'static dyn mf2_runtime::Host) -> Corpus {
        self.host = Some(host);
        self
    }

    /// The host a corpus is formatted through.
    #[cfg(all(
        feature = "host-std",
        any(feature = "native", feature = "axum", feature = "ssr")
    ))]
    pub(crate) fn host(&self) -> &'static dyn mf2_runtime::Host {
        self.host.unwrap_or(&mf2_host_std::HOST)
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
