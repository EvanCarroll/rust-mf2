//! The client half: what the translation crate's module looks like in the
//! `hydrate` build.

/// The line the client would log.
#[must_use]
pub fn report() -> String {
    i18n::report()
}

/// The client's entry point, as a Leptos application's `hydrate()`.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() -> String {
    report()
}
