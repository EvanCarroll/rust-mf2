//! `mf2-fn-datetime-web-icu` — ICU4X for the browser build of Rust MF2's
//! date functions.
//!
//! `mf2-fn-datetime` formats dates with ICU4X on either side of an
//! application: natively with its `std-icu` feature, in the browser
//! (`wasm32-unknown-unknown`) with `web-icu`. Each side compiles ICU4X only
//! with its own feature, so a server built with ICU4X beside a browser build
//! on `Intl.DateTimeFormat` puts no ICU4X crate into the browser's build.
//! Cargo scopes an optional dependency by target but not by feature, and
//! does not let one package name another twice, so the browser side reaches
//! ICU4X through this crate: `mf2-fn-datetime` names it for
//! `wasm32-unknown-unknown` only, and names the ICU4X crates themselves for
//! every other target.
//!
//! It holds no code: it re-exports the crates the ICU4X backend uses, with
//! the features that read the catalog's `icu.blob`.
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide. Applications never name this crate: `mf2`'s browser date
//! families (`host-web-datetime-icu`, `leptos-client-datetime-icu`) reach it.

#![no_std]
#![forbid(unsafe_code)]

pub use icu_calendar;
pub use icu_datetime;
pub use icu_locale_core;
pub use icu_provider;
pub use icu_provider_blob;
pub use icu_time;
pub use writeable;
