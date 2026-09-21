//! The baseline: `ox_mf2_parser` =0.14.0-alpha.12 behind [`Adapter`].
//!
//! Rows (plans/05-tooling.md §1):
//!
//! | Row | Fresh (per message) | Reused (per pass) |
//! |---|---|---|
//! | CST | new `SourceStore`, `add`, `parse_source` | one `SourceStore` (pre-sized) and one `ParseWorkspace`; per message `add` + `parse_source_session` |
//! | + model | new `SourceStore`, `add`, `parse_source`, then — if no syntax diagnostic — `build_semantic_model` + `validate_semantics` | one `SourceStore` (pre-sized); per message `add` + `parse_source` + the same model calls |
//!
//! The reused rows use the leanest reuse ox offers for each stage. For the
//! model row that is only the shared `SourceStore`: `build_semantic_model`
//! takes an owned `ParseResult`, which `parse_source_session` (borrowed from
//! the workspace) cannot provide, and `parse_source` builds its own
//! `ParseWorkspace` per call. `SourceStore` has no `clear`: it keeps every
//! message of the pass (it copies each source into an owned `String`).

use ox_mf2_parser::{
    ParseOptions, ParseWorkspace, SourceFileInput, SourceId, SourceStore, build_semantic_model,
    parse_source, parse_source_session, validate_semantics,
};
use std::hint::black_box;

use crate::adapter::{Adapter, ErrorSet, Hint, Mode, Stage};

/// The pinned version (must match `[workspace.dependencies]`).
pub const VERSION: &str = "0.14.0-alpha.12";

/// ox's defaults, as the audit used them: error recovery on, trivia kept (a
/// lossless CST).
fn options() -> ParseOptions {
    ParseOptions::default()
}

/// The `ox_mf2_parser` adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ox;

/// Reused state: the shared store, plus the parse workspace for the CST row.
pub struct OxState {
    sources: SourceStore,
    workspace: Option<ParseWorkspace>,
}

fn add(sources: &mut SourceStore, src: &str) -> SourceId {
    sources.add(SourceFileInput {
        source: src,
        ..SourceFileInput::default()
    })
}

/// `parse_source`, then the model and validation if there is no syntax
/// diagnostic. Returns the diagnostic count.
fn parse_and_validate(sources: &SourceStore, id: SourceId) -> usize {
    let Ok(parsed) = parse_source(sources, id, options()) else {
        return 1;
    };
    black_box(&parsed);
    if !parsed.diagnostics.is_empty() {
        return parsed.diagnostics.len();
    }
    match build_semantic_model(sources, &parsed).and_then(|model| {
        black_box(&model);
        validate_semantics(&model)
    }) {
        Ok(errors) => errors.len(),
        Err(_) => 1,
    }
}

/// ox's names for two Data Model Errors differ from the spec's (the mapping
/// of `probes/audit/ox-conformance`).
fn spec_name(code: &str) -> &str {
    match code {
        "variant-key-arity-mismatch" => "variant-key-mismatch",
        "invalid-declaration-dependency" => "duplicate-declaration",
        other => other,
    }
}

impl Adapter for Ox {
    type State = OxState;

