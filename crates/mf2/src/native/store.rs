//! The app-wide store (plans/19-native-and-terminal.md §5; A4's design): one
//! corpus's catalogs for the whole process, the language every thread
//! formats in, a thread's own language for a scope, and the settings.
//!
//! * **The catalogs** are a [`Catalogs`] in a `OnceLock`, loaded once and
//!   kept: each `Catalog` lives as long as the process, so a simple
//!   message's text is a `&'static str` borrowed from the executable.
//! * **The app-wide language** is one atomic: an index into the corpus's
//!   locales and where it came from. `set_locale` stores it, and every
//!   thread's next format reads it.
//! * **A thread's language** is one thread-local cell, set by `with_locale`
//!   and restored by a guard, also when its body unwinds.
//! * **The settings** (bidi, time zone) sit behind an `RwLock` with a
//!   generation counter; each thread keeps a copy and takes the lock only
//!   when the generation has moved (about 30 ns a format saved against
//!   reading the lock, A4).

use alloc::borrow::{Cow, ToOwned};
use alloc::string::String;
use core::cell::{Cell, RefCell};
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::path::Path;
use std::sync::{OnceLock, PoisonError, RwLock};

use mf2_catalog::{Catalog, Entry, MsgId};
use mf2_runtime::{BidiStrategy, FormatContext, Formatter, Sink, TimeZone};

use super::{Catalogs, Error, LocaleSource};
use crate::{Corpus, Message};

/// What the text forms need of a description, and only that (A4): a
/// `&dyn` of it lists these two, so each form is compiled once whatever
/// the description's type, and none of them links the parts path.
pub(crate) trait TextOf {
    /// The message's id: what the borrowed fast path looks up.
    fn msg_id(&self) -> MsgId;
    /// Formats it into `out`, errors discarded.
    fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink);
}

// ------------------------------------------------------------------ the store

/// The catalogs of the one corpus the process formats with.
static STORE: OnceLock<Catalogs> = OnceLock::new();

/// No language: the app-wide one before `install`, or a thread that follows
/// the app-wide one.
const NONE: usize = usize::MAX;

/// The app-wide language: an index into the corpus's locales, shifted left
/// by two, with where it came from in the low bits ([`pack`]); [`NONE`]
/// before `install`. One atomic, so the two are always read together.
static ACTIVE: AtomicUsize = AtomicUsize::new(NONE);

std::thread_local! {
    /// This thread's language (`with_locale`), or [`NONE`].
    static OVERRIDE: Cell<usize> = const { Cell::new(NONE) };
}

fn pack(index: usize, source: LocaleSource) -> usize {
    let code = match source {
        LocaleSource::Explicit => 0,
        LocaleSource::System => 1,
        LocaleSource::Source => 2,
    };
    (index << 2) | code
}

fn unpack(active: usize) -> (usize, LocaleSource) {
    let source = match active & 3 {
        0 => LocaleSource::Explicit,
        1 => LocaleSource::System,
        _ => LocaleSource::Source,
    };
    (active >> 2, source)
}

/// The store's catalogs of `corpus`, loaded by `load` if the store is empty.
/// Two threads may load at once; the first to finish is kept, and the
/// other's catalogs are dropped.
fn store_of(
    corpus: &'static Corpus,
    load: impl FnOnce() -> Result<Catalogs, Error>,
) -> Result<&'static Catalogs, Error> {
    let store = if let Some(store) = STORE.get() {
        store
    } else {
        let catalogs = load()?;
        STORE.get_or_init(move || catalogs)
    };
    if core::ptr::eq(store.corpus(), corpus) {
        Ok(store)
    } else {
        Err(Error::AnotherCorpus)
    }
}

/// Installs `corpus`'s embedded catalogs (`mf2_build::Emit::Native`) as the
/// process's, and makes the language that best serves the system's
/// preferred languages the app-wide language, else its source language
/// ([`locale_source`] says which). Call it once, at start-up; a second call
/// with the same corpus does nothing.
///
/// # Panics
///
/// If a catalog does not load, which would mean a corrupt executable or
/// catalogs from two builds, which the build cannot produce: the message
/// names the catalog. Also if another corpus is installed already: the store
/// holds one message set per process (format another with [`Catalogs`]).
pub fn install(corpus: &'static Corpus) {
    match store_of(corpus, || Catalogs::embedded(corpus)) {
        Ok(store) => choose(store),
        Err(error) => refused("install()", &error),
    }
}

