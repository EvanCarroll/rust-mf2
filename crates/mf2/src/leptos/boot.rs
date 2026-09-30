//! The client's boot and locale switch.
//!
//! **Boot.** The page already carries everything the client needs, so there
//! is no inline script (no CSP nonce), no JSON (no serde in the wasm) and no
//! extra round trip:
//!
//! * the locale is `<html lang>`;
//! * the catalog's immutable URL is `<link rel=preload data-mf2 href>`, and
//!   the gate's `fetch()` **reuses** that preload rather than making a second
//!   request (P0.2 measured exactly one catalog request).
//!
//! [`hydrate_body`] fetches it, validates it against `MANIFEST_HASH`,
//! installs it, and only then hydrates. The gate is not needed to avoid text
//! mismatches — hydration never compares text (P0.10) — but it is needed for
//! anything that reads a plain `String` during hydration (`<Title>` runs a
//! client effect and would otherwise blank the title) and for markup
//! messages, whose node *structure* comes from the catalog.
//!
//! **Switching.** [`set_locale`] fetches, validates, swaps the thread-local,
//! walks the node registry, notifies the derived conversions, and updates
//! `<html lang dir>`. On failure the old catalog stays and the error is
//! returned — except a manifest mismatch, a deploy skew: this wasm cannot
//! read the new deploy's catalogs, so `set_locale` remembers the choice and
//! reloads into it rather than misreads.
//!
//! **How the client learns another locale's URL** is owner question 1, and
//! this module implements *both* candidates so that Phase 6 can measure them
//! on the reference workload:
//!
//! | Source | Cost |
//! |---|---|
//! | `<link rel="mf2-catalog" data-mf2-locale=…>` in the page | no round trip; a few bytes per locale in **every** page |
//! | `GET /i18n/<tag>` → `307` to the immutable URL (what P0.2 used) | nothing in the page; one extra round trip, at switch time only |
//!
//! [`catalog_url`] prefers the in-page map and falls back to the redirect
//! route, so an application chooses by whether its shell emits the links.
//!
//! **A client-only application** (`csr`, §8) has no server to negotiate
//! with and no page it rendered. [`mount_to_body`] chooses the locale itself
//! — the one it remembered in `localStorage`, else the reader's first
//! `navigator.languages` entry the build has, else the source locale — and
//! finds the catalog's hashed URL in `i18n/index.json`, which `mf2 compile
//! --site` writes beside the catalogs and `index.html` preloads. Then the
//! same gate: fetch, validate, install, and only then mount. A switch
//! remembers its locale, so a reload comes back in it.

use alloc::string::String;
use alloc::vec::Vec;

use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Response};

use mf2_catalog::Dir;

use crate::error::LoadError;
use crate::leptos::links::{CATALOG_LINK_LOCALE_ATTR, CATALOG_ROUTE, PRELOAD_ATTR};
#[cfg(feature = "csr")]
use crate::leptos::links::{CSR_INDEX_ATTR, CSR_INDEX_URL, LOCALE_STORAGE_KEY};
#[cfg(not(feature = "csr"))]
use crate::leptos::links::{LOCALE_COOKIE, LOCALE_QUERY, QUERY_ATTR};
#[cfg(any(feature = "hydrate", not(feature = "static-locale")))]
use crate::leptos::registry;
use crate::leptos::{catalog, state};
// Which of the three a build names depends on its mode and features.
#[allow(unused_imports)]
use crate::line::{leptos, reactive_graph, tachys};

fn document() -> Option<Document> {
    web_sys::window()?.document()
}

fn html(document: &Document) -> Option<Element> {
    document.document_element()
}

/// The locale the server negotiated, read from `<html lang>`.
///
/// The client **never re-negotiates**: the server serialized its answer into
/// the page, and the prior-art audit found re-negotiation to be the source of
/// a long tail of hydration bugs in other i18n libraries.
#[must_use]
pub fn document_locale() -> Option<String> {
    let lang = html(&document()?)?.get_attribute("lang")?;
    (!lang.is_empty()).then_some(lang)
}

