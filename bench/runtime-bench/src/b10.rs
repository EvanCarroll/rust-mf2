//! B10 (plans/10-phase-3-work-order.md A11): formatting speed natively, in
//! the parser gate's style — one process, rows interleaved round by round,
//! medians, the counting allocator — on the four production catalogs of the
//! reference workload (stripped; `catalog_bench::corpus`), with the default
//! context (Default Bidi Strategy, `mf2-host-std`) and the release client's
//! error policy (`NoErrors`).
//!
//! | row | one operation |
//! |---|---|
//! | `simple-ref` | `Formatter::simple`: `get` + `text` (P0.8's "simple lookup") |
//! | `simple` | `Formatter::write` of a simple message into a reused `String` |
//! | `pattern-1` | `write` of a pattern message with one argument (`Arg::Str`), reused `String` (P0.8's "1-arg pattern, reused `String`") |
//! | `pattern-1-owned` | the same into a new `String::with_capacity(128)` (P0.8's owned row) |
//! | `select` | `write` of a `.match` message, integer arguments `0, 1, 2, 3, 5, 11, 21, 100` by message, reused `String` |
//! | `resolve-fn` | per-call function resolution: `Catalog::function(i)` + `Registry::get`, every FUNCS entry |
//! | `resolve-fn-table` | the alternative, a load-time table: one indexed read |
//!
//! Every sweep covers every message of its class in id order. Allocation
//! counts are exact (one counted sweep).

use std::fmt::Write as _;
use std::hint::black_box;
use std::time::{Duration, Instant};

use catalog_bench::alloc::{self, Counts};
use catalog_bench::corpus::{self, Built};
use catalog_bench::report::{Build, cpu_mhz, load_average};
use catalog_bench::stats::Spread;
use mf2_catalog::{Catalog, Entry, MsgId};
use mf2_runtime::{Arg, FormatContext, Formatter, Function, NoErrors, Registry, functions};
use serde::Serialize;

use crate::error::{Error, Result};

static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &functions::INTEGER),
    ("number", &functions::NUMBER),
    ("offset", &functions::OFFSET),
    ("string", &functions::STRING),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static CX: FormatContext = FormatContext::new(&mf2::host_std::HOST);

/// B10: a simple message ≤ 100 ns, 0 allocations.
pub(crate) const SIMPLE_NS: f64 = 100.0;
/// B10: a 1-argument pattern ≤ 500 ns, ≤ 1 allocation.
pub(crate) const PATTERN_NS: f64 = 500.0;
/// P0.8's native figures (`en`, P0.3's runtime, under load): simple lookup,
/// 1-argument pattern (reused and owned `String`), select. Phase 3 must not
/// lose them: within [`P08_FACTOR`] (`catalog-bench`'s rule for P0.8's
/// figures, whose runs varied up to 2× under load).
const P08: [(Op, f64); 4] = [
    (Op::SimpleRef, 20.7),
    (Op::Pattern1, 93.5),
    (Op::Pattern1Owned, 106.3),
    (Op::Select, 317.0),
];
/// How far a row may be from P0.8's figure.
const P08_FACTOR: f64 = catalog_bench::bench::P08_FACTOR;
/// The argument of the 1-argument patterns.
const NAME: &str = "Ada Lovelace";
/// Integer arguments of the select messages, by message.
const INTS: [i64; 8] = [0, 1, 2, 3, 5, 11, 21, 100];

/// Measurement knobs.
#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct Settings {
    pub(crate) runs: usize,
    #[serde(serialize_with = "millis")]
    pub(crate) min_sample: Duration,
    #[serde(serialize_with = "millis")]
    pub(crate) warmup: Duration,
    /// Only this row (for profiling); the gate then checks what ran.
    pub(crate) only: Option<Op>,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's signature
fn millis<S: serde::Serializer>(d: &Duration, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_u128(d.as_millis())
}

/// One row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Op {
    SimpleRef,
    Simple,
    Pattern1,
    Pattern1Owned,
    Select,
    ResolveFn,
    ResolveFnTable,
}

impl Op {
    const ALL: [Op; 7] = [
        Op::SimpleRef,
        Op::Simple,
        Op::Pattern1,
        Op::Pattern1Owned,
        Op::Select,
        Op::ResolveFn,
        Op::ResolveFnTable,
    ];

    const fn label(self) -> &'static str {
        match self {
            Op::SimpleRef => "simple: `Formatter::simple` (`get` + `text`)",
            Op::Simple => "simple: `write`, reused `String`",
            Op::Pattern1 => "1-argument pattern: `write`, reused `String`",
            Op::Pattern1Owned => "1-argument pattern: `write`, new `String::with_capacity(128)`",
            Op::Select => "select: `write`, reused `String`",
            Op::ResolveFn => "function resolution per call (`function(i)` + `Registry::get`)",
            Op::ResolveFnTable => "function resolution from a load-time table",
        }
    }
}

