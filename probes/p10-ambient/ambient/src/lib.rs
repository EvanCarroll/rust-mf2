//! A4 probe: the native ambient store (`plans/18-phase-10-work-order.md` A4;
//! master plan D17) — variant (b), the design, and variant (c), the same
//! reading the settings lock on every format.
//!
//! * **One store**, in a `OnceLock`: the corpus and its catalogs, all
//!   `&'static` — the embedded bytes are read in place and the `Catalog`
//!   values are leaked once at `install`, so a simple message's text is a
//!   `&'static str` borrowed from the executable.
//! * **The active locale**: one `AtomicUsize`. A thread's override is one
//!   thread-local `Cell`, set by [`with_locale`] and restored by a guard.
//! * **The settings** (bidi, time zone, theme): behind an `RwLock`, with a
//!   generation counter. Each thread keeps a copy and takes the lock only
//!   when the generation moved — variant (b). Variant (c) takes it on every
//!   format (`LOCK = true`).
//!
//! The functions are generic over `const LOCK: bool` so that one binary
//! runs both variants side by side.
#![forbid(unsafe_code)]

mod error;
mod line;

pub use error::Error;
pub use line::{Method, Theme, line};

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{OnceLock, PoisonError, RwLock};

use mf2::{
    BidiStrategy, Catalog, Corpus, Entry, FormatContext, Formatter, Message, MsgId, NoErrors,
    TimeZone,
};

/// A call-site description the ambient forms format. `simple_id` is the fast
/// path's key; in 2.0 the types live in `mf2` and carry it themselves.
pub trait Msg: Message {
    /// The id, when the description has no arguments.
    fn simple_id(&self) -> Option<MsgId>;
}

impl Msg for mf2::Tr {
    #[inline]
    fn simple_id(&self) -> Option<MsgId> {
        Some(self.id())
    }
}

impl Msg for mf2::TrArgs {
    #[inline]
    fn simple_id(&self) -> Option<MsgId> {
        None
    }
}

impl Msg for mf2::TrRich {
    #[inline]
    fn simple_id(&self) -> Option<MsgId> {
        None
    }
}

// ---------------------------------------------------------------- the store

struct Store {
    corpus: &'static Corpus,
    /// One per locale, in `corpus.locales()` order; leaked once.
    catalogs: &'static [Catalog],
    /// Per catalog, its string pool validated once as `&'static str`
    /// ([`Method::RangePool`]); `None` if a string in it is not UTF-8.
    /// Built by the first Ratatui conversion, so a CLI never links it.
    pools: OnceLock<Box<[Option<&'static str>]>>,
}

impl Store {
    #[inline]
    fn catalog(&self, index: usize) -> (&'static Catalog, usize) {
        let i = if index < self.catalogs.len() { index } else { 0 };
        (self.catalogs.get(i).unwrap_or_else(|| not_installed()), i)
    }

    fn pool(&self, index: usize) -> Option<&'static str> {
        self.pools
            .get_or_init(|| self.catalogs.iter().map(pool).collect())
            .get(index)
            .copied()
            .flatten()
    }
}

static STORE: OnceLock<Store> = OnceLock::new();

/// No override: the thread follows [`ACTIVE`].
const NONE: usize = usize::MAX;
/// The app-wide active locale, an index into the corpus's locales.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

std::thread_local! {
    /// This thread's override ([`with_locale`]), or [`NONE`].
    static OVERRIDE: Cell<usize> = const { Cell::new(NONE) };
}

/// Installs `corpus`'s embedded catalogs and selects the system's language,
/// as `NativeI18n::embedded` does. Idempotent for the same corpus; another
/// corpus is an error.
pub fn install(corpus: &'static Corpus) -> Result<(), Error> {
    let same = |store: &Store| {
        if std::ptr::eq(store.corpus, corpus) {
            Ok(())
        } else {
            Err(Error::AnotherCorpus)
        }
    };
    if let Some(store) = STORE.get() {
        return same(store);
    }
    // Two threads racing here both validate; one store wins, and the
    // loser's catalogs are leaked (it happens once, if ever).
    let catalogs = load(corpus)?;
    same(STORE.get_or_init(|| {
        let system = sys_locale::get_locales().find_map(|tag| index_of(corpus, &tag));
        let source = index_of(corpus, corpus.source_locale()).unwrap_or(0);
        ACTIVE.store(system.unwrap_or(source), Ordering::Relaxed);
        update(|s| s.time_zone = system_time_zone());
        Store {
            corpus,
            catalogs,
            pools: OnceLock::new(),
        }
    }))
}

