//! The node registry: one slab slot per rendered description, walked on a
//! locale switch.
//!
//! Every rendered description takes one slab slot: the DOM node it wrote,
//! and the description, so that a locale switch can write it again. The view
//! state is the **slot index** — four bytes — and dropping it frees the slot
//! in O(1).
//!
//! The alternative P0.11 measured, one `RenderEffect` per node tracking a
//! global trigger, costs 427 B and 10 allocations per node on wasm32 against
//! this one's 44.8 B and ≈ 0, and leaks: a dropped effect stays in the
//! trigger's subscriber set until the trigger next fires, and locale
//! switches are rare, so a churning list leaks without bound. A slab has no
//! effect, no `Owner` and no task per node.
//!
//! A **reactive argument** is the one subscription that is inherent, and it
//! is per node rather than per call site: a slot whose description holds an
//! [`ArgValue::Source`](crate::ArgValue::Source) owns one `RenderEffect`
//! that tracks the arguments only — the locale is this slab's business. It
//! unsubscribes from them when the slot lets it go ([`ArgsEffect`]): a
//! dropped effect otherwise stays in each source's subscriber set until that
//! source next changes, which is P0.11's leak again, keyed on the
//! application's signal instead of the locale (Phase 7 A5: ≈ 70 B per
//! churned row on wasm32 for a row whose argument reads one shared signal).
//!
//! With the `static-locale` feature (strategy C) a switch is a cookie and a
//! navigation, which is the natural fit for islands, so the locale needs no
//! bookkeeping: only a node with a **reactive argument** registers, because
//! its argument effect needs an owner and a slot to be dropped from. Every
//! other node registers nothing.

use alloc::boxed::Box;
#[cfg(not(feature = "static-locale"))]
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::line::{reactive_graph, tachys};
use mf2_catalog::Catalog;
use tachys::renderer::Rndr;
use tachys::renderer::types::{Element, Text};

use crate::leptos::catalog;
use crate::leptos::state::{self, TextUse};
use reactive_graph::effect::RenderEffect;
use reactive_graph::graph::{Subscriber, ToAnySubscriber, untrack};

use crate::leptos::text::{self, Stored};

/// No slot: what a registration returns under `static-locale`, and what an
/// unregistered view state holds.
pub(crate) const NONE: u32 = u32::MAX;

/// Where a description wrote its text, and how to write it again.
///
/// Cloning one clones a DOM handle (a `JsValue` index) and, for an
/// attribute, its key — which is what the argument effect below holds.
#[derive(Clone)]
pub(crate) enum Target {
    /// A text child — with `mark-fallback-lang`, and the wrapper its view
    /// state shares, which a write fits to the message's lender.
    Text(
        Text,
        #[cfg(feature = "mark-fallback-lang")] crate::leptos::lang::Wrapper,
    ),
    /// An attribute of an element (`title`, `placeholder`, `value`), whose
    /// name decides its isolation ([`state::attribute_use`]).
    Attribute(Element, Box<str>),
    /// A DOM property (`prop:value`), which is plain text: no bidi
    /// isolation (§9).
    Property(Element, Box<str>),
}

impl Target {
    /// What the text in this position is for, which decides bidi isolation.
    fn text_use(&self) -> TextUse {
        match self {
            Target::Text(..) => TextUse::Displayed,
            Target::Attribute(_, key) => state::attribute_use(key),
            Target::Property(..) => TextUse::Plain,
        }
    }

    /// Writes `text` where this target is.
    fn write(&self, text: &str) {
        match self {
            Target::Text(node, ..) => Rndr::set_text(node, text),
            Target::Attribute(el, key) => Rndr::set_attribute(el, key, text),
            Target::Property(el, key) => {
                Rndr::set_property_or_value(el, key, &wasm_bindgen::JsValue::from_str(text));
            }
        }
    }

