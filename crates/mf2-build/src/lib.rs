//! `mf2-build` — a directory of translated message resources to what an
//! application ships: the manifest, one catalog per locale, and a generated
//! Rust module.
//!
//! It runs from an i18n crate's `build.rs`
//! (`Build::new()?.emit_cargo(true).run()?.into_result()?`) and from
//! `mf2-cli`,
//! reading the same [`Config`] both times so that the two can never disagree
//! (`plans/05-tooling.md` §3.1, §4). Nothing here is linked into the client
//! wasm.
//!
//! ```text
//! locales/<tag>/*.mf2   ─▶ Loader ─▶ records ─▶ mf2-syntax ─▶ models
//!                                                  │
//!                                     manifest ◀───┤  ids → MsgId, slots,
//!                                                  │  markup, functions, hash
//!                             lints (mf2 check) ◀──┤
//!                                                  ▼
//!                                   fallbacks flattened (D5)
//!                                                  ▼
//!                       locale data sliced ─▶ mf2-catalog writer ─▶ .mf2b + .br/.gz
//!                                                  ▼
//!                                       the generated Rust module
//! ```
//!
//! | Module | What it does |
//! |---|---|
//! | [`config`] | `mf2.toml` |
//! | [`features`] | the client feature set, from `CARGO_FEATURE_*` or `--features` |
//! | [`loader`] | a locale's files to records: `.mf2` resources or flat JSON |
//! | [`lint`] | the names and levels of `mf2 check`'s lints |
//! | [`corpus`] | reading and parsing every locale, with errors placed in their files |
//! | [`manifest`] | ids → `MsgId`, slots, markup, functions, the hash |
//! | [`check`] | the lints themselves |
//! | [`slice`] | the locale data a corpus needs |
//! | [`catalog`] | one catalog per locale: fallbacks, compression, content hashes |
//! | [`codegen`] | the generated Rust module |
//! | [`pseudo`] | the pseudo-locales `en-XA` and `ar-XB` |
//! | [`build`] | [`Build`], which runs all of it |

#![forbid(unsafe_code)]
// `Error` carries a path and the error it wraps, which makes it wide. A build
// returns one of these at most once per run, never in a loop, so keeping the
// variants readable is worth more than the bytes.
#![allow(clippy::result_large_err)]

pub mod build;
pub mod catalog;
pub mod check;
pub mod codegen;
pub mod config;
pub mod corpus;
mod error;
pub mod features;
pub mod lint;
pub mod loader;
pub mod manifest;
pub mod pseudo;
pub mod report;
pub mod slice;

pub use build::{Build, Emit, LocaleInfo, Outcome, Published};
pub use config::{CatalogConfig, Config, DataSet, Layout, LocaleDataConfig, Missing, Strip};
pub use error::{Error, Result};
pub use features::Features;
pub use lint::{Level, Lint};
pub use loader::{Loaded, Loader, Problem, Property, Record, SourceFile};
/// The manifest a build derived from the source locale (`mf2-catalog`).
pub use mf2_catalog::Manifest;
pub use report::{Diagnostic, Report};
