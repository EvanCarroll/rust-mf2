//! The tests that hold this crate against the repository around it: the
//! integration tests that name every report code, rule and XLIFF finding,
//! and the pinned specification commit. None of that travels in the
//! package, so this file does not either (`exclude` in `Cargo.toml`), and
//! `build.rs` compiles it only where it is (the Phase 9 work order A4).

use crate::convert::leptos_fluent::rewrite::Rule;
use crate::convert::report::Code;
use crate::exchange::xliff::Finding;
use crate::stats::SPEC_COMMIT;

const CONVERT_TESTS: &str = include_str!("../tests/convert.rs");
const XLIFF_TESTS: &str = include_str!("../tests/xliff.rs");

/// Every conversion code this crate writes has its own test, named after it.
#[test]
fn every_code_is_tested() {
    for code in Code::ALL {
        let name = code.name();
        assert!(
            CONVERT_TESTS.contains(&format!("fn {}(", name.replace('-', "_"))),
            "no test named {name} in tests/convert.rs"
        );
    }
}

/// Every call-site rewrite rule has its own test, named after it.
#[test]
fn every_rule_is_tested() {
    for rule in Rule::ALL {
        let name = rule.name();
        assert!(
            CONVERT_TESTS.contains(&format!("fn rule_{}(", name.replace('-', "_"))),
            "no test named rule_{name} in tests/convert.rs"
        );
    }
}

/// `tests/xliff.rs` lists the import codes literally, because an integration
/// test cannot see `Finding`; this holds that list against the enum, both
/// its length and every code in it.
#[test]
fn every_xliff_code_is_listed_by_its_tests() {
    assert!(
        XLIFF_TESTS.contains(&format!("IMPORT_CODES: [&str; {}]", Finding::ALL.len())),
        "tests/xliff.rs lists a different number of import codes"
    );
    for f in Finding::ALL {
        assert!(
            XLIFF_TESTS.contains(&format!("\"{}\"", f.code())),
            "{} is not in tests/xliff.rs's IMPORT_CODES",
            f.code()
        );
    }
}

/// The pin `mf2 stats` reports is the one `third_party/` is at.
#[test]
fn the_spec_commit_is_the_pinned_one() {
    let pin = include_str!("../../../third_party/message-format-wg/PIN");
    let commit = pin
        .lines()
        .find_map(|l| l.strip_prefix("commit"))
        .and_then(|l| l.split('=').nth(1))
        .map(str::trim)
        .expect("the PIN names a commit");
    assert_eq!(commit, SPEC_COMMIT, "the PIN is not what stats reports");
}
