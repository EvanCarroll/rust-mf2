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

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
#[cfg(feature = "hydrate")]
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Response};

use mf2_catalog::Dir;

use crate::error::LoadError;
use crate::links::{CATALOG_LINK_LOCALE_ATTR, CATALOG_ROUTE, PRELOAD_ATTR};
use crate::{catalog, registry, state};

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
    // The preload link of the locale the page was rendered in.
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
pub async fn set_locale(tag: &str) -> Result<(), LoadError> {
    if state::dir_of(tag).is_none() {
        return Err(LoadError::UnknownLocale);
    }
    if catalog::active().is_some_and(|c| c.locale() == tag) {
        return Ok(());
    }
    let url = catalog_url(tag).ok_or(LoadError::UnknownLocale)?;
    let catalog = catalog::read(fetch(&url).await?)?;
    install_active(catalog);
    Ok(())
}

/// Installs `catalog` as the active one and brings the page up to date.
fn install_active(catalog: alloc::sync::Arc<mf2_catalog::Catalog>) {
    let tag = catalog.locale().to_string();
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
/// CSR has no boot of its own yet (Phase 7 gives it one), so only the
/// hydrating build reaches this.
#[cfg(feature = "hydrate")]
fn report_boot_failure(error: &LoadError) {
    web_sys::console::error_1(&JsValue::from_str(match error {
        LoadError::ManifestMismatch => {
            "mf2: this page's catalog is from another deploy; reloading."
        }
        LoadError::NotInstalled => "mf2: install() was not called before hydration.",
        LoadError::UnknownLocale => "mf2: the page does not say which locale it is in.",
        LoadError::Fetch => "mf2: the catalog could not be fetched; rendering the source locale.",
        LoadError::Malformed(_) => "mf2: the catalog is malformed; rendering the source locale.",
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
/// another deploy reloads the page; any other failure logs once and hydrates
/// against no catalog, so the page stays interactive with empty message text
/// rather than not at all.
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
            if matches!(error, LoadError::ManifestMismatch) {
                // The page is reloading; hydrating against the wrong
                // catalog would render text this build cannot mean.
                return;
            }
        }
        if lazy {
            leptos::mount::hydrate_lazy(app);
        } else {
            leptos::mount::hydrate_body(app);
        }
    });
}
