//! The ledger ↔ suite relation (plans/01-conformance.md §4) on the committed
//! ledger, and mutation tests showing that each harness rule turns it red.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use mf2_conformance::ledger::{Cell, Note};
use mf2_conformance::{
    Column, EXTRA_DIR, LEDGER_PATH, Ledger, Phase, SUITE_DIR, Suite, TestKey, TestKind, Violation,
    check,
};
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

fn suite() -> Suite {
    mf2_conformance::load_suite(&root()).expect("the suite loads")
}

fn committed_ledger() -> Ledger {
    let text =
        fs::read_to_string(root().join(LEDGER_PATH)).expect("conformance/ledger.toml exists");
    Ledger::parse(&text).expect("committed ledger parses")
}

/// The vendored suite as parsed JSON files, for building mutated suites.
/// The suite's files as `(relative path, JSON)`: the vendored ones and
/// ours under `extra/` (what `load_suite` loads).
fn suite_files() -> Vec<(String, Value)> {
    let mut out = files_below(&root().join(SUITE_DIR), "");
    out.extend(files_below(&root().join(EXTRA_DIR), "extra"));
    out
}

fn files_below(dir: &Path, prefix: &str) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), prefix.to_owned())];
    while let Some((path, rel)) = stack.pop() {
        for entry in fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if entry.path().is_dir() {
                stack.push((entry.path(), rel));
            } else if Path::new(&rel).extension().is_some_and(|e| e == "json") {
                let text = fs::read_to_string(entry.path()).unwrap();
                out.push((rel, serde_json::from_str(&text).unwrap()));
            }
        }
    }
    out
}

#[test]
fn committed_ledger_is_a_bijection_with_the_suite() {
    let suite = suite();
    let ledger = committed_ledger();
    let suite_keys: BTreeSet<&TestKey> = suite.tests().iter().map(|t| &t.key).collect();
    let ledger_keys: BTreeSet<&TestKey> = ledger.entries.iter().map(|e| &e.key).collect();
    assert_eq!(
        suite_keys.len(),
        suite.tests().len(),
        "suite keys are unique"
    );
    assert_eq!(
        ledger_keys.len(),
        ledger.entries.len(),
        "ledger keys are unique"
    );
    assert_eq!(
        suite_keys, ledger_keys,
        "ledger <-> suite is a bijection on the key"
    );
    let violations = check(&suite, &ledger);
    assert!(violations.is_empty(), "harness is red:\n{violations:#?}");
}

#[test]
fn suite_with_extra_has_485_tests_in_17_files() {
    // The vendored 462 and conformance/extra/functions/unit.json's 23.
    let suite = suite();
    assert_eq!(suite.files().len(), 17);
    assert_eq!(suite.tests().len(), 485);
    assert_eq!(
        suite.files().last().map(|(f, n)| (f.as_str(), *n)),
        Some(("u-options.json", 10))
    );
    assert!(
        suite
            .files()
            .iter()
            .any(|(f, n)| f == "extra/functions/unit.json" && *n == 23)
    );
}

#[test]
fn suite_at_the_pin_has_462_tests_in_16_files() {
    let suite = Suite::load(&root().join(SUITE_DIR)).expect("the vendored suite loads");
    assert_eq!(suite.files().len(), 16);
    assert_eq!(suite.tests().len(), 462);
    let kinds = |k: TestKind| suite.tests().iter().filter(|t| t.kind == k).count();
    // 133 in syntax-errors.json + 3 in bidi.json; 22 in data-model-errors.json + 3 in string.json.
    assert_eq!(kinds(TestKind::SyntaxError), 136);
    assert_eq!(kinds(TestKind::DataModelError), 25);
}

#[test]
fn defaults_are_applied_and_overridden() {
    let suite = suite();
    let find = |file: &str, index: usize| {
        suite
            .tests()
            .iter()
            .find(|t| t.key.file == file && t.index == index)
            .unwrap()
    };
    // syntax-errors.json: expErrors comes only from defaultTestProperties.
    let t = find("syntax-errors.json", 0);
    assert_eq!(t.exp_errors, ["syntax-error"]);
    assert_eq!(t.locale, "en-US");
    assert_eq!(t.bidi_isolation, None);
    // pattern-selection.json defaults to locale "und".
    assert_eq!(find("pattern-selection.json", 0).locale, "und");
    // A test's own locale overrides the default.
    assert!(suite.tests().iter().any(|t| t.locale == "ar"));
    // currency.json's default expErrors: [] and tags [":currency"].
    let t = find("functions/currency.json", 0);
    assert_eq!(t.tags, [":currency"]);
}

#[test]
fn byte_identical_tests_are_told_apart_by_nth() {
    let suite = suite();
    let syntax: Vec<_> = suite
        .tests()
        .iter()
        .filter(|t| t.key.file == "syntax.json")
        .collect();
    for (a, b) in [(78, 79), (95, 98)] {
        assert_eq!(
            syntax[a].key.hash, syntax[b].key.hash,
            "#{a} and #{b} hash alike"
        );
        assert_eq!((syntax[a].key.nth, syntax[b].key.nth), (0, 1));
    }
    let duplicated = suite.tests().iter().filter(|t| t.key.nth > 0).count();
    assert_eq!(duplicated, 2, "only those two pairs collide at the pin");
}

