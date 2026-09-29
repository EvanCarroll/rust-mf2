//! `mf2-leptos-ui-0-9` — the built-in Leptos 0.9 components of Rust MF2: the
//! locale switcher and its options, the catalog preload and links, the
//! `hreflang` block and the islands gate.
//!
//! Applications do not name this crate: `mf2::leptos` re-exports its
//! components, with `mf2`'s [`Layer`] already chosen. It exists because
//! Leptos' `view!` and `#[component]` write `::leptos` into the crate that
//! uses them, and `mf2` reaches its two Leptos lines under names of its own,
//! with the module `mf2::leptos` in the way; here the line is simply
//! `leptos`. `mf2-leptos-ui-0-8` compiles the same source against Leptos 0.8.
//!
//! Client-path code: `forbid(unsafe_code)`, no panicking operation.
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide. An application starts at [`mf2`](https://docs.rs/mf2).

#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
// Each component's `_layer` prop exists for its type alone; `#[component]`
// passes it from the props to the body, which reads nothing from it.
#![allow(
    clippy::used_underscore_binding,
    reason = "`#[component]` hands the phantom `_layer` prop to the body, which never reads it"
)]

#[cfg(all(feature = "ssr", any(feature = "hydrate", feature = "csr")))]
compile_error!("mf2-leptos-ui-0-9: turn on exactly one of `ssr`, `hydrate` and `csr`.");
#[cfg(all(feature = "hydrate", feature = "csr"))]
compile_error!("mf2-leptos-ui-0-9: turn on exactly one of `ssr`, `hydrate` and `csr`.");

mod ui;

pub use ui::*;
