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
//! text changes are rewritten — found by formatting each date message in
//! both zones.
//!
//! **The formatter** (client, with a server): the page also states the
//! server's date formatter (`data-mf2-dates`). When it is not the client's —
//! ISO on the server and `Intl` in the browser, the smallest server — the
//! served dates are the server's text, which the client never produces, so
//! the same queue rewrites **every date message** it hydrated, whatever the
//! zone. ICU4X on the server and `Intl` in the browser is the one difference
//! that is left alone: the server's text is a localized date already, and
//! stays until its node next updates (`plan/08` §4.3).
//!
//! **Only date messages** (owner, 2026-10-04): both act only on a message
//! that calls a date function — `:datetime`, `:date`, `:time`, or one the
//! application's registry marks (`Function::formats_dates`) — read from the
//! catalog's FUNCS and the message's tags, never from the page's text. Any
//! other node keeps the server's text, a number the server wrote otherwise
//! included, and is not formatted again.

use mf2_runtime::TimeZone;

#[cfg(all(feature = "hydrate", feature = "datetime"))]
use alloc::vec::Vec;

#[cfg(any(feature = "ssr", feature = "datetime"))]
std::thread_local! {
    /// The formatting zone, when it is not `Setup`'s.
    static ZONE: core::cell::Cell<Option<TimeZone>> = const { core::cell::Cell::new(None) };
}

/// The zone a format on this thread uses, if not `Setup`'s.
#[cfg(any(feature = "ssr", feature = "datetime"))]
pub(crate) fn current() -> Option<TimeZone> {
    ZONE.with(core::cell::Cell::get)
}

/// Sets the zone for everything formatted on this thread from now on.
#[cfg(all(any(feature = "hydrate", feature = "csr"), feature = "datetime"))]
fn set(zone: Option<TimeZone>) {
    ZONE.with(|z| z.set(zone));
}

/// Runs `body` with `zone` as the formatting zone, then restores the one
/// before. `None` leaves the zone as it is.
#[cfg(any(feature = "ssr", all(feature = "hydrate", feature = "datetime")))]
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
/// Without the `datetime` feature nothing is accepted: an application
/// without dates reads no zone and states none.
#[must_use]
pub fn reader_time_zone(name: &str) -> Option<TimeZone> {
    #[cfg(feature = "datetime")]
    {
        let zone = TimeZone::named(name)?;
        let host = crate::leptos::state::setup()?.host;
        host.zone_offset(name, 0).map(|_| zone)
    }
    #[cfg(not(feature = "datetime"))]
    {
        let _ = name;
        None
    }
}

/// The zone's IANA name, if it is a named zone.
#[cfg(any(feature = "ssr", all(feature = "hydrate", feature = "datetime")))]
pub(crate) fn zone_name(zone: &TimeZone) -> Option<&str> {
    match zone.as_option() {
        mf2_runtime::ZoneOption::Named(name) => Some(name),
        _ => None,
    }
}

// ---------------------------------------------------------------- client ---

/// The browser's zone, as a reader zone this build can use.
#[cfg(all(any(feature = "hydrate", feature = "csr"), feature = "datetime"))]
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
#[cfg(all(feature = "csr", feature = "datetime"))]
pub(crate) fn mount_in_reader_zone() {
    if let Some((zone, _)) = browser_zone() {
        set(Some(zone));
    }
}

#[cfg(all(feature = "hydrate", feature = "datetime"))]
mod correction {
    use super::{TimeZone, Vec, browser_zone, set, zone_name};
    use alloc::rc::Rc;
    use core::cell::{Cell, RefCell};

    use crate::leptos::catalog;
    use crate::leptos::registry::{Relocalize, Target};
    use crate::leptos::text::Stored;
    use crate::line::reactive_graph;
    use mf2_runtime::MsgId;

    /// Where the correction is.
    #[derive(Clone, Copy)]
    enum Phase {
        /// Nothing to correct: the page was rendered in the reader's zone,
        /// or the reader's is not known, and in the client's formatter.
        Idle,
        /// Hydrating in the page's zone; every hydrated node is queued, and
        /// `reader` is the zone to switch to afterwards — the page's own
        /// when only the formatter differs, in which case `moved` is false
        /// and no cookie is written.
        Hydrating { reader: TimeZone, moved: bool },
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
        /// The server's date formatter is not the client's: every queued
        /// node is rewritten, not only those whose text the zone changes.
        static REWRITE: Cell<bool> = const { Cell::new(false) };
    }

    fn rewrite() -> bool {
        REWRITE.with(Cell::get)
    }

    /// Whether message `id` formats a date: it calls a function the
    /// registry marks as a date function (`plan/08` §4.3). A catalog whose
    /// FUNCS names none has none. Without a registry nothing is known, so
    /// every message counts.
    fn formats_date(catalog: &mf2_catalog::Catalog, id: MsgId) -> bool {
        let Some(registry) = crate::leptos::state::registry() else {
            return true;
        };
        let is_date = |name: &str| registry.is_date_function(name);
        catalog.names_function(is_date) && catalog.calls(id, is_date)
    }

