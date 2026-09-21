//! Layers L1 and L2 over the whole vendored suite, and the committed ledger
//! held to their results (plans/08-phase-1-work-order.md, A7).

use std::fs;
use std::path::{Path, PathBuf};

use mf2_conformance::{Column, Harness, LEDGER_PATH, Ledger, SUITE_DIR, Suite, check, verify};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

#[test]
fn l1_and_l2_pass_on_every_applicable_test() {
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
