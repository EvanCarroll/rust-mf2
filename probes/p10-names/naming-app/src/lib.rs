//! An application beside 2.0's module names (Phase 10 A7): its own `leptos`
//! and `axum`, and the library's `naming::leptos` / `naming::axum`.

use leptos::prelude::*;
use naming::leptos::prelude::bump;

/// The application's own view, next to the library's module.
pub fn page() -> impl IntoView {
    let (count, owned) = bump();
    view! { <p>{count} {owned}</p> }
}

/// The library's router merged into the application's.
#[must_use]
pub fn router() -> axum::Router {
    axum::Router::new()
        .merge(naming::axum::routes())
        .merge(naming::axum::nested_routes())
}

/// The library's modules by path, which never collides.
#[must_use]
pub fn by_path() -> u32 {
    naming::leptos::bump().0
}

// N5: importing the module itself binds `leptos` in this scope, and a bare
// `leptos` in a `use` is then ambiguous with the application's own crate.
#[cfg(feature = "n5-bare-module-import")]
use naming::leptos;

#[cfg(feature = "n5-bare-module-import")]
use leptos::prelude::RwSignal as _;

/// With N5 on, `view!` still expands to `::leptos::…`, the crate.
#[cfg(feature = "n5-bare-module-import")]
pub fn still_fine() -> impl IntoView {
    let _ = leptos::counter();
    view! { <p>"fine"</p> }
}
