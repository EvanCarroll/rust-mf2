//! Fuzz target `parse`: `mf2-syntax` on arbitrary input.
//!
//! For every UTF-8 input: no panic (libFuzzer reports one), and
//!
//! * the CST is lossless and every span is in bounds and on char boundaries;
//! * `parse_model` gives a model exactly when `parse_cst` finds no syntax
//!   error, and agrees with it on the syntax diagnostics;
//! * a model serializes, and parsing that gives the same model back;
//!   validation and analysis run on it;
//! * **linear time**: the whole check takes at most a fixed budget per input
//!   byte (generous: sanitizers and coverage slow everything down; a
//!   quadratic parser would blow it on inputs of a few KB).
//!
//! Non-UTF-8 input is fed through `from_utf8_lossy`, so every run exercises
//! the parser.

#![no_main]

use std::time::{Duration, Instant};

use libfuzzer_sys::fuzz_target;
use mf2_model::{ErrorClass, Span};

fn span_ok(src: &str, span: Span) -> bool {
    let (s, e) = (span.start as usize, span.end as usize);
    s <= e && e <= src.len() && src.is_char_boundary(s) && src.is_char_boundary(e)
}

fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let src: &str = &src;
    let start = Instant::now();

    let cst = mf2_syntax::parse_cst(src);
    assert_eq!(cst.to_string(), src, "CST is not lossless");
    for d in cst.diagnostics() {
        let span = d.span.expect("syntax diagnostics have spans");
        assert!(span_ok(src, span), "diagnostic span {span:?}");
    }
    for n in cst.view().nodes() {
        assert!(span_ok(src, n.span()), "node span {:?}", n.span());
    }

    let parsed = mf2_syntax::parse_model(src);
    assert_eq!(parsed.message.is_some(), !cst.has_errors());
    let syntax: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.kind.class() == ErrorClass::Syntax)
        .collect();
    assert_eq!(syntax.len(), cst.diagnostics().len());
    for d in parsed.diagnostics.iter() {
        if let Some(span) = d.span {
            assert!(span_ok(src, span), "model diagnostic span {span:?}");
        }
    }

    if let Some(model) = &parsed.message {
        let text = mf2_syntax::serialize(model).expect("a parsed model serializes");
        let again = mf2_syntax::parse_model(&text);
        assert_eq!(again.message.as_ref(), Some(model), "round trip via {text:?}");
        let _ = mf2_syntax::validate(model);
        let _ = mf2_syntax::analyze(model);
    }

    let budget = Duration::from_millis(50) + Duration::from_micros(50) * src.len() as u32;
    let elapsed = start.elapsed();
    assert!(
        elapsed <= budget,
        "{elapsed:?} for {} bytes (budget {budget:?})",
        src.len()
    );
});
