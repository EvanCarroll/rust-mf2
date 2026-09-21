//! Bench mode (A9, native half): the reader's cost on the four production
//! catalogs, in the parser gate's style — one process, rows interleaved
//! round by round, medians, a counting allocator.
//!
//! Rows per locale:
//!
//! | op | what one operation is |
//! |---|---|
//! | `new` | `Catalog::new(buffer, hash)` on the stripped catalog; the buffer is moved in; buffers are prepared *before* the timed batch and taken back with `into_bytes` *after* it, so nothing is copied, allocated or freed inside it |
//! | `new-unstripped` | the same on the unstripped catalog (IDS walked too) |
//! | `clone` | `Vec::clone` + drop of the stripped buffer — the copy `new` excludes (informative; P0.8 measured it the same way) |
//! | `simple` | `get` + `text` of one simple message, sweeping every simple id in `MsgId` order (P0.8's "simple lookup") |
//! | `utf8` | `str::from_utf8` alone over the same strings (attribution: the per-access UTF-8 check inside `text`) |
//! | `get-pattern`, `get-select` | `get` of one pattern / select message (the INDEX read and the view) |
//! | `walk-pattern`, `walk-select` | `get`, then the body: every part, text resolved; for a select every selector, key and variant pattern |
//!
//! Allocation counts are exact (one counted operation or sweep). The 0-copy
//! check: after `Catalog::new`, `as_bytes().as_ptr()` is the moved-in
//! buffer's pointer.

use std::hint::black_box;
use std::time::{Duration, Instant};

use mf2_catalog::{
    Body, Catalog, CatalogError, Entry, KeyView, MsgId, Operand, PartView, PatternView, VarRef,
};
use serde::Serialize;

use crate::alloc::{self, Counts};
use crate::baseline;
use crate::corpus::{self, Built};
use crate::error::{Error, Result};
use crate::report::{Build, cpu_mhz, load_average, unix_time};
use crate::stats::Spread;

/// The gate's minimum number of samples per row.
pub const MIN_RUNS: usize = 31;
/// Load and simple lookup must be within this factor of P0.8's figures.
pub const P08_FACTOR: f64 = 1.5;
/// B10: a simple message ≤ 100 ns natively.
pub const B10_SIMPLE_NS: f64 = 100.0;

/// Measurement knobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Settings {
    /// Samples per row.
    pub runs: usize,
    /// Minimum timed duration of one sample.
    #[serde(serialize_with = "millis")]
    pub min_sample: Duration,
    /// Warm-up per row.
    #[serde(serialize_with = "millis")]
    pub warmup: Duration,
    /// `Catalog::new` / `clone` calls per timed batch (buffers are prepared
    /// before a batch and taken back after it).
    pub batch: usize,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's signature
fn millis<S: serde::Serializer>(d: &Duration, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_u128(d.as_millis())
}

/// One benchmarked operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    /// `Catalog::new`, stripped catalog.
    New,
    /// `Catalog::new`, unstripped catalog.
    NewUnstripped,
    /// `Vec::clone` of the stripped buffer.
    Clone,
    /// `get` + `text`, simple messages.
    Simple,
    /// `str::from_utf8` of the simple messages' strings alone.
    Utf8,
    /// `get`, pattern messages.
    GetPattern,
    /// `get`, select messages.
    GetSelect,
    /// `get` + a walk of the pattern.
    WalkPattern,
    /// `get` + a walk of the select.
    WalkSelect,
}

impl Op {
    /// Every row, in report order.
    pub const ALL: [Op; 9] = [
        Op::New,
        Op::NewUnstripped,
        Op::Clone,
        Op::Simple,
        Op::Utf8,
        Op::GetPattern,
        Op::GetSelect,
        Op::WalkPattern,
        Op::WalkSelect,
    ];

    /// Table label.
    pub const fn label(self) -> &'static str {
        match self {
            Op::New => "`Catalog::new` (stripped)",
            Op::NewUnstripped => "`Catalog::new` (unstripped)",
            Op::Clone => "`clone` + drop of the buffer (excluded from `new`)",
            Op::Simple => "simple: `get` + `text`",
            Op::Utf8 => "`str::from_utf8` alone, same strings (part of simple)",
            Op::GetPattern => "`get`, pattern",
            Op::GetSelect => "`get`, select",
            Op::WalkPattern => "`get` + walk, pattern",
            Op::WalkSelect => "`get` + walk, select",
        }
    }

    /// Whether one operation takes a buffer per call (timed in batches).
    const fn batched(self) -> bool {
        matches!(self, Op::New | Op::NewUnstripped | Op::Clone)
    }
}