    /// Formats `desc` against `catalog` and writes it here — every write
    /// the registry makes goes through this. With `mark-fallback-lang`, a
    /// text child also fits its wrapper to the message's lender.
    fn render(&self, desc: &Stored, catalog: &Catalog) {
        #[cfg(feature = "mark-fallback-lang")]
        if let Target::Text(node, wrapper) = self {
            text::with_marked_text(desc, catalog, |text, lender| {
                Rndr::set_text(node, text);
                wrapper.fit(node, lender);
            });
            return;
        }
        text::with_text(desc, catalog, self.text_use(), |text| self.write(text));
    }

    /// Rewrites this node if its text in the zone now in force differs from
    /// its text in `page`, the zone it was rendered in — the reader's-zone
    /// correction (`crate::leptos::zone`). A node whose text does not depend on the
    /// zone is not written at all, unless `always`: the server's date
    /// formatter was not the client's, so the node holds text the client
    /// would not write, and it is written whatever the zone. The correction
    /// calls it only for a message that formats a date.
    #[cfg(all(feature = "hydrate", feature = "datetime"))]
    pub(crate) fn correct_zone(
        &self,
        desc: &Stored,
        catalog: &Catalog,
        page: mf2_runtime::TimeZone,
        always: bool,
    ) {
        let use_ = self.text_use();
        if always {
            text::with_text(desc, catalog, use_, |now| self.write(now));
            return;
        }
        let before = crate::leptos::zone::scoped(Some(page), || {
            text::with_text(desc, catalog, use_, |t: &str| {
                alloc::string::String::from(t)
            })
        });
        text::with_text(desc, catalog, use_, |now| {
            // The lender does not depend on the zone, so a marked text's
            // wrapper already fits: only the text changes.
            if now != before {
                self.write(now);
            }
        });
    }
}

/// A node that rebuilds itself rather than rewriting one string: a rich
/// message, whose *structure* comes from the catalog (§7).
///
/// Only a build that runs in a browser calls it — a server renders each
/// request once — so elsewhere it is registered and never used. Under
/// `static-locale` nothing follows the locale, so nothing implements it.
#[cfg(any(
    not(feature = "static-locale"),
    all(feature = "hydrate", feature = "datetime")
))]
#[cfg_attr(not(any(feature = "hydrate", feature = "csr")), allow(dead_code))]
pub(crate) trait Relocalize {
    /// Rebuilds against `catalog`.
    fn relocalize(&mut self, catalog: &Catalog);

    /// Rebuilds against `catalog` and writes every text, even one the
    /// rebuild thinks unchanged: after hydration under another date
    /// formatter than the server's, the view's state holds the client's
    /// text and the page the server's (`crate::leptos::zone`).
    #[cfg(all(feature = "hydrate", feature = "datetime"))]
    fn rewrite(&mut self, catalog: &Catalog);

    /// The message it renders: the correction rewrites it only when that
    /// message formats a date (`crate::leptos::zone`).
    #[cfg(all(feature = "hydrate", feature = "datetime"))]
    fn msg_id(&self) -> mf2_runtime::MsgId;
}

enum Slot {
    /// A description in one text node, attribute or property.
    Value {
        target: Target,
        desc: Stored,
        /// Present only when an argument is reactive.
        args: Option<ArgsEffect>,
    },
    /// A rich message's fragment, shared with its view state so that both
    /// the view tree and a locale switch can reach it. Rich messages are
    /// rare (§7), so the one `Rc` per node is not on the hot path.
    #[cfg(not(feature = "static-locale"))]
    Rich(
        #[cfg_attr(not(any(feature = "hydrate", feature = "csr")), allow(dead_code))]
        Rc<RefCell<dyn Relocalize>>,
    ),
}

impl Slot {
    /// Brings this node up to date with `catalog`. The client's path: a
    /// server renders each request once and never updates a node.
    #[cfg_attr(
        any(
            not(any(feature = "hydrate", feature = "csr")),
            all(feature = "csr", feature = "static-locale")
        ),
        allow(dead_code)
    )]
    fn apply(&self, catalog: &Catalog) {
        match self {
            Slot::Value { target, desc, .. } => target.render(desc, catalog),
            #[cfg(not(feature = "static-locale"))]
            Slot::Rich(node) => {
                if let Ok(mut node) = node.try_borrow_mut() {
                    node.relocalize(catalog);
                }
            }
        }
    }
}

