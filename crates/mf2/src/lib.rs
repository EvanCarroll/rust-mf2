//! `mf2` — Unicode MessageFormat 2 for Rust applications: Leptos web
//! applications, and native command-line and terminal applications. It is
//! the crate an application names; beside it, the application's build
//! script names `mf2-build`.
//!
//! What a `tr!` call site builds is a small **description** of a message —
//! [`Tr`], [`TrArgs`], [`TrRich`], [`TrDyn`] — and nothing is formatted
//! until something renders or stringifies it. The descriptions and their
//! arguments ([`ArgValue`], converted from the call site's values through
//! [`IntoArg`]) are defined here, once, with every integration added behind
//! a feature:
//!
//! None is on by default, so `default-features = false` is never needed. The
//! features answer four questions; the user guide's *Features of `mf2`* page
//! says what each costs.
//!
//! | Feature | Adds |
//! |---|---|
//! | *(core)* | the descriptions, formatted against a [`Formatter`] the caller builds; [`mf2_runtime`]'s formatter: `:string`, `:number` / `:integer` / `:offset` with neutral symbols, markup, bidi, fallback |
//! | **Where does it run?** | |
//! | `leptos` / `leptos-0-8` | the Leptos line the layer renders with: Leptos 0.9 (the default line) or 0.8 |
//! | `ssr`, `hydrate`, `csr` | the Leptos layer, [`leptos`]: rendering in text, attributes and props, the catalog of the request or of the page, the live switch, the page's components; each mode implies its host, and `ssr` implies `tzdb-bundled` |
//! | `axum` | `mf2::axum`: each request's language, the catalogs served from the server binary, the generated `Locale` as an extractor (implies `host-std` and `tzdb-bundled`; refused when compiling for `wasm32` beside `hydrate` or `csr`) |
//! | `native` | [`native`]: a native application — a command-line tool, a terminal UI — with its catalogs embedded or beside the executable, installed once for the process, in the system's language (and, with `fn-datetime`, its time zone); the descriptions' `Display`, `to_string()` and `to_cow()` read them (std; implies `host-std`; beside `hydrate` or `csr`, refused when compiling for `wasm32`) |
//! | `ratatui` | [`ratatui`]: a terminal UI's text — a message as Ratatui `Text` or `Line`, its markup as styles (implies `native`; `ratatui-core` alone; beside `hydrate` or `csr`, refused when compiling for `wasm32`) |
//! | `clap` | a `clap` value parser on the generated `Locale`: `--lang` matched as the system's language is, and listed in `--help` |
//! | `host-std` / `host-web` | a [`Host`]: native (servers, tests, `wasm32-wasip1`), or the browser — with no framework, how an application uses `mf2` |
//! | **What can messages do?** | |
//! | `fn-number` | [`fn_number`]: `:number` / `:integer` / `:offset` localized, `:percent`, `:currency`, `:unit`, localized unannotated numbers |
//! | `fn-datetime` | [`fn_datetime`]: `:datetime` / `:date` / `:time`, unannotated date/time values (`Registry::with_dates`), and named time zones — over the neutral stub backend until a backend is on; with a Leptos mode, also dates in the reader's time zone |
//! | **Who supplies locale data?** | |
//! | `number-intl` | on `wasm32-unknown-unknown` (`INTL_NUMBERS`): numbers and plural selection through the browser's `Intl` (`host_web::NUMBERS_HOST`); the Rust path elsewhere |
//! | `datetime-icu` | ICU4X on client and server, data from the catalog's `icu.blob` (and [`compile_str`] emits it) |
//! | `datetime-intl` | the browser's `Intl.DateTimeFormat` on `wasm32-unknown-unknown`; ICU4X with compiled data elsewhere. With `datetime-icu` too, the browser keeps `Intl` and every other target ICU4X over the `icu.blob` |
//! | `tzdb-bundled` | named time zones from the IANA database built into the binary, not the machine's (nothing without `fn-datetime`) |
//! | **Behaviour and tools** | |
//! | `static-locale` | a locale switch is a cookie and a navigation (for islands) |
//! | `mark-fallback-lang` | text borrowed from a fallback language is marked with its own `lang` |
//! | `compile` | [`compile_str`]: an ad-hoc message as a one-message catalog (std; servers and tests) |
//!
//! A Leptos mode needs a line, and the modes exclude each other: an
//! application writes the line on its `mf2` dependency (`features =
//! ["leptos"]`) and the mode where it writes Leptos's own (`ssr =
//! ["leptos/ssr", "mf2/ssr"]`). This documentation shows `ssr` on Leptos
//! 0.9, and `native` and `ratatui`, which compile beside it; [`leptos`]
//! lists what the client modes add. `host-web` and `number-intl` are for
//! `wasm32-unknown-unknown`, so [`host_web`](https://docs.rs/mf2-host-web)
//! is not shown here. A native application turns on `native` (a terminal
//! UI, `ratatui`), and no Leptos mode.
//!
//! A build with no mode compiles no Leptos code, so a server and a test use
//! the descriptions as they are: formatted against a catalog the caller
//! supplies. A native application installs its catalogs with [`native`],
//! and its descriptions then show their text wherever text is wanted
//! (`println!("{}", tr!("welcome"))`).
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
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide: how the crates fit together, web and native applications, the
//! command line, and what 2.x promises.

