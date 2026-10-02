# Phase 3 results

Part of the [master plan](00-master-plan.md) (§9, P3); the record behind the
exit checklist of the [Phase 3 work order](10-phase-3-work-order.md). Every
figure names the command that produced it. Measured 2026-09-21 on the Phase 0
machine (Intel Core i7-1165G7, 4 cores / 8 threads, Linux 6.12), toolchain
stable 1.98.1; the fuzz target on nightly (2026-04-14) with cargo-fuzz 0.11.0;
wasm-opt 120, twiggy 0.8.0, GNU gzip 1.13, wasmtime 49.0.0 (pinned, in
`target/tools`), node v23.11.0 (ICU 76.1) for the ECMA-402 differential.
**Sizes and allocation counts do not depend on load; timings do** — and on
this machine the clock also drifts (1.8–2.3 GHz within the day), so timings are
ranges over the day's runs, and two builds are compared only by alternating
their binaries.

## Summary

| Exit item | Verdict | Evidence |
|---|---|---|
| L4 green for every file except `functions/{percent,currency,date,time,datetime}.json` (and `syntax.json` #90, owner), native and `wasm32-wasip1`, byte-identical; `current_phase = "P3"` | **met** | L4 416/462 in `conformance/REPORT.md`; the 46 left are `xfail until = "P4"` with reasons (§A9); 602 records byte-identical under wasmtime (`cargo xtask l4-wasi`); `current_phase` bumped in the exit commit with the harness green |
| stripped ≡ unstripped formatting on the whole suite; the note closed | **met** | L4 formats every applicable test from both catalogs and requires identical string, parts and errors; the note `stripped-formats-identically` is `pass` (§A9) |
| CLDR plural samples, every locale, cardinal and ordinal | **met** | 15,041 / 15,041 (12,396 cardinal in 224 locales, 2,645 ordinal in 108) through the client evaluator (§A6) |
| B1 (runtime part), B10, B12 in a client-only harness; B13 shown | **met** | B1: runtime part 18,428 B gz, numeric share 5,137 B gz (≤ 10 KB); B12: no panic import, no `core::fmt`; B13: 0 numeric symbols without the numeric handlers (§A11); B10 on all four locales (§A11) |
| L4 on generated input (1,000,000) and a ≥ 1 h `format` fuzz run clean | **met** | 1,000,000 generated cases clean in 51 s (§A10); 2,887,480 executions in 3,901 s on the final code, no crash or timeout (§A10) |
| the `mf2-runtime` API frozen in 03 §2 | **met** | [03](03-runtime.md) §2 is the API as built; the additions and departures are in 10 §"Status at exit" |
| the Phase 4 work order | **written** | [11-phase-4-work-order](11-phase-4-work-order.md) |

## What was built

| Crate / tool | Lines | Content |
|---|---:|---|
| `crates/mf2-runtime` | 4,792 + 378 tests | the evaluator over the catalog views, selection, bidi, parts, markup, `u:` options, the registry and `Function` trait, `:string`, `:number` / `:integer` / `:offset` over the own digit buffer (D15; `fixed_decimal` behind feature `fixed-decimal`), the plural evaluator. `no_std`, `forbid(unsafe_code)`, panic lints denied |
| `crates/mf2-locale-data` | 1,207 + 324 tests | UTS #35 rule parser, the 02 §4.1 encoder, all-locale plural tables and text directions shipped in `data/` (74,102 + 1,589 B), `cargo xtask locale-data` |
| `crates/mf2-host-std`, `crates/mf2-host-web` | 75, 47 | NFC and float text: `unicode-normalization` + `ryu`; `String.prototype.normalize` + `String(x)` via `js-sys` |
| `crates/mf2` | 162 | the facade; `compile_str`, `compile_str_stripped` |
| `conformance` | l4-runner 727; `l4.rs`, `l4gen.rs`, tests | layer L4, the `:test:*` functions through the public trait, L4 on generated input, the wasip1 runner |
| `fuzz/` | +380 | target `format`; `cargo xtask fuzz-seed` writes its 1,305 seeds |
| `bench/runtime-bench` | 999 + 76 | the numeric A/B (D15), the ECMA-402 re-run, B10 |
| `bench/b12` | 4 new harnesses | `runtime`, `runtime-nonum` (B13), `runtime-fixed` (A/B), the shared walk |
| CI | 1 job, 1 step, 4 nightly steps | `l4-wasi` (suite + 20,000 generated cases) on every push, and `mf2-host-web` built for `wasm32-unknown-unknown` in the `b12` job; nightly: `generated_l4` 1,000,000, 200,000 of them on wasip1, the `format` fuzz target, B10 |

Tests: 241 in `cargo test --workspace` (Phase 2: 205). Commits: `e892a97` (runtime,
locale data, hosts, facade), `7c20b38` (L4), `022a479` (numeric A/B, B12/B13
harnesses), `3015e71` (generated input, `format` fuzz target), `396ab72`
(B10), and the exit commit.

## A1 — the API

Written into [03](03-runtime.md) §2 before the code; the `:test:*` functions
of the suite were written against the public `Function` trait only (03 §3),
which proves a custom function can be expressed. The shape: a `Formatter` over
`(&Catalog, &Registry, &FormatContext)` with `simple` / `simple_ref` /
`write` / `parts` / `write_named` / `parts_named`, `dyn` sinks (one copy of the
walker), `FormatError` as a plain enum mapped to the suite's error names, a
closed-world `Registry` of `&'static dyn Function`, and a `Host` trait for NFC
and float text (which keeps `ryu` out of the client wasm). §2.6 records the
resolution rules the suite fixes beyond the spec text.

## A2–A4 — evaluator, selection, bidi, parts

Declarations resolve lazily, each at most once, without recursion (a
5,000-deep `.local` chain in the tests); an `.input` rebinds its slot for the
references after it. Selection is the spec's algorithm (filter, sort by
`BetterThan`, first best in source order); `:string` selection compares under
NFC (quick check, then the host). String and parts output share one walker
behind `dyn Out`; the parts concatenate to the string on every suite test and
every generated case.

