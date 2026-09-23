//! Conformance layer L5 on generated input (`plans/01-conformance.md` §3;
//! `plans/13-phase-5b-work-order.md` A8).
//!
//! The suite is 485 messages a working group wrote. This crate is however
//! many the ABNF generator produces, steered towards the runtime's functions
//! exactly as layer L4's generated cases are — compiled into a corpus by
//! `mf2-build`, called through `tr!`, and held to what L4's runner makes of
//! the same message.

// Generated code: a generated argument is whatever number the seed chose,
// and a generated name is spelled however the seed spelled it — which is
// what the macro has to fold, and the point of running this at all.
#![allow(clippy::unreadable_literal, clippy::manual_string_new)]

mf2::include_generated!();

include!(concat!(env!("OUT_DIR"), "/shared.rs"));

include!(concat!(env!("OUT_DIR"), "/cases.rs"));
