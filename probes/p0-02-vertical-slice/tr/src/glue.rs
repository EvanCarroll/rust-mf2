//! tachys 0.2 glue for `Tr` — all framework coupling lives here (plans/04 §3).
//!
//! * SSR writes are delegated to tachys' own `&str` impls, so the empty-text
//!   and `<!>` separator rules stay tachys' problem.
//! * Hydration adopts the server's node and never reads the catalog.
//! * Live updates on locale switch go through the registry (strategy B).

use crate::registry::{self, SlotHandle, Target};
use crate::{Tr, with_text};
use leptos::prelude::{Oco, Signal, TextProp};
use leptos::tachys::{
    html::attribute::{AttributeValue, any_attribute::AnyAttribute},
    hydration::Cursor,
    renderer::{CastFrom, Rndr, types},
    view::{Mountable, Position, PositionState, Render, RenderHtml},
};

// ---------------------------------------------------------------- text child

/// View state of a translated text node: the DOM node and its registry slot.
pub struct TrState {
    node: types::Text,
    slot: SlotHandle,
}

impl Mountable for TrState {
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

impl Render for Tr {
    type State = TrState;

    fn build(self) -> TrState {
        let node = with_text(self.id, Rndr::create_text_node);
        let slot = registry::register(Target::Text(node.clone()), self.id);
        TrState { node, slot }
    }

    fn rebuild(self, state: &mut TrState) {
        // `move || if x { tr!("a") } else { tr!("b") }` lands here.
        state.slot.set_id(self.id);
        with_text(self.id, |s| Rndr::set_text(&state.node, s));
    }
}

leptos::tachys::no_attrs!(Tr);

impl RenderHtml for Tr {
    type AsyncOutput = Self;
    type Owned = Self;
    const MIN_LENGTH: usize = 0;

    fn dry_resolve(&mut self) {}

    async fn resolve(self) -> Self {
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
        // D9: the catalog is resolved *now*, at render time, from the request
        // context — this is the call the streaming test exercises.
        with_text(self.id, |s| {
            <&str as RenderHtml>::to_html_with_buf(
                s,
                buf,
                position,
                escape,
                mark_branches,
                extra_attrs,
            );
        });
    }

    fn hydrate<const FROM_SERVER: bool>(self, cursor: &Cursor, position: &PositionState) -> TrState {
        // Walk exactly as `<&str as RenderHtml>::hydrate` does.
        if position.get() == Position::FirstChild {
            cursor.child();
        } else {
            cursor.sibling();
        }
        if matches!(position.get(), Position::NextChildAfterText) {
            cursor.sibling();
        }
        let node = if let Some(text) = types::Text::cast_from(cursor.current()) {
            text
        } else {
            // tachys panics here (`failed_to_cast_text_node` is crate-private);
            // the client path must not. Report and continue with a detached
            // node: this translation will not update, the rest hydrates.
            leptos::leptos_dom::logging::console_error(
                "mf2: hydration expected a text node for a translated message; \
                 the server and client markup differ structurally",
            );
            Rndr::create_text_node("")
        };
        if !FROM_SERVER {
            with_text(self.id, |s| Rndr::set_text(&node, s));
        }
        position.set(Position::NextChildAfterText);
        let slot = registry::register(Target::Text(node.clone()), self.id);
        TrState { node, slot }
    }

    fn into_owned(self) -> Self {
        self
    }
}

// ----------------------------------------------------------------- attribute

/// View state of a translated attribute value.
pub struct AttrState {
    el: types::Element,
    slot: SlotHandle,
}

fn register_attr(el: &types::Element, key: &str, id: u32) -> AttrState {
    AttrState {
        el: el.clone(),
        slot: registry::register(Target::Attr(el.clone(), key.into()), id),
    }
}

impl AttributeValue for Tr {
    type State = AttrState;
    type AsyncOutput = Self;
    type Cloneable = Self;
    type CloneableOwned = Self;

    fn html_len(&self) -> usize {
        16
    }

    fn to_html(self, key: &str, buf: &mut String) {
        with_text(self.id, |s| <&str as AttributeValue>::to_html(s, key, buf));
    }

    fn to_template(_key: &str, _buf: &mut String) {}

    fn hydrate<const FROM_SERVER: bool>(self, key: &str, el: &types::Element) -> AttrState {
        if !FROM_SERVER {
            with_text(self.id, |s| Rndr::set_attribute(el, key, s));
        }
        register_attr(el, key, self.id)
    }

    fn build(self, el: &types::Element, key: &str) -> AttrState {
        with_text(self.id, |s| Rndr::set_attribute(el, key, s));
        register_attr(el, key, self.id)
    }

    fn rebuild(self, key: &str, state: &mut AttrState) {
        state.slot.set_id(self.id);
        with_text(self.id, |s| Rndr::set_attribute(&state.el, key, s));
    }

    fn into_cloneable(self) -> Self {
        self
    }

    fn into_cloneable_owned(self) -> Self {
        self
    }

    fn dry_resolve(&mut self) {}

    async fn resolve(self) -> Self {
        self
    }
}

// --------------------------------------------------------------- conversions
// One function each, in the library — not one closure per call site.

// D9 fallback, narrowed to derived values: a `TextProp` / `Signal<String>` is
// evaluated whenever *its consumer* decides — leptos_meta reads `<Title text>`
// in `inject_meta_context`, outside the request owner, under `SsrMode::Async`
// and `PartiallyBlocked` (P0.2 finding). So under `ssr` the conversion captures
// the request's catalog at construction; `Tr` itself stays a 4-byte `Copy`
// value looked up at render time. Feature `d9-no-capture` restores pure
// render-time lookup to reproduce the failure.

#[cfg(all(feature = "ssr", not(feature = "d9-no-capture")))]
fn derived(t: Tr) -> impl Fn() -> String + Send + Sync + 'static {
    let catalog = crate::server::current().catalog;
    move || catalog.get(t.id).to_owned()
}

#[cfg(not(all(feature = "ssr", not(feature = "d9-no-capture"))))]
fn derived(t: Tr) -> impl Fn() -> String + Send + Sync + 'static {
    move || {
        crate::track_locale();
        with_text(t.id, str::to_owned)
    }
}

impl From<Tr> for TextProp {
    fn from(t: Tr) -> Self {
        let f = derived(t);
        TextProp::from(move || Oco::<'static, str>::from(f()))
    }
}

impl From<Tr> for Signal<String> {
    fn from(t: Tr) -> Self {
        Signal::derive(derived(t))
    }
}

impl From<Tr> for String {
    fn from(t: Tr) -> Self {
        t.to_string()
    }
}

impl From<Tr> for Oco<'static, str> {
    fn from(t: Tr) -> Self {
        Oco::from(t.to_string())
    }
}
