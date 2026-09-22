//! Conformance harness for mf2-two (`plans/01-conformance.md`).
//!
//! Loading the vendored WG suite with `defaultTestProperties` applied, the
//! ledger key (`file`, `hash`, `nth` — see [`key`]), the ledger
//! (`conformance/ledger.toml`) with its generator and checker, the report, and
//! the per-layer harnesses: [`l1`] (syntax) and [`l2`] (data model) since
//! Phase 1, [`l3`] (the binary catalog) since Phase 2, run by [`harness`],
//! which holds the ledger to their results. The other layers are added by the
//! phases that build them.

#![forbid(unsafe_code)]

pub mod abnf;
pub mod check;
mod error;
pub mod goldens;
pub mod harness;
pub mod key;
pub mod l1;
pub mod l2;
pub mod l3;
pub mod l4;
pub mod l4gen;
pub mod ledger;
pub mod matrix;
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
/// The ledger, relative to the repository root.
pub const LEDGER_PATH: &str = "conformance/ledger.toml";
/// The report, relative to the repository root.
pub const REPORT_PATH: &str = "conformance/REPORT.md";
