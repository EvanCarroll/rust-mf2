//! `leptos-mf2` — what a `tr!` call site builds, and how Leptos renders it:
//! the catalog of the request or of the page, the text of every `tr!`, and
//! the switch from one language to another.
//!
//! The crate has two halves, and the Leptos mode is the seam:
//!
//! * **the call-site core** (always): [`Tr`], [`TrArgs`], [`TrRich`],
//!   [`TrDyn`], [`ArgValue`] and the lowering that borrows them into the
//!   runtime's `Arg` at format time. It formats against a
//!   [`Formatter`](mf2_runtime::Formatter) the caller supplies, so a server,
//!   a test and `mf2-cli` use it with no Leptos code compiled at all;
//! * **the Leptos layer** (with a mode: `ssr`, `hydrate` or `csr`): the
//!   tachys view impls, the per-request and per-client catalog, the node
//!   registry and the locale switch.
//!
//! # Modes and Leptos lines
//!
//! An application turns on exactly one mode — `ssr` in the server's build,
//! `hydrate` in the client's (`csr` for a client-only application) — and
//! forwards it to every Leptos-aware dependency, as cargo-leptos does. The
//! modes exclude each other, so this documentation shows one: **`ssr`, on
//! Leptos 0.9**, the default line (Leptos 0.8 is the opt-in: no default
//! features, and `leptos-0-8`). The client modes add:
//!
//! | Mode | Adds |
//! |---|---|
//! | `hydrate` or `csr` | the active catalog's change notifier: `changed`, `set_active`, `track_locale`; the boot and the locale switch: `catalog_url`, `document_locale`, `load_page_catalog`, `preload_locale`, `set_document_lang`, `set_locale` |
//! | `hydrate` | `hydrate_body`, `hydrate_lazy`, `hydrate_islands`, `wait_for_catalog`; [`islands_gate!`] expands to the islands' gate (with `ssr`, to nothing) |
//! | `csr` | `client_locale`, `load_client_catalog`, `mount_to_body` |
//!
//! and lack what only a server has: [`RequestI18n`], [`catalog`],
//! [`default_catalog`], [`provide_locale`], [`provide_locale_in_zone`],
//! [`request_time_zone`].
//!
//! # Why one crate
//!
//! `mf2` re-exports every call-site type — `mf2::Tr` is this `Tr`. The
//! implementation lives here because Rust's orphan rule leaves no choice: `impl Render for Tr` needs either the trait
//! or the type to be the implementing crate's, `Render` is tachys', so `Tr`
//! has to be ours. The same holds for `AttributeValue`, `IntoProperty`,
//! `From<Tr> for TextProp` and `From<Signal<T>> for ArgValue`. Splitting the
//! types from their rendering is not expressible; splitting them by feature
//! is, and that is what this crate does.
//!
//! An application names this crate beside `mf2`, for the mode and the
//! Leptos line; a build with no mode pulls in no Leptos crate, no tachys and
//! no `reactive_graph`.
//!
//! Client-path code: `no_std` + `alloc`
//! (the Leptos layer adds `std`, which its dependencies need anyway),
//! `forbid(unsafe_code)`, no `core::fmt`, no panicking operation.
//!
//! # The user guide
//!
//! Getting started, call sites, delivery modes, switching language,
//! accessibility, migrating from `leptos-fluent`, and what 1.x promises
//! See the [rust-mf2 book](https://chattyness.github.io/rust-mf2/) for the
//! ecosystem and application guides. An application starts at
//! [`mf2`](https://docs.rs/mf2).

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
// The Leptos layer's items name the modes an application turns on, not the
// `leptos` feature they imply (`doc(cfg(...))` below).
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

extern crate alloc;
#[cfg(feature = "leptos")]
extern crate std;

// An application renders on one side or the other, and the two need
// different code: `ssr` keeps the catalog in the request's context, the
// other two in a `thread_local!`. Saying so here turns a confusing cascade
// of missing-item errors into one sentence.
#[cfg(all(feature = "ssr", any(feature = "hydrate", feature = "csr")))]
compile_error!(
    "leptos-mf2: turn on exactly one of `ssr`, `hydrate` and `csr`. \
     cargo unifies features across a workspace, so an application that is \
     built both ways belongs in a workspace of its own — as \
     `examples/demo-ssr` and `conformance/l6-web` are."
);
#[cfg(all(feature = "hydrate", feature = "csr"))]
compile_error!("leptos-mf2: turn on exactly one of `ssr`, `hydrate` and `csr`.");

