//! Server side (feature `ssr`): the catalog is per-request **context**, looked
//! up when a `Tr` renders (D9). With no request — route-list generation, a
//! static file served through the error handler — the default locale is used
//! rather than panicking (plans/04 §1, §5).
#![allow(clippy::expect_used)] // server-only: not on the client path

use crate::catalog::Catalog;
use leptos::prelude::{provide_context, with_context};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};

/// Per-request i18n state: a plain `Arc<Catalog>` from a process-wide cache
/// (no signal) and the catalog's content-hashed URL for the preload link.
#[derive(Clone, Debug)]
pub struct RequestI18n {
    pub catalog: Arc<Catalog>,
    pub href: Arc<str>,
}

static DEFAULT: OnceLock<RequestI18n> = OnceLock::new();

/// Counts renders that found no request context and fell back to the default
/// locale. The probe exposes it so the streaming test can assert it stays 0.
pub static CONTEXT_MISSES: AtomicUsize = AtomicUsize::new(0);
/// Counts render-time lookups that did find the request context.
pub static CONTEXT_HITS: AtomicUsize = AtomicUsize::new(0);

/// Installs the process-wide default (the fallback when no context exists).
pub fn install_default(default: RequestI18n) {
    let _ = DEFAULT.set(default);
}

/// Provides this request's i18n state. Call it from the `additional_context`
/// closure passed to **every** leptos_axum `*_with_context` entry point.
pub fn provide(state: RequestI18n) {
    provide_context(state);
}

/// This request's state, or the default when no context is reachable.
pub fn current() -> RequestI18n {
    if let Some(r) = with_context::<RequestI18n, _>(Clone::clone) {
        return r;
    }
    CONTEXT_MISSES.fetch_add(1, Ordering::Relaxed);
    DEFAULT.get().cloned().expect("tr::server::install_default was not called")
}

pub(crate) fn with_text<R>(id: u32, f: impl FnOnce(&str) -> R) -> R {
    let mut f = Some(f);
    // Borrow through the context without cloning the Arc.
    let hit = with_context::<RequestI18n, _>(|r| {
        CONTEXT_HITS.fetch_add(1, Ordering::Relaxed);
        f.take().map(|f| f(r.catalog.get(id)))
    })
    .flatten();
    if let Some(out) = hit {
        return out;
    }
    CONTEXT_MISSES.fetch_add(1, Ordering::Relaxed);
    let f = f.expect("closure not consumed");
    match DEFAULT.get() {
        Some(d) => f(d.catalog.get(id)),
        None => f(""),
    }
}
