//! `mf2-runtime` — the MessageFormat 2 evaluator of mf2-two
//! (`plans/03-runtime.md`): it formats a message **from a catalog**, walking
//! `mf2-catalog`'s views in place — resolution, declarations (lazily, each at
//! most once), selection, fallback, the Default Bidi Strategy, format to
//! parts, markup, the `u:` options — and it holds the function registry, the
//! custom-function API and the core functions: `:string`, and `:number`,
//! `:integer`, `:offset` with their complete semantics and neutral symbols.
//!
//! | Item | What |
//! |---|---|
//! | [`Formatter`] | the entry point: `simple`, `write`, `parts`, `*_named` |
//! | [`Sink`], [`PartSink`], [`ErrorSink`] | where output and errors go |
//! | [`Arg`], [`Value`], [`Number`] | arguments and resolved values |
//! | [`Function`], [`Registry`], [`functions`] | handlers, closed world (B13) |
//! | [`Host`] | NFC, float text, zone offsets, a date formatter from the platform |
//! | [`DateTime`], [`TimeZone`], [`NumberSpec`], [`Digits`], [`Measure`] | Phase 4's additions for the function crates (§2.7) |
//!
//! The API is `plans/03-runtime.md` §2 (§2.7: Phase 4's additions). Client-path code: `no_std` +
//! `alloc`, `forbid(unsafe_code)`, no `core::fmt`, no panicking operation
//! (B12); built-in handlers never allocate.

#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

extern crate alloc;

mod datetime;
mod error;
mod eval;
mod format;
mod function;
pub mod functions;
mod host;
mod number;
mod parts;
mod plural;
mod scratch;
mod sink;
mod text;
mod unannotated;
mod value;

pub use mf2_catalog::{Catalog, Dir, MsgId, StrRef};
pub use mf2_model::MarkupKind;

pub use datetime::{
    Date, DateFields, DateLength, DateStyle, DateTime, DateTimeOptions, DateTimeRequest, Time,
    TimePrecision, TimeZone, ZoneOption, ZoneStyle, is_zone_name,
};
pub use error::FormatError;
pub use format::{BidiStrategy, FormatContext, Formatter};
pub use function::{FnContext, Function, OptionValue, Options, Registry};
pub use host::Host;
pub use number::{Digits, Grouping, Measure, MeasureUnit, Number, NumberSpec, Sign};
pub use parts::{
    ExpressionPart, FallbackSource, Isolation, MarkupOptions, MarkupPart, Part, PartSink,
};
pub use plural::{Category, Operands, select as plural_category};
pub use sink::{ErrorSink, NoErrors, Sink, SubPartSink};
pub use value::{Arg, CustomValue, Value};