enum Cell {
    /// Free, with the index of the next free cell (or [`NONE`]).
    Free(u32),
    Live(Slot),
}

struct Slab {
    cells: Vec<Cell>,
    free: u32,
    live: u32,
}

impl Slab {
    const fn new() -> Slab {
        Slab {
            cells: Vec::new(),
            free: NONE,
            live: 0,
        }
    }

    fn insert(&mut self, slot: Slot) -> u32 {
        self.live = self.live.saturating_add(1);
        if let Some(cell) = usize::try_from(self.free)
            .ok()
            .and_then(|i| self.cells.get_mut(i))
            && let Cell::Free(next) = *cell
        {
            let index = self.free;
            self.free = next;
            *cell = Cell::Live(slot);
            return index;
        }
        let Ok(index) = u32::try_from(self.cells.len()) else {
            // 4 billion live nodes: the slot is not registered, and the
            // node simply does not follow a locale switch. Not a panic.
            self.live = self.live.saturating_sub(1);
            return NONE;
        };
        if self.cells.try_reserve(1).is_err() {
            self.live = self.live.saturating_sub(1);
            return NONE;
        }
        self.cells.push(Cell::Live(slot));
        index
    }

    fn remove(&mut self, index: u32) {
        let Ok(i) = usize::try_from(index) else {
            return;
        };
        let free = self.free;
        if let Some(cell @ Cell::Live(_)) = self.cells.get_mut(i) {
            *cell = Cell::Free(free);
            self.free = index;
            self.live = self.live.saturating_sub(1);
        }
    }

    fn get_mut(&mut self, index: u32) -> Option<&mut Slot> {
        let i = usize::try_from(index).ok()?;
        match self.cells.get_mut(i)? {
            Cell::Live(slot) => Some(slot),
            Cell::Free(_) => None,
        }
    }
}

std::thread_local! {
    static NODES: RefCell<Slab> = const { RefCell::new(Slab::new()) };
}

/// Registers a rendered description and returns its slot index.
///
/// Under `static-locale` only a description with a reactive argument is
/// stored; any other gets [`NONE`].
pub(crate) fn insert(target: Target, desc: Stored) -> u32 {
    #[cfg(feature = "static-locale")]
    if !desc.has_source() {
        return NONE;
    }
    let args = args_effect(&target, &desc);
    add(Slot::Value { target, desc, args })
}

/// [`insert`] for a node that has just hydrated, holding the server's text:
/// the reader's-zone correction sees it first (`crate::leptos::zone`).
pub(crate) fn insert_hydrated(target: Target, desc: Stored) -> u32 {
    // Untracked, as every format outside the node's own effect: see
    // `replace`.
    #[cfg(all(feature = "hydrate", feature = "datetime"))]
    untrack(|| crate::leptos::zone::hydrated(&target, &desc));
    insert(target, desc)
}

/// Registers a rich message's fragment (§7). Not under `static-locale`,
/// where a rich message's structure never changes after it is built.
#[cfg(not(feature = "static-locale"))]
pub(crate) fn insert_rich(node: Rc<RefCell<dyn Relocalize>>) -> u32 {
    add(Slot::Rich(node))
}

fn add(slot: Slot) -> u32 {
    NODES.with(|slab| match slab.try_borrow_mut() {
        Ok(mut slab) => slab.insert(slot),
        Err(_) => NONE,
    })
}

/// Frees slot `index`.
pub(crate) fn remove(index: u32) {
    if index == NONE {
        return;
    }
    NODES.with(|slab| {
        if let Ok(mut slab) = slab.try_borrow_mut() {
            slab.remove(index);
        }
    });
}