/// One locale's catalogs and the ids of each class.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// BCP 47 tag.
    pub tag: String,
    /// The manifest hash.
    pub hash: u64,
    /// Production catalog.
    pub stripped: Vec<u8>,
    /// With IDS.
    pub unstripped: Vec<u8>,
    /// Simple messages.
    pub simple: Vec<MsgId>,
    /// Pattern messages.
    pub pattern: Vec<MsgId>,
    /// Select messages.
    pub select: Vec<MsgId>,
    /// The simple messages' strings: byte ranges in the stripped buffer.
    pub simple_texts: Vec<(usize, usize)>,
}

impl Fixture {
    /// From a built locale.
    pub fn new(b: &Built) -> Result<Self> {
        let cat = load(&b.tag, b.stripped.clone())?;
        let (mut simple, mut pattern, mut select) = (Vec::new(), Vec::new(), Vec::new());
        let mut simple_texts = Vec::new();
        let base = cat.as_bytes().as_ptr().addr();
        for i in 0..cat.message_count() {
            let Some(id) = MsgId::new(0, i) else { continue };
            match cat.get(id) {
                Entry::Simple(r) => {
                    simple.push(id);
                    if let Some(t) = cat.text(r) {
                        let start = t.as_ptr().addr() - base;
                        simple_texts.push((start, start + t.len()));
                    }
                }
                Entry::Pattern(_) => pattern.push(id),
                Entry::Select(_) => select.push(id),
                Entry::Absent => {}
            }
        }
        Ok(Fixture {
            tag: b.tag.clone(),
            hash: corpus::MANIFEST_HASH,
            stripped: b.stripped.clone(),
            unstripped: b.unstripped.clone(),
            simple,
            pattern,
            select,
            simple_texts,
        })
    }

    fn ids(&self, op: Op) -> &[MsgId] {
        match op {
            Op::Simple => &self.simple,
            Op::GetPattern | Op::WalkPattern => &self.pattern,
            Op::GetSelect | Op::WalkSelect => &self.select,
            Op::New | Op::NewUnstripped | Op::Clone | Op::Utf8 => &[],
        }
    }

    /// Operations in one sweep of a lookup row.
    fn sweep_len(&self, op: Op) -> usize {
        if op == Op::Utf8 {
            self.simple_texts.len()
        } else {
            self.ids(op).len()
        }
    }

    fn buffer(&self, op: Op) -> &[u8] {
        if op == Op::NewUnstripped {
            &self.unstripped
        } else {
            &self.stripped
        }
    }
}

fn load(tag: &str, bytes: Vec<u8>) -> Result<Catalog> {
    Catalog::new(bytes, corpus::MANIFEST_HASH).map_err(|source| Error::Load {
        tag: tag.to_owned(),
        source,
    })
}

/// Text length of `r`, or 0.
#[inline]
fn text_len(cat: &Catalog, r: mf2_catalog::StrRef) -> u64 {
    cat.text(r).map_or(0, |s| s.len() as u64)
}

/// Walks every part of a pattern, resolving text; returns a checksum.
#[inline]
fn walk_pattern(cat: &Catalog, p: PatternView<'_>) -> u64 {
    let mut acc = 0u64;
    for part in p.parts() {
        acc = acc.wrapping_add(match part {
            Ok(PartView::Text(r)) => text_len(cat, r),
            Ok(PartView::Expression(e)) => {
                let op = match e.operand() {
                    Some(Operand::Variable(VarRef::External(s))) => u64::from(s) + 1,
                    Some(Operand::Variable(VarRef::Local(i))) => u64::from(i) + 64,
                    Some(Operand::Literal(r)) => text_len(cat, r),
                    None => 0,
                };
                let f = e.function().map_or(0, |f| {
                    u64::from(f.index()) + f.options().map(|o| u64::from(o.is_ok())).sum::<u64>()
                });
                op + f
            }
            Ok(PartView::Markup(m)) => text_len(cat, m.name()),
            Err(_) => 1 << 20,
        });
    }
    acc
}

