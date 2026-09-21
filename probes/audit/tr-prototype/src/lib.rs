//! Feasibility prototype: ONE concrete, non-generic `Tr` usable as a view child and as an
//! attribute value, with a single RenderEffect subscribing to a single global trigger.
use leptos::prelude::*;
use leptos::reactive::{effect::RenderEffect, signal::ArcTrigger};
use leptos::tachys::{
    html::attribute::{any_attribute::AnyAttribute, AttributeValue},
    hydration::Cursor,
    renderer::{types, CastFrom, Rndr},
    view::{Mountable, Position, PositionState, Render, RenderHtml},
};
use std::{cell::RefCell, sync::Arc};

pub struct Catalog {
    buf: Arc<[u8]>,
}
impl Catalog {
    pub fn get(&self, _id: u32) -> &str {
        // real impl: offset table lookup into buf
        std::str::from_utf8(&self.buf).unwrap_or("")
    }
}

thread_local! {
    static ACTIVE: RefCell<Option<Arc<Catalog>>> = const { RefCell::new(None) };
    static CHANGED: ArcTrigger = ArcTrigger::new();
}

#[inline(never)]
fn with_text<R>(id: u32, f: impl FnOnce(&str) -> R) -> R {
    // SSR: per-request catalog from context; client: global fast path
    #[cfg(feature = "ssr")]
    {
        if let Some(r) = with_context::<Arc<Catalog>, _>(|c| c.get(id).len()) {
            let _ = r;
        }
        let cat = use_context::<Arc<Catalog>>();
        return match &cat {
            Some(c) => f(c.get(id)),
            None => f(""),
        };
    }
    #[cfg(not(feature = "ssr"))]
    ACTIVE.with(|a| match &*a.borrow() {
        Some(c) => f(c.get(id)),
        None => f(""),
    })
}

fn track() {
    CHANGED.with(|t| t.track());
}

pub fn set_catalog(c: Arc<Catalog>) {
    ACTIVE.with(|a| *a.borrow_mut() = Some(c));
    CHANGED.with(|t| t.notify());
}

#[derive(Clone, Copy, Debug)]
pub struct Tr {
    pub id: u32,
}

#[inline(always)]
pub const fn __tr(id: u32) -> Tr {
    Tr { id }
}

// ---------- view child ----------
pub struct TrNode(types::Text);

impl Mountable for TrNode {
    fn unmount(&mut self) {
        self.0.unmount()
    }
    fn mount(&mut self, parent: &types::Element, marker: Option<&types::Node>) {
        Rndr::insert_node(parent, self.0.as_ref(), marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        self.0.insert_before_this(child)
    }
    fn elements(&self) -> Vec<types::Element> {
        vec![]
    }
}

pub struct TrState(RenderEffect<TrNode>);

impl Mountable for TrState {
    fn unmount(&mut self) {
        self.0.unmount()
    }
    fn mount(&mut self, parent: &types::Element, marker: Option<&types::Node>) {
        self.0.mount(parent, marker)
    }
    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        self.0.insert_before_this(child)
    }
    fn elements(&self) -> Vec<types::Element> {
        vec![]
    }
}

impl Render for Tr {
    type State = TrState;

    fn build(self) -> Self::State {
        let id = self.id;
        TrState(RenderEffect::new(move |prev: Option<TrNode>| {
            track();
            match prev {
                Some(node) => {
                    with_text(id, |s| Rndr::set_text(&node.0, s));
                    node
                }
                None => TrNode(with_text(id, Rndr::create_text_node)),
            }
        }))
    }

    fn rebuild(self, state: &mut Self::State) {
        // id changed (e.g. `move || if x { tr!("a") } else { tr!("b") }`)
        let id = self.id;
        let prev = state.0.take_value();
        state.0 = RenderEffect::new_with_value(
            move |prev: Option<TrNode>| {
                track();
                let node = prev.expect("state");
                with_text(id, |s| Rndr::set_text(&node.0, s));
                node
            },
            prev,
        );
    }
}

leptos::tachys::no_attrs!(Tr);

impl RenderHtml for Tr {
    type AsyncOutput = Self;
    type Owned = Self;
    const MIN_LENGTH: usize = 0;

