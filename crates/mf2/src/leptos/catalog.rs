//! Where the catalog lives.
//!
//! | Target | Storage | Why |
//! |---|---|---|
//! | client | a `thread_local!` active catalog and one change trigger | no context walk per lookup, and it is shared across lazy chunks and islands (P0.2) |
//! | server | a per-request context, from a process-wide store | requests share threads, so a global would be wrong |
//!
//! **A lookup never panics and never demands a context.** With no request
//! context — route-list generation runs the app with mock parts, the file
//! handler in a bare owner (P0.2) — the source locale's catalog is used. With
//! no catalog at all, the caller renders empty text. That is the failure mode
//! the prior-art audit found most expensive (§11): `expect_context` on a path
//! a call site can reach.

use alloc::sync::Arc;
use alloc::vec::Vec;

use mf2_catalog::Catalog;

use crate::error::LoadError;
use crate::leptos::state;
#[cfg(feature = "ssr")]
use crate::line::reactive_graph;

/// Reads `bytes` as a catalog of **this** build: the manifest hash is
/// checked before anything else, so a catalog from another deploy is
/// rejected rather than misread (F6).
pub fn read(bytes: Vec<u8>) -> Result<Arc<Catalog>, LoadError> {
    if !state::installed() {
        return Err(LoadError::NotInstalled);
    }
    Catalog::new(bytes, state::manifest_hash())
        .map(Arc::new)
        .map_err(LoadError::from_reader)
}

// ---------------------------------------------------------------- server ---

/// The server's process-wide catalogs: parsed once at startup from the
/// generated `CATALOGS`, then shared by every request.
#[cfg(feature = "ssr")]
mod store {
    use super::{Arc, Catalog, LoadError, Vec, read, state};
    use std::sync::OnceLock;

    /// One locale, as the server holds it: what to serve, under what name,
    /// and what to render with.
    pub struct CatalogEntry {
        /// The BCP 47 tag.
        pub tag: &'static str,
        /// The content-hashed file name the build published it under.
        pub file: &'static str,
        /// The bytes, for serving — the same ones the client validates.
        pub bytes: &'static [u8],
        /// The parsed catalog, shared by every request in this locale.
        pub catalog: Arc<Catalog>,
    }

    impl core::fmt::Debug for CatalogEntry {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("CatalogEntry")
                .field("tag", &self.tag)
                .field("file", &self.file)
                .field("bytes", &self.bytes.len())
                .finish_non_exhaustive()
        }
    }

    static STORE: OnceLock<Vec<CatalogEntry>> = OnceLock::new();

    /// Parses and stores the generated `CATALOGS` —
    /// `(tag, file name, bytes)`. Call it once, after
    /// [`install`](crate::leptos::install).
    ///
    /// Every catalog is checked against `MANIFEST_HASH` here, at startup,
    /// so a skewed deploy fails the server's boot rather than a request.
    pub fn install_catalogs(
        catalogs: &[(&'static str, &'static str, &'static [u8])],
    ) -> Result<(), LoadError> {
        let mut parsed = Vec::with_capacity(catalogs.len());
        for (tag, file, bytes) in catalogs {
            parsed.push(CatalogEntry {
                tag,
                file,
                bytes,
                catalog: read(bytes.to_vec())?,
            });
        }
        let _ = STORE.set(parsed);
        Ok(())
    }

    /// The catalog of `tag`.
    #[must_use]
    pub fn catalog(tag: &str) -> Option<Arc<Catalog>> {
        let store = STORE.get()?;
        store
            .iter()
            .find(|e| e.tag == tag)
            .map(|e| Arc::clone(&e.catalog))
    }

    /// The immutable file name `tag`'s catalog is published under — the
    /// preload link's URL, and what `/i18n/<tag>` resolves to.
    #[must_use]
    pub fn catalog_name(tag: &str) -> Option<&'static str> {
        STORE.get()?.iter().find(|e| e.tag == tag).map(|e| e.file)
    }

    /// The bytes published under `file`, for a server answering
    /// `/i18n/<file>`.
    #[must_use]
    pub fn catalog_file(file: &str) -> Option<&'static [u8]> {
        STORE
            .get()?
            .iter()
            .find(|e| e.file == file)
            .map(|e| e.bytes)
    }

    /// Every locale that has a catalog, in the order the build wrote them.
    #[must_use]
    pub fn catalog_entries() -> &'static [CatalogEntry] {
        STORE.get().map_or(&[], Vec::as_slice)
    }

    /// The source locale's catalog: what a render with no request context
    /// falls back to.
    #[must_use]
    pub fn default_catalog() -> Option<Arc<Catalog>> {
        catalog(state::source_locale())
    }
}

#[cfg(feature = "ssr")]
pub use store::{
    CatalogEntry, catalog, catalog_entries, catalog_file, catalog_name, default_catalog,
    install_catalogs,
};

