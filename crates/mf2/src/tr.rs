//! What a call site is: a description of a message
//! (`plans/04-leptos-integration.md` §2).
//!
//! `tr!` builds one of three values and nothing else happens — no lookup, no
//! formatting, no allocation beyond the arguments themselves. Something
//! renders or stringifies it later, in whatever reactive context that
//! happens, which is why one macro suffices for every position.
//!
//! | Type | What it holds | Size |
//! |---|---|---|
//! | [`Tr`] | a [`MsgId`] | 4 bytes, `Copy`, `const`-constructible |
//! | [`TrArgs`] | a [`MsgId`] and its arguments in slot order | one concrete type at every arity |
//! | [`TrRich`] | a [`TrArgs`] and the markup handlers, keyed by the hash of the markup name | — |
//!
//! Formatting takes the [`Formatter`] the caller built from *its* catalog,
//! registry and context: this core knows nothing about Leptos, contexts or
//! signals, so a server, a test and `mf2-cli` use it as they are. Phase 6's
//! `leptos-mf2` adds the ambient-catalog forms on top of these.
//!
//! Client-path code: no `core::fmt`, no panicking operation; the only
//! allocation a format makes is the scratch a spilled or dynamic argument
//! list needs.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use core::any::Any;

use mf2_catalog::markup_key;
use mf2_runtime::{Arg, DateTime, ErrorSink, Formatter, MsgId, NoErrors, PartSink, Sink};

use crate::arg::{ArgList, ArgValue, INLINE, to_arg};

/// A message with no arguments: the whole call site is these four bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct Tr {
    id: MsgId,
}

/// A message with arguments: **one** concrete type, whatever the arity, so
/// that a call site instantiates no generic (P0.1).
#[derive(Clone)]
pub struct TrArgs {
    id: MsgId,
    args: ArgList,
}

/// A message rendered with its markup as elements
/// (`plans/04-leptos-integration.md` §7): the arguments, plus one
/// type-erased handler per markup name of the message, found by the hash of
/// that name — the name itself never reaches the wasm.
#[derive(Clone)]
pub struct TrRich {
    args: TrArgs,
    handlers: Box<[(u64, Arc<dyn MarkupHandler>)]>,
}

/// A markup handler, as the core carries it: the rendering layer's own type,
/// erased (`plans/04-leptos-integration.md` §2.1).
///
/// The core never calls a handler — a message's markup formats to parts
/// whether a call site handles it or not. `leptos-mf2` downcasts through
/// [`as_any`] to the handler type it knows.
///
/// [`as_any`]: MarkupHandler::as_any
pub trait MarkupHandler: Send + Sync {
    /// The handler itself, for the layer that knows its type.
    fn as_any(&self) -> &dyn Any;
}

/// `tr!("id")` — a message without arguments.
///
/// Always inlined: the whole call site is a four-byte construction, and a
/// call to make one would cost more than the value (P0.1).
#[must_use]
#[inline(always)]
#[allow(clippy::inline_always)]
pub const fn tr(id: MsgId) -> Tr {
    Tr { id }
}

/// `tr!("id", …)` with no arguments but markup handlers: the [`TrArgs`]
/// [`tr_rich`] wraps.
#[must_use]
pub const fn tr_args0(id: MsgId) -> TrArgs {
    TrArgs {
        id,
        args: ArgList::EMPTY,
    }
}

/// `tr!("id", a = …)` — the arguments are in the manifest's slot order,
/// which the macro put them in.
#[must_use]
pub fn tr_args1(id: MsgId, a: ArgValue) -> TrArgs {
    TrArgs {
        id,
        args: ArgList::inline(1, [a, ArgValue::Unset, ArgValue::Unset, ArgValue::Unset]),
    }
}

/// Two arguments, in slot order.
#[must_use]
pub fn tr_args2(id: MsgId, a: ArgValue, b: ArgValue) -> TrArgs {
    TrArgs {
        id,
        args: ArgList::inline(2, [a, b, ArgValue::Unset, ArgValue::Unset]),
    }
}

