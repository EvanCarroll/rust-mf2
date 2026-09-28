//! `mf2-catalog` — the `.mf2b` binary catalog of rust-mf2: one locale's
//! messages and locale data, as a lossless encoding of the MF2 data model that the client reads in place.
//!
//! An application meets one type of this crate: [`Catalog`], a loaded
//! catalog, which `leptos-mf2` and `mf2-axum` load and serve for it and
//! which [`mf2_runtime`'s formatter](https://docs.rs/mf2-runtime) formats
//! from — with its errors ([`CatalogError`], and on the build side
//! `WriteError` and `ManifestError`). Everything else here is the byte
//! format, which 1.x does not promise (`docs/versioning.md`: rebuild the
//! server and the client together), and is hidden from the documentation:
//!
//! | Part | Feature | Side |
//! |---|---|---|
//! | `Catalog` and its views (`MsgView`, …) — the reader | *(always)* | client: `no_std`, no allocation, no panics, no `core::fmt` |
//! | `number`, `currency`, `unit` — views of the `number.*`, `currency.data` and `unit.data` LOCALE entries (`mf2-fn-number` reads them) | *(always)* | client, same rules |
//! | `Manifest` — `manifest.mf2m` and `manifest_hash` | `manifest` | build |
//! | `writer` — `writer::catalog`, `writer::single` | `writer` | build |
//! | `decode()` — the model-rebuilding decoder (for tests) | `decode` | build |
//!
//! The reader is client-path code: `Catalog::new` takes the fetched buffer
//! and validates its structure once, in linear time; every accessor is a
//! bounds-checked O(1) read that never panics; strings are checked as UTF-8
//! when read; nothing formats. `StrRef` is opaque and loading is one
//! function — the two seams kept for catalog text as JS strings.
//!
//! The byte format is version 1; its constants are in `format`.
//!
//! # The user guide
//!
//! Getting started, call sites, delivery modes, switching language,
//! accessibility, migrating from `leptos-fluent`, and what 1.x promises
//! (`versioning.md`): the user guide is the `docs/` directory of the
//! mf2-two repository. An application starts at
//! [`mf2`](https://docs.rs/mf2).

#![warn(missing_docs)]
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

mod bytes;
#[doc(hidden)]
pub mod currency;
#[cfg(feature = "decode")]
mod decode;
mod error;
#[doc(hidden)]
pub mod format;
#[cfg(feature = "manifest")]
mod manifest;
mod markup;
#[doc(hidden)]
pub mod number;
mod plural;
mod reader;
#[doc(hidden)]
pub mod unit;
mod view;
#[cfg(feature = "writer")]
#[doc(hidden)]
pub mod writer;

pub use mf2_model::{Dir, MsgId};

#[cfg(feature = "decode")]
#[doc(hidden)]
pub use decode::{Decoded, decode, decode_report};
pub use error::CatalogError;
#[cfg(feature = "decode")]
#[doc(hidden)]
pub use error::DecodeError;
#[cfg(feature = "manifest")]
pub use error::ManifestError;
#[cfg(feature = "writer")]
pub use error::WriteError;
#[cfg(feature = "manifest")]
#[doc(hidden)]
pub use manifest::Manifest;
#[doc(hidden)]
pub use markup::markup_key;
pub use reader::{Catalog, CldrVersion};
#[doc(hidden)]
pub use reader::{Entry, StrRef};
#[doc(hidden)]
pub use view::{
    Body, DeclView, Declarations, ExprView, FunctionView, KeyView, Keys, Malformed, MarkupView,
    MsgView, Names, Operand, OptionsView, PartView, Parts, PatternView, SelectView, Selectors,
    VarRef, VariantView, Variants,
};
