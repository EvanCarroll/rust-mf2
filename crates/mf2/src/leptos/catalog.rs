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
    use super::{Arc, Catalog, LoadError, Vec, state};
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
    /// `(tag, file name, bytes, server-only table)`. Call it once, after
    /// [`install`](crate::leptos::install).
    ///
    /// Every catalog is checked against `MANIFEST_HASH` here, at startup,
    /// so a skewed deploy fails the server's boot rather than a request.
    /// The server-only table (`plan/08` §4.2) holds the LOCALE entries only
    /// the server reads, which the browser's catalog leaves out; it is
    /// served to no one, and empty when there is none.
    pub fn install_catalogs(
        catalogs: &[(&'static str, &'static str, &'static [u8], &'static [u8])],
    ) -> Result<(), LoadError> {
        let mut parsed = Vec::with_capacity(catalogs.len());
        for (tag, file, bytes, server) in catalogs {
            // As `read`, with the table: kept apart so that the client's
            // `read` does not move.
            if !state::installed() {
                return Err(LoadError::NotInstalled);
            }
            let catalog = Catalog::new(bytes.to_vec(), state::manifest_hash())
                .and_then(|catalog| catalog.with_server_data(server))
                .map(Arc::new)
                .map_err(LoadError::from_reader)?;
            parsed.push(CatalogEntry {
                tag,
                file,
                bytes,
                catalog,
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
/// The catalog is the whole of it for an ordinary application, and with
/// `axum` the render provides it itself, from what the `Negotiator` layer
/// put in the request's extensions. The other two exist because something
/// does need them:
///
/// * `registry` — a process that serves two applications, and conformance
///   L6, which formats the same suite with the full registry and again with
///   the default one;
/// * `bidi` — the suite's per-test `bidiIsolation`, and an application that
///   wants a subtree rendered without the isolating marks.
///
/// The request's **time zone** is the reader's, when the `mf2_tz` cookie
/// named one this server knows; the `Negotiator` layer's render sets it.
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
/// the render calls with the `mf2_tz` cookie's zone.
#[cfg(feature = "ssr")]
pub fn provide_locale_in_zone(tag: &str, zone: Option<mf2_runtime::TimeZone>) -> &'static str {
    let found = state::locales()
        .iter()
        .find(|(t, _)| *t == tag)
        .map(|(t, _)| *t);
    if found.is_none()
        && let Some(named) = crate::warn::tag(tag)
    {
        crate::warn::once_for(crate::warn::Kind::UnknownLocale, named, || {
            alloc::format!(
                "mf2: provide_locale(\"{named}\"): this build has no catalog for that language, \
                 so the page renders in the source language, `{}`",
                state::source_locale()
            )
        });
    }
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

/// The query parameter the server's negotiator reads (its `QueryParam`'s
/// name), for this request: the name `<LocaleSwitcher>`'s `<select>`
/// submits under. The render provides it with the locale.
#[cfg(feature = "ssr")]
#[derive(Clone, Copy, Debug)]
struct LocaleQuery(&'static str);

/// Provides the query parameter the switcher submits for this request.
#[cfg(feature = "ssr")]
pub fn provide_locale_query(name: &'static str) {
    reactive_graph::owner::provide_context(LocaleQuery(name));
}

/// The query parameter the switcher submits: the installed query source's
/// on the server, else [`LOCALE_QUERY`](crate::links::LOCALE_QUERY). On the
/// client the attribute is the server's (hydration keeps a static
/// attribute), and a live switch does not submit the form.
pub(crate) fn locale_query() -> &'static str {
    #[cfg(feature = "ssr")]
    if let Some(LocaleQuery(name)) = reactive_graph::owner::use_context::<LocaleQuery>() {
        return name;
    }
    #[cfg(all(feature = "ssr", feature = "axum"))]
    if let Some(name) = reactive_graph::owner::with_context::<::http::request::Parts, _>(|parts| {
        parts
            .extensions
            .get::<crate::axum::RequestLocale>()
            .and_then(crate::axum::RequestLocale::query)
    })
    .flatten()
    {
        return name;
    }
    crate::links::LOCALE_QUERY
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
    if let Some(cx) = request() {
        return Some(cx);
    }
    let fallback = default_catalog().map(RequestI18n::new);
    unrequested(fallback.is_some());
    fallback
}

/// Says once on the server what a render with no language for its request
/// falls back to (E4): the source language's text, or, with no catalogs
/// installed, empty text.
///
/// With `axum`, only inside a request Axum's `Router` served: its `Parts`
/// carry the `OriginalUri` the router inserts into every request, routes
/// and fallback alike, before any handler runs. The `Parts` `leptos_axum`
/// makes up for the route list it builds at start-up (and for static
/// routes) come from a bare request, with none: no request, no language.
#[cfg(feature = "ssr")]
fn unrequested(source_language: bool) {
    use crate::warn::{Kind, once, pending};
    let kind = if source_language {
        Kind::Unrequested
    } else {
        Kind::NoCatalogs
    };
    if !pending(kind) {
        return;
    }
    #[cfg(feature = "axum")]
    if source_language && !served() {
        return;
    }
    once(kind, || {
        if source_language {
            alloc::format!(
                "mf2: a page rendered without the request's language, so it is in the source \
                 language, `{}`; add mf2::axum's Negotiator layer to the router, or call \
                 provide_locale in the render",
                state::source_locale()
            )
        } else {
            alloc::string::String::from(
                "mf2: a message was formatted with no catalogs installed, so it rendered as \
                 empty text; call the generated install() at start-up",
            )
        }
    });
}

/// Whether this render is inside a request Axum's `Router` served (see
/// [`unrequested`]).
#[cfg(all(feature = "ssr", feature = "axum"))]
fn served() -> bool {
    reactive_graph::owner::with_context::<::http::request::Parts, _>(|parts| {
        parts
            .extensions
            .get::<::axum::extract::OriginalUri>()
            .is_some()
    })
    .unwrap_or(false)
}

/// This request's own catalog: the one provided, else (with `axum`) the one
/// the `Negotiator` layer chose, found through the request `Parts`
/// `leptos_axum` provides and then provided here, so that the next lookup
/// under this owner finds it at once.
#[cfg(feature = "ssr")]
fn request() -> Option<RequestI18n> {
    if let Some(cx) = reactive_graph::owner::use_context::<RequestI18n>() {
        return Some(cx);
    }
    #[cfg(feature = "axum")]
    if let Some((tag, zone, query)) = from_layer() {
        if let Some(name) = query {
            provide_locale_query(name);
        }
        provide_locale_in_zone(tag, zone);
        return reactive_graph::owner::use_context::<RequestI18n>();
    }
    None
}

/// What the `Negotiator` layer put in the request's extensions: the tag, the
/// reader's time zone (the `mf2_tz` cookie, when this server knows it) and
/// the query source's name. `None` with no request or no layer.
#[cfg(all(feature = "ssr", feature = "axum"))]
fn from_layer() -> Option<(
    &'static str,
    Option<mf2_runtime::TimeZone>,
    Option<&'static str>,
)> {
    reactive_graph::owner::with_context::<::http::request::Parts, _>(|parts| {
        let request = parts.extensions.get::<crate::axum::RequestLocale>()?;
        let tag = request.read().tag;
        let zone = crate::axum::negotiate_cookie(parts, crate::leptos::links::TIME_ZONE_COOKIE)
            .and_then(crate::leptos::reader_time_zone);
        if zone.is_some() {
            request.zoned();
        }
        Some((tag, zone, request.query()))
    })
    .flatten()
}

/// The same on the client, where the catalog is a `thread_local!` and there
/// is nothing to override: one application, one registry, one page.
///
/// With none active the text is empty; a debug build says so once in the
/// browser's console (E4). A release build has no such code: its client
/// wasm is unchanged.
#[cfg(not(feature = "ssr"))]
pub(crate) fn current() -> Option<Resolved> {
    let found = active().map(|catalog| Resolved { catalog });
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    if found.is_none() {
        no_catalog_warning();
    }
    found
}

/// The console warning, once per page, of a message formatted before any
/// catalog was active.
#[cfg(all(not(feature = "ssr"), debug_assertions, target_arch = "wasm32"))]
fn no_catalog_warning() {
    use core::sync::atomic::{AtomicBool, Ordering};
    static GIVEN: AtomicBool = AtomicBool::new(false);
    if !GIVEN.swap(true, Ordering::Relaxed) {
        web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(
            "mf2: a message was formatted before any catalog was active, so it rendered as \
             empty text; start the page through mf2::leptos (hydrate_body, mount_to_body), \
             which loads the catalog first. (Debug builds only.)",
        ));
    }
}

/// The request's own catalog, with no fallback to the source locale's: the
/// first step of the one lookup when `native` is on beside `ssr`
/// (plans/19-native-and-terminal.md §5), whose next steps are the native
/// store's.
#[cfg(all(feature = "ssr", feature = "native"))]
pub(crate) fn requested() -> Option<RequestI18n> {
    request()
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

#[cfg(all(test, feature = "ssr"))]
#[allow(clippy::expect_used, reason = "a test")]
mod tests {
    use crate::warn::{Kind, given};

    #[test]
    fn a_render_with_no_catalogs_says_so_once() {
        // No unit test installs catalogs: every render falls back to none.
        assert!(super::default_catalog().is_none());
        for _ in 0..3 {
            assert!(super::current().is_none());
        }
        assert_eq!(given(Kind::NoCatalogs).len(), 1);
    }

    #[test]
    fn an_unknown_language_provided_is_named_once() {
        for _ in 0..3 {
            let _ = super::provide_locale("tlh-test");
        }
        let lines = given(Kind::UnknownLocale);
        assert_eq!(
            lines.iter().filter(|l| l.contains("\"tlh-test\"")).count(),
            1
        );
    }

    #[test]
    fn a_request_rendered_without_its_language_says_so_once() {
        // The warning is once per process, so the cases run in order in one
        // test: the quiet ones first, then the one that warns.
        #[cfg(all(feature = "axum", feature = "leptos"))]
        through_leptos_axum();
        #[cfg(not(all(feature = "axum", feature = "leptos")))]
        {
            use crate::line::reactive_graph::owner;
            owner::Owner::new().with(|| {
                #[cfg(feature = "axum")]
                owner::provide_context(served_parts());
                for _ in 0..3 {
                    super::unrequested(true);
                }
            });
        }
        let lines = given(Kind::Unrequested);
        assert_eq!(lines.len(), 1);
        assert!(
            lines
                .iter()
                .all(|l| l.contains("without the request's language"))
        );
    }

    /// A request's `Parts` as Axum's router hands them on.
    #[cfg(all(feature = "axum", not(feature = "leptos")))]
    fn served_parts() -> ::http::request::Parts {
        let mut parts = ::http::Request::builder()
            .body(())
            .expect("a request")
            .into_parts()
            .0;
        let uri = parts.uri.clone();
        parts.extensions.insert(::axum::extract::OriginalUri(uri));
        parts
    }

    /// The server as the web starter's `main` builds it: the route list at
    /// start-up, then requests through the router, with the negotiator and
    /// without it.
    #[cfg(all(feature = "axum", feature = "leptos"))]
    fn through_leptos_axum() {
        use ::axum::{Router, body::Body, http::Request};
        use leptos_axum::{LeptosRoutes, generate_route_list};
        use tower::ServiceExt;

        // What formatting does when the render has no language for its
        // request (with no catalogs installed here, `current` would take
        // the other branch): the negotiator's answer is the language.
        fn page() {
            if super::from_layer().is_none() {
                super::unrequested(true);
            }
        }
        fn server(negotiator: bool) -> Router {
            let options = crate::line::leptos::config::LeptosOptions::builder()
                .output_name("unrequested")
                .build();
            let routes = generate_route_list(page);
            let router = Router::new().leptos_routes(&options, routes, page);
            let router = if negotiator {
                router.layer(crate::axum::Negotiator::default())
            } else {
                router
            };
            router.with_state(options)
        }
        async fn get(router: Router) {
            let response = router
                .oneshot(Request::get("/").body(Body::empty()).expect("a request"))
                .await
                .expect("a response");
            let _ = ::axum::body::to_bytes(response.into_body(), usize::MAX).await;
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime");
        // Start-up: the route list, with the negotiator on the router.
        let with = server(true);
        assert!(given(Kind::Unrequested).is_empty());
        // A request the negotiator answered.
        runtime.block_on(get(with));
        assert!(given(Kind::Unrequested).is_empty());
        // A router without it: the first request says so, the next not again.
        let without = server(false);
        runtime.block_on(get(without.clone()));
        runtime.block_on(get(without));
    }
}