    /// Corrects one hydrated text, attribute or property node, if its
    /// message formats a date.
    fn correct(target: &Target, desc: &Stored, catalog: &mf2_catalog::Catalog, page: TimeZone) {
        if formats_date(catalog, desc.msg_id()) {
            target.correct_zone(desc, catalog, page, rewrite());
        }
    }

    /// The page's preload link, which carries what the server states.
    fn preload_link() -> Option<web_sys::Element> {
        web_sys::window().and_then(|w| w.document()).and_then(|d| {
            d.query_selector(&["link[", crate::leptos::links::PRELOAD_ATTR, "]"].concat())
                .ok()
                .flatten()
        })
    }

    /// Whether the dates in the page are not what this client would write:
    /// the server states a formatter that is not the client's, other than
    /// ICU4X under `Intl` (left alone, §4.3). A page that states none
    /// formats no dates on the server.
    fn formatter_differs(link: Option<&web_sys::Element>) -> bool {
        let Some(own) = crate::leptos::links::date_formatter() else {
            return false;
        };
        let Some(page) = link.and_then(|l| l.get_attribute(crate::leptos::links::DATES_ATTR))
        else {
            return false;
        };
        let page = page.as_str();
        page != own && !(page == "icu" && own == "intl")
    }

    fn phase() -> Phase {
        PHASE.with(Cell::get)
    }

    /// The zone the page states it was rendered in: its `data-mf2-zone`,
    /// else `Setup`'s (the client has the same `Setup`).
    fn page_zone(link: Option<&web_sys::Element>) -> (Option<TimeZone>, TimeZone) {
        let stated = link
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
    /// reader's if it differs, or that the server's formatter does.
    /// `islands` switches at once instead — Leptos walks the islands
    /// itself, so there is no "after" to wait for.
    pub(crate) fn before_hydration(islands: bool) {
        let link = preload_link();
        let (stated, page) = page_zone(link.as_ref());
        set(stated);
        let differs = formatter_differs(link.as_ref());
        REWRITE.with(|r| r.set(differs));
        let moved = browser_zone().filter(|(reader, _)| !same(&page, reader));
        if moved.is_none() && !differs {
            return;
        }
        if islands {
            match moved {
                Some((reader, name)) => {
                    set(Some(reader));
                    remember(&name);
                }
                None => set(Some(page)),
            }
            PHASE.with(|p| p.set(Phase::Corrected { page }));
        } else {
            let (reader, moved) = match moved {
                Some((reader, _)) => (reader, true),
                None => (page, false),
            };
            PHASE.with(|p| p.set(Phase::Hydrating { reader, moved }));
        }
    }

    /// After the synchronous hydration: the reader's zone, the queued nodes
    /// brought up to date, the conversions re-read, the cookie written.
    pub(crate) fn after_hydration() {
        let Phase::Hydrating { reader, moved } = phase() else {
            return;
        };
        let page = super::current().unwrap_or_else(|| page_zone(preload_link().as_ref()).1);
        set(Some(reader));
        PHASE.with(|p| p.set(Phase::Corrected { page }));
        let queued = QUEUE.with(|q| core::mem::take(&mut *q.borrow_mut()));
        if let Some(catalog) = catalog::active() {
            for node in &queued {
                match node {
                    Queued::Value(target, desc) => correct(target, desc, &catalog, page),
                    Queued::Rich(node) => {
                        if let Ok(mut node) = node.try_borrow_mut() {
                            relocalize(&mut *node, &catalog);
                        }
                    }
                }
            }
        }
        drop(queued);
        reactive_graph::traits::Notify::notify(&catalog::changed());
        if let Some(name) = zone_name(&reader).filter(|_| moved) {
            remember(name);
        }
    }

    /// Brings a markup message up to date, if it formats a date: rebuilt,
    /// or, when the server's formatter is not the client's, rewritten whole
    /// (`Relocalize::rewrite`). Any other markup message is left as it
    /// hydrated: no empty-then-full rebuild.
    fn relocalize(node: &mut dyn Relocalize, catalog: &mf2_catalog::Catalog) {
        if !formats_date(catalog, node.msg_id()) {
            return;
        }
        if rewrite() {
            node.rewrite(catalog);
        } else {
            node.relocalize(catalog);
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
                    correct(target, desc, &catalog, page);
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
                    relocalize(&mut *node, &catalog);
                }
            }
        }
    }
}

#[cfg(all(feature = "hydrate", feature = "datetime"))]
pub(crate) use correction::{
    after_hydration, before_hydration, hydrated, hydrated_rich, hydrating_zone,
};
