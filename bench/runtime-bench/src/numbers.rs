//! The numeric A/B (plans/10-phase-3-work-order.md A5b, decision D15) and
//! P0.5's ECMA-402 differential, re-run on the runtime (A5).
//!
//! The corpus is P0.5's (`probes/p0-05-numbers/suite/src/bin/ecma_cases.rs`
//! in the first commit): the same xorshift64* generator and seed, so its
//! 100,000 cases are the ones P0.5 measured. Each case is formatted **through
//! the runtime, from a compiled catalog** — `{|v| :number …}` for the display
//! and its errors, a `.match` over the display itself, `0`, `1` and the
//! plural keywords (exact-match serialization and the plural operands of the
//! formatted number), and the same with `select=ordinal`.
//!
//! One line per case; the binary built with and without feature
//! `fixed-decimal` must print the same lines.

use std::fmt::Write as _;
use std::time::Instant;

use catalog_bench::alloc;
use mf2::{Arg, Compiled, FormatContext, FormatError, Formatter, Function, Registry, functions};
use serde_json::{Map, Value, json};

static FUNCTIONS: [(&str, &dyn Function); 1] = [("number", &functions::NUMBER)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// The localized registry (`fn-number`) of the panel differential.
static LOCALIZED_FUNCTIONS: [(&str, &dyn Function); 2] = [
    ("number", &mf2::fn_number::NUMBER),
    ("percent", &mf2::fn_number::PERCENT),
];
static LOCALIZED: Registry = Registry::new(&LOCALIZED_FUNCTIONS);

/// The locale panel of plans/01-conformance.md §5, and two tags with a
/// non-Latin numbering system.
const PANEL: [&str; 13] = [
    "en",
    "es",
    "de",
    "fr",
    "ar",
    "he",
    "ja",
    "hi",
    "ru",
    "pl",
    "cy",
    "ar-EG",
    "hi-u-nu-deva",
];
static CX: FormatContext = {
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.bidi = mf2::BidiStrategy::None;
    cx
};

/// P0.5's generator: xorshift64*.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn index(&mut self, n: usize) -> usize {
        usize::try_from(self.below(n as u64)).unwrap_or(0)
    }
}

/// A random `number-literal` (P0.5's `value`).
fn value(r: &mut Rng) -> String {
    const EDGE: [&str; 16] = [
        "0",
        "-0",
        "1",
        "0.5",
        "1.5",
        "2.5",
        "-2.5",
        "0.05",
        "9.995",
        "999.9995",
        "0.000123456",
        "123456789.987654321",
        "1e-7",
        "-0.0004",
        "5",
        "99.5",
    ];
    if r.below(5) == 0 {
        return EDGE[r.index(EDGE.len())].to_owned();
    }
    let len = 1 + r.index(10);
    let mut digits: String = (0..len)
        .map(|_| char::from(b'0' + u8::try_from(r.below(10)).unwrap_or(0)))
        .collect();
    let point = r.index(len + 4);
    if point > 0 && point < len {
        digits.insert(point, '.');
    } else if point >= len {
        digits = format!("0.{}{}", "0".repeat(point - len), digits);
    }
    let mut s = digits.trim_start_matches('0').to_owned();
    if s.is_empty() || s.starts_with('.') {
        s.insert(0, '0');
    }
    if r.below(3) == 0 { format!("-{s}") } else { s }
}

/// One case: a value and its options, in P0.5's order.
pub(crate) struct Case {
    pub(crate) value: String,
    pub(crate) options: Vec<(String, String)>,
}

