//! Conformance layer L5, locale `fr`: the suite's messages as a corpus
//! `mf2-build` compiled, and one `tr!` call site per test
//! (`plans/01-conformance.md` §3).
//!
//! Everything here is derived from the vendored suite at build time by
//! `mf2-l5-gen`; `mf2-conformance` drives it and judges the results against
//! the same expectations L4 is judged against.

// Generated code: the suite's own strings, escaped by the generator (a
// message may carry a bidi control, which rustc refuses in a literal).
#![allow(clippy::unreadable_literal, clippy::manual_string_new)]

mf2::include_generated!();

include!(concat!(env!("OUT_DIR"), "/shared.rs"));

include!(concat!(env!("OUT_DIR"), "/cases.rs"));
