//! All tachys-facing code, one module per supported tachys line
//! (plans/04 §3). Only `tachys_0_2` exists (Leptos 0.8).

mod tachys_0_2;

use leptos::tachys::renderer::{Rndr, types};

/// Where a translated node's text goes.
#[derive(Clone)]
pub(crate) enum Target {
    Text(types::Text),
    Attr(types::Element, Box<str>),
}

impl Target {
    pub(crate) fn apply(&self, text: &str) {
        match self {
            Self::Text(node) => Rndr::set_text(node, text),
            Self::Attr(el, key) => Rndr::set_attribute(el, key, text),
        }
    }
}
