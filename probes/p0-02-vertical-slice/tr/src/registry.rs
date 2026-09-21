//! Node-update strategy B (plans/04 §4): a library-owned slab of live
//! translated nodes. One slot per node — `{target, MsgId}` — freed in O(1) by
//! the view state's `Drop`. No `RenderEffect`, no `Owner`, no subscription per
//! node, so dropped nodes cannot linger as dead subscribers.
//!
//! Compiled on every target (tachys' DOM types exist natively) but only ever
//! populated in the browser: SSR never calls `build`/`hydrate`.

use leptos::tachys::renderer::{Rndr, types};
use std::cell::RefCell;

pub(crate) enum Target {
    Text(types::Text),
    Attr(types::Element, Box<str>),
}

struct Slot {
    target: Target,
    id: u32,
}

struct Registry {
    slots: Vec<Option<Slot>>,
    free: Vec<usize>,
    live: usize,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = const {
        RefCell::new(Registry { slots: Vec::new(), free: Vec::new(), live: 0 })
    };
}

/// Owns one registry slot; dropping it frees the slot.
pub(crate) struct SlotHandle(usize);

impl SlotHandle {
    pub(crate) fn set_id(&self, id: u32) {
        REGISTRY.with(|r| {
            if let Ok(mut r) = r.try_borrow_mut()
                && let Some(Some(slot)) = r.slots.get_mut(self.0)
            {
                slot.id = id;
            }
        });
    }
}

impl Drop for SlotHandle {
    fn drop(&mut self) {
        REGISTRY.with(|r| {
            if let Ok(mut r) = r.try_borrow_mut()
                && let Some(s) = r.slots.get_mut(self.0)
                && s.take().is_some()
            {
                r.free.push(self.0);
                r.live = r.live.saturating_sub(1);
            }
        });
    }
}

pub(crate) fn register(target: Target, id: u32) -> SlotHandle {
    REGISTRY.with(|r| {
        let Ok(mut r) = r.try_borrow_mut() else {
            // Re-entrant registration cannot happen (refresh runs no user code);
            // if it ever did, the node simply would not update on switch.
            return SlotHandle(usize::MAX);
        };
        let slot = Some(Slot { target, id });
        r.live = r.live.saturating_add(1);
        if let Some(i) = r.free.pop()
            && let Some(s) = r.slots.get_mut(i)
        {
            *s = slot;
            SlotHandle(i)
        } else {
            r.slots.push(slot);
            SlotHandle(r.slots.len() - 1)
        }
    })
}

/// Rewrites every live node from `lookup` — called once per locale switch.
pub(crate) fn refresh_all(lookup: impl Fn(u32, &mut dyn FnMut(&str))) {
    REGISTRY.with(|r| {
        let Ok(r) = r.try_borrow() else { return };
        for slot in r.slots.iter().flatten() {
            match &slot.target {
                Target::Text(node) => lookup(slot.id, &mut |s| Rndr::set_text(node, s)),
                Target::Attr(el, key) => lookup(slot.id, &mut |s| Rndr::set_attribute(el, key, s)),
            }
        }
    });
}

/// Number of live registered nodes (probe diagnostics; e2e reads it).
pub fn live_nodes() -> usize {
    REGISTRY.with(|r| r.try_borrow().map(|r| r.live).unwrap_or(0))
}
