//! tachys 0.2 glue: `Render`, `RenderHtml`, `AddAnyAttr`, `AttributeValue`
//! for the two concrete types. Every trait method is a one-line forward to a
//! shared, non-generic, out-of-line function, so a call site (and the
//! monomorphised view code around it) only ever emits a call.

use leptos::tachys::html::attribute::AttributeValue;
use leptos::tachys::html::attribute::any_attribute::AnyAttribute;
use leptos::tachys::hydration::Cursor;
use leptos::tachys::renderer::{CastFrom, Rndr, types};
use leptos::tachys::view::{Position, PositionState, Render, RenderHtml};

use super::Target;
use crate::args::ArgValue;
use crate::format::{Tracking, with_html_resolved, with_resolved};
use crate::update::{Binding, TrText};
use crate::{MsgId, Tr, TrArgs};

// ---------- shared bodies ----------

fn text_build(id: MsgId, args: Box<[ArgValue]>) -> TrText {
    let mut node = None;
    with_resolved(id, &args, Tracking::Untracked, &mut |s| {
        node = Some(Rndr::create_text_node(s));
    });
    let node = node.unwrap_or_else(|| Rndr::create_text_node(""));
    TrText::new(node, id, args, true)
}

/// Walks the cursor exactly as `<&str as RenderHtml>::hydrate` does and adopts
/// the server's text node without reading the catalog. A structural mismatch
/// (not a text node) degrades to a detached node instead of panicking.
fn text_hydrate(
    id: MsgId,
    args: Box<[ArgValue]>,
    cursor: &Cursor,
    position: &PositionState,
    from_server: bool,
) -> TrText {
    if position.get() == Position::FirstChild {
        cursor.child();
    } else {
        cursor.sibling();
    }
    if matches!(position.get(), Position::NextChildAfterText) {
        cursor.sibling();
    }
    let node =
        types::Text::cast_from(cursor.current()).unwrap_or_else(|| Rndr::create_text_node(""));
    if !from_server {
        with_resolved(id, &args, Tracking::Untracked, &mut |s| Rndr::set_text(&node, s));
    }
    position.set(Position::NextChildAfterText);
    TrText::new(node, id, args, true)
}

/// Server: delegate to tachys' own `&str` so its separator and escaping rules
/// stay its problem (plans/04 §3).
fn text_html(
    id: MsgId,
    args: &[ArgValue],
    buf: &mut String,
    position: &mut Position,
    escape: bool,
    mark_branches: bool,
    extra_attrs: Vec<AnyAttribute>,
) {
    let mut extra = Some(extra_attrs);
    with_html_resolved(id, args, &mut |s| {
        let attrs = extra.take().unwrap_or_default();
        <&str as RenderHtml>::to_html_with_buf(s, buf, position, escape, mark_branches, attrs);
    });
}

fn attr_html(id: MsgId, args: &[ArgValue], key: &str, buf: &mut String) {
    with_html_resolved(id, args, &mut |s| <&str as AttributeValue>::to_html(s, key, buf));
}

fn attr_build(id: MsgId, args: Box<[ArgValue]>, el: &types::Element, key: &str) -> Binding {
    with_resolved(id, &args, Tracking::Untracked, &mut |s| Rndr::set_attribute(el, key, s));
    Binding::new(Target::Attr(el.clone(), Box::from(key)), id, args, true)
}

fn attr_hydrate(
    id: MsgId,
    args: Box<[ArgValue]>,
    el: &types::Element,
    key: &str,
    from_server: bool,
) -> Binding {
    if !from_server {
        with_resolved(id, &args, Tracking::Untracked, &mut |s| Rndr::set_attribute(el, key, s));
    }
    Binding::new(Target::Attr(el.clone(), Box::from(key)), id, args, true)
}

// ---------- out-of-line entry points (one per type × operation) ----------

#[inline(never)]
fn tr_build(t: Tr) -> TrText {
    text_build(t.id, Box::default())
}
#[inline(never)]
fn tr_rebuild(t: Tr, state: &mut TrText) {
    state.update(t.id, Box::default());
}
#[inline(never)]
fn tr_hydrate(t: Tr, cursor: &Cursor, position: &PositionState, from_server: bool) -> TrText {
    text_hydrate(t.id, Box::default(), cursor, position, from_server)
}
#[inline(never)]
fn tr_html(
    t: Tr,
    buf: &mut String,
    position: &mut Position,
    escape: bool,
    mark_branches: bool,
    extra_attrs: Vec<AnyAttribute>,
) {
    text_html(t.id, &[], buf, position, escape, mark_branches, extra_attrs);
}
#[inline(never)]
fn tr_attr_html(t: Tr, key: &str, buf: &mut String) {
    attr_html(t.id, &[], key, buf);
}
#[inline(never)]
fn tr_attr_build(t: Tr, el: &types::Element, key: &str) -> Binding {
    attr_build(t.id, Box::default(), el, key)
}
#[inline(never)]
fn tr_attr_hydrate(t: Tr, el: &types::Element, key: &str, from_server: bool) -> Binding {
    attr_hydrate(t.id, Box::default(), el, key, from_server)
}
#[inline(never)]
fn tr_attr_rebuild(t: Tr, state: &mut Binding) {
    state.update(t.id, Box::default());
}

