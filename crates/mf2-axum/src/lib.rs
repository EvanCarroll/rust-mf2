//! `mf2-axum` — locale negotiation and catalog serving for an Axum + Leptos
//! application.
//!
//! Three things, and nothing else:
//!
//! 1. **Negotiation** ([`Negotiator`]) — an ordered list of typed
//!    [`LocaleSource`]s and [`LocaleSink`]s: a cookie, `Accept-Language`, a
//!    path prefix, a query parameter. The first source that offers a locale
//!    this build has wins. Never a boolean matrix of
//!    `from_<source>_to_<target>` options.
//! 2. **The catalogs** ([`catalog_routes`]) — `/i18n/*` from the bytes
//!    embedded in the server binary, with the precompressed variant and
//!    `Cache-Control: immutable`.
//! 3. **The per-request context** ([`provide_locale`]) — the catalog for
//!    every `Tr` the request renders, plus `Content-Language`, `Vary` and the
//!    cookie on the way out.
//!
//! The negotiated locale is **serialized into the page** — `<html lang dir>`
//! and the preload link — and the client reads it rather than negotiating
//! again: negotiating again at hydration is behind a long tail of bugs in
//! other i18n libraries, and this library's browser checks assert that it
//! does not.
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
//!
//! # The user guide
//!
//! Getting started, call sites, delivery modes, switching language,
//! accessibility, migrating from `leptos-fluent`, and what 1.x promises
//! See the [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) for the
//! ecosystem and application guides. An application starts at
//! [`mf2`](https://docs.rs/mf2).

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]

// The Leptos line, as in leptos-mf2 (`plans/04-leptos-integration.md` §10):
// the 0.8 crates, when they are the ones on, renamed back.
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate leptos_0_8 as leptos;
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate leptos_axum_0_8 as leptos_axum;

#[cfg(all(feature = "leptos-0-8", feature = "leptos-0-9"))]
compile_error!(
    "mf2-axum: `leptos-0-8` is on, and so is the default `leptos-0-9`. \
     For Leptos 0.8, every dependency on leptos-mf2 and mf2-axum needs \
     `default-features = false` beside `features = [\"leptos-0-8\"]`."
);
#[cfg(not(any(feature = "leptos-0-8", feature = "leptos-0-9")))]
compile_error!(
    "mf2-axum: no Leptos line. Depend on mf2-axum with its default features \
     (Leptos 0.9), or turn on `leptos-0-8` for Leptos 0.8."
);

mod context;
mod negotiate;
mod redirect;
mod serve;

pub use context::{negotiated, provide_locale};
pub use negotiate::{
    AcceptLanguage, CookieLocale, LocaleSink, LocaleSource, Negotiated, Negotiator, PathPrefix,
    QueryParam,
};
pub use redirect::path_prefix_redirect;
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
