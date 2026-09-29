//! `leptos-mf2` — 1.x's Leptos layer of Rust MF2, kept as a shim: every item
//! it named, under the path it named it, re-exported from
//! [`mf2`](https://docs.rs/mf2), where the layer now lives as
//! `mf2::leptos` and the call-site types at `mf2`'s root.
//!
//! Its features forward to `mf2`'s: `ssr`, `hydrate` and `csr` to the
//! modes of the same name; `leptos-0-9` (the default) to `mf2`'s `leptos`,
//! and `leptos-0-8` to `mf2`'s `leptos-0-8`; `static-locale`,
//! `mark-fallback-lang` and `fn-datetime` to theirs. A 1.x application
//! keeps compiling unchanged; a new one names `mf2` alone:
//!
//! ```toml
//! mf2 = { version = "2", features = ["leptos"] }
//! ```
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide. An application starts at [`mf2`](https://docs.rs/mf2).

#![no_std]
#![forbid(unsafe_code)]

/// The call-site core, and what `tr!` expands to.
pub use mf2::{
    ArgList, ArgSource, ArgValue, DateTimeValue, Handler, IntoMarkupHandler, MarkupHandler, Text,
    Tr, TrArgs, TrDyn, TrRich,
};
#[doc(hidden)]
pub use mf2::{
    markup, tr, tr_args_n, tr_args0, tr_args1, tr_args2, tr_args3, tr_args4, tr_dyn, tr_rich,
};

/// The Leptos layer, with a mode: everything `mf2::leptos` names, and
/// `islands_gate!`.
#[cfg(any(feature = "ssr", feature = "hydrate", feature = "csr"))]
pub use mf2::leptos::*;

/// `islands_gate!` in a build with no mode, where 1.x had it too: it
/// expands to nothing.
#[cfg(not(any(feature = "ssr", feature = "hydrate", feature = "csr")))]
#[doc(hidden)]
#[macro_export]
macro_rules! islands_gate {
    () => {};
}
