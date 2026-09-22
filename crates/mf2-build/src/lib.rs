//! `mf2-build` — a directory of translated message resources to what an
//! application ships: the manifest, one catalog per locale, and a generated
//! Rust module.
//!
//! It runs from an i18n crate's `build.rs`
//! (`mf2::build::Build::new().source_locale("en").run()`) and from `mf2-cli`,
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

#![forbid(unsafe_code)]

pub mod config;
mod error;
pub mod features;
pub mod lint;
pub mod loader;

pub use config::{CatalogConfig, Config, DataSet, Layout, LocaleDataConfig, Missing, Strip};
pub use error::{Error, Result};
pub use features::Features;
pub use lint::{Level, Lint};
pub use loader::{Loaded, Loader, Problem, Property, Record, SourceFile};
