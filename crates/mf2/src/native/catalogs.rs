//! [`Catalogs`]: a corpus's catalogs, formatted in whichever language the
//! caller names — the explicit form, with no globals, that the app-wide
//! store is built on.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use std::path::Path;

use mf2_catalog::{Catalog, Dir};
use mf2_runtime::{
    BidiStrategy, ErrorSink, FormatContext, FormatError, Formatter, NoErrors, Sink, TimeZone,
};

use super::Error;
use super::locale::{LocaleSource, best};
use crate::{CatalogFile, Corpus, Message};

/// A generated corpus's catalogs, explicitly: `format(locale, &message)`
/// formats a message in the language named, with no app-wide state. For a
/// server answering each request in its reader's language, a tool, a test,
/// or a second message set beside the one [`install`](super::install)ed.
///
/// Its own settings start as the store's do: bidi isolation off, and the
/// system's time zone.
///
/// ```ignore
/// let catalogs = mf2::native::Catalogs::embedded(&my_i18n::CORPUS)?;
/// let text = catalogs.format("fr", &my_i18n::tr!("welcome"));
/// ```
pub struct Catalogs {
    corpus: &'static Corpus,
    /// One per locale, in `corpus.locales()` order; `None` where no file
    /// was shipped ([`Catalogs::from_directory`]).
    by_locale: Vec<Option<Catalog>>,
    /// The locales that have a catalog, in the build's order: what a tag is
    /// matched against.
    available: Vec<(&'static str, Dir)>,
    cx: FormatContext,
}

/// The source locale, the locales it has catalogs for, each catalog (its
/// own `Debug`: locale, messages, bytes) and the settings. The corpus's
/// embedded bytes are left out.
impl fmt::Debug for Catalogs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Catalogs")
            .field("source_locale", &self.corpus.source_locale())
            .field("catalogs", &self.by_locale)
            .field("context", &self.cx)
            .finish_non_exhaustive()
    }
}

/// What a text form needs of a message, and only that: a `&dyn` of it lists
/// `write` alone, so a program that never formats to parts links none of
/// that path (`plans/18-phase-10-work-order.md`, A4).
trait Writes {
    fn write_to(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink);
}

impl<M: Message + ?Sized> Writes for M {
    fn write_to(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        self.write(f, out, errs);
    }
}

impl Catalogs {
    /// Loads the catalogs the build embedded (`mf2_build::Emit::Native`).
    /// Nothing is copied: each catalog reads the executable's own bytes.
    /// Every locale's must be there.
    pub fn embedded(corpus: &'static Corpus) -> Result<Catalogs, Error> {
        Catalogs::load(corpus, |tag, file| {
            let bytes = file
                .bytes()
                .ok_or_else(|| Error::NotEmbedded(tag.to_owned()))?;
            Catalog::from_static(bytes, corpus.manifest_hash())
                .map(Some)
                .map_err(|source| Error::Catalog {
                    locale: tag.to_owned(),
                    source,
                })
        })
    }

    /// Loads the catalog files the build wrote (`mf2_build::Emit::NativeFiles`,
    /// or `Emit::Native`) from `directory`, under the content-hashed names
    /// the corpus records. A partial set is accepted: only the source
    /// locale's file is required, and a locale whose file is absent is not
    /// offered.
    ///
    /// Each file must hash to the name it is read under: a catalog from
    /// another build — even one that changed only a message's text, which
    /// keeps the manifest hash — is [`Error::ContentMismatch`], not the other
    /// build's text.
    pub fn from_directory(
        corpus: &'static Corpus,
        directory: impl AsRef<Path>,
    ) -> Result<Catalogs, Error> {
        Catalogs::files(corpus, directory.as_ref(), true)
    }

