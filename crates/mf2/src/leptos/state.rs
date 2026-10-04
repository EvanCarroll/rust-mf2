//! What an application installs once, and the formatter every render builds
//! from it.
//!
//! The generated i18n module holds the three things a formatter needs that
//! are the *application's*, not ours: the closed-world [`Registry`], the
//! [`Host`] its corpus needs, and the `MANIFEST_HASH` every catalog is
//! checked against. [`install`] takes them once, before anything renders.
//!
//! Nothing here is per request. The catalog is ([`crate::leptos::catalog`]); this is
//! the process-wide half, so a lookup costs no context walk and no lock.

use core::sync::atomic::{AtomicBool, Ordering};

use mf2_catalog::Dir;
use mf2_runtime::{BidiStrategy, FormatContext, Host, Registry, TimeZone};

use crate::{LanguageMatching, Tr};

use std::sync::OnceLock;

/// Where formatted text is going, which is what decides bidi isolation.
///
/// The isolating marks (U+2066–U+2069) belong in text a person reads. In a
/// value a program consumes — `prop:value`, the clipboard, a `String` handed
/// to a server function — they are invisible junk that breaks comparisons.
/// `BidiStrategy` is a [`FormatContext`] field, so this is a choice between
/// two formatters the library already holds, at no per-call-site cost.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextUse {
    /// Text a person reads: a text child, a markup part, an attribute a
    /// person reads (`title`, `aria-label`, `placeholder`, `alt`; the
    /// attribute's name decides), and a message formatted as a single
    /// string (`to_string()`, `String::from`, `Oco`) — the spec's default.
    /// Isolated.
    Displayed,
    /// Text a program consumes: a DOM property, an attribute a program reads
    /// (`value`, `href`, `download`, `data-*`), `to_plain_string()`. Not
    /// isolated.
    Plain,
}

/// Attributes whose value a program reads — a form submits it, a browser
/// resolves it as a URL or a file name, a selector or a script matches it —
/// so they are plain. `data-*` is matched by prefix in [`attribute_use`].
const PLAIN_ATTRIBUTES: [&str; 16] = [
    "value",
    "href",
    "src",
    "srcset",
    "action",
    "formaction",
    "poster",
    "cite",
    "download",
    "id",
    "name",
    "for",
    "form",
    "list",
    "class",
    "type",
];

/// What the text of the attribute `key` is for, which decides its bidi
/// isolation: plain for the attributes a program
/// reads, isolated for every other — the ones a person reads (`title`,
/// `alt`, `aria-*`, `placeholder`, `label`, `content`, …).
///
/// Every place an attribute's text is made — the server's HTML, a client
/// build or hydration, the registry's rewrite on a switch — asks this one
/// function, so server and client agree. HTML attribute names are ASCII
/// case-insensitive, and so is the match.
pub(crate) fn attribute_use(key: &str) -> TextUse {
    let data = key
        .get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("data-"));
    if data || PLAIN_ATTRIBUTES.iter().any(|a| a.eq_ignore_ascii_case(key)) {
        TextUse::Plain
    } else {
        TextUse::Displayed
    }
}

/// The application's own half of a formatter, installed once
/// ([`install`]): what the translation crate's generated `setup()` returns.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct Setup {
    /// The handlers this corpus uses — the generated `registry()`.
    pub registry: &'static Registry,
    /// The platform services — the generated `host::HOST`.
    pub host: &'static dyn Host,
    /// The generated `MANIFEST_HASH`: every catalog is checked against it
    /// before it is installed (F6).
    pub manifest_hash: u64,
    /// The generated `SOURCE_LOCALE`: what a lookup with no catalog falls
    /// back to.
    pub source_locale: &'static str,
    /// The generated `LOCALES`.
    pub locales: &'static [(&'static str, Dir)],
    /// The time zone a date is shown in when its message names none and
    /// the reader's zone is not known yet.
    pub time_zone: TimeZone,
    /// The build's cut of CLDR's language-matching data
    /// ([`Setup::with_language_matching`]).
    matching: Option<&'static LanguageMatching>,
    /// Each language's name ([`Setup::with_names`]).
    names: Option<fn(usize) -> Option<Tr>>,
}

