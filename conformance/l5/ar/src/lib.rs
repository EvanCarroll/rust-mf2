//! Conformance layer L5, locale `ar`: the suite's messages as a corpus
//! `mf2-build` compiled, and one `tr!` call site per test
//! (`plans/01-conformance.md` §3).
//!
//! Everything here is derived from the vendored suite at build time by
//! `mf2-l5-gen`; `mf2-conformance` drives it and judges the results against
//! the same expectations L4 is judged against.

// The suite's argument names are deliberately not all in NFC — matching a
// decomposed name against the manifest's composed one is what those tests
// test — so the generated call sites carry them as written.
#![allow(clippy::unicode_not_nfc)]

mf2::include_generated!();

include!(concat!(env!("OUT_DIR"), "/shared.rs"));

include!(concat!(env!("OUT_DIR"), "/cases.rs"));
