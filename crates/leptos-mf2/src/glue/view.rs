//! Everything that names tachys (`plans/04-leptos-integration.md` §3), for
//! the 0.2 line (Leptos 0.8) and, with `tachys-0-3`, the 0.3 line (Leptos
//! 0.9).
//!
//! The two lines differ here in one method: `to_html_with_buf` takes
//! `escape` and `mark_branches` on 0.2 and one `RenderFlags` on 0.3, with the
//! text-separator rule moving from `escape` to `flags.hydrate` (§1). Both
//! impls of it below exist in both forms, switched by the feature; each form
//! only passes its parameters on to tachys' own impl, so the separator rules
//! stay tachys' on either line. Everything else is shared.
//!
//! Three rules hold everywhere below.
//!
//! 1. **Delegate the writing.** A `simple` message hands tachys the
//!    catalog's borrowed `&str` and lets `<&str as RenderHtml>` apply its own
//!    empty-text and `<!>` separator rules; a pattern formats into the shared
//!    scratch first and delegates the same way. Writing piecewise would make
//!    those rules ours, and they are tachys'.
//! 2. **Never panic while hydrating.** Stock tachys traps on a node that is
//!    not a `Text` (P0.10: the wasm aborts and the page goes inert), and its
//!    `failed_to_cast_text_node` is `pub(crate)` — so the cursor is walked
//!    here, and a mismatch logs once and degrades.
//! 3. **One function per position, not one per call site.** Each impl below
//!    is written once against [`Description`], for all four description
//!    types (B5).

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use tachys::html::attribute::any_attribute::AnyAttribute;
use tachys::html::attribute::{Attribute, AttributeValue};
use tachys::html::property::IntoProperty;
use tachys::hydration::Cursor;
#[cfg(not(feature = "mark-fallback-lang"))]
use tachys::renderer::CastFrom;
use tachys::renderer::Rndr;
use tachys::renderer::types::{Element, Text as TextNode};
use tachys::view::add_attr::AddAnyAttr;
use tachys::view::any_view::AnyViewState;
use tachys::view::iterators::VecState;
// With `tachys-0-3` on Leptos 0.8 this import is the first error, and rustc
// prints its line, so the line says what to do.
#[cfg(feature = "tachys-0-3")]
use tachys::view::RenderFlags; // `tachys-0-3` needs Leptos 0.9; for Leptos 0.8, turn it off
use tachys::view::{Mountable, Position, PositionState, Render, RenderHtml, ToTemplate};

#[cfg(feature = "mark-fallback-lang")]
use crate::lang::{self, Wrapper};
#[cfg(not(feature = "static-locale"))]
use crate::registry::Relocalize;
use crate::registry::{self, Target};
use crate::rich;
use crate::state::{self, TextUse};
use crate::text::{self, Description};
use crate::{Tr, TrArgs, TrDyn, TrRich};

/// The retained state of a rendered description: the registry slot, and
/// nothing else. Four bytes, and `Drop` frees the slot in O(1) (D7).
///
/// With `mark-fallback-lang`, also the wrapper the slot shares: the text
/// node keeps its identity for the life of the view, and a `<span lang>`
/// comes and goes around it.
pub struct TrState {
    slot: u32,
    node: TextNode,
    #[cfg(feature = "mark-fallback-lang")]
    wrapper: Wrapper,
}

impl Drop for TrState {
    fn drop(&mut self) {
        registry::remove(self.slot);
    }
}

/// Without `mark-fallback-lang` the node is the text; with it, the wrapper
/// when there is one — every method acts on the outer node.
impl Mountable for TrState {
    fn unmount(&mut self) {
        #[cfg(feature = "mark-fallback-lang")]
        if let Some(mut wrapper) = self.wrapper.get() {
            wrapper.unmount();
            return;
        }
        self.node.unmount();
    }