/// One locale's catalog and message classes.
struct Fixture {
    tag: String,
    catalog: Catalog,
    simple: Vec<MsgId>,
    pattern1: Vec<MsgId>,
    select: Vec<MsgId>,
    /// Per select message, its arguments.
    select_args: Vec<Vec<Arg<'static>>>,
    /// The registry entry of every FUNCS index (the load-time table).
    table: Vec<Option<&'static dyn Function>>,
}

impl Fixture {
    fn new(b: &Built) -> Result<Self> {
        let catalog = Catalog::new(b.stripped.clone(), corpus::MANIFEST_HASH).map_err(|e| {
            Error::Bench(format!(
                "{}: the stripped catalog does not load: {e:?}",
                b.tag
            ))
        })?;
        let (mut simple, mut pattern1, mut select, mut select_args) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for i in 0..catalog.message_count() {
            let Some(id) = MsgId::new(0, i) else { continue };
            match catalog.get(id) {
                Entry::Simple(_) => simple.push(id),
                Entry::Pattern(m) if m.names().external_count() == 1 => pattern1.push(id),
                Entry::Select(m) => {
                    let n = m.names().external_count() as usize;
                    let v = INTS[select.len() % INTS.len()];
                    select.push(id);
                    select_args.push(vec![Arg::Int(v); n]);
                }
                _ => {}
            }
        }
        let table = (0..catalog.function_count())
            .map(|i| catalog.function(i).and_then(|n| REGISTRY.get(n)))
            .collect();
        Ok(Fixture {
            tag: b.tag.clone(),
            catalog,
            simple,
            pattern1,
            select,
            select_args,
            table,
        })
    }

    fn per_pass(&self, op: Op) -> usize {
        match op {
            Op::SimpleRef | Op::Simple => self.simple.len(),
            Op::Pattern1 | Op::Pattern1Owned => self.pattern1.len(),
            Op::Select => self.select.len(),
            Op::ResolveFn | Op::ResolveFnTable => self.table.len(),
        }
    }
}

/// One sweep of `op`; returns a checksum.
fn sweep(fx: &Fixture, f: &Formatter<'_>, op: Op, out: &mut String) -> u64 {
    let mut acc = 0u64;
    match op {
        Op::SimpleRef => {
            for &id in black_box(&fx.simple) {
                acc = acc.wrapping_add(f.simple(black_box(id)).map_or(0, |s| s.len() as u64));
            }
        }
        Op::Simple => {
            for &id in black_box(&fx.simple) {
                out.clear();
                f.write(black_box(id), &[], out, &mut NoErrors);
                acc = acc.wrapping_add(out.len() as u64);
            }
        }
        Op::Pattern1 => {
            let args = [Arg::Str(black_box(NAME))];
            for &id in black_box(&fx.pattern1) {
                out.clear();
                f.write(black_box(id), &args, out, &mut NoErrors);
                acc = acc.wrapping_add(out.len() as u64);
            }
        }
        Op::Pattern1Owned => {
            let args = [Arg::Str(black_box(NAME))];
            for &id in black_box(&fx.pattern1) {
                let mut s = String::with_capacity(128);
                f.write(black_box(id), &args, &mut s, &mut NoErrors);
                acc = acc.wrapping_add(black_box(s).len() as u64);
            }
        }
        Op::Select => {
            for (&id, args) in black_box(&fx.select).iter().zip(&fx.select_args) {
                out.clear();
                f.write(black_box(id), args, out, &mut NoErrors);
                acc = acc.wrapping_add(out.len() as u64);
            }
        }
        Op::ResolveFn => {
            for i in 0..black_box(fx.catalog.function_count()) {
                let h = fx
                    .catalog
                    .function(black_box(i))
                    .and_then(|n| REGISTRY.get(n));
                acc = acc.wrapping_add(u64::from(black_box(h).is_some()));
            }
        }
        Op::ResolveFnTable => {
            for i in 0..black_box(fx.table.len()) {
                let h = fx.table.get(black_box(i)).copied().flatten();
                acc = acc.wrapping_add(u64::from(black_box(h).is_some()));
            }
        }
    }
    acc
}

/// One row's result.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Row {
    pub(crate) locale: String,
    pub(crate) op: Op,
    pub(crate) per_pass: usize,
    pub(crate) passes_per_sample: u64,
    pub(crate) ns: Spread,
    pub(crate) allocs_per_op: f64,
    pub(crate) bytes_per_op: f64,
    pub(crate) checksum: u64,
    pub(crate) p08_ns: Option<f64>,
}