#![warn(missing_docs, missing_debug_implementations)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
// The Leptos layer's items name the modes an application turns on, not the
// line they need (`doc(cfg(...))` below).
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
#![forbid(unsafe_code)]
// The call-site core is what 2,000 client call sites are made of, so the
// crate keeps `mf2-runtime`'s client-path discipline (B12): no panicking
// operation, and no `core::fmt` on the client path.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

extern crate alloc;
// The Leptos layer needs `std`, which its dependencies need anyway; so does
// the native module, which reads files and the system's settings. With the
// native host (a server, a test, a native application) `std` is linked
// already, and `tr!` takes paths and `SystemTime` as arguments. clap is std.
#[cfg(any(
    feature = "host-std",
    feature = "native",
    feature = "clap",
    all(
        any(feature = "ssr", feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    )
))]
extern crate std;

// An application renders on one side or the other, and the two need
// different code: `ssr` keeps the catalog in the request's context, the
// other two in a `thread_local!`. Saying so here turns a confusing cascade
// of missing-item errors into one sentence.
#[cfg(all(feature = "ssr", any(feature = "hydrate", feature = "csr")))]
compile_error!(
    "mf2: turn on exactly one of `ssr`, `hydrate` and `csr`. cargo unifies \
     features across a workspace, so an application that is built both ways \
     belongs in a workspace of its own — as `examples/demo-ssr` and \
     `conformance/l6-web` are."
);
#[cfg(all(feature = "hydrate", feature = "csr"))]
compile_error!("mf2: turn on exactly one of `ssr`, `hydrate` and `csr`.");

// The Leptos line: `leptos` is 0.9, the default line; `leptos-0-8` the
// other. A mode needs exactly one. The layer below is compiled only with a
// mode and a line, so each of these mistakes shows its own sentence and
// nothing else.
#[cfg(all(feature = "leptos", feature = "leptos-0-8"))]
compile_error!(
    "mf2: both Leptos lines are on, `leptos` (Leptos 0.9) and `leptos-0-8`: \
     turn on one."
);
#[cfg(all(feature = "ssr", not(any(feature = "leptos", feature = "leptos-0-8"))))]
compile_error!(
    "mf2: `ssr` needs a Leptos line: turn on `leptos` (Leptos 0.9) or \
     `leptos-0-8` beside it."
);
#[cfg(all(
    feature = "hydrate",
    not(any(feature = "leptos", feature = "leptos-0-8"))
))]
compile_error!(
    "mf2: `hydrate` needs a Leptos line: turn on `leptos` (Leptos 0.9) or \
     `leptos-0-8` beside it."
);
#[cfg(all(feature = "csr", not(any(feature = "leptos", feature = "leptos-0-8"))))]
compile_error!(
    "mf2: `csr` needs a Leptos line: turn on `leptos` (Leptos 0.9) or \
     `leptos-0-8` beside it."
);

