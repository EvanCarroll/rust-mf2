# P0.5 — numbers: core semantics over `fixed_decimal`, `fn-number` layer, `:currency`/`:unit`

Probe for `plans/06-size-and-perf.md` §5 P0.5; sets the numeric share of **B1**,
**B2**, **B3**, **B8**, and checks **B12**. Throwaway code.

## Question

1. Can the complete `:number` / `:integer` / `:offset` semantics (every REQUIRED
   option, all rounding modes, increments, rounding priority, trailing zeros,
   sign display, option inheritance, `select` = exact/plural operands) be written
   over `fixed_decimal`, `no_std`, fmt-free, panic-free — and what do they cost
   in wasm (plan: ≤ 6 KB gz)?
2. What does the `fn-number` localization layer (symbols, grouping, digits,
   `:percent`) cost with its data in the catalog (B2 ≤ 15 KB gz *estimate*),
   and how big is that data per locale (B8 ≤ 2 KB gz)?
3. What do `:currency` + `:unit` add (B3 ≤ 5 KB gz *estimate*), and their data
   for "used" vs "all" codes?
4. For comparison: `icu_decimal` + blob, and `Intl.NumberFormat` glue.

## What was built

| Crate | Role |
|---|---|
| `numcore/` | Core semantics, `no_std`, `forbid(unsafe_code)`, clippy `unwrap/expect/indexing/panic` denied: strict `number-literal` parsing, operands `Str`/`Int`/(`f64` feature) `Float`/resolved value; table-driven option resolution; ECMA-402 digit options (`SetNumberFormatDigitOptions` / `FormatNumericToString`) incl. `roundingPriority`, `roundingIncrement` (all 15 values), 9 rounding modes, `trailingZeroDisplay`, `minimumIntegerDigits`, `signDisplay` (incl. −0); option inheritance and the `:integer` / `:percent` discard lists; literal-only `select`, inherited `select` → *Bad Option* + no selection; `:offset` (`add`/`subtract`, exact decimal addition); neutral writer; exact-match serialization; CLDR plural operands (i v w f t c) of the **formatted** digits. A `Spec` per function (closed world: `:percent`/`:currency`/`:unit` reuse the machinery). |
| `numloc/` | `fn-number` layer, same discipline: reads LOCALE entries (container `tag u8, len u16, payload`), locale symbols, primary/secondary grouping, minimum grouping digits, `useGrouping` auto/always/min2/never, native digits, number patterns (affixes, explicit negative sub-patterns, `alphaNextToNumber`), `:percent`; features `currency` (`currency`, `currencySign`, `currencyDisplay` ×5, `fractionDigits`, not selectable) and `unit` (`unit`, `unitDisplay`; `usage` → *Unsupported Operation*). |
| `locdata/` | Build side: slices `third_party/cldr-json` (48.2.1) into those entries per locale / entry group. |
| `suite/` | Dev harness: a minimal MF2 parser + evaluator (declarations, `.match` with the spec's ranking, fallbacks, errors) running WG suite files through `numcore` (neutral) or `numloc` (`--loc`); `ecma_cases` emits random option sets for the ECMA-402 differential. en plural rules are a harness stub (the evaluator is P0.4's). |
| `diff/` | `numloc` vs `icu_decimal` (compiled CLDR 48.2.1), and `:percent`/`:currency`/`:unit` cases for node `Intl`. |
| `wasm/w-*` | cdylibs, one exported `fmt(data, locale, func, opts, operand, key) -> String` each (wasm-bindgen); `w-base` does no numeric work but returns an allocated `String` (dlmalloc and std's panic runtime are in the base — checked with twiggy). |
| `wasm/b12-numcore` | B12 harness: `no_std`, bump allocator, panic handler calls an imported `probe.panic_reached` and ignores `PanicInfo`. |

## Commands

From `probes/p0-05-numbers/`, stable 1.98.1 via the repo `rust-toolchain.toml`,
`CARGO_BUILD_JOBS=3`: **`scripts/all.sh`** (writes `out/*.txt`), which runs:

```sh
cargo test -p numcore
cargo run -p suite --bin suite -- $T/number.json $T/integer.json $T/offset.json   # T = third_party/.../test/tests/functions
cargo run --release -p suite --bin ecma_cases -- 100000 | node scripts/ecma-diff.cjs
cargo run --release -p locdata -- --cldr ../../third_party/cldr-json --out out/locale
scripts/datasizes.sh
cargo run -p suite --bin suite -- --loc out/locale/en.all-used.bin $T/{number,integer,offset,percent,currency}.json
cargo run --release -p diff --bin diff
cargo run --release -p diff --bin loc_cases | node scripts/intl-loc.cjs
scripts/measure.sh base core core-f64 loc loc-cu icu-decimal-blob intl-num
scripts/b12.sh
```

`scripts/measure.sh` = `cargo build --target wasm32-unknown-unknown --profile
wasm-release` (opt-level `z`, fat LTO, 1 CGU, `panic=abort`, `strip`) →
`wasm-bindgen --target web` → `wasm-opt -Oz` → `gzip -9`. Tools: wasm-opt 120,
wasm-bindgen 0.2.128, twiggy 0.8.0, node 23.11 (ICU 76.1 / CLDR 46). Crates:
fixed_decimal 0.7.2, icu_decimal 2.3.0.

## Numbers

### Step 1 — core semantics: correctness

* **Suite, neutral output: 70/70** — `number.json` 41/41, `integer.json` 13/13,
  `offset.json` 16/16 (`exp` where given, `expErrors` always; absent = none).
  **No test in these three files needs locale data**: every `exp` is ASCII
  digits < 1000 and the one plural-keyword test is decided by an exact key.
* **ECMA-402 differential** (100,000 random value × option sets vs
  `Intl.NumberFormat('en', {useGrouping:false, …})`, exact decimal strings):
  **95,675 identical, 0 different**; the 4,325 sets Intl rejects (RangeError)
  are exactly the ones `numcore` reports as *Bad Option*. (One bug found and
  fixed on the way: `morePrecision`/`lessPrecision` compare rounding magnitudes
  strictly.)
* Plural operands from the formatted number (`1.50`→ i=1 v=1 w=1 f=5 t=5,
  `1.20` with 2 fraction digits → v=2 f=20 t=2) unit-tested; operands keep 18
  low digits, avoiding `checked_mul` (which pulls `__multi3` on wasm32).

### Step 1 — core semantics: size

| Build | raw | gz | Δ raw | **Δ gz** |
|---|---:|---:|---:|---:|
| `w-base` (std + wasm-bindgen, allocator + panic runtime live) | 13,664 | 6,208 | — | — |
| `w-core`: parse + resolve + format + selection path | 35,337 | 16,441 | +21,673 | **+10,233** |
| `w-core-f64`: + `Float(f64)` operands (`fixed_decimal/ryu`) | 39,897 | 19,273 | +26,233 | **+13,065** |
| `b12-numcore` (`no_std`, absolute, incl. bump allocator + harness) | 17,117 | 8,450 | | |
| `b12-numcore` + f64 (absolute) | 21,715 | 11,392 | | |

Attribution (`no_std`, gz): formatting ≈ 2.9 KB, selection path (exact match +
plural operands) ≈ 0.7 KB, `:integer`+`:offset` ≈ 0.8 KB, the rest parse +
options + `fixed_decimal` (`try_from_utf8` 1.3 KB raw, increment rounding
1.9 KB raw, `SmallVec` growth). In the std build, `fixed_decimal`/`smallvec`
panic sites drag std's panic formatting in: core::fmt symbols 7 → 22
(+3.6 KB raw) versus the base.

### Step 1 — B12

`no_std` harness: **0 `core::fmt` symbols, 0 panic-message or source-path
strings** — but the `panic_reached` import **survives** LTO + wasm-opt: six
panic entry points (`panic_bounds_check`, `unwrap_failed`, `expect_failed`,
`panic`, `slice_index_fail`, `panic_nounwind_fmt`), all traced (out-of-line
attribution build, feature `noinline`) to **`fixed_decimal` 0.7.2 and its
`smallvec`** (bounds checks inlined into `resolve`/`add_small`, `SmallVec`
growth `unwrap`/`expect`, slicing in `Decimal::try_from_utf8`). `numcore` and
`numloc` themselves pass clippy's unwrap/expect/indexing/panic denials.

### Step 2 — `fn-number` layer

| Build | raw | gz | Δ vs `w-core` raw | **Δ gz** |
|---|---:|---:|---:|---:|
| `w-loc`: symbols, grouping, digits, `:percent` | 38,851 | 18,169 | +3,514 | **+1,728** |
| `w-loc-cu`: + `:currency` + `:unit` | 45,373 | 21,104 | +10,036 | +4,663 (**+2,935** over `w-loc`) |

Correctness:
* Suite with fn-number on (en-US data): **95/95** — the 70 above plus
  `percent.json` 13/13 (incl. `one`/`other` selection on ×100) and
  `currency.json` 12/12 (incl. `bad-selector` on a currency value).
* **vs `icu_decimal`** (same rounded digits, 236 values × 4 grouping
  strategies, 11 locales + `ar-u-nu-arab`, `hi-u-nu-deva`): **12,228 identical,
  44 different** — all 44 are `useGrouping=always` in es/pl (minimum grouping 2)
  on 4-digit integers: we print `1.234`, as ECMA-402 does (node confirms), while
  ICU4X documents `GroupingStrategy::Always` as "same behavior as Auto" — i.e.
  `icu_decimal` cannot implement MF2's `always`.
* **vs node `Intl`** (CLDR 46): `:percent` **198/198** identical;
  `:currency` 691/693; `:unit` 379/396 — every difference is the harness's
  plural stub (es `1,00 euro`, ru/pl/ar/he/hi count forms) or a CLDR 46→48 data
  change (cy `kg`→`cilogram`, narrow `°`→`°C`).
* wasm smoke test (`scripts/wasm-smoke.cjs`): `w-loc-cu` and `w-intl-num` agree
  on 12/12 decimal/percent samples incl. hi Indian grouping, `ar-u-nu-arab`,
  pl/es minimum grouping.

### Per-locale data (raw / standalone gzip -9 bytes; `out/datasizes.txt`)

| loc | `number.symbols` | + percent pattern | currency patterns | currencies used (USD EUR JPY GBP) | currencies all | units used (5 × 3 widths) | units all (~230 × 3) |
|---|---:|---:|---:|---:|---:|---:|---:|
| en | 31 / 63 | 40 / 64 | 88 / 105 | 190 / 163 | 23,620 / 8,084 | 362 / 205 | 18,913 / 5,895 |
| es | 31 / 63 | 42 / 66 | 69 / 80 | 242 / 186 | 20,431 / 7,235 | 333 / 215 | 16,968 / 5,651 |
| de | 31 / 63 | 42 / 66 | 69 / 80 | 186 / 151 | 23,414 / 7,343 | 327 / 195 | 16,227 / 5,073 |
| fr | 33 / 64 | 44 / 67 | 85 / 88 | 238 / 181 | 22,322 / 7,672 | 404 / 238 | 19,122 / 5,912 |
| ar (+ arab system) | 117 / 124 | 126 / 125 | 124 / 115 | 223 / 169 | 21,350 / 5,989 | 1,273 / 363 | 70,012 / 9,193 |
| he | 37 / 67 | 46 / 68 | 164 / 95 | 199 / 155 | 16,547 / 4,974 | 763 / 305 | 29,884 / 6,661 |
| ja | 31 / 63 | 40 / 64 | 78 / 100 | 145 / 145 | 22,373 / 6,514 | 234 / 201 | 11,705 / 4,363 |
| hi (+ deva system) | 65 / 74 | 74 / 75 | 65 / 87 | 327 / 201 | 20,317 / 4,683 | 707 / 286 | 29,370 / 5,956 |
| ru | 45 / 80 | 56 / 82 | 69 / 80 | 585 / 260 | 44,281 / 9,639 | 921 / 321 | 46,414 / 9,002 |
| pl | 32 / 64 | 41 / 65 | 85 / 88 | 352 / 224 | 27,992 / 8,836 | 583 / 270 | 27,690 / 7,162 |
| cy | 31 / 63 | 40 / 64 | 79 / 101 | 348 / 204 | 39,517 / 9,164 | 697 / 255 | 32,798 / 8,427 |

B8 (plural + number symbols): with the CLDR plural rule text as an upper-bound
proxy for P0.4's encoding, symbols + percent + rules = **40–231 B raw,
59–173 B gz** per locale (standalone gzip; inside a compressed catalog less).

### Step 4 — comparisons

| Localization backend on top of `w-core` | Δ raw | Δ gz | JS Δ gz |
|---|---:|---:|---:|
| `numloc` (+ `:percent`) | +3,514 | **+1,728** | 0 |
| `Intl.NumberFormat` glue (digits pinned, `style:percent`) | +3,981 | **+1,597** | +614 |
| `icu_decimal` + blob provider (decimal only) | +35,759 | **+15,885** | +23 |

`icu_decimal`'s data per locale (from P0.6's blob breakdown): `DecimalSymbolsV1`
≈ 87 B gz + `DecimalDigitsV1` ≈ 0.86 KB gz (all systems).