/// P0.5's corpus: `n` cases from its seed.
pub(crate) fn corpus(n: usize) -> Vec<Case> {
    const MODES: [&str; 9] = [
        "ceil",
        "floor",
        "expand",
        "trunc",
        "halfCeil",
        "halfFloor",
        "halfExpand",
        "halfTrunc",
        "halfEven",
    ];
    const SIGNS: [&str; 5] = ["auto", "always", "exceptZero", "negative", "never"];
    const PRIO: [&str; 3] = ["auto", "morePrecision", "lessPrecision"];
    const INCS: [u16; 15] = [
        1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000,
    ];
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut cases = Vec::with_capacity(n);
    for _ in 0..n {
        let v = value(&mut r);
        let mut o: Vec<(String, String)> = Vec::new();
        let mut add = |k: &str, val: String| o.push((k.to_owned(), val));
        if r.below(4) == 0 {
            add("minimumIntegerDigits", (1 + r.below(5)).to_string());
        }
        let inc = if r.below(6) == 0 {
            INCS[r.index(15)]
        } else {
            1
        };
        if inc == 1 {
            if r.below(3) == 0 {
                add("minimumFractionDigits", r.below(5).to_string());
            }
            if r.below(3) == 0 {
                add("maximumFractionDigits", r.below(7).to_string());
            }
            if r.below(3) == 0 {
                add("minimumSignificantDigits", (1 + r.below(6)).to_string());
            }
            if r.below(3) == 0 {
                add("maximumSignificantDigits", (1 + r.below(8)).to_string());
            }
            if r.below(3) == 0 {
                add("roundingPriority", PRIO[r.index(3)].to_owned());
            }
        } else {
            let fd = r.below(4).to_string();
            add("minimumFractionDigits", fd.clone());
            add("maximumFractionDigits", fd);
            add("roundingIncrement", inc.to_string());
        }
        if r.below(2) == 0 {
            add("roundingMode", MODES[r.index(9)].to_owned());
        }
        if r.below(3) == 0 {
            add("signDisplay", SIGNS[r.index(5)].to_owned());
        }
        if r.below(4) == 0 {
            add("trailingZeroDisplay", "stripIfInteger".to_owned());
        }
        cases.push(Case {
            value: v,
            options: o,
        });
    }
    cases
}

fn options_src(options: &[(String, String)]) -> String {
    let mut s = String::new();
    for (k, v) in options {
        let _ = write!(s, " {k}=|{v}|");
    }
    s
}

