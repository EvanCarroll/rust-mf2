//! A description's text: the one place any of them is formatted
//! (`plans/04-leptos-integration.md` §3).
//!
//! Two things keep this off the per-call-site budget:
//!
//! * a `simple` message — no placeholders, no selection — hands back the
//!   catalog's **borrowed** `&str`, so tachys writes it with no `String` in
//!   between, exactly as it writes a literal;
//! * everything else formats into a scratch buffer this module reuses, so a
//!   locale switch over 2,000 nodes allocates once, not 2,000 times.
//!
//! [`Description`] is what the three call-site types have in common, and the
//! rendering code is written once against it rather than three times.

use alloc::boxed::Box;
use alloc::string::String;

use mf2_catalog::Catalog;
use mf2_runtime::{Formatter, MsgId, NoErrors, Sink};

use crate::TrDyn;
use crate::catalog;
#[cfg(feature = "mark-fallback-lang")]
use crate::lang::Lender;
use crate::state::{self, TextUse};
use crate::tr::{Tr, TrArgs, TrRich};

/// What a call site built, as the renderer sees it.
///
/// Sealed: the four implementors are this crate's, and a fifth would have to
/// be a description too.
pub trait Description: Clone + Send + Sync + 'static + private::Sealed {
    /// The message's id — what the `simple` fast path needs.
    fn msg_id(&self) -> MsgId;

    /// Formats it into `out`, errors discarded (the release client's policy,
    /// `plans/03-runtime.md` §8).
    fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink);

    /// The form the node registry stores, so that a locale switch can
    /// re-format it (D7).
    fn into_stored(self) -> Stored;
}

mod private {
    pub trait Sealed {}
    impl Sealed for super::Tr {}
    impl Sealed for super::TrArgs {}
    impl Sealed for super::TrRich {}
    impl Sealed for super::TrDyn {}
}

impl Description for Tr {
    fn msg_id(&self) -> MsgId {
        self.id()
    }

    fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink) {
        (*self).write(f, out, &mut NoErrors);
    }

    fn into_stored(self) -> Stored {
        Stored::Tr(self)
    }
}

impl Description for TrArgs {
    fn msg_id(&self) -> MsgId {
        self.id()
    }

    fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink) {
        TrArgs::write(self, f, out, &mut NoErrors);
    }

    fn into_stored(self) -> Stored {
        Stored::Args(Box::new(self))
    }
}

impl Description for TrRich {
    fn msg_id(&self) -> MsgId {
        self.id()
    }

    fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink) {
        TrRich::write(self, f, out, &mut NoErrors);
    }

    /// Markup writes no text, so in a text or attribute position a rich
    /// description **is** its arguments: the registry stores those and never
    /// has to rebuild a fragment (the view position does that itself).
    fn into_stored(self) -> Stored {
        Stored::Args(Box::new(self.args().clone()))
    }
}

impl Description for TrDyn {
    fn msg_id(&self) -> MsgId {
        self.id()
    }

    fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink) {
        TrDyn::write(self, f, out, &mut NoErrors);
    }

    fn into_stored(self) -> Stored {
        Stored::Dyn(Box::new(self))
    }
}

/// A description as the node registry keeps it: one pointer wide, so a slot
/// costs the same whether or not the call site had arguments.
#[derive(Clone)]
#[non_exhaustive]
pub enum Stored {
    /// No arguments — the whole description is four bytes.
    Tr(Tr),
    /// Arguments, in slot order.
    Args(Box<TrArgs>),
    /// Arguments by name (`plans/13` A3): not the client path, but a tool or
    /// a conformance run renders through it.
    Dyn(Box<TrDyn>),
}

impl Stored {
    /// The message it formats.
    #[must_use]
    pub fn msg_id(&self) -> MsgId {
        match self {
            Stored::Tr(t) => t.id(),
            Stored::Args(a) => a.id(),
            Stored::Dyn(d) => d.id(),
        }
    }

    pub(crate) fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink) {
        match self {
            Stored::Tr(t) => t.write_text(f, out),
            Stored::Args(a) => a.write_text(f, out),
            Stored::Dyn(d) => d.write_text(f, out),
        }
    }

    /// Whether any argument is read at format time — the one thing that
    /// makes a node need an effect of its own (§4).
    pub(crate) fn has_source(&self) -> bool {
        let source = |v: &crate::ArgValue| matches!(v, crate::ArgValue::Source(_));
        match self {
            Stored::Tr(_) => false,
            Stored::Args(a) => a.args().iter().any(source),
            Stored::Dyn(d) => d.args().iter().any(|(_, v)| source(v)),
        }
    }
}

impl Description for Stored {
    fn msg_id(&self) -> MsgId {
        Stored::msg_id(self)
    }

    fn write_text(&self, f: &Formatter<'_>, out: &mut dyn Sink) {
        Stored::write_text(self, f, out);
    }

    fn into_stored(self) -> Stored {
        self
    }
}