/// A gate rule, evaluated.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Check {
    pub(crate) rule: String,
    pub(crate) pass: bool,
    /// Whether `--gate` fails on it (else reported only).
    pub(crate) gated: bool,
}

/// The report.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Report {
    pub(crate) build: String,
    pub(crate) settings: Settings,
    pub(crate) load_before: Option<[f64; 3]>,
    pub(crate) load_after: Option<[f64; 3]>,
    pub(crate) cpu_mhz: Option<f64>,
    pub(crate) functions: Vec<String>,
    pub(crate) classes: Vec<(String, usize, usize, usize)>,
    pub(crate) rows: Vec<Row>,
    pub(crate) checks: Vec<Check>,
}

#[allow(clippy::cast_precision_loss)] // counts far below 2^52
fn per_op(total: Duration, ops: u64) -> f64 {
    total.as_nanos() as f64 / ops.max(1) as f64
}

/// Runs every row on every locale.
#[allow(clippy::cast_precision_loss)] // counts far below 2^52
pub(crate) fn run(settings: Settings) -> Result<Report> {
    let load_before = load_average();
    let locales = corpus::generate(&catalog_bench::repo_root())
        .map_err(|e| Error::Bench(format!("corpus: {e}")))?;
    let (manifest, built) =
        corpus::build(&locales, false).map_err(|e| Error::Bench(format!("build: {e}")))?;
    let fixtures: Vec<Fixture> = built.iter().map(Fixture::new).collect::<Result<_>>()?;
    let formatters: Vec<Formatter<'_>> = fixtures
        .iter()
        .map(|fx| Formatter::new(&fx.catalog, &REGISTRY, &CX))
        .collect();
    let mut out = String::with_capacity(4096);
    let cells: Vec<(usize, Op)> = (0..fixtures.len())
        .flat_map(|l| Op::ALL.into_iter().map(move |op| (l, op)))
        .filter(|&(_, op)| settings.only.is_none_or(|o| o == op))
        .collect();

    // Calibrate: passes per sample so a sample lasts `min_sample`; warm up.
    let mut passes = Vec::with_capacity(cells.len());
    let mut checksums = Vec::with_capacity(cells.len());
    for &(l, op) in &cells {
        let (fx, f) = (&fixtures[l], &formatters[l]);
        let start = Instant::now();
        let mut n = 0u64;
        let mut check = 0;
        while start.elapsed() < settings.warmup || n == 0 {
            check = sweep(fx, f, op, &mut out);
            n += 1;
        }
        let per = start.elapsed().as_secs_f64() / n as f64;
        let want = settings.min_sample.as_secs_f64() / per.max(1e-9);
        // `want` is small and positive.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        passes.push((want.ceil() as u64).max(1));
        checksums.push(check);
    }
    let mut samples: Vec<Vec<f64>> = vec![Vec::with_capacity(settings.runs); cells.len()];
    for _ in 0..settings.runs {
        for (c, &(l, op)) in cells.iter().enumerate() {
            let (fx, f) = (&fixtures[l], &formatters[l]);
            let start = Instant::now();
            for _ in 0..passes[c] {
                black_box(sweep(fx, f, op, &mut out));
            }
            let t = start.elapsed();
            samples[c].push(per_op(t, passes[c] * fx.per_pass(op) as u64));
        }
    }
    let mut rows = Vec::with_capacity(cells.len());
    for (c, &(l, op)) in cells.iter().enumerate() {
        let (fx, f) = (&fixtures[l], &formatters[l]);
        let (check, counts): (u64, Counts) = alloc::count(|| sweep(fx, f, op, &mut out));
        if check != checksums[c] {
            return Err(Error::Bench(format!(
                "{} {op:?}: the output changed between sweeps",
                fx.tag
            )));
        }
        let n = fx.per_pass(op).max(1) as f64;
        rows.push(Row {
            locale: fx.tag.clone(),
            op,
            per_pass: fx.per_pass(op),
            passes_per_sample: passes[c],
            ns: Spread::of(&mut samples[c]).ok_or_else(|| Error::Bench("no samples".into()))?,
            allocs_per_op: counts.allocs as f64 / n,
            bytes_per_op: counts.bytes as f64 / n,
            checksum: check,
            p08_ns: (l == 0)
                .then(|| P08.iter().find(|(o, _)| *o == op).map(|&(_, ns)| ns))
                .flatten(),
        });
    }
    let checks = gate(&rows);
    Ok(Report {
        build: Build::current().line(),
        settings,
        load_before,
        load_after: load_average(),
        cpu_mhz: cpu_mhz(),
        functions: manifest.functions.clone(),
        classes: fixtures
            .iter()
            .map(|fx| {
                (
                    fx.tag.clone(),
                    fx.simple.len(),
                    fx.pattern1.len(),
                    fx.select.len(),
                )
            })
            .collect(),
        rows,
        checks,
    })
}

