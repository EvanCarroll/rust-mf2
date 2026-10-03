//! `mf2-runtime` — the MessageFormat 2 evaluator of Rust MF2: it formats a message **from a catalog**, walking
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
//! | [`Function`], [`Registry`], [`functions`] | handlers; a registry names only the handlers its corpus uses, and no other is linked |
//! | [`Host`] | NFC, float text, zone offsets, a date formatter, a number formatter (`intl`) from the platform |
//! | [`DateTime`], [`TimeZone`], [`NumberSpec`], [`Digits`], [`Measure`] | what the function crates and custom functions build on |
//!
//! Client-path code: `no_std` + `alloc`, `forbid(unsafe_code)`, no
//! `core::fmt`, no panicking operation; built-in handlers never allocate.
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide: how the crates fit together, web and native applications, the
//! command line, and what 2.x promises.
//! An application reaches this crate through
//! [`mf2`](https://docs.rs/mf2), which re-exports it; a custom function is
//! written against [`Function`].

#![warn(missing_docs, missing_debug_implementations)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
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
mod nfc;
mod number;
mod parts;
mod plural;
mod scratch;
mod sink;
mod text;
mod unannotated;
mod value;

#[doc(hidden)]
pub use mf2_catalog::StrRef;
pub use mf2_catalog::{Catalog, Dir, MsgId};
pub use mf2_model::MarkupKind;

pub use datetime::{
    Date, DateFields, DateLength, DateStyle, DateTime, DateTimeOptions, DateTimeRequest, Time,
    TimePrecision, TimeZone, ZoneOption, ZoneStyle, is_zone_name,
};
pub use error::FormatError;
pub use format::{BidiStrategy, FormatContext, Formatter};
pub use function::{FnContext, Function, OptionValue, Options, Registry};
pub use host::{Host, NumberFormatter};
/// Canonical equivalence with a catalog key from the catalog's map, for the
/// differential fuzz target (`fuzz/fuzz_targets/nfc.rs`) and the
/// conformance crate's differential test, both of which check it against
/// full NFC. Applications use [`FnContext::equivalent`].
#[doc(hidden)]
pub use nfc::equivalent as nfc_equivalent;
pub use number::{
    CurrencyDisplay, DigitOptions, Digits, Grouping, Measure, MeasureUnit, Number, NumberOut,
    NumberRequest, NumberSpec, NumberStyle, RoundingMode, RoundingPriority, Sign, SignDisplay,
    UnitDisplay,
};
pub use parts::{
    ExpressionPart, FallbackSource, Isolation, MarkupOptions, MarkupPart, Part, PartSink,
};
#[doc(hidden)]
pub use plural::select as plural_category;
pub use plural::{Category, Operands};
pub use sink::{ErrorSink, NoErrors, Sink, SubPartSink};
pub use value::{Arg, CustomValue, Value};

/// Whether numbers format through the host (`plans/03-runtime.md` §2.7,
/// §5.3): feature `intl`, on `wasm32-unknown-unknown` only. The numeric
/// functions — the core's and `mf2-fn-number`'s — then take the display,
/// `:integer`'s rounding and the plural category from
/// the host's [`NumberFormatter`] ([`Host::numbers`]: the browser's
/// `Intl.NumberFormat` and `Intl.PluralRules`) instead of the Rust digit
/// plan, rounding and plural evaluator, which are not linked. Everywhere
/// else — servers, `wasm32-wasip1`, native tests — `false`: the Rust path.
#[doc(hidden)]
pub const INTL_NUMBERS: bool = cfg!(all(
    feature = "intl",
    target_arch = "wasm32",
    target_os = "unknown"
));
