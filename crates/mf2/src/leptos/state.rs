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
/// ([`install`]).
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
        }
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

/// The base direction of `tag`, as the build recorded it.
#[must_use]
pub fn dir_of(tag: &str) -> Option<Dir> {
    locales().iter().find(|(t, _)| *t == tag).map(|(_, d)| *d)
}

/// RFC 4647 lookup of `candidate` among `locales`: the candidate, then the
/// candidate with its last subtag removed, and so on; then any locale whose
/// language subtag matches, so that `fr` finds `fr-CA` when that is all the
/// build has. Case-insensitive; `*` and the empty range match nothing.
///
/// One matcher for both sides: `mf2-axum` negotiates a request with it, and
/// a client-only application its stored choice and `navigator.languages`.
#[must_use]
pub fn lookup_locale(
    candidate: &str,
    locales: &[(&'static str, Dir)],
) -> Option<(&'static str, Dir)> {
    if candidate.is_empty() || candidate == "*" {
        return None;
    }
    let mut range = candidate;
    loop {
        if let Some(found) = locales
            .iter()
            .find(|(tag, _)| tag.eq_ignore_ascii_case(range))
        {
            return Some(*found);
        }
        match range.rfind('-') {
            Some(at) => range = range.get(..at).unwrap_or(""),
            None => break,
        }
    }
    let language = candidate.split('-').next().unwrap_or(candidate);
    locales
        .iter()
        .find(|(tag, _)| {
            tag.split('-')
                .next()
                .is_some_and(|l| l.eq_ignore_ascii_case(language))
        })
        .copied()
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
    #[cfg(any(feature = "ssr", feature = "fn-datetime"))]
    {
        cx.time_zone = crate::leptos::zone::current().unwrap_or(runtime.setup.time_zone);
    }
    #[cfg(not(any(feature = "ssr", feature = "fn-datetime")))]
    {
        cx.time_zone = runtime.setup.time_zone;
    }
    Some(cx)
}

/// The installed registry.
pub(crate) fn registry() -> Option<&'static Registry> {
    RUNTIME.get().map(|r| r.setup.registry)
}