/// Every embedded catalog of `corpus`, validated, leaked once.
fn load(corpus: &'static Corpus) -> Result<&'static [Catalog], Error> {
    let mut catalogs = Vec::with_capacity(corpus.locales().len());
    for &(tag, _) in corpus.locales() {
        let bytes = corpus
            .catalogs()
            .iter()
            .find(|f| f.tag() == tag)
            .and_then(mf2::CatalogFile::bytes)
            .ok_or_else(|| Error::NotEmbedded(tag.to_owned()))?;
        let catalog =
            Catalog::from_static(bytes, corpus.manifest_hash()).map_err(|source| {
                Error::Catalog {
                    locale: tag.to_owned(),
                    source,
                }
            })?;
        catalogs.push(catalog);
    }
    Ok(Box::leak(catalogs.into_boxed_slice()))
}

/// A catalog's string pool as one `&'static str`, validated once.
fn pool(catalog: &'static Catalog) -> Option<&'static str> {
    let (_, off, len) = catalog
        .sections()
        .find(|&(kind, _, _)| kind == mf2_catalog::format::section::STRINGS)?;
    let start = usize::try_from(off).ok()?;
    let end = start.checked_add(usize::try_from(len).ok()?)?;
    std::str::from_utf8(catalog.as_bytes().get(start..end)?).ok()
}

#[inline]
fn store() -> &'static Store {
    match STORE.get() {
        Some(store) => store,
        None => not_installed(),
    }
}

/// A native-only build formatting with nothing installed is a programming
/// error, named (plans/18, "Decided without asking").
#[cold]
#[inline(never)]
fn not_installed() -> ! {
    panic!("mf2: no catalogs are installed: call install() at start-up")
}

/// The locale index this thread formats in now.
#[inline]
fn active() -> usize {
    let o = OVERRIDE.with(Cell::get);
    if o == NONE {
        ACTIVE.load(Ordering::Relaxed)
    } else {
        o
    }
}

/// The supported locale `tag` names: exact (ignoring case, `_` as `-`),
/// then with subtags dropped from the end. (The real matcher is C3's.)
fn index_of(corpus: &Corpus, tag: &str) -> Option<usize> {
    let bare = tag.split(['.', '@']).next()?.replace('_', "-");
    let mut range = bare.as_str();
    loop {
        if let Some(i) = corpus
            .locales()
            .iter()
            .position(|(t, _)| t.eq_ignore_ascii_case(range))
        {
            return Some(i);
        }
        range = range.get(..range.rfind('-')?)?;
    }
}

/// Makes `tag` the app-wide locale: every thread's next format uses it.
pub fn set_locale(tag: &str) -> Result<(), Error> {
    let i = index_of(store().corpus, tag).ok_or_else(|| Error::UnknownLocale(tag.to_owned()))?;
    ACTIVE.store(i, Ordering::Relaxed);
    Ok(())
}

/// The locale this thread formats in.
#[must_use]
pub fn locale() -> &'static str {
    let s = store();
    s.corpus
        .locales()
        .get(active())
        .map_or(s.corpus.source_locale(), |(t, _)| t)
}

/// Runs `body` with this thread pinned to `tag`; the previous state is
/// restored when it returns or unwinds.
pub fn with_locale<R>(tag: &str, body: impl FnOnce() -> R) -> Result<R, Error> {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERRIDE.with(|o| o.set(self.0));
        }
    }
    let i = index_of(store().corpus, tag).ok_or_else(|| Error::UnknownLocale(tag.to_owned()))?;
    let _restore = Restore(OVERRIDE.with(|o| o.replace(i)));
    Ok(body())
}

// ------------------------------------------------------------- the settings

#[derive(Clone, Copy)]
struct Settings {
    bidi: BidiStrategy,
    time_zone: TimeZone,
    /// `None`: the default theme.
    theme: Option<&'static Theme>,
}

impl Settings {
    const INITIAL: Settings = Settings {
        bidi: BidiStrategy::None,
        time_zone: TimeZone::UTC,
        theme: None,
    };
}

static SETTINGS: RwLock<Settings> = RwLock::new(Settings::INITIAL);
/// Bumped, under the write lock, by every settings change.
static GENERATION: AtomicU64 = AtomicU64::new(1);

