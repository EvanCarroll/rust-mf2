//! What an application installs once, and the formatter every render builds
//! from it (`plans/04-leptos-integration.md` §5).
//!
//! The generated i18n module holds the three things a formatter needs that
//! are the *application's*, not ours: the closed-world [`Registry`], the
//! [`Host`] its corpus needs, and the `MANIFEST_HASH` every catalog is
//! checked against. [`install`] takes them once, before anything renders.
//!
//! Nothing here is per request. The catalog is ([`crate::catalog`]); this is
//! the process-wide half, so a lookup costs no context walk and no lock.

use core::sync::atomic::{AtomicBool, Ordering};

use mf2_catalog::{Catalog, Dir};
use mf2_runtime::{BidiStrategy, FormatContext, Formatter, Host, Registry, TimeZone};

use std::sync::OnceLock;

/// Where formatted text is going, which is what decides bidi isolation
/// (`plans/04-leptos-integration.md` §9).
///
/// The isolating marks (U+2066–U+2069) belong in text a person reads. In a
/// value a program consumes — `prop:value`, the clipboard, a `String` handed
/// to a server function — they are invisible junk that breaks comparisons.
/// `BidiStrategy` is a [`FormatContext`] field, so this is a choice between
/// two formatters the library already holds, at no per-call-site cost.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextUse {
    /// Text a person reads: a text child, an attribute (`title`,
    /// `aria-label`, `placeholder`, `alt`), a markup part. Isolated.
    Displayed,
    /// Text a program consumes: a DOM property, [`ToString`], `String` and
    /// `Oco` conversions. Not isolated.
    Plain,
}

/// The application's own half of a formatter, installed once
/// ([`install`]).
#[derive(Clone, Copy, Debug)]
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
    /// The default time zone (`plans/03-runtime.md` §6).
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

    /// The same with another default time zone.
    #[must_use]
    pub const fn with_time_zone(mut self, zone: TimeZone) -> Setup {
        self.time_zone = zone;
        self
    }
}

/// The installed state: [`Setup`] plus one [`FormatContext`] per bidi
/// strategy, so that a position picks a formatter rather than building one.
struct Runtime {
    setup: Setup,
    displayed: FormatContext,
    plain: FormatContext,
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
    let mut context = |bidi| {
        let mut cx = FormatContext::new(setup.host);
        cx.bidi = bidi;
        cx.time_zone = setup.time_zone;
        cx
    };
    let runtime = Runtime {
        setup,
        displayed: context(BidiStrategy::Default),
        plain: context(BidiStrategy::None),
    };
    if RUNTIME.set(runtime).is_err() {
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

/// Runs `body` with a formatter over `catalog` for the given position.
///
/// `None` before [`install`] — which is the one state in which nothing can
/// be formatted, and in which every caller renders empty text rather than
/// panicking.
pub(crate) fn with_formatter<R>(
    catalog: &Catalog,
    use_: TextUse,
    body: impl FnOnce(&Formatter<'_>) -> R,
) -> Option<R> {
    let runtime = RUNTIME.get()?;
    let cx = match use_ {
        TextUse::Displayed => &runtime.displayed,
        TextUse::Plain => &runtime.plain,
    };
    Some(body(&Formatter::new(catalog, runtime.setup.registry, cx)))
}
