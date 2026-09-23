//! Conformance L7's `ar` set: the suite's ar tests, with the twin
//! `en-GB` (`conformance/l7-web/set-build`).

// Generated code: the suite's own strings, escaped by the generator.
#![allow(clippy::unreadable_literal, clippy::manual_string_new)]

mf2::include_generated!();

include!(concat!(env!("OUT_DIR"), "/shared.rs"));

include!(concat!(env!("OUT_DIR"), "/cases.rs"));

include!(concat!(env!("OUT_DIR"), "/set.rs"));
