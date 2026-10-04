//! What the module `mf2-build` generates calls — never written by hand.
//!
//! **The cfg-forwarding macros** (A2). The generated module is compiled in
//! the application's crate, whose own features say nothing about how `mf2`
//! was built. Each macro here is defined twice, under a `cfg` of `mf2`'s and
//! under its negation, so that exactly one definition exists in any build:
//! the tokens it is given pass through, or vanish. The generated text is
//! then the same in every build, and each compile-time choice in it follows
//! `mf2`'s features, whichever crate turned them on:
//!
//! * `__if_host_std!` — not a browser's client: the embedded catalogs;
//! * `__if_native!` — `mf2::native`: `CORPUS`'s typed forms;
//! * `__if_ssr!`, `__if_client!` (`hydrate` or `csr`), `__if_csr!`,
//!   `__if_leptos!` (any of the three) — the Leptos layer;
//! * `__if_mode!` — `native` or a Leptos mode: `install()` and the locale
//!   functions;
//! * `__if_clap!`, `__if_ratatui!` — `Locale`'s value parser, `markup::*`;
//! * `__use_host!` — the host, as one `pub use`, so that the others are
//!   never linked (B1′);
//! * `__date_statics!` — the date handlers of the corpus's ICU4X form
//!   (`mf2_fn_datetime`'s, re-exported at the crate root: its own features
//!   say whether it formats with ICU4X);
//! * `__best_locale!` — `Locale::from_str`: the one matcher, but in a
//!   hydrated page, which never matches (the server chose), an exact tag,
//!   so that its client links none of the matcher.
//!
//! **The typed forms' helpers**: what the generated `Locale::format`,
//! `with_locale`, `set_locale`, `preload_locale`, `current_locale` and
//! `install` call, each in the lookup's order (§5).
//!
//! Client-path code: no `core::fmt`, no panicking operation.

use mf2_catalog::Dir;

/// Defines `$name` twice: keeping its input under `cfg($cfg)`, dropping it
/// otherwise. `$d` is a `$`, which a macro cannot write into the macro it
/// defines any other way.
macro_rules! forward {
    ($d:tt $name:ident, $($cfg:tt)*) => {
        #[cfg($($cfg)*)]
        #[doc(hidden)]
        #[macro_export]
        macro_rules! $name {
            ($d($d t:tt)*) => { $d($d t)* };
        }
        #[cfg(not($($cfg)*))]
        #[doc(hidden)]
        #[macro_export]
        macro_rules! $name {
            ($d($d t:tt)*) => {};
        }
    };
}

forward!($ __if_host_std, feature = "host-std");
forward!($ __if_native, feature = "native");
forward!($ __if_ssr, all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8")));
forward!($ __if_client, all(any(feature = "hydrate", feature = "csr"), any(feature = "leptos", feature = "leptos-0-8")));
forward!($ __if_csr, all(feature = "csr", any(feature = "leptos", feature = "leptos-0-8")));
forward!($ __if_leptos, all(any(feature = "ssr", feature = "hydrate", feature = "csr"), any(feature = "leptos", feature = "leptos-0-8")));
forward!($ __if_mode, any(feature = "native", all(any(feature = "ssr", feature = "hydrate", feature = "csr"), any(feature = "leptos", feature = "leptos-0-8"))));
forward!($ __if_clap, feature = "clap");
forward!($ __if_axum, feature = "axum");
// `Locale::format`: a native application, a Leptos server, an Axum server.
forward!($ __if_format, any(feature = "native", feature = "axum", all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))));
// `install()`: every mode, and an Axum server.
forward!($ __if_install, any(feature = "native", feature = "axum", all(any(feature = "ssr", feature = "hydrate", feature = "csr"), any(feature = "leptos", feature = "leptos-0-8"))));
forward!($ __if_ratatui, feature = "ratatui");

