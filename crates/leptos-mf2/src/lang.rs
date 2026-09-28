//! `mark-fallback-lang`: text the catalog borrowed from a fallback locale
//! renders inside `<span lang>` (WCAG 3.1.2, Language of Parts;
//! `plans/04-leptos-integration.md` §9, `plans/15-phase-7-work-order.md`
//! §"A14 — design").
//!
//! Only a **borrowed** message in a **view position** is wrapped, so a page
//! with no missing translation renders exactly as it does without the
//! feature. The span says the lender's tag as the build wrote it, and adds
//! `dir` only when the lender's direction differs from the catalog's —
//! English borrowed into an Arabic page must not be laid out right to left.
//!
//! Three sides agree on one shape:
//!
//! * the **server** writes `<span lang [dir]>` around the text, with the
//!   `<!>` separator before it exactly when tachys would write one before a
//!   text, and leaves the position a text leaves ([`write_open`]);
//! * **hydration** adopts the shape the server wrote — a text, or an
//!   element holding one — and never asks the catalog (§3);
//! * a **switch** or a rebuild fits the wrapper around a text node that
//!   keeps its identity ([`Wrapper::fit`]).

use alloc::rc::Rc;
use alloc::string::String;
use core::cell::Cell;

use mf2_catalog::{Catalog, Dir, MsgId};
use tachys::html::attribute::AttributeValue;
use tachys::renderer::types::{Element, Node, Text};
use tachys::renderer::{CastFrom, Rndr};

use crate::state;

/// The locale a message's text was borrowed from, as the catalog in force
/// records it.
#[derive(Clone, Copy)]
pub(crate) struct Lender<'a> {
    /// The lender's BCP 47 tag, as the build wrote it.
    tag: &'a str,
    /// The lender's direction, only when it differs from the catalog's.
    dir: Option<Dir>,
}

impl<'a> Lender<'a> {
    /// The lender of message `id` in `catalog`, or `None` when the catalog
    /// has its own text — one binary search in the FALLBACK section.
    pub(crate) fn of(catalog: &'a Catalog, id: MsgId) -> Option<Lender<'a>> {
        let tag = catalog.fallback_locale(id)?;
        let dir = state::dir_of(tag).filter(|d| *d != catalog.dir());
        Some(Lender { tag, dir })
    }

    /// The `dir` attribute's value, when there is one.
    fn dir_attr(self) -> Option<&'static str> {
        self.dir.map(|d| match d {
            Dir::Rtl => "rtl",
            Dir::Ltr => "ltr",
            Dir::Auto => "auto",
        })
    }

    /// Sets `lang` on `span`, and `dir` or its removal.
    fn apply(self, span: &Element) {
        Rndr::set_attribute(span, "lang", self.tag);
        match self.dir_attr() {
            Some(dir) => Rndr::set_attribute(span, "dir", dir),
            None => Rndr::remove_attribute(span, "dir"),
        }
    }

    /// A rich message's fragment, wrapped: one tachys element that tachys
    /// writes, hydrates and replaces like any other.
    pub(crate) fn wrap(
        self,
        root: alloc::vec::Vec<tachys::view::any_view::AnyView>,
    ) -> tachys::view::any_view::AnyView {
        use tachys::html::attribute::global::GlobalAttributes;
        use tachys::html::element::ElementChild;
        use tachys::view::any_view::IntoAny;

        tachys::html::element::span()
            .lang(String::from(self.tag))
            .dir(self.dir_attr())
            .child(root)
            .into_any()
    }
}

/// The server's opening tag, `<span lang="…" [dir="…"]>`, attribute values
/// escaped as tachys escapes any attribute.
pub(crate) fn write_open(buf: &mut String, lender: Lender<'_>) {
    buf.push_str("<span");
    <&str as AttributeValue>::to_html(lender.tag, "lang", buf);
    if let Some(dir) = lender.dir_attr() {
        <&str as AttributeValue>::to_html(dir, "dir", buf);
    }
    buf.push('>');
}

/// The elements whose content is text, never markup: a `<span lang>` inside
/// one would be shown as characters (`textarea`, `title`, `option`) or run
/// as code (`script`, `style`). A borrowed message there stays unmarked.
const TEXT_ONLY: &[&str] = &["textarea", "title", "option", "script", "style"];