/// The URL of `tag`'s catalog: the page's own link if the shell emitted one,
/// else the redirect route.
///
/// A client-only application looks in the index its boot loaded first, and
/// has no redirect route to fall back to: a static host serves files.
#[must_use]
pub fn catalog_url(tag: &str) -> Option<String> {
    #[cfg(feature = "csr")]
    if let Some(url) = indexed_url(tag) {
        return Some(url);
    }
    let document = document()?;
    // The preload link, which names the locale the page was *rendered* in —
    // not necessarily the one being asked for, once a switch has happened.
    if document_locale().as_deref() == Some(tag)
        && let Ok(Some(link)) =
            document.query_selector(&["link[", PRELOAD_ATTR, "][href]"].concat())
        && let Some(href) = link.get_attribute("href")
    {
        return Some(href);
    }
    // The in-page map, if there is one.
    let selector = ["link[", CATALOG_LINK_LOCALE_ATTR, "=\"", tag, "\"][href]"].concat();
    if let Ok(Some(link)) = document.query_selector(&selector)
        && let Some(href) = link.get_attribute("href")
    {
        return Some(href);
    }
    if cfg!(feature = "csr") {
        return None;
    }
    // The redirect route: one extra round trip, at switch time only.
    Some([CATALOG_ROUTE, tag].concat())
}

/// Fetches `url`: a response with a success status, or `Fetch`.
async fn fetch_response(url: &str) -> Result<Response, LoadError> {
    let window = web_sys::window().ok_or(LoadError::Fetch)?;
    let response: Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|_| LoadError::Fetch)?
        .dyn_into()
        .map_err(|_| LoadError::Fetch)?;
    if response.ok() {
        Ok(response)
    } else {
        Err(LoadError::Fetch)
    }
}

