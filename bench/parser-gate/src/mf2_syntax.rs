//! The contender: this workspace's `mf2-syntax` behind [`Adapter`].
//!
//! Rows (plans/05-tooling.md §1, "like for like"):
//!
//! | Row | Fresh (per message) | Reused (per pass) |
//! |---|---|---|
//! | CST | `parse_cst`: a new arena per message; lossless CST with trivia | one `Parser` (arena) per pass; `Parser::parse_cst` |
//! | + model | `parse_model`: straight to the data model, plus validation (fast path, or a new arena per message) | one `Parser` per pass; `Parser::parse_model` |
//!
//! The reused state starts empty (no pre-sizing): the arena grows to the
//! largest message of the pass, and that growth is part of the measurement.

use std::hint::black_box;

use mf2_model::ErrorClass;
use mf2_syntax::Parser;

use crate::adapter::{Adapter, ErrorSet, Hint, Mode, Stage};

/// The `mf2-syntax` adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mf2Syntax;

impl Adapter for Mf2Syntax {
    type State = Parser;

    fn key(&self) -> &'static str {
        "mf2-syntax"
    }

    fn name(&self) -> &'static str {
        concat!(
            "mf2-syntax ",
            env!("CARGO_PKG_VERSION"),
            " (this workspace)"
        )
    }

    fn api(&self, stage: Stage, mode: Mode) -> &'static str {
        match (stage, mode) {
            (Stage::Cst, Mode::Fresh) => "parse_cst (new arena per message)",
            (Stage::Model, Mode::Fresh) => {
                "parse_model (fast path, or new arena per message) + validation"
            }
            (Stage::Cst, Mode::Reused) => "one Parser per pass + Parser::parse_cst",
            (Stage::Model, Mode::Reused) => {
                "one Parser per pass + Parser::parse_model (+ validation)"
            }
        }
    }

    fn new_state(&self, _stage: Stage, _hint: Hint) -> Parser {
        Parser::new()
    }

    fn cst_fresh(&self, src: &str) -> usize {
        let cst = mf2_syntax::parse_cst(src);
        black_box(&cst);
        cst.diagnostics().len()
    }

    fn model_fresh(&self, src: &str) -> usize {
        let parsed = mf2_syntax::parse_model(src);
        black_box(&parsed);
        parsed.diagnostics.len()
    }

    fn cst_reused(&self, state: &mut Parser, src: &str) -> usize {
        let cst = state.parse_cst(src);
        black_box(&cst);
        cst.diagnostics().len()
    }

    fn model_reused(&self, state: &mut Parser, src: &str) -> usize {
        let parsed = state.parse_model(src);
        black_box(&parsed);
        parsed.diagnostics.len()
    }

    fn classify(&self, src: &str) -> ErrorSet {
        let parsed = mf2_syntax::parse_model(src);
        let mut got = ErrorSet::new();
        if parsed.diagnostics.has_class(ErrorClass::Syntax) {
            got.insert("syntax-error".to_owned());
        } else {
            for d in &parsed.diagnostics {
                got.insert(d.kind.suite_name().to_owned());
            }
        }
        got
    }
}

#[cfg(test)]
mod tests {
    use super::Mf2Syntax;
    use crate::adapter::{Contender, ErrorSet, Mode, Stage};

    fn set(items: &[&str]) -> ErrorSet {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    const TINY: &[&str] = &[
        "Hello",
        "Hello {$name}!",
        "{",
        ".input {$n :integer} .match $n one {{one}} * {{other}}",
        ".input {$x :number} .input {$x :number} {{dup}}",
        ".input {$n :integer} .match $n one {{one}}",
    ];

    #[test]
    fn classifies_syntax_and_data_model_errors() {
        assert_eq!(Mf2Syntax.classify("Hello {$name}!"), set(&[]));
        assert_eq!(Mf2Syntax.classify("{"), set(&["syntax-error"]));
        assert_eq!(
            Mf2Syntax.classify(".input {$x :number} .input {$x :number} {{dup}}"),
            set(&["duplicate-declaration"])
        );
        assert_eq!(
            Mf2Syntax.classify(".input {$n :integer} .match $n one {{one}}"),
            set(&["missing-fallback-variant"])
        );
        // Where ox over-reports (data-model-errors.json #0), mf2-syntax is exact.
        assert_eq!(
            Mf2Syntax.classify(".input {$foo :x} .match $foo * * {{foo}}"),
            set(&["variant-key-mismatch"])
        );
    }

    #[test]
    fn fresh_and_reused_agree_per_stage() {
        let msgs: Vec<String> = TINY.iter().map(|s| (*s).to_owned()).collect();
        for stage in Stage::ALL {
            let fresh = Mf2Syntax.pass(&msgs, stage, Mode::Fresh);
            let reused = Mf2Syntax.pass(&msgs, stage, Mode::Reused);
            assert_eq!(fresh, reused, "{stage:?}");
        }
        // One syntax diagnostic for "{"; the model stage adds the two Data
        // Model errors.
        let cst = Mf2Syntax.pass(&msgs, Stage::Cst, Mode::Fresh);
        let model = Mf2Syntax.pass(&msgs, Stage::Model, Mode::Fresh);
        assert_eq!(cst, 1);
        assert_eq!(model, cst + 2);
    }

    #[test]
    fn placeholder_free_model_rows_allocate_nothing() {
        let msgs: Vec<String> = ["Send", "Can’t reach the server.", "  leading space", ""]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        for mode in Mode::ALL {
            let (_, used) = crate::alloc::count(|| Mf2Syntax.pass(&msgs, Stage::Model, mode));
            assert_eq!(used.allocs, 0, "{mode:?}");
        }
        let (_, used) = crate::alloc::count(|| Mf2Syntax.pass(&msgs, Stage::Cst, Mode::Reused));
        // The reused arena grows once, on the first message.
        assert!(used.allocs <= 1, "{used:?}");
    }
}