    fn mount(&mut self, parent: &Element, marker: Option<&tachys::renderer::types::Node>) {
        #[cfg(feature = "mark-fallback-lang")]
        Rndr::insert_node(parent, &self.wrapper.outer(&self.node), marker);
        #[cfg(not(feature = "mark-fallback-lang"))]
        Rndr::insert_node(parent, self.node.as_ref(), marker);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        #[cfg(feature = "mark-fallback-lang")]
        if let Some(wrapper) = self.wrapper.get() {
            return wrapper.insert_before_this(child);
        }
        self.node.insert_before_this(child)
    }

    fn elements(&self) -> Vec<Element> {
        #[cfg(feature = "mark-fallback-lang")]
        if let Some(wrapper) = self.wrapper.get() {
            return alloc::vec![wrapper];
        }
        Vec::new()
    }
}

/// The text of `description` against the catalog this render reads, handed
/// to `body` — borrowed from the catalog when the message is `simple`, and
/// `""` when there is no catalog at all.
fn with_text<D: Description, R>(description: &D, use_: TextUse, body: impl FnOnce(&str) -> R) -> R {
    text::with_active_text(description, use_, body)
}

/// Walks the hydration cursor exactly as `<&str as RenderHtml>::hydrate`
/// does, and adopts the node it lands on.
///
/// **Where this differs from tachys**: a node that is not a `Text` does not
/// panic. Stock tachys calls `failed_to_cast_text_node`, which logs and then
/// panics in debug and hits `unreachable!()` in release; either way the wasm
/// traps and the whole page stops hydrating (P0.10). Here the mismatch is
/// reported once and a detached text node is used instead, so this one
/// message is wrong and everything after it still hydrates.
#[cfg(not(feature = "mark-fallback-lang"))]
fn adopt_text(cursor: &Cursor, position: &PositionState, text: &str) -> TextNode {
    let node = walk(cursor, position);
    if let Some(text_node) = TextNode::cast_from(node.clone()) {
        return text_node;
    }
    hydration_mismatch(&node);
    Rndr::create_text_node(text)
}

/// Moves the cursor to a text's node — child or sibling, then over the
/// separator — and leaves the position where a text leaves it.
fn walk(cursor: &Cursor, position: &PositionState) -> tachys::renderer::types::Node {
    if position.get() == Position::FirstChild {
        cursor.child();
    } else {
        cursor.sibling();
    }
    // The separating placeholder marker comes before the text node.
    if matches!(position.get(), Position::NextChildAfterText) {
        cursor.sibling();
    }
    position.set(Position::NextChildAfterText);
    cursor.current()
}

/// [`adopt_text`] under `mark-fallback-lang`: the shape the server wrote is
/// adopted, not asked of the catalog (§3) — a text as it is, or a
/// `<span lang>` as the wrapper with its text inside. The cursor stays on
/// the outer node, so what follows walks as it would after a text.
#[cfg(feature = "mark-fallback-lang")]
fn adopt_marked(
    cursor: &Cursor,
    position: &PositionState,
    text: &str,
    wrapper: &Wrapper,
) -> TextNode {
    let node = walk(cursor, position);
    if let Some(text_node) = wrapper.adopt(node.clone()) {
        return text_node;
    }
    hydration_mismatch(&node);
    Rndr::create_text_node(text)
}

/// A text child built on the client, wrapped when its message is borrowed.
#[cfg(feature = "mark-fallback-lang")]
fn build_marked<D: Description>(description: D) -> TrState {
    let wrapper = Wrapper::default();
    let node = text::with_active_marked_text(&description, |text, lender| {
        let node = Rndr::create_text_node(text);
        wrapper.fit(&node, lender);
        node
    });
    let slot = registry::insert(
        Target::Text(node.clone(), wrapper.clone()),
        description.into_stored(),
    );
    TrState {
        slot,
        node,
        wrapper,
    }
}

