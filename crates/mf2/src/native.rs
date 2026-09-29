//! A native application's messages — a command-line tool or a terminal UI,
//! with no Leptos: one generated corpus's catalogs, the reader's language
//! chosen from the system's, and the descriptions `tr!` builds formatted in
//! it.
//!
//! See the user guide's [native applications page](https://evancarroll.github.io/rust-mf2/native-apps.html).
//!
//! The build script runs `mf2_build` with `Emit::Native` (the catalogs
//! embedded in the executable) or `Emit::NativeFiles` (the catalogs shipped
//! beside it), which generates one [`Corpus`] value, `CORPUS`. The locale
//! then belongs to a [`NativeI18n`], not to a process or thread global.
//!
//! ```ignore
//! let mut i18n = mf2::native::NativeI18n::embedded(&my_i18n::CORPUS)?; // the system's locale
//! if let Some(lang) = args.lang.as_deref() {
//!     i18n.set_locale(lang)?; // an unsupported --lang is an error
//! }
//! println!("{}", i18n.format(&my_i18n::tr!("welcome")));
//! ```
//!
//! Native only: the module is `std`, and reads the system's preferred
//! languages and time zone and files beside the executable. With `hydrate`
//! or `csr`, which build the browser's client, the `native` feature is a
//! compile error.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use std::path::Path;

use mf2_catalog::{Catalog, Dir};
use mf2_runtime::{BidiStrategy, FormatContext, FormatError, Formatter, TimeZone};

use crate::{CatalogFile, Corpus, Message};

mod locale;

pub use crate::error::NativeError;
pub use locale::LocaleOrigin;

use locale::{match_locale, negotiate};

/// An app-owned native translation context.
///
/// It holds every catalog of one generated [`Corpus`], validated once, and
/// the active locale. Keep it in the CLI's or the TUI's state; `set_locale`
/// changes what the next `format` produces. Nothing is process-global.
pub struct NativeI18n {
    corpus: &'static Corpus,
    /// One per locale, in `corpus.locales()` order.
    catalogs: Vec<Catalog>,
    cx: FormatContext,
    active: usize,
    source: LocaleOrigin,
}

/// The active locale, where it came from, the catalogs and the formatting
/// settings. The corpus's embedded bytes are left out.
impl fmt::Debug for NativeI18n {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeI18n")
            .field("locale", &self.locale())
            .field("locale_source", &self.source)
            .field("catalogs", &self.catalogs)
            .field("context", &self.cx)
            .finish_non_exhaustive()
    }
}

impl NativeI18n {
    /// Loads the catalogs the build embedded (`mf2_build::Emit::Native`)
    /// and selects the first of the system's preferred locales the corpus
    /// supports, else its source locale.
    ///
    /// Nothing is copied: each catalog reads the executable's own bytes.
    pub fn embedded(corpus: &'static Corpus) -> Result<Self, NativeError> {
        Self::load(corpus, |tag, file| {
            let bytes = file
                .bytes()
                .ok_or_else(|| NativeError::NotEmbedded(tag.to_owned()))?;
            Catalog::from_static(bytes, corpus.manifest_hash()).map_err(|source| {
                NativeError::Catalog {
                    locale: tag.to_owned(),
                    source,
                }
            })
        })
    }

    /// Loads the catalog files the build wrote (`mf2_build::Emit::NativeFiles`,
    /// or `Emit::Native`) from `directory`, under the content-hashed names
    /// the corpus records, and selects a locale as [`NativeI18n::embedded`]
    /// does.
    ///
    /// Each file must hash to the name it is read under: a catalog from
    /// another build — even one that changed only a message's text, which
    /// keeps the manifest hash — is [`NativeError::ContentMismatch`], not
    /// the other build's text.
    pub fn from_directory(
        corpus: &'static Corpus,
        directory: impl AsRef<Path>,
    ) -> Result<Self, NativeError> {
        let directory = directory.as_ref();
        Self::load(corpus, |tag, file| {
            let path = directory.join(file.file_name());
            let bytes = std::fs::read(&path).map_err(|source| NativeError::Io {
                path: path.clone(),
                source,
            })?;
            let actual = mf2_catalog::content_hash(&bytes);
            if name_hash(file.file_name()) != Some(actual.as_str()) {
                return Err(NativeError::ContentMismatch { path, actual });
            }
            Catalog::new(bytes, corpus.manifest_hash()).map_err(|source| NativeError::Catalog {
                locale: tag.to_owned(),
                source,
            })
        })
    }

    fn load(
        corpus: &'static Corpus,
        mut read: impl FnMut(&str, &CatalogFile) -> Result<Catalog, NativeError>,
    ) -> Result<Self, NativeError> {
        let mut catalogs = Vec::with_capacity(corpus.locales().len());
        for &(tag, _) in corpus.locales() {
            let file = corpus
                .catalogs()
                .iter()
                .find(|f| f.tag() == tag)
                .ok_or_else(|| NativeError::MissingCatalog(tag.to_owned()))?;
            let catalog = read(tag, file)?;
            if catalog.locale() != tag {
                return Err(NativeError::LocaleMismatch {
                    expected: tag.to_owned(),
                    actual: catalog.locale().to_owned(),
                });
            }
            catalogs.push(catalog);
        }
        let system = sys_locale::get_locales().collect::<Vec<_>>();
        let (tag, source) = match negotiate(system.iter().map(String::as_str), corpus.locales()) {
            Some(tag) => (tag, LocaleOrigin::System),
            None => (corpus.source_locale(), LocaleOrigin::Source),
        };
        let active = corpus
            .locales()
            .iter()
            .position(|(t, _)| *t == tag)
            .ok_or_else(|| NativeError::UnknownLocale(tag.to_owned()))?;
        let mut cx = FormatContext::new(&mf2_host_std::HOST);
        cx.bidi = BidiStrategy::None;
        cx.time_zone = system_time_zone();
        Ok(NativeI18n {
            corpus,
            catalogs,
            cx,
            active,
            source,
        })
    }