/// Fetches `url` and returns its bytes.
async fn fetch(url: &str) -> Result<Vec<u8>, LoadError> {
    let response = fetch_response(url).await?;
    let buffer = JsFuture::from(response.array_buffer().map_err(|_| LoadError::Fetch)?)
        .await
        .map_err(|_| LoadError::Fetch)?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

/// Fetches and validates `tag`'s catalog without installing it — what a
/// language menu calls on hover to warm the HTTP cache.
pub async fn preload_locale(tag: &str) -> Result<(), LoadError> {
    let url = catalog_url(tag).ok_or(LoadError::UnknownLocale)?;
    catalog::read(fetch(&url).await?).map(|_| ())
}

/// Switches to `tag`: fetch → validate → swap → update every live node →
/// notify the derived conversions → `<html lang dir>`.
///
/// On failure the active catalog is untouched and the error is returned —
/// except a catalog from another deploy ([`LoadError::ManifestMismatch`]):
/// the server has moved on to a build this wasm cannot read, so the choice
/// is remembered as below, the page reloads into it, and `Ok` is returned
/// with the navigation under way. Every control that calls `set_locale`
/// gets that, not only [`LocaleSwitcher`](crate::leptos::LocaleSwitcher).
///
/// The choice is remembered for the next visit. A server-rendered page
/// writes the cookie the server negotiates from
/// ([`LOCALE_COOKIE`](crate::leptos::links::LOCALE_COOKIE)) and takes a `?lang=`
/// out of the address, which would otherwise outrank it on a reload; a
/// client-only application writes
/// [`LOCALE_STORAGE_KEY`](crate::leptos::links::LOCALE_STORAGE_KEY), which its next
/// boot reads first.
///
/// Under `static-locale` (strategy C, the default for islands) nothing
/// follows the locale, so a switch is instead the cookie the server reads
/// ([`LOCALE_COOKIE`](crate::leptos::links::LOCALE_COOKIE)) and a reload: the server renders the whole page —
/// server-only components included — in the new locale. A client-only
/// application under `static-locale` remembers the locale and reloads.
#[cfg(not(feature = "static-locale"))]
pub async fn set_locale(tag: &str) -> Result<(), LoadError> {
    if state::dir_of(tag).is_none() {
        return Err(LoadError::UnknownLocale);
    }
    match switch_live(tag).await {
        Ok(()) => {}
        Err(LoadError::ManifestMismatch) => {
            web_sys::console::error_1(&JsValue::from_str(
                "mf2: the catalog is from another deploy; reloading into the new locale.",
            ));
            let window = web_sys::window().ok_or(LoadError::Fetch)?;
            return reload_into(&window, tag);
        }
        Err(error) => return Err(error),
    }
    #[cfg(feature = "csr")]
    remember_locale(tag);
    #[cfg(not(feature = "csr"))]
    if let Some(window) = web_sys::window() {
        // Best effort: the page has switched, and a cookie the browser
        // refuses only costs the next visit its choice.
        let _ = write_locale_cookie(&window, tag);
        drop_locale_query(&window);
    }
    Ok(())
}

/// See the live build's `set_locale`: under `static-locale` a switch is the
/// cookie and a reload.
#[cfg(feature = "static-locale")]
#[allow(clippy::unused_async)]
pub async fn set_locale(tag: &str) -> Result<(), LoadError> {
    if state::dir_of(tag).is_none() {
        return Err(LoadError::UnknownLocale);
    }
    if document_locale().as_deref() == Some(tag) {
        return Ok(());
    }
    let window = web_sys::window().ok_or(LoadError::Fetch)?;
    reload_into(&window, tag)
}

/// A client-only application's reload into `tag` — a `static-locale`
/// switch, or a live one that met another deploy's catalog: there is no
/// server to tell, so the choice goes where the next boot reads it first.
#[cfg(feature = "csr")]
fn reload_into(window: &web_sys::Window, tag: &str) -> Result<(), LoadError> {
    remember_locale(tag);
    window.location().reload().map_err(|_| LoadError::Fetch)
}

/// A server-rendered page's reload into `tag` — a `static-locale` switch,
/// or a live one that met another deploy's catalog: the cookie the server
/// negotiates from, and a navigation.
#[cfg(not(feature = "csr"))]
fn reload_into(window: &web_sys::Window, tag: &str) -> Result<(), LoadError> {
    write_locale_cookie(window, tag)?;
    let location = window.location();
    // A locale in the address outranks the cookie, so it goes; changing the
    // query navigates, and an unchanged one needs a reload.
    match location
        .search()
        .ok()
        .and_then(|search| without_param(&search, &page_query()))
    {
        Some(search) => location.set_search(&search),
        None => location.reload(),
    }
    .map_err(|_| LoadError::Fetch)
}

/// The cookie the server negotiates from, with the attributes `mf2::axum`'s
/// `CookieLocale` writes by default. A server-rendered page's switch, live
/// or not, is remembered here.
#[cfg(not(feature = "csr"))]
fn write_locale_cookie(window: &web_sys::Window, tag: &str) -> Result<(), LoadError> {
    let document = window.document().ok_or(LoadError::Fetch)?;
    let secure = window.location().protocol().is_ok_and(|p| p == "https:");
    let cookie = [
        LOCALE_COOKIE,
        "=",
        tag,
        "; path=/; max-age=31536000; samesite=lax",
        if secure { "; secure" } else { "" },
    ]
    .concat();
    // `document.cookie` is not in the web-sys features this crate enables
    // (it lives on `HtmlDocument`); the property set is the same call.
    js_sys::Reflect::set(
        &document,
        &JsValue::from_str("cookie"),
        &JsValue::from_str(&cookie),
    )
    .map_err(|_| LoadError::Fetch)?;
    Ok(())
}

/// The reader's time zone, for the server to render the next page in
/// ([`TIME_ZONE_COOKIE`](crate::leptos::links::TIME_ZONE_COOKIE)), with the locale
/// cookie's attributes. Behind the feature, like everything of the
/// reader's zone, so a build without dates compiles none of it.
#[cfg(all(feature = "hydrate", feature = "fn-datetime"))]
pub(crate) fn write_zone_cookie(window: &web_sys::Window, zone: &str) {
    let Some(document) = window.document() else {
        return;
    };
    let secure = window.location().protocol().is_ok_and(|p| p == "https:");
    let cookie = [
        crate::leptos::links::TIME_ZONE_COOKIE,
        "=",
        zone,
        "; path=/; max-age=31536000; samesite=lax",
        if secure { "; secure" } else { "" },
    ]
    .concat();
    // Best effort: a refused cookie only costs the next page its zone.
    let _ = js_sys::Reflect::set(
        &document,
        &JsValue::from_str("cookie"),
        &JsValue::from_str(&cookie),
    );
}

/// After a live switch: the address without its `?lang=`, in place. The
/// query source outranks the cookie, so a reload of `?lang=en` after a
/// switch to French would otherwise come back in English. No navigation:
/// the page is already in the new locale.
#[cfg(all(not(feature = "static-locale"), not(feature = "csr")))]
fn drop_locale_query(window: &web_sys::Window) {
    let location = window.location();
    let Some(search) = location
        .search()
        .ok()
        .and_then(|search| without_param(&search, &page_query()))
    else {
        return;
    };
    let (Ok(path), Ok(hash)) = (location.pathname(), location.hash()) else {
        return;
    };
    let url = [path.as_str(), search.as_str(), hash.as_str()].concat();
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&url));
    }
}

