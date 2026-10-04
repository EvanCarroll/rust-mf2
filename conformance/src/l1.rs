//! Layer L1 — syntax.
//!
//! For each test: parse `src` with `mf2-syntax`. A syntax error is reported
//! **iff** `expErrors` contains `syntax-error`. Invariants: the CST is
//! lossless (`cst.to_string() == src`, byte for byte); every diagnostic and
//! node span is in bounds and on char boundaries; the model entry point
//! agrees on whether there is a syntax error, and gives a model exactly when
//! there is none. (Never panicking is implied: a panic fails the run.)

use mf2_model::{ErrorKind, Span};

use crate::suite::SuiteTest;

/// Checks one test at L1: `Ok` = pass, `Err` = what failed.
pub fn check(test: &SuiteTest) -> Result<(), String> {
    check_source(
        &test.src,
        test.exp_errors.iter().any(|e| e == "syntax-error"),
    )
}

/// Checks `src` at L1, given whether a syntax error is expected.
pub fn check_source(src: &str, expect_syntax_error: bool) -> Result<(), String> {
    let cst = mf2_syntax::parse_cst(src);
    if cst.has_errors() != expect_syntax_error {
        return Err(if expect_syntax_error {
            "no syntax error reported".to_owned()
        } else {
            format!("unexpected syntax error(s): {:?}", cst.diagnostics())
        });
    }
    let text = cst.to_string();
    if text != src {
        return Err(format!("CST is not lossless: {text:?}"));
    }
    for d in cst.diagnostics() {
        if d.kind != ErrorKind::Syntax {
            return Err(format!("parse_cst reported a non-syntax error: {d:?}"));
        }
        match d.span {
            Some(span) => span_ok(src, span).map_err(|e| format!("diagnostic {d:?}: {e}"))?,
            None => return Err(format!("diagnostic without a span: {d:?}")),
        }
    }
    for n in cst.view().nodes() {
        span_ok(src, n.span()).map_err(|e| format!("node {n:?}: {e}"))?;
    }
    let parsed = mf2_syntax::parse_model(src);
    let model_syntax_error = parsed.diagnostics.has(ErrorKind::Syntax);
    if model_syntax_error != expect_syntax_error {
        return Err(format!(
            "parse_model disagrees about syntax errors: {:?}",
            parsed.diagnostics
        ));
    }
    if parsed.message.is_some() == model_syntax_error {
        return Err("parse_model must give a model exactly when there is no syntax error".into());
    }
    Ok(())
}

/// A span is in bounds and on char boundaries.
pub fn span_ok(src: &str, span: Span) -> Result<(), String> {
    let (start, end) = (span.start as usize, span.end as usize);
    if start > end || end > src.len() {
        return Err(format!(
            "span {start}..{end} out of bounds (len {})",
            src.len()
        ));
    }
    if !src.is_char_boundary(start) || !src.is_char_boundary(end) {
        return Err(format!("span {start}..{end} not on char boundaries"));
    }
    Ok(())
}
