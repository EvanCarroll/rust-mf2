//! Every generated message through `tr!`, against what layer L4 makes of it
//! (`plans/13-phase-5b-work-order.md` A8).
//!
//! The two sides are given the same message and the same arguments and must
//! produce the same record — the same string, the same errors, the same
//! parts. What differs between them is the whole point: L4 compiles one
//! message on its own and formats it with every handler, while L5 formats a
//! call site the macro lowered to slots, against a corpus `mf2-build`
//! compiled and the closed-world registry that corpus needs.

use mf2::{BidiStrategy, Catalog, FormatContext, Formatter};
use mf2_l4_runner::{ArgSpec, Case, Config, JsonParts, Record, record_from, run};

/// The arguments of `id`, as both sides take them.
fn args(id: &str) -> Vec<(String, ArgSpec)> {
    mf2_l5_generated::ARGS
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, f)| f())
        .unwrap_or_default()
}

/// The call site's record: `tr!` against the corpus's catalog.
fn l5_record(id: &str, bidi: BidiStrategy, catalog: &Catalog) -> Result<Record, String> {
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.bidi = bidi;
    let f = Formatter::new(catalog, mf2_l5_generated::registry(), &cx);
    let case = mf2_l5_generated::case(id).ok_or_else(|| format!("no call site for {id}"))?;

    let mut text = String::new();
    let mut errors = Vec::new();
    case.write(&f, &mut text, &mut errors);

    let mut parts = JsonParts::default();
    let mut parts_errors = Vec::new();
    case.parts(&f, &mut parts, &mut parts_errors);

    record_from(text, &errors, &parts, &parts_errors)
}

#[test]
fn every_generated_case_formats_as_layer_l4_does() {
    let catalog = Catalog::new(
        mf2_l5_generated::catalog_bytes().to_vec(),
        mf2_l5_generated::MANIFEST_HASH,
    )
    .expect("the corpus's catalog loads");

    let mut checked = 0usize;
    let mut failures = Vec::new();
    for (id, source, bidi_none) in mf2_l5_generated::META {
        let bidi = if *bidi_none {
            BidiStrategy::None
        } else {
            BidiStrategy::Default
        };
        let compiled = match mf2::compile_str(source, mf2_l5_generated::LOCALE) {
            Ok(compiled) => compiled,
            // The corpus accepted it, so `compile_str` must too; if it does
            // not, that is the finding.
            Err(e) => {
                failures.push(format!("{id}: compile_str refused it: {e} {:?}", e.kinds()));
                continue;
            }
        };
        let case = Case {
            id: (*id).to_owned(),
            manifest_hash: compiled.manifest.hash(),
            catalog: compiled.catalog.into_bytes(),
            bidi,
            args: args(id),
            config: Config::All,
        };
        let l4 = match run(&case) {
            Ok(record) => record,
            Err(e) => {
                failures.push(format!("{id}: L4 did not run: {e}"));
                continue;
            }
        };
        match l5_record(id, bidi, &catalog) {
            Ok(l5) if l5 == l4 => checked += 1,
            Ok(l5) => failures.push(format!(
                "{id}: {source:?}\n    L4 {}\n    L5 {}",
                l4.line(),
                l5.line()
            )),
            Err(e) => failures.push(format!("{id}: L5 did not run: {e}")),
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} generated cases differ:\n{}",
        failures.len(),
        mf2_l5_generated::META.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(checked, mf2_l5_generated::META.len());
    assert!(
        checked >= 200,
        "the default run is 200 cases, got {checked}"
    );
}
