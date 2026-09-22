//! Layer L4 — the runtime, from the catalog (plans/01-conformance.md §3;
//! plans/10-phase-3-work-order.md A9).
//!
//! A test expecting a syntax or data-model error passes when the one-message
//! compile (`mf2::compile_str`) refuses the message with exactly those
//! kinds. Every other test is compiled for its `locale`, **unstripped and
//! stripped**, and formatted from each catalog by `mf2-l4-runner` — the
//! same code `cargo xtask l4-wasi` runs on `wasm32-wasip1` — with its
//! `params` (named, and positionally through the catalog's slot names), its
//! `bidiIsolation`, and the suite's `:test:*` functions:
//!
//! * the stripped catalog formats exactly as the unstripped one (the
//!   ledger's `stripped-formats-identically` obligation);
//! * the string equals `exp`, when the test has one;
//! * the parts match `expParts`, when the test has them: the same number
//!   of parts, and every key a part of `expParts` names present with an
//!   equal value (the suite's parts name only some keys — a string part's
//!   `locale` in one test, not in the next);
//! * the errors, as a multiset (the spec fixes no order), equal `expErrors`
//!   (absent ⇒ none), in string and in parts output alike.

use mf2_l4_runner::{ArgSpec, Case, Record};
use mf2_runtime::BidiStrategy;
use serde_json::Value;

use crate::matrix::TestKind;
use crate::suite::SuiteTest;

/// Checks one test at L4: `Ok` = pass, `Err` = what failed.
pub fn check(test: &SuiteTest) -> Result<(), String> {
    match test.kind {
        TestKind::SyntaxError | TestKind::DataModelError => check_rejected(test),
        TestKind::Other => {
            let (unstripped, stripped) = cases(test)?;
            let got = mf2_l4_runner::run(&unstripped)?;
            let got_stripped = mf2_l4_runner::run(&stripped)?;
            if got != got_stripped {
                return Err(format!(
                    "the stripped catalog formats differently:\n    unstripped {}\n    stripped   {}",
                    got.line(),
                    got_stripped.line()
                ));
            }
            compare(test, &got)
        }
    }
}

/// The compile refuses the message with exactly the expected kinds.
fn check_rejected(test: &SuiteTest) -> Result<(), String> {
    match mf2::compile_str(&test.src, &test.locale) {
        Ok(_) => Err(format!(
            "compile_str accepted a message the suite expects to be refused ({:?})",
            test.exp_errors
        )),
        Err(e) => {
            let mut got: Vec<&str> = e.kinds().iter().map(|k| k.suite_name()).collect();
            got.sort_unstable();
            got.dedup();
            let mut want: Vec<&str> = test.exp_errors.iter().map(String::as_str).collect();
            want.sort_unstable();
            want.dedup();
            if got == want {
                Ok(())
            } else {
                Err(format!(
                    "compile_str refused with {got:?}, expected {want:?}"
                ))
            }
        }
    }
}

/// The test's arguments, as the runner takes them.
pub fn args(test: &SuiteTest) -> Vec<(String, ArgSpec)> {
    let Some(Value::Array(params)) = &test.params else {
        return Vec::new();
    };
    params
        .iter()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?.to_owned();
            let typed = p.get("type").and_then(Value::as_str);
            let arg = match (typed, p.get("value")) {
                (None, Some(Value::String(s))) => ArgSpec::Str(s.clone()),
                (None, Some(Value::Number(n))) => match n.as_i64() {
                    Some(i) => ArgSpec::Int(i),
                    None => n.as_f64().map_or(ArgSpec::Other, ArgSpec::Float),
                },
                // Booleans, objects, typed (`datetime`) values: nothing a core
                // function takes (dates are Phase 4).
                _ => ArgSpec::Other,
            };
            Some((name, arg))
        })
        .collect()
}

/// The test compiled unstripped and stripped.
pub fn cases(test: &SuiteTest) -> Result<(Case, Case), String> {
    let bidi = match test.bidi_isolation.as_deref() {
        Some("none") => BidiStrategy::None,
        _ => BidiStrategy::Default,
    };
    let id = format!("{}#{}", test.key.file, test.index);
    let args = args(test);
    let make = |stripped: bool| -> Result<Case, String> {
        let compiled = if stripped {
            mf2::compile_str_stripped(&test.src, &test.locale)
        } else {
            mf2::compile_str(&test.src, &test.locale)
        };
        let compiled = compiled.map_err(|e| format!("compile_str: {e} {:?}", e.kinds()))?;
        Ok(Case {
            id: id.clone(),
            manifest_hash: compiled.manifest.hash(),
            catalog: compiled.catalog.into_bytes(),
            bidi,
            args: args.clone(),
        })
    };
    Ok((make(false)?, make(true)?))
}

fn compare(test: &SuiteTest, got: &Record) -> Result<(), String> {
    let mut problems = Vec::new();
    if let Some(exp) = &test.exp
        && &got.text != exp
    {
        problems.push(format!("string {:?}, expected {exp:?}", got.text));
    }
    let mut want: Vec<String> = test.exp_errors.clone();
    want.sort();
    if got.errors != want {
        problems.push(format!("errors {:?}, expected {want:?}", got.errors));
    }
    if got.parts_errors != want {
        problems.push(format!(
            "errors of the parts run {:?}, expected {want:?}",
            got.parts_errors
        ));
    }
    if let Some(exp_parts) = &test.exp_parts {
        let parts: Value = serde_json::from_str(&got.parts)
            .map_err(|e| format!("the runner's parts are not JSON: {e}"))?;
        if !subset(exp_parts, &parts) {
            problems.push(format!("parts {}, expected {exp_parts}", got.parts));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// `want` ⊆ `got`: arrays of equal length, element-wise; objects key-wise
/// (every key of `want` in `got`, recursively); other values equal.
fn subset(want: &Value, got: &Value) -> bool {
    match (want, got) {
        (Value::Array(w), Value::Array(g)) => {
            w.len() == g.len() && w.iter().zip(g).all(|(w, g)| subset(w, g))
        }
        (Value::Object(w), Value::Object(g)) => w
            .iter()
            .all(|(k, wv)| g.get(k).is_some_and(|gv| subset(wv, gv))),
        _ => want == got,
    }
}
