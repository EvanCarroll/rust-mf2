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
//! | `intl` | on `wasm32-unknown-unknown` ([`INTL_NUMBERS`]): numbers and plural selection through the browser's `Intl` (`host_web::NUMBERS_HOST`); the Rust path elsewhere |
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

mod arg;
#[cfg(feature = "compile")]
mod compile;
mod dynamic;
#[cfg(feature = "compile")]
mod error;
mod tr;

/// The call-site core (`plans/04-leptos-integration.md` §2.1): what `tr!`
/// builds, and what formats it against a catalog the caller supplies.
pub use arg::{ArgList, ArgSource, ArgValue, DateTimeValue, Text};
pub use dynamic::{TrDyn, tr_dyn};
pub use tr::{
    MarkupHandler, Tr, TrArgs, TrRich, markup, tr, tr_args_n, tr_args0, tr_args1, tr_args2,
    tr_args3, tr_args4, tr_rich,
};

/// The proc-macro behind the generated `tr!` wrapper — reached as
/// `__mf2::__tr_impl!`, never named by an application (`plans/05-tooling.md`
/// §4).
#[doc(hidden)]
pub use mf2_macros::__tr_impl;

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
pub use mf2_catalog::Manifest;
pub use mf2_catalog::{Catalog, CatalogError, Dir, Entry, MsgId, StrRef, markup_key};
pub use mf2_model::{ErrorKind, MarkupKind};
pub use mf2_runtime::{
    Arg, BidiStrategy, Category, CurrencyDisplay, CustomValue, Date, DateFields, DateLength,
    DateStyle, DateTime, DateTimeOptions, DateTimeRequest, DigitOptions, Digits, ErrorSink,
    ExpressionPart, FallbackSource, FnContext, FormatContext, FormatError, Formatter, Function,
    Grouping, Host, INTL_NUMBERS, Isolation, MarkupOptions, MarkupPart, Measure, MeasureUnit,
    NoErrors, Number, NumberFormatter, NumberOut, NumberRequest, NumberSpec, NumberStyle, Operands,
    OptionValue, Options, Part, PartSink, Registry, RoundingMode, RoundingPriority, Sign,
    SignDisplay, Sink, SubPartSink, Time, TimePrecision, TimeZone, UnitDisplay, Value, ZoneOption,
    ZoneStyle, functions, is_zone_name, plural_category,
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