impl private::Sealed for Stored {}

// One scratch `String`, reused. Taken while it is in use, so that a markup
// handler rendering another description during a format gets its own rather
// than a borrow panic — reentrancy costs an allocation, not a page.
std::thread_local! {
    static SCRATCH: core::cell::RefCell<String> = const { core::cell::RefCell::new(String::new()) };
}

fn with_scratch<R>(body: impl FnOnce(&mut String) -> R) -> R {
    let mut buf = SCRATCH.with(|s| {
        s.try_borrow_mut()
            .map(|mut held| core::mem::take(&mut *held))
            .unwrap_or_default()
    });
    buf.clear();
    let out = body(&mut buf);
    SCRATCH.with(|s| {
        if let Ok(mut slot) = s.try_borrow_mut()
            && slot.capacity() < buf.capacity()
        {
            *slot = buf;
        }
    });
    out
}

/// Runs `body` on the text of `description` against `catalog`.
///
/// A `simple` message never leaves the catalog: `body` gets the borrowed
/// `&str`. Anything else is formatted into the reused scratch. `body` is
/// called **exactly once**, with `""` when there is nothing to format with,
/// so that no caller has to carry a "what if there is no catalog" branch.
pub(crate) fn with_text<D: Description, R>(
    description: &D,
    catalog: &Catalog,
    use_: TextUse,
    body: impl FnOnce(&str) -> R,
) -> R {
    format_with(description, catalog, use_, None, None, body)
}

/// The one formatting call: everything above it decides *what* to format and
/// *with what*, and this decides nothing.
fn format_with<D: Description, R>(
    description: &D,
    catalog: &Catalog,
    use_: TextUse,
    registry: Option<&'static mf2_runtime::Registry>,
    bidi: Option<mf2_runtime::BidiStrategy>,
    body: impl FnOnce(&str) -> R,
) -> R {
    let (Some(cx), Some(installed)) = (state::context_for(use_, bidi), state::registry()) else {
        return body("");
    };
    let f = Formatter::new(catalog, registry.unwrap_or(installed), &cx);
    match f.simple(description.msg_id()) {
        Some(text) => body(text),
        None => with_scratch(|buf| {
            description.write_text(&f, buf);
            body(buf)
        }),
    }
}

/// The text of `description` against whatever catalog this render reads —
/// the ambient form (§2.1: "Phase 6 adds the ambient-catalog forms on top of
/// these, not beside them").
///
/// With no catalog the text is empty: a lookup never panics (§5).
pub(crate) fn with_active_text<D: Description, R>(
    description: &D,
    use_: TextUse,
    body: impl FnOnce(&str) -> R,
) -> R {
    match catalog::current() {
        Some(cx) => cx.in_zone(|| {
            format_with(
                description,
                cx.catalog(),
                use_,
                cx.registry(),
                cx.bidi(),
                body,
            )
        }),
        None => body(""),
    }
}

/// [`with_text`] for a text child, handing `body` the message's lender
/// beside its text — found against the catalog already in hand, so the
/// position looks the catalog up once (`mark-fallback-lang`).
#[cfg(feature = "mark-fallback-lang")]
pub(crate) fn with_marked_text<D: Description, R>(
    description: &D,
    catalog: &Catalog,
    body: impl FnOnce(&str, Option<Lender<'_>>) -> R,
) -> R {
    let lender = Lender::of(catalog, description.msg_id());
    format_with(
        description,
        catalog,
        TextUse::Displayed,
        None,
        None,
        |text| body(text, lender),
    )
}

/// [`with_active_text`] for a text child, with the lender beside the text.
#[cfg(feature = "mark-fallback-lang")]
pub(crate) fn with_active_marked_text<D: Description, R>(
    description: &D,
    body: impl FnOnce(&str, Option<Lender<'_>>) -> R,
) -> R {
    match catalog::current() {
        Some(cx) => {
            let lender = Lender::of(cx.catalog(), description.msg_id());
            cx.in_zone(|| {
                format_with(
                    description,
                    cx.catalog(),
                    TextUse::Displayed,
                    cx.registry(),
                    cx.bidi(),
                    |text| body(text, lender),
                )
            })
        }
        None => body("", None),
    }
}

/// The text of `description`, owned — the **ambient string** form, which is
/// what `to_string()`, `String::from` and the `Oco` conversion go through.
///
/// On the client it subscribes to the locale change, so that a closure like
/// `move || label(x.get())` — half of real call sites produce a `String`
/// this way (04 §2) — re-runs after `set_locale`. Nothing is tracked when
/// there is no observer, which is the event-handler case §4 says must not
/// warn; the node registry, not this, is what updates a rendered node.
pub(crate) fn to_string<D: Description>(description: &D, use_: TextUse) -> String {
    #[cfg(not(feature = "ssr"))]
    catalog::track_locale();
    with_active_text(description, use_, |text: &str| String::from(text))
}