std::thread_local! {
    /// This thread's copy of the settings, and the generation it was taken at.
    static LOCAL: Cell<(u64, Settings)> = const { Cell::new((0, Settings::INITIAL)) };
}

fn update(change: impl FnOnce(&mut Settings)) {
    let mut s = SETTINGS.write().unwrap_or_else(PoisonError::into_inner);
    change(&mut s);
    GENERATION.fetch_add(1, Ordering::Release);
}

/// (b): the thread's copy, refreshed when the generation moved.
#[inline]
fn settings_cached() -> Settings {
    let generation = GENERATION.load(Ordering::Acquire);
    LOCAL.with(|local| {
        let (g, s) = local.get();
        if g == generation {
            s
        } else {
            refresh(local, generation)
        }
    })
}

#[cold]
#[inline(never)]
fn refresh(local: &Cell<(u64, Settings)>, generation: u64) -> Settings {
    let s = settings_locked();
    local.set((generation, s));
    s
}

/// (c): the lock, every time.
#[inline]
fn settings_locked() -> Settings {
    *SETTINGS.read().unwrap_or_else(PoisonError::into_inner)
}

#[inline]
fn settings<const LOCK: bool>() -> Settings {
    if LOCK {
        settings_locked()
    } else {
        settings_cached()
    }
}

/// Sets the bidi strategy for every thread's next format.
pub fn set_bidi(bidi: BidiStrategy) {
    update(|s| s.bidi = bidi);
}

/// Sets the default time zone for every thread's next format.
pub fn set_time_zone(zone: TimeZone) {
    update(|s| s.time_zone = zone);
}

/// Sets the app-wide theme (leaked: a theme is set once or rarely).
pub fn set_theme(theme: Theme) {
    let theme: &'static Theme = Box::leak(Box::new(theme));
    update(|s| s.theme = Some(theme));
}

/// The bidi strategy the next format on this thread uses.
#[must_use]
pub fn bidi() -> BidiStrategy {
    settings_cached().bidi
}

/// R2's one-time cost, again: every catalog's pool validated, uncached.
/// Returns the bytes validated.
#[doc(hidden)]
#[must_use]
pub fn pools_cost() -> usize {
    store()
        .catalogs
        .iter()
        .map(|c| pool(c).map_or(0, str::len))
        .sum()
}

/// The settings read alone, as a format does it — (b)'s copy or (c)'s lock
/// — plus the locale lookup: what the store adds to every format.
#[doc(hidden)]
#[inline(never)]
pub fn lookup_cost<const LOCK: bool>() -> (usize, BidiStrategy) {
    let s = store();
    let (catalog, _) = s.catalog(active());
    (catalog.as_bytes().len(), settings::<LOCK>().bidi)
}

/// The time zone the next format on this thread uses.
#[must_use]
pub fn time_zone() -> TimeZone {
    settings_cached().time_zone
}

fn context(s: &Settings) -> FormatContext {
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.bidi = s.bidi;
    cx.time_zone = s.time_zone;
    cx
}

/// The system's time zone, as 1.x's `mf2-native` reads it.
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

// ------------------------------------------------------------- formatting

/// Runs `body` with a formatter over the catalog this thread formats with
/// now: the one ambient lookup (the native half of D17's order).
#[inline]
pub fn with_formatter<const LOCK: bool, R>(body: impl FnOnce(&Formatter<'_>) -> R) -> R {
    let s = store();
    let (catalog, _) = s.catalog(active());
    let cx = context(&settings::<LOCK>());
    body(&Formatter::new(catalog, s.corpus.registry(), &cx))
}

/// A simple message's text in the current locale, borrowed from the
/// executable: no copy, no allocation.
#[inline]
#[must_use]
pub fn simple_static(id: MsgId) -> Option<&'static str> {
    let (catalog, _) = store().catalog(active());
    simple_in(catalog, id)
}

#[inline]
fn simple_in(catalog: &'static Catalog, id: MsgId) -> Option<&'static str> {
    match catalog.get(id) {
        Entry::Simple(r) => catalog.text(r),
        _ => None,
    }
}

/// The text, borrowed when the message is simple, else formatted.
///
/// Generic only at the surface: the work is one non-generic function per
/// form, whatever the description type, so each form is compiled once.
#[inline]
pub fn to_cow<const LOCK: bool>(m: &impl Msg) -> Cow<'static, str> {
    cow_dyn::<LOCK>(m)
}