    fn key(&self) -> &'static str {
        "ox"
    }

    fn name(&self) -> &'static str {
        "ox_mf2_parser 0.14.0-alpha.12"
    }

    fn api(&self, stage: Stage, mode: Mode) -> &'static str {
        match (stage, mode) {
            (Stage::Cst, Mode::Fresh) => "SourceStore per message + parse_source",
            (Stage::Model, Mode::Fresh) => {
                "SourceStore per message + parse_source + build_semantic_model + validate_semantics"
            }
            (Stage::Cst, Mode::Reused) => {
                "shared SourceStore + shared ParseWorkspace + parse_source_session"
            }
            (Stage::Model, Mode::Reused) => {
                "shared SourceStore + parse_source + build_semantic_model + validate_semantics"
            }
        }
    }

    fn new_state(&self, stage: Stage, hint: Hint) -> OxState {
        OxState {
            sources: SourceStore::with_capacity(hint.messages),
            workspace: match stage {
                Stage::Cst => Some(ParseWorkspace::new()),
                Stage::Model => None,
            },
        }
    }

    fn cst_fresh(&self, src: &str) -> usize {
        let mut sources = SourceStore::new();
        let id = add(&mut sources, src);
        match parse_source(&sources, id, options()) {
            Ok(parsed) => {
                black_box(&parsed);
                parsed.diagnostics.len()
            }
            Err(_) => 1,
        }
    }

    fn model_fresh(&self, src: &str) -> usize {
        let mut sources = SourceStore::new();
        let id = add(&mut sources, src);
        parse_and_validate(&sources, id)
    }

    fn cst_reused(&self, state: &mut OxState, src: &str) -> usize {
        let id = add(&mut state.sources, src);
        let Some(workspace) = state.workspace.as_mut() else {
            // `new_state(Stage::Cst, …)` always creates the workspace.
            return 1;
        };
        match parse_source_session(&state.sources, id, workspace, options()) {
            Ok(parsed) => {
                black_box(&parsed);
                parsed.diagnostics.len()
            }
            Err(_) => 1,
        }
    }

    fn model_reused(&self, state: &mut OxState, src: &str) -> usize {
        let id = add(&mut state.sources, src);
        parse_and_validate(&state.sources, id)
    }

    fn classify(&self, src: &str) -> ErrorSet {
        let mut sources = SourceStore::new();
        let id = add(&mut sources, src);
        let mut got = ErrorSet::new();
        match parse_source(&sources, id, options()) {
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
                            got.insert(spec_name(e.code().json_code()).to_owned());
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
}

#[cfg(test)]
mod tests {
    use super::Ox;
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

    /// Reports name the version; the root manifest must pin exactly that one.
    #[test]
    fn name_matches_the_exact_pin() {
        assert!(Contender::name(&Ox).ends_with(super::VERSION));
        let manifest = std::fs::read_to_string(crate::repo_root().join("Cargo.toml"))
            .expect("root Cargo.toml");
        let pin = manifest
            .lines()
            .find(|l| l.trim_start().starts_with("ox_mf2_parser"))
            .expect("ox_mf2_parser in [workspace.dependencies]");
        assert!(pin.contains(&format!("\"={}\"", super::VERSION)), "{pin}");
    }

    #[test]
    fn classifies_syntax_and_data_model_errors() {
        assert_eq!(Ox.classify("Hello {$name}!"), set(&[]));
        assert_eq!(Ox.classify("{"), set(&["syntax-error"]));
        assert_eq!(
            Ox.classify(".input {$x :number} .input {$x :number} {{dup}}"),
            set(&["duplicate-declaration"])
        );
        assert_eq!(
            Ox.classify(".input {$n :integer} .match $n one {{one}}"),
            set(&["missing-fallback-variant"])
        );
    }

    #[test]
    fn fresh_and_reused_agree_per_stage() {
        let msgs: Vec<String> = TINY.iter().map(|s| (*s).to_owned()).collect();
        for stage in Stage::ALL {
            let fresh = Ox.pass(&msgs, stage, Mode::Fresh);
            let reused = Ox.pass(&msgs, stage, Mode::Reused);
            assert_eq!(fresh, reused, "{stage:?}");
        }
        // One syntax diagnostic at least for "{"; the model stage adds the two
        // Data Model errors.
        let cst = Ox.pass(&msgs, Stage::Cst, Mode::Fresh);
        let model = Ox.pass(&msgs, Stage::Model, Mode::Fresh);
        assert!(cst >= 1);
        assert_eq!(model, cst + 2);
    }

    #[test]
    fn reused_state_allocates_less_than_fresh() {
        let msgs: Vec<String> = TINY.iter().map(|s| (*s).to_owned()).collect();
        let (_, fresh) = crate::alloc::count(|| Ox.pass(&msgs, Stage::Cst, Mode::Fresh));
        let (_, reused) = crate::alloc::count(|| Ox.pass(&msgs, Stage::Cst, Mode::Reused));
        assert!(fresh.allocs > 0);
        assert!(reused.allocs < fresh.allocs, "{reused:?} vs {fresh:?}");
    }
}
