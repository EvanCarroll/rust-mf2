//! The D1 gate's rules (plans/05-tooling.md §1), applied to a finished run.
//!
//! For every contender other than the baseline, on **every** row:
//!
//! * time: median ns/msg ≤ [`TIME_RATIO_MAX`] × the baseline's median;
//! * allocation count per pass ≤ the baseline's, exactly;
//! * allocated bytes per pass ≤ the baseline's, exactly;
//!
//! and once per contender: exact on every suite test (the baseline need not
//! be — ox is at 460/462 — so passing this is "strictly more correct").
//!
//! With the baseline alone the outcome is [`Status::BaselineOnly`].

use serde::Serialize;

use crate::correctness::Correctness;
use crate::measure::Row;

/// The noise allowance on time: ours ≤ 1.05 × ox.
pub const TIME_RATIO_MAX: f64 = 1.05;

/// Samples the gate requires per row (the plan's "median of ≥ 30 runs").
pub const MIN_RUNS: usize = 30;

/// What a check compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Criterion {
    /// Median ns/msg, relative to the baseline.
    Time,
    /// Allocation calls per pass.
    Allocs,
    /// Bytes requested per pass.
    Bytes,
    /// Suite tests exact.
    Correctness,
}

/// One comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    /// Contender key.
    pub parser: &'static str,
    /// Row label (`corpus / stage / mode`), or `suite` for correctness.
    pub row: String,
    /// What is compared.
    pub criterion: Criterion,
    /// The contender's value.
    pub value: f64,
    /// The largest passing value.
    pub limit: f64,
    /// `value ≤ limit`.
    pub passed: bool,
}

/// The overall result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Only the baseline is registered: nothing to gate.
    BaselineOnly,
    /// Every check passed.
    Pass,
    /// At least one check failed.
    Fail,
}

/// The gate's verdict with every check behind it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outcome {
    /// Overall status.
    pub status: Status,
    /// Every comparison made (empty for [`Status::BaselineOnly`]).
    pub checks: Vec<Check>,
}

/// Row label used in checks.
pub fn row_label(row: &Row) -> String {
    format!(
        "{} / {} / {}",
        row.corpus.key(),
        row.stage.label(),
        row.mode.label()
    )
}

/// Applies the rules to `rows` (every row's `results[0]` is the baseline) and
/// `correctness` (one entry per contender, same order).
pub fn evaluate(rows: &[Row], correctness: &[Correctness]) -> Outcome {
    let contenders = rows.first().map_or(0, |r| r.results.len());
    if contenders < 2 {
        return Outcome {
            status: Status::BaselineOnly,
            checks: Vec::new(),
        };
    }
    let mut checks = Vec::new();
    for row in rows {
        let Some((base, others)) = row.results.split_first() else {
            continue;
        };
        let label = row_label(row);
        for ours in others {
            let mut push = |criterion, value: f64, limit: f64| {
                checks.push(Check {
                    parser: ours.parser,
                    row: label.clone(),
                    criterion,
                    value,
                    limit,
                    passed: value <= limit,
                });
            };
            push(
                Criterion::Time,
                ours.ns_per_msg.median,
                TIME_RATIO_MAX * base.ns_per_msg.median,
            );
            push(Criterion::Allocs, ours.allocs as f64, base.allocs as f64);
            push(
                Criterion::Bytes,
                ours.alloc_bytes as f64,
                base.alloc_bytes as f64,
            );
        }
    }
    for c in correctness.iter().skip(1) {
        checks.push(Check {
            parser: c.parser,
            row: "suite".to_owned(),
            criterion: Criterion::Correctness,
            value: c.suite_exact as f64,
            limit: c.suite_total as f64,
            passed: c.is_complete(),
        });
    }
    let status = if checks.iter().all(|c| c.passed) {
        Status::Pass
    } else {
        Status::Fail
    };
    Outcome { status, checks }
}

#[cfg(test)]
mod tests {
    use super::{Criterion, Status, evaluate};
    use crate::adapter::{Mode, Stage};
    use crate::corpus::CorpusId;
    use crate::correctness::Correctness;
    use crate::measure::{Measurement, Row, Spread};

    fn m(parser: &'static str, median: f64, allocs: u64, bytes: u64) -> Measurement {
        Measurement {
            parser,
            api: "",
            ns_per_msg: Spread {
                min: median,
                q1: median,
                median,
                q3: median,
                max: median,
            },
            mb_per_s: 0.0,
            allocs,
            alloc_bytes: bytes,
            allocs_per_msg: 0.0,
            alloc_bytes_per_msg: 0.0,
            diagnostics: 0,
        }
    }

    fn row(results: Vec<Measurement>) -> Row {
        Row {
            corpus: CorpusId::Workload,
            stage: Stage::Model,
            mode: Mode::Fresh,
            messages: 1,
            bytes: 1,
            runs: 31,
            passes_per_sample: 1,
            results,
        }
    }

    fn correct(parser: &'static str, exact: usize) -> Correctness {
        Correctness {
            parser,
            suite_total: 462,
            suite_exact: exact,
            mismatches: Vec::new(),
            workload_total: 0,
            workload_with_errors: 0,
        }
    }

    #[test]
    fn baseline_alone_is_baseline_only() {
        let out = evaluate(
            &[row(vec![m("ox", 500.0, 10, 1000)])],
            &[correct("ox", 460)],
        );
        assert_eq!(out.status, Status::BaselineOnly);
        assert!(out.checks.is_empty());
    }

    #[test]
    fn within_five_percent_and_leaner_passes() {
        let rows = [row(vec![
            m("ox", 500.0, 10, 1000),
            m("ours", 524.0, 10, 1000),
        ])];
        let out = evaluate(&rows, &[correct("ox", 460), correct("ours", 462)]);
        assert_eq!(out.status, Status::Pass, "{:?}", out.checks);
        assert_eq!(out.checks.len(), 4);
    }

    #[test]
    fn any_failed_rule_fails() {
        let base = || m("ox", 500.0, 10, 1000);
        let cases = [
            (m("ours", 526.0, 10, 1000), 462, Criterion::Time),
            (m("ours", 100.0, 11, 1000), 462, Criterion::Allocs),
            (m("ours", 100.0, 0, 1001), 462, Criterion::Bytes),
            (m("ours", 100.0, 0, 0), 461, Criterion::Correctness),
        ];
        for (ours, exact, failing) in cases {
            let out = evaluate(
                &[row(vec![base(), ours])],
                &[correct("ox", 460), correct("ours", exact)],
            );
            assert_eq!(out.status, Status::Fail);
            let failed: Vec<Criterion> = out
                .checks
                .iter()
                .filter(|c| !c.passed)
                .map(|c| c.criterion)
                .collect();
            assert_eq!(failed, vec![failing]);
        }
    }
}
