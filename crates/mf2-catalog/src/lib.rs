//! `mf2-catalog` — the `.mf2b` binary catalog of mf2-two
//! (`plans/02-catalog-format.md`): one locale's messages and locale data, as
//! a lossless encoding of the MF2 data model that the client reads in place.
//!
//! | Part | Feature | Side |
//! |---|---|---|
//! | [`Catalog`] and its views ([`MsgView`], …) — the reader | *(always)* | client: `no_std`, no allocation, no panics, no `core::fmt` |
//! | [`Manifest`] — `manifest.mf2m` and `manifest_hash` | `manifest` | build |
//! | [`writer`] — `writer::catalog`, `writer::single` | `writer` | build |
//! | [`decode()`] — the model-rebuilding decoder (layer L3) | `decode` | build |
//!
//! The reader is client-path code: `Catalog::new` takes the fetched buffer
//! and validates its structure once, linearly (F2, F4); every accessor is a
//! bounds-checked O(1) read that never panics; strings are checked as UTF-8
//! when read; nothing formats. [`StrRef`] is opaque and loading is one
//! function — the two seams kept for catalog text as JS strings.
//!
//! The byte format is version 1, frozen at the exit of Phase 2
//! (`plans/02-catalog-format.md` §2); its constants are in [`format`].

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
#[cfg(feature = "decode")]
mod decode;
mod error;
pub mod format;
#[cfg(feature = "manifest")]
mod manifest;
mod plural;
mod reader;
mod view;
#[cfg(feature = "writer")]
pub mod writer;

pub use mf2_model::{Dir, MsgId};

#[cfg(feature = "decode")]
pub use decode::{Decoded, decode, decode_report};
pub use error::CatalogError;
#[cfg(feature = "decode")]
pub use error::DecodeError;
#[cfg(feature = "manifest")]
pub use error::ManifestError;
#[cfg(feature = "writer")]
pub use error::WriteError;
#[cfg(feature = "manifest")]
pub use manifest::Manifest;
pub use reader::{Catalog, CldrVersion, Entry, StrRef};
pub use view::{
    Body, DeclView, Declarations, ExprView, FunctionView, KeyView, Keys, Malformed, MarkupView,
    MsgView, Names, Operand, OptionsView, PartView, Parts, PatternView, SelectView, Selectors,
    VarRef, VariantView, Variants,
};