#[test]
fn init_is_green_and_round_trips() {
    let suite = suite();
    let ledger = Ledger::init(&suite);
    assert_eq!(ledger.current_phase, Phase::P0);
    assert!(check(&suite, &ledger).is_empty());
    let reparsed = Ledger::parse(&ledger.to_toml()).unwrap();
    assert_eq!(reparsed, ledger);
    // Every applicable cell is xfail until the layer's phase; the rest n/a.
    for (e, t) in ledger.entries.iter().zip(suite.tests()) {
        for c in Column::ALL {
            let expected = if t.kind.applies(c) {
                Cell::Xfail {
                    until: c.deadline(&t.key),
                    reason: None,
                    via: None,
                }
            } else {
                Cell::NotApplicable
            };
            assert_eq!(e.cells[&c], expected, "{} {c}", t.key);
        }
    }
}

// ---- mutations: each must turn the harness red ----

#[test]
fn deleting_a_ledger_entry_is_red() {
    let suite = suite();
    let mut ledger = committed_ledger();
    let removed = ledger.entries.remove(200);
    let v = check(&suite, &ledger);
    assert_eq!(
        v,
        [Violation::MissingEntry {
            key: removed.key,
            index: removed.index
        }]
    );
}

#[test]
fn deleting_an_entry_from_the_ledger_text_is_red() {
    // The same, on the TOML text: drop the last [[test]] block.
    let text = fs::read_to_string(root().join(LEDGER_PATH)).unwrap();
    let cut = text.rfind("\n[[test]]").unwrap();
    let ledger = Ledger::parse(&text[..cut]).unwrap();
    let v = check(&suite(), &ledger);
    assert!(
        matches!(v.as_slice(), [Violation::MissingEntry { .. }]),
        "{v:?}"
    );
}

#[test]
fn duplicating_a_suite_test_is_red() {
    let mut files = suite_files();
    let (_, syntax) = files.iter_mut().find(|(p, _)| p == "syntax.json").unwrap();
    let tests = syntax["tests"].as_array_mut().unwrap();
    let copy = tests[5].clone();
    tests.push(copy);
    let mutated = Suite::from_values(files).unwrap();
    assert_eq!(mutated.tests().len(), 486);
    let v = check(&mutated, &committed_ledger());
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(matches!(&v[0], Violation::MissingEntry { key, index: 114 } if key.nth == 1));
}

#[test]
fn changing_a_suite_input_is_red() {
    let mut files = suite_files();
    let (_, bidi) = files.iter_mut().find(|(p, _)| p == "bidi.json").unwrap();
    bidi["tests"][0]["locale"] = Value::from("fr");
    let v = check(&Suite::from_values(files).unwrap(), &committed_ledger());
    assert!(
        v.iter()
            .any(|x| matches!(x, Violation::MissingEntry { .. }))
    );
    assert!(v.iter().any(|x| matches!(x, Violation::OrphanEntry { .. })));
}

#[test]
fn orphan_and_duplicate_entries_are_red() {
    let suite = suite();
    let mut ledger = committed_ledger();
    let mut orphan = ledger.entries[0].clone();
    orphan.key.hash = "00000000".to_owned();
    ledger.entries.push(orphan);
    ledger.entries.push(ledger.entries[1].clone());
    let v = check(&suite, &ledger);
    assert!(v.iter().any(|x| matches!(x, Violation::OrphanEntry { .. })));
    assert!(
        v.iter()
            .any(|x| matches!(x, Violation::DuplicateEntry { .. }))
    );
}

#[test]
fn until_at_or_before_current_phase_is_red() {
    let suite = suite();
    let mut ledger = committed_ledger();
    // Every layer has a harness and the committed ledger has no `xfail` left
    // (L6 and L6d were the last, and Phase 6 turned them green), so the
    // ledger is clean at every phase.
    for phase in [
        Phase::P1,
        Phase::P2,
        Phase::P3,
        Phase::P4,
        Phase::P5a,
        Phase::P5b,
        Phase::P6,
    ] {
        ledger.current_phase = phase;
        assert_eq!(check(&suite, &ledger), [], "at {phase}");
    }
    // The rule itself still has to bite: an `xfail` whose deadline has
    // arrived is a violation, not a note.
    ledger.entries[0].cells.insert(
        Column::L6,
        Cell::Xfail {
            until: Phase::P6,
            reason: None,
            via: None,
        },
    );
    ledger.current_phase = Phase::P6;
    let v = check(&suite, &ledger);
    assert!(
        v.iter().any(|x| matches!(
            x,
            Violation::UntilNotInFuture {
                column: Column::L6,
                until: Phase::P6,
                ..
            }
        )),
        "{v:?}"
    );
}