/// What one request formats with, in its `Owner`.
///
/// The catalog is the whole of it for an ordinary application, and
/// `mf2-axum` provides that from `additional_context` — it must be passed to
/// **every** `leptos_axum` `_with_context` entry point. The other two
/// exist because something does need them:
///
/// * `registry` — a process that serves two applications, and conformance
///   L6, which formats the same suite with the full registry and again with
///   the default one;
/// * `bidi` — the suite's per-test `bidiIsolation`, and an application that
///   wants a subtree rendered without the isolating marks.
///
/// The request's **time zone** is the reader's, when the `mf2_tz` cookie
/// named one this server knows; `mf2-axum` sets it.
#[cfg(feature = "ssr")]
#[derive(Clone, Debug)]
pub struct RequestI18n {
    catalog: Arc<Catalog>,
    registry: Option<&'static mf2_runtime::Registry>,
    bidi: Option<mf2_runtime::BidiStrategy>,
    time_zone: Option<mf2_runtime::TimeZone>,
}

#[cfg(feature = "ssr")]
impl RequestI18n {
    /// This request renders `catalog`, with everything else as installed.
    #[must_use]
    pub fn new(catalog: Arc<Catalog>) -> RequestI18n {
        RequestI18n {
            catalog,
            registry: None,
            bidi: None,
            time_zone: None,
        }
    }

    /// …with another registry than the installed one.
    #[must_use]
    pub fn with_registry(mut self, registry: &'static mf2_runtime::Registry) -> RequestI18n {
        self.registry = Some(registry);
        self
    }

    /// …with another bidi strategy for *displayed* text. Plain positions
    /// (`prop:value`, an attribute a program reads such as `value=` or
    /// `data-*`, `to_plain_string()`) are unaffected: they are never isolated.
    #[must_use]
    pub fn with_bidi(mut self, bidi: mf2_runtime::BidiStrategy) -> RequestI18n {
        self.bidi = Some(bidi);
        self
    }

    /// …in the reader's time zone, `zone` — which
    /// [`reader_time_zone`](crate::leptos::reader_time_zone) accepted. It outranks
    /// `Setup::with_time_zone`, and a zone the message names outranks it.
    #[must_use]
    pub fn with_time_zone(mut self, zone: mf2_runtime::TimeZone) -> RequestI18n {
        self.time_zone = Some(zone);
        self
    }

    /// Installs it for this request.
    pub fn provide(self) {
        reactive_graph::owner::provide_context(self);
    }
}

/// Provides the catalog of `tag` for this request; the source locale's if
/// `tag` was not built. Returns the tag actually used.
#[cfg(feature = "ssr")]
pub fn provide_locale(tag: &str) -> &'static str {
    provide_locale_in_zone(tag, None)
}

/// [`provide_locale`], with the reader's time zone when it is known — what
/// `mf2-axum` calls with the `mf2_tz` cookie's zone.
#[cfg(feature = "ssr")]
pub fn provide_locale_in_zone(tag: &str, zone: Option<mf2_runtime::TimeZone>) -> &'static str {
    let found = state::locales()
        .iter()
        .find(|(t, _)| *t == tag)
        .map(|(t, _)| *t);
    let tag = found.unwrap_or_else(state::source_locale);
    if let Some(c) = catalog(tag) {
        let request = RequestI18n::new(c);
        match zone {
            Some(zone) => request.with_time_zone(zone),
            None => request,
        }
        .provide();
    }
    tag
}

/// The reader's time zone this request renders in, if it knows one — what
/// the page states on its preload link.
#[cfg(feature = "ssr")]
#[must_use]
pub fn request_time_zone() -> Option<mf2_runtime::TimeZone> {
    current().and_then(|c| c.time_zone)
}

/// The catalog this render reads, on the server: the request's, else the
/// source locale's.
#[cfg(feature = "ssr")]
#[must_use]
pub fn active() -> Option<Arc<Catalog>> {
    current().map(|c| c.catalog)
}

/// Everything one format needs, resolved in **one** context lookup: doing it
/// per field would walk the owner chain twice for every rendered node.
#[cfg(feature = "ssr")]
pub(crate) fn current() -> Option<RequestI18n> {
    match reactive_graph::owner::use_context::<RequestI18n>() {
        Some(cx) => Some(cx),
        None => default_catalog().map(RequestI18n::new),
    }
}

/// The same on the client, where the catalog is a `thread_local!` and there
/// is nothing to override: one application, one registry, one page.
#[cfg(not(feature = "ssr"))]
pub(crate) fn current() -> Option<Resolved> {
    active().map(|catalog| Resolved { catalog })
}

/// The request's own catalog, with no fallback to the source locale's: the
/// first step of the one lookup when `native` is on beside `ssr`
/// (plans/19-native-and-terminal.md §5), whose next steps are the native
/// store's.
#[cfg(all(feature = "ssr", feature = "native"))]
pub(crate) fn requested() -> Option<RequestI18n> {
    reactive_graph::owner::use_context::<RequestI18n>()
}

