//! Variable analysis: the suite forms layer L5 must cover (`.input`, `.local`
//! shadowing, variables used only in options, selectors or markup options), NFC
//! ordering, and agreement with the Phase 0 manifest code on the reference
//! workload.
//!
//! `fixtures/workload-1600.p07-manifest.json` was produced once by the P0.7
//! probe's own code (`probes/p0-07-catalog-encoding`: `external_vars`,
//! `markup_names`, `function_names`, and `Manifest::build`'s NFC + sort +
//! dedup) over `bench/corpora/workload-1600.json`: per message with anything
//! to report, its slot names, markup names and functions. The probe is
//! throwaway (task C5 deletes it); the fixture keeps its answer.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mf2_syntax::{Analysis, analyze, parse_model};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

fn names(v: &[mf2_syntax::Name<'_>]) -> Vec<String> {
    v.iter().map(|n| n.nfc.to_string()).collect()
}

fn spellings(v: &[mf2_syntax::Name<'_>]) -> Vec<String> {
    v.iter().map(|n| n.spelling.to_owned()).collect()
}

fn analysis(src: &str) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let parsed = parse_model(src);
    let m = parsed.message.expect("parses");
    let Analysis {
        externals,
        locals,
        markup,
        functions,
        ..
    } = analyze(&m);
    (
        names(&externals),
        names(&locals),
        names(&markup),
        names(&functions),
    )
}

fn v(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn input_declarations_are_external() {
    let (ext, loc, _, fun) = analysis(".input {$n :number} {{{$n}}}");
    assert_eq!((ext, loc, fun), (v(&["n"]), v(&[]), v(&["number"])));
    // An input that the pattern never uses is still an argument.
    let (ext, _, _, _) = analysis(".input {$gender :string} {{Hello}}");
    assert_eq!(ext, v(&["gender"]));
}

#[test]
fn locals_shadow_and_are_not_external() {
    // syntax.json #94: `.local $foo = {$bar}` — bar is external, foo local.
    let (ext, loc, _, _) = analysis(".local $foo = {$bar} {{bar {$foo}}}");
    assert_eq!((ext, loc), (v(&["bar"]), v(&["foo"])));
    // syntax.json #95: a chain of locals.
    let (ext, loc, _, _) = analysis(".local $foo = {$baz} .local $bar = {$foo} {{bar {$bar}}}");
    assert_eq!((ext, loc), (v(&["baz"]), v(&["foo", "bar"])));
    // syntax.json #99: literal locals need no argument.
    let (ext, loc, _, _) = analysis(".local $x = {42} .local $y = {$x} {{{$x} {$y}}}");
    assert_eq!((ext, loc), (v(&[]), v(&["x", "y"])));
    // A local may overwrite an external name that no earlier declaration uses.
    let (ext, loc, _, _) = analysis(".local $x = {1} {{{$x}}}");
    assert_eq!((ext, loc), (v(&[]), v(&["x"])));
}

#[test]
fn option_selector_and_markup_option_variables_are_external() {
    // functions/number.json #24: a variable only in an option.
    let (ext, _, _, fun) = analysis("hello {4.2 :number minimumFractionDigits=$foo}");
    assert_eq!((ext, fun), (v(&["foo"]), v(&["number"])));
    // syntax.json #104: variables only in markup options.
    let (ext, _, mk, _) = analysis("{#tag a:foo=|foo| b:bar=$bar}");
    assert_eq!((ext, mk), (v(&["bar"]), v(&["tag"])));
    // A selector.
    let (ext, loc, _, _) = analysis(
        ".input {$x :test:select} .local $y = {$x} .match $y 1.0 {{1.0}} 1 {{1}} * {{other}}",
    );
    assert_eq!((ext, loc), (v(&["x"]), v(&["y"])));
    // Options in declarations.
    let (ext, _, _, fun) =
        analysis(".local $n = {42 :number minimumFractionDigits=$d} {{{$n :integer}}}");
    assert_eq!((ext, fun), (v(&["d"]), v(&["integer", "number"])));
}

#[test]
fn names_are_unique_under_nfc_ordered_by_nfc_bytes_and_keep_their_spelling() {
    // syntax.json #110: `$Ḍ̇` written two ways is one variable.
    let src = ".input {$\u{1E0C}\u{307}} {{{$D\u{323}\u{307}}}}";
    let m = parse_model(src).message.expect("parses");
    let a = analyze(&m);
    assert_eq!(names(&a.externals), v(&["\u{1E0C}\u{307}"]));
    assert_eq!(spellings(&a.externals), v(&["\u{1E0C}\u{307}"]));
    let src = "{$D\u{323}\u{307}} {$\u{1E0C}\u{307}}";
    let m = parse_model(src).message.expect("parses");
    let a = analyze(&m);
    assert_eq!(names(&a.externals), v(&["\u{1E0C}\u{307}"]));
    assert_eq!(spellings(&a.externals), v(&["D\u{323}\u{307}"]));
    // Ascending bytewise order of the NFC form (the manifest's slot order).
    let (ext, _, mk, fun) = analysis("{$b :z} {$a :y} {$B} {#q}{/p}{$\u{e9}} {$e\u{301}}");
    assert_eq!(ext, v(&["B", "a", "b", "\u{e9}"]));
    assert_eq!(mk, v(&["p", "q"]));
    assert_eq!(fun, v(&["y", "z"]));
}

#[test]
fn agrees_with_the_phase_0_manifest_code_on_the_workload() {
    let workload: BTreeMap<String, String> = serde_json::from_str(
        &fs::read_to_string(root().join("bench/corpora/workload-1600.json")).expect("workload"),
    )
    .expect("workload JSON");
    let fixture: BTreeMap<String, BTreeMap<String, Vec<String>>> = serde_json::from_str(
        &fs::read_to_string(root().join("conformance/fixtures/workload-1600.p07-manifest.json"))
            .expect("fixture"),
    )
    .expect("fixture JSON");
    assert_eq!(workload.len(), 1600);
    assert_eq!(fixture.len(), 344);
    let empty = Vec::new();
    for (id, src) in &workload {
        let m = parse_model(src).message.expect("the workload is valid MF2");
        let a = analyze(&m);
        let want = fixture.get(id);
        let field = |f: &str| want.and_then(|w| w.get(f)).unwrap_or(&empty);
        assert_eq!(&names(&a.externals), field("slots"), "{id}: slots");
        assert_eq!(&names(&a.markup), field("markup"), "{id}: markup");
        let mut functions = names(&a.functions);
        functions.sort();
        assert_eq!(&functions, field("functions"), "{id}: functions");
    }
}