/// The query parameter that names a language in this page's address: the
/// one its preload link states (`data-mf2-query`, the server's installed
/// query source), else [`LOCALE_QUERY`].
#[cfg(not(feature = "csr"))]
fn page_query() -> String {
    document()
        .and_then(|document| {
            document
                .query_selector(&["link[", PRELOAD_ATTR, "][", QUERY_ATTR, "]"].concat())
                .ok()
                .flatten()
        })
        .and_then(|link| link.get_attribute(QUERY_ATTR))
        .unwrap_or_else(|| String::from(LOCALE_QUERY))
}

/// `search` (`?a=1&lang=fr&b=2`) without the pairs named `name`, or `None`
/// if it has none.
#[cfg(not(feature = "csr"))]
fn without_param(search: &str, name: &str) -> Option<String> {
    let query = search.strip_prefix('?').unwrap_or(search);
    let named = |pair: &&str| pair.split('=').next() == Some(name);
    if !query.split('&').any(|pair| named(&pair)) {
        return None;
    }
    let mut out = String::new();
    for pair in query
        .split('&')
        .filter(|pair| !pair.is_empty() && !named(pair))
    {
        out.push(if out.is_empty() { '?' } else { '&' });
        out.push_str(pair);
    }
    Some(out)
}

/// Strategy B: fetch, swap, and bring every live node up to date.
#[cfg(not(feature = "static-locale"))]
async fn switch_live(tag: &str) -> Result<(), LoadError> {
    if catalog::active().is_some_and(|c| c.locale() == tag) {
        return Ok(());
    }
    let url = catalog_url(tag).ok_or(LoadError::UnknownLocale)?;
    let catalog = catalog::read(fetch(&url).await?)?;
    install_active(catalog);
    Ok(())
}

/// Installs `catalog` as the active one and brings the page up to date.
#[cfg(not(feature = "static-locale"))]
fn install_active(catalog: alloc::sync::Arc<mf2_catalog::Catalog>) {
    let tag = alloc::string::ToString::to_string(catalog.locale());
    let dir = catalog.dir();
    catalog::set_active(catalog);
    // The registry first — synchronously, so the page is consistent before
    // anything reactive runs (P0.11) — then the conversions.
    registry::relocalize_all();
    reactive_graph::traits::Notify::notify(&catalog::changed());
    set_document_lang(&tag, dir);
}

/// `<html lang dir>` (WCAG 3.1.1). `dir` is written only for RTL and back to
/// `ltr`, never removed, so that a switch cannot leave a stale direction.
pub fn set_document_lang(tag: &str, dir: Dir) {
    let Some(document) = document() else { return };
    let Some(html) = html(&document) else { return };
    let _ = html.set_attribute("lang", tag);
    let _ = html.set_attribute("dir", if dir == Dir::Rtl { "rtl" } else { "ltr" });
}

/// Loads the catalog the page was rendered with, and installs it — without
/// notifying, because nothing has rendered yet.
///
/// This is the hydration gate: call it before hydrating.
pub async fn load_page_catalog() -> Result<(), LoadError> {
    let tag = document_locale().ok_or(LoadError::UnknownLocale)?;
    let url = catalog_url(&tag).ok_or(LoadError::UnknownLocale)?;
    let catalog = catalog::read(fetch(&url).await?)?;
    catalog::set_active(catalog);
    Ok(())
}

/// Reports a boot failure the way a client can act on: one console message,
/// and — for a deploy skew — a reload, which is the only correct answer to a
/// wasm and a catalog that disagree (F6).
///
/// What the reader is left with: a hydrating page keeps its served HTML; a
/// client-only one keeps whatever `index.html` holds, and nothing mounts.
#[cfg(feature = "hydrate")]
const LEFT_AS: &str = "; the page stays as served, not interactive.";
#[cfg(feature = "csr")]
const LEFT_AS: &str = "; the application is not started.";

