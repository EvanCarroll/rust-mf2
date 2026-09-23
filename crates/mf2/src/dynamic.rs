//! Named arguments, resolved at run time
//! (`plans/13-phase-5b-work-order.md` A3).
//!
//! `tr!` is the compile-time path: the macro turns names into slots and the
//! wasm carries neither. Some callers cannot work that way —
//!
//! * the WG suite's tests whose `params` **deliberately** mismatch the
//!   message (an argument the message does not declare, or a declared
//!   variable left unset, so that the run reports *Unresolved Variable*);
//!   those cells are marked `dyn` in the ledger
//!   (`plans/01-conformance.md` §3);
//! * tools and servers formatting a message whose arguments arrive as data
//!   — `mf2-cli`, a preview, a test fixture.
//!
//! So [`TrDyn`] carries the names and hands them to
//! [`Formatter::write_named`], which matches each slot by the catalog's own
//! NAMES entry under NFC. It is **not** the client path: names in the wasm
//! are what `tr!` exists to avoid (B6), and this type allocates where the
//! `tr!` types do not.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use mf2_runtime::{Arg, ErrorSink, Formatter, MsgId, NoErrors, PartSink, Sink};

use crate::arg::{ArgValue, Text};
use crate::tr::with_args;

/// A message and its arguments **by name**, matched against the catalog's
/// NAMES when it formats — including the names the message does not have.
#[derive(Clone)]
pub struct TrDyn {
    id: MsgId,
    args: Box<[(Text, ArgValue)]>,
}

/// A message with named arguments: what the suite's mismatching tests, and
/// tools with data-driven arguments, format through.
#[must_use]
pub fn tr_dyn(id: MsgId, args: impl Into<Box<[(Text, ArgValue)]>>) -> TrDyn {
    TrDyn {
        id,
        args: args.into(),
    }
}

impl TrDyn {
    /// The message's id.
    #[must_use]
    pub const fn id(&self) -> MsgId {
        self.id
    }

    /// The arguments, as given.
    #[must_use]
    pub fn args(&self) -> &[(Text, ArgValue)] {
        &self.args
    }

    /// Formats it into `out`. A name the message does not declare is
    /// ignored; a variable no name matches is an Unresolved Variable, with
    /// its fallback text — which is exactly what the suite asserts.
    pub fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        self.with_named(|args| f.write_named(self.id, args, out, errs));
    }

    /// Formats it to parts.
    pub fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        self.with_named(|args| f.parts_named(self.id, args, out, errs));
    }

    /// Its text, with the errors discarded.
    #[must_use]
    pub fn format(&self, f: &Formatter<'_>) -> String {
        let mut out = String::new();
        self.write(f, &mut out, &mut NoErrors);
        out
    }

    /// Borrows the values the way the positional path does — a source is
    /// read, a date/time is borrowed — and pairs each with its name.
    fn with_named<R>(&self, body: impl FnOnce(&[(&str, Arg<'_>)]) -> R) -> R {
        let values: Vec<ArgValue> = self.args.iter().map(|(_, v)| v.clone()).collect();
        with_args(&values, |lowered| {
            let named: Vec<(&str, Arg<'_>)> = self
                .args
                .iter()
                .map(|(name, _)| name.as_str())
                .zip(lowered.iter().copied())
                .collect();
            body(&named)
        })
    }
}