The first L4 run (412/462) corrected one rule the spec text leaves open:
**a fallback operand reaches the handler** (`Value::Fallback`) after the
function lookup, so `{$x :f}` is still *Unknown Function*, the numeric
functions report *Bad Operand*, and `:string` selects on `{$x}` without *Bad
Selector* (03 §2.6). A runner bug (positional arguments mapped by un-normalized
names) was the other four.

## A5 — `:string` and the core numeric semantics

Every digit and rounding option, `signDisplay`, option inheritance, operand
rules, `select` = `exact` / `plural` / `ordinal` (plural operands from the
*formatted* digits, exact match on the formatted text), over the own digit
buffer: 44 digits inline, operands up to 40 significant digits and exponents
±9,999, *Unsupported Operation* past that. Suite: `functions/{string,number,
integer,offset}.json` all pass at L4. P0.5's ECMA-402 differential re-run
through the runtime, from compiled catalogs:

```sh
cargo run --release -p runtime-bench -- numbers ecma 100000 | node bench/runtime-bench/ecma-diff.cjs
```

95,675 identical, **0 different**; the 4,325 option sets `Intl.NumberFormat`
rejects are all *Bad Option* here (P0.5's figures).

## A5b — the digit buffer against `fixed_decimal` (D15)

Full report: [`bench/runtime-bench/NUMBER-AB-P3.md`](../bench/runtime-bench/NUMBER-AB-P3.md)
(`bash bench/runtime-bench/number-ab.sh`, `bash bench/b12/check.sh`).

| Row | Own buffer | `fixed_decimal` 0.7.2 |
|---|---|---|
| Output on P0.5's 100,000 cases (display, errors, cardinal and ordinal selection) | identical | identical |
| ECMA-402 | 95,675 / 0 | the same |
| Suite | 70/70 | 70/70 |
| Numeric share of B1 | **5,142 B gz** | 7,305 B gz |
| B12 | **clean** | 8 panic / alloc-failure symbols |
| Allocations per format | **0** | 0.50 |
| Speed per format | 923.3 ns | 928.3 ns (noise) |

No worse on any row: **the own buffer stays** (D15 settled); `fixed_decimal`
remains behind feature `fixed-decimal` as the baseline and fallback.

## A6 — plural and `mf2-locale-data`

The P0.4 evaluator in `mf2-runtime`; `mf2-locale-data` parses the UTS #35
rules of every CLDR locale and encodes the 02 §4.1 entries; the tables are
generated offline (`cargo xtask locale-data`) and held to `third_party/` by a
test. Lookup: exact tag, then subtag truncation, then `und`. **15,041 /
15,041** CLDR samples pass through the client evaluator (`cargo test -p
mf2-locale-data`). The catalog-bench plural entries now come from here and are
byte-identical to P0.4's, so B7 does not move. The same crate gives each
locale's text direction (likelySubtags + scriptMetadata, vendored by
`cargo xtask cldr-sync`), used by `compile_str` and the generated-input cases.

