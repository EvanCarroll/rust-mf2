//! `mf2-leptos-ui-0-8` — the built-in Leptos 0.8 components of Rust MF2: the
//! locale switcher and its options, the catalog preload and links, the
//! `hreflang` block and the islands gate.
//!
//! Applications do not name this crate: `mf2::leptos` wraps each of its
//! components, with `mf2`'s [`Layer`] chosen, when `mf2`'s
//! `leptos-0-8` feature is on. It compiles `mf2-leptos-ui-0-9`'s source
//! (`src/ui.rs` is a link to that crate's) against Leptos 0.8, which this
//! crate names `leptos`, so `view!` and `#[component]` reach it.
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

// The 0.8 line under its real name: `view!` and `#[component]` expand to
// `::leptos::…`. The workspace declares it once, as `leptos_0_8`; this crate
// has no module named `leptos`, so the name is free for it here.
extern crate leptos_0_8 as leptos;

// `mf2`'s own sentences: `mf2` forwards its mode here, and cargo compiles
// this crate before `mf2`, so with two modes this is the error the user
// reads, and `mf2`'s is never reached. The features it names are `mf2`'s.
#[cfg(all(feature = "ssr", any(feature = "hydrate", feature = "csr")))]
compile_error!(
    "mf2: turn on exactly one of `ssr`, `hydrate` and `csr`. cargo unifies \
     features across a workspace, so an application that is built both ways \
     belongs in a workspace of its own — as `examples/demo-ssr` and \
     `conformance/l6-web` are."
);
#[cfg(all(feature = "hydrate", feature = "csr"))]
compile_error!("mf2: turn on exactly one of `ssr`, `hydrate` and `csr`.");

// `src/ui.rs` is a symbolic link to `mf2-leptos-ui-0-9`'s, as every crate's
// `LICENSE` is a link to the repository's: `cargo package` follows it, so
// the published crate carries the file, and `cargo xtask package` checks
// where it points. A checkout without symbolic links (git's
// `core.symlinks=false`) holds the target's path as the file's text, and
// this crate does not compile there: such a checkout needs symbolic links
// turned on (on Windows, Developer Mode).
mod ui;

pub use ui::*;
