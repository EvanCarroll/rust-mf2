# P0.4 — plural rules: RESULT

**Question** (plans/07, plans/06 §5): can our own UTS #35 plural-rule parser
(build side) + a compact byte encoding of the `plural.cardinal` /
`plural.ordinal` LOCALE entries (plans/02 §4) + a `no_std`, fmt-free,
panic-free evaluator (client side) pass **every CLDR `@integer`/`@decimal`
sample for every locale**, within ≤ 1.5 KB gz of code and ≤ 300 B of data per
locale, with no `core::fmt`/panic machinery (B12)? That confirms or refutes D3.

Input: `third_party/cldr-json/cldr-core/supplemental/{plurals,ordinals}.json`,
CLDR 48 (tag 48.2.1, see its `PIN`). Toolchain: rustc/cargo 1.98.1 (stable, from
the repo's `rust-toolchain.toml`), wasm-opt version 120, twiggy 0.8.0,
wasmtime 0.28.0, ICU4X `icu_plurals` 2.3.0 / `icu_provider_blob` 2.3.0 /
`icu_provider_export` 2.3.0 / `icu_provider_source` 2.3.1 (latest on crates.io,
2026-09-20).

## Verdicts

| Threshold (06 §5) | Measured | Verdict |
|---|---|---|
| 100 % of CLDR samples | **15,041 / 15,041** (12,396 cardinal samples over 224 locales; 2,645 ordinal samples over 108 locales). 0 failures. Every sample is claimed by exactly its own category, `other` included | **met** |
| Evaluator code ≤ 1.5 KB gz | **429 B gz** (667 B raw) as a delta against its base, after `wasm-opt -Oz`. Inside a std module: 462 B gz (764 B raw) | **met** |
| Data ≤ 300 B per locale | cardinal + ordinal entries per locale: **min 0 / median 3.5 / max 84 B** (`kw`). Cardinal alone max 61 B (`kw`), ordinal alone max 50 B (`az`) | **met** |
| B12: no `core::fmt`, no panic machinery | 0 fmt symbols, 0 panic symbols, no panic strings; the panic handler is unreachable (its import is absent after LTO + `wasm-opt`). In a std module the evaluator adds exactly 2 functions (`select`, `Reader::varint`), neither of them fmt or panic | **met** |

**D3 is confirmed.** Own evaluator plus per-locale rules in the catalog:
0.43 KB gz of code against 14.5 KB gz (`icu_plurals` + blob) and 17.9 KB gz
(`icu_plurals` compiled data), both measured on the same fair baseline, and
0–84 B of data per locale.

## Layout

| Path | What it is |
|---|---|
| `eval/` (`plural-eval`) | **Client side.** `#![no_std]`, `forbid(unsafe_code)`, denies `unwrap_used`, `expect_used`, `indexing_slicing`, `panic`, `arithmetic_side_effects`, `as_conversions`; clippy pedantic clean. `select(entry, &Operands) -> Category` (≈ 100 lines) and the byte-format constants. `Operands::parse` (decimal string → operands, incl. `1c6` / `1.1e6`) is used by the tests only. Its types have no `Debug`. |
| `rules/` (`plural-rules`, bin `p04`) | **Build side (std).** UTS #35 rule + sample parser (`rule.rs`, `samples.rs`), encoder (`encode.rs`), an **independent reference evaluator** (`reference.rs`: string-based operands, exact rational arithmetic over the AST), CLDR loader (`cldr.rs`), the correctness run (`check.rs`), errors via `thiserror` (`error.rs`), clap CLI. `tests/cldr_samples.rs` makes the 100 % threshold a `cargo test`. |
| `icu-data/` | ICU4X side: postcard payload and `BlobDataProvider` blob sizes, both built from **our vendored CLDR 48.2.1** through `icu_provider_source` (no download). ICU4X also serves as a differential oracle. |
| `wasm/*` | cdylib size harnesses, described under "Code size" below. |
| `scripts/size.sh`, `scripts/b12.sh`, `scripts/data.sh` | the measurements. Their outputs are in `out/` (`size.tsv`, `b12.txt`, `data.txt`, `check.txt`, `oracle.txt`). |

## Commands (from `probes/p0-04-plural/`)

```sh
CARGO_BUILD_JOBS=3 cargo test --release                        # unit tests + the 100 % gate (tests/cldr_samples.rs)
CARGO_BUILD_JOBS=3 cargo run --release --bin p04 -- check      # counts + failures      -> out/check.txt
CARGO_BUILD_JOBS=3 cargo run --release --bin p04 -- sizes --all
CARGO_BUILD_JOBS=3 cargo run --release --bin p04 -- dump ru    # rules + encoded bytes of one locale
scripts/size.sh                                                # wasm code sizes         -> out/size.tsv
scripts/b12.sh                                                 # B12                     -> out/b12.txt
scripts/data.sh                                                # data sizes, ICU4X payload/blob, gz -> out/data.txt
CARGO_BUILD_JOBS=3 cargo run --release -p icu-data -- oracle   # ICU4X differential run  -> out/oracle.txt
# the same correctness run compiled to wasm:
CARGO_BUILD_JOBS=3 cargo build --release --target wasm32-wasip1 -p plural-rules --bin p04
wasmtime run --mapdir "/cldr::$(realpath ../../third_party/cldr-json/cldr-core/supplemental)" \
  target/wasm32-wasip1/release/p04.wasm -- --cldr /cldr check
```

What `size.sh` does for each harness `X` (the 06 §3 method):
`cargo build --target wasm32-unknown-unknown --profile wasm-release -p wasm-X`
(`opt-level="z"`, `lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip=true`) →
`wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue` →
`gzip -9 -n -c | wc -c`. `b12.sh` uses profile `wasm-syms` (the same, but `strip=false`) and `wasm-opt -Oz --debuginfo`.

## Correctness

### Counts (CLDR 48)

| | locales | rules with a condition | `other` rules | listed sample items (of which ranges) | expanded samples | `@integer` | `@decimal` | with exponent (`1c6`, `1.1c6`) |
|---|---|---|---|---|---|---|---|---|
| cardinal | 224 | 275 | 224 | 5,424 (729) | **12,396** | 5,798 | 6,598 | 216 |
| ordinal | 108 | 82 | 108 | 1,039 (135) | **2,645** | 2,645 | 0 | 0 |

Distinct rule sets: **40 cardinal** (the plan's "≈ 40" is exact) and **25 ordinal**,
the same counts whether compared by condition text or by encoded bytes. All 40
distinct cardinal entries concatenated come to 634 B; all 25 ordinal entries, 303 B.

### What each sample goes through (5 assertions each, 75,205 in total, 0 failures)

1. Operands: `plural_eval::Operands::parse` must equal the reference
   extraction (string shifting, `u128`) on `i v w f t e`.
2. `select(encode(locale rules), operands)` must be the sample's category.
3. The reference evaluator (AST, rational `n`) must agree.
4. Exclusivity, through the encoded path: each non-`other` rule is encoded alone
   and evaluated. Exactly the sample's own rule may match, or none for an
   `other` sample.
5. The same exclusivity check through the reference evaluator.

### Sample expansion (UTS #35 sample syntax; implemented in `rules/src/samples.rs`)

* A single value is one sample, kept **verbatim as a string**. `1`, `1.0` and
  `1.00` are three different samples, because visible fraction digits are
  operands (`v`, `f`).
* `x~y` expands to every value from `x` to `y` inclusive, **in steps of one unit
  of the last visible fraction digit**. Both endpoints must have the same number
  of fraction digits and no exponent; otherwise it is an error, and none occurs in
  CLDR 48. So `0.0~1.5` gives 0.0, 0.1, …, 1.5 (16 samples); `0.00~0.04` gives
  0.00 … 0.04; `2~16` gives 2 … 16. The largest range in the data has 16 values.
* A trailing `…` (or `...`) only says the list is open. It adds nothing to test
  and must come last.
* Compact exponent: `c` (and its synonym `e`) moves the decimal point right and
  keeps visible trailing zeros. `1.20050c3` is 1200.50 with `e`=3 (`i`=1200, `v`=2,
  `w`=1, `f`=50, `t`=5); `1.0000001c6` is 1000000.1 with `e`=6; `1c3` is 1000 with `e`=3.

### Further evidence

* **Differential oracle (ICU4X 2.3).** ICU4X's own evaluator, with data
  generated by `icu_provider_source` from the **same** CLDR 48.2.1 files, was
  compared with ours on every sample **plus a grid of 2,724 extra numbers per
  locale and kind** (0…2000; large values up to 3·10¹²; decimals with 1–3
  fraction digits; compact forms): 12,396 + 610,176 cardinal and
  2,645 + 294,192 ordinal comparisons, **0 mismatches**.
* **Wasm.** The whole run compiled to `wasm32-wasip1` under wasmtime gives the
  identical result: 15,041 / 15,041.
* **The run can fail.** Temporary mutations of the evaluator are all caught:
  "`n` always integral" → 92.4 % pass; "ignore `!=`" → 86.1 %; "exclusive upper
  bound" → 98.5 %; "`w` for `t`" → 99.90 %; "`f` for `v`" → 99.2 %.
* **The audit's `hand` evaluator was not correct.** It treated `c`/`e` as 0, which
  fails 54 samples in 9 locales (6 each in `ca es fr it lld pt pt-PT scn vec`, the
  `many` rule `… or e != 0..5`). Our evaluator replaces it.
* Unit tests cover the operand table of UTS #35 Part 3 §5.1.1 (including
  `1.2c6`, `123c5`, `1.20050c3`), malformed input (never panics; yields `other`),
  the parser's rejections, sample expansion and exact encoder bytes.

## Byte encoding — `plural.cardinal` / `plural.ordinal` entry, v1 (for Phase 2 to freeze)

The entry is the complete payload of one LOCALE-container entry. The container
supplies the length and, through the key, the version. **An empty payload is
valid** and means that every number is `other` (`ja`, `zh`, and most ordinals).

```text
entry       := rule*
rule        := rule_hdr or_group{G}
rule_hdr    := u8   bits 7..5  C  category: 0 zero, 1 one, 2 two, 3 few, 4 many (5..7 reserved)
                    bits 4..0  G  number of OR groups, 1..31
or_group    := relation+           ; an AND of relations; its last relation has L = 1
relation    := rel_hdr [modulus] item+
rel_hdr     := u8   bit 7      L  last relation of its OR group
                    bit 6      X  negated (`!=`)
                    bits 5..3  M  modulus: 0 none; k = 1..6 → `% 10^k`; 7 → explicit modulus follows
                    bits 2..0  O  operand: 0 n, 1 i, 2 v, 3 w, 4 f, 5 t, 6 c (= e); 7 reserved
modulus     := leb128 (≥ 1)        ; only when M = 7
item        := leb128(lo << 2 | R << 1 | E) [leb128(hi − lo) if R = 1]
                    R = 1: range lo..hi with hi > lo;  R = 0: single value lo
                    E = 1: last item of the relation's list
leb128      := unsigned LEB128, minimal length, ≤ 10 bytes, values u64
```

**Semantics.**
* Rules are tried in order, and the result is the category of the first rule
  that holds, or `other` if none does.
* A rule holds if any of its OR groups holds. A group holds if all of its
  relations hold.
* A relation takes the operand's value `x` (u64), applies `x %= m` if it has a
  modulus, and sets `hit` = (the operand is integral) and some item has
  `lo ≤ x ≤ hi`. The relation holds iff `hit ≠ X`.
* Only `n` can be non-integral, namely when `t ≠ 0`. It then matches no item,
  so `n = …` is false and `n != …` is true (UTS #35: a range matches integers
  only). When `t = 0`, `n` and `n % m` equal `i` and `i % m`.
* Malformed data — truncation, `O = 7`, modulus 0, a varint over 10 bytes,
  `hi` overflow — stops evaluation with result `other`. It never panics.

**Canonical writer (deterministic, 02 F8).**
* Rules go in category order zero < one < two < few < many. `other` is never
  written.
* OR groups, relations and items keep their source order.
* LEB128 is minimal. A range with `lo = hi` is written as a single value.
* A modulus equal to 10^k (1 ≤ k ≤ 6) uses the short code; any other modulus
  uses `M = 7`.
* CLDR 48 needs no explicit modulus: its moduli are exactly
  {10, 100, 1000, 100000, 1000000}, its largest literal is 1,000,000, and its
  rules use the operands `n i v f t e`.
* CLDR categories are pairwise disjoint, as the exclusivity check verifies, so
  rule order never changes a result. The canonical order exists only for
  byte-identity.

**Examples** (`p04 dump <loc>`):
* `en` cardinal, `one: i = 1 and v = 0`, is `21 01 05 82 01` (5 B).
* `en` ordinal (one/two/few) is `21 08 05 d0 2d 41 08 09 d0 31 61 08 0d d0 35` (15 B).
* `ar` cardinal is 17 B, `ru` 31 B, `pl` 32 B.

Container framing (key and length per entry) is Phase 2's to define and is not
included in the byte counts here. Expect 2–3 B per entry.

**Operands contract for Phase 3.** The formatter supplies
`Operands { i, f, t: u64, v, w, e: u32 }`; `n` is not stored. Values ≥ 10¹⁸ are
stored as `10¹⁸ + (value mod 10¹⁸)`. That is exact for every modulus that divides
10¹⁸ and for every literal below 10¹⁸, which covers all of CLDR 48. The encoder
in `mf2-locale-data` should assert both conditions at build time.

The pinned MF2 `:number` has no compact-notation option
(`spec/functions/number.md`), so from MF2 core `e` is always 0. The evaluator
supports `e` for full CLDR conformance and for any future `notation` option.

## Code size (wasm32, `wasm-opt -Oz`, gzip -9; `out/size.tsv`)

| Harness | What it links | raw B | gz B | base | **Δ raw** | **Δ gz** |
|---|---|---|---|---|---|---|
| `base` | `no_std` scaffolding: slice + 6 operand args | 98 | 114 | — | — | — |
| `eval` | + `plural_eval::select` | 765 | 543 | base | **667** | **429** |
| `base-ops` | `no_std` scaffolding: two slices | 100 | 110 | — | — | — |
| `eval-ops` | + `Operands::parse` + `select` (test-side path) | 1,731 | 1,055 | base-ops | 1,631 | 945 |
| `audit-hand` | the audit's `hand` evaluator, verbatim (string input, **incorrect**) | 900 | 604 | base-ops | 800 | 494 |
| `std-base` | the audit's std base (its `Vec` is optimised away) | 56 | 76 | — | — | — |
| `std-alloc` | fair std base: the same, with the `Vec` kept alive (allocator + std panic runtime linked) | 11,778 | 5,197 | std-base | 11,722 | 5,121 |
| `std-eval` | our evaluator in the audit's std harness (rules blob + `u32`) | 12,542 | 5,659 | std-alloc | **764** | **462** |
| `icu-blob` | `icu_plurals` 2.3 + `BlobDataProvider`, cardinal, locale string + `u32` | 41,616 | 19,648 | std-alloc | **29,838** | **14,451** |
| `icu-compiled` | `icu_plurals` 2.3 compiled data (all locales), cardinal | 51,557 | 23,062 | std-alloc | **39,779** | **17,865** |

**Audit reproduction** (`en`/`ar`/`ru` harnesses as in `probes/audit/plural-size`):
* **ICU4X.** The audit's absolute sizes reproduce: blob 41.6 KB / 19.6 KB gz
  (audit: 41 KB / 19.6 KB), compiled 51.6 KB / 23.1 KB gz (audit: 51 KB / 23.3 KB).
* **The audit's base was degenerate.** LLVM deletes the unused `Vec` in its base,
  so a delta against it also counts the allocator and std's panic runtime:
  11.7 KB raw / 5.1 KB gz, which any Leptos app already has. Against the fair
  `std-alloc` base, ICU4X costs 29.8 KB / 14.5 KB gz (blob) and
  39.8 KB / 17.9 KB gz (compiled), against our 0.76 KB / 0.46 KB gz.
  That is 31–39× less in gz.
* **Our evaluator.** Evaluator alone: 667 B / 429 B gz. The audit's `hand`
  (size probe, never tested) rebuilt verbatim today measures 900 / 604 B,
  including its string parser; the audit's 1.2 KB / 0.9 KB came from a different
  build, cause not investigated. With a string parser that handles `c`/`e`, large
  values and validation, ours is 1,631 / 945 B. The product does not link that
  parser.

## Data size (`out/data.txt`)

Every locale (224, the locales of `plurals.json`; ordinal rules resolved by subtag
truncation, else root = empty):
* cardinal entry: min 0 / median 3 / max 61 B / mean 6.5;
* ordinal entry: min 0 / median 0 / max 50 B / mean 3.9 (over the 108 locales of
  `ordinals.json`);
* **both per locale: min 0 / median 3.5 / max 84 B (`kw`) / mean 8.4.**

Panel. Raw bytes are the figure that counts: inside a catalog the entries are
compressed together with everything else. gzip -9 of a standalone entry is shown
for context only, and costs about 20 B of gzip header by itself. ICU4X payloads
and blobs were built from the same CLDR 48.2.1 files. The audit's range of 5–188 B
cardinal and 5–125 B ordinal for ICU4X payloads reproduces exactly.

| locale | ours cardinal raw/gz | ours ordinal raw/gz | **ours both raw** / gz | ICU4X postcard payload cardinal / ordinal | ICU4X blob cardinal raw/gz | ICU4X blob cardinal+ordinal raw/gz |
|---|---|---|---|---|---|---|
| en | 5 / 25 | 15 / 35 | **20** / 40 | 36 / 98 | 53 / 50 | 165 / 96 |
| es | 15 / 35 | 0 / 20 | **15** / 35 | 97 / 5 | 114 / 79 | 132 / 91 |
| de | 5 / 25 | 0 / 20 | **5** / 25 | 36 / 5 | 53 / 50 | 71 / 64 |
| fr | 16 / 36 | 3 / 23 | **19** / 39 | 105 / 21 | 122 / 80 | 157 / 102 |
| ar | 17 / 37 | 0 / 20 | **17** / 37 | 85 / 5 | 102 / 69 | 120 / 82 |
| he | 14 / 34 | 0 / 20 | **14** / 34 | 97 / 5 | 114 / 74 | 132 / 87 |
| ja | 0 / 20 | 0 / 20 | **0** / 20 | 5 / 5 | 22 / 36 | 31 / 44 |
| hi | 5 / 25 | 13 / 33 | **18** / 38 | 36 / 77 | 53 / 52 | 143 / 90 |
| ru | 31 / 48 | 0 / 20 | **31** / 48 | 188 / 5 | 206 / 100 | 224 / 112 |
| pl | 32 / 52 | 0 / 20 | **32** / 52 | 188 / 5 | 206 / 109 | 224 / 121 |
| cy | 15 / 35 | 20 / 40 | **35** / 49 | 85 / 125 | 102 / 62 | 241 / 101 |

`en`+`ar`+`ru` in one ICU4X cardinal blob: 344 B raw / 154 B gz; ours: 53 B raw.

## B12 (`out/b12.txt`)

* **`no_std` harnesses** (`wasm-eval`, `wasm-eval-ops`). twiggy over the
  non-stripped build and over the `wasm-opt -Oz --debuginfo` build finds
  0 `core::fmt`/`Formatter`/`Arguments`/`Debug`/`Display` symbols and 0
  `panic`/`unwrap`/`expect`/`bounds`/`overflow` symbols. The data contains no
  panic strings.
* **Panic reachability.** The harness's `#[panic_handler]` calls an `extern`
  import `p04_panic_reachable`. After LTO and `wasm-opt` the import is **absent**,
  which proves no panic path survives. This is stronger than a symbol grep.
* **What the module contains.** The optimised module holds `select` (566 B),
  `Reader::varint` (127 B) and the export glue.
* **std harness.** Symbol names were compared without hashes and LLVM clone
  suffixes. The evaluator adds only `plural_eval::select` and `Reader::varint`
  to `std-alloc`, and 0 fmt/panic functions.
* **`checked_pow`.** An earlier version used `u64::checked_pow`, which on wasm32
  pulled in the 128-bit multiply helper `__multi3` (122 B) for its overflow test.
  It was replaced by a bounded wrapping loop. This is worth a client-path coding
  note.

## Caveats

* The standalone measurements hold in an app too. The evaluator uses no
  allocator and no fmt, so it shares nothing that a Leptos app would already
  have paid for.
* Phase 3 must convert `fixed_decimal` into `Operands` in the formatter; this
  probe does not measure that code. The string parser here is test-only.
* Legacy UTS #35 keywords (`is`, `in`, `not`, `within`, `mod`) are rejected. They
  do not occur in CLDR 48. Supporting `within` would need one more flag bit
  (operand code 7 could carry it).
* The UTS #35 operand table in the unit tests was transcribed from the spec
  without network access; the boundary rule forbids unicode.org. The CLDR
  samples and the ICU4X oracle confirm the same semantics independently.
* Not every locale has ordinal rules. 116 of the 224 locales have none and
  resolve to root (empty entry).
* Only two locale ids in the files carry subtags (`kok-Latn`, `pt-PT`). Legacy
  aliases (`mo`, `tl`, `sh`, `no`) are present with rules identical to their
  canonical forms. Subtag truncation followed by root is a sufficient resolution
  for CLDR 48.
* ICU4X 2.3's compiled data does not carry 75 of the cardinal and 5 of the
  ordinal CLDR locales (for example `an`, `kw`, `lld`, `mo`, `tl`); for those it
  answers `other` for every number. That is another reason to build from CLDR
  JSON, as 05 §7 plans.

## Departures and inputs for C1/C2 (plans not edited, per the work order)

1. **D3 figure.** "≈ 0.9 KB gz vs ≈ 20 KB gz" becomes **0.43 KB gz** (0.46 KB in
   std) **vs 14.5 KB gz (blob) / 17.9 KB gz (compiled)** on a fair base. The
   figures are 19.6 / 23.1 KB gz against the audit's degenerate base.
2. **06 §3 "Preliminary measurements" plural rows.** Replace with the tables
   above. Per-locale plural data: ours is 0–61 B cardinal, 0–50 B ordinal and
   0–84 B combined, against ICU4X payloads of 5–188 / 5–125 B. The plural share
   of B8 is negligible.
3. **05 §7.** "Only 40 distinct cardinal rule sets" is exact at CLDR 48.2.1, plus
   25 ordinal. All distinct entries together are 937 B, so the compact tables in
   `mf2-locale-data` are tiny.
4. **02 §4 `plural.*` entries.** The encoding above is ready to freeze. One
   addition: an **empty entry is valid**. Recommendation: the entry is present
   (possibly empty) whenever the manifest says the corpus selects on plural or
   ordinal. `Catalog::new` should walk it once for structure (F4), so `select`
   never meets malformed data in practice; it still falls back to `other`.
5. **Method note for other probes.** A std baseline must keep its allocation
   alive (`black_box`). Otherwise every "delta" silently includes the allocator
   and std's panic runtime, about 5 KB gz.
6. **B12 technique.** Propose the import-in-panic-handler check for P0.3 and the
   CI B12 gate. It proves panic-freedom rather than inferring it from names.
7. **Tooling.** The machine's wasmtime is 0.28.0, with the old CLI
   (`--mapdir GUEST::HOST`, `--` before guest arguments). This matters for
   conformance layer L4 on wasip1.

## Owner questions

None.