// The Leptos line (`plans/04-leptos-integration.md` §10): 0.9 by default,
// 0.8 as an opt-in whose crates are renamed back here, so that every `use
// leptos::…` / `tachys::…` / `reactive_graph::…` below — and every path the
// `view!` macro expands to — names whichever line is on.
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate leptos_0_8 as leptos;
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate reactive_graph_0_2 as reactive_graph;
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate tachys_0_2 as tachys;

#[cfg(all(feature = "leptos", feature = "leptos-0-8", feature = "leptos-0-9"))]
compile_error!(
    "leptos-mf2: `leptos-0-8` is on, and so is the default `leptos-0-9`. \
     For Leptos 0.8, every dependency on leptos-mf2 and mf2-axum needs \
     `default-features = false` beside `features = [\"leptos-0-8\"]`."
);
#[cfg(all(
    feature = "leptos",
    not(feature = "leptos-0-8"),
    not(feature = "leptos-0-9")
))]
compile_error!(
    "leptos-mf2: no Leptos line. Depend on leptos-mf2 with its default \
     features (Leptos 0.9), or turn on `leptos-0-8` for Leptos 0.8."
);

mod arg;
mod dynamic;
mod markup;
mod tr;

#[cfg(any(feature = "hydrate", feature = "csr"))]
mod boot;
#[cfg(feature = "leptos")]
mod catalog;
/// The page's own i18n furniture: the preload link, the catalog map, the
/// `hreflang` block and the reference locale switcher (§9).
#[cfg(feature = "leptos")]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub mod components;
#[cfg(feature = "leptos")]
mod convert;
#[cfg(feature = "leptos")]
mod error;
/// Everything that names tachys, in one module for every supported line.
#[cfg(feature = "leptos")]
#[doc(hidden)]
pub mod glue;
#[cfg(feature = "leptos")]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub use components::{
    AlternateLinks, CatalogLinks, CatalogPreload, IslandsGate, LocaleOption, LocaleSwitcher,
    html_lang,
};

/// The names the page carries, shared by the shell, the boot and
/// `mf2-axum`: the wire between them, not an application's.
#[cfg(feature = "leptos")]
#[doc(hidden)]
pub mod links;

#[cfg(all(feature = "leptos", feature = "mark-fallback-lang"))]
mod lang;

#[cfg(feature = "leptos")]
mod registry;
#[cfg(feature = "leptos")]
mod rich;
#[cfg(feature = "leptos")]
mod state;
#[cfg(feature = "leptos")]
mod text;
#[cfg(feature = "leptos")]
mod zone;

pub use arg::{ArgList, ArgSource, ArgValue, DateTimeValue, Text};
pub use dynamic::TrDyn;
/// What `tr!` and the generated module expand to (`mf2-macros`,
/// `mf2-build`'s codegen); never written by hand.
#[doc(hidden)]
pub use dynamic::tr_dyn;
#[doc(hidden)]
pub use markup::markup;
pub use markup::{Handler, IntoMarkupHandler};
pub use tr::{MarkupHandler, Tr, TrArgs, TrRich};
#[doc(hidden)]
pub use tr::{tr, tr_args_n, tr_args0, tr_args1, tr_args2, tr_args3, tr_args4, tr_rich};

/// The view closure a rich call site writes, and the flat handler
/// conformance L6 compares against `expParts`.
#[cfg(feature = "leptos")]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub use markup::{Flat, FlatHandler, NestingHandler};

#[cfg(feature = "leptos")]
mod signal;
#[cfg(feature = "leptos")]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub use signal::{SignalArg, signal_arg};

/// What an application installs once, and what it can ask about the build
/// (`plans/04-leptos-integration.md` §5).
#[cfg(feature = "leptos")]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub use state::{
    Setup, TextUse, dir_of, install, locales, lookup_locale, manifest_hash, setup, source_locale,
};
/// Whether [`install`] ran, and ran more than once: for the library's tests.
#[cfg(feature = "leptos")]
#[doc(hidden)]
pub use state::{installed, installed_twice};

