//! The reader's time zone.
//!
//! A date is shown in its own zone, else **the reader's**, else
//! `Setup::with_time_zone`'s, else UTC. The server cannot know the reader's
//! zone, so the client tells it: the browser's IANA name goes into the
//! `mf2_tz` cookie, `mf2::axum` puts it in the request's context, and the page
//! states the zone it was rendered in (`data-mf2-zone` on the preload link).
//!
//! **One formatting zone per thread.** [`current`] is what
//! `state::context_for` puts in every `FormatContext`. The client sets it
//! once at boot — the page's zone while hydrating, the reader's after; a
//! server sets it around each format from the request's `RequestI18n`
//! ([`scoped`]): a format is synchronous, so the scope is exact and costs no
//! second context lookup.
//!
//! **The correction** (client, with a server): hydration runs in the page's
//! zone, so everything formatted while hydrating agrees with the served
//! HTML. After it, the zone becomes the reader's, and exactly the nodes whose
//! text changes are rewritten — found by formatting each in both zones.

use mf2_runtime::TimeZone;

#[cfg(all(feature = "hydrate", feature = "fn-datetime"))]
use alloc::vec::Vec;

#[cfg(any(feature = "ssr", feature = "fn-datetime"))]
std::thread_local! {
    /// The formatting zone, when it is not `Setup`'s.
    static ZONE: core::cell::Cell<Option<TimeZone>> = const { core::cell::Cell::new(None) };
}

/// The zone a format on this thread uses, if not `Setup`'s.
#[cfg(any(feature = "ssr", feature = "fn-datetime"))]
pub(crate) fn current() -> Option<TimeZone> {
    ZONE.with(core::cell::Cell::get)
}

/// Sets the zone for everything formatted on this thread from now on.
#[cfg(all(any(feature = "hydrate", feature = "csr"), feature = "fn-datetime"))]
fn set(zone: Option<TimeZone>) {
    ZONE.with(|z| z.set(zone));
}

/// Runs `body` with `zone` as the formatting zone, then restores the one
/// before. `None` leaves the zone as it is.
#[cfg(any(feature = "ssr", all(feature = "hydrate", feature = "fn-datetime")))]
pub(crate) fn scoped<R>(zone: Option<TimeZone>, body: impl FnOnce() -> R) -> R {
    let Some(zone) = zone else {
        return body();
    };
    let before = ZONE.with(|z| z.replace(Some(zone)));
    let out = body();
    ZONE.with(|z| z.set(before));
    out
}

/// The reader's time zone named `name` — the browser's
/// `Intl.DateTimeFormat().resolvedOptions().timeZone`, or the `mf2_tz`
/// cookie a server received — if it can be used: a well-formed IANA name of
/// at most 64 bytes **that the installed host knows**. Anything else is
/// `None`, which means "not known", never an error.
///
/// Without the `fn-datetime` feature nothing is accepted: an application
/// without dates reads no zone and states none.
#[must_use]
pub fn reader_time_zone(name: &str) -> Option<TimeZone> {
    #[cfg(feature = "fn-datetime")]
    {
        let zone = TimeZone::named(name)?;
        let host = crate::leptos::state::setup()?.host;
        host.zone_offset(name, 0).map(|_| zone)
    }
    #[cfg(not(feature = "fn-datetime"))]
    {
        let _ = name;
        None
    }
}

/// The zone's IANA name, if it is a named zone.
#[cfg(any(feature = "ssr", all(feature = "hydrate", feature = "fn-datetime")))]
pub(crate) fn zone_name(zone: &TimeZone) -> Option<&str> {
    match zone.as_option() {
        mf2_runtime::ZoneOption::Named(name) => Some(name),
        _ => None,
    }
}

// ---------------------------------------------------------------- client ---

/// The browser's zone, as a reader zone this build can use.
#[cfg(all(any(feature = "hydrate", feature = "csr"), feature = "fn-datetime"))]
fn browser_zone() -> Option<(TimeZone, alloc::string::String)> {
    let format = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &js_sys::Object::new());
    let name = js_sys::Reflect::get(
        &format.resolved_options(),
        &wasm_bindgen::JsValue::from_str("timeZone"),
    )
    .ok()?
    .as_string()?;
    reader_time_zone(&name).map(|zone| (zone, name))
}

/// A client-only application's boot: the reader's zone, before anything
/// mounts. There is no server, so no cookie and nothing to correct.
#[cfg(all(feature = "csr", feature = "fn-datetime"))]
pub(crate) fn mount_in_reader_zone() {
    if let Some((zone, _)) = browser_zone() {
        set(Some(zone));
    }
}

#[cfg(all(feature = "hydrate", feature = "fn-datetime"))]
mod correction {
    use super::{TimeZone, Vec, browser_zone, set, zone_name};
    use alloc::rc::Rc;
    use core::cell::{Cell, RefCell};

    use crate::leptos::catalog;
    use crate::leptos::registry::{Relocalize, Target};
    use crate::leptos::text::Stored;
    use crate::line::reactive_graph;

    /// Where the correction is.
    #[derive(Clone, Copy)]
    enum Phase {
        /// Nothing to correct: the page was rendered in the reader's zone,
        /// or the reader's is not known.
        Idle,
        /// Hydrating in the page's zone; every hydrated node is queued, and
        /// `reader` is the zone to switch to afterwards.
        Hydrating { reader: TimeZone },
        /// Switched to the reader's zone. A node that hydrates now — a lazy
        /// route's chunk, a `Suspense`, an island — still holds text in
        /// `page`, and is corrected as it registers.
        Corrected { page: TimeZone },
    }

