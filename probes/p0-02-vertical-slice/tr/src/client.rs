//! Browser side (feature `hydrate`): one thread-local active catalog, the boot
//! gate (fetch the preloaded catalog → validate → install → only then
//! hydrate), and `set_locale`.
//!
//! No `#[wasm_bindgen]` items are defined here — only js-sys / web-sys calls —
//! so `#![forbid(unsafe_code)]` holds for this crate (plans/04 §10).

use crate::{catalog::Catalog, error::LoadError, registry};
use leptos::IntoView;
use leptos::reactive::graph::Observer;
use leptos::reactive::signal::ArcTrigger;
use leptos::reactive::traits::{Notify, Track};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

thread_local! {
    static ACTIVE: RefCell<Option<Rc<Catalog>>> = const { RefCell::new(None) };
    static EXPECT_MANIFEST: Cell<u64> = const { Cell::new(0) };
    // Only derived values (`TextProp`, `Signal<String>`, `to_string()` under an
    // observer) subscribe here; DOM nodes go through the registry instead.
    static CHANGED: ArcTrigger = ArcTrigger::new();
}

fn active() -> Option<Rc<Catalog>> {
    ACTIVE.with(|a| a.try_borrow().ok().and_then(|a| a.clone()))
}

pub(crate) fn with_text<R>(id: u32, f: impl FnOnce(&str) -> R) -> R {
    match active() {
        Some(c) => f(c.get(id)),
        None => f(""),
    }
}

pub(crate) fn track_locale() {
    // Tracking outside an observer makes reactive_graph warn in debug builds;
    // event handlers and `format!`-style uses must stay silent.
    if Observer::get().is_some() {
        CHANGED.with(Track::track);
    }
}

pub(crate) fn current_locale() -> String {
    track_locale();
    active().map(|c| c.locale().to_owned()).unwrap_or_default()
}

/// Boot gate + `leptos::mount::hydrate_body`.
pub fn hydrate_body<F, N>(app: F, expect_manifest: u64)
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    gate(expect_manifest, move || leptos::mount::hydrate_body(app));
}

/// Boot gate + `leptos::mount::hydrate_lazy` (for `#[lazy]` routes / `--split`).
pub fn hydrate_lazy<F, N>(app: F, expect_manifest: u64)
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    gate(expect_manifest, move || leptos::mount::hydrate_lazy(app));
}

fn gate(expect_manifest: u64, hydrate: impl FnOnce() + 'static) {
    EXPECT_MANIFEST.set(expect_manifest);
    wasm_bindgen_futures::spawn_local(async move {
        match boot(expect_manifest).await {
            Ok(()) => hydrate(),
            // Deploy skew / broken catalog: never misread. The real library
            // reloads once (plans/04 §6); the probe reports and stays inert.
            Err(e) => leptos::leptos_dom::logging::console_error(e.code()),
        }
    });
}

async fn boot(expect_manifest: u64) -> Result<(), LoadError> {
    let window = web_sys::window().ok_or(LoadError::NoPreloadLink)?;
    let document = window.document().ok_or(LoadError::NoPreloadLink)?;
    // The <link rel=preload data-mf2> *is* the boot data: URL from it, locale
    // from <html lang>. No inline script, no JSON.
    let href = document
        .query_selector("link[data-mf2]")
        .ok()
        .flatten()
        .and_then(|l| l.get_attribute("href"))
        .ok_or(LoadError::NoPreloadLink)?;
    let lang = document.document_element().and_then(|e| e.get_attribute("lang"));
    let catalog = Catalog::new(fetch_bytes(&window, &href).await?, expect_manifest)?;
    if lang.as_deref() != Some(catalog.locale()) {
        return Err(LoadError::LangMismatch);
    }
    ACTIVE.with(|a| {
        if let Ok(mut a) = a.try_borrow_mut() {
            *a = Some(Rc::new(catalog));
        }
    });
    Ok(())
}

async fn fetch_bytes(window: &web_sys::Window, url: &str) -> Result<Vec<u8>, LoadError> {
    // Default fetch() = mode "cors", credentials "same-origin": matches the
    // `crossorigin` (anonymous) preload, so the preloaded response is reused.
    let resp = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|_| LoadError::Fetch)?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| LoadError::Fetch)?;
    if !resp.ok() {
        return Err(LoadError::Status);
    }
    let buf = JsFuture::from(resp.array_buffer().map_err(|_| LoadError::Fetch)?)
        .await
        .map_err(|_| LoadError::Fetch)?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

/// Switches locale at runtime: fetch → validate → swap the thread-local →
/// rewrite registered nodes → `<html lang dir>` + cookie → notify derived
/// values. On failure the old catalog stays and the error is returned.
///
/// The client never guesses a content hash: `/i18n/<tag>` answers with a
/// redirect to the hashed, immutable URL (one extra round trip, at switch
/// time only).
pub async fn set_locale(tag: &str) -> Result<(), LoadError> {
    let window = web_sys::window().ok_or(LoadError::Fetch)?;
    let mut url = String::from("/i18n/");
    url.push_str(tag);
    let catalog = Rc::new(Catalog::new(fetch_bytes(&window, &url).await?, EXPECT_MANIFEST.get())?);
    ACTIVE.with(|a| {
        if let Ok(mut a) = a.try_borrow_mut() {
            *a = Some(Rc::clone(&catalog));
        }
    });
    registry::refresh_all(|id, write| write(catalog.get(id)));
    if let Some(document) = window.document() {
        if let Some(root) = document.document_element() {
            let _ = root.set_attribute("lang", catalog.locale());
            let _ = root.set_attribute("dir", catalog.dir().as_str());
        }
        if let Ok(doc) = document.dyn_into::<web_sys::HtmlDocument>() {
            let mut cookie = String::from("mf2-locale=");
            cookie.push_str(catalog.locale());
            cookie.push_str("; Path=/; Max-Age=31536000; SameSite=Lax");
            let _ = doc.set_cookie(&cookie);
        }
    }
    CHANGED.with(Notify::notify);
    Ok(())
}
