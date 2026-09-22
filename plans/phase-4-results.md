# Phase 4 results

Part of the [master plan](00-master-plan.md) (§9, P4); the record behind the
exit checklist of the [Phase 4 work order](11-phase-4-work-order.md). Every
figure names the command that produced it. Measured 2026-09-22 on the Phase 0
machine (Intel Core i7-1165G7, 4 cores / 8 threads, Linux 6.12), toolchain
stable 1.98.1; wasm-opt 120, twiggy 0.8.0, GNU gzip 1.13, wasmtime 49.0.0
(pinned, in `target/tools`), node v23.11.0 (ICU 76.1, CLDR 46) as the local
`Intl` reference. **Sizes do not depend on load; timings do**, and the clock
drifts (1.8–2.3 GHz within the day): timings are ranges, and two builds are
compared only by alternating their binaries.

## Summary

| Exit item | Verdict | Evidence |
|---|---|---|
| L4 **100 %** in the all-features configuration — every suite file, `syntax.json` #90 and `conformance/extra/functions/unit.json` — native and `wasm32-wasip1`, byte-identical; `current_phase = "P4"` with the harness green | **met** | L4 485/485 in `conformance/REPORT.md`; `cargo xtask l4-wasi`: 7,211 records identical (§A4, §A6) |
| the default configuration's degradations recorded (L4d), none `xfail` | **met** | 416 pass + 69 degraded, each with its kind and detail (§A7) |
| B2–B4, B8, B13, B1′ and B12 measured and met, or restated with the owner | **met** (B3 restated to ≤ 5.5 KB gz, owner 2026-09-22) | §A11 |
| locale-output goldens identical on both targets for the Rust backends | **met** | four families, 5,915 golden cases through `l4-wasi` (§A8) |
| L4 on generated input (1,000,000) and a ≥ 1 h `format` fuzz run clean | **met** | §A9 |
| the API additions in 03 §2 | **met** | 03 §2.7, all additive (§A1) |
| the `intl` probe reported, decision 4 recorded; adopted, so L4 in Chromium, Firefox and WebKit with every engine difference in the ledger | **met** | §A0, §"The `intl` option as built"; `cargo xtask l4-web` 324/324 in each engine |
| the Phase 5a and 5b work orders written | **written** | [12-phase-5a-work-order](12-phase-5a-work-order.md), [13-phase-5b-work-order](13-phase-5b-work-order.md) |

## A0 — the `intl` probe (owner decision 4)


`bench/intl-probe` (kept as the A/B baseline; `RESULTS.md` has every
command) built the runtime for `wasm32-unknown-unknown` with `Intl`-backed
numeric handlers against the Rust path, in Chromium 143 and Firefox 155
(WebKit could not start then; it runs since 2026-09-22):

| Item | Rust path | `intl` |
|---|---|---|
| Size, wasm + JS gz over the same harness: core numbers | 5,382 | 7,541 (774 of it JS) |
| … + locale symbols and `:percent` | 7,103 | 7,557 |
| … + `:currency` / `:unit` | ≈ 10,038 *(estimate)* | 8,280 |
| Time per numeric placeholder | — | 2–6× slower (+2–10 µs; +10–40 µs at 4× CPU throttle in Chromium) |
| P0.5's 100,000 ECMA-402 cases | 95,675 / 0 | 95,675 / 0 in both engines; = the Rust path on all 100,000 |
| The panel's 4,004 localized cases | — | identical |
| CLDR plural samples (15,041) | 15,041 | Chromium 12,281 / 12,396 cardinal, Firefox 12,111: locales an engine lacks answered with `en-US` rules, silently |
| `Intl.NumberFormat` v3 floor | — | every option present in both engines |
| L4 number files | 95/95 | 95/95 in each engine |

