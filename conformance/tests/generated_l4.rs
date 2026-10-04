//! Layer L4 on grammar-driven input (plans/01-conformance.md §5;
//! plans/10-phase-3-work-order.md A10): random messages generated from the
//! vendored `message.abnf`, steered towards the runtime's functions and
//! options, compiled for a random locale with generated arguments
//! ([`mf2_conformance::l4gen`]), formatted by `mf2-l4-runner` — the code
//! `cargo xtask l4-wasi --generated` runs on `wasm32-wasip1`:
//!
//! * formatting never panics — valid messages and messages with data-model
//!   errors alike (the writer accepts both);
//! * named and positional arguments give the same output, and the parts
//!   concatenate to the string (checked inside `mf2_l4_runner::run`);
//! * the output is deterministic (a second run gives the same record);
//! * the stripped catalog formats exactly as the unstripped one;
//! * a catalog whose `icu.blob` is sliced to the message (02 §4.4) formats
//!   exactly as one whose blob has every shape's data.
//!
//! `cargo test` runs a bounded number of cases; set `MF2_GEN_CASES` (and
//! optionally `MF2_GEN_SEED`) for longer runs, e.g.
//! `MF2_GEN_CASES=1000000 cargo test --release -p mf2-conformance --test generated_l4`.
//! The seed is `generated.rs`'s, so case `n` starts from the same message.

use std::collections::BTreeSet;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use mf2_conformance::abnf::Grammar;
use mf2_conformance::l4gen;
use mf2_conformance::spec::{ABNF, read_spec};
use mf2_l4_runner::{Case, Record};

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

/// Runs `case`, turning a panic or a runner error into a report naming the
/// case and its message.
fn run(case: &Case, n: u64, source: &str) -> Record {
    match catch_unwind(AssertUnwindSafe(|| mf2_l4_runner::run(case))) {
        Ok(Ok(record)) => record,
        Ok(Err(e)) => panic!("case {n} ({}): {e}\n  message {source:?}", case.id),
        Err(payload) => {
            let why = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("(no message)");
            panic!(
                "case {n} ({}): formatting panicked: {why}\n  message {source:?}",
                case.id
            )
        }
    }
}

/// What the cases of one thread reached.
#[derive(Default)]
struct Counts {
    valid: u64,
    clean: u64,
    numeric: u64,
    selects: u64,
    formatted: u64,
    // Phase 4 (A9): the localized and date/time families.
    percent: u64,
    measure: u64,
    dates: u64,
    dated: u64,
    sliced: u64,
    kinds: BTreeSet<String>,
}

impl Counts {
    fn add(&mut self, other: Counts) {
        self.valid += other.valid;
        self.clean += other.clean;
        self.numeric += other.numeric;
        self.selects += other.selects;
        self.formatted += other.formatted;
        self.percent += other.percent;
        self.measure += other.measure;
        self.dates += other.dates;
        self.dated += other.dated;
        self.sliced += other.sliced;
        self.kinds.extend(other.kinds);
    }
}

/// Runs the cases of `range` and counts what they reached.
fn run_cases(g: &Grammar, base: u64, range: Range<u64>) -> Counts {
    let mut c = Counts::default();
    for case in range {
        let generated = match l4gen::case(g, base.wrapping_add(case)) {
            Ok(x) => x,
            Err(e) => panic!("case {case}: {e}"),
        };
        let src = &generated.source;
        let first = run(&generated.unstripped, case, src);
        let again = run(&generated.unstripped, case, src);
        assert_eq!(
            first.line(),
            again.line(),
            "case {case}: formatting is not deterministic on {src:?}"
        );
        let stripped = run(&generated.stripped, case, src);
        assert_eq!(
            first.line(),
            stripped.line(),
            "case {case}: the stripped catalog formats differently on {src:?}"
        );
        if let Some(all) = &generated.all_dates {
            let every = run(all, case, src);
            assert_eq!(
                first.line(),
                every.line(),
                "case {case}: icu.blob sliced to the message formats differently from one with \
                 every shape's data on {src:?}"
            );
            c.sliced += 1;
        }
        c.valid += u64::from(generated.valid);
        c.clean += u64::from(first.errors.is_empty());
        c.numeric += u64::from(src.contains(":number") || src.contains(":integer"));
        c.selects += u64::from(src.starts_with('.') && src.contains(".match"));
        // A number or a selection that went through a core handler.
        c.formatted += u64::from(first.parts.contains("\"type\":\"number\""));
        c.percent += u64::from(src.contains(":percent"));
        c.measure += u64::from(src.contains(":currency") || src.contains(":unit"));
        c.dates += u64::from(src.contains(":date") || src.contains(":time"));
        c.dated += u64::from(first.parts.contains("\"type\":\"datetime\""));
        c.kinds.extend(first.errors);
    }
    c
}

#[test]
fn generated_messages_format_from_a_catalog() {
    let g = grammar();
    let base = seed();
    let n = cases();
    // The cases run on one thread per core (`mf2_conformance::parallel`);
    // case `n` is the same message whichever thread runs it.
    let mut total = Counts::default();
    for c in mf2_conformance::parallel::cases(n, |range| run_cases(&g, base, range)) {
        total.add(c);
    }
    let Counts {
        valid,
        clean,
        numeric,
        selects,
        formatted,
        percent,
        measure,
        dates,
        dated,
        sliced,
        kinds,
    } = total;
    eprintln!(
        "generated_l4: {n} cases: {valid} valid, {clean} formatted without errors, \
         {numeric} calling :number or :integer, {formatted} with a formatted number, \
         {selects} selections, {percent} calling :percent, {measure} :currency or :unit, \
         {dates} a date/time function, {dated} with a formatted date/time, {sliced} with \
         icu.blob (sliced = every shape); errors reached: {kinds:?}"
    );
    // The steering reaches the handlers: valid messages, messages formatted
    // without errors, numbers formatted, selections.
    assert!(valid * 4 > n, "only {valid} valid messages");
    assert!(clean * 10 > n, "only {clean} messages without errors");
    assert!(numeric * 4 > n, "only {numeric} messages call :number");
    assert!(formatted * 20 > n, "only {formatted} numbers formatted");
    assert!(selects * 10 > n, "only {selects} selection messages");
    assert!(percent * 50 > n, "only {percent} messages call :percent");
    assert!(
        dates * 20 > n,
        "only {dates} messages call a date/time function"
    );
    assert!(dated * 100 > n, "only {dated} dates formatted");
    for kind in [
        "bad-operand",
        "bad-option",
        "bad-selector",
        "unknown-function",
        "unresolved-variable",
    ] {
        assert!(kinds.contains(kind), "no {kind} error reached");
    }
}
