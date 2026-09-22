//! `mf2` — Unicode MessageFormat 2 for Leptos: the one crate an application
//! names (`plans/05-tooling.md` §9). It re-exports the public API of the
//! mf2-two crates and carries the application's feature flags
//! (`plans/00-master-plan.md` §5); it has no logic of its own beyond
//! [`compile_str`].
//!
//! | Feature | Adds |
//! |---|---|
//! | *(core)* | [`mf2_runtime`]: the formatter, `:string`, `:number` / `:integer` / `:offset` with neutral symbols, markup, bidi, fallback |
//! | `compile` | [`compile_str`]: an ad-hoc message as a one-message catalog (std; servers and tests) |
//! | `fn-number` | [`fn_number`]: `:number` / `:integer` / `:offset` localized, `:percent`, localized unannotated numbers |
//! | `fn-datetime` | [`fn_datetime`]: `:datetime` / `:date` / `:time`, unannotated date/time values (`Registry::with_dates`) |
//! | `host-std` / `host-web` | a [`Host`]: native (and `wasm32-wasip1`), or the browser |
//!
//! Phase 5b adds the `tr!` macro, Phase 6 the Leptos and Axum layers.
//!
//! ```
//! # #[cfg(all(feature = "compile", feature = "host-std"))] {
//! use mf2::{Arg, FormatContext, Formatter, Registry, functions};
//!
//! static FUNCTIONS: [(&str, &dyn mf2::Function); 1] = [("integer", &functions::INTEGER)];
//! static REGISTRY: Registry = Registry::new(&FUNCTIONS);
//! static CX: FormatContext = FormatContext::new(&mf2::host_std::HOST);
//!
//! let m = mf2::compile_str(
//!     ".input {$n :integer} .match $n one {{{$n} item}} * {{{$n} items}}",
//!     "en",
//! )
//! .unwrap();
//! let f = Formatter::new(&m.catalog, &REGISTRY, &CX);
//! let mut out = String::new();
//! let mut errors = Vec::new();
//! f.write(mf2::Compiled::ID, &[Arg::Int(3)], &mut out, &mut errors);
//! assert_eq!(out, "3 items");
//! assert!(errors.is_empty());
//! # }
//! ```

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "compile")]
mod compile;
#[cfg(feature = "compile")]
mod error;

pub use mf2_catalog::{Catalog, CatalogError, Dir, Entry, Manifest, MsgId, StrRef};
pub use mf2_model::{ErrorKind, MarkupKind};
pub use mf2_runtime::{
    Arg, BidiStrategy, Category, CustomValue, Date, DateFields, DateLength, DateStyle, DateTime,
    DateTimeOptions, DateTimeRequest, Digits, ErrorSink, ExpressionPart, FallbackSource, FnContext,
    FormatContext, FormatError, Formatter, Function, Grouping, Host, Isolation, MarkupOptions,
    MarkupPart, Measure, MeasureUnit, NoErrors, Number, NumberSpec, Operands, OptionValue, Options,
    Part, PartSink, Registry, Sign, Sink, SubPartSink, Time, TimePrecision, TimeZone, Value,
    ZoneOption, ZoneStyle, functions, is_zone_name, plural_category,
};

#[cfg(feature = "compile")]
pub use compile::{Compiled, compile_str, compile_str_stripped};
#[cfg(feature = "compile")]
pub use error::CompileError;

/// The localized numeric functions (`mf2-fn-number`, feature `fn-number`).
#[cfg(feature = "fn-number")]
pub use mf2_fn_number as fn_number;

/// The date/time functions (`mf2-fn-datetime`, feature `fn-datetime`).
#[cfg(feature = "fn-datetime")]
pub use mf2_fn_datetime as fn_datetime;

/// The native host (`mf2-host-std`).
#[cfg(feature = "host-std")]
pub use mf2_host_std as host_std;

/// The browser host (`mf2-host-web`).
#[cfg(feature = "host-web")]
pub use mf2_host_web as host_web;