/// Installs the catalog files `corpus`'s build wrote
/// (`mf2_build::Emit::NativeFiles`, or `Emit::Native`) from `directory` as
/// the process's, and chooses the app-wide language as [`install`] does.
/// A partial set is accepted: only the source locale's file is required,
/// and a language whose file is absent is not offered. Each file must hash
/// to the name it is read under (a catalog from another build is
/// [`Error::ContentMismatch`]). With the store loaded already, the
/// directory is not read again.
pub fn install_from_directory(
    corpus: &'static Corpus,
    directory: impl AsRef<Path>,
) -> Result<(), Error> {
    let store = store_of(corpus, || Catalogs::from_directory(corpus, directory))?;
    choose(store);
    Ok(())
}

/// Chooses the app-wide language, once: a language set explicitly, or
/// chosen by an earlier `install`, stays.
fn choose(store: &Catalogs) {
    if ACTIVE.load(Ordering::Acquire) != NONE {
        return;
    }
    let (index, source) = store.system_choice();
    let _ = ACTIVE.compare_exchange(
        NONE,
        pack(index, source),
        Ordering::AcqRel,
        Ordering::Acquire,
    );
}

/// Makes the language that best serves `locale` the app-wide language: the
/// corpus's closest by CLDR's language-matching data, if close enough
/// (`fr_CA.UTF-8` finds `fr`, `zh-Hant-TW` finds `zh-TW`, `sr-Latn` finds
/// `sr`; plans/19-native-and-terminal.md §9). Every thread's next format
/// uses it, but a thread inside [`with_locale`]. A locale nothing serves is
/// an error and changes nothing, so a mistyped `--lang` is reported, not
/// ignored.
///
/// # Panics
///
/// Before [`install`]: `mf2: no catalogs are installed: call install() at
/// start-up`.
pub fn set_locale(locale: &str) -> Result<(), Error> {
    let (Some(store), false) = (STORE.get(), ACTIVE.load(Ordering::Acquire) == NONE) else {
        not_installed()
    };
    let index = store
        .index(locale)
        .ok_or_else(|| Error::UnknownLocale(locale.to_owned()))?;
    ACTIVE.store(pack(index, LocaleSource::Explicit), Ordering::Release);
    Ok(())
}

/// The language this thread formats in: its own ([`with_locale`]), else the
/// app-wide one.
///
/// # Panics
///
/// Before [`install`], outside `with_locale`.
#[must_use]
pub fn locale() -> &'static str {
    match current() {
        Some((store, index)) => store.tag(index),
        None => not_installed(),
    }
}

/// Where the app-wide language came from: the system's preferences, the
/// source language (none of the system's matched), or [`set_locale`] — to
/// tell a user that their system language is not supported, say.
///
/// # Panics
///
/// Before [`install`].
#[must_use]
pub fn locale_source() -> LocaleSource {
    match ACTIVE.load(Ordering::Acquire) {
        NONE => not_installed(),
        active => unpack(active).1,
    }
}

/// Runs `body` with this thread formatting in the language that best serves
/// `locale` (as [`set_locale`] chooses it), and returns what it returns;
/// the thread's language before is restored
/// when `body` returns or unwinds. Other threads are not affected, so tests
/// pinned to different languages run in parallel.
///
/// It needs no [`install`]: it loads `corpus`'s embedded catalogs on first
/// use, so that a test or a server can format in a named language without
/// choosing an app-wide one. An unsupported locale is an error, and
/// another corpus than the store's is [`Error::AnotherCorpus`].
pub fn with_locale<R>(
    corpus: &'static Corpus,
    locale: &str,
    body: impl FnOnce() -> R,
) -> Result<R, Error> {
    let store = store_of(corpus, || Catalogs::embedded(corpus))?;
    let index = store
        .index(locale)
        .ok_or_else(|| Error::UnknownLocale(locale.to_owned()))?;
    let _restore = Restore(OVERRIDE.with(|o| o.replace(index)));
    Ok(body())
}

/// Puts the thread's language back, also when the body unwinds.
struct Restore(usize);

