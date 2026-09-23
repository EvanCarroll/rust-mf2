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
use crate::l4::DefaultOutcome;
use crate::ledger::{Cell, DegradedKind, Ledger, Via};
use crate::matrix::{Column, HARNESSED};
use crate::spec::{DATA_MODEL_SCHEMA, spec_path};
use crate::suite::{Suite, SuiteTest};
use crate::{l1, l2, l3, l4, l5};

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
    /// The default-features cells (L4d) that degrade in a documented way:
    /// the kind and a description. Their `cells` outcome is an `Err`.
    pub degraded: BTreeMap<(TestKey, Column), (DegradedKind, String)>,
    /// How a macro-layer cell was driven, where it was not the macro: the
    /// suite's tests whose `params` deliberately do not name the message's
    /// variables go through the dynamic named-argument API, and the ledger
    /// says so (`via = "dyn"`).
    // A map rather than a set, because it mirrors the ledger's own model:
    // `via` is an enum there, and a later layer may drive a cell another way.
    #[allow(clippy::zero_sized_map_values)]
    pub via: BTreeMap<(TestKey, Column), Via>,
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

    /// The number of documented degradations in `column`.
    pub fn degradations(&self, column: Column) -> usize {
        self.degraded.keys().filter(|(_, c)| *c == column).count()
    }

    /// The failing cells, in key order — a documented degradation is not a
    /// failure.
    pub fn failures(&self) -> impl Iterator<Item = (&TestKey, Column, &str)> {
        self.cells.iter().filter_map(|((k, c), o)| {
            if self.degraded.contains_key(&(k.clone(), *c)) {
                return None;
            }
            o.as_ref().err().map(|e| (k, *c, e.as_str()))
        })
    }
}

/// What a default-features column says about a test: L4d runs the one-message
/// catalog with the default registry, L5d the corpus the build accepted —
/// and the messages it refused, which is that layer's whole point.
fn default_outcome(column: Column, test: &SuiteTest) -> DefaultOutcome {
    match column {
        Column::L5d => l5::check_default(test),
        _ => l4::check_default(test),
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
    ///
    pub fn run(&self, column: Column, test: &SuiteTest) -> Option<Outcome> {
        let run = || match column {
            Column::L1 => Some(l1::check(test)),
            Column::L2 => Some(l2::check(test, &self.schema)),
            Column::L3 => Some(l3::check(test)),
            Column::L4 => Some(l4::check(test)),
            Column::L5 => Some(l5::check(test)),
            Column::L4d | Column::L5d => Some(match default_outcome(column, test) {
                DefaultOutcome::Pass => Ok(()),
                DefaultOutcome::Degraded(kind, detail) => {
                    Err(format!("degraded: {}: {detail}", kind.as_str()))
                }
                DefaultOutcome::Fail(e) => Err(e),
            }),
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
                if column.is_macro_layer() && l5::is_dyn(test) {
                    results.via.insert((test.key.clone(), column), Via::Dyn);
                }
                if column.is_default_features() {
                    let outcome = catch_unwind(AssertUnwindSafe(|| default_outcome(column, test)))
                        .unwrap_or_else(|_| DefaultOutcome::Fail(format!("{column} panicked")));
                    let key = (test.key.clone(), column);
                    let cell = match outcome {
                        DefaultOutcome::Pass => Ok(()),
                        DefaultOutcome::Degraded(kind, detail) => {
                            let text = format!("degraded: {}: {detail}", kind.as_str());
                            results.degraded.insert(key.clone(), (kind, detail));
                            Err(text)
                        }
                        DefaultOutcome::Fail(e) => Err(e),
                    };
                    results.cells.insert(key, cell);
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
/// `skip`, is a violation; so is a `degraded` cell whose run does not
/// degrade in exactly that way, and an `xfail` / `skip` whose run does.
pub fn verify(ledger: &Ledger, results: &Results) -> Vec<Violation> {
    let mut v = Vec::new();
    for entry in &ledger.entries {
        for (&column, cell) in &entry.cells {
            let at = (entry.key.clone(), column);
            let Some(outcome) = results.cells.get(&at) else {
                continue;
            };
            // `via` is how the harness drove it, not a claim the ledger may
            // make on its own: a test moves between the macro and the
            // dynamic path when its message's variables change.
            if column.is_macro_layer() && matches!(cell, Cell::Pass { .. }) {
                let want = results.via.get(&at).copied();
                if cell.via() != want {
                    let name = |v: Option<Via>| match v {
                        Some(Via::Dyn) => "via = \"dyn\"",
                        None => "through the macro",
                    };
                    v.push(Violation::ViaMismatch {
                        key: entry.key.clone(),
                        column,
                        want: name(cell.via()),
                        got: name(want),
                    });
                }
            }
            let degraded = results.degraded.get(&at);
            if let Cell::Degraded { kind, .. } = cell {
                let got = match (degraded, outcome) {
                    (Some((k, _)), _) if k == kind => continue,
                    (Some((k, d)), _) => format!("degrades as {} ({d})", k.as_str()),
                    (None, Ok(())) => "passes".to_owned(),
                    (None, Err(e)) => format!("fails: {e}"),
                };
                v.push(Violation::DegradationMismatch {
                    key: entry.key.clone(),
                    column,
                    want: kind.as_str(),
                    got,
                });
                continue;
            }
            if let (Cell::Xfail { .. } | Cell::Skip { .. }, Some((kind, detail))) = (cell, degraded)
            {
                v.push(Violation::UnrecordedDegradation {
                    key: entry.key.clone(),
                    column,
                    status: cell.status(),
                    kind: kind.as_str(),
                    detail: detail.clone(),
                });
                continue;
            }
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
/// becomes `pass`, and every `xfail` cell whose run degrades in a documented
/// way becomes `degraded` with that kind. Returns how many cells changed.
/// (`skip` cells are left alone: their reason is a fact about the test, to
/// be reviewed by hand.)
pub fn promote(ledger: &mut Ledger, results: &Results) -> usize {
    let mut changed = 0;
    for entry in &mut ledger.entries {
        for (&column, cell) in &mut entry.cells {
            let at = (entry.key.clone(), column);
            let passes = matches!(results.cells.get(&at), Some(Ok(())));
            if let Cell::Xfail { via, .. } = cell {
                // How the harness drove it wins over what the cell said: a
                // test moves between the two paths when its message changes.
                let via = results.via.get(&at).copied().or(*via);
                if passes {
                    *cell = Cell::Pass { via };
                    changed += 1;
                } else if let Some((kind, detail)) = results.degraded.get(&at) {
                    *cell = Cell::Degraded {
                        kind: *kind,
                        detail: Some(detail.clone()),
                    };
                    changed += 1;
                }
            }
        }
    }
    changed
}
