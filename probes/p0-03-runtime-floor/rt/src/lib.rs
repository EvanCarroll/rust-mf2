//! P0.3 — the runtime floor: a `no_std`, fmt-free, panic-free catalog reader
//! and MF2 evaluator skeleton over P0.7's encoding (NUL-terminated pool
//! strings, byte-plane INDEX). Covers: `Catalog::new` validation (F2/F4/F6),
//! O(1) `get`, the simple fast path, patterns with positional arguments,
//! `.input`/`.local`, `:string` / `:integer` / `:number` (integers, neutral
//! digits), selection with exact + plural/ordinal keys (P0.4's evaluator) and
//! the spec's BetterThan order, markup, parts output, fallback output using
//! NAMES, and the Default Bidi Strategy. Throwaway probe code.
#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unreachable, clippy::todo)]

extern crate alloc;

mod catalog;
mod error;
mod format;
mod num;

pub use catalog::{Catalog, Dir, Entry, MsgId};
pub use error::{CatalogError, FormatError};
pub use format::{
    Arg, BidiStrategy, ErrorSink, Formatter, Isolate, MarkupKind, MarkupOptions, NoErrors, OptValue, Part, PartSink, Sink, Source, ValueDir,
    ValueKind,
};
