//! `mf2-build` — a directory of translated message resources to what an
//! application ships: the manifest, one catalog per locale, and a generated
//! Rust module.
//!
//! It runs from the `build.rs` of the crate that includes the generated
//! module — `fn main() { mf2_build::run(); }`, which reads `mf2`'s features
//! through `links` ([`run`]) — and from `mf2-cli`, reading the same
//! [`Config`] both times so that the two can never disagree. Nothing here is
//! linked into the client wasm.
//!
//! ```text
//! locales/<tag>/*.mf2   ─▶ Loader ─▶ records ─▶ mf2-syntax ─▶ models
//!                                                  │
//!                                     manifest ◀───┤  ids → MsgId, slots,
//!                                                  │  markup, functions, hash
//!                             lints (mf2 check) ◀──┤
//!                                                  ▼
//!                                      fallbacks flattened
//!                                                  ▼
//!                       locale data sliced ─▶ mf2-catalog writer ─▶ .mf2b + .br/.gz
//!                                                  ▼
//!                                       the generated Rust module
//! ```
//!
//! # What 2.x promises here
//!
//! What an i18n crate's `build.rs` and a tool call: [`Build`] and what it
//! returns ([`Outcome`], [`Published`], [`LocaleInfo`], [`Report`],
//! [`Diagnostic`]), the `mf2.toml` it reads ([`Config`] and its parts), the
//! client [`Features`], the lints ([`Lint`], [`Level`]) and the [`Error`].
//! The pipeline — the modules below, which `mf2-cli` and the conformance
//! crate reach into — is private or hidden from the documentation, and not
//! promised (`docs/versioning.md`):
//!
//! | Module | What it does |
//! |---|---|
//! | `config` | `mf2.toml` |
//! | `features` | the client feature set, from `CARGO_FEATURE_*` or `--features` |
//! | `loader` | a locale's files to records: `.mf2` resources or flat JSON |
//! | `lint` | the names and levels of `mf2 check`'s lints |
//! | `corpus` | reading and parsing every locale, with errors placed in their files |
//! | `manifest` | ids → `MsgId`, slots, markup, functions, the hash |
//! | `check` | the lints themselves |
//! | `slice` | the locale data a corpus needs |
//! | `catalog` | one catalog per locale: fallbacks, compression, content hashes |
//! | `codegen` | the generated Rust module |
//! | `pseudo` | the pseudo-locales `en-XA` and `ar-XB` |
//! | `build` | [`Build`], which runs all of it |
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide: how the crates fit together, web and native applications, the
//! command line, and what 2.x promises.
//! An application names this crate in its i18n crate's
//! `[build-dependencies]`, and [`mf2`](https://docs.rs/mf2) in its
//! `[dependencies]`.

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
// `Error` carries a path and the error it wraps, which makes it wide. A build
// returns one of these at most once per run, never in a loop, so keeping the
// variants readable is worth more than the bytes.
#![allow(clippy::result_large_err)]

mod build;
#[doc(hidden)]
pub mod catalog;
#[doc(hidden)]
pub mod check;
#[doc(hidden)]
pub mod codegen;
mod config;
#[doc(hidden)]
pub mod corpus;
mod error;
mod features;
mod lint;
#[doc(hidden)]
pub mod loader;
#[doc(hidden)]
pub mod manifest;
#[doc(hidden)]
pub mod pseudo;
mod report;
mod run;
#[doc(hidden)]
pub mod slice;

pub use build::{Build, Emit, LocaleInfo, Outcome, Published};
/// What `mf2-cli` and the conformance crate read of the modules above,
/// which are private: the client feature set's function and option tables,
/// and where `mf2.toml` and `locales/` are.
#[doc(hidden)]
pub use config::FILE_NAME as CONFIG_FILE;
#[doc(hidden)]
pub use config::Layout;
pub use config::{CatalogConfig, Config, DataSet, LocaleDataConfig, Missing, Strip};
pub use error::{Error, Result};
#[doc(hidden)]
pub use features::{BUILTINS, CATALOG_FEATURES, OPTIONS, defines_option};
pub use features::{DATE_LINES, DateFamily, DateFormatter, Features, Side};
pub use lint::{Level, Lint};
#[doc(hidden)]
pub use loader::{Loaded, Loader, Problem, Property, Record, SourceFile};
/// The manifest a build derived from the source locale (`mf2-catalog`).
#[doc(hidden)]
pub use mf2_catalog::Manifest;
pub use report::{Diagnostic, Report};
pub use run::run;
