//! What a select costs, piece by piece (A11 attribution for
//! `B10-P3.md`): the same short text with an unannotated integer, an
//! `:integer` placeholder, an `.input` declaration, and selects with and
//! without a placeholder, in `en` and `pl` (Int 3, reused `String`, best of
//! 15 × 200,000). Timings on a machine whose clock drifts: compare builds by
//! alternating their binaries, not one run after another.
//!
//! ```sh
//! cargo run --release -p runtime-bench --example select_cost
//! ```

use mf2::{Arg, Compiled, FormatContext, Formatter, Function, NoErrors, Registry, functions};
use std::hint::black_box;
use std::time::Instant;
static F: [(&str, &dyn Function); 2] = [
    ("integer", &functions::INTEGER),
    ("number", &functions::NUMBER),
];
static R: Registry = Registry::new(&F);
static CX: FormatContext = FormatContext::new(&mf2::host_std::HOST);
#[allow(clippy::cast_precision_loss)]
fn time(label: &str, src: &str, locale: &str) {
    let compiled: Compiled = mf2::compile_str(src, locale).expect("the message compiles");
    let formatter = Formatter::new(&compiled.catalog, &R, &CX);
    let mut out = String::with_capacity(256);
    let args = [Arg::Int(3)];
    let mut best = f64::MAX;
    let calls: u32 = 200_000;
    for _ in 0..15 {
        let start = Instant::now();
        for _ in 0..calls {
            out.clear();
            formatter.write(
                black_box(Compiled::ID),
                black_box(&args),
                &mut out,
                &mut NoErrors,
            );
        }
        best = best.min(start.elapsed().as_nanos() as f64 / f64::from(calls));
    }
    println!("{best:7.1} ns  {label:<40} -> {out:?}");
}

#[allow(clippy::cast_precision_loss)]
fn main() {
    time("text only", "Some words here and there.", "en");
    time("{$n} unannotated int", "Some {$n} words.", "en");
    time("{$n :integer}", "Some {$n :integer} words.", "en");
    time("{$n :number}", "Some {$n :number} words.", "en");
    time(
        ".input :integer, pattern {$n}",
        ".input {$n :integer} {{Some {$n} words.}}",
        "en",
    );
    time(
        "select, no placeholder",
        ".input {$n :integer} .match $n one {{One word.}} * {{Some words.}}",
        "en",
    );
    time(
        "select + placeholder",
        ".input {$n :integer} .match $n one {{One {$n} word.}} * {{Some {$n} words.}}",
        "en",
    );
    time(
        "select exact key 3",
        ".input {$n :integer} .match $n 3 {{Three.}} one {{One.}} * {{Some.}}",
        "en",
    );
    time(
        "select + placeholder, pl",
        ".input {$n :integer} .match $n one {{One {$n} word.}} few {{F {$n}}} many {{M {$n}}} * {{Some {$n} words.}}",
        "pl",
    );
}