impl Drop for Restore {
    fn drop(&mut self) {
        OVERRIDE.with(|o| o.set(self.0));
    }
}

// ------------------------------------------------ for the generated module

/// `corpus`'s catalogs, for the generated module's typed forms: loaded
/// from its embedded catalogs if the store is empty.
fn typed(corpus: &'static Corpus, call: &str) -> &'static Catalogs {
    match store_of(corpus, || Catalogs::embedded(corpus)) {
        Ok(store) => store,
        Err(error) => refused(call, &error),
    }
}

/// The generated `Locale::format`: `message` in `locale`, one of
/// `corpus`'s own (else, its catalog not shipped, the closest there is,
/// else the source). It needs no [`install`], and chooses no app-wide
/// language.
pub(crate) fn format_in(corpus: &'static Corpus, locale: &str, message: &impl Message) -> String {
    let store = typed(corpus, "Locale::format");
    store.format_at(store.typed_index(locale), message)
}

/// The generated `with_locale(Locale, body)`: [`with_locale`] with a
/// language of the corpus's own, which cannot be refused — so it returns
/// what `body` returns, and panics only as [`install`] does.
pub(crate) fn with_locale_in<R>(
    corpus: &'static Corpus,
    locale: &str,
    body: impl FnOnce() -> R,
) -> R {
    let store = typed(corpus, "with_locale");
    let _restore = Restore(OVERRIDE.with(|o| o.replace(store.typed_index(locale))));
    body()
}

/// The language this thread formats in, as [`locale`] reads it, or `None`
/// where `locale` would panic: what the generated `current_locale()` reads
/// beside a Leptos mode, where the lookup never panics.
#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
pub(crate) fn try_locale() -> Option<&'static str> {
    current().map(|(store, index)| store.tag(index))
}

/// The catalogs and the language this thread formats with now: its own,
/// else the app-wide one; `None` when neither is set.
#[inline]
fn current() -> Option<(&'static Catalogs, usize)> {
    let store = STORE.get()?;
    let own = OVERRIDE.with(Cell::get);
    let index = if own == NONE {
        match ACTIVE.load(Ordering::Relaxed) {
            NONE => return None,
            active => unpack(active).0,
        }
    } else {
        own
    };
    Some((store, index))
}

/// A native-only build formatting with nothing installed is a programming
/// error, and says so (plans/18-phase-10-work-order.md, "Decided without
/// asking").
#[cold]
#[inline(never)]
#[allow(
    clippy::panic,
    reason = "native only: the ambient forms of a build whose only mode is `native` panic before install() (plans/19 §5)"
)]
pub(crate) fn not_installed() -> ! {
    panic!("mf2: no catalogs are installed: call install() at start-up")
}

#[cold]
#[inline(never)]
#[allow(
    clippy::panic,
    reason = "native only: install() returns nothing, and a catalog that does not load means a corrupt executable (plans/19 §5)"
)]
fn refused(call: &str, error: &Error) -> ! {
    panic!("mf2: {call}: {error}")
}

// --------------------------------------------------------------- the settings

#[derive(Clone, Copy)]
struct Settings {
    bidi: BidiStrategy,
    /// `None`: the system's, read when it is first needed.
    time_zone: Option<TimeZone>,
}

static SETTINGS: RwLock<Settings> = RwLock::new(Settings {
    bidi: BidiStrategy::None,
    time_zone: None,
});
/// Bumped, under the write lock, by every change of the settings.
static GENERATION: AtomicU64 = AtomicU64::new(1);

std::thread_local! {
    /// This thread's copy of the settings, the time zone resolved, and the
    /// generation it was taken at.
    static LOCAL: Cell<(u64, BidiStrategy, TimeZone)> =
        const { Cell::new((0, BidiStrategy::None, TimeZone::UTC)) };
}

fn update(change: impl FnOnce(&mut Settings)) {
    let mut settings = SETTINGS.write().unwrap_or_else(PoisonError::into_inner);
    change(&mut settings);
    GENERATION.fetch_add(1, Ordering::Release);
}

/// The settings a format on this thread uses: its copy, taken again when
/// the generation has moved.
#[inline]
fn settings() -> (BidiStrategy, TimeZone) {
    let generation = GENERATION.load(Ordering::Acquire);
    LOCAL.with(|local| {
        let (taken, bidi, zone) = local.get();
        if taken == generation {
            (bidi, zone)
        } else {
            refresh(local, generation)
        }
    })
}