// A native application's module reads the system's settings and files
// beside the executable: it has no place in a browser build, which is what
// `hydrate` and `csr` make for `wasm32`. On the host the two compile
// together: cargo unifies features across the packages it builds together,
// so `cargo check --workspace` (and rust-analyzer's check) over a browser
// client and a native application turns both on, as 1.x allowed
// (plans/19-native-and-terminal.md §3). `ratatui` implies `native` and has
// a sentence of its own, below, so that the one error names the feature an
// application turned on.
#[cfg(all(
    feature = "native",
    not(feature = "ratatui"),
    any(feature = "hydrate", feature = "csr"),
    target_arch = "wasm32"
))]
compile_error!(
    "mf2: `native` is on beside `hydrate` or `csr` in a build for the \
     browser (`wasm32`): `native` is for an application that runs natively \
     (a command-line tool, a terminal UI, a server), never for a browser \
     build. cargo unifies features across the packages it builds together: \
     build the browser client on its own (`-p`), and keep `native` off in \
     every crate it depends on."
);
// `axum` serves HTTP: a server's, never a browser build's. As `native`,
// it is refused beside a client mode only when compiling for the browser.
#[cfg(all(
    feature = "axum",
    any(feature = "hydrate", feature = "csr"),
    target_arch = "wasm32"
))]
compile_error!(
    "mf2: `axum` is on beside `hydrate` or `csr` in a build for the \
     browser (`wasm32`): `axum` is for the server, never for a browser \
     build. cargo unifies features across the packages it builds together: \
     build the browser client on its own (`-p`), and keep `axum` off in \
     every crate it depends on."
);
#[cfg(all(
    feature = "ratatui",
    any(feature = "hydrate", feature = "csr"),
    target_arch = "wasm32"
))]
compile_error!(
    "mf2: `ratatui` is on beside `hydrate` or `csr` in a build for the \
     browser (`wasm32`): `ratatui` is for a terminal UI, which runs \
     natively, and implies `native`; neither belongs in a browser build. \
     cargo unifies features across the packages it builds together: build \
     the browser client on its own (`-p`), and keep `ratatui` and `native` \
     off in every crate it depends on."
);

// Each Leptos line is a dependency under a name of its own (`leptos_0_9`,
// `leptos_0_8`, …), reached through these internal aliases. There is no
// rename at the crate root: the module `leptos` has that name, and a crate
// bound to it there would clash with it. The six components, which need
// `view!` and `#[component]`, are the line's helper crate's (`ui`).
#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
mod line {
    #[cfg(all(feature = "leptos-0-8", not(feature = "leptos")))]
    pub(crate) use ::{
        leptos_0_8 as leptos, mf2_leptos_ui_0_8 as ui, reactive_graph_0_2 as reactive_graph,
        tachys_0_2 as tachys,
    };
    #[cfg(feature = "leptos")]
    pub(crate) use ::{
        leptos_0_9 as leptos, mf2_leptos_ui_0_9 as ui, reactive_graph_0_3 as reactive_graph,
        tachys_0_3 as tachys,
    };
}

mod arg;
#[cfg(feature = "compile")]
mod compile;
mod corpus;
mod debug;
#[cfg(any(
    feature = "native",
    all(
        any(feature = "ssr", feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    )
))]
mod display;
mod dynamic;
mod error;
mod into_arg;
// The names the server and the browser's client share.
#[cfg(any(
    feature = "axum",
    all(
        any(feature = "ssr", feature = "hydrate", feature = "csr"),
        any(feature = "leptos", feature = "leptos-0-8")
    )
))]
mod links;
mod markup;
mod matching;
mod message;
mod tr;
// The server's one-time warnings (E4).
#[cfg(any(feature = "axum", feature = "ssr"))]
mod warn;

// A native application's catalogs and locale (its documentation is the
// module's own).
#[cfg(feature = "native")]
#[cfg_attr(docsrs, doc(cfg(feature = "native")))]
pub mod native;

// A terminal UI's text, through the native module's catalogs (its
// documentation is the module's own).
#[cfg(feature = "ratatui")]
#[cfg_attr(docsrs, doc(cfg(feature = "ratatui")))]
pub mod ratatui;

// An Axum server's negotiation, catalog routes and extractor, with or
// without Leptos (its documentation is the module's own).
#[cfg(feature = "axum")]
#[cfg_attr(docsrs, doc(cfg(feature = "axum")))]
pub mod axum;

// The Leptos layer (its documentation is the module's own: an outer doc
// comment here would make rustdoc resolve the module's links at the root).
#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
pub mod leptos;

