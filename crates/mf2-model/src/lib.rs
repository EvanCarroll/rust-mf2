//! `mf2-model` — the Unicode MessageFormat 2 (MF2) interchange data model as
//! Rust types, plus the identities and error kinds every rust-mf2 crate shares
//! and the [`Frontend`] trait a parser implements.
//!
//! The data model's structs mirror the specification's `message.json` field
//! for field, and code builds them by literal; a later MF2 that defines a
//! new structure adds a variant, which is why the enums are
//! `#[non_exhaustive]`.
//!
//! * [`MsgId`], [`Dir`] — identities shared with the catalog and the runtime.
//! * [`is_name_start`], [`is_name_char`] — the ABNF's name classes, shared by
//!   the two frontends.
//! * [`ErrorKind`], [`ErrorClass`], [`Span`], [`Diagnostic`], [`Diagnostics`] —
//!   the 13 error kinds of the WG test suite plus two, and how a frontend
//!   reports them.
//! * [`Message`] and everything below it — the specification's interchange
//!   data model, one-to-one.
//! * [`Parsed`], [`Frontend`] — the parser boundary.
//!
//! Values are kept **as written**: nothing in this crate normalizes. Names
//! exclude the bidi marks the syntax allows around them (spec, "Names and
//! Identifiers"); equality is exact (bytewise), which is what the round-trip
//! properties of the conformance tests need. Normalization belongs to
//! comparison (validation in `mf2-syntax`) and to catalog encoding.
//!
//! `#![no_std]` + `alloc`, no dependencies by default. Features: `serde` (JSON
//! that validates against the specification's `message.json`) and `suite-names` (error kinds ↔ the
//! suite's strings). The client wasm links only [`MsgId`], [`Dir`] and
//! [`ErrorKind`].
//!
//! # The user guide
//!
//! Getting started, call sites, delivery modes, switching language,
//! accessibility, migrating from `leptos-fluent`, and what 1.x promises
//! See the [rust-mf2 book](https://chattyness.github.io/rust-mf2/) for the
//! ecosystem and application guides. An application formatting messages starts at
//! [`mf2`](https://docs.rs/mf2); this crate is for tools that work on the
//! data model itself, with `mf2-syntax`.

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
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

#[doc(hidden)]
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