/// Whether `name` is one of [`TEXT_ONLY`], in any case.
pub(crate) fn is_text_only(name: &str) -> bool {
    TEXT_ONLY.iter().any(|t| t.eq_ignore_ascii_case(name))
}

/// Whether `el` holds only text.
fn holds_text_only(el: &Element) -> bool {
    is_text_only(&el.tag_name())
}

/// The name of the element whose opening tag `buf` ends with, if it does —
/// the parent a server writes a first child into.
pub(crate) fn open_tag(buf: &str) -> Option<&str> {
    let tag = buf.strip_suffix('>')?;
    let tag = &tag[tag.rfind('<')? + 1..];
    let name = tag.split(|c: char| c.is_ascii_whitespace()).next()?;
    (!name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .then_some(name)
}

/// The element around a rendered text node, if its message is borrowed:
/// shared by the view state and the node registry's slot, so that a switch
/// can add or remove it without reaching into the view tree.
#[derive(Clone, Default)]
pub(crate) struct Wrapper(Rc<Cell<Option<Element>>>);

impl Wrapper {
    /// The wrapper, if there is one.
    pub(crate) fn get(&self) -> Option<Element> {
        let el = self.0.take();
        self.0.set(el.clone());
        el
    }

    /// The node a parent holds: the wrapper, else the text.
    pub(crate) fn outer(&self, text: &Text) -> Node {
        match self.get() {
            Some(el) => el.into(),
            None => text.clone().into(),
        }
    }

    /// Adopts the node hydration landed on: a text as it is, an element as
    /// the wrapper with its first child as the text (created when the span
    /// arrived empty). `None` for anything else — a mismatch.
    pub(crate) fn adopt(&self, node: Node) -> Option<Text> {
        if let Some(text) = Text::cast_from(node.clone()) {
            return Some(text);
        }
        let el = Element::cast_from(node)?;
        let text = Rndr::first_child(el.as_ref())
            .and_then(Text::cast_from)
            .unwrap_or_else(|| {
                let text = Rndr::create_text_node("");
                Rndr::insert_node(&el, text.as_ref(), None);
                text
            });
        self.0.set(Some(el));
        Some(text)
    }

    /// The node to insert into `parent`: the wrapper, else the text — and
    /// no wrapper at all in an element that holds only text, where a text
    /// built unmounted loses the span it was fitted with.
    pub(crate) fn mount_into(&self, parent: &Element, text: &Text) -> Node {
        if self.get().is_some() && holds_text_only(parent) {
            self.fit(text, None);
        }
        self.outer(text)
    }

    /// Makes the wrapper match `lender`, around `text`, which keeps its
    /// identity: a span is added or removed around it, or its attributes
    /// change. Works mounted or not; mounted in an element that holds only
    /// text, it adds none.
    pub(crate) fn fit(&self, text: &Text, lender: Option<Lender<'_>>) {
        match (self.0.take(), lender) {
            (None, None) => {}
            (None, Some(lender)) => {
                let parent = Rndr::get_parent(text.as_ref()).and_then(Element::cast_from);
                if parent.as_ref().is_some_and(holds_text_only) {
                    return;
                }
                let span = Rndr::create_element("span", None);
                lender.apply(&span);
                if let Some(parent) = parent {
                    Rndr::insert_node(&parent, span.as_ref(), Some(text.as_ref()));
                }
                Rndr::insert_node(&span, text.as_ref(), None);
                self.0.set(Some(span));
            }
            (Some(span), Some(lender)) => {
                lender.apply(&span);
                self.0.set(Some(span));
            }
            (Some(span), None) => {
                match Rndr::get_parent(span.as_ref()).and_then(Element::cast_from) {
                    Some(parent) => Rndr::insert_node(&parent, text.as_ref(), Some(span.as_ref())),
                    None => {
                        let _ = Rndr::remove_node(&span, text.as_ref());
                    }
                }
                Rndr::remove(span.as_ref());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_text_only, open_tag};

    #[test]
    fn the_parent_s_tag_is_read_from_what_was_just_written() {
        assert_eq!(
            open_tag("<p><textarea name=\"a b\" rows=\"2\">"),
            Some("textarea")
        );
        assert_eq!(open_tag("<p>"), Some("p"));
        assert_eq!(open_tag("<p>text"), None);
        assert_eq!(open_tag("<!>"), None);
        assert!(is_text_only("TEXTAREA") && is_text_only("title") && !is_text_only("span"));
    }
}