/// A text child hydrated: from the server, the shape it wrote; from a
/// template clone, the catalog's text, fitted.
#[cfg(feature = "mark-fallback-lang")]
fn hydrate_marked<const FROM_SERVER: bool, D: Description>(
    description: D,
    cursor: &Cursor,
    position: &PositionState,
) -> TrState {
    let wrapper = Wrapper::default();
    let node = if FROM_SERVER {
        adopt_marked(cursor, position, "", &wrapper)
    } else {
        text::with_active_marked_text(&description, |text, lender| {
            let node = adopt_marked(cursor, position, text, &wrapper);
            Rndr::set_text(&node, text);
            wrapper.fit(&node, lender);
            node
        })
    };
    let slot = registry::insert(
        Target::Text(node.clone(), wrapper.clone()),
        description.into_stored(),
    );
    TrState {
        slot,
        node,
        wrapper,
    }
}

/// A text child on the server. Unborrowed, `write` — tachys' own `&str`
/// rules — is the whole of it. Borrowed: the separator exactly when tachys
/// would write one before a text (`separator`), the span, the text written
/// as a first child, and the position a text leaves, so that whatever
/// follows writes and hydrates the same either way.
#[cfg(feature = "mark-fallback-lang")]
fn html_marked<D: Description>(
    description: &D,
    buf: &mut String,
    position: &mut Position,
    separator: bool,
    write: impl FnOnce(&str, &mut String, &mut Position),
) {
    text::with_active_marked_text(description, |text, lender| match lender {
        None => write(text, buf, position),
        Some(lender) => {
            if separator {
                buf.push_str("<!>");
            }
            lang::write_open(buf, lender);
            write(text, buf, &mut Position::FirstChild);
            buf.push_str("</span>");
            *position = Position::NextChildAfterText;
        }
    });
}

/// One diagnostic, then degrade (§3). Not a panic, and not `format!`: the
/// client path carries no fmt machinery (B12).
fn hydration_mismatch(node: &tachys::renderer::types::Node) {
    #[cfg(any(feature = "hydrate", feature = "csr"))]
    web_sys::console::error_2(
        &wasm_bindgen::JsValue::from_str(
            "mf2: expected a text node while hydrating a translated message, but found this. \
             The message will not be interactive; the rest of the page still hydrates.",
        ),
        node.as_ref(),
    );
    #[cfg(not(any(feature = "hydrate", feature = "csr")))]
    let _ = node;
}

