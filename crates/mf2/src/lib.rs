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
//! | `fn-datetime` | [`fn_datetime`]: `:datetime` / `:date` / `:time`, unannotated date/time values (`Registry::with_dates`) — over the neutral stub backend until a backend is on |
//! | `datetime-icu` | ICU4X on client and server, data from the catalog's `icu.blob` (and [`compile_str`] emits it) |
//! | `datetime-intl` | the browser's `Intl.DateTimeFormat` on `wasm32-unknown-unknown`; ICU4X with compiled data elsewhere |
//! | `host-std` / `host-web` | a [`Host`]: native (and `wasm32-wasip1`), or the browser |
//! | `intl` | on `wasm32-unknown-unknown` (`INTL_NUMBERS`): numbers and plural selection through the browser's `Intl` (`host_web::NUMBERS_HOST`); the Rust path elsewhere |
//!
//! Beyond the re-exports the facade carries one thing of its own: the
//! **call-site core** (`plans/04-leptos-integration.md` §2.1) — [`Tr`],
//! [`TrArgs`], [`TrRich`], [`ArgValue`] and the lowering that borrows them
//! into the runtime's [`Arg`], with [`include_generated!`] and the `tr!`
//! proc-macro behind it. It is Leptos-free, so a server, a test and
//! `mf2-cli` use it as they are; Phase 6's `leptos-mf2` adds rendering, the
//! catalog context and the reactive argument on top of it.
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
//!
//! With [`fn_datetime`], a date: a handler over a chosen backend (here the
//! neutral stub; `DATETIME` and `DATES` are these over the default one) and
//! the registry that formats unannotated date/time values with it.
//!
//! ```
//! # #[cfg(all(feature = "compile", feature = "host-std", feature = "fn-datetime"))] {
//! use mf2::fn_datetime::{DateTimeFunction, Neutral};
//! use mf2::{FormatContext, Formatter, Registry};
//!
//! static DATETIME: DateTimeFunction<Neutral> = DateTimeFunction::datetime(Neutral);
//! static DATES: DateTimeFunction<Neutral> = DateTimeFunction::unannotated(Neutral);
//! static FUNCTIONS: [(&str, &dyn mf2::Function); 1] = [("datetime", &DATETIME)];
//! static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_dates(&DATES);
//! static CX: FormatContext = FormatContext::new(&mf2::host_std::HOST);
//!
//! let m = mf2::compile_str("{|2006-01-02T15:04:06| :datetime timePrecision=second}", "en").unwrap();
//! let mut out = String::new();
//! let mut errors = Vec::new();
//! Formatter::new(&m.catalog, &REGISTRY, &CX).write(mf2::Compiled::ID, &[], &mut out, &mut errors);
//! assert_eq!(out, "2006-01-02 15:04:06");
//! assert!(errors.is_empty());
//! # }
//! ```

#![warn(missing_docs)]
#![no_std]
#![forbid(unsafe_code)]
// The call-site core is what 2,000 client call sites are made of, so the
// facade keeps `mf2-runtime`'s client-path discipline (B12).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

extern crate alloc;

#[cfg(feature = "compile")]
mod compile;
#[cfg(feature = "compile")]
mod error;

/// The call-site core (`plans/04-leptos-integration.md` §2.1): what `tr!`
/// builds, and what formats it against a catalog the caller supplies.
///
/// It is declared in `leptos-mf2` and named here, because Rust's orphan rule
/// keeps a type and its `Render` impl in one crate (that crate's `lib.rs`
/// says why). Without the `leptos` feature nothing of Leptos is compiled,
/// so `mf2::Tr` is the Leptos-free description §2.1 describes.
pub use leptos_mf2::{
    ArgList, ArgSource, ArgValue, DateTimeValue, Handler, IntoMarkupHandler, MarkupHandler, Text,
    Tr, TrArgs, TrDyn, TrRich,
};

/// What `tr!` and the generated module expand to; never written by hand.
#[doc(hidden)]
pub use leptos_mf2::{
    markup, tr, tr_args_n, tr_args0, tr_args1, tr_args2, tr_args3, tr_args4, tr_dyn, tr_rich,
};

/// The Leptos layer (`plans/04-leptos-integration.md` §§3–7).
#[cfg(feature = "leptos")]
pub use leptos_mf2::{Flat, FlatHandler, NestingHandler, SignalArg, signal_arg};

/// The Leptos layer in full, for what this facade does not name one by one.
#[cfg(feature = "leptos")]
pub use leptos_mf2;

/// The proc-macro behind the generated `tr!` wrapper — reached as
/// `__mf2::__tr_impl!`, never named by an application (`plans/05-tooling.md`
/// §4).
#[doc(hidden)]
pub use mf2_macros::__tr_impl;

/// The proc-macro behind the generated `msg_id!` wrapper.
#[doc(hidden)]
pub use mf2_macros::__msg_id_impl;

/// Includes what `mf2-build` wrote into `OUT_DIR`: the manifest hash, the
/// locale table, the closed-world registry, the host, `__mf2` and the `tr!`
/// wrapper (`plans/05-tooling.md` §4). An i18n crate's whole `src/lib.rs` is
///
/// ```ignore
/// mf2::include_generated!();
/// ```
///
/// With `include_generated!(catalogs)` it includes the catalog table
/// instead, for the server-only crate a build with
/// `Build::emit(Emit::Catalogs)` writes.
#[macro_export]
macro_rules! include_generated {
    () => {
        include!(concat!(env!("OUT_DIR"), "/mf2_generated.rs"));
    };
    (catalogs) => {
        include!(concat!(env!("OUT_DIR"), "/mf2_catalogs.rs"));
    };
    ($file:literal) => {
        include!(concat!(env!("OUT_DIR"), "/", $file));
    };
}

/// The manifest (build side: `mf2-catalog`'s `manifest` feature).
#[cfg(feature = "compile")]
#[doc(hidden)]
pub use mf2_catalog::Manifest;
pub use mf2_catalog::{Catalog, CatalogError, Dir, MsgId};
/// The catalog's layout, which the generated module and the function crates
/// read.
#[doc(hidden)]
pub use mf2_catalog::{Entry, StrRef, markup_key};
pub use mf2_model::{ErrorKind, MarkupKind};
pub use mf2_runtime::{
    Arg, BidiStrategy, Category, CurrencyDisplay, CustomValue, Date, DateFields, DateLength,
    DateStyle, DateTime, DateTimeOptions, DateTimeRequest, DigitOptions, Digits, ErrorSink,
    ExpressionPart, FallbackSource, FnContext, FormatContext, FormatError, Formatter, Function,
    Grouping, Host, Isolation, MarkupOptions, MarkupPart, Measure, MeasureUnit, NoErrors, Number,
    NumberFormatter, NumberOut, NumberRequest, NumberSpec, NumberStyle, Operands, OptionValue,
    Options, Part, PartSink, Registry, RoundingMode, RoundingPriority, Sign, SignDisplay, Sink,
    SubPartSink, Time, TimePrecision, TimeZone, UnitDisplay, Value, ZoneOption, ZoneStyle,
    functions, is_zone_name,
};
#[doc(hidden)]
pub use mf2_runtime::{INTL_NUMBERS, plural_category};

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
