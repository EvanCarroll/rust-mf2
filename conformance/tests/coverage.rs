//! The spec coverage matrix on the committed
//! `conformance/coverage.toml`: every normative statement of the vendored
//! spec covered, `COVERAGE.md` current, and mutations showing each rule of
//! `coverage::check` turns it red.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use mf2_conformance::coverage::{
    self, COVERAGE_MD, Coverage, Entry, Gap, KEYWORDS, Na, NaKind, Statement,
};
use mf2_conformance::spec::spec_dir;
use mf2_conformance::{Suite, load_suite};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

fn inputs() -> (Vec<Statement>, Coverage, Suite) {
    let root = root();
    (
        spec_dir(&root)
            .and_then(|dir| coverage::statements(&dir))
            .unwrap_or_else(|e| panic!("{e}")),
        Coverage::load(&root).expect("conformance/coverage.toml parses"),
        load_suite(&root).expect("the suite loads"),
    )
}

#[test]
fn every_normative_statement_is_covered() {
    let (statements, cov, suite) = inputs();
    let gaps = coverage::check(&statements, &cov, &suite, &root());
    let shown: Vec<String> = gaps.iter().map(ToString::to_string).collect();
    assert!(
        gaps.is_empty(),
        "{} gap(s):\n{}",
        gaps.len(),
        shown.join("\n")
    );
}

#[test]
fn coverage_md_is_current() {
    let (statements, cov, suite) = inputs();
    let root = root();
    let want = coverage::render(&statements, &cov, &suite, coverage::pin(&root).as_deref());
    let have = fs::read_to_string(root.join(COVERAGE_MD)).unwrap_or_default();
    assert!(
        have == want,
        "{COVERAGE_MD} is stale: run `cargo xtask conformance-report`"
    );
}

/// Every use of a key word outside fenced code (a quoted mention excepted)
/// is inside some extracted statement: the sentence splitter loses none.
#[test]
fn the_extraction_loses_no_key_word() {
    let spec = spec_dir(&root()).unwrap_or_else(|e| panic!("{e}"));
    let statements = coverage::statements(&spec).expect("the spec reads");
    // The files with statements, and those that have none.
    let files: BTreeSet<&str> = statements
        .iter()
        .map(|s| s.file.as_str())
        .chain(["intro.md", "appendices.md", "README.md"])
        .collect();
    let mut total = 0;
    for file in files {
        let text = fs::read_to_string(spec.join(file)).expect("spec file");
        // Paragraph by paragraph, so a mention wrapped across lines
        // (`"SHALL` / `NOT"`) is still seen as quoted.
        let mut fenced = false;
        let mut in_file = 0;
        let mut para = String::new();
        for line in text.lines() {
            let t = line.trim_start().trim_start_matches('>').trim_start();
            if t.starts_with("```") {
                fenced = !fenced;
            }
            if fenced || t.is_empty() || t.starts_with("```") {
                in_file += uses(&para);
                para.clear();
            } else {
                para.push(' ');
                para.push_str(t);
            }
        }
        in_file += uses(&para);
        let extracted: usize = statements
            .iter()
            .filter(|s| s.file == file)
            .map(|s| uses(&s.text))
            .sum();
        assert_eq!(
            extracted, in_file,
            "{file}: key-word uses outside statements"
        );
        total += in_file;
    }
    assert!(total > 150, "the spec uses its key words ({total})");
}

/// Uses of key words in `s`, a quoted mention excepted, counting `MUST NOT`
/// once.
fn uses(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut at = 0;
    while at < bytes.len() {
        let before = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
        let hit = KEYWORDS.iter().find(|kw| {
            before
                && bytes[at..].starts_with(kw.as_bytes())
                && bytes
                    .get(at + kw.len())
                    .is_none_or(|c| !c.is_ascii_alphanumeric())
        });
        match hit {
            Some(kw) => {
                let quoted =
                    at > 0 && bytes[at - 1] == b'"' && bytes.get(at + kw.len()) == Some(&b'"');
                count += usize::from(!quoted);
                at += kw.len();
            }
            None => at += 1,
        }
    }
    count
}

fn gaps_after(mutate: impl FnOnce(&mut Coverage, &[Statement])) -> Vec<Gap> {
    let (statements, mut cov, suite) = inputs();
    mutate(&mut cov, &statements);
    coverage::check(&statements, &cov, &suite, &root())
}

fn first_tested(cov: &mut Coverage) -> &mut Entry {
    cov.entries
        .iter_mut()
        .find(|e| !e.tests.is_empty())
        .expect("an entry with tests")
}

#[test]
fn a_statement_without_an_entry_is_uncovered() {
    let gaps = gaps_after(|cov, _| {
        cov.entries.remove(0);
    });
    assert!(matches!(gaps.as_slice(), [Gap::Missing { .. }]), "{gaps:?}");
}

#[test]
fn an_entry_for_no_statement_is_stale() {
    let gaps = gaps_after(|cov, _| {
        let mut e = cov.entries[0].clone();
        e.id = "syntax.md#00000000".into();
        cov.entries.push(e);
    });
    assert!(matches!(gaps.as_slice(), [Gap::Stale { .. }]), "{gaps:?}");
}

#[test]
fn a_test_that_is_not_there_is_dangling() {
    for bad in [
        "syntax.json@00000000",
        "crates/mf2-syntax/tests/cst.rs::no_such_test",
        "no/such/file.rs::x",
        "not a reference",
    ] {
        let gaps = gaps_after(|cov, _| first_tested(cov).tests.push(bad.into()));
        assert!(
            matches!(gaps.as_slice(), [Gap::Dangling { test, .. }] if test == bad),
            "{bad}: {gaps:?}"
        );
    }
}

#[test]
fn an_entry_needs_exactly_one_of_tests_and_na() {
    let na = Na {
        kind: NaKind::ByConstruction,
        reason: "r".into(),
    };
    let both = gaps_after(|cov, _| first_tested(cov).na = Some(na.clone()));
    assert!(
        matches!(both.as_slice(), [Gap::Undecided { .. }]),
        "{both:?}"
    );
    let neither = gaps_after(|cov, _| first_tested(cov).tests.clear());
    assert!(
        matches!(neither.as_slice(), [Gap::Undecided { .. }]),
        "{neither:?}"
    );
}

#[test]
fn a_requirement_is_not_waived_as_a_permission() {
    let gaps = gaps_after(|cov, statements| {
        let must = statements
            .iter()
            .find(|s| s.keywords.contains(&"MUST"))
            .expect("a MUST");
        let e = cov
            .entries
            .iter_mut()
            .find(|e| e.id == must.id)
            .expect("its entry");
        e.tests.clear();
        e.na = Some(Na {
            kind: NaKind::Permission,
            reason: "r".into(),
        });
    });
    assert!(
        matches!(gaps.as_slice(), [Gap::NotAPermission { .. }]),
        "{gaps:?}"
    );
}
