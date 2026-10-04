//! `ox_mf2_parser` as a differential oracle: on every suite test, the syntax
//! and Data Model errors it reports include the expected ones (*expected ⊆
//! reported*), and where it reports more, it is one of its two known
//! over-reports. `mf2-syntax`, held to *exactly* the expected set by L1/L2,
//! agrees with it everywhere else.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mf2_conformance::matrix::DATA_MODEL_ERRORS;
use mf2_conformance::{SUITE_DIR, Suite};
use mf2_model::ErrorClass;
use ox_mf2_parser::{
    ParseOptions, SourceFileInput, SourceStore, build_semantic_model, parse_source,
    validate_semantics,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

type Set = BTreeSet<String>;

/// ox's report for `src`: `syntax-error` if it has a syntax diagnostic, else
/// its Data Model errors under the spec's names.
fn ox(src: &str) -> Set {
    let mut sources = SourceStore::new();
    let id = sources.add(SourceFileInput {
        source: src,
        ..SourceFileInput::default()
    });
    let mut got = Set::new();
    match parse_source(&sources, id, ParseOptions::default()) {
        Err(e) => {
            got.insert(format!("internal:parse:{e}"));
        }
        Ok(parsed) if !parsed.diagnostics.is_empty() => {
            got.insert("syntax-error".to_owned());
        }
        Ok(parsed) => {
            match build_semantic_model(&sources, &parsed).and_then(|m| validate_semantics(&m)) {
                Ok(errors) => {
                    for e in errors {
                        // ox's names for two of the errors differ from the spec's.
                        let name = match e.code().json_code() {
                            "variant-key-arity-mismatch" => "variant-key-mismatch",
                            "invalid-declaration-dependency" => "duplicate-declaration",
                            other => other,
                        };
                        got.insert(name.to_owned());
                    }
                }
                Err(e) => {
                    got.insert(format!("internal:semantic:{e}"));
                }
            }
        }
    }
    got
}

/// `mf2-syntax`'s report, in the same shape.
fn ours(src: &str) -> Set {
    let parsed = mf2_syntax::parse_model(src);
    if parsed.diagnostics.has_class(ErrorClass::Syntax) {
        return ["syntax-error".to_owned()].into();
    }
    parsed
        .diagnostics
        .iter()
        .map(|d| d.kind.suite_name().to_owned())
        .collect()
}

#[test]
fn ox_reports_every_expected_error_and_mf2_syntax_exactly_those() {
    let suite = Suite::load(&root().join(SUITE_DIR)).expect("suite");
    let mut over_reports = Vec::new();
    for t in suite.tests() {
        let want: Set = t
            .exp_errors
            .iter()
            .filter(|e| *e == "syntax-error" || DATA_MODEL_ERRORS.contains(&e.as_str()))
            .cloned()
            .collect();
        let theirs = ox(&t.src);
        assert!(
            want.is_subset(&theirs),
            "{}#{}: ox misses expected errors: want {want:?}, ox {theirs:?}, src {:?}",
            t.key.file,
            t.index,
            t.src
        );
        assert_eq!(ours(&t.src), want, "{}#{}", t.key.file, t.index);
        if theirs != want {
            over_reports.push((t.key.file.clone(), t.index, theirs));
        }
    }
    // The two over-reports recorded by the Phase 0 baseline (P0.12): ox adds
    // `missing-fallback-variant` next to `variant-key-mismatch` although a
    // variant of all-`*` keys exists (the key count is what is wrong).
    let expected_extra: Set = ["missing-fallback-variant", "variant-key-mismatch"]
        .map(str::to_owned)
        .into();
    assert_eq!(
        over_reports,
        vec![
            (
                "data-model-errors.json".to_owned(),
                0,
                expected_extra.clone()
            ),
            ("data-model-errors.json".to_owned(), 1, expected_extra),
        ]
    );
}