/// `Render`, `RenderHtml`, `AddAnyAttr` and `ToTemplate` for one description
/// type. The bodies are calls into this module's shared functions, so the
/// macro repeats declarations, never logic.
macro_rules! render_description {
    ($ty:ty) => {
        impl AddAnyAttr for $ty {
            type Output<SomeNewAttr: Attribute> = $ty;

            fn add_any_attr<NewAttr: Attribute>(self, _attr: NewAttr) -> Self::Output<NewAttr> {
                self
            }
        }

        impl Render for $ty {
            type State = TrState;

            #[cfg(not(feature = "mark-fallback-lang"))]
            fn build(self) -> Self::State {
                let node = with_text(&self, TextUse::Displayed, |text| {
                    Rndr::create_text_node(text)
                });
                let slot = registry::insert(Target::Text(node.clone()), self.into_stored());
                TrState { slot, node }
            }

            #[cfg(feature = "mark-fallback-lang")]
            fn build(self) -> Self::State {
                build_marked(self)
            }

            fn rebuild(self, state: &mut Self::State) {
                let node = &state.node;
                #[cfg(feature = "mark-fallback-lang")]
                let target = || Some(Target::Text(node.clone(), state.wrapper.clone()));
                #[cfg(not(feature = "mark-fallback-lang"))]
                let target = || Some(Target::Text(node.clone()));
                registry::replace(&mut state.slot, target, self.into_stored());
            }
        }

        impl RenderHtml for $ty {
            type AsyncOutput = Self;
            type Owned = Self;

            const MIN_LENGTH: usize = 0;

            fn dry_resolve(&mut self) {}

            fn resolve(self) -> impl core::future::Future<Output = Self::AsyncOutput> + Send {
                core::future::ready(self)
            }

            fn html_len(&self) -> usize {
                0
            }

            #[cfg(not(feature = "tachys-0-3"))]
            fn to_html_with_buf(
                self,
                buf: &mut String,
                position: &mut Position,
                escape: bool,
                mark_branches: bool,
                extra_attrs: Vec<AnyAttribute>,
            ) {
                #[cfg(feature = "mark-fallback-lang")]
                html_marked(
                    &self,
                    buf,
                    position,
                    matches!(*position, Position::NextChildAfterText),
                    |text, buf, position| {
                        <&str as RenderHtml>::to_html_with_buf(
                            text,
                            buf,
                            position,
                            escape,
                            mark_branches,
                            extra_attrs,
                        );
                    },
                );
                #[cfg(not(feature = "mark-fallback-lang"))]
                with_text(&self, TextUse::Displayed, |text| {
                    <&str as RenderHtml>::to_html_with_buf(
                        text,
                        buf,
                        position,
                        escape,
                        mark_branches,
                        extra_attrs,
                    );
                });
            }

            #[cfg(feature = "tachys-0-3")]
            fn to_html_with_buf(
                self,
                buf: &mut String,
                position: &mut Position,
                flags: RenderFlags,
                extra_attrs: Vec<AnyAttribute>,
            ) {
                #[cfg(feature = "mark-fallback-lang")]
                html_marked(
                    &self,
                    buf,
                    position,
                    flags.hydrate && matches!(*position, Position::NextChildAfterText),
                    |text, buf, position| {
                        <&str as RenderHtml>::to_html_with_buf(
                            text,
                            buf,
                            position,
                            flags,
                            extra_attrs,
                        );
                    },
                );
                #[cfg(not(feature = "mark-fallback-lang"))]
                with_text(&self, TextUse::Displayed, |text| {
                    <&str as RenderHtml>::to_html_with_buf(text, buf, position, flags, extra_attrs);
                });
            }

            #[cfg(feature = "mark-fallback-lang")]
            fn hydrate<const FROM_SERVER: bool>(
                self,
                cursor: &Cursor,
                position: &PositionState,
            ) -> Self::State {
                hydrate_marked::<FROM_SERVER, _>(self, cursor, position)
            }

            #[cfg(not(feature = "mark-fallback-lang"))]
            fn hydrate<const FROM_SERVER: bool>(
                self,
                cursor: &Cursor,
                position: &PositionState,
            ) -> Self::State {
                // The text is read only when this is *not* server HTML: from
                // the server the node already carries it, and reading the
                // catalog here would be the one thing hydration must not
                // depend on (§3).
                let node = if FROM_SERVER {
                    adopt_text(cursor, position, "")
                } else {
                    with_text(&self, TextUse::Displayed, |text| {
                        let node = adopt_text(cursor, position, text);
                        Rndr::set_text(&node, text);
                        node
                    })
                };
                let slot = registry::insert(Target::Text(node.clone()), self.into_stored());
                TrState { slot, node }
            }

            fn into_owned(self) -> Self::Owned {
                self
            }
        }

        impl ToTemplate for $ty {
            const TEMPLATE: &'static str = " <!>";

            fn to_template(
                buf: &mut String,
                _class: &mut String,
                _style: &mut String,
                _inner_html: &mut String,
                position: &mut Position,
            ) {
                <&str as ToTemplate>::to_template(buf, _class, _style, _inner_html, position);
            }
        }
    };
}

render_description!(Tr);
render_description!(TrArgs);
render_description!(TrDyn);

