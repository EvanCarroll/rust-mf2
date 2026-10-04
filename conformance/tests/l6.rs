//! Layer L6's own assertions about itself.
//!
//! The ledger says L6 is green. These say *what* is green: that the markup
//! half is exercised at all, and that a wrong render is actually caught —
//! two ways a comparison can pass without comparing anything.

use mf2_conformance::l6;
use mf2_conformance::matrix::TestKind;
use mf2_conformance::suite::Suite;

fn suite() -> Suite {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository root");
    Suite::load_with_extra(
        &root.join("third_party/message-format-wg/test/tests"),
        &root.join("conformance/extra"),
    )
    .expect("the vendored suite loads")
}

/// The markup comparison runs on real tests. Without this, `judge_markup`
/// could be comparing two empty lists for the whole suite and nothing would
/// say so.
#[test]
fn the_markup_half_is_not_vacuous() {
    let suite = suite();
    let with_markup: Vec<_> = suite
        .tests()
        .iter()
        .filter(|t| t.kind == TestKind::Other && l6::expects_markup(t))
        .collect();
    assert!(
        with_markup.len() >= 4,
        "only {} runtime-valid tests expect markup parts; the suite's markup \
         tests are what L6's flat handler is compared against",
        with_markup.len()
    );
    for test in with_markup {
        l6::check(test).unwrap_or_else(|e| panic!("{}: {e}", test.key));
    }
}

/// A wrong render is caught. Without this, a comparison that always said
/// `Ok` would look exactly like a green layer.
#[test]
fn a_wrong_render_fails() {
    let suite = suite();
    let test = suite
        .tests()
        .iter()
        .find(|t| t.kind == TestKind::Other && t.exp.as_deref().is_some_and(|e| !e.is_empty()))
        .expect("the suite has a test with expected text");

    l6::check(test).expect("it passes as it is");

    let mut wrong = test.clone();
    wrong.exp = Some(format!("{}!", test.exp.clone().unwrap_or_default()));
    let error = l6::check(&wrong).expect_err("a wrong expectation must fail");
    assert!(error.contains("rendered"), "{error}");
}

/// Every runtime-valid test renders, and renders what the suite expects.
#[test]
fn every_runtime_valid_test_renders() {
    let suite = suite();
    let mut ran = 0;
    for test in suite.tests().iter().filter(|t| t.kind == TestKind::Other) {
        l6::check(test).unwrap_or_else(|e| panic!("{}: {e}", test.key));
        ran += 1;
    }
    assert!(ran >= 300, "only {ran} tests ran");
}
