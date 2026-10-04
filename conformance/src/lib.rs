//! Conformance harness for rust-mf2.
//!
//! Loading the vendored WG suite with `defaultTestProperties` applied, the
//! ledger key (`file`, `hash`, `nth` — see [`key`]), the ledger
//! (`conformance/ledger.toml`) with its generator and checker, the report, and
//! the per-layer harnesses: [`l1`] (syntax) and [`l2`] (data model) since
//! Phase 1, [`l3`] (the binary catalog) since Phase 2, run by [`harness`],
//! which holds the ledger to their results. The other layers are added by the
//! phases that build them.

#![forbid(unsafe_code)]

// Layer L6's Leptos line: the 0.8
// crates, when they are the ones on, renamed back.
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate leptos_0_8 as leptos;
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate tachys_0_2 as tachys;

pub mod abnf;
pub mod check;
pub mod coverage;
mod error;
pub mod goldens;
pub mod harness;
pub mod key;
pub mod l1;
pub mod l2;
pub mod l3;
pub mod l4;
pub mod l4gen;
pub mod l5;
pub mod l6;
pub mod ledger;
pub mod matrix;
pub mod parallel;
pub mod report;
pub mod spec;
pub mod suite;

pub use check::{Violation, check};
pub use error::{Error, Result};
pub use harness::{Harness, Results, promote, verify};
pub use key::TestKey;
pub use ledger::{Cell, Ledger};
pub use matrix::{Column, Phase, TestKind};
pub use suite::{Suite, SuiteDiff, SuiteTest, diff};

/// The vendored suite's test directory, relative to the repository root.
pub const SUITE_DIR: &str = "third_party/message-format-wg/test/tests";
/// Our own tests in the WG schema, relative to
/// the repository root; they load as `extra/…`.
pub const EXTRA_DIR: &str = "conformance/extra";

/// The suite every layer runs: the vendored WG suite and [`EXTRA_DIR`].
pub fn load_suite(root: &std::path::Path) -> Result<Suite> {
    Suite::load_with_extra(&root.join(SUITE_DIR), &root.join(EXTRA_DIR))
}
/// The ledger, relative to the repository root.
pub const LEDGER_PATH: &str = "conformance/ledger.toml";
/// The report, relative to the repository root.
pub const REPORT_PATH: &str = "conformance/REPORT.md";