/// The retained state of a description in an attribute or a property: the
/// registry slot that writes it again on a locale switch — and, under
/// `static-locale`, where most nodes have no slot, the element, so that a
/// rebuild can still write it.
pub struct TrAttrState {
    slot: u32,
    #[cfg(feature = "static-locale")]
    el: Element,
}

impl TrAttrState {
    #[cfg_attr(not(feature = "static-locale"), allow(unused_variables))]
    fn new(slot: u32, el: &Element) -> TrAttrState {
        TrAttrState {
            slot,
            #[cfg(feature = "static-locale")]
            el: el.clone(),
        }
    }

    /// Rewrites this attribute or property for `desc`. `target` builds the
    /// registry's target from the element, which only a `static-locale`
    /// state keeps: elsewhere every node has a slot that holds it.
    #[cfg_attr(not(feature = "static-locale"), allow(unused_variables))]
    fn rebuild(&mut self, target: impl FnOnce(&Element) -> Target, desc: crate::text::Stored) {
        #[cfg(feature = "static-locale")]
        let el = &self.el;
        #[cfg(feature = "static-locale")]
        registry::replace(&mut self.slot, || Some(target(el)), desc);
        #[cfg(not(feature = "static-locale"))]
        registry::replace(&mut self.slot, || None, desc);
    }
}

/// As for a text node: dropping the state frees the slot, and with it the
/// slot's handle on the element. Without this an attribute leaked one slot
/// each time its element unmounted — a route left, a list row removed —
/// and the registry kept writing to detached elements on every switch
/// (Phase 7 A3 found it; nothing before it ever unmounted an attribute).
impl Drop for TrAttrState {
    fn drop(&mut self) {
        registry::remove(self.slot);
    }
}

/// `AttributeValue` and `IntoProperty` for one description type.
///
/// An attribute is isolated or not by its name — `title` is text a person
/// reads, `value` is text a program reads ([`state::attribute_use`]); a
/// property is text a program reads, so it is never isolated (§9).
macro_rules! attribute_description {
    ($ty:ty) => {
        impl AttributeValue for $ty {
            type State = TrAttrState;
            type AsyncOutput = Self;
            type Cloneable = Self;
            type CloneableOwned = Self;

            fn html_len(&self) -> usize {
                0
            }

            fn to_html(self, key: &str, buf: &mut String) {
                with_text(&self, state::attribute_use(key), |text| {
                    <&str as AttributeValue>::to_html(text, key, buf);
                });
            }

            fn to_template(_key: &str, _buf: &mut String) {}

            fn hydrate<const FROM_SERVER: bool>(self, key: &str, el: &Element) -> Self::State {
                // As tachys does for `&str`: server HTML already carries the
                // attribute, and a `<template>` clone does not.
                if !FROM_SERVER {
                    with_text(&self, state::attribute_use(key), |text| {
                        Rndr::set_attribute(el, key, text);
                    });
                }
                let slot = registry::insert(
                    Target::Attribute(el.clone(), key.into()),
                    self.into_stored(),
                );
                TrAttrState::new(slot, el)
            }

            fn build(self, el: &Element, key: &str) -> Self::State {
                with_text(&self, state::attribute_use(key), |text| {
                    Rndr::set_attribute(el, key, text);
                });
                let slot = registry::insert(
                    Target::Attribute(el.clone(), key.into()),
                    self.into_stored(),
                );
                TrAttrState::new(slot, el)
            }

            fn rebuild(self, key: &str, state: &mut Self::State) {
                state.rebuild(
                    |el| Target::Attribute(el.clone(), key.into()),
                    self.into_stored(),
                );
            }

            fn into_cloneable(self) -> Self::Cloneable {
                self
            }

            fn into_cloneable_owned(self) -> Self::CloneableOwned {
                self
            }

            fn dry_resolve(&mut self) {}

            fn resolve(self) -> impl core::future::Future<Output = Self::AsyncOutput> + Send {
                core::future::ready(self)
            }
        }

        impl IntoProperty for $ty {
            type State = TrAttrState;
            type Cloneable = Self;
            type CloneableOwned = Self;

            fn hydrate<const FROM_SERVER: bool>(self, el: &Element, key: &str) -> Self::State {
                let _ = FROM_SERVER;
                IntoProperty::build(self, el, key)
            }

            fn build(self, el: &Element, key: &str) -> Self::State {
                with_text(&self, TextUse::Plain, |text| {
                    Rndr::set_property_or_value(el, key, &wasm_bindgen::JsValue::from_str(text));
                });
                let slot =
                    registry::insert(Target::Property(el.clone(), key.into()), self.into_stored());
                TrAttrState::new(slot, el)
            }

            fn rebuild(self, state: &mut Self::State, key: &str) {
                state.rebuild(
                    |el| Target::Property(el.clone(), key.into()),
                    self.into_stored(),
                );
            }

            fn into_cloneable(self) -> Self::Cloneable {
                self
            }

            fn into_cloneable_owned(self) -> Self::CloneableOwned {
                self
            }
        }
    };
}

