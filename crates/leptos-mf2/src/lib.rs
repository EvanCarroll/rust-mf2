//! `leptos-mf2` — what a `tr!` call site builds, and how Leptos renders it
//! (`plans/04-leptos-integration.md`).
//!
//! The crate has two halves, and the `leptos` feature is the seam:
//!
//! * **the call-site core** (always): [`Tr`], [`TrArgs`], [`TrRich`],
//!   [`TrDyn`], [`ArgValue`] and the lowering that borrows them into the
//!   runtime's `Arg` at format time (§2.1). It formats against a
//!   [`Formatter`](mf2_runtime::Formatter) the caller supplies, so a server,
//!   a test and `mf2-cli` use it with no Leptos code compiled at all;
//! * **the Leptos layer** (`leptos`): the tachys view impls, the per-request
//!   and per-client catalog, the node registry and the locale switch
//!   (§§3–7).
//!
//! **Why one crate.** [04](plans/04-leptos-integration.md) §2.1 puts the
//! core in the facade, and `mf2` still re-exports every one of these types —
//! `mf2::Tr` is this `Tr`. The implementation lives here because Rust's
//! orphan rule leaves no choice: `impl Render for Tr` needs either the trait
//! or the type to be the implementing crate's, `Render` is tachys', so `Tr`
//! has to be ours. The same holds for `AttributeValue`, `IntoProperty`,
//! `From<Tr> for TextProp` and `From<Signal<T>> for ArgValue`. Splitting the
//! types from their rendering is not expressible; splitting them by feature
//! is, and that is what this crate does.
//!
//! An application still names one crate — `mf2`, with `features =
//! ["leptos", …]` — and a build without that feature pulls in no Leptos
//! crate, no tachys and no `reactive_graph`.
//!
//! Client-path code (`plans/06-size-and-perf.md`, B12): `no_std` + `alloc`
//! (the Leptos layer adds `std`, which its dependencies need anyway),
//! `forbid(unsafe_code)`, no `core::fmt`, no panicking operation.

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
pub mod components;
#[cfg(feature = "leptos")]
mod convert;
#[cfg(feature = "leptos")]
mod error;
/// Everything that names tachys, one module per supported tachys line.
#[cfg(feature = "leptos")]
pub mod glue;
#[cfg(feature = "leptos")]
pub use components::{
    AlternateLinks, CatalogLinks, CatalogPreload, IslandsGate, LocaleOption, LocaleSwitcher,
    html_lang,
};

/// The names the page carries, shared by the shell and the boot.
#[cfg(feature = "leptos")]
pub mod links;

#[cfg(feature = "leptos")]
mod registry;
#[cfg(feature = "leptos")]
mod rich;
#[cfg(feature = "leptos")]
mod state;
#[cfg(feature = "leptos")]
mod text;

pub use arg::{ArgList, ArgSource, ArgValue, DateTimeValue, Text};
pub use dynamic::{TrDyn, tr_dyn};
pub use markup::{Handler, IntoMarkupHandler, markup};
pub use tr::{
    MarkupHandler, Tr, TrArgs, TrRich, tr, tr_args_n, tr_args0, tr_args1, tr_args2, tr_args3,
    tr_args4, tr_rich,
};

/// The view closure a rich call site writes, and the flat handler
/// conformance L6 compares against `expParts`.
#[cfg(feature = "leptos")]
pub use markup::{Flat, FlatHandler, NestingHandler};

#[cfg(feature = "leptos")]
mod signal;
#[cfg(feature = "leptos")]
pub use signal::{SignalArg, signal_arg};

/// What an application installs once, and what it can ask about the build
/// (`plans/04-leptos-integration.md` §5).
#[cfg(feature = "leptos")]
pub use state::{
    Setup, TextUse, dir_of, install, installed, installed_twice, locales, lookup_locale,
    manifest_hash, setup, source_locale,
};

/// The catalog a render reads, and what can go wrong loading one.
#[cfg(feature = "leptos")]
pub use catalog::{active, read as read_catalog};
#[cfg(feature = "leptos")]
pub use error::LoadError;

/// The server's catalogs and per-request locale (§5, §6).
#[cfg(feature = "ssr")]
pub use catalog::{
    CatalogEntry, RequestI18n, catalog, catalog_entries, catalog_file, catalog_name,
    default_catalog, install_catalogs, provide_locale,
};

/// The client's active catalog and its change notifier.
#[cfg(all(feature = "leptos", not(feature = "ssr")))]
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
#[macro_export]
macro_rules! islands_gate {
    () => {};
}

/// How many nodes follow the locale (D7's registry) — the number the browser
/// checks read to know that hydration finished and that dropped nodes freed
/// their slots.
#[cfg(feature = "leptos")]
pub use registry::live_nodes;

/// What the three call-site types have in common, which is what the
/// rendering is written against.
#[cfg(feature = "leptos")]
pub use text::{Description, Stored};

/// The retained view state of a rendered description.
#[cfg(feature = "leptos")]
pub use glue::tachys_0_2::{TrAttrState, TrRichState, TrState};
