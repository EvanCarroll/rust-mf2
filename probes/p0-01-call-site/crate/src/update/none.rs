//! Strategy C (plans/04 §4, feature `static-locale` in the plan): no live
//! update, no per-node bookkeeping; a locale switch is cookie + navigation.
//! The view state is the bare text node (no `Drop` beyond the node handle).
//! Here for P0.1's size comparison only.

use leptos::tachys::renderer::{Rndr, types};
use leptos::tachys::view::Mountable;

use crate::MsgId;
use crate::args::ArgValue;
use crate::format::{Tracking, with_resolved};
use crate::glue::Target;

/// No per-node handle: attributes keep nothing either.
pub struct Binding;

impl Binding {
    pub(crate) fn new(target: Target, id: MsgId, args: Box<[ArgValue]>, rendered: bool) -> Self {
        if !rendered {
            with_resolved(id, &args, Tracking::Untracked, &mut |s| target.apply(s));
        }
        Self
    }

    pub(crate) fn update(&mut self, _id: MsgId, _args: Box<[ArgValue]>) {}
}

/// View state of a translated text node: the node only.
pub struct TrText {
    node: types::Text,
}

impl TrText {
    pub(crate) fn new(node: types::Text, _id: MsgId, _args: Box<[ArgValue]>, _rendered: bool) -> Self {
        Self { node }
    }

    #[inline(never)]
    pub(crate) fn update(&mut self, id: MsgId, args: Box<[ArgValue]>) {
        with_resolved(id, &args, Tracking::Untracked, &mut |s| Rndr::set_text(&self.node, s));
    }
}

impl Mountable for TrText {
    fn unmount(&mut self) {
        self.node.unmount();
    }

    fn mount(&mut self, parent: &types::Element, marker: Option<&types::Node>) {
        Rndr::insert_node(parent, self.node.as_ref(), marker);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        self.node.insert_before_this(child)
    }

    fn elements(&self) -> Vec<types::Element> {
        Vec::new()
    }
}

pub(crate) fn refresh_all() {}

pub(crate) fn live() -> usize {
    0
}
