//! An Axum server's side of Rust MF2, with or without Leptos: locale
//! negotiation, the catalogs served from the server binary, and the
//! generated `Locale` as an extractor.
//!
//! 1. **Negotiation** ([`Negotiator`]) — an ordered list of typed
//!    [`LocaleSource`]s and [`LocaleSink`]s: a cookie, `Accept-Language`, a
//!    path prefix, a query parameter. The first source that offers a locale
//!    this build has wins, through the one matcher. Never a boolean matrix of
//!    `from_<source>_to_<target>` options.
//! 2. **The catalogs** ([`catalog_routes`]) — `/i18n/*` from the bytes
//!    embedded in the server binary, with the precompressed variant and
//!    `Cache-Control: immutable`.
//! 3. **The extractor.** With `axum`, the generated module's `Locale` is an
//!    extractor, and `Locale::format` formats a description in its language:
//!
//! ```ignore
//! async fn hello(locale: my_i18n::Locale) -> String {
//!     locale.format(&my_i18n::tr!("hello"))
//! }
//!
//! my_i18n::install(); // once, before the router serves
//! let app = axum::Router::new()
//!     .route("/", axum::routing::get(hello))
//!     .merge(mf2::axum::catalog_routes());
//! ```
//!
//! The extractor takes the language a layer negotiated, when one did, else
//! negotiates the request with [`Negotiator::default`]'s sources.
//!
//! A Leptos server (`ssr`) serializes the negotiated locale into the page
//! (`<html lang dir>` and the preload link), and the client reads it rather
//! than negotiating again. Until the negotiation is a tower layer, its
//! request glue (`provide_locale`, which needs `leptos_axum`) is the
//! `mf2-axum` crate's.
//!
//! See the user guide, the [Rust MF2 book](https://evancarroll.github.io/rust-mf2/).

mod negotiate;
mod redirect;
mod serve;

use std::sync::OnceLock;

use mf2_catalog::Dir;

use crate::Corpus;

pub use negotiate::{
    AcceptLanguage, CookieLocale, LocaleSink, LocaleSource, Negotiated, Negotiator, PathPrefix,
    QueryParam,
};
pub use redirect::path_prefix_redirect;
pub use serve::{CATALOG_PREFIX, catalog_routes};

/// The corpus the generated `install()` gave the server: its locales, its
/// source locale, and the catalogs `/i18n/*` serves.
static INSTALLED: OnceLock<&'static Corpus> = OnceLock::new();

/// What the generated `install()` calls under `axum`. A second corpus is
/// ignored: one process serves one application's catalogs.
pub(crate) fn install(corpus: &'static Corpus) {
    let _ = INSTALLED.set(corpus);
}

/// Every locale the server offers: the installed corpus's, else (a Leptos
/// server that installed its setup by hand) the Leptos layer's, else none.
pub(crate) fn locales() -> &'static [(&'static str, Dir)] {
    if let Some(corpus) = INSTALLED.get() {
        return corpus.locales();
    }
    #[cfg(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8")))]
    {
        crate::leptos::locales()
    }
    #[cfg(not(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))))]
    {
        &[]
    }
}

/// The locale a request with no match gets, from the same place as
/// [`locales`].
pub(crate) fn source_locale() -> &'static str {
    if let Some(corpus) = INSTALLED.get() {
        return corpus.source_locale();
    }
    #[cfg(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8")))]
    {
        crate::leptos::source_locale()
    }
    #[cfg(not(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))))]
    {
        ""
    }
}

/// The immutable file name `tag`'s catalog is published under.
pub(crate) fn catalog_name(tag: &str) -> Option<&'static str> {
    if let Some(corpus) = INSTALLED.get() {
        return corpus
            .catalogs()
            .iter()
            .find(|file| file.tag() == tag && file.bytes().is_some())
            .map(crate::CatalogFile::file_name);
    }
    #[cfg(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8")))]
    {
        crate::leptos::catalog_name(tag)
    }
    #[cfg(not(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))))]
    {
        None
    }
}

/// The catalog published under `name`, with the name as a `&'static str`.
pub(crate) fn catalog_file(name: &str) -> Option<(&'static str, &'static [u8])> {
    if let Some(corpus) = INSTALLED.get() {
        return corpus
            .catalogs()
            .iter()
            .find(|file| file.file_name() == name)
            .and_then(|file| Some((file.file_name(), file.bytes()?)));
    }
    #[cfg(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8")))]
    {
        crate::leptos::catalog_entries()
            .iter()
            .find(|entry| entry.file == name)
            .map(|entry| (entry.file, entry.bytes))
    }
    #[cfg(not(all(feature = "ssr", any(feature = "leptos", feature = "leptos-0-8"))))]
    {
        None
    }
}

/// The one matcher over CLDR's whole table (plans/19-native-and-terminal.md
/// §9), a reader's list best first.
pub(crate) fn best_locale<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    locales: &[(&'static str, Dir)],
) -> Option<(&'static str, Dir)> {
    let at = crate::LanguageMatching::cldr().best_match(candidates, locales)?;
    locales.get(at).copied()
}

/// What the generated extractor calls: the index, among `locales`, of the
/// language a layer negotiated for this request, else of what
/// [`Negotiator::default`]'s sources negotiate over `locales`.
pub(crate) fn locale_index(
    parts: &::http::request::Parts,
    locales: &'static [(&'static str, Dir)],
    source: &'static str,
) -> Option<usize> {
    let tag = match parts.extensions.get::<Negotiated>() {
        Some(negotiated) => negotiated.tag,
        None => {
            Negotiator::over(locales, source)
                .defaults()
                .negotiate(parts)
                .tag
        }
    };
    locales.iter().position(|(t, _)| *t == tag)
}
