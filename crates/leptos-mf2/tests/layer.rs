//! The Leptos layer under 1.x's `leptos_mf2::` paths, whichever crate turned
//! the mode on.
//!
//! In 1.x, `mf2/ssr` turned on `leptos-mf2/ssr`. The layer is `mf2`'s now, so
//! in an application that names its mode on `mf2` alone, a crate that depends
//! on this one with only `leptos` (as `conformance/l7-web/sets/*` do) sees it
//! with no mode of its own, and must still reach the layer through it.
//! `cargo xtask ci` runs this in that arrangement:
//!
//! ```sh
//! cargo test -p leptos-mf2 --features leptos,mf2/ssr --test layer
//! ```

use core::any::TypeId;

/// Whether `A` and `B` are one type.
fn same<A: ?Sized + 'static, B: ?Sized + 'static>() -> bool {
    TypeId::of::<A>() == TypeId::of::<B>()
}

// As an application writes it beside its client's entry point; with `ssr`
// it expands to nothing.
leptos_mf2::islands_gate!();

#[test]
fn the_layer_is_reached_through_the_shim() {
    assert!(same::<leptos_mf2::Setup, mf2::leptos::Setup>());
    assert!(same::<leptos_mf2::LoadError, mf2::leptos::LoadError>());
    assert!(same::<leptos_mf2::RequestI18n, mf2::leptos::RequestI18n>());
    assert!(same::<
        leptos_mf2::LocaleSwitcherProps,
        mf2::leptos::LocaleSwitcherProps,
    >());
    let _: fn(leptos_mf2::Setup) = leptos_mf2::install;
    let _: fn() -> (String, &'static str) = leptos_mf2::html_lang;
    let _ = leptos_mf2::catalog;
    let _ = leptos_mf2::links::catalog_href;
    let _ = leptos_mf2::components::LocaleSwitcher;
    // Nothing is installed in this test.
    assert!(leptos_mf2::setup().is_none());
}