#[cold]
#[inline(never)]
fn refresh(
    local: &Cell<(u64, BidiStrategy, TimeZone)>,
    generation: u64,
) -> (BidiStrategy, TimeZone) {
    let settings = *SETTINGS.read().unwrap_or_else(PoisonError::into_inner);
    let zone = settings.time_zone.unwrap_or_else(super::zone::system);
    local.set((generation, settings.bidi, zone));
    (settings.bidi, zone)
}

/// Sets the bidi strategy of every thread's next format.
/// [`BidiStrategy::None`] until set: terminals and logs show the isolation
/// controls of [`BidiStrategy::Default`] (U+2066–U+2069) as stray
/// characters more often than they apply them. `to_plain_string()` is never
/// isolated.
pub fn set_bidi(bidi: BidiStrategy) {
    update(|s| s.bidi = bidi);
}

/// The bidi strategy of this thread's next format.
#[must_use]
pub fn bidi() -> BidiStrategy {
    settings().0
}

/// Sets the default time zone of every thread's next format.
pub fn set_time_zone(zone: TimeZone) {
    update(|s| s.time_zone = Some(zone));
}

/// The default time zone of this thread's next format: the system's until
/// set — by its IANA name; without one, a zone that follows the system's
/// daylight-saving rules ([`TimeZone::rules`]); else UTC.
#[must_use]
pub fn time_zone() -> TimeZone {
    settings().1
}

// ------------------------------------------------------------------ the text

/// The text of `m` through the store — steps 2 and 3 of the one lookup
/// (plans/19 §5): this thread's language, else the app-wide one; `None`
/// when neither is set. A simple message is borrowed from the catalog; any
/// other is formatted into a reused buffer and copied out once. `plain`:
/// never isolated.
pub(crate) fn text(m: &dyn TextOf, plain: bool) -> Option<Cow<'static, str>> {
    let (store, index) = current()?;
    let catalog: &'static Catalog = store.catalog(index)?;
    if let Entry::Simple(r) = catalog.get(m.msg_id())
        && let Some(text) = catalog.text(r)
    {
        return Some(Cow::Borrowed(text));
    }
    let (bidi, zone) = settings();
    let mut cx = FormatContext::new(store.context().host);
    cx.bidi = if plain { BidiStrategy::None } else { bidi };
    cx.time_zone = zone;
    let f = Formatter::new(catalog, store.corpus().registry(), &cx);
    Some(Cow::Owned(with_scratch(|buf| {
        m.write_text(&f, buf);
        String::from(buf.as_str())
    })))
}

/// A formatter over the catalog of this thread's language, else the
/// app-wide one — steps 2 and 3 of the one lookup, as [`text`] — handed to
/// `body` with that catalog, for a sink that borrows its text
/// (`mf2::ratatui`). Never isolated: a terminal places every cell itself.
/// `None` when neither language is set.
#[cfg(feature = "ratatui")]
pub(crate) fn with_formatter<R>(
    body: impl FnOnce(&'static Catalog, &Formatter<'_>) -> R,
) -> Option<R> {
    let (store, index) = current()?;
    let catalog: &'static Catalog = store.catalog(index)?;
    let mut cx = FormatContext::new(store.context().host);
    cx.bidi = BidiStrategy::None;
    cx.time_zone = settings().1;
    let f = Formatter::new(catalog, store.corpus().registry(), &cx);
    Some(body(catalog, &f))
}

std::thread_local! {
    static SCRATCH: RefCell<String> = const { RefCell::new(String::new()) };
}

/// A reused buffer, taken while in use, so that a format nested in another
/// (an argument's `Display` that formats a message) gets its own.
pub(crate) fn with_scratch<R>(body: impl FnOnce(&mut String) -> R) -> R {
    let mut buf = SCRATCH.with(|s| {
        s.try_borrow_mut()
            .map(|mut held| core::mem::take(&mut *held))
            .unwrap_or_default()
    });
    buf.clear();
    let out = body(&mut buf);
    SCRATCH.with(|s| {
        if let Ok(mut slot) = s.try_borrow_mut()
            && slot.capacity() < buf.capacity()
        {
            *slot = buf;
        }
    });
    out
}
