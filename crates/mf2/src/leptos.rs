//! The Leptos layer: how a description renders — in text, attributes and
//! props — together with the catalog of the request or of the page, the live
//! locale switch, `<html lang dir>`, the page's i18n components and the
//! reader's time zone for dates.
//!
//! # Modes and Leptos lines
//!
//! An application turns on exactly one mode — `ssr` in the server's build,
//! `hydrate` in the client's (`csr` for a client-only application) — and a
//! Leptos line: `leptos` (Leptos 0.9, the default line) or `leptos-0-8`. It
//! writes the line once, on its `mf2` dependency, and the mode where it
//! writes Leptos's own:
//!
//! ```toml
//! [dependencies]
//! mf2 = { version = "2", features = ["leptos"] }
//!
//! [features]
//! ssr = ["leptos/ssr", "mf2/ssr"]
//! hydrate = ["leptos/hydrate", "mf2/hydrate"]
//! ```
//!
//! The modes exclude each other, so this documentation shows one: **`ssr`,
//! on Leptos 0.9**. The client modes add:
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
//! `static-locale` makes a switch a navigation (for islands), and
//! `mark-fallback-lang` marks text borrowed from a fallback language with
//! its own `lang`.
//!
//! # Importing from here
//!
//! Import items from `mf2::leptos` (`use mf2::leptos::{LocaleSwitcher,
//! LocaleOption};`) or write their full paths. Never `use mf2::leptos;` in a
//! module that also uses the `leptos` crate: the module's name would then
//! shadow the crate's in every bare path of that module.
//!
//! Client-path code: `forbid(unsafe_code)`, no `core::fmt`, no panicking
//! operation.

// Where the catalog lives, and how a description is formatted from it.
pub(crate) mod catalog;
pub(crate) mod state;
pub(crate) mod text;

// The client's boot and switch.
#[cfg(any(feature = "hydrate", feature = "csr"))]
pub(crate) mod boot;

pub mod components;
pub(crate) mod convert;
/// Everything that names tachys, in one module for every supported line.
#[doc(hidden)]
pub mod glue;
#[cfg(feature = "mark-fallback-lang")]
pub(crate) mod lang;
/// The names the page carries, shared by the shell, the boot and
/// `mf2-axum`: the wire between them, not an application's.
#[doc(hidden)]
pub mod links;
pub(crate) mod markup;
pub(crate) mod registry;
pub(crate) mod rich;
pub(crate) mod signal;
pub(crate) mod zone;

pub use components::{
    AlternateLinks, AlternateLinksProps, CatalogLinks, CatalogLinksProps, CatalogPreload,
    CatalogPreloadProps, IslandsGate, IslandsGateProps, LocaleOption, LocaleOptionProps,
    LocaleSwitcher, LocaleSwitcherProps, LocaleTag, html_lang,
};

/// The view closure a rich call site writes, and the flat handler
/// conformance L6 compares against `expParts`.
pub use markup::{Flat, FlatHandler, NestingHandler};

/// A signal as an argument, read when the message formats.
pub use signal::{SignalArg, signal_arg};

/// [`lookup_locale`] over a reader's list: what `mf2-axum` negotiates each
/// source's candidates with.
#[doc(hidden)]
pub use state::best_locale;
/// What an application installs once, and what it can ask about the build.
pub use state::{
    Setup, TextUse, dir_of, install, locales, lookup_locale, manifest_hash, setup, source_locale,
};
/// Whether [`install`] ran, and ran more than once: for the library's tests.
#[doc(hidden)]
pub use state::{installed, installed_twice};

pub use crate::error::LoadError;
/// The catalog a render reads, and what can go wrong loading one.
pub use catalog::{active, read as read_catalog};

/// The table of catalogs the server serves, which `mf2-axum` installs and
/// reads.
#[cfg(feature = "ssr")]
#[doc(hidden)]
pub use catalog::{
    CatalogEntry, catalog_entries, catalog_file, catalog_name, install_catalogs,
    provide_locale_query,
};
/// The server's catalogs and per-request locale.
#[cfg(feature = "ssr")]
#[cfg_attr(docsrs, doc(cfg(feature = "ssr")))]
pub use catalog::{
    RequestI18n, catalog, default_catalog, provide_locale, provide_locale_in_zone,
    request_time_zone,
};

/// The reader's time zone: whether a name the browser or the `mf2_tz`
/// cookie gave can be used.
pub use zone::reader_time_zone;

/// The client's active catalog and its change notifier.
#[cfg(not(feature = "ssr"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "hydrate", feature = "csr"))))]
pub use catalog::{changed, set_active, track_locale};

/// The client's boot and locale switch.
#[cfg(any(feature = "hydrate", feature = "csr"))]
pub use boot::{
    catalog_url, document_locale, load_page_catalog, preload_locale, set_document_lang, set_locale,
};

/// Booting a client-only application: the locale from storage, then
/// `navigator.languages`, then the default; the catalog from the index.
#[cfg(feature = "csr")]
pub use boot::{client_locale, load_client_catalog, mount_to_body};

/// Booting an application that hydrates, in each of its three shapes: a
/// whole page, a page with code-split routes, and a page of islands.
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
/// mf2::leptos::islands_gate!();
/// ```
///
/// It expands to nothing in a server build.
pub use crate::__islands_gate as islands_gate;

/// How many nodes follow the locale — the number the browser checks read to
/// know that hydration finished and that dropped nodes freed their slots.
#[doc(hidden)]
pub use registry::live_nodes;

/// What the four call-site types have in common, which is what the
/// rendering is written against.
#[doc(hidden)]
pub use text::{Description, Stored};

/// The retained view state of a rendered description.
#[doc(hidden)]
pub use glue::view::{TrAttrState, TrRichState, TrState};