fn report_boot_failure(error: &LoadError) {
    let what = match error {
        LoadError::ManifestMismatch => {
            "mf2: this page's catalog is from another deploy; reloading."
        }
        LoadError::NotInstalled => "mf2: install() was not called before the boot.",
        LoadError::UnknownLocale if cfg!(feature = "csr") => {
            "mf2: the catalog index does not list this locale"
        }
        LoadError::UnknownLocale => "mf2: the page does not say which locale it is in",
        LoadError::Fetch if cfg!(feature = "csr") => {
            "mf2: the catalog index or the catalog could not be fetched"
        }
        LoadError::Fetch => "mf2: the catalog could not be fetched",
        LoadError::Malformed(_) => "mf2: the catalog is malformed",
    };
    let line = match error {
        LoadError::ManifestMismatch | LoadError::NotInstalled => String::from(what),
        _ => [what, LEFT_AS].concat(),
    };
    web_sys::console::error_1(&JsValue::from_str(&line));
    if matches!(error, LoadError::ManifestMismatch)
        && let Some(window) = web_sys::window()
    {
        let _ = window.location().reload();
    }
}

/// Boots the client and hydrates `app`:
///
/// ```ignore
/// #[wasm_bindgen]
/// pub fn hydrate() {
///     my_app_i18n::install();          // the generated Setup
///     mf2::leptos::hydrate_body(App);
/// }
/// ```
///
/// The catalog is fetched — reusing the page's preload, so no extra request
/// — validated and installed **before** `app` hydrates. A catalog from
/// another deploy reloads the page; any other failure logs once and leaves
/// the served HTML as it is, readable but not hydrated (see `boot_then`).
#[cfg(feature = "hydrate")]
pub fn hydrate_body<F, N>(app: F)
where
    F: FnOnce() -> N + 'static,
    N: leptos::IntoView,
{
    boot_then(app, false);
}

/// [`hydrate_body`] for an application built with `#[lazy]` routes or
/// `cargo leptos --split`: the lazy chunks share this crate's state, since
/// they share linear memory, statics and the `Owner`.
///
/// On Leptos 0.9 the application's `hydrate` feature must also turn on
/// `leptos/lazy`, or tachys panics when hydration reaches a lazy route.
#[cfg(feature = "hydrate")]
pub fn hydrate_lazy<F, N>(app: F)
where
    F: FnOnce() -> N + 'static,
    N: leptos::IntoView,
{
    boot_then(app, true);
}

#[cfg(feature = "hydrate")]
fn boot_then<F, N>(app: F, lazy: bool)
where
    F: FnOnce() -> N + 'static,
    N: leptos::IntoView,
{
    // The executor has to exist before anything is awaited; leptos' own
    // mount functions do this too, and a second call is a no-op.
    let _ = any_spawner::Executor::init_wasm_bindgen();
    leptos::task::spawn_local(async move {
        if let Err(error) = load_page_catalog().await {
            report_boot_failure(&error);
            // **Do not hydrate.** With no catalog every message formats to
            // nothing, so hydrating would replace a page the reader can
            // read with an interactive page that says nothing — and a page
            // with markup would not even get that far: the fragment its
            // hydration walks comes from the catalog, so with none it walks
            // a shape the document does not have and traps the wasm
            // (P0.10). Served HTML, un-hydrated, is the better failure.
            return;
        }
        // Dates: hydrate in the zone the page was rendered in, then correct
        // to the reader's (`crate::leptos::zone`).
        #[cfg(feature = "fn-datetime")]
        {
            crate::leptos::zone::before_hydration(false);
            if lazy {
                // `hydrate_lazy` itself, awaited rather than spawned, so
                // that the correction runs when it has hydrated.
                leptos::mount::hydrate_from_async(tachys::dom::body(), app)
                    .await
                    .forget();
            } else {
                leptos::mount::hydrate_body(app);
            }
            crate::leptos::zone::after_hydration();
        }
        #[cfg(not(feature = "fn-datetime"))]
        if lazy {
            leptos::mount::hydrate_lazy(app);
        } else {
            leptos::mount::hydrate_body(app);
        }
    });
}

