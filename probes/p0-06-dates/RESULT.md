# P0.6 — dates: `icu_datetime` + blob vs `Intl.DateTimeFormat` glue

Probe for `plans/06-size-and-perf.md` §5 P0.6; sets budget **B4**. Throwaway code.

## Question

What does the MF2 date family (`:datetime`, `:date`, `:time`, Draft, spec at the
pin `5c4ddb27`) cost in the client wasm with each backend of decision D4 —
(a) ICU4X `icu_datetime` with its data in a per-locale runtime blob, (b) the
browser's ECMA-402 `Intl.DateTimeFormat` through `js-sys` — and how large is the
per-locale ICU4X blob when restricted to the markers the code needs? Audit
figures to confirm or replace: `datetime-intl` ≤ 3 KB gz (*estimate*),
`datetime-icu` ≈ 100 KB gz (*audit*).

## What was built

| Crate | Role |
|---|---|
| `mf2dt/` | Backend-independent semantics, `no_std`, `forbid(unsafe_code)`, fmt-free, panic-free: date/time literal parsing (the spec's regex, whole string, impossible days rejected), option resolution for all three functions (literal-only rule → *Bad Option* + ignore; override options `timeZone` / `hour12` / `calendar` and their inheritance from a date operand; unknown options ignored), `timeZone` semantics (`input`, `UTC`, `±hh:mm`, IANA name; floating vs. offset operands), days↔civil arithmetic. Unit tests. |
| `dt-icu/` | `datetime-icu` backend: MF2 plan → ICU4X `FieldSetBuilder` (runtime semantic skeleton) → `FixedCalendarDateTimeFormatter<Gregorian>` or `DateTimeFormatter` (any calendar); `format_unchecked` with offset-only zones; data from a blob (`buffer`) or compiled (`compiled`). Includes `OneLocale`, a 20-line provider adapter that replaces ICU4X runtime locale fallback for a one-locale blob. |
| `dt-intl/` | `datetime-intl` backend: MF2 plan → ECMA-402 options → `new Intl.DateTimeFormat(…)` (constructor `RangeError` caught) → `format`. Plus `inline`, a leaner one-import variant (options as a hand-built JSON string). Both `no_std` + `forbid(unsafe_code)` — the `#[wasm_bindgen] extern` block compiles under `forbid`. |
| `wasm/w-*` | cdylib per variant, all with the **same** exported `fmt(blob, locale, func, opts, operand) -> String` (wasm-bindgen); `w-base` does no date work. |
| `wasm/b12-mf2dt` | B12 harness: `no_std` cdylib, no allocator, panic handler ignores `PanicInfo`. |
| `blobgen/` | Per-locale ICU4X blobs restricted to the markers a wasm binary requests (`icu::markers_for_bin`), exported from the ICU4X data crates' own baked CLDR 48.2.1 (made iterable with their `ITER` macros) — nothing downloaded. |
| `check/` | Native: the 20 suite cases of `functions/{date,time,datetime}.json`; blob ≡ compiled-data identity on an option matrix per locale; output sample. |

MF2 → ECMA-402 mapping (in `dt-intl/src/lib.rs`): `dateFields=year-month-day`
(or no date) with minute/second precision and no `timeZoneStyle` → `dateStyle`
= length, `timeStyle` = `short`/`medium` (CLDR's standard formats, which is what
the semantic skeletons resolve to); anything else → component options (`year`
numeric / 2-digit when short, `month` long/short/numeric, `day`, `weekday`,
`hour`, `minute`/`second` 2-digit, `timeZoneName` long/short); `hour12`,
`calendar`, `timeZone` pass through (`±hh:mm` offset ids need ES2024-level
engines).

## Commands

All from `probes/p0-06-dates/`, toolchain from the repo's `rust-toolchain.toml`
(stable 1.98.1), `CARGO_BUILD_JOBS=3`:

```sh
scripts/all.sh          # runs everything below and writes out/*.txt
# which is:
cargo test -p mf2dt
scripts/measure.sh base sem intl intl-inline icu-blob-fixed icu-blob-fixed-nozone \
    icu-blob-fixed-offz icu-blob-any icu-compiled-fixed icu-compiled-any
scripts/blobs.sh        # per-locale blobs (needs the wasm-release builds above)
scripts/b12.sh
cargo run -p check --release --bin check
wasm-bindgen --target nodejs --out-dir out/node/intl target/wasm32-unknown-unknown/wasm-release/w_intl.wasm
node scripts/intl-sample.cjs out/node/intl/w_intl.js
```

`scripts/measure.sh <variant>` = `cargo build -p w-<variant> --target
wasm32-unknown-unknown --profile wasm-release` (opt-level `z`, fat LTO, 1 CGU,
`panic=abort`, `strip`) → `wasm-bindgen --target web` → `wasm-opt -Oz` (rustc's
default wasm features enabled) → `gzip -9`. `PROFILE=wasm-syms` keeps names for
`twiggy`. Tools: wasm-opt 120, wasm-bindgen 0.2.128, twiggy 0.8.0; crates
icu_datetime 2.3.0 (CLDR 48.2.1), js-sys 0.3.105.

## Numbers

### Code size in wasm (delta against `w-base` = 15,493 B raw / 7,063 B gz)

| Variant | wasm raw | wasm gz | Δ raw | **Δ gz** | JS glue Δ gz |
|---|---:|---:|---:|---:|---:|
| `sem` — MF2 date semantics only (neutral output) | 21,668 | 10,427 | +6,175 | **+3,364** | 0 |
| `intl` — semantics + `Intl.DateTimeFormat` via js-sys | 26,726 | 12,501 | +11,233 | **+5,438** | +628 |
| `intl-inline` — semantics + one inline-JS import | 25,909 | 12,140 | +10,416 | **+5,077** | +240 |
| `icu-blob-fixed` — Gregorian, all MF2 options incl. `timeZoneStyle` | 236,549 | 100,088 | +221,056 | **+93,025** | +17 |
| `icu-blob-fixed-nozone` — Gregorian, no time-zone styles | 161,854 | 71,479 | +146,361 | **+64,416** | +29 |
| `icu-blob-fixed-offz` — as `fixed`, zones mapped to localized-offset styles | 236,552 | 100,088 | +221,059 | +93,025 | +24 |
| `icu-blob-any` — any calendar (honours `calendar`, locale default calendar) | 272,048 | 112,317 | +256,555 | **+105,254** | +13 |
| `icu-compiled-fixed` — compiled data, all locales | 3,432,082 | 1,078,464 | +3,416,589 | +1,071,401 | +26 |
| `icu-compiled-any` — compiled data, all locales | 3,868,842 | 1,196,137 | +3,853,349 | +1,189,074 | +22 |

Split of `intl`: semantics 3.36 KB gz + `Intl` glue 2.07 KB gz (+0.63 KB gz of
JS). `mf2dt` alone in the `no_std` B12 harness: 6,037 B raw / 3,344 B gz
(agrees with `sem`). `icu-blob-fixed` by crate (twiggy, names excluded):
`icu_datetime` 25 %, `core` 12 %, `icu_provider` 9 %, `zerovec` 8 %, data
segments 8 %, `icu_locale_core` 7 %, `alloc` 6 %, `zerotrie` 4 %. `offz` =
`fixed` byte for byte: a runtime-built `CompositeFieldSet` links every zone
style whichever one is used; only the static field-set types prune.

### Per-locale ICU4X data blob (postcard, markers from `markers_for_bin`)

Each blob holds the locale (fully resolved, `DeduplicationStrategy::None`) plus
`und` (root-only attribute-keyed entries such as `DecimalDigitsV1`, which the
exporter never copies into child locales).

| Blob | markers | en | ar | ja | ru |
|---|---|---:|---:|---:|---:|
| `fixed` (all MF2 options incl. `timeZoneStyle`) | 20 | 30,293 / **16,755** | 48,791 / **22,523** | 45,378 / **21,913** | 49,699 / **23,028** |
| `nozone` (no `timeZoneStyle`) | 9 | 4,380 / **2,298** | 4,466 / **2,385** | 3,956 / **2,157** | 5,124 / **2,572** |
| `date1` (corpus uses only default `:date`/`:time`/`:datetime`: skeletons `ym0d`, `j`, glue `mdt`) | 9 | 2,739 / 1,743 | 2,913 / 1,832 | 2,491 / 1,601 | 3,413 / 2,007 |

(raw / gz bytes.) `und`'s share: `fixed-und` 24,652 / 14,502, `nozone-und`
2,802 / 1,639. Largest markers in `fixed-en` (gz): `TimezonePeriodsV1` 5,281
(singleton), `TimezoneNamesLocationsRootV1` 5,113, `…LocationsOverrideV1` 2,495,
`…CitiesRootV1` 2,168, `…SpecificLongV1` 989, `DecimalDigitsV1` 862 (every
numbering system), `DatetimePatternsDateGregorianV1` 644. Time-zone names are
≈ 85 % of the `fixed` blob although the ICU4X backend, having no tz rules, can
only ever *display* offsets (see caveats).

### Correctness checks

* Suite, `functions/{date,time,datetime}.json` (20 tests, hand-lowered to calls):
  **20/20** error expectations met (`bad-operand` for missing / non-date
  operands, none otherwise). The suite has no `exp` for successful date output
  — every such test needs locale data by nature.
* Blob ≡ compiled data: **93/93** option sets identical for each of en, ar, ja,
  ru (`fixed` blob) and 81/81 (`nozone`); an `en` blob serves an `en-US`
  request through `OneLocale`; the `date1` blob formats `:date` (any length)
  and correctly fails `fields=weekday`.
* ICU4X (CLDR 48.2.1) vs `Intl` (node 23.11, ICU 76.1 / CLDR 46): on the 7
  sample rows without zones, **28/28** cells identical **only after mapping
  U+202F → U+0020** — V8 deliberately emits a plain space where CLDR has a
  narrow no-break space ("3:04 PM" in en, "2006 г." in ru). With
  `timeZoneStyle`, outputs differ structurally: ECMA-402 forbids
  `dateStyle`+`timeZoneName`, so the component fallback gives "2 يناير 2006"
  where the semantic skeleton gives "02/01/2006"; `UTC` long is "Coordinated
  Universal Time" vs ICU4X "GMT+00:00" (offset-only zone). Named zones and
  `calendar=japanese` work in `Intl`; an unknown zone (`Mars/Olympus`) is caught
  as *Bad Option*.

### B12

`mf2dt` in the `no_std` harness: **0** `core::fmt` symbols, **0** panic symbols
(twiggy over the non-stripped build). The ICU4X variants carry 72 `core::fmt`
symbols (7.9 KB) — irrelevant to B12, which covers the runtime crates, but it
means `datetime-icu` brings the fmt machinery into any app that enables it.

## Conclusions against thresholds

| Budget | Estimate | Measured | Verdict |
|---|---|---|---|
| **B4 `datetime-intl`** | ≤ 3 KB gz | **5.4 KB gz** wasm + 0.6 KB gz JS (js-sys); 5.1 KB gz + 0.24 KB gz (inline JS). Of it, 3.4 KB gz is the MF2 semantics every backend needs | **not met** |
| **B4 `datetime-icu`** | ≈ 100 KB gz (audit) | **93 KB gz** (Gregorian, all options), 64 KB gz without `timeZoneStyle`, 105 KB gz any-calendar; data 17–23 KB gz per locale with zones, **2.2–2.6 KB gz** without | **confirmed** (≈ 100 KB gz) |
| B12 (date semantics crate) | absent | 0 fmt / 0 panic symbols | **met** |
| B1′ / B13 | — | not measured here (closed-world linking is a P3/P4 property); note that within `datetime-icu`, runtime skeleton selection links all zone styles | — |

**Recommended budget values** (C2):

* B4 `datetime-intl`: **≤ 6 KB gz** wasm + ≤ 1 KB gz JS glue, of which ≤ 3.5 KB
  gz date semantics. Reachable savings, not taken in the probe: have the build
  intern option names/values (they are literals except the three override
  options) — about 1 KB gz of string tables and compares.
* B4 `datetime-icu`: **≈ 95 KB gz** (Gregorian) / **≈ 105 KB gz** (any calendar)
  code; `icu.blob` per locale **≤ 3 KB gz without zone names, ≤ 25 KB gz with**.
  Worth a plan note: the build knows whether the corpus uses `timeZoneStyle`
  and should link the no-zone field-set type when it does not (−29 KB gz code,
  −85 % data).
* Compiled data is not an option on the client (> 1 MB gz).

## Server path (ICU4X always on the server)

* Same `dt-icu` code with compiled data (size irrelevant natively) **or** the
  same per-locale blobs. For `datetime-icu` byte identity with the client, the
  server must use the **same ICU4X data** as the catalog's `icu.blob` —
  simplest: the server formats from the same blobs.
* **Time zones**: ICU4X 2.3 has no tz transition rules. Converting an instant to
  an IANA zone (`timeZone=America/New_York`, or the visitor's zone from the
  cookie of 03 §6) and naming it needs a tz database beside ICU4X on the server
  (e.g. `jiff` with the system or a bundled tzdb) → the zone id + offset + name
  timestamp then go into `DateTimeInputUnchecked`. The probe returns
  *Unsupported Operation* for named zones on the ICU4X path. The client with
  `datetime-icu` has the same gap unless a tzdb is shipped too (not measured).
* With `datetime-intl`, SSR text (ICU4X, CLDR 48) and hydrated client text
  (browser ICU/CLDR) differ at least in U+202F vs U+0020 and in zone-styled
  layouts — the P0.10 text-mismatch tolerance is required, not optional.

## Caveats

* Probe harness, not product: options arrive as `k=v` text; one formatter is
  built per call (the product caches one per locale × plan); the blob provider
  is re-created per call (size-neutral).
* ICU4X zone input is offset-only (`TimeZone::UNKNOWN` + offset + name
  timestamp), so `timeZoneStyle` renders localized offsets ("GMT+5:30"); UTC
  could be given the `utc` zone id to read "Coordinated Universal Time" as
  `Intl` does.
* `Intl` cannot place a *floating* wall-clock time in a *named* zone (needs
  Temporal or a two-pass offset search); reported *Unsupported Operation*. It
  also cannot reproduce "standard date style + zone name"; the component
  fallback differs from ICU4X there.
* `datetime-icu` blobs keep every numbering system's digits (`DecimalDigitsV1`,
  0.86 KB gz); an attribute filter to the locale's systems would trim it.
* `blobgen` works around an ICU4X 2.3.0 bug: `icu_time_data`'s
  `impl_timezone_periods_v1!(…, ITER)` does not compile (`BtreeSet`), so that
  singleton's iterable impl is hand-written.
* Selection on dates is not required by the spec and was not probed.