/// What the text forms need of a description, and only that: a `&dyn` of
/// it keeps no other method alive (a vtable links every method it lists;
/// through `&dyn Msg`, the parts path came into a CLI that never uses it).
trait TextOf {
    fn simple(&self) -> Option<MsgId>;
    fn write_to(&self, f: &Formatter<'_>, out: &mut dyn mf2::Sink);
}

impl<M: Msg> TextOf for M {
    #[inline]
    fn simple(&self) -> Option<MsgId> {
        self.simple_id()
    }

    #[inline]
    fn write_to(&self, f: &Formatter<'_>, out: &mut dyn mf2::Sink) {
        self.write(f, out, &mut NoErrors);
    }
}

/// What the Ratatui form needs of a description.
pub(crate) trait PartsOf {
    fn parts_to(&self, f: &Formatter<'_>, out: &mut dyn mf2::PartSink);
}

impl<M: Msg> PartsOf for M {
    #[inline]
    fn parts_to(&self, f: &Formatter<'_>, out: &mut dyn mf2::PartSink) {
        self.parts(f, out, &mut NoErrors);
    }
}

fn cow_dyn<const LOCK: bool>(m: &dyn TextOf) -> Cow<'static, str> {
    let s = store();
    let (catalog, _) = s.catalog(active());
    if let Some(text) = m.simple().and_then(|id| simple_in(catalog, id)) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(format_owned(s, catalog, &settings::<LOCK>(), m))
}

/// The text, owned (one exact allocation).
#[inline]
pub fn to_string<const LOCK: bool>(m: &impl Msg) -> String {
    string_dyn::<LOCK>(m)
}

fn string_dyn<const LOCK: bool>(m: &dyn TextOf) -> String {
    let s = store();
    let (catalog, _) = s.catalog(active());
    if let Some(text) = m.simple().and_then(|id| simple_in(catalog, id)) {
        return String::from(text);
    }
    format_owned(s, catalog, &settings::<LOCK>(), m)
}

fn format_owned(s: &Store, catalog: &Catalog, set: &Settings, m: &dyn TextOf) -> String {
    let cx = context(set);
    let f = Formatter::new(catalog, s.corpus.registry(), &cx);
    with_scratch(|buf| {
        m.write_to(&f, buf);
        String::from(buf.as_str())
    })
}

std::thread_local! {
    static SCRATCH: RefCell<String> = const { RefCell::new(String::new()) };
}

/// A reused buffer, taken while in use so that a nested format gets its own
/// (as `leptos-mf2`'s `with_scratch`).
fn with_scratch<R>(body: impl FnOnce(&mut String) -> R) -> R {
    let mut buf = SCRATCH.with(|s| {
        s.try_borrow_mut()
            .map(|mut held| std::mem::take(&mut *held))
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

/// `Display` through the ambient lookup: the text streams into the
/// `fmt::Formatter`, with no `String` in between (unless a width or
/// precision asks for padding).
pub struct Show<'m, M>(pub &'m M);

impl<M: Msg> fmt::Display for Show<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display(self.0, f)
    }
}

/// `Display`, once for every description type.
fn display(m: &dyn TextOf, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let s = store();
    let (catalog, _) = s.catalog(active());
    if let Some(text) = m.simple().and_then(|id| simple_in(catalog, id)) {
        return f.pad(text);
    }
    let cx = context(&settings_cached());
    let fm = Formatter::new(catalog, s.corpus.registry(), &cx);
    if f.width().is_some() || f.precision().is_some() {
        let mut text = String::new();
        m.write_to(&fm, &mut text);
        return f.pad(&text);
    }
    let mut out = FmtSink { f, result: Ok(()) };
    m.write_to(&fm, &mut out);
    out.result
}

struct FmtSink<'a, 'b> {
    f: &'a mut fmt::Formatter<'b>,
    result: fmt::Result,
}

impl mf2::Sink for FmtSink<'_, '_> {
    fn push_str(&mut self, s: &str) {
        if self.result.is_ok() {
            self.result = self.f.write_str(s);
        }
    }
}

/// The store's parts for the Ratatui sink: the catalog this thread formats
/// with now, its pool, and the theme.
#[inline]
fn line_parts<const LOCK: bool>() -> (&'static Store, &'static Catalog, Option<&'static str>, Settings) {
    let s = store();
    let (catalog, i) = s.catalog(active());
    (s, catalog, s.pool(i), settings::<LOCK>())
}
