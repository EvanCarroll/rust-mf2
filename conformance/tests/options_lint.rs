//! `unknown-option`'s lists against the formatter (Phase 9 A2).
//!
//! `mf2_build::OPTIONS` says which options each built-in function defines,
//! so that `mf2 check` can warn about one it does not — which MF2 ignores
//! silently. A list that drifted from the functions would either warn about
//! an option that works or stay quiet about one that does nothing, so this
//! holds every list against what a formatter *reads*: given a value no
//! option can take, an option the function defines is reported (Bad Option,
//! or Unsupported Operation for `usage`), and one it does not is ignored.
//!
//! Every name any list has is tried on every function, with two that no
//! function has (one borrowed from `Intl`), in both registries an
//! application can have: the core functions alone, and every feature on.

use std::collections::BTreeSet;

use mf2::{Compiled, FormatContext, Formatter, Function, Registry, functions};
use mf2_build::OPTIONS;

/// The core functions, without `fn-number`.
static CORE_FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &functions::INTEGER),
    ("number", &functions::NUMBER),
    ("offset", &functions::OFFSET),
    ("string", &functions::STRING),
];
static CORE: Registry = Registry::new(&CORE_FUNCTIONS);
static CX: FormatContext = FormatContext::new(&mf2::host_std::HOST);

/// The operand, and the option a function needs before any other is tried:
/// the first of these that is not the option under test (`None`: none).
fn base(function: &str) -> (&'static str, &'static [&'static str]) {
    match function {
        "datetime" | "date" | "time" => ("|2026-01-01T09:30:00|", &[""]),
        "string" => ("|text|", &[""]),
        "currency" => ("|1|", &["currency=EUR", ""]),
        "unit" => ("|1|", &["unit=meter", ""]),
        // `:offset` takes exactly one of the two.
        "offset" => ("|1|", &["add=1", "subtract=1"]),
        _ => ("|1|", &[""]),
    }
}

/// How many errors formatting `source` reports.
fn errors(source: &str, registry: &Registry) -> usize {
    let compiled = mf2::compile_str(source, "en").unwrap_or_else(|e| panic!("{source}: {e}"));
    let formatter = Formatter::new(&compiled.catalog, registry, &CX);
    let mut out = String::new();
    let mut errors = Vec::new();
    formatter.write(Compiled::ID, &[], &mut out, &mut errors);
    errors.len()
}

/// Every function `registry` has, against every option name.
fn check(registry: &Registry, functions: &[&str]) -> Vec<String> {
    let mut names: BTreeSet<&str> = OPTIONS
        .iter()
        .flat_map(|(_, o)| o.iter().copied())
        .collect();
    names.extend(["dateStyle", "notAnOption"]);
    let mut wrong = Vec::new();
    for &(function, defined) in &OPTIONS {
        if !functions.contains(&function) {
            continue;
        }
        let (operand, bases) = base(function);
        for &name in &names {
            // A base option is replaced, not repeated: a duplicate option is
            // a data-model error, not what this measures.
            let prefix = format!("{name}=");
            let base = bases
                .iter()
                .find(|b| !b.starts_with(&prefix))
                .copied()
                .unwrap_or("");
            let without = errors(&format!("{{{operand} :{function} {base}}}"), registry);
            let with = errors(
                &format!("{{{operand} :{function} {base} {name}=|~nonsense~|}}"),
                registry,
            );
            let reads = with > without;
            if reads != defined.contains(&name) {
                wrong.push(format!(
                    ":{function} {name}: the list says {}, the formatter {}",
                    if defined.contains(&name) {
                        "defined"
                    } else {
                        "unknown"
                    },
                    if reads { "reads it" } else { "ignores it" },
                ));
            }
        }
    }
    wrong
}

#[test]
fn every_list_is_what_the_core_functions_read() {
    let wrong = check(&CORE, &["string", "number", "integer", "offset"]);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn every_list_is_what_every_function_reads() {
    let all: Vec<&str> = OPTIONS.iter().map(|(f, _)| *f).collect();
    let wrong = check(&mf2_l4_runner::REGISTRY, &all);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn the_lists_cover_every_built_in() {
    let listed: Vec<&str> = OPTIONS.iter().map(|(f, _)| *f).collect();
    let builtins: Vec<&str> = mf2_build::BUILTINS.iter().map(|(f, _)| *f).collect();
    assert_eq!(listed, builtins);
}