    /// The locale the next `format` uses.
    #[must_use]
    pub fn locale(&self) -> &'static str {
        self.corpus
            .locales()
            .get(self.active)
            .map_or(self.corpus.source_locale(), |(tag, _)| tag)
    }

    /// Where the active locale came from — to tell a user that their system
    /// language is not supported, say.
    #[must_use]
    pub const fn locale_source(&self) -> LocaleOrigin {
        self.source
    }

    /// The active locale's base direction.
    #[must_use]
    pub fn dir(&self) -> Dir {
        self.corpus
            .locales()
            .get(self.active)
            .map_or(Dir::Ltr, |(_, dir)| *dir)
    }

    /// The corpus's source locale, the final fallback.
    #[must_use]
    pub const fn source_locale(&self) -> &'static str {
        self.corpus.source_locale()
    }

    /// The supported locale tags, in the build's order — for a
    /// `--list-locales`, say.
    pub fn available_locales(&self) -> impl ExactSizeIterator<Item = &'static str> + use<> {
        self.corpus.locales().iter().map(|(tag, _)| *tag)
    }

    /// Makes `locale` the active locale: the supported locale it matches
    /// (case-insensitive, `_` read as `-`, `fr-CA` falling back to `fr`).
    /// An unsupported locale is an error and leaves the active one as it
    /// was, so a mistyped `--lang` is reported, not ignored.
    pub fn set_locale(&mut self, locale: &str) -> Result<(), NativeError> {
        let active = match_locale(locale, self.corpus.locales())
            .and_then(|tag| self.corpus.locales().iter().position(|(t, _)| *t == tag))
            .ok_or_else(|| NativeError::UnknownLocale(locale.to_owned()))?;
        self.active = active;
        self.source = LocaleOrigin::Explicit;
        Ok(())
    }

    /// The bidi strategy. [`BidiStrategy::None`] by default: terminals
    /// and logs show the isolation controls of [`BidiStrategy::Default`]
    /// (U+2066–U+2069) as stray characters more often than they apply them.
    #[must_use]
    pub const fn bidi(&self) -> BidiStrategy {
        self.cx.bidi
    }

    /// Sets the bidi strategy for subsequent formatting.
    pub const fn set_bidi(&mut self, bidi: BidiStrategy) {
        self.cx.bidi = bidi;
    }

    /// The default time zone of dates and times: the system's, else UTC.
    #[must_use]
    pub const fn time_zone(&self) -> TimeZone {
        self.cx.time_zone
    }

    /// Sets the default time zone for subsequent formatting.
    pub const fn set_time_zone(&mut self, zone: TimeZone) {
        self.cx.time_zone = zone;
    }

    /// A formatter over the active catalog, for code that needs more than
    /// text: parts, for instance.
    #[must_use]
    pub fn formatter(&self) -> Option<Formatter<'_>> {
        let catalog = self.catalogs.get(self.active)?;
        Some(Formatter::new(catalog, self.corpus.registry(), &self.cx))
    }

    /// Formats a message in the active locale, discarding MF2 errors (the
    /// text shows the fallback the specification defines).
    #[must_use]
    pub fn format(&self, message: &impl Message) -> String {
        self.format_with_errors(message).0
    }

    /// Formats a message in the active locale, with its MF2 errors.
    #[must_use]
    pub fn format_with_errors(&self, message: &impl Message) -> (String, Vec<FormatError>) {
        let mut out = String::new();
        let mut errors = Vec::new();
        if let Some(formatter) = self.formatter() {
            message.write(&formatter, &mut out, &mut errors);
        }
        (out, errors)
    }

    /// The content-hashed file name of `locale`'s catalog, for copying the
    /// catalogs into an application data directory.
    #[must_use]
    pub fn catalog_file_name(&self, locale: &str) -> Option<&'static str> {
        let tag = match_locale(locale, self.corpus.locales())?;
        self.corpus
            .catalogs()
            .iter()
            .find(|f| f.tag() == tag)
            .map(CatalogFile::file_name)
    }
}

/// The content hash in a catalog file name, `<locale>.<hash>.mf2b`; `None`
/// when the name carries none, which no file then matches.
fn name_hash(file_name: &str) -> Option<&str> {
    let (_, hash) = file_name.strip_suffix(".mf2b")?.rsplit_once('.')?;
    (hash.len() == mf2_catalog::CONTENT_HASH_LEN).then_some(hash)
}

/// The system's time zone: its IANA name when it has one, else its current
/// UTC offset, else UTC.
fn system_time_zone() -> TimeZone {
    let Ok(zone) = jiff::tz::TimeZone::try_system() else {
        return TimeZone::UTC;
    };
    if let Some(named) = zone.iana_name().and_then(TimeZone::named) {
        return named;
    }
    let offset = zone.to_offset(jiff::Timestamp::now()).seconds();
    TimeZone::offset(offset).unwrap_or(TimeZone::UTC)
}