/// Replaces slot `*index`'s description and writes its text again — what
/// `rebuild` does when an enclosing closure produced a different message.
///
/// `target` is asked for only when there is no slot to find it in, which
/// under `static-locale` is the usual case: the node registered nothing, so
/// the new text is written straight to it — or, if the new description has
/// a reactive argument, it registers now.
///
/// The write is **untracked**: the node's argument effect is what follows
/// its signals. Read in the enclosing render, they would also re-run that
/// render — the closure this rebuild came from, or a lazy route's view,
/// replaced whole with fresh state (Phase 9 B1).
pub(crate) fn replace(index: &mut u32, target: impl FnOnce() -> Option<Target>, desc: Stored) {
    untrack(move || replace_now(index, target, desc));
}

fn replace_now(index: &mut u32, target: impl FnOnce() -> Option<Target>, desc: Stored) {
    let Some(catalog) = catalog::active() else {
        return;
    };
    if *index == NONE {
        #[cfg(feature = "static-locale")]
        if let Some(target) = target() {
            if desc.has_source() {
                // The argument effect writes the text as it subscribes.
                *index = insert(target, desc);
            } else {
                target.render(&desc, &catalog);
            }
        }
        #[cfg(not(feature = "static-locale"))]
        let _ = (target, desc);
        return;
    }
    let index = *index;
    NODES.with(|slab| {
        if let Ok(mut slab) = slab.try_borrow_mut()
            && let Some(Slot::Value {
                target,
                desc: stored,
                args,
            }) = slab.get_mut(index)
        {
            *stored = desc;
            *args = args_effect(target, stored);
            target.render(stored, &catalog);
        }
    });
}

/// Re-formats every live node against the catalog now active: the whole cost
/// of a locale switch, walked synchronously (P0.11: 2.3–3.5 ms for 2,000
/// nodes, 6.8 ms at 4× CPU throttle).
///
/// Under `static-locale` there is no switch, and only an islands page whose
/// catalog landed after an island hydrated calls this.
#[cfg(any(feature = "hydrate", feature = "csr"))]
#[cfg_attr(all(feature = "csr", feature = "static-locale"), allow(dead_code))]
pub(crate) fn relocalize_all() {
    let Some(catalog) = catalog::active() else {
        return;
    };
    NODES.with(|slab| {
        if let Ok(slab) = slab.try_borrow() {
            for cell in &slab.cells {
                if let Cell::Live(slot) = cell {
                    slot.apply(&catalog);
                }
            }
        }
    });
}

/// How many nodes follow the locale. The browser checks read it to know that
/// hydration has finished and that dropped nodes freed their slots.
#[must_use]
pub fn live_nodes() -> usize {
    NODES.with(|slab| {
        slab.try_borrow()
            .map_or(0, |slab| usize::try_from(slab.live).unwrap_or(0))
    })
}

/// A node's argument effect, which leaves its sources' subscriber sets when
/// it is dropped — when the slot is freed, or replaced by a rebuild.
///
/// `reactive_graph` 0.2 removes an effect from its sources only when it
/// re-runs; dropping one leaves a dead entry (and the allocation it points
/// to) in every source it read, until that source next notifies. An
/// argument is typically an application signal shared by many rows — a
/// count, a user — that may not change for the whole session.
struct ArgsEffect(RenderEffect<()>);

impl Drop for ArgsEffect {
    fn drop(&mut self) {
        let subscriber = self.0.to_any_subscriber();
        subscriber.clear_sources(&subscriber);
    }
}

/// One effect per node **with a reactive argument**, tracking the arguments
/// and nothing else (§4). A description with no source registers none, which
/// is almost all of them — and the effect is the library's one closure type,
/// not one per call site (P0.1).
fn args_effect(target: &Target, desc: &Stored) -> Option<ArgsEffect> {
    if !desc.has_source() {
        return None;
    }
    let target = target.clone();
    let desc = desc.clone();
    // `RenderEffect::new` runs the closure at once, which is what subscribes
    // to the sources; the write it makes is the one `build` just made.
    Some(ArgsEffect(RenderEffect::new(move |_| {
        if let Some(catalog) = catalog::active() {
            target.render(&desc, &catalog);
        }
    })))
}