#[inline(never)]
fn args_build(t: TrArgs) -> TrText {
    let (id, args) = t.into_parts();
    text_build(id, args)
}
#[inline(never)]
fn args_rebuild(t: TrArgs, state: &mut TrText) {
    let (id, args) = t.into_parts();
    state.update(id, args);
}
#[inline(never)]
fn args_hydrate(t: TrArgs, cursor: &Cursor, position: &PositionState, from_server: bool) -> TrText {
    let (id, args) = t.into_parts();
    text_hydrate(id, args, cursor, position, from_server)
}
#[inline(never)]
fn args_html(
    t: TrArgs,
    buf: &mut String,
    position: &mut Position,
    escape: bool,
    mark_branches: bool,
    extra_attrs: Vec<AnyAttribute>,
) {
    text_html(t.id, t.args.as_slice(), buf, position, escape, mark_branches, extra_attrs);
}
#[inline(never)]
fn args_attr_html(t: TrArgs, key: &str, buf: &mut String) {
    attr_html(t.id, t.args.as_slice(), key, buf);
}
#[inline(never)]
fn args_attr_build(t: TrArgs, el: &types::Element, key: &str) -> Binding {
    let (id, args) = t.into_parts();
    attr_build(id, args, el, key)
}
#[inline(never)]
fn args_attr_hydrate(t: TrArgs, el: &types::Element, key: &str, from_server: bool) -> Binding {
    let (id, args) = t.into_parts();
    attr_hydrate(id, args, el, key, from_server)
}
#[inline(never)]
fn args_attr_rebuild(t: TrArgs, state: &mut Binding) {
    let (id, args) = t.into_parts();
    state.update(id, args);
}

// ---------- trait impls: one-line forwards ----------

macro_rules! glue {
    ($ty:ty, $build:ident, $rebuild:ident, $hydrate:ident, $html:ident,
     $a_html:ident, $a_build:ident, $a_hydrate:ident, $a_rebuild:ident, $len:literal) => {
        leptos::tachys::no_attrs!($ty);

        impl Render for $ty {
            type State = TrText;

            fn build(self) -> TrText {
                $build(self)
            }

            fn rebuild(self, state: &mut TrText) {
                $rebuild(self, state);
            }
        }

        impl RenderHtml for $ty {
            type AsyncOutput = Self;
            type Owned = Self;
            const MIN_LENGTH: usize = 0;

            fn dry_resolve(&mut self) {}

            async fn resolve(self) -> Self::AsyncOutput {
                self
            }

            fn html_len(&self) -> usize {
                $len
            }

            fn to_html_with_buf(
                self,
                buf: &mut String,
                position: &mut Position,
                escape: bool,
                mark_branches: bool,
                extra_attrs: Vec<AnyAttribute>,
            ) {
                $html(self, buf, position, escape, mark_branches, extra_attrs);
            }

            fn hydrate<const FROM_SERVER: bool>(
                self,
                cursor: &Cursor,
                position: &PositionState,
            ) -> TrText {
                $hydrate(self, cursor, position, FROM_SERVER)
            }

            fn into_owned(self) -> Self::Owned {
                self
            }
        }

        impl AttributeValue for $ty {
            type State = Binding;
            type AsyncOutput = Self;
            type Cloneable = Self;
            type CloneableOwned = Self;

            fn html_len(&self) -> usize {
                $len
            }

            fn to_html(self, key: &str, buf: &mut String) {
                $a_html(self, key, buf);
            }

            fn to_template(_key: &str, _buf: &mut String) {}

            fn hydrate<const FROM_SERVER: bool>(self, key: &str, el: &types::Element) -> Binding {
                $a_hydrate(self, el, key, FROM_SERVER)
            }

            fn build(self, el: &types::Element, key: &str) -> Binding {
                $a_build(self, el, key)
            }

            fn rebuild(self, _key: &str, state: &mut Binding) {
                $a_rebuild(self, state);
            }

            fn into_cloneable(self) -> Self {
                self
            }

            fn into_cloneable_owned(self) -> Self {
                self
            }

            fn dry_resolve(&mut self) {}

            async fn resolve(self) -> Self::AsyncOutput {
                self
            }
        }
    };
}

glue!(Tr, tr_build, tr_rebuild, tr_hydrate, tr_html,
      tr_attr_html, tr_attr_build, tr_attr_hydrate, tr_attr_rebuild, 16);
glue!(TrArgs, args_build, args_rebuild, args_hydrate, args_html,
      args_attr_html, args_attr_build, args_attr_hydrate, args_attr_rebuild, 24);