/// A literal's source form (`|…|`, escaping `\` and `|`).
fn quoted(v: &str) -> String {
    let mut s = String::from("|");
    for c in v.chars() {
        if c == '\\' || c == '|' {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('|');
    s
}

fn compile(src: &str) -> Compiled {
    match mf2::compile_str(src, "en") {
        Ok(c) => c,
        Err(e) => panic!("{src}: {e} {:?}", e.kinds()),
    }
}

fn format(c: &Compiled) -> (String, Vec<FormatError>) {
    let f = Formatter::new(&c.catalog, &REGISTRY, &CX);
    let mut out = String::new();
    let mut errs = Vec::new();
    f.write(Compiled::ID, &[] as &[Arg<'_>], &mut out, &mut errs);
    (out, errs)
}

fn error_names(errs: &[FormatError]) -> String {
    errs.iter()
        .filter_map(|e| e.kind())
        .map(mf2::ErrorKind::suite_name)
        .collect::<Vec<_>>()
        .join(",")
}

/// The source of the display message of a case.
pub(crate) fn display_source(c: &Case) -> String {
    format!(
        "{{{} :number{}}}",
        quoted(&c.value),
        options_src(&c.options)
    )
}

/// One case's line: value, options, display, its errors, the cardinal and
/// the ordinal selection (with their errors).
pub(crate) fn line(c: &Case) -> String {
    let opts = options_src(&c.options);
    let (display, errs) = format(&compile(&display_source(c)));
    // The display itself, 0 and 1 as exact keys, each once (by value).
    let mut keys = vec![display.clone(), "0".to_owned(), "1".to_owned()];
    keys.sort();
    keys.dedup();
    let mut variants = String::new();
    for k in &keys {
        let k = quoted(k);
        let _ = write!(variants, " {k} {{{{={k}}}}}");
    }
    let cardinal = format!(
        ".local $n = {{{} :number{opts}}} .match $n{variants} zero {{{{zero}}}} one {{{{one}}}} two {{{{two}}}} few {{{{few}}}} many {{{{many}}}} * {{{{other}}}}",
        quoted(&c.value)
    );
    let (card, card_errs) = format(&compile(&cardinal));
    let ordinal = format!(
        ".local $n = {{{} :number{opts} select=ordinal}} .match $n one {{{{one}}}} two {{{{two}}}} few {{{{few}}}} * {{{{other}}}}",
        quoted(&c.value)
    );
    let (ord, ord_errs) = format(&compile(&ordinal));
    format!(
        "{}\t{}\t{display}\t{}\t{card}\t{}\t{ord}\t{}",
        c.value,
        opts.trim_start(),
        error_names(&errs),
        error_names(&card_errs),
        error_names(&ord_errs),
    )
}

/// One case as P0.5's JSON line for `ecma-diff.cjs`: the display and the
/// number of errors.
pub(crate) fn ecma_json(c: &Case) -> String {
    let (display, errs) = format(&compile(&display_source(c)));
    let obj: Map<String, Value> = c
        .options
        .iter()
        .map(|(k, v)| {
            let jv = v.parse::<u64>().map_or_else(|_| json!(v), |n| json!(n));
            (k.clone(), jv)
        })
        .collect();
    json!({"v": c.value, "o": obj, "out": display, "errs": errs.len()}).to_string()
}

/// Formatting speed and allocations over the first `n` cases' display
/// messages (catalogs compiled beforehand): `(ns per format, allocations
/// per format, bytes per format)`, median of `runs` passes.
#[allow(clippy::cast_precision_loss)] // counts far below 2^52
pub(crate) fn speed(cases: &[Case], runs: usize) -> (f64, f64, f64) {
    let compiled: Vec<Compiled> = cases.iter().map(|c| compile(&display_source(c))).collect();
    let formatters: Vec<Formatter<'_>> = compiled
        .iter()
        .map(|c| Formatter::new(&c.catalog, &REGISTRY, &CX))
        .collect();
    let mut out = String::with_capacity(256);
    let mut errs: Vec<FormatError> = Vec::with_capacity(16);
    let pass = |out: &mut String, errs: &mut Vec<FormatError>| {
        for f in &formatters {
            out.clear();
            errs.clear();
            f.write(Compiled::ID, &[], out, errs);
        }
    };
    pass(&mut out, &mut errs);
    let ((), counts) = alloc::count(|| pass(&mut out, &mut errs));
    let mut samples: Vec<f64> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            pass(&mut out, &mut errs);
            t.elapsed().as_nanos() as f64 / formatters.len() as f64
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    let median = samples.get(samples.len() / 2).copied().unwrap_or(0.0);
    let n = formatters.len() as f64;
    (median, counts.allocs as f64 / n, counts.bytes as f64 / n)
}

/// Case `i` localized: its locale from the panel (in turn), `:percent` for
/// every fourth case (without the options `:percent` does not take), a
/// `useGrouping` value in turn; the JSON line for `loc-diff.cjs`.
pub(crate) fn locale_json(i: usize, c: &Case) -> String {
    const GROUPING: [&str; 4] = ["auto", "always", "min2", "never"];
    let locale = PANEL[i % PANEL.len()];
    let percent = i % 4 == 3;
    let mut options: Vec<(String, String)> = c
        .options
        .iter()
        .filter(|(k, _)| !(percent && (k == "minimumIntegerDigits" || k == "roundingIncrement")))
        .cloned()
        .collect();
    options.push(("useGrouping".to_owned(), GROUPING[(i / 13) % 4].to_owned()));
    let function = if percent { "percent" } else { "number" };
    let src = format!(
        "{{{} :{function}{}}}",
        quoted(&c.value),
        options_src(&options)
    );
    let compiled = match mf2::compile_str(&src, locale) {
        Ok(m) => m,
        Err(e) => panic!("{src}: {e} {:?}", e.kinds()),
    };
    let f = Formatter::new(&compiled.catalog, &LOCALIZED, &CX);
    let mut out = String::new();
    let mut errs = Vec::new();
    f.write(Compiled::ID, &[] as &[Arg<'_>], &mut out, &mut errs);
    let obj: Map<String, Value> = options
        .iter()
        .map(|(k, v)| {
            let jv = v.parse::<u64>().map_or_else(|_| json!(v), |n| json!(n));
            (k.clone(), jv)
        })
        .collect();
    json!({"l": locale, "f": function, "v": c.value, "o": obj, "out": out, "errs": errs.len()})
        .to_string()
}
