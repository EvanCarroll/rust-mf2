//! The client's boot and locale switch (`plans/04-leptos-integration.md`
//! §6).
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
//! returned — a manifest mismatch is a deploy skew, so the caller reloads
//! rather than misreads.
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

use alloc::string::String;
use alloc::vec::Vec;

use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
#[cfg(any(feature = "hydrate", feature = "static-locale"))]
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Response};

use mf2_catalog::Dir;

use crate::error::LoadError;
use crate::links::{CATALOG_LINK_LOCALE_ATTR, CATALOG_ROUTE, PRELOAD_ATTR};
#[cfg(feature = "static-locale")]
use crate::links::{LOCALE_COOKIE, LOCALE_QUERY};
#[cfg(any(feature = "hydrate", not(feature = "static-locale")))]
use crate::registry;
use crate::{catalog, state};

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
/// a long tail of hydration bugs (§11, item 3).
#[must_use]
pub fn document_locale() -> Option<String> {
    let lang = html(&document()?)?.get_attribute("lang")?;
    (!lang.is_empty()).then_some(lang)
}

/// The URL of `tag`'s catalog: the page's own link if the shell emitted one,
/// else the redirect route.
#[must_use]
pub fn catalog_url(tag: &str) -> Option<String> {
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
    // The redirect route: one extra round trip, at switch time only.
    Some([CATALOG_ROUTE, tag].concat())
}

/// Fetches `url` and returns its bytes.
async fn fetch(url: &str) -> Result<Vec<u8>, LoadError> {
    let window = web_sys::window().ok_or(LoadError::Fetch)?;
    let response: Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|_| LoadError::Fetch)?
        .dyn_into()
        .map_err(|_| LoadError::Fetch)?;
    if !response.ok() {
        return Err(LoadError::Fetch);
    }
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
/// On failure the active catalog is untouched.
///
/// Under `static-locale` (strategy C, the default for islands) nothing
/// follows the locale, so a switch is instead the cookie the server reads
/// ([`LOCALE_COOKIE`](crate::links::LOCALE_COOKIE)) and a reload: the server renders the whole page —
/// server-only components included — in the new locale.
#[cfg(not(feature = "static-locale"))]
pub async fn set_locale(tag: &str) -> Result<(), LoadError> {
    if state::dir_of(tag).is_none() {
        return Err(LoadError::UnknownLocale);
    }
    switch_live(tag).await
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
    let location = window.location();
    // A locale in the address outranks the cookie, so it goes; changing the
    // query navigates, and an unchanged one needs a reload.
    match location
        .search()
        .ok()
        .and_then(|search| without_param(&search, LOCALE_QUERY))
    {
        Some(search) => location.set_search(&search),
        None => location.reload(),
    }
    .map_err(|_| LoadError::Fetch)
}

/// `search` (`?a=1&lang=fr&b=2`) without the pairs named `name`, or `None`
/// if it has none.
#[cfg(feature = "static-locale")]
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
/// CSR has no boot of its own yet (Phase 7 A2 gives it one), so only the
/// hydrating builds reach this.
#[cfg(feature = "hydrate")]
fn report_boot_failure(error: &LoadError) {
    web_sys::console::error_1(&JsValue::from_str(match error {
        LoadError::ManifestMismatch => {
            "mf2: this page's catalog is from another deploy; reloading."
        }
        LoadError::NotInstalled => "mf2: install() was not called before hydration.",
        LoadError::UnknownLocale => "mf2: the page does not say which locale it is in.",
        LoadError::Fetch => {
            "mf2: the catalog could not be fetched; the page stays as served, not interactive."
        }
        LoadError::Malformed(_) => {
            "mf2: the catalog is malformed; the page stays as served, not interactive."
        }
    }));
    if matches!(error, LoadError::ManifestMismatch)
        && let Some(window) = web_sys::window()
    {
        let _ = window.location().reload();
    }
}

/// Boots the client and hydrates `app` (§6):
///
/// ```ignore
/// #[wasm_bindgen]
/// pub fn hydrate() {
///     my_app_i18n::install();          // the generated Setup
///     leptos_mf2::hydrate_body(App);
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
/// they share linear memory, statics and the `Owner` (P0.2).
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
        if lazy {
            leptos::mount::hydrate_lazy(app);
        } else {
            leptos::mount::hydrate_body(app);
        }
    });
}

/// Boots an **islands** application (§8).
///
/// ```ignore
/// #[wasm_bindgen]
/// pub fn hydrate() {
///     my_app_i18n::install();          // the generated Setup
///     leptos_mf2::hydrate_islands();
/// }
/// ```
///
/// and, in the shell, [`IslandsGate`](crate::IslandsGate) as the first
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
/// catalog (§7) — for no page bytes and no extra request: the fetch reuses
/// the preload, which began before the wasm did.
///
/// This function starts that fetch and sets the reactive owner the walk
/// needs. Without a gate in the page the catalog still loads *alongside*:
/// text keeps the server's words until it lands (hydration never reads
/// text, P0.10) and is then brought up to date, but a markup message inside
/// an island would hydrate against no structure and trap. So: a page with
/// rich messages in islands renders the gate.
///
/// On a failed load the gate never resolves, so the islands stay as served
/// — readable, not interactive — rather than hydrating against no catalog;
/// the same rule as [`hydrate_body`].
#[cfg(feature = "hydrate")]
pub fn hydrate_islands() {
    // Synchronously, before the walk that follows this function: the owner.
    leptos::mount::hydrate_islands();
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
/// Applications do not call this; [`islands_gate!`](crate::islands_gate)
/// exports it under the name [`IslandsGate`](crate::IslandsGate) renders.
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
