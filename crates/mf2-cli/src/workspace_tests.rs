//! The tests that hold this crate against the repository around it: the
//! plan's tables of report codes and rules, the integration tests that name
//! them, and the pinned specification commit. None of that travels in the
//! package, so this file does not either (`exclude` in `Cargo.toml`), and
//! `build.rs` compiles it only where it is
//! (`plans/17-phase-9-work-order.md` A4).

use crate::convert::leptos_fluent::rewrite::Rule;
use crate::convert::report::Code;
use crate::exchange::xliff::Finding;
use crate::stats::SPEC_COMMIT;

const PLAN: &str = include_str!("../../../plans/05-tooling.md");
const CONVERT_TESTS: &str = include_str!("../tests/convert.rs");

/// Every code of `plans/05-tooling.md` §6.1 has its own test, and the plan
/// names every code this crate writes.
#[test]
fn every_code_is_tested_and_planned() {
    for code in Code::ALL {
        let name = code.name();
        assert!(
            CONVERT_TESTS.contains(&format!("fn {}(", name.replace('-', "_"))),
            "no test named {name} in tests/convert.rs"
        );
        assert!(
            PLAN.contains(&format!("`{name}`")),
            "{name} is not in plans/05 §6.1"
        );
    }
}

/// Every rule of §6.2 has its own test, named after it, and the plan names
/// every rule.
#[test]
fn every_rule_is_tested_and_planned() {
    for rule in Rule::ALL {
        let name = rule.name();
        assert!(
            CONVERT_TESTS.contains(&format!("fn rule_{}(", name.replace('-', "_"))),
            "no test named rule_{name} in tests/convert.rs"
        );
        assert!(
            PLAN.contains(&format!("| `{name}` |")),
            "{name} is not in plans/05 §6.2"
        );
    }
}

/// Every XLIFF finding is in 05 §6.3's table.
#[test]
fn every_xliff_code_is_in_the_plan() {
    for f in Finding::ALL {
        assert!(
            PLAN.contains(&format!("**`{}`**", f.code())),
            "{}",
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