/// `get`, then the whole body; returns a checksum.
#[inline]
fn walk(cat: &Catalog, id: MsgId) -> u64 {
    let (Entry::Pattern(v) | Entry::Select(v)) = cat.get(id) else {
        return 0;
    };
    match v.body() {
        Ok(Body::Pattern(p)) => walk_pattern(cat, p),
        Ok(Body::Select(s)) => {
            let mut acc = s.selectors().count() as u64;
            for var in s.variants() {
                let Ok(var) = var else {
                    return acc + (1 << 20);
                };
                for k in var.keys() {
                    acc = acc.wrapping_add(match k {
                        Ok(KeyView::Literal(r)) => text_len(cat, r),
                        Ok(KeyView::CatchAll) => 1,
                        Err(_) => 1 << 20,
                    });
                }
                acc = acc.wrapping_add(walk_pattern(cat, var.pattern()));
            }
            acc
        }
        Err(_) => 1 << 20,
    }
}

/// One sweep of lookup row `op`; returns a checksum.
#[inline]
fn sweep(fx: &Fixture, cat: &Catalog, op: Op) -> u64 {
    let ids = black_box(fx.ids(op));
    let mut acc = 0u64;
    match op {
        Op::Utf8 => {
            let bytes = cat.as_bytes();
            for &(a, b) in black_box(&fx.simple_texts) {
                if let Some(s) = bytes.get(a..b) {
                    acc = acc.wrapping_add(
                        std::str::from_utf8(black_box(s)).map_or(0, |t| t.len() as u64),
                    );
                }
            }
        }
        Op::Simple => {
            for &id in ids {
                if let Entry::Simple(r) = cat.get(black_box(id)) {
                    acc = acc.wrapping_add(text_len(cat, r));
                }
            }
        }
        Op::GetPattern | Op::GetSelect => {
            for &id in ids {
                let e = black_box(cat.get(black_box(id)));
                acc =
                    acc.wrapping_add(u64::from(matches!(e, Entry::Pattern(_) | Entry::Select(_))));
            }
        }
        Op::WalkPattern | Op::WalkSelect => {
            for &id in ids {
                acc = acc.wrapping_add(walk(cat, black_box(id)));
            }
        }
        Op::New | Op::NewUnstripped | Op::Clone => {}
    }
    acc
}

/// Reused buffers of the batched rows (allocated once per row).
struct Batch {
    /// Buffers ready to be moved into `Catalog::new`.
    inputs: Vec<Vec<u8>>,
    /// The catalogs of one batch, taken apart after it.
    catalogs: Vec<std::result::Result<Catalog, CatalogError>>,
}

impl Batch {
    fn new(n: usize) -> Self {
        Batch {
            inputs: Vec::with_capacity(n),
            catalogs: Vec::with_capacity(n),
        }
    }
}

/// `reps` repetitions of `op`: batches of `batch` calls, or sweeps over the
/// ids. Returns the timed duration, the number of operations and a checksum.
fn run_op(
    fx: &Fixture,
    cat: &Catalog,
    op: Op,
    reps: u64,
    batch: usize,
    b: &mut Batch,
) -> (Duration, u64, u64) {
    let mut timed = Duration::ZERO;
    let mut check = 0u64;
    if op == Op::Clone {
        let buf = fx.buffer(op);
        let start = Instant::now();
        for _ in 0..reps * batch as u64 {
            check = check.wrapping_add(black_box(black_box(buf).to_vec()).len() as u64);
        }
        timed += start.elapsed();
        return (timed, reps * batch as u64, check);
    }
    if op.batched() {
        let buf = fx.buffer(op);
        for _ in 0..reps {
            // Buffers come back from the previous batch (`into_bytes`); only
            // the first batch (or a failed load) clones.
            while b.inputs.len() < batch {
                b.inputs.push(buf.to_vec());
            }
            let start = Instant::now();
            for v in b.inputs.drain(..) {
                b.catalogs
                    .push(Catalog::new(black_box(v), black_box(fx.hash)));
            }
            timed += start.elapsed();
            for c in b.catalogs.drain(..).flatten() {
                check += 1;
                b.inputs.push(c.into_bytes());
            }
        }
        return (timed, reps * batch as u64, check);
    }
    let start = Instant::now();
    for _ in 0..reps {
        check = check.wrapping_add(sweep(fx, cat, op));
    }
    timed += start.elapsed();
    (timed, reps * fx.sweep_len(op) as u64, black_box(check))
}