## A7–A8 — hosts, facade

`mf2-host-std` serves native and `wasm32-wasip1`; `mf2-host-web` is minimal
(NFC and `String(x)` through `js-sys`), built for `wasm32-unknown-unknown` in
CI, exercised by L6 later. `mf2::compile_str` = parse → validate (syntax and
data-model errors refuse the message with their kinds) → analyze →
`writer::single` with the locale's direction and plural entries.

## A9 — layer L4

```sh
cargo xtask conformance-report     # L4 416/462, green
cargo xtask l4-wasi                # 602 records byte-identical on wasm32-wasip1
```

416 of 462 pass; the 46 left are `xfail until = "P4"`, each with a reason: the
45 tests of `functions/{percent,currency,date,time,datetime}.json`, and
`syntax.json` #90 (the French decimal comma on unannotated floats needs
Phase 4's number symbols; moved by the owner, 2026-09-21). Every test is
formatted from the unstripped **and** the stripped catalog and must agree, so
the note `stripped-formats-identically` is closed. The `wasm32-wasip1` run
formats every case — 301 tests × 2 catalogs — under wasmtime; the records are
byte-identical to native.

## A10 — generated input and fuzzing

**Generated L4** (`conformance/src/l4gen.rs`, `tests/generated_l4.rs`): the
messages of `generated.rs` (from the vendored `message.abnf`, same seeds),
steered towards the runtime — function names and options the handlers read,
with valid and invalid values; numeric operands; placeholders and selectors
that refer to declarations; plural keys — compiled for a random locale
(`en`, `pl`, `ar`, `he`, `cy`, `ja`, `fr-CA`, `und`), valid or not, with
generated arguments of every kind. Checked: no panic, deterministic,
named = positional, parts = string, stripped = unstripped.

```sh
MF2_GEN_CASES=1000000 cargo test --release -p mf2-conformance --test generated_l4
cargo xtask l4-wasi --generated 200000
```

1,000,000 cases clean in 51 s: 745,726 valid models, 550,911 formatted without
errors, 389,562 calling `:number` / `:integer`, 139,185 with a formatted
number, 249,142 selections; nine error kinds reached. On wasip1, 200,000 of
them (every fifth) give 400,602 records identical to native (≈ 80 s).

**Fuzz target `format`** (`fuzz/fuzz_targets/format.rs`, README): catalog
bytes (or source + locale) and argument bytes; every message formatted
positionally twice, to parts and with named arguments; a time budget of 50 ms
+ 50 µs per input byte + 100 ns per byte of output. Its first minutes found
two divergences on damaged catalogs, both fixed in `mf2-runtime`:

* a markup name that is not UTF-8 stopped parts output (*Malformed*) but not
  string output, which skipped markup — the name is now checked in both modes
  (regression test `malformed_markup_name`);
* a variable reference past the message's last slot read a positional argument
  but not a named one — positional arguments are now cut to the message's
  slots, as named ones were (03 §2.1).

**Exit run** on the final code:

```sh
cargo xtask fuzz-seed
cd fuzz && cargo +nightly fuzz run format -- -dict=mf2.dict -max_len=131072 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900
```

**2,887,480 executions in 3,901 s** (740 per second; the seeds include the
73 KB workload catalogs), 10,618 coverage points, 45,188 features, the corpus
grown from 1,305 seeds to 5,268 inputs (33 MB), peak RSS 580 MB; **no crash,
no timeout, no budget failure**. No build ran on the machine during it (load
average 1.32 at the start, 1.45 at the end).

## A11 — budgets

```sh
bash bench/b12/check.sh                                   # B1, B12, B13
cargo run --release -p runtime-bench -- b10 --gate \
    --md bench/runtime-bench/B10-P3.md --json bench/runtime-bench/b10-p3.json
cargo run --release -p runtime-bench --example select_cost
```

**B1, runtime part** (`b12-runtime`: a client stand-in that formats every
message of a catalog to string and parts, positional and named, with every
argument kind; `wasm-release`, `wasm-opt -Oz`, gzip -9; Δ against the base):

| Harness | Δ raw | **Δ gz** |
|---|---:|---:|
| reader + a walk over its whole API (Phase 2) | 13,599 | 6,788 |
| **runtime**: reader + evaluator + `:string` + core numbers + plural + bidi + parts | 37,126 | **18,428** |
| runtime without the numeric handlers (`b12-runtime-nonum`) | 26,899 | 13,291 |
| the numeric share (runtime − nonum; B1 allows ≤ 10 KB gz) | 10,227 | **5,137** |
| the same over `fixed_decimal` | 15,039 | 7,311 |