/// Boots an **islands** application.
///
/// ```ignore
/// #[wasm_bindgen]
/// pub fn hydrate() {
///     my_app_i18n::install();          // the generated Setup
///     mf2::leptos::hydrate_islands();
/// }
/// ```
///
/// and, in the shell, [`IslandsGate`](crate::leptos::IslandsGate) as the first
/// thing in `<body>`.
///
/// **Why islands need a gate of their own.** Leptos' island script calls the
/// entry point and walks the document for islands in the same turn —
/// `mod.hydrate(); hydrateIslands(document.body, mod)` — without awaiting
/// the first, so there is no moment in `hydrate` to wait for a fetch in.
/// What the walk *does* await is an island whose function returns a
/// promise, and it walks one island at a time in document order. The gate is
/// such an island: empty, first, and resolved when the catalog is
/// installed. Every island after it hydrates against the catalog the page
/// was rendered with — markup included, whose node structure comes from the
/// catalog — for no page bytes and no extra request: the fetch reuses
/// the preload, which began before the wasm did.
///
/// This function starts that fetch and sets the reactive owner the walk
/// needs. Without a gate in the page the catalog still loads *alongside*:
/// text keeps the server's words until it lands (hydration never reads
/// text) and is then brought up to date, but a markup message inside
/// an island would hydrate against no structure and trap. So: a page with
/// rich messages in islands renders the gate.
///
/// On a failed load the gate never resolves, so the islands stay as served
/// — readable, not interactive — rather than hydrating against no catalog;
/// the same rule as [`hydrate_body`].
#[cfg(feature = "hydrate")]
pub fn hydrate_islands() {
    // Synchronously, before the walk that follows this function: the owner,
    // and the reader's time zone, which each island is corrected to as it
    // hydrates (`crate::leptos::zone`).
    leptos::mount::hydrate_islands();
    #[cfg(feature = "fn-datetime")]
    crate::leptos::zone::before_hydration(true);
    let boot = wasm_bindgen_futures::future_to_promise(async {
        match load_page_catalog().await {
            Ok(()) => {
                // Islands that hydrated before this — only possible without
                // a gate — hold the server's text; bring them up to date.
                registry::relocalize_all();
                reactive_graph::traits::Notify::notify(&catalog::changed());
                Ok(JsValue::UNDEFINED)
            }
            Err(error) => {
                report_boot_failure(&error);
                Err(JsValue::UNDEFINED)
            }
        }
    });
    ISLANDS_BOOT.with(|slot| slot.replace(Some(boot)));
}

#[cfg(feature = "hydrate")]
std::thread_local! {
    /// The catalog load [`hydrate_islands`] started, for the gate to await.
    static ISLANDS_BOOT: core::cell::RefCell<Option<js_sys::Promise>> =
        const { core::cell::RefCell::new(None) };
}

/// What the gate's export awaits: the catalog load [`hydrate_islands`]
/// started. Resolves once it is installed; **never** resolves if it failed,
/// which stops Leptos' island walk there and leaves the islands as served.
///
/// Applications do not call this; [`islands_gate!`](crate::leptos::islands_gate)
/// exports it under the name [`IslandsGate`](crate::leptos::IslandsGate) renders.
#[cfg(feature = "hydrate")]
pub async fn wait_for_catalog() {
    let Some(boot) = ISLANDS_BOOT.with(|slot| slot.borrow().clone()) else {
        // `hydrate_islands` was not called: nothing to wait for, and nothing
        // better to do than let the islands hydrate.
        return;
    };
    if JsFuture::from(boot).await.is_err() {
        core::future::pending::<()>().await;
    }
}

// ------------------------------------------------------------ client-only ---

#[cfg(feature = "csr")]
std::thread_local! {
    /// The catalog index the boot loaded: tag → URL, for the locales this
    /// build knows. A tag the index has and the build does not is dropped.
    static CSR_INDEX: core::cell::RefCell<Vec<(&'static str, String)>> =
        const { core::cell::RefCell::new(Vec::new()) };
}

/// `tag`'s catalog URL, from the index the boot loaded.
#[cfg(feature = "csr")]
fn indexed_url(tag: &str) -> Option<String> {
    CSR_INDEX.with(|index| {
        index
            .borrow()
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, url)| url.clone())
    })
}

#[cfg(feature = "csr")]
fn storage() -> Option<web_sys::Storage> {
    // `localStorage` throws where storage is disabled; that is "nothing
    // remembered", not a failure.
    web_sys::window()?.local_storage().ok()?
}

/// Remembers `tag` for the next boot. Where storage is unavailable the
/// choice lasts until the page is closed, which is all that can be done.
#[cfg(feature = "csr")]
fn remember_locale(tag: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(LOCALE_STORAGE_KEY, tag);
    }
}