    /// The files of `directory`: every locale's, or (`partial`) the source
    /// locale's and whichever others are there.
    pub(crate) fn files(
        corpus: &'static Corpus,
        directory: &Path,
        partial: bool,
    ) -> Result<Catalogs, Error> {
        Catalogs::load(corpus, |tag, file| {
            let path = directory.join(file.file_name());
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(source)
                    if partial
                        && source.kind() == std::io::ErrorKind::NotFound
                        && tag != corpus.source_locale() =>
                {
                    return Ok(None);
                }
                Err(source) => return Err(Error::Io { path, source }),
            };
            let actual = mf2_catalog::content_hash(&bytes);
            if name_hash(file.file_name()) != Some(actual.as_str()) {
                return Err(Error::ContentMismatch { path, actual });
            }
            Catalog::new(bytes, corpus.manifest_hash())
                .map(Some)
                .map_err(|source| Error::Catalog {
                    locale: tag.to_owned(),
                    source,
                })
        })
    }

    fn load(
        corpus: &'static Corpus,
        mut read: impl FnMut(&str, &CatalogFile) -> Result<Option<Catalog>, Error>,
    ) -> Result<Catalogs, Error> {
        let mut by_locale = Vec::with_capacity(corpus.locales().len());
        let mut available = Vec::with_capacity(corpus.locales().len());
        for &(tag, dir) in corpus.locales() {
            let file = corpus
                .catalogs()
                .iter()
                .find(|f| f.tag() == tag)
                .ok_or_else(|| Error::MissingCatalog(tag.to_owned()))?;
            let catalog = read(tag, file)?;
            if let Some(catalog) = &catalog {
                if catalog.locale() != tag {
                    return Err(Error::LocaleMismatch {
                        expected: tag.to_owned(),
                        actual: catalog.locale().to_owned(),
                    });
                }
                available.push((tag, dir));
            }
            by_locale.push(catalog);
        }
        // The host the generated module named for this corpus: with dates
        // the one that resolves a named zone, else the plain one, so that a
        // corpus no date can reach links no zone database (`plan/01` §4.1).
        let mut cx = FormatContext::new(corpus.host());
        cx.bidi = BidiStrategy::None;
        // Without dates, nothing reads the zone, so nothing looks it up.
        #[cfg(feature = "datetime")]
        {
            cx.time_zone = mf2_host_std::system_time_zone();
        }
        Ok(Catalogs {
            corpus,
            by_locale,
            available,
            cx,
        })
    }

    /// The corpus these catalogs are of.
    #[must_use]
    pub const fn corpus(&self) -> &'static Corpus {
        self.corpus
    }

    /// The locales there is a catalog for, in the build's order.
    pub fn locales(&self) -> impl ExactSizeIterator<Item = &'static str> + '_ {
        self.available.iter().map(|(tag, _)| *tag)
    }

    /// Formats `message` in `locale` — the catalog of the language that
    /// best serves it, by CLDR's language-matching data (`fr_CA.UTF-8`
    /// finds `fr`, `zh-Hant-TW` finds `zh-TW`), else the source locale's —
    /// discarding MF2 errors (the text shows the fallback the specification
    /// defines).
    #[must_use]
    pub fn format(&self, locale: &str, message: &impl Message) -> String {
        self.format_at(self.index_or_source(locale), message)
    }

    /// Formats `message` in `locale`, as [`Catalogs::format`] does, with its
    /// MF2 errors.
    #[must_use]
    pub fn format_with_errors(
        &self,
        locale: &str,
        message: &impl Message,
    ) -> (String, Vec<FormatError>) {
        self.format_at_with_errors(self.index_or_source(locale), message)
    }

    /// A formatter over `locale`'s catalog (else the source locale's), for
    /// code that needs more than text: parts, for instance.
    #[must_use]
    pub fn formatter(&self, locale: &str) -> Option<Formatter<'_>> {
        self.formatter_at(self.index_or_source(locale))
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

    /// The default time zone of dates and times: the system's (by its IANA
    /// name, else the rules it follows), else UTC.
    #[must_use]
    pub const fn time_zone(&self) -> TimeZone {
        self.cx.time_zone
    }

    /// Sets the default time zone for subsequent formatting.
    pub const fn set_time_zone(&mut self, zone: TimeZone) {
        self.cx.time_zone = zone;
    }

    // ------------------------------------------------ for the store and 1.x

    /// The index, among the corpus's locales, of the locale that best
    /// serves `locale` among those there is a catalog for.
    pub(crate) fn index(&self, locale: &str) -> Option<usize> {
        let tag = best(self.corpus.language_matching(), [locale], &self.available)?;
        self.position(tag)
    }

    fn index_or_source(&self, locale: &str) -> usize {
        self.index(locale).unwrap_or_else(|| self.source())
    }

    /// The index of `tag`, one of the corpus's own, when its catalog is
    /// there — no matching — else as [`Catalogs::format`] chooses: the
    /// locale that best serves it, else the source.
    pub(crate) fn typed_index(&self, tag: &str) -> usize {
        match self.position(tag) {
            Some(index) if self.catalog(index).is_some() => index,
            _ => self.index_or_source(tag),
        }
    }

    /// The index of the source locale.
    pub(crate) fn source(&self) -> usize {
        self.position(self.corpus.source_locale()).unwrap_or(0)
    }

    fn position(&self, tag: &str) -> Option<usize> {
        self.corpus.locales().iter().position(|(t, _)| *t == tag)
    }

    /// The locale the system's list of preferred languages is best served
    /// in, among those there is a catalog for (the one matcher, the list's
    /// later entries demoted), and whether it came from the system or is
    /// the source locale.
    pub(crate) fn system_choice(&self) -> (usize, LocaleSource) {
        let system = sys_locale::get_locales().collect::<Vec<_>>();
        let matching = self.corpus.language_matching();
        match best(matching, system.iter().map(String::as_str), &self.available)
            .and_then(|tag| self.position(tag))
        {
            Some(i) => (i, LocaleSource::System),
            None => (self.source(), LocaleSource::Source),
        }
    }

    /// The tag of the locale at `index`.
    pub(crate) fn tag(&self, index: usize) -> &'static str {
        self.corpus
            .locales()
            .get(index)
            .map_or(self.corpus.source_locale(), |(tag, _)| tag)
    }

    /// The base direction of the locale at `index`.
    pub(crate) fn dir(&self, index: usize) -> Dir {
        self.corpus
            .locales()
            .get(index)
            .map_or(Dir::Ltr, |(_, dir)| *dir)
    }

    /// The catalog of the locale at `index`.
    pub(crate) fn catalog(&self, index: usize) -> Option<&Catalog> {
        self.by_locale.get(index)?.as_ref()
    }

    pub(crate) fn context(&self) -> &FormatContext {
        &self.cx
    }

    pub(crate) fn formatter_at(&self, index: usize) -> Option<Formatter<'_>> {
        Some(Formatter::new(
            self.catalog(index)?,
            self.corpus.registry(),
            &self.cx,
        ))
    }

    pub(crate) fn format_at(&self, index: usize, message: &impl Message) -> String {
        self.text_at(index, message)
    }

    pub(crate) fn format_at_with_errors(
        &self,
        index: usize,
        message: &impl Message,
    ) -> (String, Vec<FormatError>) {
        self.text_with_errors_at(index, message)
    }

    /// The one text path of the explicit forms, whatever the message's type:
    /// a call site passes a pointer and a vtable, and this is compiled once.
    fn text_at(&self, index: usize, message: &dyn Writes) -> String {
        let mut out = String::new();
        if let Some(f) = self.formatter_at(index) {
            message.write_to(&f, &mut out, &mut NoErrors);
        }
        out
    }

    fn text_with_errors_at(
        &self,
        index: usize,
        message: &dyn Writes,
    ) -> (String, Vec<FormatError>) {
        let mut out = String::new();
        let mut errors = Vec::new();
        if let Some(f) = self.formatter_at(index) {
            message.write_to(&f, &mut out, &mut errors);
        }
        (out, errors)
    }
}

/// The content hash in a catalog file name, `<locale>.<hash>.mf2b`; `None`
/// when the name carries none, which no file then matches.
fn name_hash(file_name: &str) -> Option<&str> {
    let (_, hash) = file_name.strip_suffix(".mf2b")?.rsplit_once('.')?;
    (hash.len() == mf2_catalog::CONTENT_HASH_LEN).then_some(hash)
}
