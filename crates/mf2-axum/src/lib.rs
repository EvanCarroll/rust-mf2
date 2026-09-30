//! `mf2-axum` — 1.x's Axum crate of Rust MF2, kept as a shim: everything
//! [`mf2::axum`](https://docs.rs/mf2) names, re-exported, and the Leptos
//! server's request glue, [`provide_locale`] and [`install`].
//!
//! The negotiation ([`Negotiator`], its sources and sinks), the catalog
//! routes ([`catalog_routes`]) and [`path_prefix_redirect`] are `mf2`'s
//! module `mf2::axum`, which also serves a plain Axum application with no
//! Leptos. A new application names `mf2` alone, with `axum` beside its mode:
//!
//! ```toml
//! mf2 = { version = "2", features = ["leptos", "axum"] }
//! ```
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide. An application starts at [`mf2`](https://docs.rs/mf2).

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

pub use context::{negotiated, provide_locale};
/// Negotiation, the catalog routes and the path-prefix redirect: `mf2::axum`.
pub use mf2::axum::*;

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
