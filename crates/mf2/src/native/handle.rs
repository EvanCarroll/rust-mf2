//! [`NativeI18n`]: 1.x's app-owned handle, kept beside the 2.0 forms.
//! 2.0's forms are the store
//! ([`install`](super::install), `Display`, `to_string()`) and
//! [`Catalogs`](super::Catalogs), which this is built on.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use std::path::Path;

use mf2_catalog::Dir;
use mf2_runtime::{BidiStrategy, FormatError, Formatter, TimeZone};

use super::{Catalogs, Error, LocaleSource};
use crate::{CatalogFile, Corpus, Message};

/// An app-owned native translation context: 1.x's `mf2_native::NativeI18n`,
/// kept for 1.x applications. A 2.0 application installs its catalogs
/// for the process instead ([`install`](super::install)) and shows a
/// description where text is wanted, or formats with
/// [`Catalogs`](super::Catalogs), explicitly.
///
/// It holds every catalog of one generated [`Corpus`], validated once, and
/// the active locale. Keep it in the CLI's or the TUI's state; `set_locale`
/// changes what the next `format` produces. Nothing is process-global.
pub struct NativeI18n {
    catalogs: Catalogs,
    active: usize,
    source: LocaleSource,
}

/// The active locale, where it came from, the catalogs and the formatting
/// settings. The corpus's embedded bytes are left out.
impl fmt::Debug for NativeI18n {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeI18n")
            .field("locale", &self.locale())
            .field("locale_source", &self.source)
            .field("catalogs", &self.catalogs)
            .finish_non_exhaustive()
    }
}

impl NativeI18n {
    /// Loads the catalogs the build embedded (`mf2_build::Emit::Native`)
    /// and selects the locale that best serves the system's preferred
    /// locales, else its source locale.
    ///
    /// Nothing is copied: each catalog reads the executable's own bytes.
    pub fn embedded(corpus: &'static Corpus) -> Result<Self, Error> {
        Ok(NativeI18n::with(Catalogs::embedded(corpus)?))
    }

    /// Loads the catalog files the build wrote (`mf2_build::Emit::NativeFiles`,
    /// or `Emit::Native`) from `directory`, under the content-hashed names
    /// the corpus records, and selects a locale as [`NativeI18n::embedded`]
    /// does. Every locale's file is required.
    ///
    /// Each file must hash to the name it is read under: a catalog from
    /// another build — even one that changed only a message's text, which
    /// keeps the manifest hash — is [`Error::ContentMismatch`], not the other
    /// build's text.
    pub fn from_directory(
        corpus: &'static Corpus,
        directory: impl AsRef<Path>,
    ) -> Result<Self, Error> {
        Ok(NativeI18n::with(Catalogs::files(
            corpus,
            directory.as_ref(),
            false,
        )?))
    }

    fn with(catalogs: Catalogs) -> NativeI18n {
        let (active, source) = catalogs.system_choice();
        NativeI18n {
            catalogs,
            active,
            source,
        }
    }

    /// The locale the next `format` uses.
    #[must_use]
    pub fn locale(&self) -> &'static str {
        self.catalogs.tag(self.active)
    }

    /// Where the active locale came from — to tell a user that their system
    /// language is not supported, say.
    #[must_use]
    pub const fn locale_source(&self) -> LocaleSource {
        self.source
    }

    /// The active locale's base direction.
    #[must_use]
    pub fn dir(&self) -> Dir {
        self.catalogs.dir(self.active)
    }

    /// The corpus's source locale, the final fallback.
    #[must_use]
    pub const fn source_locale(&self) -> &'static str {
        self.catalogs.corpus().source_locale()
    }

    /// The supported locale tags, in the build's order — for a
    /// `--list-locales`, say.
    pub fn available_locales(&self) -> impl ExactSizeIterator<Item = &'static str> + use<> {
        self.catalogs.corpus().locales().iter().map(|(tag, _)| *tag)
    }

    /// Makes the supported locale that best serves `locale` the active
    /// locale, by CLDR's language-matching data (`fr_CA.UTF-8` finds `fr`,
    /// `zh-Hant-TW` finds `zh-TW`). A locale nothing serves is an error and
    /// leaves the active one as it was, so a mistyped `--lang` is reported,
    /// not ignored.
    pub fn set_locale(&mut self, locale: &str) -> Result<(), Error> {
        self.active = self
            .catalogs
            .index(locale)
            .ok_or_else(|| Error::UnknownLocale(locale.to_owned()))?;
        self.source = LocaleSource::Explicit;
        Ok(())
    }

    /// The bidi strategy. [`BidiStrategy::None`] by default: terminals
    /// and logs show the isolation controls of [`BidiStrategy::Default`]
    /// (U+2066–U+2069) as stray characters more often than they apply them.
    #[must_use]
    pub const fn bidi(&self) -> BidiStrategy {
        self.catalogs.bidi()
    }

    /// Sets the bidi strategy for subsequent formatting.
    pub const fn set_bidi(&mut self, bidi: BidiStrategy) {
        self.catalogs.set_bidi(bidi);
    }

    /// The default time zone of dates and times: the system's (by its IANA
    /// name, else the rules it follows), else UTC — and UTC where no
    /// message can show a date, which does not read the system's.
    #[must_use]
    pub const fn time_zone(&self) -> TimeZone {
        self.catalogs.time_zone()
    }

    /// Sets the default time zone for subsequent formatting.
    pub const fn set_time_zone(&mut self, zone: TimeZone) {
        self.catalogs.set_time_zone(zone);
    }

    /// A formatter over the active catalog, for code that needs more than
    /// text: parts, for instance.
    #[must_use]
    pub fn formatter(&self) -> Option<Formatter<'_>> {
        self.catalogs.formatter_at(self.active)
    }

    /// Formats a message in the active locale, discarding MF2 errors (the
    /// text shows the fallback the specification defines).
    #[must_use]
    pub fn format(&self, message: &impl Message) -> String {
        self.catalogs.format_at(self.active, message)
    }

    /// Formats a message in the active locale, with its MF2 errors.
    #[must_use]
    pub fn format_with_errors(&self, message: &impl Message) -> (String, Vec<FormatError>) {
        self.catalogs.format_at_with_errors(self.active, message)
    }

    /// The content-hashed file name of `locale`'s catalog, for copying the
    /// catalogs into an application data directory.
    #[must_use]
    pub fn catalog_file_name(&self, locale: &str) -> Option<&'static str> {
        let tag = self.catalogs.tag(self.catalogs.index(locale)?);
        self.catalogs
            .corpus()
            .catalogs()
            .iter()
            .find(|f| f.tag() == tag)
            .map(CatalogFile::file_name)
    }
}