/// The host: the native one wherever there is one (a server, a native
/// application, a test) — for a corpus a date can reach, the one that
/// resolves a named time zone, so that nothing else links a time-zone
/// database. `numbers` (a corpus a number can reach, under `number-intl`) changes
/// nothing here: off the browser the Rust path formats numbers.
#[cfg(all(feature = "host-std", feature = "datetime"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    ($(numbers)?) => {
        pub use $crate::host_std::HOST;
    };
    (dates $(numbers)?) => {
        pub use $crate::host_std::ZONES_HOST as HOST;
    };
}
/// The native one, without dates.
#[cfg(all(feature = "host-std", not(feature = "datetime")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    ($($use:ident)*) => {
        pub use $crate::host_std::HOST;
    };
}
/// The browser's, with `Intl.DateTimeFormat` for a corpus with dates: the
/// browser formats with `Intl` when it is the strongest of its side's
/// formatters (plan/08 §3.2).
#[cfg(all(
    not(feature = "host-std"),
    feature = "host-web",
    feature = "host-web-datetime-intl",
    not(feature = "host-web-datetime-icu")
))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    () => {
        pub use $crate::host_web::HOST;
    };
    (numbers) => {
        $crate::__numbers_host!(HOST, NUMBERS_HOST);
    };
    (dates) => {
        pub use $crate::host_web::INTL_HOST as HOST;
    };
    (dates numbers) => {
        $crate::__numbers_host!(INTL_HOST, INTL_DATES_NUMBERS_HOST);
    };
}
/// The browser's, with its zone data for a corpus with dates: ICU4X formats
/// in the browser, the strongest, even with `intl` on too.
#[cfg(all(
    not(feature = "host-std"),
    feature = "host-web",
    feature = "host-web-datetime-icu"
))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    () => {
        pub use $crate::host_web::HOST;
    };
    (numbers) => {
        $crate::__numbers_host!(HOST, NUMBERS_HOST);
    };
    (dates) => {
        pub use $crate::host_web::ZONES_HOST as HOST;
    };
    (dates numbers) => {
        $crate::__numbers_host!(ZONES_HOST, ZONES_NUMBERS_HOST);
    };
}
/// The browser's plain host: `iso`, or no formatter.
#[cfg(all(
    not(feature = "host-std"),
    feature = "host-web",
    not(feature = "host-web-datetime-intl"),
    not(feature = "host-web-datetime-icu")
))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    ($(dates)?) => {
        pub use $crate::host_web::HOST;
    };
    ($(dates)? numbers) => {
        $crate::__numbers_host!(HOST, NUMBERS_HOST);
    };
}
/// No host in this build: nothing to name.
#[cfg(not(any(feature = "host-std", feature = "host-web")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __use_host {
    ($($use:ident)*) => {};
}

/// A browser's host for a corpus a number can reach: with `number-intl`, the
/// `IntlNumbers` host over the one it would otherwise be (its second name),
/// so `Host::numbers` answers with `Intl.NumberFormat` and
/// `Intl.PluralRules` (`plan/01` §8 F1); without, that one (its first).
#[cfg(feature = "number-intl")]
#[doc(hidden)]
#[macro_export]
macro_rules! __numbers_host {
    ($plain:ident, $numbers:ident) => {
        pub use $crate::host_web::$numbers as HOST;
    };
}
/// Without `number-intl`: the host it would otherwise be.
#[cfg(not(feature = "number-intl"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __numbers_host {
    ($plain:ident, $numbers:ident) => {
        pub use $crate::host_web::$plain as HOST;
    };
}

/// `Locale::from_str`: the index in `$locales` of the locale that best
/// serves `$tag`, by the one matcher and the corpus's cut of CLDR's data.
#[cfg(any(feature = "host-std", not(feature = "hydrate")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __best_locale {
    ($matching:ident, $locales:ident, $tag:expr) => {
        $matching.best_match([$tag], $locales)
    };
}
/// In a hydrated page, which never matches: the exact tag, and none of the
/// matcher or its data linked.
#[cfg(all(feature = "hydrate", not(feature = "host-std")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __best_locale {
    ($matching:ident, $locales:ident, $tag:expr) => {
        $crate::__generated::exact_locale($locales, $tag)
    };
}

