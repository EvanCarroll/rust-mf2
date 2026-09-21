//! Strategy A (plans/04 §4): one `RenderEffect` per translated node, each
//! tracking the global locale trigger (and its reactive arguments). Simplest;
//! a dropped effect stays subscribed to the trigger until the trigger next
//! fires — the leak P0.11 measures.

use std::cell::Cell;

use leptos::prelude::RenderEffect;
use leptos::tachys::renderer::{Rndr, types};
use leptos::tachys::view::Mountable;

use crate::MsgId;
use crate::args::ArgValue;
use crate::catalog::track_locale;
use crate::format::{Tracking, with_resolved};
use crate::glue::Target;

thread_local! {
    static LIVE: Cell<usize> = const { Cell::new(0) };
}

fn spawn(target: Target, id: MsgId, args: Box<[ArgValue]>, rendered: bool) -> RenderEffect<()> {
    RenderEffect::new(move |prev: Option<()>| {
        track_locale();
        let apply = prev.is_some() || !rendered;
        // Formatting also reads (and so subscribes to) reactive arguments.
        with_resolved(id, &args, Tracking::Tracked, &mut |s| {
            if apply {
                target.apply(s);
            }
        });
    })
}

/// The per-node update handle: one effect.
pub struct Binding {
    target: Target,
    fx: RenderEffect<()>,
}

impl Binding {
    /// Subscribes a node. `rendered`: the DOM already shows the right text.
    #[inline(never)]
    pub(crate) fn new(target: Target, id: MsgId, args: Box<[ArgValue]>, rendered: bool) -> Self {
        let _ = LIVE.try_with(|l| l.set(l.get() + 1));
        let fx = spawn(target.clone(), id, args, rendered);
        Self { target, fx }
    }

    /// New message and/or arguments for the same node (`rebuild`).
    #[inline(never)]
    pub(crate) fn update(&mut self, id: MsgId, args: Box<[ArgValue]>) {
        self.fx = spawn(self.target.clone(), id, args, false);
    }
}

impl Drop for Binding {
    #[inline(never)]
    fn drop(&mut self) {
        let _ = LIVE.try_with(|l| l.set(l.get().saturating_sub(1)));
    }
}

/// View state of a translated text node.
pub struct TrText {
    node: types::Text,
    binding: Binding,
}

impl TrText {
    pub(crate) fn new(node: types::Text, id: MsgId, args: Box<[ArgValue]>, rendered: bool) -> Self {
        let binding = Binding::new(Target::Text(node.clone()), id, args, rendered);
        Self { node, binding }
    }

    pub(crate) fn update(&mut self, id: MsgId, args: Box<[ArgValue]>) {
        self.binding.update(id, args);
    }
}

impl Mountable for TrText {
    #[inline(never)]
    fn unmount(&mut self) {
        self.node.unmount();
    }

    #[inline(never)]
    fn mount(&mut self, parent: &types::Element, marker: Option<&types::Node>) {
        Rndr::insert_node(parent, self.node.as_ref(), marker);
    }

    #[inline(never)]
    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        self.node.insert_before_this(child)
    }

    fn elements(&self) -> Vec<types::Element> {
        Vec::new()
    }
}

/// Strategy A updates through the trigger's notify; nothing to walk.
pub(crate) fn refresh_all() {}

pub(crate) fn live() -> usize {
    LIVE.try_with(Cell::get).unwrap_or(0)
}