    /// A hydrated node, held until the page has hydrated.
    enum Queued {
        Value(Target, Stored),
        Rich(Rc<RefCell<dyn Relocalize>>),
    }

    std::thread_local! {
        static PHASE: Cell<Phase> = const { Cell::new(Phase::Idle) };
        static QUEUE: RefCell<Vec<Queued>> = const { RefCell::new(Vec::new()) };
    }

    fn phase() -> Phase {
        PHASE.with(Cell::get)
    }

    /// The zone the page states it was rendered in: its `data-mf2-zone`,
    /// else `Setup`'s (the client has the same `Setup`).
    fn page_zone() -> (Option<TimeZone>, TimeZone) {
        let stated = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| {
                d.query_selector(
                    &[
                        "link[",
                        crate::leptos::links::PRELOAD_ATTR,
                        "][",
                        crate::leptos::links::ZONE_ATTR,
                        "]",
                    ]
                    .concat(),
                )
                .ok()
                .flatten()
            })
            .and_then(|link| link.get_attribute(crate::leptos::links::ZONE_ATTR))
            .and_then(|name| TimeZone::named(&name));
        let setup = crate::leptos::state::setup().map_or(TimeZone::UTC, |s| s.time_zone);
        (stated, stated.unwrap_or(setup))
    }

    /// Whether `a` and `b` are the same zone to a reader: the same name,
    /// or UTC and a name for it.
    fn same(a: &TimeZone, b: &TimeZone) -> bool {
        if a == b {
            return true;
        }
        let utc =
            |z: &TimeZone| *z == TimeZone::UTC || matches!(zone_name(z), Some("UTC" | "Etc/UTC"));
        utc(a) && utc(b)
    }

    /// Before hydration: format in the page's zone, and remember the
    /// reader's if it differs. `islands` switches at once instead — Leptos
    /// walks the islands itself, so there is no "after" to wait for.
    pub(crate) fn before_hydration(islands: bool) {
        let (stated, page) = page_zone();
        set(stated);
        let Some((reader, name)) = browser_zone() else {
            return;
        };
        if same(&page, &reader) {
            return;
        }
        if islands {
            set(Some(reader));
            PHASE.with(|p| p.set(Phase::Corrected { page }));
            remember(&name);
        } else {
            PHASE.with(|p| p.set(Phase::Hydrating { reader }));
        }
    }

    /// After the synchronous hydration: the reader's zone, the queued nodes
    /// brought up to date, the conversions re-read, the cookie written.
    pub(crate) fn after_hydration() {
        let Phase::Hydrating { reader } = phase() else {
            return;
        };
        let page = super::current().unwrap_or_else(|| page_zone().1);
        set(Some(reader));
        PHASE.with(|p| p.set(Phase::Corrected { page }));
        let queued = QUEUE.with(|q| core::mem::take(&mut *q.borrow_mut()));
        if let Some(catalog) = catalog::active() {
            for node in &queued {
                match node {
                    Queued::Value(target, desc) => target.correct_zone(desc, &catalog, page),
                    Queued::Rich(node) => {
                        if let Ok(mut node) = node.try_borrow_mut() {
                            node.relocalize(&catalog);
                        }
                    }
                }
            }
        }
        drop(queued);
        reactive_graph::traits::Notify::notify(&catalog::changed());
        if let Some(name) = zone_name(&reader) {
            remember(name);
        }
    }

    /// The cookie the server renders the next page in.
    fn remember(name: &str) {
        if let Some(window) = web_sys::window() {
            crate::leptos::boot::write_zone_cookie(&window, name);
        }
    }

    /// A text, attribute or property node has hydrated, holding the
    /// server's text.
    pub(crate) fn hydrated(target: &Target, desc: &Stored) {
        match phase() {
            Phase::Idle => {}
            Phase::Hydrating { .. } => {
                QUEUE.with(|q| {
                    q.borrow_mut()
                        .push(Queued::Value(target.clone(), desc.clone()));
                });
            }
            Phase::Corrected { page } => {
                if let Some(catalog) = catalog::active() {
                    target.correct_zone(desc, &catalog, page);
                }
            }
        }
    }

    /// The zone a markup message hydrates in, if it is not the current one:
    /// the page's, once the correction has switched away from it.
    pub(crate) fn hydrating_zone() -> Option<TimeZone> {
        match phase() {
            Phase::Corrected { page } => Some(page),
            _ => None,
        }
    }

    /// A markup message has hydrated. While the page hydrates it is queued;
    /// after, it is rebuilt now (tachys rewrites only the text that
    /// changed).
    pub(crate) fn hydrated_rich(node: Rc<RefCell<dyn Relocalize>>) {
        match phase() {
            Phase::Idle => {}
            Phase::Hydrating { .. } => {
                QUEUE.with(|q| q.borrow_mut().push(Queued::Rich(node)));
            }
            Phase::Corrected { .. } => {
                if let (Some(catalog), Ok(mut node)) = (catalog::active(), node.try_borrow_mut()) {
                    node.relocalize(&catalog);
                }
            }
        }
    }
}

#[cfg(all(feature = "hydrate", feature = "fn-datetime"))]
pub(crate) use correction::{
    after_hydration, before_hydration, hydrated, hydrated_rich, hydrating_zone,
};