#[test]
fn an_overdue_open_note_is_red() {
    let suite = suite();
    let mut ledger = committed_ledger();
    let i = ledger
        .notes
        .iter()
        .position(|n| n.id == "stripped-formats-identically")
        .expect("the stripping note is in the ledger");
    // Closed at P3: L4 formats every test from both catalogs.
    let note = &ledger.notes[i];
    assert_eq!((note.status.as_str(), note.until), ("pass", None));
    assert!(check(&suite, &ledger).is_empty());
    // Reopened with a phase that has come: red.
    let current = ledger.current_phase;
    let note = &mut ledger.notes[i];
    note.status = "open".to_owned();
    note.until = Some(current);
    assert!(matches!(
        check(&suite, &ledger).as_slice(),
        [Violation::NoteOverdue { .. }]
    ));
    // Open until a later phase: an obligation, green.
    ledger.notes[i].until = Some(Phase::P9);
    assert!(check(&suite, &ledger).is_empty());
}

#[test]
fn until_after_the_layer_deadline_is_red() {
    let suite = suite();
    let mut ledger = committed_ledger();
    // P9: after L1's deadline, and after any current phase (so not overdue).
    ledger.entries[0].cells.insert(
        Column::L1,
        Cell::Xfail {
            until: Phase::P9,
            reason: None,
            via: None,
        },
    );
    let v = check(&suite, &ledger);
    assert!(matches!(
        v.as_slice(),
        [Violation::UntilAfterDeadline {
            column: Column::L1,
            deadline: Phase::P1,
            ..
        }]
    ));
}

#[test]
fn skip_by_tag_is_red_and_skip_by_fact_is_not() {
    let suite = suite();
    let i = suite
        .tests()
        .iter()
        .position(|t| t.tags.iter().any(|g| g == ":currency"))
        .unwrap();
    let mut ledger = committed_ledger();
    ledger.entries[i].cells.insert(
        Column::L4,
        Cell::Skip {
            reason: "optional :currency feature".to_owned(),
        },
    );
    let v = check(&suite, &ledger);
    assert!(
        matches!(v.as_slice(), [Violation::SkipByTag { .. }]),
        "{v:?}"
    );

    ledger.entries[i].cells.insert(
        Column::L4,
        Cell::Skip {
            reason: "expects output the spec leaves implementation-defined".to_owned(),
        },
    );
    assert!(check(&suite, &ledger).is_empty());
}

#[test]
fn na_matrix_is_enforced_both_ways() {
    let suite = suite();
    let syn = suite
        .tests()
        .iter()
        .position(|t| t.kind == TestKind::SyntaxError)
        .unwrap();
    let other = suite
        .tests()
        .iter()
        .position(|t| t.kind == TestKind::Other)
        .unwrap();
    let mut ledger = committed_ledger();
    ledger.entries[syn].cells.insert(
        Column::L2,
        Cell::Xfail {
            until: Phase::P1,
            reason: None,
            via: None,
        },
    );
    ledger.entries[other]
        .cells
        .insert(Column::L4, Cell::NotApplicable);
    let v = check(&suite, &ledger);
    assert_eq!(v.len(), 2, "{v:?}");
    assert!(v.iter().any(|x| matches!(
        x,
        Violation::MustBeNotApplicable {
            column: Column::L2,
            ..
        }
    )));
    assert!(v.iter().any(|x| matches!(
        x,
        Violation::NotApplicableMisused {
            column: Column::L4,
            ..
        }
    )));
}

#[test]
fn missing_column_is_red_and_unknown_column_is_rejected() {
    let suite = suite();
    let mut ledger = committed_ledger();
    ledger.entries[3].cells.remove(&Column::L6d);
    assert!(matches!(
        check(&suite, &ledger).as_slice(),
        [Violation::MissingColumn {
            column: Column::L6d,
            ..
        }]
    ));
    let text = fs::read_to_string(root().join(LEDGER_PATH)).unwrap();
    let text = text.replacen("L6d = ", "L7 = ", 1);
    assert!(Ledger::parse(&text).is_err());
}

#[test]
fn every_column_is_verifiable() {
    // The rule this replaces a red case for: a `pass` in a column nothing
    // checks is a silent skip, so the ledger refuses it. Since Phase 6 there
    // is no such column — L6 and L6d were the last two without a harness —
    // and *that* is what has to stay true. A tenth column added without a
    // harness would fail here before it could claim anything.
    for column in Column::ALL {
        assert!(
            mf2_conformance::matrix::HARNESSED.contains(&column),
            "{column} has no harness, so nothing can verify a claim in it"
        );
    }
    let suite = suite();
    let ledger = committed_ledger();
    assert_eq!(
        check(&suite, &ledger)
            .iter()
            .filter(|x| matches!(x, Violation::UnverifiedClaim { .. }))
            .count(),
        0
    );
}

#[test]
fn missing_surrogates_note_is_red() {
    let suite = suite();
    let mut ledger = committed_ledger();
    ledger
        .notes
        .retain(|n: &Note| n.id != "unpaired-surrogates");
    assert!(matches!(
        check(&suite, &ledger).as_slice(),
        [Violation::MissingNote(_)]
    ));
}