/// Three arguments, in slot order.
#[must_use]
pub fn tr_args3(id: MsgId, a: ArgValue, b: ArgValue, c: ArgValue) -> TrArgs {
    TrArgs {
        id,
        args: ArgList::inline(3, [a, b, c, ArgValue::Unset]),
    }
}

/// Four arguments — the reference workload's maximum — in slot order.
#[must_use]
pub fn tr_args4(id: MsgId, a: ArgValue, b: ArgValue, c: ArgValue, d: ArgValue) -> TrArgs {
    TrArgs {
        id,
        args: ArgList::inline(4, [a, b, c, d]),
    }
}

/// Five or more arguments, in slot order: the only shape that spills to the
/// heap, and the only one whose call site instantiates a generic (the array
/// into the box).
#[must_use]
pub fn tr_args_n(id: MsgId, args: Box<[ArgValue]>) -> TrArgs {
    TrArgs {
        id,
        args: ArgList::spill(args),
    }
}

/// `tr!("id", kbd = …)` — the handlers in the message's ascending markup
/// order, each with the hash of its name.
#[must_use]
pub fn tr_rich(args: TrArgs, handlers: Box<[(u64, Arc<dyn MarkupHandler>)]>) -> TrRich {
    TrRich { args, handlers }
}

/// The conversion a call site's markup handler goes through.
///
/// Phase 5b's core takes anything that is already a [`MarkupHandler`];
/// `leptos-mf2` (Phase 6) provides the one that takes a view closure, and
/// the facade re-exports it under the `leptos` feature. Either way the
/// macro's expansion is the same — `markup(h)` — which is what makes the
/// expansion contract stable across the two phases.
#[must_use]
pub fn markup<H: MarkupHandler + 'static>(handler: H) -> Arc<dyn MarkupHandler> {
    Arc::new(handler)
}

impl Tr {
    /// The message's id.
    #[must_use]
    pub const fn id(self) -> MsgId {
        self.id
    }

    /// Formats it into `out`, reporting errors to `errs`.
    pub fn write(self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        f.write(self.id, &[], out, errs);
    }

    /// Formats it to parts.
    pub fn parts(self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        f.parts(self.id, &[], out, errs);
    }

    /// Its text, with the errors discarded — the release client's policy
    /// (`plans/03-runtime.md` §8). Use [`Tr::write`] to see them.
    #[must_use]
    pub fn format(self, f: &Formatter<'_>) -> String {
        let mut out = String::new();
        self.write(f, &mut out, &mut NoErrors);
        out
    }
}

impl From<Tr> for TrArgs {
    fn from(t: Tr) -> TrArgs {
        tr_args0(t.id)
    }
}

impl TrArgs {
    /// The message's id.
    #[must_use]
    pub const fn id(&self) -> MsgId {
        self.id
    }

    /// The arguments, in slot order.
    #[must_use]
    pub fn args(&self) -> &[ArgValue] {
        self.args.as_slice()
    }

    /// Formats it into `out`, reporting errors to `errs`.
    pub fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        with_args(self.args.as_slice(), |args| {
            f.write(self.id, args, out, errs);
        });
    }

    /// Formats it to parts.
    pub fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        with_args(self.args.as_slice(), |args| {
            f.parts(self.id, args, out, errs);
        });
    }

    /// Its text, with the errors discarded.
    #[must_use]
    pub fn format(&self, f: &Formatter<'_>) -> String {
        let mut out = String::new();
        self.write(f, &mut out, &mut NoErrors);
        out
    }
}

impl TrRich {
    /// The message's id.
    #[must_use]
    pub const fn id(&self) -> MsgId {
        self.args.id
    }

    /// The description without its handlers.
    #[must_use]
    pub const fn args(&self) -> &TrArgs {
        &self.args
    }

