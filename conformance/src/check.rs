//! The harness rules of plans/01-conformance.md §4 that apply to a ledger and
//! the suite before any layer exists, plus the well-formedness rules that
//! follow from §3–§4:
//!
//! 1. ledger ↔ suite is a bijection on the key (rule 1);
//! 2. every entry has exactly the nine columns, `n/a` exactly where the matrix
//!    says (a stray `n/a` would be a silent skip);
//! 3. no `xfail` whose `until` is `current_phase` or earlier (rule 4), and none
//!    later than the phase at which its layer must be green (§3);
//! 4. no `skip` by tag (rule 5);
//! 5. `degraded` only in L4d/L5d/L6d, `via` only in L5/L5d;
//! 6. `pass`/`degraded` only in columns whose harness exists
//!    ([`crate::matrix::HARNESSED`]) — rules 2 and 3 run there;
//! 7. the unpaired-surrogates note is present.
//!
//! Rules 2 and 3 themselves — a `pass` must pass, an `xfail` must not — need
//! the layers to run: [`crate::harness::verify`].

use std::collections::{HashMap, HashSet};

use crate::key::TestKey;
use crate::ledger::{Cell, Ledger, SURROGATES_NOTE_ID};
use crate::matrix::{Column, HARNESSED, Phase};
use crate::suite::{Suite, SuiteTest};

/// The tags the schema defines. None of them may justify a skip.
pub const KNOWN_TAGS: &[&str] = &[":currency", ":percent", "u:dir", "u:id"];

/// A broken harness rule. Any violation turns the harness red.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Violation {
    #[error("{key} (#{index}): suite test has no ledger entry")]
    MissingEntry { key: TestKey, index: usize },

    #[error("{key} (index hint {index}): ledger entry has no suite test")]
    OrphanEntry { key: TestKey, index: usize },

    #[error("{key}: more than one ledger entry")]
    DuplicateEntry { key: TestKey },

    #[error("{key}: column {column} is missing")]
    MissingColumn { key: TestKey, column: Column },

    #[error("{key}: {column} must be \"n/a\" for a {kind} test")]
    MustBeNotApplicable {
        key: TestKey,
        column: Column,
        kind: &'static str,
    },

    #[error("{key}: {column} is \"n/a\", but that layer applies to a {kind} test")]
    NotApplicableMisused {
        key: TestKey,
        column: Column,
        kind: &'static str,
    },

    #[error("{key}: {column} is xfail until {until}, which is not after current_phase {current}")]
    UntilNotInFuture {
        key: TestKey,
        column: Column,
        until: Phase,
        current: Phase,
    },

    #[error(
        "{key}: {column} is xfail until {until}, but {column} must be green by the exit of {deadline}"
    )]
    UntilAfterDeadline {
        key: TestKey,
        column: Column,
        until: Phase,
        deadline: Phase,
    },

    #[error("{key}: {column} is skipped by tag ({tag}); tags are not a skip reason")]
    SkipByTag {
        key: TestKey,
        column: Column,
        tag: String,
    },

    #[error("{key}: {column} is degraded; degraded is only valid in L4d/L5d/L6d")]
    DegradedOutsideDefaultColumns { key: TestKey, column: Column },

    #[error("{key}: {column} has via = \"dyn\"; via is only valid in L5/L5d")]
    ViaOutsideMacroLayer { key: TestKey, column: Column },

    #[error("{key}: {column} claims {status}, but no {column} harness exists yet to verify it")]
    UnverifiedClaim {
        key: TestKey,
        column: Column,
        status: &'static str,
    },

    #[error("ledger lacks the [[note]] with id = \"{0}\"")]
    MissingNote(&'static str),

    #[error("{key}: {column} is \"pass\" but the harness fails: {detail}")]
    PassFails {
        key: TestKey,
        column: Column,
        detail: String,
    },

    #[error(
        "{key}: {column} passes but the ledger says {status}; tighten the ledger (the pass count only goes up)"
    )]
    UnexpectedPass {
        key: TestKey,
        column: Column,
        status: &'static str,
    },
}