## Conclusions against thresholds

| Budget | Plan | Measured | Verdict |
|---|---|---|---|
| **B1 numeric share** | ≤ 6 KB gz | **10.2 KB gz** in a std/wasm-bindgen app (8.5 KB gz `no_std` absolute); +2.8 KB gz with `f64` operands | **not met** |
| **B2** fn-number incl. `:percent` | ≤ 15 KB gz *(estimate)* | **1.7 KB gz** | **met** |
| **B3** `:currency` + `:unit` | ≤ 5 KB gz more *(estimate)* | **2.9 KB gz** | **met** |
| **B8** plural + number symbols | ≤ 2 KB gz / locale | ≤ 0.2 KB gz | **met** |
| **B12** | `core::fmt` absent | our crates: absent (0 symbols, `no_std`); **`fixed_decimal`/`smallvec` keep panic paths reachable**, which in a std wasm add ≈ 3.6 KB raw of fmt | **met for our code; not met for the dependency in std builds** |

**Recommended budget values** (C2): B1 numeric share **≤ 10 KB gz** (or ≤ 8 KB
gz after the options below); B2 **≤ 3 KB gz**; B3 **≤ 4 KB gz**; B8 **≤ 0.5 KB
gz**; currency/unit data: "used" ≤ 0.4 KB gz, "all" ≈ 5–10 KB gz per locale
each (keep the plan's "used by default" policy — 02 §4).

Ways to close the B1 gap, not implemented: `Float(f64)` via the `Host`
(JavaScript's `String(x)` is the shortest round-trip, which our parser accepts)
instead of `ryu` on the client (−2.8 KB gz); build-time interning of option
names/values (≈ −0.5 KB gz); a `fixed_decimal` without panicking paths (upstream
patch, or our own digit buffer — which per D1's rule needs a baseline, gate and
fallback).

## Caveats and gaps

* **A4 gap**: `cldr-core/supplemental/numberingSystems.json` and
  `currencyData.json` are not vendored. Digits of native systems came from
  ICU4X's compiled data (same CLDR 48.2.1); currency fraction digits use a
  20-entry placeholder table in `locdata` (default 2). Both belong in
  `cargo xtask cldr-sync`.
* Plural categories in the harness are stubs (en exact; others approximate);
  the P0.4 evaluator slots into the `category` callback.
* Not implemented: `:unit` compound units beyond CLDR's precomposed ids, unit
  `usage` conversion (optional; *Unsupported Operation*), currency cash digits,
  compact/scientific notation (not MF2 options).
* `:offset` passes an inherited `select` on without error (implementation
  choice; the spec's operand-`select` rule is written for selectors).
* `:integer` rounds half-expand; values > 18 significant digits give
  *Unsupported Operation* in `:offset` (spec-permitted limit).