    fn dry_resolve(&mut self) {}

    async fn resolve(self) -> Self::AsyncOutput {
        self
    }

    fn html_len(&self) -> usize {
        16
    }

    fn to_html_with_buf(
        self,
        buf: &mut String,
        position: &mut Position,
        escape: bool,
        mark_branches: bool,
        extra_attrs: Vec<AnyAttribute>,
    ) {
        // delegate: marker comment, empty-string handling and escaping stay in tachys
        with_text(self.id, |s| {
            <&str as RenderHtml>::to_html_with_buf(
                s,
                buf,
                position,
                escape,
                mark_branches,
                extra_attrs,
            )
        })
    }

    fn hydrate<const FROM_SERVER: bool>(
        self,
        cursor: &Cursor,
        position: &PositionState,
    ) -> Self::State {
        if position.get() == Position::FirstChild {
            cursor.child();
        } else {
            cursor.sibling();
        }
        if matches!(position.get(), Position::NextChildAfterText) {
            cursor.sibling();
        }
        let node = types::Text::cast_from(cursor.current())
            .expect("i18n: expected a text node during hydration");
        position.set(Position::NextChildAfterText);

        let id = self.id;
        TrState(RenderEffect::new(move |prev: Option<TrNode>| {
            track();
            match prev {
                Some(node) => {
                    with_text(id, |s| Rndr::set_text(&node.0, s));
                    node
                }
                None => {
                    // server text is already in the DOM: no catalog access needed
                    if !FROM_SERVER {
                        with_text(id, |s| Rndr::set_text(&node, s));
                    }
                    TrNode(node.clone())
                }
            }
        }))
    }

    fn into_owned(self) -> Self::Owned {
        self
    }
}

// ---------- attribute value ----------
impl AttributeValue for Tr {
    type State = RenderEffect<types::Element>;
    type AsyncOutput = Self;
    type Cloneable = Self;
    type CloneableOwned = Self;

    fn html_len(&self) -> usize {
        16
    }

    fn to_html(self, key: &str, buf: &mut String) {
        with_text(self.id, |s| <&str as AttributeValue>::to_html(s, key, buf))
    }

    fn to_template(_key: &str, _buf: &mut String) {}

    fn hydrate<const FROM_SERVER: bool>(self, key: &str, el: &types::Element) -> Self::State {
        attr_effect(self.id, key, el, FROM_SERVER)
    }

    fn build(self, el: &types::Element, key: &str) -> Self::State {
        attr_effect(self.id, key, el, false)
    }

    fn rebuild(self, key: &str, state: &mut Self::State) {
        if let Some(el) = state.take_value() {
            *state = attr_effect(self.id, key, &el, false);
        }
    }

    fn into_cloneable(self) -> Self::Cloneable {
        self
    }

    fn into_cloneable_owned(self) -> Self::CloneableOwned {
        self
    }

    fn dry_resolve(&mut self) {}

    async fn resolve(self) -> Self::AsyncOutput {
        self
    }
}

#[inline(never)]
fn attr_effect(id: u32, key: &str, el: &types::Element, skip_first: bool) -> RenderEffect<types::Element> {
    let key: Arc<str> = Rndr::intern(key).into();
    let el = el.clone();
    RenderEffect::new(move |prev: Option<types::Element>| {
        track();
        let first = prev.is_none();
        let el = prev.unwrap_or_else(|| el.clone());
        if !(first && skip_first) {
            with_text(id, |s| Rndr::set_attribute(&el, &key, s));
        }
        el
    })
}

// ---------- conversions for component props ----------
impl From<Tr> for TextProp {
    fn from(t: Tr) -> Self {
        (move || {
            track();
            with_text(t.id, |s| Oco::<'static, str>::from(s.to_string()))
        })
        .into()
    }
}

impl From<Tr> for Signal<String> {
    fn from(t: Tr) -> Self {
        Signal::derive(move || {
            track();
            with_text(t.id, str::to_string)
        })
    }
}


#[inline(never)]
pub fn lookup_string(id: u32) -> String {
    track();
    with_text(id, str::to_string)
}