/// Checks `ledger` against `suite`. Empty ⇒ green.
pub fn check(suite: &Suite, ledger: &Ledger) -> Vec<Violation> {
    let mut v = Vec::new();

    let tests: HashMap<&TestKey, &SuiteTest> = suite.tests().iter().map(|t| (&t.key, t)).collect();
    let mut seen: HashSet<&TestKey> = HashSet::new();
    for e in &ledger.entries {
        if !seen.insert(&e.key) {
            v.push(Violation::DuplicateEntry { key: e.key.clone() });
            continue;
        }
        match tests.get(&e.key) {
            None => v.push(Violation::OrphanEntry {
                key: e.key.clone(),
                index: e.index,
            }),
            Some(t) => check_entry(t, &e.cells, ledger.current_phase, &mut v),
        }
    }
    for t in suite.tests() {
        if !seen.contains(&t.key) {
            v.push(Violation::MissingEntry {
                key: t.key.clone(),
                index: t.index,
            });
        }
    }
    if !ledger.notes.iter().any(|n| n.id == SURROGATES_NOTE_ID) {
        v.push(Violation::MissingNote(SURROGATES_NOTE_ID));
    }
    v
}

fn check_entry(
    t: &SuiteTest,
    cells: &std::collections::BTreeMap<Column, Cell>,
    current: Phase,
    v: &mut Vec<Violation>,
) {
    let key = || t.key.clone();
    for column in Column::ALL {
        let Some(cell) = cells.get(&column) else {
            v.push(Violation::MissingColumn { key: key(), column });
            continue;
        };
        let applies = t.kind.applies(column);
        match (applies, cell) {
            (false, Cell::NotApplicable) => continue,
            (false, _) => {
                v.push(Violation::MustBeNotApplicable {
                    key: key(),
                    column,
                    kind: t.kind.as_str(),
                });
                continue;
            }
            (true, Cell::NotApplicable) => {
                v.push(Violation::NotApplicableMisused {
                    key: key(),
                    column,
                    kind: t.kind.as_str(),
                });
                continue;
            }
            (true, _) => {}
        }
        match cell {
            Cell::Xfail { until, via, .. } => {
                if *until <= current {
                    v.push(Violation::UntilNotInFuture {
                        key: key(),
                        column,
                        until: *until,
                        current,
                    });
                }
                let deadline = column.deadline(&t.key.file);
                if *until > deadline {
                    v.push(Violation::UntilAfterDeadline {
                        key: key(),
                        column,
                        until: *until,
                        deadline,
                    });
                }
                if via.is_some() && !column.is_macro_layer() {
                    v.push(Violation::ViaOutsideMacroLayer { key: key(), column });
                }
            }
            Cell::Pass { via } => {
                if via.is_some() && !column.is_macro_layer() {
                    v.push(Violation::ViaOutsideMacroLayer { key: key(), column });
                }
                if !HARNESSED.contains(&column) {
                    v.push(Violation::UnverifiedClaim {
                        key: key(),
                        column,
                        status: "pass",
                    });
                }
            }
            Cell::Degraded { .. } => {
                if !column.is_default_features() {
                    v.push(Violation::DegradedOutsideDefaultColumns { key: key(), column });
                }
                if !HARNESSED.contains(&column) {
                    v.push(Violation::UnverifiedClaim {
                        key: key(),
                        column,
                        status: "degraded",
                    });
                }
            }
            Cell::Skip { reason } => {
                let lower = reason.to_lowercase();
                let tag = KNOWN_TAGS
                    .iter()
                    .copied()
                    .chain(t.tags.iter().map(String::as_str))
                    .find(|tag| lower.contains(&tag.to_lowercase()));
                if let Some(tag) = tag {
                    v.push(Violation::SkipByTag {
                        key: key(),
                        column,
                        tag: tag.to_owned(),
                    });
                }
            }
            Cell::NotApplicable => {}
        }
    }
}

/// Entries whose `index` hint no longer matches the test's position. Not a
/// violation (the key is what counts), but worth refreshing.
pub fn stale_index_hints<'a>(
    suite: &'a Suite,
    ledger: &'a Ledger,
) -> Vec<(&'a TestKey, usize, usize)> {
    let tests: HashMap<&TestKey, &SuiteTest> = suite.tests().iter().map(|t| (&t.key, t)).collect();
    ledger
        .entries
        .iter()
        .filter_map(|e| {
            let t = tests.get(&e.key)?;
            (t.index != e.index).then_some((&e.key, e.index, t.index))
        })
        .collect()
}
