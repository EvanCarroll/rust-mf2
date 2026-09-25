//! The reference application migrated from `leptos-fluent`: its call sites
//! as `mf2 convert --from leptos-fluent` rewrote them, and this file, the
//! server and the shell finished by hand as
//! docs/migrating-from-leptos-fluent.md says — the initializer replaced by
//! the translation crate's `setup()`, installed before the client boots.

#![recursion_limit = "512"]
#![allow(unused, clippy::all, clippy::pedantic)]

pub mod app;
pub mod components;
pub mod support;
pub mod tables;
pub mod widgets;

/// The A/B's timing hooks, in the timed build only.
#[cfg(all(feature = "ab-bench", feature = "hydrate"))]
pub mod ab;

/// Client entry point: install the translation crate, then hydrate once the
/// page's catalog is in (`hydrate_lazy`, for the `#[lazy_route]` routes).
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos_mf2::install(workload_i18n::setup());
    #[cfg(not(feature = "ab-bench"))]
    leptos_mf2::hydrate_lazy(app::App);
    #[cfg(feature = "ab-bench")]
    leptos_mf2::hydrate_lazy(ab::app);
}
