//! Getting started's application in one crate: the page, the browser's
//! entry point, and the translations (`build.rs`, `mf2.toml`, `locales/`).

// Declared before the include: imports `tr!` through the crate's prelude.
mod pages;

// tr!, msg_id!, the prelude, MANIFEST_HASH, LOCALES, registry(), host,
// and — under `ssr` — CATALOGS.
mf2::include_generated!("mf2_generated_3c.rs");

// Declared after the include: imports `tr!` by path.
mod app;

pub use app::{App, shell};

/// What the application installs once on each side (1.x's hand-shaped
/// `setup()`, now in the application itself).
#[cfg(any(feature = "ssr", feature = "hydrate"))]
#[must_use]
pub fn setup() -> mf2::leptos_mf2::Setup {
    mf2::leptos_mf2::Setup::new(
        registry(),
        &host::HOST,
        MANIFEST_HASH,
        SOURCE_LOCALE,
        LOCALES,
    )
}

/// The crate root, after the include: `tr!` with no import.
#[must_use]
pub fn title() -> mf2::Tr {
    tr!("app-title")
}

/// The browser's entry point.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos_mf2::install(setup());
    leptos_mf2::hydrate_lazy(App);
}
