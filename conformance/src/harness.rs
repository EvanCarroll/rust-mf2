//! Running the layer harnesses over the suite, and holding the ledger to the
//! results (plans/01-conformance.md §4, harness rules 2 and 3).
//!
//! Every applicable cell of a harnessed column ([`crate::matrix::HARNESSED`])
//! is run. A `pass` that fails is a violation; so is a failing cell's
//! `xfail`/`skip` that unexpectedly passes (the ratchet: the pass count only
//! goes up). A panic inside a layer counts as a failure, not a crash of the
//! harness, so one bad test cannot hide the others.

use std::collections::BTreeMap;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use crate::check::Violation;
use crate::error::{Error, Result};
use crate::key::TestKey;
use crate::ledger::{Cell, Ledger};
use crate::matrix::{Column, HARNESSED};
use crate::spec::{DATA_MODEL_SCHEMA, spec_path};
use crate::suite::{Suite, SuiteTest};
use crate::{l1, l2, l3};

/// What the harnesses need beyond the suite: the data model's JSON Schema.
pub struct Harness {
    schema: jsonschema::Validator,
}

impl std::fmt::Debug for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Harness")
    }
}

/// One run of one layer on one test.
pub type Outcome = std::result::Result<(), String>;

/// The outcome of every harnessed, applicable (test, column) cell.
#[derive(Debug, Default)]
pub struct Results {
    /// Keyed by test and column.
    pub cells: BTreeMap<(TestKey, Column), Outcome>,
}

impl Results {
    /// `(passed, run)` for `column`.
    pub fn tally(&self, column: Column) -> (usize, usize) {
        let mut passed = 0;
        let mut run = 0;
        for ((_, c), o) in &self.cells {
            if *c == column {
                run += 1;
                passed += usize::from(o.is_ok());
            }
        }
        (passed, run)
    }

    /// The failing cells, in key order.
    pub fn failures(&self) -> impl Iterator<Item = (&TestKey, Column, &str)> {
        self.cells
            .iter()
            .filter_map(|((k, c), o)| o.as_ref().err().map(|e| (k, *c, e.as_str())))
    }
}

impl Harness {
    /// Loads what the harnesses need from the repository at `root`.
    pub fn load(root: &Path) -> Result<Self> {
        let path = spec_path(root, DATA_MODEL_SCHEMA);
        let text = fs::read_to_string(&path).map_err(|source| Error::IoAt {
            path: path.clone(),
            source,
        })?;
        let schema = l2::schema(&text).map_err(|message| Error::Spec { path, message })?;
        Ok(Harness { schema })
    }

    /// Runs `column` on `test`; `None` if the column has no harness yet.
    pub fn run(&self, column: Column, test: &SuiteTest) -> Option<Outcome> {
        let run = || match column {
            Column::L1 => Some(l1::check(test)),
            Column::L2 => Some(l2::check(test, &self.schema)),
            Column::L3 => Some(l3::check(test)),
            _ => None,
        };
        match catch_unwind(AssertUnwindSafe(run)) {
            Ok(outcome) => outcome,
            Err(_) => Some(Err(format!("{column} panicked"))),
        }
    }

    /// Runs every harnessed column on every test it applies to.
    pub fn run_all(&self, suite: &Suite) -> Results {
        let mut results = Results::default();
        for test in suite.tests() {
            for &column in HARNESSED {
                if !test.kind.applies(column) {
                    continue;
                }
                if let Some(outcome) = self.run(column, test) {
                    results.cells.insert((test.key.clone(), column), outcome);
                }
            }
        }
        results
    }
}

/// Holds `ledger` to `results`: a failing `pass`, or a passing `xfail` /
/// `skip`, is a violation.
pub fn verify(ledger: &Ledger, results: &Results) -> Vec<Violation> {
    let mut v = Vec::new();
    for entry in &ledger.entries {
        for (&column, cell) in &entry.cells {
            let Some(outcome) = results.cells.get(&(entry.key.clone(), column)) else {
                continue;
            };
            match (cell, outcome) {
                (Cell::Pass { .. }, Err(detail)) => v.push(Violation::PassFails {
                    key: entry.key.clone(),
                    column,
                    detail: detail.clone(),
                }),
                (Cell::Xfail { .. } | Cell::Skip { .. }, Ok(())) => {
                    v.push(Violation::UnexpectedPass {
                        key: entry.key.clone(),
                        column,
                        status: cell.status(),
                    });
                }
                _ => {}
            }
        }
    }
    v
}

/// Tightens `ledger` to `results`: every `xfail` cell whose harness passes
/// becomes `pass`. Returns how many cells changed. (`skip` cells are left
/// alone: their reason is a fact about the test, to be reviewed by hand.)
pub fn promote(ledger: &mut Ledger, results: &Results) -> usize {
    let mut changed = 0;
    for entry in &mut ledger.entries {
        for (&column, cell) in &mut entry.cells {
            let passes = matches!(
                results.cells.get(&(entry.key.clone(), column)),
                Some(Ok(()))
            );
            if passes && let Cell::Xfail { via, .. } = cell {
                *cell = Cell::Pass { via: *via };
                changed += 1;
            }
        }
    }
    changed
}
