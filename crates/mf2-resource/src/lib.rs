//! `mf2-resource` — the **W3C Message Resource** container for MessageFormat
//! 2: a parser, a serializer and the data model, generic over the message
//! type.
//!
//! Unicode defines no file format for MF2 (`spec/syntax.md` points at "a
//! future *`MessageResource`* specification"). mf2-two adopts the draft the MF2
//! spec editor is writing, incubated by the W3C i18n WG — decision D2 of the
//! master plan (`plans/00-master-plan.md`), with the format described in
//! `plans/05-tooling.md` §2.
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
//! The draft states no license (`third_party/w3c-message-resource/PIN`), so
//! nothing is copied from it: this crate implements the working grammar of
//! `plans/05-tooling.md` §2, written from the draft at the pin. **Where the
//! draft and that text disagree, the draft wins** and the plan is corrected.
//! The same holds for the JSON shape behind the `serde` feature.
//!
//! `#![no_std]` + `alloc`; never linked into the client wasm.

#![warn(missing_docs)]
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