/// One row's result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// Locale.
    pub locale: String,
    /// Operation.
    pub op: Op,
    /// Operations in one sweep (ids) or batch.
    pub per_pass: usize,
    /// Passes per sample.
    pub passes_per_sample: u64,
    /// ns per operation over the samples.
    pub ns: Spread,
    /// Allocations of one operation (batched rows) or one sweep.
    pub allocs: Counts,
    /// Allocations per operation.
    pub allocs_per_op: f64,
    /// Checksum of one pass (successful loads, or text bytes read).
    pub checksum: u64,
    /// P0.8's figure for this row, ns, when it has one.
    pub p08_ns: Option<f64>,
}

/// The load checks of one catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoadCheck {
    /// Locale.
    pub locale: String,
    /// `stripped` or `unstripped`.
    pub catalog: &'static str,
    /// Buffer length.
    pub bytes: usize,
    /// Allocations made by `Catalog::new`.
    pub allocs: Counts,
    /// `as_bytes()` is the moved-in buffer (pointer and length).
    pub zero_copy: bool,
}

/// One gate rule, evaluated.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GateCheck {
    /// What.
    pub rule: String,
    /// Measured.
    pub value: f64,
    /// Limit.
    pub limit: f64,
    /// Whether it holds.
    pub passed: bool,
    /// Reported only: does not decide the gate.
    pub informative: bool,
}

/// The whole bench run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BenchReport {
    /// `catalog-bench <version>`.
    pub tool: String,
    /// How the harness was built.
    pub build: Build,
    /// Knobs.
    pub settings: Settings,
    /// `available_parallelism`.
    pub available_parallelism: usize,
    /// Load averages (1, 5, 15 min) before and after the run.
    pub load_before: Option<[f64; 3]>,
    /// After.
    pub load_after: Option<[f64; 3]>,
    /// Mean CPU clock (MHz) before and after the run.
    pub cpu_mhz: [Option<f64>; 2],
    /// Seconds since the Unix epoch at the end.
    pub unix_time: u64,
    /// Every row.
    pub rows: Vec<Row>,
    /// 0 allocations and 0 copies at load.
    pub loads: Vec<LoadCheck>,
    /// The gate.
    pub gate: Vec<GateCheck>,
    /// Every gate rule held.
    pub passed: bool,
}

/// The load checks: allocations of `Catalog::new` and the buffer identity.
fn load_checks(fx: &Fixture) -> Result<Vec<LoadCheck>> {
    let mut out = Vec::new();
    for (name, bytes) in [("stripped", &fx.stripped), ("unstripped", &fx.unstripped)] {
        let v = bytes.clone();
        let (ptr, len) = (v.as_ptr(), v.len());
        let (cat, allocs) = alloc::count(|| Catalog::new(v, fx.hash));
        let cat = cat.map_err(|source| Error::Load {
            tag: fx.tag.clone(),
            source,
        })?;
        out.push(LoadCheck {
            locale: fx.tag.clone(),
            catalog: name,
            bytes: len,
            allocs,
            zero_copy: std::ptr::eq(cat.as_bytes().as_ptr(), ptr) && cat.as_bytes().len() == len,
        });
    }
    Ok(out)
}

struct Cell<'f> {
    fx: &'f Fixture,
    cat: Catalog,
    op: Op,
    reps: u64,
    samples: Vec<f64>,
    allocs: Counts,
    checksum: u64,
    per_pass: usize,
    batch: Batch,
}

