//! Layer L2 — the data model (plans/01-conformance.md §3).
//!
//! For each test that parses (the `n/a` matrix excludes syntax-error tests):
//! lower to the interchange data model and validate. The set of Data Model
//! Errors reported equals the data-model subset of `expErrors`. Invariants:
//!
//! * every data-model diagnostic has an in-bounds span, and
//!   [`mf2_syntax::validate`] on the model alone reports the same kinds;
//! * `serialize(m)` is L1-clean and `parse(serialize(m)) == m`;
//! * for a message without data-model errors, `m` as JSON validates against
//!   the vendored `spec/data-model/message.json`.

use std::collections::BTreeSet;

use mf2_model::{Diagnostics, ErrorClass};

use crate::l1::span_ok;
use crate::matrix::DATA_MODEL_ERRORS;
use crate::suite::SuiteTest;

/// Checks one test at L2 against the data model's JSON Schema: `Ok` = pass,
/// `Err` = what failed.
pub fn check(test: &SuiteTest, schema: &jsonschema::Validator) -> Result<(), String> {
    let want: BTreeSet<&str> = test
        .exp_errors
        .iter()
        .map(String::as_str)
        .filter(|e| DATA_MODEL_ERRORS.contains(e))
        .collect();
    check_source(&test.src, &want, schema)
}

/// Checks `src` at L2, given the expected data-model error names.
pub fn check_source(
    src: &str,
    want: &BTreeSet<&str>,
    schema: &jsonschema::Validator,
) -> Result<(), String> {
    let parsed = mf2_syntax::parse_model(src);
    let Some(model) = parsed.message else {
        return Err(format!("syntax error: {:?}", parsed.diagnostics));
    };
    let got = kinds(&parsed.diagnostics);
    if &got != want {
        return Err(format!("data-model errors: want {want:?}, got {got:?}"));
    }
    for d in &parsed.diagnostics {
        if d.kind.class() != ErrorClass::DataModel {
            return Err(format!("non-data-model diagnostic: {d:?}"));
        }
        let span = d
            .span
            .ok_or_else(|| format!("diagnostic without a span: {d:?}"))?;
        span_ok(src, span).map_err(|e| format!("diagnostic {d:?}: {e}"))?;
    }
    let alone = kinds(&mf2_syntax::validate(&model));
    if alone != got {
        return Err(format!(
            "validate(model) reports {alone:?}, parse_model reports {got:?}"
        ));
    }

    let serialized = mf2_syntax::serialize(&model).map_err(|e| format!("serialize: {e}"))?;
    crate::l1::check_source(&serialized, false)
        .map_err(|e| format!("serialize(m) = {serialized:?} is not L1-clean: {e}"))?;
    let again = mf2_syntax::parse_model(&serialized);
    if again.message.as_ref() != Some(&model) {
        return Err(format!(
            "parse(serialize(m)) != m; serialize(m) = {serialized:?}"
        ));
    }

    if want.is_empty() {
        let json = serde_json::to_value(&model).map_err(|e| format!("model → JSON: {e}"))?;
        if let Err(e) = schema.validate(&json) {
            return Err(format!("model JSON fails message.json: {e}; JSON: {json}"));
        }
    }
    Ok(())
}

fn kinds(d: &Diagnostics) -> BTreeSet<&'static str> {
    d.iter().map(|d| d.kind.suite_name()).collect()
}

/// Compiles the data model's JSON Schema (`spec/data-model/message.json`).
pub fn schema(text: &str) -> Result<jsonschema::Validator, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("message.json is not JSON: {e}"))?;
    jsonschema::validator_for(&value).map_err(|e| format!("message.json is not a schema: {e}"))
}
