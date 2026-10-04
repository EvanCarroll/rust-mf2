//! Layers L1–L4 over the whole vendored suite, and the committed ledger
//! held to their results.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use mf2_conformance::ledger::Cell;
use mf2_conformance::{
    Column, Harness, LEDGER_PATH, Ledger, Phase, TestKey, Violation, check, verify,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

#[test]
fn every_harnessed_layer_passes_but_the_ledgers_xfails() {
    let suite = mf2_conformance::load_suite(&root()).expect("suite");
    let text = fs::read_to_string(root().join(LEDGER_PATH)).expect("ledger");
    let ledger = Ledger::parse(&text).expect("ledger parses");
    let xfail: BTreeSet<(TestKey, Column)> = ledger
        .entries
        .iter()
        .flat_map(|e| {
            e.cells
                .iter()
                .filter(|(_, cell)| matches!(cell, Cell::Xfail { .. }))
                .map(|(c, _)| (e.key.clone(), *c))
        })
        .collect();
    let harness = Harness::load(&root()).expect("harness");
    let results = harness.run_all(&suite);
    let failures: Vec<String> = results
        .failures()
        .filter(|(k, c, _)| !xfail.contains(&((*k).clone(), *c)))
        .map(|(k, c, e)| {
            let src = suite
                .tests()
                .iter()
                .find(|t| &t.key == k)
                .map_or("", |t| t.src.as_str());
            format!("{k} {c}: {e}\n    src {src:?}")
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
    // The vendored suite's 462 tests and the 150 of conformance/extra/ (23
    // in functions/unit.json, Phase 4 A4; 127 from the spec coverage matrix,
    // Phase 7 A12) run through every layer.
    assert_eq!(results.tally(Column::L1), (612, 612));
    assert_eq!(results.tally(Column::L2), (470, 470));
    assert_eq!(results.tally(Column::L3), (444, 444));
    // L4: every test — the date/time files since A5, functions/percent.json
    // and syntax.json #90 since A3, functions/currency.json and
    // extra/functions/unit.json since A4.
    assert_eq!(results.tally(Column::L4), (612, 612));
    // L4d (default features, A7): the 68 tests of the gated functions'
    // files degrade to Unknown Function, syntax.json #90 to neutral digits;
    // and 50 of the matrix's numeric-option tests (A12) degrade the same two
    // ways.
    assert_eq!(results.tally(Column::L4d), (493, 612));
    assert_eq!(results.degradations(Column::L4d), 119);
    // L5 (the macro layer, Phase 5b A4): the same 612, through `tr!` against
    // a corpus `mf2-build` compiled — the tests whose message the spec
    // refuses assert that the *build* refused it, with the same kinds.
    assert_eq!(results.tally(Column::L5), (612, 612));
    // L5d: the same degradations as L4d, except that a gated function is the
    // build refusing the corpus rather than a run-time Unknown Function.
    assert_eq!(results.tally(Column::L5d), (493, 612));
    assert_eq!(results.degradations(Column::L5d), 119);
}

#[test]
fn committed_ledger_matches_the_harness() {
    let suite = mf2_conformance::load_suite(&root()).expect("suite");
    let text = fs::read_to_string(root().join(LEDGER_PATH)).expect("ledger");
    let ledger = Ledger::parse(&text).expect("ledger parses");
    let harness = Harness::load(&root()).expect("harness");
    let results = harness.run_all(&suite);
    let mut violations = check(&suite, &ledger);
    violations.extend(verify(&ledger, &results));
    assert!(
        violations.is_empty(),
        "{} violation(s):\n{}",
        violations.len(),
        violations
            .iter()
            .take(20)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_ledger_is_held_to_l3_both_ways() {
    let suite = mf2_conformance::load_suite(&root()).expect("suite");
    let text = fs::read_to_string(root().join(LEDGER_PATH)).expect("ledger");
    let mut ledger = Ledger::parse(&text).expect("ledger parses");
    let mut results = Harness::load(&root()).expect("harness").run_all(&suite);
    let i = ledger
        .entries
        .iter()
        .position(|e| e.cells[&Column::L3] == Cell::Pass { via: None })
        .expect("an L3 pass");
    let key = ledger.entries[i].key.clone();

    // A regression: the L3 cell that says pass fails.
    let passed = results
        .cells
        .insert((key.clone(), Column::L3), Err("injected".to_owned()));
    assert_eq!(passed, Some(Ok(())));
    assert!(matches!(
        verify(&ledger, &results).as_slice(),
        [Violation::PassFails {
            column: Column::L3,
            ..
        }]
    ));
    results.cells.insert((key, Column::L3), Ok(()));

    // The ratchet: an L3 xfail that passes must be tightened.
    ledger.entries[i].cells.insert(
        Column::L3,
        Cell::Xfail {
            until: Phase::P2,
            reason: None,
            via: None,
        },
    );
    assert!(matches!(
        verify(&ledger, &results).as_slice(),
        [Violation::UnexpectedPass {
            column: Column::L3,
            ..
        }]
    ));
}

#[test]
fn l3_exercises_stripping_both_ways_on_the_suite() {
    // Messages with COLD data (the stripped catalog drops something and says
    // so) and without (the stripped catalog decodes to the model itself).
    let suite = mf2_conformance::load_suite(&root()).expect("suite");
    let (mut with, mut without) = (0, 0);
    for t in suite.tests().iter().filter(|t| t.kind.applies(Column::L3)) {
        let model = mf2_syntax::parse_model(&t.src)
            .message
            .expect("an L3-applicable test parses");
        if mf2_conformance::l3::has_cold_data(&model) {
            with += 1;
        } else {
            without += 1;
        }
    }
    eprintln!("L3 on the suite: {with} messages with COLD data, {without} without");
    assert_eq!(with + without, 444); // 301 of the vendored suite, 143 of extra/
    assert!(with > 0 && without > 0, "{with} with, {without} without");
}