/// Runs the bench over `built` (every locale).
pub fn run(
    built: &[Built],
    settings: &Settings,
    mut progress: impl FnMut(&str),
) -> Result<BenchReport> {
    if settings.runs == 0 || settings.batch == 0 {
        return Err(Error::Settings(
            "--runs and --batch must be at least 1".to_owned(),
        ));
    }
    let load_before = load_average();
    let mhz_before = cpu_mhz();
    let fixtures: Vec<Fixture> = built.iter().map(Fixture::new).collect::<Result<_>>()?;
    let mut loads = Vec::new();
    for fx in &fixtures {
        loads.extend(load_checks(fx)?);
    }

    // Warm-up, allocation count and calibration, row by row.
    let mut cells = Vec::new();
    for fx in &fixtures {
        for op in Op::ALL {
            let cat = load(&fx.tag, fx.stripped.clone())?;
            let mut batch = Batch::new(settings.batch);
            let deadline = Instant::now() + settings.warmup;
            loop {
                black_box(run_op(fx, &cat, op, 1, settings.batch, &mut batch));
                if Instant::now() >= deadline {
                    break;
                }
            }
            let (allocs, checksum, per_pass) = if op.batched() {
                let v = fx.buffer(op).to_vec();
                let (checksum, allocs) = if op == Op::Clone {
                    let (c, a) = alloc::count(|| black_box(v.clone()).len());
                    (c as u64, a)
                } else {
                    let (c, a) = alloc::count(|| Catalog::new(v, fx.hash).is_ok());
                    (u64::from(c), a)
                };
                (allocs, checksum, settings.batch)
            } else {
                let (c, a) = alloc::count(|| sweep(fx, &cat, op));
                (a, c, fx.sweep_len(op))
            };
            let one = (0..3)
                .map(|_| run_op(fx, &cat, op, 1, settings.batch, &mut batch).0)
                .min()
                .unwrap_or(Duration::from_nanos(1));
            let reps = u64::try_from(
                settings
                    .min_sample
                    .as_nanos()
                    .div_ceil(one.as_nanos().max(1)),
            )
            .unwrap_or(u64::MAX)
            .max(1);
            cells.push(Cell {
                fx,
                cat,
                op,
                reps,
                samples: Vec::with_capacity(settings.runs),
                allocs,
                checksum,
                per_pass,
                batch,
            });
        }
    }
    progress(&format!(
        "{} rows warmed up, counted and calibrated; {} rounds",
        cells.len(),
        settings.runs
    ));

    // Interleaved samples: one per row per round, the order reversing every
    // other round.
    for round in 0..settings.runs {
        let forward = round.is_multiple_of(2);
        for k in 0..cells.len() {
            let i = if forward { k } else { cells.len() - 1 - k };
            let c = &mut cells[i];
            let (dt, ops, _) = run_op(c.fx, &c.cat, c.op, c.reps, settings.batch, &mut c.batch);
            #[allow(clippy::cast_precision_loss)]
            c.samples.push(dt.as_nanos() as f64 / ops.max(1) as f64);
        }
        if (round + 1).is_multiple_of(5) || round + 1 == settings.runs {
            progress(&format!("round {}/{}", round + 1, settings.runs));
        }
    }

    let rows: Vec<Row> = cells
        .into_iter()
        .map(|mut c| {
            let p08 = baseline::p08(&c.fx.tag);
            let p08_ns = match c.op {
                Op::New => p08.map(|p| p.new_us * 1000.0),
                Op::Simple => p08.and_then(|p| p.simple_ns),
                _ => None,
            };
            #[allow(clippy::cast_precision_loss)]
            let allocs_per_op = if c.op.batched() {
                c.allocs.allocs as f64
            } else {
                c.allocs.allocs as f64 / c.per_pass.max(1) as f64
            };
            Row {
                locale: c.fx.tag.clone(),
                op: c.op,
                per_pass: c.per_pass,
                passes_per_sample: c.reps,
                ns: Spread::of(&mut c.samples).unwrap_or(Spread {
                    min: 0.0,
                    q1: 0.0,
                    median: 0.0,
                    q3: 0.0,
                    max: 0.0,
                }),
                allocs: c.allocs,
                allocs_per_op,
                checksum: c.checksum,
                p08_ns,
            }
        })
        .collect();
    let gate = gate(&rows, &loads);
    Ok(BenchReport {
        tool: format!("catalog-bench {}", env!("CARGO_PKG_VERSION")),
        build: Build::current(),
        settings: *settings,
        available_parallelism: std::thread::available_parallelism().map_or(0, usize::from),
        load_before,
        load_after: load_average(),
        cpu_mhz: [mhz_before, cpu_mhz()],
        unix_time: unix_time(),
        passed: gate.iter().all(|g| g.passed || g.informative),
        rows,
        loads,
        gate,
    })
}

/// The reference locale: B10 is stated for the reference workload (plans/06
/// §2–3), so its simple-lookup check decides the gate for `en` only; the
/// other locales' are reported.
pub const REFERENCE: &str = "en";