/// The index in `locales` of `tag`, ignoring ASCII case: what a hydrated
/// page's `Locale::from_str` compares.
#[must_use]
pub fn exact_locale(locales: &[(&str, Dir)], tag: &str) -> Option<usize> {
    locales
        .iter()
        .position(|(t, _)| t.eq_ignore_ascii_case(tag))
}

/// The index of `tag` in `locales`.
#[cfg(any(
    feature = "native",
    all(
        any(feature = "ssr", feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    )
))]
fn position(locales: &[(&str, Dir)], tag: &str) -> Option<usize> {
    locales.iter().position(|(t, _)| *t == tag)
}

/// The generated `current_locale()`: the index in `locales` of the language
/// formatted in now, in the lookup's order — the request's or the page's
/// catalog (tracked, in a reactive client), then the native store's — or
/// `None` when nothing is chosen yet. Beside a Leptos mode it never panics.
#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
#[must_use]
pub fn current_locale(locales: &[(&str, Dir)]) -> Option<usize> {
    #[cfg(not(feature = "ssr"))]
    crate::leptos::track_locale();
    if let Some(catalog) = crate::leptos::active() {
        return position(locales, catalog.locale());
    }
    #[cfg(feature = "native")]
    if let Some(tag) = crate::native::store::try_locale() {
        return position(locales, tag);
    }
    None
}

/// The generated `current_locale()` in a build whose only mode is
/// `native`: the language this thread formats in.
///
/// # Panics
///
/// Before `install()` outside `with_locale`, as `mf2::native::locale()`
/// does.
#[cfg(all(
    feature = "native",
    not(all(
        any(feature = "ssr", feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    ))
))]
#[must_use]
pub fn current_locale(locales: &[(&str, Dir)]) -> Option<usize> {
    position(locales, crate::native::locale())
}

/// The generated `set_locale(Locale)`: the native app-wide language, and a
/// client's switch, spawned (a failure is logged once and leaves the page as
/// it is). On the
/// server it does nothing: the request's language is the negotiation's.
///
/// # Panics
///
/// With `native`, before `install()`, as `mf2::native::set_locale` does.
#[cfg(any(
    feature = "native",
    all(
        any(feature = "ssr", feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    )
))]
pub fn set_locale(tag: &'static str) {
    // A tag of the corpus always matches; one whose catalog was not
    // shipped (`install_from_directory`) gets the closest there is, else
    // leaves the language as it is.
    #[cfg(feature = "native")]
    let _ = crate::native::set_locale(tag);
    #[cfg(all(
        any(feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    ))]
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = crate::leptos::set_locale(tag).await {
            crate::leptos::components::switch_failed(&error);
        }
    });
    let _ = tag;
}

/// The generated `preload_locale(Locale)`: a client fetches and checks the
/// catalog, spawned (a failure is logged once); the server does nothing.
#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
pub fn preload_locale(tag: &'static str) {
    #[cfg(any(feature = "hydrate", feature = "csr"))]
    wasm_bindgen_futures::spawn_local(async move {
        if crate::leptos::preload_locale(tag).await.is_err() {
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(
                "mf2: the locale could not be preloaded",
            ));
        }
    });
    let _ = tag;
}

/// The generated `install()` on a server whose module embeds the catalogs:
/// the setup, then the catalogs, each checked against the manifest hash.
///
/// # Panics
///
/// If a catalog does not load, which would mean a corrupt executable: the
/// build embeds the catalogs it checked.
#[cfg(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8")))]
pub fn install_server(
    setup: crate::leptos::Setup,
    catalogs: &'static [(&'static str, &'static str, &'static [u8], &'static [u8])],
) {
    crate::leptos::install(setup);
    if let Err(error) = crate::leptos::install_catalogs(catalogs) {
        refused(&error);
    }
}