/// On the client, the page's catalog, which has no fallback anyway.
#[cfg(all(not(feature = "ssr"), feature = "native"))]
pub(crate) fn requested() -> Option<Resolved> {
    current()
}

/// The client's resolved state.
#[cfg(not(feature = "ssr"))]
pub(crate) struct Resolved {
    catalog: Arc<Catalog>,
}

#[cfg(not(feature = "ssr"))]
impl Resolved {
    pub(crate) fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Always the installed one: a page is one application, so there is
    /// nothing to override. The shape mirrors the server's so that the
    /// formatting path is one piece of code rather than two.
    #[allow(clippy::unused_self)]
    pub(crate) fn registry(&self) -> Option<&'static mf2_runtime::Registry> {
        None
    }

    /// Likewise: a per-request bidi override is a server's business (the
    /// suite's `bidiIsolation`, a subtree rendered plain). On the client the
    /// position decides — `prop:value`, an attribute a program reads by its
    /// name, `to_plain_string()` — §9.
    #[allow(clippy::unused_self)]
    pub(crate) fn bidi(&self) -> Option<mf2_runtime::BidiStrategy> {
        None
    }

    /// Likewise: the client's zone is the thread's (`crate::leptos::zone`), set at
    /// boot, so there is nothing to scope.
    #[allow(clippy::unused_self)]
    pub(crate) fn in_zone<R>(&self, body: impl FnOnce() -> R) -> R {
        body()
    }
}

#[cfg(feature = "ssr")]
impl RequestI18n {
    pub(crate) fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub(crate) fn registry(&self) -> Option<&'static mf2_runtime::Registry> {
        self.registry
    }

    pub(crate) fn bidi(&self) -> Option<mf2_runtime::BidiStrategy> {
        self.bidi
    }

    /// Runs `body` in this request's time zone (`crate::leptos::zone`).
    pub(crate) fn in_zone<R>(&self, body: impl FnOnce() -> R) -> R {
        crate::leptos::zone::scoped(self.time_zone, body)
    }
}

// ---------------------------------------------------------------- client ---

#[cfg(not(feature = "ssr"))]
mod client {
    use super::{Arc, Catalog};
    use crate::line::reactive_graph;
    use core::cell::RefCell;
    use reactive_graph::signal::ArcTrigger;

    std::thread_local! {
        /// The one active catalog. A `thread_local!` rather than a context:
        /// a context lookup costs a lock and a hash lookup per owner level,
        /// and every effect adds a level (§5).
        static ACTIVE: RefCell<Option<Arc<Catalog>>> = const { RefCell::new(None) };
        /// Fires when the active catalog changes. The node registry does not
        /// need it — `set_locale` walks the slab itself — but the derived
        /// conversions (`TextProp`, `Signal<String>`) do.
        static CHANGED: ArcTrigger = ArcTrigger::new();
    }

    /// The active catalog.
    #[must_use]
    pub fn active() -> Option<Arc<Catalog>> {
        ACTIVE.with(|a| a.borrow().clone())
    }

    /// Installs `catalog` as the active one, without notifying: the boot
    /// path, before anything has rendered.
    pub fn set_active(catalog: Arc<Catalog>) {
        ACTIVE.with(|a| *a.borrow_mut() = Some(catalog));
    }

    /// The trigger, cloned, for a caller that wants to subscribe — prefer
    /// [`track_locale`], which also unsubscribes.
    #[must_use]
    pub fn changed() -> ArcTrigger {
        CHANGED.with(Clone::clone)
    }

    /// Subscribes the running observer — the effect or memo reading a
    /// description — to the locale change, **until its owner is next cleaned
    /// up**: when it re-runs (and subscribes again) or is disposed.
    ///
    /// Tracking [`changed`] alone leaks: a dropped effect stays in the
    /// trigger's subscriber set until the trigger next fires, and a locale
    /// switch is rare, so an effect in a churning list — a row taking a
    /// `TextProp` — left ≈ 70 B behind per row on wasm32 (measured by
    /// `bench/churn`). The owner's cleanup takes it out instead.
    /// Every `reactive_graph` observer runs under an owner of its own, which
    /// is cleaned before each re-run and when it is dropped. With no owner
    /// nothing can clean up, and this is plain tracking.
    ///
    /// Outside an observer it does nothing (an event handler must not
    /// warn).
    pub fn track_locale() {
        use reactive_graph::graph::{Observer, Source};
        use reactive_graph::owner::Owner;
        use reactive_graph::traits::Track;

        let Some(observer) = Observer::get() else {
            return;
        };
        let trigger = changed();
        trigger.track();
        Owner::on_cleanup(move || trigger.remove_subscriber(&observer));
    }
}

#[cfg(not(feature = "ssr"))]
pub use client::{active, changed, set_active, track_locale};
