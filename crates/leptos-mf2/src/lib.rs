//! `leptos-mf2` — 1.x's Leptos layer of Rust MF2, kept as a shim: every item
//! it named, under the path it named it, re-exported from
//! [`mf2`](https://docs.rs/mf2), where the layer now lives as
//! `mf2::leptos` and the call-site types at `mf2`'s root.
//!
//! Its features forward to `mf2`'s: `ssr`, `hydrate` and `csr` to the
//! modes of the same name; `leptos-0-9` (the default) to `mf2`'s `leptos`,
//! and `leptos-0-8` to `mf2`'s `leptos-0-8`; `static-locale`,
//! `mark-fallback-lang` and `fn-datetime` to theirs. A 1.x application
//! keeps compiling unchanged, beside a native application on `mf2-native`
//! or `mf2-ratatui` in one workspace too: `mf2` refuses `native` and
//! `ratatui` beside `hydrate` or `csr` only when compiling for the browser
//! (`wasm32`). A new one names `mf2` alone:
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

/// With no mode here, the layer all the same whenever `mf2` compiles it: in
/// 1.x `mf2/ssr` turned on `leptos-mf2/ssr`, so an application may name its
/// mode on `mf2` alone while a crate it depends on has only this crate's
/// `leptos` (`tests/layer.rs`). With no layer at all, `islands_gate!`, which
/// expands to nothing, as in 1.x.
#[cfg(not(any(feature = "ssr", feature = "hydrate", feature = "csr")))]
#[doc(hidden)]
pub use mf2::leptos_mf2::*;