/// The locale a client-only application starts in: the one it remembered,
/// else the one that best serves the reader's `navigator.languages` (and
/// `navigator.language`), as a list — each later entry demoted — matched
/// as `mf2::axum` matches `Accept-Language` (the one matcher,
/// [`lookup_locale`](crate::leptos::lookup_locale)), else the source locale.
///
/// It never fails: a remembered locale the build no longer has, or a reader
/// whose languages it has none of, starts in the source locale.
#[cfg(feature = "csr")]
#[must_use]
pub fn client_locale() -> &'static str {
    let locales = state::locales();
    if let Some((tag, _)) = storage()
        .and_then(|s| s.get_item(LOCALE_STORAGE_KEY).ok().flatten())
        .as_deref()
        .and_then(|remembered| state::lookup_locale(remembered, locales))
    {
        return tag;
    }
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        // Older engines have no `languages`; every engine has `language`,
        // which is otherwise the list's first entry again.
        let mut languages: Vec<String> = navigator
            .languages()
            .iter()
            .filter_map(|l| l.as_string())
            .collect();
        languages.extend(navigator.language());
        if let Some((tag, _)) = state::best_locale(languages.iter().map(String::as_str), locales) {
            return tag;
        }
    }
    state::source_locale()
}

/// Loads the catalog index: the URL from `index.html`'s preload link (so the
/// fetch reuses it), else [`CSR_INDEX_URL`]. It is JSON — `{"fr":
/// "fr.3fa9c1.mf2b", …}` — parsed by the browser, so the wasm carries no
/// JSON parser; each file name is relative to the index.
#[cfg(feature = "csr")]
async fn load_index() -> Result<(), LoadError> {
    let href = document()
        .and_then(|d| {
            d.query_selector(&["link[", CSR_INDEX_ATTR, "][href]"].concat())
                .ok()
                .flatten()
        })
        .and_then(|link| link.get_attribute("href"))
        .unwrap_or_else(|| String::from(CSR_INDEX_URL));
    let response = fetch_response(&href).await?;
    let json = JsFuture::from(response.json().map_err(|_| LoadError::Fetch)?)
        .await
        .map_err(|_| LoadError::Fetch)?;
    let base = href.rfind('/').and_then(|at| href.get(..=at)).unwrap_or("");
    let mut index = Vec::new();
    for (tag, _) in state::locales() {
        if let Some(file) = js_sys::Reflect::get(&json, &JsValue::from_str(tag))
            .ok()
            .and_then(|value| value.as_string())
        {
            index.push((*tag, [base, file.as_str()].concat()));
        }
    }
    CSR_INDEX.with(|slot| *slot.borrow_mut() = index);
    Ok(())
}

/// A client-only application's gate: the index, the locale, and that
/// locale's catalog — fetched, validated and installed, with `<html lang
/// dir>` set to match, before anything renders.
#[cfg(feature = "csr")]
pub async fn load_client_catalog() -> Result<(), LoadError> {
    load_index().await?;
    let tag = client_locale();
    let url = indexed_url(tag).ok_or(LoadError::UnknownLocale)?;
    let catalog = catalog::read(fetch(&url).await?)?;
    let dir = catalog.dir();
    catalog::set_active(catalog);
    set_document_lang(tag, dir);
    #[cfg(feature = "fn-datetime")]
    crate::leptos::zone::mount_in_reader_zone();
    Ok(())
}

/// Boots a client-only application and mounts `app` to `<body>`:
///
/// ```ignore
/// fn main() {
///     my_app_i18n::install();          // the generated Setup
///     mf2::leptos::mount_to_body(App);
/// }
/// ```
///
/// with, in `index.html`, the index preloaded so that it downloads in
/// parallel with the wasm:
///
/// ```html
/// <link rel="preload" as="fetch" crossorigin="anonymous" href="i18n/index.json" data-mf2-index>
/// ```
///
/// The same gate as [`hydrate_body`](crate::leptos::hydrate_body): nothing renders
/// until the catalog is installed, so the first frame is already in the
/// reader's language and a markup message has its structure. A catalog from
/// another deploy reloads; any other failure logs one `mf2:` line and
/// mounts nothing, leaving what `index.html` holds.
#[cfg(feature = "csr")]
pub fn mount_to_body<F, N>(app: F)
where
    F: FnOnce() -> N + 'static,
    N: leptos::IntoView,
{
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = load_client_catalog().await {
            report_boot_failure(&error);
            return;
        }
        leptos::mount::mount_to_body(app);
    });
}