pub use corpus::{CatalogFile, Corpus};
pub use error::UnknownLocale;
pub use matching::LanguageMatching;
pub use message::Message;

/// The call-site core: what `tr!` builds, and what formats it against a
/// catalog the caller supplies.
pub use arg::{ArgList, ArgSource, ArgValue, DateTimeValue, Text};
pub use dynamic::TrDyn;
pub use into_arg::IntoArg;
pub use markup::{Handler, IntoMarkupHandler};
pub use tr::{MarkupHandler, Tr, TrArgs, TrRich};

/// What `tr!` and the generated module expand to; never written by hand.
#[doc(hidden)]
pub use dynamic::tr_dyn;
// How `tr!` converts an argument (`IntoArg`, else 1.x's `From`, else
// `Display`, else `IntoArg`'s message); never written by hand. Its
// documentation is the module's own: an outer doc comment here would make
// rustdoc resolve the module's links at the root.
#[doc(hidden)]
pub mod __arg;
// What the generated module calls: the cfg-forwarding macros and the typed
// forms' helpers; never written by hand. Its documentation is the module's
// own.
#[doc(hidden)]
pub mod __generated;
#[doc(hidden)]
pub use markup::{markup, markup_view};
#[doc(hidden)]
pub use tr::{tr, tr_args_n, tr_args0, tr_args1, tr_args2, tr_args3, tr_args4, tr_rich};

/// The markup handlers and the reactive argument of the Leptos layer, where
/// 1.x named them; [`leptos`] is their home.
#[cfg(all(
    any(feature = "ssr", feature = "hydrate", feature = "csr"),
    any(feature = "leptos", feature = "leptos-0-8")
))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "ssr", feature = "hydrate", feature = "csr")))
)]
#[doc(no_inline)]
pub use leptos::{Flat, FlatHandler, NestingHandler, SignalArg, signal_arg};

/// The proc-macro behind the generated `tr!` wrapper — reached as
/// `__mf2::__tr_impl!`, never named by an application.
#[doc(hidden)]
pub use mf2_macros::__tr_impl;

/// The proc-macro behind the generated `msg_id!` wrapper.
#[doc(hidden)]
pub use mf2_macros::__msg_id_impl;

/// Includes what `mf2-build` wrote into `OUT_DIR`: the manifest hash, the
/// locale table, the closed-world registry, the host, `__mf2` and the `tr!`
/// wrapper. An i18n crate's whole `src/lib.rs` is
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

/// What [`leptos::islands_gate!`] is; never named by this path.
#[cfg(all(feature = "hydrate", any(feature = "leptos", feature = "leptos-0-8")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __islands_gate {
    () => {
        $crate::__islands_gate_export! {}
    };
}

/// The export [`leptos::islands_gate!`] writes into the application.
///
/// A macro of its own, so that the one `mf2::leptos` re-exports carries no
/// `#[rustfmt::skip]`: rustc counts a macro with a tool attribute as
/// macro-expanded, and such a macro cannot be re-exported by path from its
/// own crate. rustfmt re-indents a `$crate` attribute inside a macro on
/// every run, so this one is not formatted.
#[cfg(all(feature = "hydrate", any(feature = "leptos", feature = "leptos-0-8")))]
#[doc(hidden)]
#[macro_export]
#[rustfmt::skip]
macro_rules! __islands_gate_export {
    () => {
        #[$crate::leptos::__private::wasm_bindgen::prelude::wasm_bindgen(
            wasm_bindgen = $crate::leptos::__private::wasm_bindgen,
            wasm_bindgen_futures = $crate::leptos::__private::wasm_bindgen_futures,
            js_name = "mf2_islands_gate"
        )]
        #[doc(hidden)]
        pub async fn __mf2_islands_gate(_island: $crate::leptos::__private::web_sys::HtmlElement) {
            $crate::leptos::wait_for_catalog().await
        }
    };
}

/// What [`leptos::islands_gate!`] is in a server build: nothing to export.
#[cfg(not(all(feature = "hydrate", any(feature = "leptos", feature = "leptos-0-8"))))]
#[doc(hidden)]
#[macro_export]
macro_rules! __islands_gate {
    () => {};
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
