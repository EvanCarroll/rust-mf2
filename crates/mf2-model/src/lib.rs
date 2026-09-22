//! `mf2-model` — the Unicode MessageFormat 2 (MF2) interchange data model as
//! Rust types, plus the identities and error kinds every mf2-two crate shares
//! and the [`Frontend`] trait a parser implements.
//!
//! The public types are **frozen** by the Phase 1 work order
//! (`plans/08-phase-1-work-order.md`, "Frozen public types of `mf2-model`"):
//! later phases are written against these names, fields and signatures.
//! Adding a method is allowed; changing or removing one needs a change to
//! that document in the same commit.
//!
//! * [`MsgId`], [`Dir`] — identities shared with the catalog and the runtime.
//! * [`is_name_start`], [`is_name_char`] — the ABNF's name classes, shared by
//!   the two frontends.
//! * [`ErrorKind`], [`ErrorClass`], [`Span`], [`Diagnostic`], [`Diagnostics`] —
//!   the 13 error kinds of the WG test suite plus two, and how a frontend
//!   reports them.
//! * [`Message`] and everything below it — the interchange data model of
//!   `spec/data-model/README.md`, one-to-one.
//! * [`Parsed`], [`Frontend`] — the parser boundary (decision D1's gate and
//!   fallback).
//!
//! Values are kept **as written**: nothing in this crate normalizes. Names
//! exclude the bidi marks the syntax allows around them (spec, "Names and
//! Identifiers"); equality is exact (bytewise), which is what the round-trip
//! properties of conformance layers L2 and L3 need. Normalization belongs to
//! comparison (validation in `mf2-syntax`) and to catalog encoding.
//!
//! `#![no_std]` + `alloc`, no dependencies by default. Features: `serde` (JSON
//! per `spec/data-model/message.json`) and `suite-names` (error kinds ↔ the
//! suite's strings). The client wasm links only [`MsgId`], [`Dir`] and
//! [`ErrorKind`].

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod chars;
mod diagnostic;
mod expression;
mod frontend;
mod id;
#[cfg(feature = "serde")]
mod json;
mod message;
mod pattern;

pub use alloc::borrow::Cow;

pub use chars::{is_name_char, is_name_start};
pub use diagnostic::{Diagnostic, Diagnostics, ErrorClass, ErrorKind, Span};
pub use expression::{
    Attributes, Expression, FunctionExpression, FunctionRef, Literal, LiteralExpression, Markup,
    MarkupKind, OptionValue, Options, VariableExpression, VariableRef, split_identifier,
};
pub use frontend::{Frontend, Parsed};
pub use id::{Dir, MsgId};
pub use message::{
    CatchAllKey, Declaration, InputDeclaration, Key, LocalDeclaration, Message, PatternMessage,
    SelectMessage, Variant,
};
pub use pattern::{Pattern, PatternPart};
