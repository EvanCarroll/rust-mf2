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

mod arg;
mod dynamic;
mod markup;
mod tr;

#[cfg(any(feature = "hydrate", feature = "csr"))]
mod boot;
#[cfg(feature = "leptos")]
mod catalog;
#[cfg(feature = "leptos")]
mod convert;
#[cfg(feature = "leptos")]
mod error;
/// Everything that names tachys, one module per supported tachys line.
#[cfg(feature = "leptos")]
pub mod glue;
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
pub use markup::markup;
pub use tr::{
    MarkupHandler, Tr, TrArgs, TrRich, tr, tr_args_n, tr_args0, tr_args1, tr_args2, tr_args3,
    tr_args4, tr_rich,
};

/// The view closure a rich call site writes, and the flat handler
/// conformance L6 compares against `expParts`.
#[cfg(feature = "leptos")]
pub use markup::{Flat, FlatHandler, IntoMarkupHandler, NestingHandler};

#[cfg(feature = "leptos")]
mod signal;
#[cfg(feature = "leptos")]
pub use signal::{SignalArg, signal_arg};

/// What an application installs once, and what it can ask about the build
/// (`plans/04-leptos-integration.md` §5).
#[cfg(feature = "leptos")]
pub use state::{
    Setup, TextUse, dir_of, install, installed, installed_twice, locales, manifest_hash, setup,
    source_locale,
};

/// The catalog a render reads, and what can go wrong loading one.
#[cfg(feature = "leptos")]
pub use catalog::{active, read as read_catalog};
#[cfg(feature = "leptos")]
pub use error::LoadError;

/// The server's catalogs and per-request locale (§5, §6).
#[cfg(feature = "ssr")]
pub use catalog::{
    RequestCatalog, catalog, catalog_locales, catalog_name, default_catalog, install_catalogs,
    provide_locale,
};

/// The client's active catalog and its change notifier.
#[cfg(all(feature = "leptos", not(feature = "ssr")))]
pub use catalog::{changed, set_active};

/// The client's boot and locale switch (§6).
#[cfg(any(feature = "hydrate", feature = "csr"))]
pub use boot::{
    CATALOG_LINK_LOCALE_ATTR, CATALOG_LINK_REL, CATALOG_ROUTE, catalog_url, document_locale,
    load_page_catalog, preload_locale, set_document_lang, set_locale,
};

/// Booting an application that hydrates (§6).
#[cfg(feature = "hydrate")]
pub use boot::{hydrate_body, hydrate_lazy};

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
