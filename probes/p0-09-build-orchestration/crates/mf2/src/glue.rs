//! tachys 0.2 glue for `Tr` and `TrArgs`, after `probes/audit/tr-prototype`
//! but without per-node reactivity (a locale switch is out of P0.9's scope):
//! text child, attribute value, and the prop conversions. SSR delegates the
//! write to tachys' own `&str` impls; hydration adopts the server's text node.

use leptos::prelude::*;
use leptos::tachys::html::attribute::AttributeValue;
use leptos::tachys::html::attribute::any_attribute::AnyAttribute;
use leptos::tachys::hydration::Cursor;
use leptos::tachys::renderer::{CastFrom, Rndr, types};
use leptos::tachys::view::{Mountable, Position, PositionState, Render, RenderHtml};

use crate::{Tr, TrArgs};

/// View state of a translated text node.
pub struct TextState(types::Text);

impl Mountable for TextState {
    fn unmount(&mut self) {
        self.0.unmount();
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

fn hydrate_text(cursor: &Cursor, position: &PositionState) -> Option<types::Text> {
    if position.get() == Position::FirstChild {
        cursor.child();
    } else {
        cursor.sibling();
    }
    if matches!(position.get(), Position::NextChildAfterText) {
        cursor.sibling();
    }
    let node = types::Text::cast_from(cursor.current());
    position.set(Position::NextChildAfterText);
    node
}

macro_rules! glue {
    ($t:ty) => {
        impl Render for $t {
            type State = TextState;

            fn build(self) -> Self::State {
                TextState(Rndr::create_text_node(&self.to_string()))
            }

            fn rebuild(self, state: &mut Self::State) {
                Rndr::set_text(&state.0, &self.to_string());
            }
        }

        leptos::tachys::no_attrs!($t);

        impl RenderHtml for $t {
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
                let s = self.to_string();
                <&str as RenderHtml>::to_html_with_buf(
                    &s,
                    buf,
                    position,
                    escape,
                    mark_branches,
                    extra_attrs,
                );
            }

            fn hydrate<const FROM_SERVER: bool>(
                self,
                cursor: &Cursor,
                position: &PositionState,
            ) -> Self::State {
                match hydrate_text(cursor, position) {
                    Some(node) => {
                        if !FROM_SERVER {
                            Rndr::set_text(&node, &self.to_string());
                        }
                        TextState(node)
                    }
                    None => {
                        leptos::logging::error!("mf2: expected a text node during hydration");
                        TextState(Rndr::create_text_node(""))
                    }
                }
            }

            fn into_owned(self) -> Self::Owned {
                self
            }
        }

        impl AttributeValue for $t {
            type State = types::Element;
            type AsyncOutput = Self;
            type Cloneable = Self;
            type CloneableOwned = Self;

            fn html_len(&self) -> usize {
                16
            }

            fn to_html(self, key: &str, buf: &mut String) {
                let s = self.to_string();
                <&str as AttributeValue>::to_html(&s, key, buf);
            }

            fn to_template(_key: &str, _buf: &mut String) {}

            fn hydrate<const FROM_SERVER: bool>(
                self,
                key: &str,
                el: &types::Element,
            ) -> Self::State {
                if !FROM_SERVER {
                    Rndr::set_attribute(el, key, &self.to_string());
                }
                el.clone()
            }

            fn build(self, el: &types::Element, key: &str) -> Self::State {
                Rndr::set_attribute(el, key, &self.to_string());
                el.clone()
            }

            fn rebuild(self, key: &str, state: &mut Self::State) {
                Rndr::set_attribute(state, key, &self.to_string());
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

        impl From<$t> for TextProp {
            fn from(t: $t) -> Self {
                TextProp::from(t.to_string())
            }
        }

        impl From<$t> for Signal<String> {
            fn from(t: $t) -> Self {
                Signal::stored(t.to_string())
            }
        }

        impl From<$t> for String {
            fn from(t: $t) -> Self {
                t.to_string()
            }
        }
    };
}

glue!(Tr);
glue!(TrArgs);
