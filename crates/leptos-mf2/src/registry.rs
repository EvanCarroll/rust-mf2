//! The node registry — decision D7, strategy **B**
//! (`plans/04-leptos-integration.md` §4; probe P0.11).
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
//! that tracks the arguments only — the locale is this slab's business.
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

use mf2_catalog::Catalog;
use tachys::renderer::Rndr;
use tachys::renderer::types::{Element, Text};

use crate::catalog;
use crate::state::TextUse;
use reactive_graph::effect::RenderEffect;

use crate::text::{self, Stored};

/// No slot: what a registration returns under `static-locale`, and what an
/// unregistered view state holds.
pub(crate) const NONE: u32 = u32::MAX;

/// Where a description wrote its text, and how to write it again.
///
/// Cloning one clones a DOM handle (a `JsValue` index) and, for an
/// attribute, its key — which is what the argument effect below holds.
#[derive(Clone)]
pub(crate) enum Target {
    /// A text child.
    Text(Text),
    /// An attribute of an element (`title`, `placeholder`, `aria-label`).
    Attribute(Element, Box<str>),
    /// A DOM property (`prop:value`), which is plain text: no bidi
    /// isolation (§9).
    Property(Element, Box<str>),
}

impl Target {
    /// What the text in this position is for, which decides bidi isolation.
    fn text_use(&self) -> TextUse {
        match self {
            Target::Text(_) | Target::Attribute(..) => TextUse::Displayed,
            Target::Property(..) => TextUse::Plain,
        }
    }

    /// Writes `text` where this target is.
    fn write(&self, text: &str) {
        match self {
            Target::Text(node) => Rndr::set_text(node, text),
            Target::Attribute(el, key) => Rndr::set_attribute(el, key, text),
            Target::Property(el, key) => {
                Rndr::set_property_or_value(el, key, &wasm_bindgen::JsValue::from_str(text));
            }
        }
    }
}

/// A node that rebuilds itself rather than rewriting one string: a rich
/// message, whose *structure* comes from the catalog (§7).
///
/// Only a build that runs in a browser calls it — a server renders each
/// request once — so elsewhere it is registered and never used. Under
/// `static-locale` nothing follows the locale, so nothing implements it.
#[cfg(not(feature = "static-locale"))]
#[cfg_attr(not(any(feature = "hydrate", feature = "csr")), allow(dead_code))]
pub(crate) trait Relocalize {
    /// Rebuilds against `catalog`.
    fn relocalize(&mut self, catalog: &Catalog);
}

enum Slot {
    /// A description in one text node, attribute or property.
    Value {
        target: Target,
        desc: Stored,
        /// Present only when an argument is reactive.
        args: Option<RenderEffect<()>>,
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
            Slot::Value { target, desc, .. } => {
                text::with_text(desc, catalog, target.text_use(), |text| target.write(text));
            }
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
pub(crate) fn replace(index: &mut u32, target: impl FnOnce() -> Option<Target>, desc: Stored) {
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
                text::with_text(&desc, &catalog, target.text_use(), |text| {
                    target.write(text);
                });
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
            text::with_text(stored, &catalog, target.text_use(), |text| {
                target.write(text);
            });
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

/// One effect per node **with a reactive argument**, tracking the arguments
/// and nothing else (§4). A description with no source registers none, which
/// is almost all of them — and the effect is the library's one closure type,
/// not one per call site (P0.1).
fn args_effect(target: &Target, desc: &Stored) -> Option<RenderEffect<()>> {
    if !desc.has_source() {
        return None;
    }
    let target = target.clone();
    let desc = desc.clone();
    // `RenderEffect::new` runs the closure at once, which is what subscribes
    // to the sources; the write it makes is the one `build` just made.
    Some(RenderEffect::new(move |_| {
        if let Some(catalog) = catalog::active() {
            let use_ = target.text_use();
            text::with_text(&desc, &catalog, use_, |text| {
                target.write(text);
            });
        }
    }))
}