The probe also found a Rust bug: `:offset` gave a zero addend's sum the sign
of zero (`{0 :offset subtract=5}` → `5`), fixed in `770ed68` with a
regression test over both digit backends. **Owner decision 4 (2026-09-22):
adopted as an opt-in client feature, off by default, requiring
`Intl.NumberFormat` v3** (D4; 03 §5.3).

## The `intl` option as built (owner decision 4)


`mf2-runtime/intl`, `mf2-fn-number/intl`, `mf2-host-web/intl` (`NUMBERS_HOST`),
and the facade's `intl`: on `wasm32-unknown-unknown` only, the numeric
functions — core and localized, `:percent`, `:currency`, `:unit`, unannotated
numbers — take the display, `:integer`'s rounding and the plural category
from `Host::numbers()` (the browser's `Intl.NumberFormat` and
`Intl.PluralRules`); option errors, operand rules, exact-match keys and
`:offset` stay in Rust (03 §2.7, §5.3). Everywhere else nothing changes: L4,
L4d and `l4-wasi` as before.

| Figure | Value | Command |
|---|---|---|
| Size, wasm + JS gz: core numbers (Rust → intl) | 5,408 → 7,613 (**+2,205**) | `bench/intl-probe/scripts/build.sh` |
| … with `fn-number` | 7,215 → 7,604 (**+389**) | same |
| … with `:currency` + `:unit` | 12,659 → 9,016 (**−3,643**) | same |
| JS glue | 983 B gz | same |
| Speed per numeric placeholder or select | 1.7–6× the Rust path, +1.1 to +7.1 µs (+4 to +27 µs at 4× throttle, Chromium) | `scripts/2-speed.sh`, three alternated runs |
| L4 in Chromium 143, Firefox 155, WebKit 26.6 | **324 / 324** each; no suite test formats otherwise than the Rust path | `cargo xtask l4-web` |
| Golden differences (recorded in the ledger's `[[intl]]` tables) | bidi marks as separate `literal` parts in `ar`, `ar-EG`, `he` negatives (234 cases, every engine); `ar-EG` `currencyDisplay=never` keeps a U+200F (9); Chromium writes English Welsh currency and unit names (72) | same |
| WebKit's `Intl.PluralRules` | 11,952 / 12,396 cardinal, 2,595 / 2,645 ordinal CLDR samples — `en-US` rules for 33 locales it lacks | `engines.sh floor plural` |
| B1 cost of the `Host::numbers` slot on every build | +17 B raw / **+15 B gz** | `bench/b12/check.sh` |
| B1′ with `intl` on and no number used | −34 B raw / −69 B gz against `runtime-nonum` (gate: ≤ +0) | same |

B12 is clean in the three new harnesses (`runtime-intl`,
`runtime-fn-number-intl`, `runtime-intl-unused`), and no Rust rounding,
display or plural-evaluator symbol is linked with `intl`. Full tables:
`bench/intl-probe/RESULTS.md` §8, 06 §3.

## A1 — API additions


Written into [03](03-runtime.md) §2.7 before code; all additive to the API
frozen at Phase 3 (date/time values and `Arg::DateTime` by reference, so
`Arg` stays 24 bytes; the context's `TimeZone`; `Host::zone_offset` and
`Host::format_date_time` with defaults; the numeric core's public surface —
`NumberSpec`, `Number::resolve`, `Digits`, `Measure`; `Registry::with_numbers`
/ `with_dates`; later `Value::digit_size`). A first cut formatted unannotated
dates as ISO 8601 in the core, which cost every client ~350 B gz of B1; they
go through `with_dates` instead. B1's runtime part after the phase's
changes: **18,849 B gz** (Phase 3: 18,428), numeric share 5,386 B gz
(`bash bench/b12/check.sh`, `runtime` and `numbers` rows).

## A2 — number data


`cargo xtask locale-data` writes `data/numbers.txt` (766 locales, each as
its differences from its CLDR parent; regeneration re-resolves every locale
and must equal CLDR's resolved files). Two LOCALE kinds, v1: key 3
`number.symbols`, key 4 `number.patterns` (02 §4.2–§4.5, byte-exact
vectors). Departures written into the plans: no `number.systems` entry (the
pinned spec has no `numberingSystem` option), no per-mille / exponent /
infinity / NaN symbols (no MF2 function produces them), currency spacing a
constant rule (identical in all 910 CLDR records). **B8** (plural +
`number.symbols` + the percent pattern, gzip -9 of the entries) on the
panel: **47–85 B gz** per locale, limit 512 (`cargo test -p
mf2-locale-data --test numbers b8 -- --nocapture`).

## A3 — `mf2-fn-number`


The localization layer over the core's rounded digits (symbols, grouping
`auto` / `always` / `min2` / `never` with the locale's minimum grouping
digits, the numbering system's digits, the percent pattern), `:percent`,
and unannotated numbers through `Registry::with_numbers`. `functions/percent.json`
and `syntax.json` #90 green at L4. The localized differential (`runtime-bench
numbers locale 100000 | node bench/runtime-bench/loc-diff.cjs`): P0.5's
corpus over the panel and two non-Latin numbering systems against node's
`Intl.NumberFormat`: **95,675 identical, 0 different**; the 4,325 option
sets `Intl` rejects are errors here too.

## A4 — `:currency` and `:unit`


L4 485/485 in the all-features configuration since A4: `functions/currency.json`
(12) and `conformance/extra/functions/unit.json` (23, ours, WG schema) moved
`xfail` → `pass` (`cargo test -p mf2-conformance --test layers`). The panel
goldens (`conformance/goldens/currency.tsv`, 1,170 cases; `units.tsv`, 1,053)
were compared before committing with node's `Intl.NumberFormat`: **2,106 of
2,106** comparable cases identical (the 117 `currencyDisplay=never` cases have
no `Intl` counterpart); `cargo xtask l4-wasi` formats all 4,095 golden cases
identically on native and `wasm32-wasip1`.

## A5 — `mf2-fn-datetime` semantics


`:datetime`, `:date`, `:time` once, in Rust (03 §5.2, §5.4): operands (date/time
values, `CustomValue`, strings matching the spec's literal exactly), every
option with the literal-only rule, the override options and their
inheritance, the context's time zone, conversions through `Host::zone_offset`
(a floating wall time placed in a named zone: the earlier instant in an
overlap, forward in a gap), unannotated values through `with_dates`. The 20
tests of `functions/{date,time,datetime}.json` green at L4 over the neutral
stub backend. **Owner decision 1 (2026-09-21):** the server's tz database is
`jiff` with its bundled IANA data, behind `Host::zone_offset`.

## A6 — date backends


`datetime-icu` (ICU4X 2.3 over the catalog's `icu.blob`, key 48) and
`datetime-intl` (`Intl.DateTimeFormat` through `mf2-host-web`'s
`INTL_HOST`); named zones from `jiff`'s bundled database on the server
(owner decision 1) and from the browser's own zone data in the client.
The blob is sliced to what the corpus formats (a new `DateNeeds` rule over
each date expression's literal options, `hour12` values and calendars; a
variable calendar brings every calendar, as a variable currency does; a
placeholder that can receive a date gets `:datetime`'s defaults, as
`number.symbols` does for #90), and `generated_l4` checks that a sliced
blob formats exactly as a full one. Entry and vectors: 02 §4.9; slicing:
02 §4.4.

| Figure | Value | Command |
|---|---|---|
| L4, the date files, with `datetime-icu` | green (real ICU4X text); L4 485/485 and L4d 416 + 69 unchanged | `cargo test -p mf2-conformance` |
| Native ≡ `wasm32-wasip1` | **7,211 records identical** (suite, both configurations, and every golden case) | `cargo xtask l4-wasi` |
| `icu.blob` per locale, gzip -9: every shape, no zone names | **0.61–0.91 KB** (`th`, any calendar: 1.05) | `cargo test -p mf2-locale-data --features icu-blob --test icu_blob sizes -- --nocapture` |
| … `:datetime` defaults only | 0.28–0.45 KB | same |
| … with zone styles | 18.3–20.9 KB (limit ≤ 25) | same |
| `datetime-intl` against ICU4X in three engines | 18/18 checks in Chromium 143, Firefox 155, WebKit 26.6; the suite's date files 20/20 in each | `node tools/e2e/run.mjs datetime --browser all` |
| … the panel's 187 exactly-mapped cases per engine (identical / within P0.10's tolerance / named divergence) | 143/20/24 Chromium, 164/20/3 Firefox, 162/20/5 WebKit | same |
| Date placeholder, native | 2.7–4.5 µs; 38 µs with a zone style (the blob is copied into a provider and the formatter built per format) — no budget covers it | `cargo run --release -p runtime-bench --example date_cost` |

Two limits are written down rather than worked around: a damaged `icu.blob`
can make ICU4X panic (a debug assertion, and `DecimalFormatter::try_new` →
`DataMarkerAttributes::from_str_or_panic` in release too), so the fuzz
target formats dates with the neutral backend in catalog mode and 02 §4.9
says a damaged blob is a damaged deployment; and `hour12` on a time alone
maps to components, because V8 and WebKit mangle `hour12` in 24-hour
locales.

## A7 — L4d, the default configuration


`mf2_l4_runner::DEFAULT_REGISTRY` is what an application generates with
`fn-number` and `fn-datetime` off; `l4::check_default` classifies each run as
a pass or a documented degradation (`unknown-function`,
`unsupported-operation`, and the new `neutral-numbers`), and the ledger must
name exactly that. **L4d: 416 pass, 69 degraded** (the 68 tests of the
gated functions' files `unknown-function`, `syntax.json` #90
`neutral-numbers`), none `xfail`; `cargo xtask l4-wasi` compares both
configurations on `wasm32-wasip1`.

## A8 — goldens


`conformance/goldens/{numbers,currency,units}.tsv`: 1,872 + 1,170 + 1,053
cases over the 13-tag panel (the 11 locales, `ar-EG`, `hi-u-nu-deva`),
invisible characters escaped; `tests/goldens.rs` requires them to equal a
fresh render, `cargo xtask l4-wasi` formats every case on `wasm32-wasip1`
(4,095 golden records identical). `dates.tsv` joins with A6: 20 messages ×
7 values × 13 tags = 1,820 cases, none with an error. Of the 1,456 with an
exact node `Intl` counterpart, 1,099 are identical and 135 within P0.10's
tolerance; the other 222 are explained and none is our defect — 135 dates
before the common era (ICU4X writes the era, node does not), 39 Spanish
two-digit hours (CLDR 48 `HH` vs 46 `H`), 48 CLDR-version or engine
differences (Hebrew month numbering, bidi marks in the GMT format, Spanish
"a las" glue, Welsh AM/PM, Polish short dates).

## A9 — generated input and fuzzing (steering)


`l4gen` and the `format` fuzz target reach every new function, option and
panel locale, date/time arguments, and the default configuration (fuzz
flags bit 1; 1,606 seeds from `cargo xtask fuzz-seed`). 100,000 generated
cases clean (5,872 calling `:percent`, 11,000 `:currency` or `:unit`,
15,908 a date/time function). With A6 the steering also reaches
daylight-saving gaps and overlaps, more zones and the Hebrew calendar
(formatted dates in 3,000 generated cases: 54 → 71), and every generated
catalog carries the sliced blob, checked against the full one. A 15-minute
`format` smoke run was clean (919,558 runs, cov 21,920); it found three
problems, all fixed: building every locale's blob up front hit libFuzzer's
10 s timeout (blobs are now built on first use), the linear-time clock
covered build-side compilation (it now covers catalog load and formatting
only), and ICU4X's panic on a damaged blob (above). The exit runs are
below.

### The exit runs

**1,000,000 generated L4 cases clean** (`MF2_GEN_CASES=1000000 cargo test
--release -p mf2-conformance --test generated_l4 -- --nocapture`, 445 s):
745,590 valid messages, 527,806 formatted without errors, 298,303 calling
`:number` or `:integer`, 249,142 selections, 57,252 `:percent`, 109,540
`:currency` or `:unit`, 157,808 a date/time function, 27,280 with a
formatted date/time, and 201,707 carrying an `icu.blob` sliced to the
message's shapes and checked against the full one. Nine error kinds were
reached (bad-operand, bad-option, bad-selector, bad-variant-key,
message-function-error, missing-fallback-variant, unknown-function,
unresolved-variable, unsupported-operation).

**A ≥ 1 h `format` fuzz run clean on the final code**: `cargo +nightly fuzz
run format -- -dict=mf2.dict -max_len=131072 -timeout=10 -rss_limit_mb=2048
-max_total_time=3900` — **2,145,733 executions in 3,901 s**, no crash, no
timeout, no leak (13:40–14:45 on 2026-09-22, the machine otherwise quiet).
The two crash artifacts the phase's earlier smoke runs had left
(`fuzz/artifacts/format/`, 2026-09-21 19:43 and 19:45 — the libFuzzer
timeout from building every locale's blob up front, and ICU4X's panic on a
damaged blob) replay clean on this code.

## A10 — numeric speed

The two candidates of the work order, each A/B-measured by alternating the
two binaries on `runtime-bench b10 --only select` (the reference workload's
select messages; 31 samples a row inside each run) on a machine under the
owner's own load, its clock drifting 1.2–3.6 GHz between runs. Medians and
minima over the alternated runs:

| Change | `en` median / min (ns) | `pl` median / min (ns) | Δ size, B1's runtime part |
|---|---|---|---|
| baseline (the merged tree) | 600 / 452 | 948 / 723 | — |
| **the plural category once per selector** (12 runs each) | **562 / 480** | **736 / 624** | **+24 B gz** |
| … and an integer path in `shown` (10 runs each, against the row above) | 632 / 446 *(cache alone 750 / 463)* | 835 / 587 *(987 / 602)* | +25 B gz more |
| baseline against both (15 runs each) | 576 / 458 → 638 / 447 | 914 / 723 → 843 / 589 | +49 B gz |

**Kept: the category once per selector** — a `Cell<u8>` on the resolved
number's display (the `intl` backend already had one), so a selector
evaluates the locale's plural rules once instead of once per literal key.
It takes 14–22 % off a `pl` select (three plural keys) and leaves `en`
within the noise, because `en`'s select messages have one literal key and a
catch-all: the catch-all never asks for a category, so there is nothing to
reuse. 24 B gz for that is worth it.

**Dropped: the integer path** (skip rounding in `shown` when the value is
already an integer and the plan is the default fraction one). Its gain —
about 3 % on the minima, medians contradicting — never came out of the
noise on this machine, and it costs another 25 B gz, so by A10's rule
("kept only if faster") it is not kept. The code is in this commit's
message for whoever measures it on a quiet machine.

`cargo run --release -p runtime-bench --example select_cost` with the cache,
one sample each (ns per format): text only 29, `{$n}` unannotated 162,
`{$n :integer}` 363, `.input :integer` + a placeholder 550, a select with no
placeholder 511, select + placeholder 657, an exact key 606, `pl` select +
placeholder 780.

**Against P0.8's 317 ns** (measured on P0.3's integer-only runtime, itself
under load 2–5): `en` select is 1.45× at this machine's best clock and 1.8×
at its worst — the 1.5× criterion straddles the clock, and the figure is
reported, not gated, in `bench/runtime-bench` and the B10 gate. Owner
decision 3 of the work order.

## A11 — budgets

### B2, B3, B1′ — the numeric family


`bash bench/b12/check.sh` (wasm-release, `wasm-opt -Oz`, `gzip -9 -n`; each
figure a delta between two harnesses that differ only in their registry):

| Budget | Harnesses | Δ raw | **Δ gz** | Limit |
|---|---|---:|---:|---|
| B2: `fn-number` on and used (localized `:number` / `:integer` / `:offset`, `:percent`, unannotated numbers) | `runtime-fn-number` − `runtime` | 4,062 | **2,034** | ≤ 3,072 |
| B3: + `:currency`, `:unit` | `runtime-fn-number-measure` − `runtime-fn-number` | 11,779 | **5,454** | ≤ 5,632 (restated; was 4,096) |
| B1′: `fn-number` linked, unused | `runtime-fn-number-unused` − `runtime` | 0 | **0** | +0 |

(Figures on the merged tree; B2 and B3 were 2,069 and 5,432 before the
`intl` option's numeric rework and the `Host::numbers` slot moved the
`runtime` base by +15 B gz.)

B12 holds for all three (no panic import after LTO + `wasm-opt -Oz`, no
`core::fmt` symbol). **B3 as first built measured 6,048 B gz.** A twiggy diff
of the two harnesses (`twiggy top` on the `wasm-syms` builds) put it in the
two functions' option loops, their writers and the catalog views' record
readers; what was duplicated or over-general came out without changing any
output — one options loop for both functions over keyword tables, one
`fill` over `dyn FnMut` arguments instead of five monomorphized copies,
blanks split by byte instead of Unicode `is_whitespace` / `trim`, the currency
spacing decided once, the runtime's digit-size parser (`Value::digit_size`) —
6,048 → 5,432 (B2 2,151 → 2,069 with it). **The owner restated B3 to
≤ 5.5 KB gz** (decision 5 of the work order; 06 §3): P0.5's 2.9 KB probe had a
plural stub and `en-US` data only, while the functions as built carry
plural-form currency and unit names, the `…alphaNextToNumber` patterns and
currency spacing, currency-specific patterns and separators, accounting and
narrow symbols, unit-width fallback and `X-per-Y` composition. The option not
taken — the catalog writer precomputing each currency's affixes per display
and sign style and composing literal compound units — was estimated at −0.9
to −1.4 KB gz for a larger catalog and literal-only compound units.

### B4 — the date backends


`bash bench/b12/check.sh`, deltas in B gz (harnesses and method in
`bench/b12/README.md`):

| Budget | Measured | Limit |
|---|---:|---|
| `datetime-icu`, Gregorian, with zone styles | **69,641** (no zone styles: 42,820) | ≤ 97,280 |
| `datetime-icu`, any calendar, with zone styles | **83,028** (no zone styles: 55,476) | ≤ 107,520 |
| `datetime-intl`, wasm | **5,131** | ≤ 6,144 |
| `datetime-intl`, JS glue | **668** | ≤ 1,024 |
| The date semantics every backend needs | **3,577** (7,600 raw) | ≤ 3,584 |
| B1′: `fn-datetime` on, unused | **0** | +0 |
| B1′: `mf2-host-web`'s date features on, `HOST` named | **0** wasm, 0 JS | +0 |

B12 is clean for the date semantics, the neutral backend and the unused
crate; for the ICU4X harnesses it is reported, not gated — ICU4X keeps its
own `core::fmt` and panic paths, the feature's documented cost (06 B4).
B13: no date symbol is linked without a date function. The shared date
semantics missed their 3.5 KB gz note by 4 B when the date work was measured
on its own — a smaller fraction parser took off 51 B gz, a shared option
table would have added 46 and was dropped — and came out **11 B under** it
on the merged tree, where the `intl` option's numeric rework sits beneath
them. No budget was restated for dates.

