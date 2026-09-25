//! Layer L3 on grammar-driven input (plans/01-conformance.md §5;
//! plans/09-phase-2-work-order.md A6): random well-formed messages generated
//! from the vendored `message.abnf`, each written as a one-message catalog
//! and decoded back ([`mf2_conformance::l3::check_model`]):
//!
//! * the unstripped catalog decodes to the model — every model that parses,
//!   including those with data-model errors (the writer accepts invalid
//!   models; the build refuses to ship them, the format does not);
//! * the catalog is rejected under a wrong manifest hash;
//! * the stripped catalog decodes to the formatting-relevant model, and says
//!   `cold_dropped` exactly when the message has COLD data;
//! * the formatting-relevant projection is idempotent.
//!
//! `cargo test` runs a bounded number of cases; set `MF2_GEN_CASES` (and
//! optionally `MF2_GEN_SEED`) for longer runs, e.g.
//! `MF2_GEN_CASES=1000000 cargo test --release -p mf2-conformance --test generated_l3`.
//! The seed is `generated.rs`'s, so case `n` is the same message in both.

use std::path::{Path, PathBuf};

use mf2_conformance::abnf::{Generator, Grammar};
use mf2_conformance::l3::{check_model, formatting_model};
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
fn generated_messages_round_trip_through_a_catalog() {
    let g = grammar();
    let base = seed();
    let (mut complex, mut invalid, mut cold, mut spelling) = (0u64, 0u64, 0u64, 0u64);
    for case in 0..cases() {
        let mut generator = Generator::new(&g, base.wrapping_add(case));
        let src = generator.generate("message");
        let parsed = mf2_syntax::parse_model(&src);
        let Some(model) = parsed.message else {
            panic!("case {case}: no model for {src:?}");
        };
        complex += u64::from(!model.declarations().is_empty());
        invalid += u64::from(!parsed.diagnostics.is_empty());
        let a = mf2_syntax::analyze(&model);
        spelling += u64::from(
            [&a.externals, &a.locals, &a.markup, &a.functions]
                .into_iter()
                .flatten()
                .any(|n| n.spelling != n.nfc),
        );
        if let Err(e) = check_model(&model, "en") {
            panic!("case {case}: L3 fails on generated {src:?}: {e}");
        }
        let projected = formatting_model(&model);
        cold += u64::from(projected != model);
        assert_eq!(
            formatting_model(&projected),
            projected,
            "case {case}: the projection is not idempotent on {src:?}"
        );
    }
    // The generator reaches complex messages, messages with COLD data (among
    // them names not in NFC) and invalid models, not only simple ones.
    let n = cases();
    assert!(complex * 10 > n, "only {complex} complex messages");
    assert!(cold * 10 > n, "only {cold} messages with COLD data");
    assert!(
        spelling * 1000 > n,
        "only {spelling} messages with a non-NFC name"
    );
    assert!(invalid * 100 > n, "only {invalid} invalid models");
    eprintln!(
        "generated_l3: {n} cases: {complex} complex, {cold} with COLD data, \
         {spelling} with a non-NFC name, {invalid} with data-model errors"
    );
}