    /// The handler for the markup name `name`, as a markup part carries it.
    #[must_use]
    pub fn handler(&self, name: &str) -> Option<&dyn MarkupHandler> {
        let key = markup_key(name);
        self.handlers
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, h)| &**h)
    }

    /// Every handler, in the message's ascending markup order, with the key
    /// its name hashes to.
    #[must_use]
    pub fn handlers(&self) -> &[(u64, Arc<dyn MarkupHandler>)] {
        &self.handlers
    }

    /// Formats it into `out` — markup contributes no text, so this is the
    /// arguments' own formatting.
    pub fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        self.args.write(f, out, errs);
    }

    /// Formats it to parts: the markup parts a renderer pairs with the
    /// handlers.
    pub fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        self.args.parts(f, out, errs);
    }

    /// Its text, with the errors discarded.
    #[must_use]
    pub fn format(&self, f: &Formatter<'_>) -> String {
        self.args.format(f)
    }
}

/// Borrows `values` into the runtime's `&[Arg]` and runs `body` on it.
///
/// Three things have to happen first, each only when the call site needs it:
/// a [`ArgValue::Source`] is read (inside `body`'s caller, which is inside
/// whatever reactive context is formatting), a date/time is turned into the
/// borrowed [`DateTime`] the runtime takes by reference, and — beyond four
/// arguments — the scratch goes to the heap. An allocation that fails
/// formats with no arguments rather than panicking: every placeholder then
/// reports an Unresolved Variable, which is a defined outcome.
fn with_args<R>(values: &[ArgValue], body: impl FnOnce(&[Arg<'_>]) -> R) -> R {
    if values.iter().any(|v| matches!(v, ArgValue::Source(_))) {
        let mut resolved = alloc::vec::Vec::new();
        if resolved.try_reserve_exact(values.len()).is_err() {
            return body(&[]);
        }
        resolved.extend(values.iter().map(ArgValue::resolve));
        return with_args(&resolved, body);
    }
    with_dates(values, |dates| {
        let mut inline = [Arg::Unset; INLINE];
        let mut spill = alloc::vec::Vec::new();
        let args: &[Arg<'_>] = if values.len() <= INLINE {
            for (i, value) in values.iter().enumerate() {
                if let Some(slot) = inline.get_mut(i) {
                    *slot = to_arg(value, dates.get(i).and_then(Option::as_ref));
                }
            }
            inline.get(..values.len()).unwrap_or(&[])
        } else {
            if spill.try_reserve_exact(values.len()).is_err() {
                return body(&[]);
            }
            spill.extend(
                values
                    .iter()
                    .enumerate()
                    .map(|(i, value)| to_arg(value, dates.get(i).and_then(Option::as_ref))),
            );
            &spill
        };
        body(args)
    })
}

/// The borrowed date/time of each value that is one, positionally. Nothing
/// is allocated for a call site without dates — which is almost all of them.
fn with_dates<'a, R>(values: &'a [ArgValue], body: impl FnOnce(&[Option<DateTime<'a>>]) -> R) -> R {
    if !values.iter().any(|v| matches!(v, ArgValue::DateTime(_))) {
        return body(&[]);
    }
    let borrow = |value: &'a ArgValue| match value {
        ArgValue::DateTime(d) => Some(d.borrow()),
        _ => None,
    };
    if values.len() <= INLINE {
        let mut inline: [Option<DateTime<'a>>; INLINE] = [None; INLINE];
        for (i, value) in values.iter().enumerate() {
            if let Some(slot) = inline.get_mut(i) {
                *slot = borrow(value);
            }
        }
        return body(inline.get(..values.len()).unwrap_or(&[]));
    }
    let mut spill = alloc::vec::Vec::new();
    if spill.try_reserve_exact(values.len()).is_err() {
        return body(&[]);
    }
    spill.extend(values.iter().map(borrow));
    body(&spill)
}