/// The gate: 0 allocations and 0 copies at load; no allocation in any lookup;
/// load and simple lookup within 1.5× of P0.8; simple ≤ 100 ns (B10) for the
/// reference `en` (reported, not gated, for the other locales).
#[allow(clippy::cast_precision_loss)]
pub fn gate(rows: &[Row], loads: &[LoadCheck]) -> Vec<GateCheck> {
    let mut out = Vec::new();
    let mut rule = |rule: String, value: f64, limit: f64, passed: bool, informative: bool| {
        out.push(GateCheck {
            rule,
            value,
            limit,
            passed,
            informative,
        });
    };
    for l in loads {
        rule(
            format!("{} {}: `Catalog::new` allocations", l.locale, l.catalog),
            l.allocs.allocs as f64,
            0.0,
            l.allocs.allocs == 0,
            false,
        );
        rule(
            format!(
                "{} {}: 0 copies (buffer moved in, kept)",
                l.locale, l.catalog
            ),
            if l.zero_copy { 0.0 } else { 1.0 },
            0.0,
            l.zero_copy,
            false,
        );
    }
    for r in rows {
        if !r.op.batched() {
            rule(
                format!("{} {}: allocations per sweep", r.locale, r.op.label()),
                r.allocs.allocs as f64,
                0.0,
                r.allocs.allocs == 0,
                false,
            );
        }
        if let Some(p) = r.p08_ns {
            rule(
                format!(
                    "{} {}: median ≤ {P08_FACTOR} × P0.8 ({})",
                    r.locale,
                    r.op.label(),
                    duration(p)
                ),
                r.ns.median,
                P08_FACTOR * p,
                r.ns.median <= P08_FACTOR * p,
                false,
            );
        }
        if r.op == Op::Simple {
            let reference = r.locale == REFERENCE;
            rule(
                format!(
                    "{} simple ≤ 100 ns (B10, the reader's share){}",
                    r.locale,
                    if reference {
                        ""
                    } else {
                        " — reported: B10 is stated for the reference workload"
                    }
                ),
                r.ns.median,
                B10_SIMPLE_NS,
                r.ns.median <= B10_SIMPLE_NS,
                !reference,
            );
        }
    }
    out
}

/// `20.7 ns` / `5.73 µs`.
pub fn duration(ns: f64) -> String {
    if ns >= 1000.0 {
        format!("{:.2} µs", ns / 1000.0)
    } else {
        format!("{ns:.1} ns")
    }
}

fn mhz_line(m: Option<f64>) -> String {
    m.map_or_else(|| "n/a".to_owned(), |m| format!("{m:.0} MHz"))
}

fn load_line(l: Option<[f64; 3]>) -> String {
    l.map_or_else(
        || "n/a".to_owned(),
        |[a, b, c]| format!("{a:.2} / {b:.2} / {c:.2}"),
    )
}

