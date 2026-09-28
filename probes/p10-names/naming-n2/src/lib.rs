//! N2 (Phase 10 A7): `pub mod leptos` beside a dependency named `leptos`
//! (the 0.9 line under its real name). What works and what does not.

/// The module named after the framework.
pub mod leptos {
    // Inside the module, a bare `leptos` in a `use` is the crate.
    use leptos::prelude::*;

    /// A signal of the line.
    #[must_use]
    pub fn counter() -> RwSignal<u32> {
        RwSignal::new(0)
    }

    /// `view!` inside the module: it expands to `::leptos::…`, the crate.
    pub fn hello() -> impl IntoView {
        view! { <p>"hello"</p> }
    }
}

// The root needs the prelude's traits in scope for `view!`, like any
// Leptos code; `::leptos` is unambiguous here.
use ::leptos::prelude::ElementChild;

/// A component at the root, beside the module: `#[component]` and `view!`
/// expand to `::leptos::…`, which is always the crate — so with the 0.9 line
/// under its real name the macros work in the crate that has the module.
/// Only 0.8, which needed the name bound by an `extern crate` item, clashed.
#[::leptos::component]
pub fn Greeting() -> impl ::leptos::IntoView {
    ::leptos::view! { <p>"greeting"</p> }
}

// N2: a bare `leptos` in a `use` at the root. Expected: E0659, ambiguous.
#[cfg(feature = "n2-root-use")]
use leptos::prelude::RwSignal;

/// N2's use of the import.
#[cfg(feature = "n2-root-use")]
#[must_use]
pub fn root_signal() -> RwSignal<u32> {
    RwSignal::new(1)
}
