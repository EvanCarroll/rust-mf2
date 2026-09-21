//! Layers L1, L2 and L3 over the whole vendored suite, and the committed
//! ledger held to their results (plans/08-phase-1-work-order.md A7,
//! plans/09-phase-2-work-order.md A6).

use std::fs;
use std::path::{Path, PathBuf};

use mf2_conformance::ledger::Cell;
use mf2_conformance::{
    Column, Harness, LEDGER_PATH, Ledger, Phase, SUITE_DIR, Suite, Violation, check, verify,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

#[test]
fn l1_l2_and_l3_pass_on_every_applicable_test() {
    let suite = Suite::load(&root().join(SUITE_DIR)).expect("suite");
    let harness = Harness::load(&root()).expect("harness");
    let results = harness.run_all(&suite);
    let failures: Vec<String> = results
        .failures()
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
    assert_eq!(results.tally(Column::L1), (462, 462));
    assert_eq!(results.tally(Column::L2), (326, 326));
    assert_eq!(results.tally(Column::L3), (301, 301));
}

#[test]
fn committed_ledger_matches_the_harness() {
    let suite = Suite::load(&root().join(SUITE_DIR)).expect("suite");
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
    let suite = Suite::load(&root().join(SUITE_DIR)).expect("suite");
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
    let suite = Suite::load(&root().join(SUITE_DIR)).expect("suite");
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
    assert_eq!(with + without, 301);
    assert!(with > 0 && without > 0, "{with} with, {without} without");
}
