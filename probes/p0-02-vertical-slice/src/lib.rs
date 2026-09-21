//! P0.2 + P0.10 probe application (cargo-leptos: lib = client wasm + SSR app,
//! bin = Axum server).
#![recursion_limit = "256"]

pub mod app;
pub mod error;
#[cfg(feature = "ssr")]
pub mod i18n_server;
pub mod msg;
pub mod p010;

/// The wasm entry point. The `#[wasm_bindgen]` export belongs to the
/// application, not to the i18n crate (which stays `forbid(unsafe_code)`).
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    // fetch the preloaded catalog → validate → install → hydrate_lazy
    tr::client::hydrate_lazy(app::App, msg::MANIFEST_HASH);
}

/// Probe diagnostics for the e2e harness: live registry slots (strategy B).
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mf2_live_nodes() -> usize {
    tr::live_nodes()
}