impl Setup {
    /// The generated module's four items, in UTC.
    #[must_use]
    pub const fn new(
        registry: &'static Registry,
        host: &'static dyn Host,
        manifest_hash: u64,
        source_locale: &'static str,
        locales: &'static [(&'static str, Dir)],
    ) -> Setup {
        Setup {
            registry,
            host,
            manifest_hash,
            source_locale,
            locales,
            time_zone: TimeZone::UTC,
            matching: None,
            names: None,
        }
    }

    /// The same, with the build's cut of CLDR's language-matching data for
    /// its locales, the generated `LANGUAGE_MATCHING`: what a client-only
    /// (`csr`) application's boot matches the reader's languages with, so
    /// that `fr-CA` finds `fr`, `zh-Hant-TW` finds `zh-TW`, and a reader of
    /// Breton is served French, as a server would choose.
    ///
    /// Without it a client matches with no data: a tag finds its own
    /// language's locales (`fr-CA` still finds `fr`), but nothing is filled
    /// in and no other language is accepted. A hydrated page's client never
    /// matches — it takes the server's choice — and needs none; carrying one
    /// there would only add bytes. A server matches with CLDR's whole table
    /// whatever its setup says.
    ///
    /// ```ignore
    /// Setup::new(registry(), &host::HOST, MANIFEST_HASH, SOURCE_LOCALE, LOCALES)
    ///     .with_language_matching(&LANGUAGE_MATCHING)
    /// ```
    #[must_use]
    pub const fn with_language_matching(mut self, matching: &'static LanguageMatching) -> Setup {
        self.matching = Some(matching);
        self
    }

    /// The same with each language's name: `names(index)` is the
    /// `language.<tag>` message of `locales[index]`, if the corpus has one —
    /// what `<LocaleSwitcher>` with no children lists. The generated
    /// `setup()` passes it.
    #[must_use]
    pub const fn with_names(mut self, names: fn(usize) -> Option<Tr>) -> Setup {
        self.names = Some(names);
        self
    }

    /// The same with another default time zone. The reader's zone, once
    /// known, comes first.
    #[must_use]
    pub const fn with_time_zone(mut self, zone: TimeZone) -> Setup {
        self.time_zone = zone;
        self
    }
}

impl core::fmt::Debug for Setup {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Setup")
            .field("manifest_hash", &self.manifest_hash)
            .field("source_locale", &self.source_locale)
            .field("locales", &self.locales)
            .field("time_zone", &self.time_zone)
            .finish_non_exhaustive()
    }
}

/// The installed state.
struct Runtime {
    setup: Setup,
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
/// Whether a second, different [`install`] was refused — reported once by
/// [`installed`] rather than by a panic on a path a call site can reach.
static RE_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Installs the application's registry, host and manifest hash. Call it once,
/// before anything renders: `main` on the server, `hydrate` on the client.
///
/// A second call is ignored — the first wins, and nothing panics.
pub fn install(setup: Setup) {
    if RUNTIME.set(Runtime { setup }).is_err() {
        RE_INSTALLED.store(true, Ordering::Relaxed);
    }
}

/// Whether [`install`] has run.
#[must_use]
pub fn installed() -> bool {
    RUNTIME.get().is_some()
}

/// Whether a second [`install`] was refused: a build error the application
/// can assert on, never a panic in a render.
#[must_use]
pub fn installed_twice() -> bool {
    RE_INSTALLED.load(Ordering::Relaxed)
}

/// The installed [`Setup`], or `None` before [`install`].
#[must_use]
pub fn setup() -> Option<Setup> {
    RUNTIME.get().map(|r| r.setup)
}

/// The manifest hash every catalog is checked against; `0` before
/// [`install`], which no catalog carries.
#[must_use]
pub fn manifest_hash() -> u64 {
    RUNTIME.get().map_or(0, |r| r.setup.manifest_hash)
}

/// The locale a lookup falls back to.
#[must_use]
pub fn source_locale() -> &'static str {
    RUNTIME.get().map_or("", |r| r.setup.source_locale)
}