With P0.1's call-site library (7.5 KB gz) the runtime part makes 25.9 of B1's
30 KB, leaving 4.1 KB for fetch and boot, which P6 measures. Where the bytes go
(twiggy, names kept, raw): the evaluator 12.3 KB (`Formatter::run` 5.7 KB — the
walk, selection and pattern code inlined into it), the numeric options, format
and selection 8.6 KB, the digit buffer 3.2 KB, the reader 2.7 KB plus
`Catalog::new` 3.4 KB inlined into the harness, parts and fallback 1.3 KB,
`:string` 0.9 KB. Against P0.3's 7.0 KB gz skeleton, the difference is scope:
P0.3 had no `u:` options, decimals or digit options, registry, NFC host,
named arguments, or the frozen reader's full validation. Known savings are
left for the size gate (P6): IDS validation at load (every IDS access is
bounds-checked anyway), and the option interning of 06 §3.

**B12**: the panic import is absent after LTO + `wasm-opt -Oz` in `runtime`
and `runtime-nonum`; no `core::fmt`. It first failed: at `opt-level = "z"`
LLVM inlines `Vec::push` (and so proves its growth branch dead after
`try_reserve`) only for a function with one caller — hence one `push` site
per list type (`scratch.rs`), and a guarded `push_str` for `String`.
**B13**: `runtime-nonum` links 0 numeric symbols (14 with the handlers).

**B10** (`runtime-bench b10`, the four stripped production catalogs, default
context, `NoErrors`, medians of 31):

| Row | en | pl | en-XA | ar-XB | P0.8 (en) |
|---|---:|---:|---:|---:|---:|
| simple, `Formatter::simple` | 13.0–22.3 | 56.8–71.4 | 72.9–87.7 | 19.8–33.6 | 20.7 |
| simple, `write` (≤ 100 ns, 0 allocs) | **20.1–34.5** | 69.6–81.3 | 83.3–93.2 | 33.6–43.6 | — |
| 1-argument pattern, reused `String` (≤ 500 ns, ≤ 1 alloc) | **118–137** | 133–149 | 155–175 | 130–150 | 93.5 |
| 1-argument pattern, new `String::with_capacity(128)` | 134–159, 1.02 allocs | 147–177 | 175–208, 1.15 | 144–173 | 106.3 |
| select, reused `String` | 433–514, 4 allocs | 703–802 | 508–573 | 473–533 | 317 |

Ranges are over the day's heap-only runs (two full runs for every locale, seven
for `en`); the committed `B10-P3.md` caught the clock at 1,823 MHz. B10 holds
on every locale, 0 allocations; the non-Latin UTF-8 finding of
Phase 2 therefore needs no change to the reader. P0.8's simple and
1-argument figures are kept within 1.5× (catalog-bench's rule for them).
**Select is at 1.37–1.62× P0.8** (reported, not gated: B10 has no select row).
Attribution (`select_cost`, `en`, Int 3): an unannotated `{$n}` 87 ns,
`{$n :integer}` 196 ns, with `.input` 284 ns, select + placeholder 368 ns — the
full numeric semantics cost ~110 ns where P0.3's `:integer` was integer-only.
The allocations are not the cause; measured by alternating binaries:

| `Scratch` variant | select | 1-argument pattern | size |
|---|---|---|---|
| heap only (kept) | 436–448 ns, 4 allocs | 118–137 ns | 18,428 B gz |
| four items inline, enum | 490 ns, 0 allocs | 164–165 ns | +2,491 B gz |
| four items inline, fixed array | 490 ns, 0 allocs | 165–187 ns | +811 B gz |
| index lists inline only | 484–494 ns, 1 alloc | 140 ns | — |

A simple integer fast path in the display step was not measurable above the
noise and was not kept. Phase 4 rebuilds the numeric layer and carries the
select figure (11 §"What Phase 3 changes here").

**Function resolution** per call (`catalog.function(i)` → `Registry::get`)
costs 12.6 ns, a load-time table 2.8 ns: it stays per call (03 §2.4).

## A12 — ledger

`current_phase = "P3"` in the exit commit; `cargo xtask conformance-report`
green at P3. L4: 416 pass, 46 `xfail until = "P4"` with reasons; every L4d
(default-features) cell `xfail until = "P4"`; no open note.