impl BenchReport {
    /// The JSON form.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self).map(|mut s| {
            s.push('\n');
            s
        })
    }

    /// The Markdown form.
    pub fn to_markdown(&self) -> String {
        use std::fmt::Write as _;
        let mut o = String::new();
        let _ = writeln!(o, "# Catalog reader bench — native (A9)\n");
        let _ = writeln!(
            o,
            "Generated by `{}` ({}). Catalogs: the four production (stripped) catalogs of the \
             size report, built in process from the reference workload. Each row: median of \
             **{}** samples, each ≥ {} ms of timed work after a {} ms warm-up, interleaved over \
             every row (one sample of every row per round, the order reversing every other \
             round); `Catalog::new` is timed in batches of {} calls, the buffers prepared \
             before and taken back (`into_bytes`) after the timed region, so nothing is \
             copied, allocated or freed while the clock runs. Allocations: one \
             operation (load rows) or one sweep (lookup rows) under the counting allocator — \
             exact and load-independent. Machine: {} hardware threads; load average (1 / 5 / \
             15 min) {} before, {} after the run; mean CPU clock {} before, {} after.\n",
            self.tool,
            self.build.line(),
            self.settings.runs,
            self.settings.min_sample.as_millis(),
            self.settings.warmup.as_millis(),
            self.settings.batch,
            self.available_parallelism,
            load_line(self.load_before),
            load_line(self.load_after),
            mhz_line(self.cpu_mhz[0]),
            mhz_line(self.cpu_mhz[1]),
        );

        let _ = writeln!(o, "## Load: allocations and copies\n");
        let _ = writeln!(
            o,
            "| locale | catalog | bytes | allocations | bytes allocated | 0-copy |"
        );
        let _ = writeln!(o, "|---|---|---:|---:|---:|---|");
        for l in &self.loads {
            let _ = writeln!(
                o,
                "| {} | {} | {} | {} | {} | {} |",
                l.locale,
                l.catalog,
                crate::report::n(l.bytes),
                l.allocs.allocs,
                l.allocs.bytes,
                if l.zero_copy { "yes" } else { "**NO**" }
            );
        }

        let _ = writeln!(o, "\n## Timings\n");
        let _ = writeln!(
            o,
            "| locale | operation | median | best | IQR | P0.8 | × P0.8 | allocs / op | ops per pass |"
        );
        let _ = writeln!(o, "|---|---|---:|---:|---:|---:|---:|---:|---:|");
        let mut last = "";
        for r in &self.rows {
            let first = last != r.locale;
            last = &r.locale;
            let _ = writeln!(
                o,
                "| {} | {} | **{}** | {} | {:.1} % | {} | {} | {} | {} |",
                if first { r.locale.as_str() } else { "" },
                r.op.label(),
                duration(r.ns.median),
                duration(r.ns.min),
                r.ns.relative_iqr() * 100.0,
                r.p08_ns.map_or_else(String::new, duration),
                r.p08_ns
                    .map_or_else(String::new, |p| format!("{:.2}", r.ns.median / p)),
                if r.allocs_per_op == 0.0 {
                    "0".to_owned()
                } else {
                    format!("{:.2}", r.allocs_per_op)
                },
                crate::report::n(r.per_pass),
            );
        }
        let _ = writeln!(
            o,
            "\nP0.8 (plans/phase-0-results.md §P0.8): P0.3's reader over P0.7's catalogs, \
             per-access UTF-8, criterion, taken under load 2–5; its simple lookup is \
             `Formatter::simple` (= `get` + `text`), measured for `en` and `ar-XB` only."
        );

        let _ = writeln!(o, "\n## Gate\n");
        let _ = writeln!(
            o,
            "**{}** — `Catalog::new` allocates nothing and keeps the moved-in buffer; no \
             lookup allocates; load and simple lookup within {P08_FACTOR}× of P0.8; simple ≤ \
             100 ns (B10) for the reference `en` (the other locales' B10 rows are reported, \
             not gated).\n",
            if self.passed { "PASS" } else { "FAIL" }
        );
        let _ = writeln!(o, "| rule | value | limit | result |");
        let _ = writeln!(o, "|---|---:|---:|---|");
        for g in &self.gate {
            let (v, l) = if g.limit > 0.0 {
                (duration(g.value), duration(g.limit))
            } else {
                (format!("{}", g.value), format!("{}", g.limit))
            };
            let _ = writeln!(
                o,
                "| {} | {} | {} | {} |",
                g.rule,
                v,
                l,
                match (g.passed, g.informative) {
                    (true, false) => "pass",
                    (false, false) => "**FAIL**",
                    (true, true) => "within (reported)",
                    (false, true) => "**over** (reported)",
                }
            );
        }
        o
    }
}

#[cfg(test)]
mod tests {
    use super::{Fixture, load_checks};
    use crate::corpus;

    /// The deterministic half of the A9 gate, on the real workload: loading
    /// any of the eight catalogs allocates nothing and keeps the buffer.
    #[test]
    fn workload_catalogs_load_without_allocating_or_copying() {
        let sources = corpus::generate(&crate::repo_root()).unwrap();
        let (manifest, built) = corpus::build(&sources, false).unwrap();
        assert_eq!(manifest.hash(), corpus::MANIFEST_HASH);
        assert_eq!(built.len(), 4);
        for b in &built {
            let fx = Fixture::new(b).unwrap();
            assert_eq!(
                (fx.simple.len(), fx.pattern.len(), fx.select.len()),
                (1256, 330, 14),
                "{}",
                b.tag
            );
            for l in load_checks(&fx).unwrap() {
                assert_eq!(l.allocs.allocs, 0, "{} {}", l.locale, l.catalog);
                assert!(l.zero_copy, "{} {}", l.locale, l.catalog);
            }
        }
    }
}