#[cfg(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8")))]
#[cold]
#[inline(never)]
#[allow(
    clippy::panic,
    reason = "server only: install() returns nothing, and an embedded catalog that does not load means a corrupt executable"
)]
fn refused(error: &crate::leptos::LoadError) -> ! {
    panic!("mf2: install(): {error}")
}

/// The generated `Locale::format`: `message` in `locale`'s language, from
/// `corpus`'s catalogs, which it loads if nothing is installed.
///
/// # Panics
///
/// As `mf2::native::install` does: another corpus installed, or a catalog
/// that does not load.
#[cfg(feature = "native")]
#[must_use]
pub fn format_in(
    corpus: &'static crate::Corpus,
    locale: &str,
    message: &impl crate::Message,
) -> alloc::string::String {
    crate::native::store::format_in(corpus, locale, message)
}

/// The generated `Locale::format` on a server with no `native`: `message`
/// in `locale`'s language, from the catalogs `corpus` embeds, loaded once.
///
/// # Panics
///
/// If an embedded catalog does not load: a corrupt executable.
#[cfg(all(
    not(feature = "native"),
    any(
        feature = "axum",
        all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))
    )
))]
#[must_use]
pub fn format_in(
    corpus: &'static crate::Corpus,
    locale: &str,
    message: &impl crate::Message,
) -> alloc::string::String {
    server::format_in(corpus, locale, message)
}

#[cfg(all(
    not(feature = "native"),
    any(
        feature = "axum",
        all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))
    )
))]
mod server {
    use alloc::string::String;
    use alloc::vec::Vec;
    use std::sync::OnceLock;

    use mf2_catalog::{Catalog, CatalogError};
    use mf2_runtime::{FormatContext, Formatter, NoErrors};

    use crate::{Corpus, Message};

    /// The one corpus a server formats from, its catalogs loaded.
    struct Loaded {
        corpus: &'static Corpus,
        catalogs: Vec<Option<Catalog>>,
    }

    static LOADED: OnceLock<Loaded> = OnceLock::new();

    fn load(corpus: &'static Corpus) -> Vec<Option<Catalog>> {
        corpus
            .catalogs()
            .iter()
            .map(|file| {
                let bytes = file.bytes()?;
                match Catalog::new(Vec::from(bytes), corpus.manifest_hash())
                    .and_then(|catalog| catalog.with_server_data(file.server_data()))
                {
                    Ok(catalog) => Some(catalog),
                    Err(error) => refused(error),
                }
            })
            .collect()
    }

    pub(super) fn format_in(
        corpus: &'static Corpus,
        locale: &str,
        message: &impl Message,
    ) -> String {
        let loaded = LOADED.get_or_init(|| Loaded {
            corpus,
            catalogs: load(corpus),
        });
        // Another corpus than the first (two generated modules in one
        // server): loaded for this call alone.
        let other;
        let catalogs = if core::ptr::eq(loaded.corpus, corpus) {
            &loaded.catalogs
        } else {
            other = load(corpus);
            &other
        };
        let at = |tag: &str| {
            let index = corpus
                .catalogs()
                .iter()
                .position(|file| file.tag() == tag)?;
            catalogs.get(index)?.as_ref()
        };
        let mut out = String::new();
        if let Some(catalog) = at(locale).or_else(|| at(corpus.source_locale())) {
            let cx = FormatContext::new(corpus.host());
            let f = Formatter::new(catalog, corpus.registry(), &cx);
            message.write(&f, &mut out, &mut NoErrors);
        }
        out
    }

    #[cold]
    #[inline(never)]
    #[allow(
        clippy::panic,
        reason = "server only: Locale::format returns text, and an embedded catalog that does not load means a corrupt executable"
    )]
    fn refused(error: CatalogError) -> ! {
        panic!("mf2: Locale::format(): {error}")
    }
}

