//! Grammar-driven properties:
//! random well-formed messages generated from the vendored `message.abnf`.
//!
//! * L1: no syntax error (the grammar is the definition of well-formed), a
//!   lossless CST, spans in bounds;
//! * L2 round trip: `parse(serialize(m)) == m`, and `serialize(m)` is
//!   L1-clean;
//! * validation is deterministic.
//!
//! `cargo test` runs a bounded number of cases; set `MF2_GEN_CASES` (and
//! optionally `MF2_GEN_SEED`) for longer runs, e.g.
//! `MF2_GEN_CASES=1000000 cargo test --release -p mf2-conformance --test generated`.

use std::path::{Path, PathBuf};

use mf2_conformance::abnf::{Generator, Grammar};
use mf2_conformance::spec::{ABNF, read_spec};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

fn grammar() -> Grammar {
    let text = read_spec(&root(), ABNF).unwrap_or_else(|e| panic!("{e}"));
    Grammar::parse(&text).expect("the spec's ABNF parses")
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn cases() -> u64 {
    env_u64("MF2_GEN_CASES", 3000)
}

fn seed() -> u64 {
    env_u64("MF2_GEN_SEED", 0x6d66_3274_776f)
}

#[test]
fn the_spec_abnf_defines_every_rule_it_uses() {
    let g = grammar();
    for rule in [
        "message",
        "simple-message",
        "complex-message",
        "name-start",
        "bidi",
    ] {
        assert!(g.rule(rule).is_some(), "{rule}");
    }
}

#[test]
fn generated_messages_are_well_formed_lossless_and_round_trip() {
    let g = grammar();
    let base = seed();
    let mut complex = 0u64;
    for case in 0..cases() {
        let mut generator = Generator::new(&g, base.wrapping_add(case));
        let src = generator.generate("message");
        if let Err(e) = mf2_conformance::l1::check_source(&src, false) {
            panic!("case {case}: L1 fails on generated {src:?}: {e}");
        }
        let parsed = mf2_syntax::parse_model(&src);
        let Some(model) = parsed.message else {
            panic!("case {case}: no model for {src:?}");
        };
        complex += u64::from(!model.declarations().is_empty());
        let serialized = match mf2_syntax::serialize(&model) {
            Ok(s) => s,
            Err(e) => panic!("case {case}: serialize fails on {src:?}: {e}"),
        };
        if let Err(e) = mf2_conformance::l1::check_source(&serialized, false) {
            panic!("case {case}: serialize({src:?}) = {serialized:?} is not L1-clean: {e}");
        }
        let again = mf2_syntax::parse_model(&serialized);
        assert_eq!(
            again.message.as_ref(),
            Some(&model),
            "case {case}: round trip of {src:?} via {serialized:?}"
        );
        // Deterministic validation, and parse_model's data-model errors are
        // exactly what validate() finds on the model alone.
        let kinds =
            |d: &mf2_model::Diagnostics| d.iter().map(|d| (d.kind, d.code)).collect::<Vec<_>>();
        let alone = mf2_syntax::validate(&model);
        assert_eq!(kinds(&alone), kinds(&mf2_syntax::validate(&model)));
        let mut from_parse = kinds(&parsed.diagnostics);
        let mut from_model = kinds(&alone);
        from_parse.sort_by_key(|(k, c)| (*k as u8, *c));
        from_model.sort_by_key(|(k, c)| (*k as u8, *c));
        assert_eq!(from_parse, from_model, "case {case}: {src:?}");
    }
    // The generator reaches complex messages, not only simple ones.
    assert!(complex * 10 > cases(), "only {complex} complex messages");
}
