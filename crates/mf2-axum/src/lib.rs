//! `mf2-axum` — locale negotiation and catalog serving for an Axum + Leptos
//! application (`plans/04-leptos-integration.md` §6).
//!
//! Three things, and nothing else:
//!
//! 1. **Negotiation** ([`Negotiator`]) — an ordered list of typed
//!    [`LocaleSource`]s and [`LocaleSink`]s: a cookie, `Accept-Language`, a
//!    path prefix, a query parameter. The first source that offers a locale
//!    this build has wins. Never a boolean matrix of
//!    `from_<source>_to_<target>` (§11, item 5).
//! 2. **The catalogs** ([`catalog_routes`]) — `/i18n/*` from the bytes
//!    embedded in the server binary, with the precompressed variant and
//!    `Cache-Control: immutable`.
//! 3. **The per-request context** ([`provide_locale`]) — the catalog for
//!    every `Tr` the request renders, plus `Content-Language`, `Vary` and the
//!    cookie on the way out.
//!
//! The negotiated locale is **serialized into the page** — `<html lang dir>`
//! and the preload link — and the client reads it rather than negotiating
//! again. The prior-art audit found re-negotiation at hydration behind a long
//! tail of bugs (§11, item 3), and the browser checks assert that this
//! library does not do it.
//!
//! ```ignore
//! use mf2_axum::{AcceptLanguage, CookieLocale, Negotiator};
//!
//! let negotiator = Arc::new(
//!     Negotiator::empty()
//!         .source(CookieLocale::default())
//!         .source(AcceptLanguage)
//!         .sink(CookieLocale::default()),
//! );
//! ```

mod context;
mod negotiate;
mod serve;

pub use context::{negotiated, provide_locale};
pub use negotiate::{
    AcceptLanguage, CookieLocale, LocaleSink, LocaleSource, Negotiated, Negotiator, PathPrefix,
    QueryParam,
};
pub use serve::{CATALOG_PREFIX, catalog_routes};

/// Installs the application's generated i18n module on the server: the
/// registry, the host, the manifest hash, the locale table, and the
/// catalogs. Call it once in `main`, before the router is built.
///
/// ```ignore
/// mf2_axum::install(
///     my_app_i18n::setup(),
///     my_app_i18n::CATALOGS,
/// )?;
/// ```
///
/// Every catalog is validated against `MANIFEST_HASH` here, so a deploy that
/// mixes a wasm with another build's catalogs fails at boot rather than in a
/// request.
pub fn install(
    setup: leptos_mf2::Setup,
    catalogs: &[(&'static str, &'static str, &'static [u8])],
) -> Result<(), leptos_mf2::LoadError> {
    leptos_mf2::install(setup);
    leptos_mf2::install_catalogs(catalogs)
}