/// The catalog a render reads, and what can go wrong loading one.
#[cfg(feature = "leptos")]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub use catalog::{active, read as read_catalog};
#[cfg(feature = "leptos")]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub use error::LoadError;

/// The table of catalogs the server serves, which `mf2-axum` installs and
/// reads.
#[cfg(feature = "ssr")]
#[doc(hidden)]
pub use catalog::{CatalogEntry, catalog_entries, catalog_file, catalog_name, install_catalogs};
/// The server's catalogs and per-request locale (§5, §6).
#[cfg(feature = "ssr")]
#[cfg_attr(docsrs, doc(cfg(feature = "ssr")))]
pub use catalog::{
    RequestI18n, catalog, default_catalog, provide_locale, provide_locale_in_zone,
    request_time_zone,
};

/// The reader's time zone (`plans/03-runtime.md` §6.1): whether a name the
/// browser or the `mf2_tz` cookie gave can be used.
#[cfg(feature = "leptos")]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub use zone::reader_time_zone;

/// The client's active catalog and its change notifier.
#[cfg(all(feature = "leptos", not(feature = "ssr")))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "hydrate", feature = "csr"))))]
pub use catalog::{changed, set_active, track_locale};

/// The client's boot and locale switch (§6).
#[cfg(any(feature = "hydrate", feature = "csr"))]
pub use boot::{
    catalog_url, document_locale, load_page_catalog, preload_locale, set_document_lang, set_locale,
};

/// Booting a client-only application (§8): the locale from storage, then
/// `navigator.languages`, then the default; the catalog from the index.
#[cfg(feature = "csr")]
pub use boot::{client_locale, load_client_catalog, mount_to_body};

/// Booting an application that hydrates (§6), in each of its three shapes:
/// a whole page, a page with code-split routes, and a page of islands (§8).
#[cfg(feature = "hydrate")]
pub use boot::{hydrate_body, hydrate_islands, hydrate_lazy, wait_for_catalog};

/// What [`islands_gate!`] expands to names these, so that an application
/// needs no direct dependency on them.
#[cfg(feature = "hydrate")]
#[doc(hidden)]
pub mod __private {
    pub use wasm_bindgen;
    pub use wasm_bindgen_futures;
    pub use web_sys;
}

/// Exports the island [`IslandsGate`] renders, in the application's crate
/// (this one forbids the `unsafe` a `#[wasm_bindgen]` export expands to).
/// Write it once, beside the `hydrate` entry point:
///
/// ```ignore
/// leptos_mf2::islands_gate!();
/// ```
///
/// It expands to nothing in a server build.
// rustfmt re-indents a `$crate` attribute inside a macro on every run.
#[cfg(feature = "hydrate")]
#[macro_export]
#[rustfmt::skip]
macro_rules! islands_gate {
    () => {
        #[$crate::__private::wasm_bindgen::prelude::wasm_bindgen(
            wasm_bindgen = $crate::__private::wasm_bindgen,
            wasm_bindgen_futures = $crate::__private::wasm_bindgen_futures,
            js_name = "mf2_islands_gate"
        )]
        #[doc(hidden)]
        pub async fn __mf2_islands_gate(_island: $crate::__private::web_sys::HtmlElement) {
            $crate::wait_for_catalog().await
        }
    };
}

/// Exports the island [`IslandsGate`] renders — in a client build. This is
/// a server build, where there is nothing to export.
#[cfg(not(feature = "hydrate"))]
// Not a feature's item: `islands_gate!` is always there (above, with `hydrate`).
#[cfg_attr(docsrs, doc(auto_cfg = false))]
#[macro_export]
macro_rules! islands_gate {
    () => {};
}

/// How many nodes follow the locale (D7's registry) — the number the browser
/// checks read to know that hydration finished and that dropped nodes freed
/// their slots.
#[cfg(feature = "leptos")]
#[doc(hidden)]
pub use registry::live_nodes;

/// What the three call-site types have in common, which is what the
/// rendering is written against.
#[cfg(feature = "leptos")]
#[doc(hidden)]
pub use text::{Description, Stored};

/// The retained view state of a rendered description.
#[cfg(feature = "leptos")]
#[doc(hidden)]
pub use glue::view::{TrAttrState, TrRichState, TrState};