attribute_description!(Tr);
attribute_description!(TrArgs);
attribute_description!(TrRich);
attribute_description!(TrDyn);

/// `to_string()` as an **inherent** method, and the `String` conversion.
///
/// Not `Display`: `Display` would put `core::fmt` on the client path, which
/// B12 forbids — 04 §3 asks for a "`Display`-free `to_string()`" for exactly
/// that reason. An inherent method also wins name resolution, so a call site
/// writes what it would have written anyway.
macro_rules! string_conversions {
    ($ty:ty) => {
        impl $ty {
            /// The message's text against the catalog in force, **isolated**:
            /// the spec's Default Bidi Strategy, which `formatting.md` makes
            /// the default for a message formatted as a single string (§9,
            /// revised in Phase 7 A12). [`to_plain_string`](Self::to_plain_string)
            /// is the form for text a program consumes.
            #[must_use]
            #[allow(clippy::inherent_to_string)]
            pub fn to_string(&self) -> String {
                text::to_string(self, TextUse::Displayed)
            }

            /// The same text with no bidi isolation, for a `String` a
            /// program consumes — a server function, a comparison, the
            /// clipboard, `format!` — where U+2066–U+2069 would be invisible
            /// junk.
            #[must_use]
            pub fn to_plain_string(&self) -> String {
                text::to_string(self, TextUse::Plain)
            }

            /// A synonym of [`to_string`](Self::to_string), kept from when
            /// `to_string()` was plain.
            #[must_use]
            pub fn to_display_string(&self) -> String {
                text::to_string(self, TextUse::Displayed)
            }
        }

        impl From<$ty> for String {
            fn from(description: $ty) -> String {
                text::to_string(&description, TextUse::Displayed)
            }
        }
    };
}

string_conversions!(Tr);
string_conversions!(TrArgs);
string_conversions!(TrRich);
string_conversions!(TrDyn);

// ------------------------------------------------- markup as a fragment ---

/// A rich message's rendered fragment, with the description that built it.
///
/// The view state and the node registry **share** this through an `Rc`, so
/// that a locale switch can rebuild the fragment (its structure comes from
/// the catalog, §7) without reaching into the view tree. One allocation per
/// rich node, and rich nodes are rare.
pub(crate) struct RichNode {
    desc: TrRich,
    state: VecState<AnyViewState>,
}

#[cfg(not(feature = "static-locale"))]
impl Relocalize for RichNode {
    fn relocalize(&mut self, catalog: &mf2_catalog::Catalog) {
        rich::fragment(&self.desc, catalog).rebuild(&mut self.state);
    }
}

/// The retained state of a rendered rich message.
pub struct TrRichState {
    node: Rc<RefCell<RichNode>>,
    slot: u32,
}

