//! `mf2-resource` — the **W3C Message Resource** container for MessageFormat
//! 2: a parser, a serializer and the data model, generic over the message
//! type.
//!
//! Unicode defines no file format for MF2; its specification leaves that to
//! a future message resource specification. rust-mf2 adopts the draft being
//! written for it, incubated by the W3C i18n WG.
//!
//! ```text
//! # A comment about the file.
//! @locale en-US
//! ---
//!
//! chat-send = Send
//!
//! @param $count - How many people are in the room; a whole number.
//! users-online =
//!   .input {$count :integer}
//!   .match $count
//!   one {{{$count} user online}}
//!   *   {{{$count} users online}}
//!
//! [hotkeys]
//! # → id "hotkeys.release"
//! release = Release {#kbd}?{/kbd} to close
//! ```
//!
//! # Not part of 1.x's promise
//!
//! `mf2-build` and the `mf2` command line read and write `.mf2` files with
//! this crate; an application never names it. What 1.x promises is the
//! **file format as `mf2 fmt` writes it** (`docs/versioning.md`), not this
//! Rust API: it mirrors a draft, and follows the draft as it changes. Its
//! items are therefore hidden from the documentation.
//!
//! | Entry point | Gives |
//! |---|---|
//! | `parse` | a `Resource` of MF2 sources as written, plus the syntax errors found |
//! | `Resource::map_values` | the draft's `Resource<Message>`: the same tree with parsed messages |
//! | `serialize` / `serialize_with` | canonical source (`mf2 fmt`) |
//! | `LineIndex` | a byte offset as a line and a column |
//! | `ValueMap` | a cooked value's offset back to where the file wrote it |
//!
//! # The draft is not vendored
//!
//! The draft states no license, so nothing is copied from it: this crate
//! implements a grammar written from the draft at a pinned revision. **Where
//! the draft and this crate disagree, the draft wins**, and the crate
//! follows it. The same holds for the JSON shape behind the `serde`
//! feature.
//!
//! `#![no_std]` + `alloc`; never linked into the client wasm.
//!
//! # The user guide
//!
//! Getting started, call sites, delivery modes, switching language,
//! accessibility, migrating from `leptos-fluent`, and what 1.x promises
//! (`versioning.md`): the user guide is the `docs/` directory of the
//! mf2-two repository. Its getting-started page shows the files this crate
//! reads, and `versioning.md` what 1.x promises about their format.

#![warn(missing_docs)]
// docs.rs (`cargo xtask docs-rs`): each feature-gated item says which features it needs.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[doc(hidden)]
pub mod code;
mod diagnostic;
mod error;
#[cfg(feature = "serde")]
mod json;
mod lines;
mod model;
mod parse;
mod serialize;

#[doc(hidden)]
pub use diagnostic::Diagnostic;
#[doc(hidden)]
pub use error::{Error, Role};
#[doc(hidden)]
pub use lines::{LineIndex, Position};
// Re-exported so that building a resource needs only this crate.
#[doc(hidden)]
pub use mf2_model::Span;
#[doc(hidden)]
pub use model::{
    Comment, Detached, Entry, EntryInfo, EntryRef, Head, Id, Meta, Resource, Section, Segment,
    ValueMap, is_id_char,
};
#[doc(hidden)]
pub use parse::parse;
#[doc(hidden)]
pub use serialize::{Style, serialize, serialize_with};