/// The generated `install()` under `axum`: the corpus whose locales
/// `mf2::axum` negotiates among and whose catalogs it serves.
#[cfg(feature = "axum")]
pub fn install_axum(corpus: &'static crate::Corpus) {
    crate::axum::install(corpus);
}

/// What the generated `Locale` extractor names.
#[cfg(feature = "axum")]
pub mod axum {
    pub use ::axum::extract::FromRequestParts;
    pub use ::http::request::Parts;
    use mf2_catalog::Dir;

    /// The index, among `locales`, of this request's language: what a layer
    /// negotiated, else what `Negotiator::default()`'s sources negotiate.
    #[must_use]
    pub fn locale_index(
        parts: &Parts,
        locales: &'static [(&'static str, Dir)],
        source: &'static str,
    ) -> Option<usize> {
        crate::axum::locale_index(parts, locales, source)
    }
}

/// The generated `with_locale(Locale, body)`: `body`, with this thread
/// formatting in `locale`'s language; loads `corpus`'s catalogs if nothing
/// is installed.
///
/// # Panics
///
/// As [`format_in`].
#[cfg(feature = "native")]
pub fn with_locale_in<R>(
    corpus: &'static crate::Corpus,
    locale: &str,
    body: impl FnOnce() -> R,
) -> R {
    crate::native::store::with_locale_in(corpus, locale, body)
}

/// The string type of the generated `Locale::format`.
#[cfg(any(
    feature = "native",
    feature = "axum",
    all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))
))]
pub use alloc::string::String;

/// The path type of the generated `install_from_directory`.
#[cfg(feature = "native")]
pub use std::path::Path;

/// The value parser a generated `Locale` gets with `clap`: it parses
/// through the one matcher (`--lang fr_CA.UTF-8` is French), and lists the
/// tags in `--help`. clap's `ValueEnum` would match exactly.
#[cfg(feature = "clap")]
pub mod clap {
    use alloc::boxed::Box;
    use std::ffi::OsStr;

    pub use ::clap::builder::ValueParserFactory;
    use ::clap::builder::{PossibleValue, TypedValueParser};
    use mf2_catalog::Dir;

    use crate::UnknownLocale;

    /// `Locale`'s parser: its `from_str`, and its tags.
    pub struct LocaleParser<L> {
        locales: &'static [(&'static str, Dir)],
        parse: fn(&str) -> Result<L, UnknownLocale>,
    }

    impl<L> LocaleParser<L> {
        /// What the generated `ValueParserFactory` impl returns.
        #[must_use]
        pub const fn new(
            locales: &'static [(&'static str, Dir)],
            parse: fn(&str) -> Result<L, UnknownLocale>,
        ) -> Self {
            LocaleParser { locales, parse }
        }
    }

    /// Its tags.
    impl<L> core::fmt::Debug for LocaleParser<L> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("LocaleParser")
                .field("locales", &self.locales)
                .finish_non_exhaustive()
        }
    }

    impl<L> Clone for LocaleParser<L> {
        fn clone(&self) -> Self {
            LocaleParser {
                locales: self.locales,
                parse: self.parse,
            }
        }
    }

    impl<L: Clone + Send + Sync + 'static> TypedValueParser for LocaleParser<L> {
        type Value = L;

        fn parse_ref(
            &self,
            cmd: &::clap::Command,
            arg: Option<&::clap::Arg>,
            value: &OsStr,
        ) -> Result<L, ::clap::Error> {
            // clap's own parser of a `FromStr` type: its message names the
            // argument and the value, and gives `UnknownLocale`'s text.
            self.parse.parse_ref(cmd, arg, value)
        }

        fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
            Some(Box::new(
                self.locales.iter().map(|(tag, _)| PossibleValue::new(*tag)),
            ))
        }
    }
}
