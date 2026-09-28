//! A stand-in for 2.0's `mf2` (Phase 10 A7, items 1 and 3): public modules
//! named after the frameworks — `leptos`, `axum` — in the crate that depends
//! on those frameworks, with no root rename.
//!
//! The Leptos line is reached through an internal alias (`line`), so no
//! crate named `leptos` is in this crate's extern prelude at all; `axum`
//! keeps its real name, and only the crate root has to say which it means.

/// The active Leptos line under one internal name. Code reaches it as
/// `crate::line::leptos`, never as a bare `leptos`.
mod line {
    #[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
    pub(crate) use ::leptos_0_8 as leptos;
    #[cfg(feature = "leptos-0-9")]
    pub(crate) use ::leptos_0_9 as leptos;
}

/// `mf2::leptos`: the Leptos integration, named after the framework.
pub mod leptos {
    use crate::line::leptos::prelude::{Get, Owner, RwSignal, Set};

    /// A signal of the active line, made from inside the module named
    /// `leptos`.
    #[must_use]
    pub fn counter() -> RwSignal<u32> {
        RwSignal::new(0)
    }

    /// Reads and writes it, and asks the line for the current owner.
    #[must_use]
    pub fn bump() -> (u32, bool) {
        let owner = Owner::new();
        owner.with(|| {
            let c = counter();
            c.set(c.get() + 1);
            (c.get(), Owner::current().is_some())
        })
    }

    /// What an application imports.
    pub mod prelude {
        pub use super::{bump, counter};
    }
}

/// `mf2::axum`: negotiation and catalog serving, named after the framework.
#[cfg(feature = "axum")]
pub mod axum {
    // Inside the module named `axum`, a bare `axum` in a `use` is the crate:
    // a module is not in its own scope.
    use axum::Router;

    /// The catalog routes, as a router of the real crate.
    #[must_use]
    pub fn routes() -> Router {
        Router::new()
    }

    pub(crate) mod nested {
        // Deeper still: the same.
        pub(crate) fn empty() -> axum::Router {
            axum::Router::new()
        }
    }

    /// The nested module's router.
    #[must_use]
    pub fn nested_routes() -> Router {
        nested::empty()
    }
}

// N1: the 1.x arrangement beside `pub mod leptos` — the 0.8 line renamed
// back to `leptos` at the root, as `view!` needs. Expected: E0260.
#[cfg(feature = "n1-root-rename")]
extern crate leptos_0_8 as leptos;

// N3: a bare `axum` in a `use` at the root, where both the module and the
// crate are in scope. Expected: E0659, ambiguous.
#[cfg(feature = "n3-axum-root-use")]
use axum::Router;

/// N3's use of the import.
#[cfg(feature = "n3-axum-root-use")]
#[must_use]
pub fn root_router() -> Router {
    Router::new()
}

/// N4: an expression path `axum::Router` at the root. Expected: it names
/// the module, which has no `Router` — `::axum::Router` is the crate's.
#[cfg(feature = "n4-axum-root-path")]
#[must_use]
pub fn root_router_path() -> axum::Router {
    axum::Router::new()
}

/// The fix for N3 and N4: `::axum` is always the crate.
#[cfg(feature = "axum")]
#[must_use]
pub fn root_router_absolute() -> ::axum::Router {
    ::axum::Router::new()
}
