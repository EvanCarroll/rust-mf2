//! Stub catalog and the locale notifier.
//!
//! Client: one thread-local active catalog (no context walk per lookup,
//! plans/04 §5). Server: a per-request [`ServerCatalog`] in context, looked
//! up when rendering to HTML, falling back to the thread-local.
//!
//! The stub maps `MsgId` → text by index. Its contents arrive at run time
//! ([`boot_from_dom`] / [`set_catalog`]), so the id always reaches an opaque
//! lookup and the optimiser cannot fold a call site away.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use leptos::prelude::{ArcTrigger, Notify, Track, use_context};

use crate::MsgId;

/// Per-request catalog on the server, provided through context.
#[derive(Clone)]
pub struct ServerCatalog(pub Arc<[Box<str>]>);

pub(crate) struct Catalog {
    texts: Vec<Box<str>>,
}

impl Catalog {
    pub(crate) fn get(&self, id: MsgId) -> &str {
        self.texts.get(id.0 as usize).map_or("", |s| s)
    }
}

thread_local! {
    static ACTIVE: RefCell<Option<Rc<Catalog>>> = const { RefCell::new(None) };
    static LOCALE: ArcTrigger = ArcTrigger::new();
}

/// The active client catalog (cheap `Rc` clone; `None` before boot).
#[inline(never)]
pub(crate) fn active() -> Option<Rc<Catalog>> {
    ACTIVE
        .try_with(|a| a.try_borrow().ok().and_then(|c| (*c).clone()))
        .ok()
        .flatten()
}

/// Subscribes the current observer (if any) to locale changes.
#[inline(never)]
pub(crate) fn track_locale() {
    let _ = LOCALE.try_with(Track::track);
}

/// Text of `id` for server rendering: the request's catalog from context,
/// else the thread-local one, else the empty string (never a panic).
#[inline(never)]
pub(crate) fn with_html_text<R>(id: MsgId, f: impl FnOnce(&str) -> R) -> R {
    if let Some(c) = use_context::<ServerCatalog>() {
        return f(c.0.get(id.0 as usize).map_or("", |s| s));
    }
    let cat = active();
    f(cat.as_deref().map_or("", |c| c.get(id)))
}

/// Installs a catalog and updates every live translated node: the registry
/// walk (strategy B) or the effects' trigger (strategy A), then the
/// conversions (`TextProp`, `Signal<String>`, `to_string` under observers).
#[inline(never)]
pub fn set_catalog(texts: Vec<Box<str>>) {
    let _ = ACTIVE.try_with(|a| {
        if let Ok(mut a) = a.try_borrow_mut() {
            *a = Some(Rc::new(Catalog { texts }));
        }
    });
    crate::update::refresh_all();
    let _ = LOCALE.try_with(Notify::notify);
}

/// Client boot for the probe: `id`-ordered lines from `<html data-catalog>`
/// (stands in for fetching the preloaded catalog).
pub fn boot_from_dom() {
    let data = leptos::prelude::document()
        .document_element()
        .and_then(|e| e.get_attribute("data-catalog"));
    let texts: Vec<Box<str>> = data
        .map(|d| d.lines().map(Box::from).collect())
        .unwrap_or_default();
    set_catalog(texts);
}
