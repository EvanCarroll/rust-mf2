//! Support module (P0.9 templates `tr`, `trivial`, `direct`): the hydrate
//! boot hook compares the manifest hash compiled into the wasm with the one
//! the server rendered, and runs the second crate's client code.

/// Called first in `hydrate()`.
pub fn boot() {
    #[cfg(feature = "hydrate")]
    {
        p09_i18n::__mf2::client_check(p09_i18n::MANIFEST_HASH);
        p09_second::client_boot();
    }
}
