//! Linear time on adversarial input (the parser never
//! recurses on its input, and runs in linear time).
//!
//! Each input is large (hundreds of KB) and built to provoke backtracking,
//! re-scanning or cascading recovery. A linear parser handles each in
//! milliseconds; a quadratic one would need minutes to hours — so the bound
//! is loose enough to hold on a loaded machine and in a debug build, and
//! still separates the two by orders of magnitude. The fuzz target checks a
//! per-byte time budget on every input it tries.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

const N: usize = 200_000;

fn chain_of_locals(n: usize) -> String {
    let mut src = String::from(".input {$a0 :f} ");
    for i in 1..n {
        let _ = write!(src, ".local $a{i} = {{$a{}}} ", i - 1);
    }
    let _ = write!(src, ".match $a{} * {{{{}}}}", n - 1);
    src
}

fn inputs() -> Vec<(&'static str, String)> {
    vec![
        ("open braces", "{".repeat(N)),
        ("close braces", "}".repeat(N)),
        ("double open braces", "{{".repeat(N / 2)),
        ("pipes", "|".repeat(N)),
        ("backslashes", "\\".repeat(N)),
        ("dots", ".".repeat(N)),
        ("unknown keywords", ".a ".repeat(N / 3)),
        ("locals without body", ".local $x = {1} ".repeat(N / 16)),
        ("bidi marks", "\u{200e}".repeat(N / 3)),
        (
            "bidi after a name",
            format!("{{$x{} }}", "\u{200e}".repeat(N / 3)),
        ),
        (
            "bidi before a colon",
            format!("{{:ns{}", "\u{200e}".repeat(N / 3)),
        ),
        (
            "whitespace in an expression",
            format!("{{{}", " ".repeat(N)),
        ),
        ("options", format!("{{:f {}}}", "a=b ".repeat(N / 4))),
        (
            "options without values",
            format!("{{:f {}}}", "a ".repeat(N / 2)),
        ),
        (
            "attributes without space",
            format!("{{a {}}}", "@b".repeat(N / 2)),
        ),
        ("placeholders", "{a}".repeat(N / 3)),
        ("broken placeholders", "{a b".repeat(N / 4)),
        (
            "keys",
            format!(".input {{$x :f}} .match $x {} {{{{}}}}", "a ".repeat(N / 2)),
        ),
        (
            "variants",
            format!(".input {{$x :f}} .match $x {}", "a {{}} ".repeat(N / 7)),
        ),
        (
            "stars",
            format!(".input {{$x :f}} .match $x {}", "*".repeat(N)),
        ),
        (
            "selectors",
            format!(".match {} * {{{{}}}}", "$x ".repeat(N / 3)),
        ),
        ("text", "plain text ".repeat(N / 11)),
        ("quoted literal", format!("{{|{}|}}", "a\\|".repeat(N / 3))),
        ("unterminated literals", "{|".repeat(N / 2)),
        ("nul", "\0".repeat(N)),
        // Valid messages with many errors: validation and span resolution
        // must not be quadratic either.
        (
            "redeclarations",
            format!("{}{{{{}}}}", ".local $x = {1} ".repeat(N / 16)),
        ),
        ("chain of locals", chain_of_locals(N / 24)),
        (
            "duplicate variants",
            format!(
                ".input {{$x :f}} .match $x {}* {{{{}}}}",
                "a {{}} ".repeat(N / 7)
            ),
        ),
        (
            "duplicate markup options",
            format!("{{#m {}}}", "a=b ".repeat(N / 4)),
        ),
    ]
}

#[test]
fn adversarial_inputs_parse_in_linear_time() {
    for (what, src) in inputs() {
        let start = Instant::now();
        let cst = mf2_syntax::parse_cst(&src);
        assert_eq!(cst.to_string().len(), src.len(), "{what}");
        let parsed = mf2_syntax::parse_model(&src);
        if let Some(m) = &parsed.message {
            let _ = mf2_syntax::serialize(m);
            let _ = mf2_syntax::analyze(m);
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(5),
            "{what}: {elapsed:?} for {} bytes",
            src.len()
        );
    }
}