impl TrRichState {
    fn new(desc: TrRich, state: VecState<AnyViewState>) -> TrRichState {
        let node = Rc::new(RefCell::new(RichNode { desc, state }));
        #[cfg(not(feature = "static-locale"))]
        let slot = {
            let erased: Rc<RefCell<dyn Relocalize>> = node.clone();
            registry::insert_rich(erased)
        };
        // Nothing follows the locale, and the fragment's own rebuild goes
        // through this state, not the registry.
        #[cfg(feature = "static-locale")]
        let slot = registry::NONE;
        TrRichState { node, slot }
    }

    fn with<R>(&self, body: impl FnOnce(&mut VecState<AnyViewState>) -> R) -> Option<R> {
        let mut node = self.node.try_borrow_mut().ok()?;
        Some(body(&mut node.state))
    }
}

impl Drop for TrRichState {
    fn drop(&mut self) {
        registry::remove(self.slot);
    }
}

impl Mountable for TrRichState {
    fn unmount(&mut self) {
        self.with(Mountable::unmount);
    }

    fn mount(&mut self, parent: &Element, marker: Option<&tachys::renderer::types::Node>) {
        self.with(|state| state.mount(parent, marker));
    }

    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        self.with(|state| state.insert_before_this(child))
            .unwrap_or(false)
    }

    fn elements(&self) -> Vec<Element> {
        self.with(|state| state.elements()).unwrap_or_default()
    }
}

impl AddAnyAttr for TrRich {
    type Output<SomeNewAttr: Attribute> = TrRich;

    fn add_any_attr<NewAttr: Attribute>(self, _attr: NewAttr) -> Self::Output<NewAttr> {
        self
    }
}

impl Render for TrRich {
    type State = TrRichState;

    fn build(self) -> Self::State {
        let state = rich::active_fragment(&self).build();
        TrRichState::new(self, state)
    }

    fn rebuild(self, state: &mut Self::State) {
        let fragment = rich::active_fragment(&self);
        if let Ok(mut node) = state.node.try_borrow_mut() {
            node.desc = self;
            fragment.rebuild(&mut node.state);
        }
    }
}

impl RenderHtml for TrRich {
    type AsyncOutput = Self;
    type Owned = Self;

    const MIN_LENGTH: usize = 0;

    fn dry_resolve(&mut self) {}

    fn resolve(self) -> impl core::future::Future<Output = Self::AsyncOutput> + Send {
        core::future::ready(self)
    }

    fn html_len(&self) -> usize {
        0
    }

    #[cfg(not(feature = "tachys-0-3"))]
    fn to_html_with_buf(
        self,
        buf: &mut String,
        position: &mut Position,
        escape: bool,
        mark_branches: bool,
        extra_attrs: Vec<AnyAttribute>,
    ) {
        rich::active_fragment(&self).to_html_with_buf(
            buf,
            position,
            escape,
            mark_branches,
            extra_attrs,
        );
    }

    #[cfg(feature = "tachys-0-3")]
    fn to_html_with_buf(
        self,
        buf: &mut String,
        position: &mut Position,
        flags: RenderFlags,
        extra_attrs: Vec<AnyAttribute>,
    ) {
        rich::active_fragment(&self).to_html_with_buf(buf, position, flags, extra_attrs);
    }

    /// The structure came from the catalog, so the catalog has to be the one
    /// the server rendered with — which is what the boot gate guarantees
    /// (§6). Nothing here can panic on a mismatch: each child is an
    /// `AnyView`, and the leaves are this crate's own text hydration.
    fn hydrate<const FROM_SERVER: bool>(
        self,
        cursor: &Cursor,
        position: &PositionState,
    ) -> Self::State {
        let state = rich::active_fragment(&self).hydrate::<FROM_SERVER>(cursor, position);
        TrRichState::new(self, state)
    }

    fn into_owned(self) -> Self::Owned {
        self
    }
}