fn gate(rows: &[Row]) -> Vec<Check> {
    let row = |op: Op| rows.iter().find(|r| r.locale == "en" && r.op == op);
    let mut checks = Vec::new();
    if let Some(r) = row(Op::Simple) {
        checks.push(Check {
            rule: format!(
                "B10 simple (`en`, `write`): {:.1} ns ≤ {SIMPLE_NS} ns, {:.3} allocations = 0",
                r.ns.median, r.allocs_per_op
            ),
            pass: r.ns.median <= SIMPLE_NS && r.allocs_per_op == 0.0,
            gated: true,
        });
    }
    if let Some(r) = row(Op::Pattern1) {
        checks.push(Check {
            rule: format!(
                "B10 1-argument pattern (`en`, reused `String`): {:.1} ns ≤ {PATTERN_NS} ns, {:.3} allocations ≤ 1",
                r.ns.median, r.allocs_per_op
            ),
            pass: r.ns.median <= PATTERN_NS && r.allocs_per_op <= 1.0,
            gated: true,
        });
    }
    if let Some(r) = row(Op::Pattern1Owned) {
        checks.push(Check {
            rule: format!(
                "B10 1-argument pattern (`en`, new `String`): {:.1} ns ≤ {PATTERN_NS} ns, {:.3} allocations ≤ 1.05 (a result past 128 B grows)",
                r.ns.median, r.allocs_per_op
            ),
            pass: r.ns.median <= PATTERN_NS && r.allocs_per_op <= 1.05,
            gated: true,
        });
    }
    for r in rows.iter().filter(|r| r.locale == "en") {
        if let Some(p) = r.p08_ns {
            checks.push(Check {
                rule: format!(
                    "P0.8 not lost (`en`, {}): {:.1} ns ≤ {P08_FACTOR} × {p} ns",
                    r.op.label(),
                    r.ns.median
                ),
                pass: r.ns.median <= P08_FACTOR * p,
                // B10 has no select row; P0.3's select ran an integer-only
                // `:integer` (see the report's notes).
                gated: r.op != Op::Select,
            });
        }
    }
    checks
}

/// The report as Markdown.
pub(crate) fn markdown(r: &Report) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# B10 — formatting speed, native (Phase 3, A11)\n");
    let _ = writeln!(
        s,
        "Written by `cargo run --release -p runtime-bench -- b10 --md …` (`src/b10.rs`). \
         {}. {} samples per row, each ≥ {} ms, rows interleaved round by round; \
         load average {} before, {} after; CPU {} MHz.\n",
        r.build,
        r.settings.runs,
        r.settings.min_sample.as_millis(),
        load(r.load_before),
        load(r.load_after),
        r.cpu_mhz
            .map_or_else(|| "?".to_owned(), |m| format!("{m:.0}")),
    );
    let _ = writeln!(
        s,
        "Catalogs: the reference workload's four locales, stripped. Messages per \
         class (simple / 1-argument pattern / select): {}. Functions the corpus \
         uses: {}.\n",
        r.classes
            .iter()
            .map(|(t, a, b, c)| format!("{t} {a} / {b} / {c}"))
            .collect::<Vec<_>>()
            .join(", "),
        r.functions
            .iter()
            .map(|f| format!("`:{f}`"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    let _ = writeln!(
        s,
        "| locale | row | median ns | q1–q3 | allocs / op | B / op | P0.8 (en) |"
    );
    let _ = writeln!(s, "|---|---|---:|---:|---:|---:|---:|");
    for row in &r.rows {
        let _ = writeln!(
            s,
            "| {} | {} | **{:.1}** | {:.1}–{:.1} | {:.3} | {:.1} | {} |",
            row.locale,
            row.op.label(),
            row.ns.median,
            row.ns.q1,
            row.ns.q3,
            row.allocs_per_op,
            row.bytes_per_op,
            row.p08_ns.map_or_else(String::new, |p| format!("{p}")),
        );
    }
    let _ = writeln!(s, "\n## Gate (B10 and P0.8, `en`)\n");
    for c in &r.checks {
        let _ = writeln!(
            s,
            "- {} — **{}**{}",
            c.rule,
            if c.pass { "pass" } else { "FAIL" },
            if c.gated {
                ""
            } else {
                " (reported, not gated)"
            }
        );
    }
    s
}

fn load(l: Option<[f64; 3]>) -> String {
    l.map_or_else(
        || "?".to_owned(),
        |[a, b, c]| format!("{a:.2} {b:.2} {c:.2}"),
    )
}