/// Every locale this corpus was built for, with its base direction.
#[must_use]
pub fn locales() -> &'static [(&'static str, Dir)] {
    RUNTIME.get().map_or(&[], |r| r.setup.locales)
}

/// The name of the language at `index` in [`locales`], if the installed
/// setup has one ([`Setup::with_names`]).
pub(crate) fn locale_name(index: usize) -> Option<Tr> {
    RUNTIME
        .get()
        .and_then(|r| r.setup.names)
        .and_then(|names| names(index))
}

/// The base direction of `tag`, as the build recorded it.
#[must_use]
pub fn dir_of(tag: &str) -> Option<Dir> {
    locales().iter().find(|(t, _)| *t == tag).map(|(_, d)| *d)
}

/// The locale of `locales` that best serves a reader of `candidate`: the
/// one matcher (plans/19-native-and-terminal.md §9), CLDR's language-matching
/// data read as UTS #35 Part 1 states — the tag filled in by likely
/// subtags, a distance per field, a match only below 50. `fr-CA` and
/// `fr_CA.UTF-8` find `fr`, and `fr` finds `fr-CA`; `zh-Hant-TW` finds
/// `zh-TW`; `sr-Latn` finds `sr`; `zh-TW` does not find `zh-CN`, nor
/// `pa-Arab` `pa`. `*`, `C` and the empty range match nothing.
///
/// One matcher for both sides: `mf2::axum` negotiates a request with it, and
/// a client-only application its stored choice and `navigator.languages`.
/// A server matches with CLDR's whole table; a browser's client with the
/// build's cut for its locales, which gives them the same answers, when
/// its [`Setup`] carries one.
#[must_use]
pub fn lookup_locale(
    candidate: &str,
    locales: &[(&'static str, Dir)],
) -> Option<(&'static str, Dir)> {
    best_locale([candidate], locales)
}

/// [`lookup_locale`] over a reader's list, best first: each later entry is
/// demoted, so that a regional variant of the first language beats an
/// exact second one, and none past the tenth can match.
#[doc(hidden)]
#[must_use]
pub fn best_locale<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    locales: &[(&'static str, Dir)],
) -> Option<(&'static str, Dir)> {
    let at = matching().best_match(candidates, locales)?;
    locales.get(at).copied()
}

/// The data the matcher reads: CLDR's whole table where this build carries
/// it (a server, or a native build beside), else the cut the setup
/// installed, else none.
fn matching() -> &'static LanguageMatching {
    #[cfg(feature = "host-std")]
    {
        LanguageMatching::cldr()
    }
    #[cfg(not(feature = "host-std"))]
    {
        RUNTIME
            .get()
            .and_then(|r| r.setup.matching)
            .unwrap_or(&LanguageMatching::EMPTY)
    }
}

/// The formatting context for a position, with the request's bidi override
/// applied if it has one.
///
/// `None` before [`install`] — the one state in which nothing can be
/// formatted, and in which every caller renders empty text rather than
/// panicking. It is built per format rather than cached: a `FormatContext`
/// is a host pointer, a strategy and a time zone, and building one is
/// cheaper than the branch that would choose between three cached ones.
pub(crate) fn context_for(use_: TextUse, bidi: Option<BidiStrategy>) -> Option<FormatContext> {
    let runtime = RUNTIME.get()?;
    let mut cx = FormatContext::new(runtime.setup.host);
    cx.bidi = match use_ {
        // Plain text is never isolated, whatever the request says: the marks
        // are what makes it not plain.
        TextUse::Plain => BidiStrategy::None,
        TextUse::Displayed => bidi.unwrap_or(BidiStrategy::Default),
    };
    // The reader's zone, when this thread knows it (`crate::leptos::zone`).
    #[cfg(any(feature = "ssr", feature = "datetime"))]
    {
        cx.time_zone = crate::leptos::zone::current().unwrap_or(runtime.setup.time_zone);
    }
    #[cfg(not(any(feature = "ssr", feature = "datetime")))]
    {
        cx.time_zone = runtime.setup.time_zone;
    }
    Some(cx)
}

/// The installed registry.
pub(crate) fn registry() -> Option<&'static Registry> {
    RUNTIME.get().map(|r| r.setup.registry)
}
