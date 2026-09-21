//! Strategy B (plans/04 §4, recommended): a library-owned registry.
//!
//! One slab slot per live translated node — `{target, MsgId, args}` — freed
//! in O(1) when the node's view state drops. A locale switch walks the slab.
//! No `RenderEffect`, no `Owner`, no subscription per node; a node with a
//! reactive argument additionally owns one effect that tracks **its
//! arguments only** (never the rarely-firing locale trigger).
//!
//! The view state a call site holds is just the slot index (4 bytes): the
//! node handle and the optional effect live in the slot, so the per-site
//! drop glue is one call.

use std::cell::RefCell;

use leptos::prelude::RenderEffect;
use leptos::tachys::renderer::{Rndr, types};
use leptos::tachys::view::Mountable;

use crate::MsgId;
use crate::args::ArgValue;
use crate::format::{Tracking, has_reactive, with_resolved};
use crate::glue::Target;

struct Entry {
    target: Target,
    id: MsgId,
    args: Box<[ArgValue]>,
    fx: Option<RenderEffect<()>>,
}

enum Slot {
    Used(Entry),
    Free(Option<u32>),
}

struct Registry {
    slots: Vec<Slot>,
    free: Option<u32>,
    live: usize,
}

impl Registry {
    fn insert(&mut self, e: Entry) -> u32 {
        self.live += 1;
        if let Some(i) = self.free
            && let Some(slot) = self.slots.get_mut(i as usize)
            && let Slot::Free(next) = *slot
        {
            *slot = Slot::Used(e);
            self.free = next;
            return i;
        }
        #[allow(clippy::cast_possible_truncation)]
        let i = self.slots.len() as u32;
        self.slots.push(Slot::Used(e));
        i
    }

    fn remove(&mut self, i: u32) -> Option<Entry> {
        let slot = self.slots.get_mut(i as usize)?;
        if !matches!(slot, Slot::Used(_)) {
            return None;
        }
        let old = core::mem::replace(slot, Slot::Free(self.free));
        self.free = Some(i);
        self.live -= 1;
        match old {
            Slot::Used(e) => Some(e),
            Slot::Free(_) => None,
        }
    }

    fn get_mut(&mut self, i: u32) -> Option<&mut Entry> {
        match self.slots.get_mut(i as usize) {
            Some(Slot::Used(e)) => Some(e),
            _ => None,
        }
    }
}

thread_local! {
    static REGISTRY: RefCell<Registry> = const {
        RefCell::new(Registry { slots: Vec::new(), free: None, live: 0 })
    };
}

fn with_registry<R>(f: impl FnOnce(&mut Registry) -> R) -> Option<R> {
    REGISTRY
        .try_with(|r| r.try_borrow_mut().ok().map(|mut r| f(&mut r)))
        .ok()
        .flatten()
}

/// Formats slot `i`; applies the text to its target when `apply`.
#[inline(never)]
fn render_slot(i: u32, tracking: Tracking, apply: bool) {
    with_registry(|r| {
        if let Some(e) = r.get_mut(i) {
            let target = &e.target;
            with_resolved(e.id, &e.args, tracking, &mut |s| {
                if apply {
                    target.apply(s);
                }
            });
        }
    });
}

/// Creates the arguments' effect (outside any registry borrow: its first run
/// is synchronous) and parks it in the slot.
fn attach_effect(slot: u32, rendered: bool) {
    let fx = RenderEffect::new(move |prev: Option<()>| {
        render_slot(slot, Tracking::Tracked, prev.is_some() || !rendered);
    });
    let old = with_registry(|r| r.get_mut(slot).map(|e| e.fx.replace(fx))).flatten();
    drop(old);
}

/// The per-node update handle: a slab slot index.
pub struct Binding {
    slot: u32,
}

impl Binding {
    /// Registers a node. `rendered`: the DOM already shows the right text.
    #[inline(never)]
    pub(crate) fn new(target: Target, id: MsgId, args: Box<[ArgValue]>, rendered: bool) -> Self {
        let reactive = has_reactive(&args);
        let entry = Entry {
            target,
            id,
            args,
            fx: None,
        };
        let slot = with_registry(|r| r.insert(entry)).unwrap_or(u32::MAX);
        if reactive {
            attach_effect(slot, rendered);
        } else if !rendered {
            render_slot(slot, Tracking::Untracked, true);
        }
        Self { slot }
    }

    /// New message and/or arguments for the same node (`rebuild`).
    #[inline(never)]
    pub(crate) fn update(&mut self, id: MsgId, args: Box<[ArgValue]>) {
        let reactive = has_reactive(&args);
        let swapped = with_registry(|r| {
            r.get_mut(self.slot).map(|e| {
                let same = e.id == id && e.args.is_empty() && args.is_empty();
                e.id = id;
                let old_args = core::mem::replace(&mut e.args, args);
                (same, e.fx.take(), old_args)
            })
        })
        .flatten();
        let Some((same, old_fx, old_args)) = swapped else {
            return;
        };
        drop(old_fx);
        drop(old_args);
        if reactive {
            attach_effect(self.slot, false);
        } else if !same {
            render_slot(self.slot, Tracking::Untracked, true);
        }
    }

    fn node(&self) -> Option<types::Text> {
        with_registry(|r| match r.get_mut(self.slot) {
            Some(Entry {
                target: Target::Text(n),
                ..
            }) => Some(n.clone()),
            _ => None,
        })
        .flatten()
    }
}

impl Drop for Binding {
    #[inline(never)]
    fn drop(&mut self) {
        // Drop the entry (node handle, effect) after the borrow is released.
        let old = with_registry(|r| r.remove(self.slot)).flatten();
        drop(old);
    }
}

/// View state of a translated text node: the slot (the node lives in it).
pub struct TrText(Binding);

impl TrText {
    pub(crate) fn new(node: types::Text, id: MsgId, args: Box<[ArgValue]>, rendered: bool) -> Self {
        Self(Binding::new(Target::Text(node), id, args, rendered))
    }

    pub(crate) fn update(&mut self, id: MsgId, args: Box<[ArgValue]>) {
        self.0.update(id, args);
    }
}

impl Mountable for TrText {
    #[inline(never)]
    fn unmount(&mut self) {
        if let Some(mut n) = self.0.node() {
            n.unmount();
        }
    }

    #[inline(never)]
    fn mount(&mut self, parent: &types::Element, marker: Option<&types::Node>) {
        if let Some(n) = self.0.node() {
            Rndr::insert_node(parent, n.as_ref(), marker);
        }
    }

    #[inline(never)]
    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        self.0.node().is_some_and(|n| n.insert_before_this(child))
    }

    fn elements(&self) -> Vec<types::Element> {
        Vec::new()
    }
}

/// Locale switch: re-render every live node.
#[inline(never)]
pub(crate) fn refresh_all() {
    with_registry(|r| {
        for slot in &r.slots {
            if let Slot::Used(e) = slot {
                let target = &e.target;
                with_resolved(e.id, &e.args, Tracking::Untracked, &mut |s| target.apply(s));
            }
        }
    });
}

pub(crate) fn live() -> usize {
    with_registry(|r| r.live).unwrap_or(0)
}
