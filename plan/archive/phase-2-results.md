# Phase 2 results

Part of the [master plan](00-master-plan.md) (§9, P2); the record behind the
exit checklist of the [Phase 2 work order](09-phase-2-work-order.md). Every
figure names the command that produced it. Measured 2026-09-21 on the Phase 0
machine (Intel Core i7-1165G7, 4 cores / 8 threads, Linux 6.12), toolchain
stable 1.98.1; the fuzz target on nightly (2026-04-14) with cargo-fuzz 0.11.0;
wasm-opt 120, twiggy 0.8.0, GNU gzip 1.13. Up to five agents built
concurrently: **sizes and allocation counts do not depend on load; timings
do**, and are marked with the load they were taken under.

## Summary

| Exit item | Verdict | Evidence |
|---|---|---|
| L3 100 % (301/301); `current_phase = "P2"` | **met** | L1 462/462, L2 326/326, L3 301/301 in `conformance/REPORT.md`; `current_phase` bumped in the exit commit with the harness green |
| the L3 property on 1,000,000 generated messages | **met** | 1,000,000 ABNF-generated messages (invalid models included) round-trip unstripped and stripped, 40.0 s in release (§A6) |
| deterministic output across runs and processes (F8) | **met** | every test writes twice and compares; a test re-runs itself as a child process and compares a digest over every suite and workload catalog |
| B7 on the committed corpus and the scaled rule | **met** | restated on brotli 11 by the owner: `en` 18,072 B br (≤ 23,296); every locale within 0.91 × (0.5 × source + 1 KB) and 1.25 × source + 8 B/message (§A8) |
| decoder fuzz run ≥ 1 h clean; the linear-time test green | **met** | 2,077,927 executions in 3,901 s on the final code, no crash or timeout (§A7); `tests/linear.rs`: 19 adversarial shapes grow 3.6–4.6× from n to 4n |
| reader `no_std`, `forbid(unsafe_code)`, B12 clean, load allocates nothing | **met** | B12: the panic import is absent after LTO + `wasm-opt -Oz`, no `core::fmt`, controls fire (§A9); 0 allocations and 0 copies loading all 8 catalogs |
| format version 1 frozen: 02 §2 complete, with test vectors; the API | **met** | [02](02-catalog-format.md) §2 is the byte grammar, §2.10 three hand-worked vectors the writer reproduces byte for byte; the API as frozen plus additions, listed in 09 §"Status at exit" |
| `stripped ≡ unstripped formatting` open until L4 | **recorded** | ledger note `stripped-formats-identically`, `status = "open"`, `until = "P3"`; the checker turns red if it is still open at P3 |
| the Phase 3 work order | **written** | [10-phase-3-work-order](10-phase-3-work-order.md) |

## What was built

| Crate / tool | Lines | Content |
|---|---:|---|
| `crates/mf2-catalog` | 3,836 + 2,406 tests | the reader (client path: `no_std`, `forbid(unsafe_code)`, panic lints denied), the views, the manifest (feature `manifest`), the writer (`writer`), the decoder (`decode`) |
| `conformance` | +1,137 −354 | `l3.rs` (the harness and the formatting-relevant projection), `tests/generated_l3.rs`, open ledger notes with `until` |
| `fuzz/` | +865 | target `catalog`; `cargo xtask fuzz-seed` writes its seeds (+144) |
| `bench/catalog-bench` | 3,271 | B7 size report (`cargo xtask catalog-size`), reader bench (`bench --gate`) |
| `bench/b12` | 673 | the B12 wasm harnesses and `check.sh` |
| CI | 2 jobs, 2 nightly steps | `b12` and `catalog-size` on every push; the catalog fuzz target and the reader bench nightly |
| `probes/` | −123,580 | deleted (Phase 0 C5), in the first commit's history |

Tests: 205 in `cargo test --workspace`. Commits: `9e122f3` (format and
crate), `369479c` (L3), `ca2ef1f` (fuzz), `3478af3` (benches), `677d2d0`
(C5), and the exit commit.

## A1 — the byte grammar, version 1

Written into [02](02-catalog-format.md) §2 before the code; the three vectors
of §2.10 (simple, pattern with an attribute, select with a plural entry) were
worked by hand — the manifest hashes with an independent script — and the
writer reproduced all three byte for byte on its first run. What version 1
adds to P0.7's prototype:

* **COLD is per message, keyed by site.** MESSAGES holds the
  formatting-relevant model only: names, keys, function and option
  identifiers in NFC (what the spec compares), variables as slots. A message's
  COLD record lists overrides by *site* — a number every expression, markup,
  variable, function, option name, markup name and key takes in byte order —
  of three kinds: the spelling as written (only where it is not what MESSAGES
  and NAMES give), attributes, a catch-all key's value. The reader never reads
  it; writer and decoder number sites the same way. Stripping keeps each
  message's COLD bit, so the decoder reports exactly which messages lost data.
* **Invalid models round-trip**: every variant carries its own key count, a
  VarRef that resolves to a `.local` is `Local(i)` even inside an `.input`,
  duplicate declarations resolve to the latest binding.
* **O(1) where the prototype scanned**: NAMES and FUNCS hold 4-byte string
  references (`str32`), FALLBACK is a sorted `u32` list (binary search), IDS
  has a restart table every 16 ids (`lookup` in O(log n)).
* **Header**: `format_version` is `major << 8 | minor` (readers reject an
  unknown major, F9), a `cldr_version` field, flags for stripped COLD / IDS;
  32 B + 10 B per section.
* **Strictness**: minimal varints only; the last pool byte is NUL; the LOCALE
  keys ascend and the plural entries are walked for structure at load; a
  pattern with an empty or adjacent TEXT part is malformed.

Two changes came from Phase 2's own measurements (§A8): the COLD bit moved
from the NAMES reference to the declaration count (`varint names · varint
(decl_count << 1 | c)`), and the writer orders NAMES entries most-referenced
first. Together they took MESSAGES from +116 B to −2 B against P0.7.

## A2 — reader

`Catalog::new` validates in one pass per section: magic, version, hash (F6),
the section table (order, bounds, duplicates, STRINGS last and NUL-ended),
INDEX (bounds, strictly increasing record offsets), NAMES, FUNCS, FALLBACK,
LOCALE with its plural entries (the §4.1 grammar), IDS (restart offsets,
shared lengths). It allocates nothing and keeps the buffer it is given. Every
accessor is bounds-checked; strings are UTF-8-checked when read (F4). The
views (`MsgView`, `Declarations`, `PatternView`/`Parts`, `ExprView`,
`FunctionView`, `OptionsView`, `MarkupView`, `SelectView`, `Selectors`,
`Variants`, `Keys`, `Names`) are `Copy` cursors; a malformed record yields one
`Err(Malformed)` and ends its iterator.

`CatalogError` gained `Header`, `Ids` and `Strings` beside the frozen list (a
`#[non_exhaustive]` enum; 09 records the change). `tests/reader.rs` provokes
every variant, rejects every truncation of a catalog holding every section,
and flips every bit of it: each flip is rejected or loads and is walked
completely (views, strings, names, functions, decoding) without a panic.

## A3 — manifest

`Manifest { ids, slots, markup, functions }`, `hash()` (FNV-1a 64 over the
canonical serialization, now byte-exact in 02 §3), `write`/`read` of
`manifest.mf2m` (`"MF2M" · version · hash · serialization`; `read` rejects a
wrong magic or major, a hash mismatch, trailing bytes and unordered lists).

```sh
cargo test -p mf2-catalog --all-features --test manifest
```

The manifest built from `bench/corpora/workload-1600.json` (slots and markup
from `analyze`, functions `["integer"]`) hashes to **`43e0dc12eeb05ef1`**,
P0.7's figure.

## A4 — writer

Two passes through the same code: pass 1 only collects strings, the pool is
laid out (identifiers first, then text, each sorted bytewise), pass 2 writes
with the final offsets — so the passes cannot disagree. Deterministic by
construction (ordered maps only); a test writes every suite message and the
workload twice and in a second process and compares. It refuses U+0000 (also
in stripped COLD data), a variable neither local nor in the slot list, a
function outside the manifest, an `.input` whose name differs from its
variable, direction `auto`, and sizes past the format's limits; it accepts
data-model errors.

## A5 — decoder

`decode` / `decode_report` walk the reader's own views, so reader and decoder
share one reading of the grammar, and apply the COLD record site by site. With
COLD stripped they return the formatting-relevant model and `cold_dropped`.
Round trips: every node kind (the `mf2-model` JSON fixture), non-NFC
spellings of variables, keys, functions, options and markup (composed vs
decomposed é, the Kelvin sign), invalid models, shapes MF2 syntax cannot write
(no selectors, keyless variants, catch-all values), all 326 parseable suite
messages, and the whole workload as one catalog (every id found by `lookup`).

## A6 — conformance L3

```sh
cargo xtask conformance-report --promote   # once: 301 cells xfail → pass
cargo xtask conformance-report             # L1 462/462, L2 326/326, L3 301/301: green
MF2_GEN_CASES=1000000 cargo test --release -p mf2-conformance --test generated_l3   # 40.0 s
```

Per test: write unstripped, load, decode — equal to the L2 model; the header
says what was written; a wrong expected hash gives `ManifestMismatch`; the
stripped catalog decodes to the **formatting-relevant model**, a projection
written independently of the writer (`conformance/src/l3.rs`,
`formatting_model`: attributes and catch-all values removed; keys and function,
option and markup names in NFC; a variable reference spelled as its `.local`
declares it, else its NFC slot name), and `cold_dropped` is set exactly when
the message has COLD data. 19 of the 301 messages have COLD data.

Generated input: 1,000,000 messages (the Phase 1 generator's seed; every
message that parses, invalid ones included): 299,205 with declarations,
431,851 with COLD data, 8,347 with a non-NFC name, 250,110 with data-model
errors — all round-trip, in 40.0 s (49.6 s under load; a second seed, 12345,
40.5 s). The test fails if any of those shares drops below a floor. It runs in
the nightly workflow.

## A7 — fuzzing and linear time

**Fuzz target `catalog`** (`fuzz/fuzz_targets/catalog.rs`): an input starting
`MF2B` is a catalog, loaded under the hash in its own header; anything else is
MF2 source compiled with `writer::single`, then damaged by input-driven edits
inside chosen sections. On every catalog that loads: every accessor, a full
view walk of every message with the iterator contracts checked, `decode`
(which must agree with the walk, part for part), and a rewrite of the decoded
models that must load and decode to the same models (F1). Time budget 50 ms +
50 µs per input byte + 100 ns per byte of resolved text.

```sh
cargo xtask fuzz-seed          # 1,306 catalog seeds: suite messages (both forms), the workload
cd fuzz
cargo +nightly fuzz run catalog -- -dict=mf2.dict -max_len=131072 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900 -print_final_stats=1
```

* Development runs: 3 min clean; 15 min that found **a bug** — `lookup` assumed
  IDS front coding always shares the longest prefix, which the format does not
  require, and missed ids a catalog holds (fixed; a regression test tries all
  729 shorter-than-possible codings; 02 §2.8 now says readers must not rely on
  it); 15 min clean on the fix (701,688 executions).
* **Exit run (final code), 2026-09-21 16:51–17:56: clean.** 2,077,927
  executions in 3,901 s (532/s, `nice -n 10`), 0 crashes, 0 timeouts, no
  artifacts, slowest unit < 1 s; coverage 9,302 edges / 42,437 features;
  corpus 1,306 seeds → 5,243 inputs (57 MB) plus what a first attempt had
  added; peak RSS 594 MB.
* That first attempt (16:44) stopped after 193,591 executions on the time
  budget: a 228-byte source-mode input took 64 ms against 61 ms while two
  `cargo xtask ci` builds ran at a higher priority than the fuzzer (load
  average 4.7). Run alone, the same input passes; the run above was started
  with no build beside it.

**Linear-time test** (`crates/mf2-catalog/tests/linear.rs`): 19 adversarial
shapes built at n and 4n (up to 360 KB) — 50,000 options, 16,000 variants,
100,000 keys, a 10,000-link chain of `.local`s, 100,000 parts, 16,000 FUNCS
entries, 50,000 attributes and spelling overrides, 10,000 messages, long and
long-prefix IDS, 20,000 fallbacks, 40,000 LOCALE entries, one NAMES entry of
24,000 names shared by 24,000 messages, 65,528 unknown sections. Load, full
walk, decode of every message, lookup: every shape grows 3.6–4.6× (linear is
4); the whole test takes 4–9 s in a debug build.

**Finding, decided: records that run into each other.** A record carries no
length, so a hostile catalog can make a message's part count run on through
the records after it; walking *every* message is then O(messages × size)
(1,000 → 4,000 such records: 0.6 → 9.7 s in debug). A length per record would
bound records but not strings — a hostile catalog can already point every
reference at one long string, which F4 checks per access by design — so the
format does not pay for it. 02 §2 ("Cost bounds") states the guarantee
instead: load is linear, every operation is linear in the bytes it reads and
never reads past the file; a corrupt catalog can make work repeat, never
panic or read out of bounds. A test holds the single-message bound.

## A8 — B7 on the committed corpus

```sh
cargo xtask catalog-size --md bench/catalog-bench/SIZE-P2.md --json bench/catalog-bench/size-p2.json
```

The four locales of the reference workload (`en` is the committed corpus byte
for byte), manifest `43e0dc12eeb05ef1`, P0.4's `plural.cardinal` entry per
locale, CLDR 48.2.1. Production catalogs (COLD and IDS stripped).

**B7 is stated on brotli** (owner, 2026-09-21): the build writes a `.br` file
beside each catalog and serves it to every client that accepts brotli, which
every current browser does over HTTPS; gzip only reaches the rest. The limits
are the former gzip ones scaled by 0.91, the *worst* brotli/gzip ratio
measured on these locales (en-XA: 0.909 in P0.7 and in Phase 2, to three
decimals), rounded up, so no locale is held tighter than under gzip — each
uses a slightly smaller share of its new limit than of the old one, and
P0.7's own figures pass too. gzip is reported, not gated.

| locale | MF2 source | raw (limit) | **brotli 11** (limit) | share of limit | gzip -9 (former limit) | structure gz | pool gz | unstripped br | B7 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| en | 43,250 | 51,502 (66,862) | **18,072** (20,610; ref. 23,296) | 87.7 % | 20,592 (22,649) | 6,940 | 12,804 | 26,446 | met |
| pl | 58,518 | 67,930 (85,947) | **24,137** (27,557) | 87.6 % | 26,706 (30,283) | 7,242 | 18,567 | 32,609 | met |
| en-XA | 95,078 | 101,156 (131,647) | **21,537** (44,192) | 48.7 % | 23,686 (48,563) | 7,337 | 16,255 | 30,081 | met |
| ar-XB | 54,491 | 61,418 (80,913) | **18,423** (25,725) | 71.6 % | 20,597 (28,269) | 6,958 | 13,401 | 26,987 | met |

**Against P0.7's recommended layout**: brotli +143 / +69 / +60 / −68 B
(`en` +0.80 %: past the report's 100 B "noise" line), gzip +56 / +59 / +66 /
+79 B (+0.22 to +0.39 %). The pool is byte-identical; the whole delta is
structure: NAMES +626–633 B raw (the `str32` references), FUNCS +2, header
+2, MESSAGES −2. Before the two A1 changes above the gz delta was +107–130 B
(MESSAGES +116 B raw: heads that crossed to two bytes).

**NAMES encoding, decided.** `str32` keeps every name lookup O(1). Measured
alternatives (the report re-encodes NAMES every run): P0.7's varints would
save 17–204 B brotli (up to 1.1 %, `en`), 44–74 B gz, but make a name an O(i)
scan; fixed 2-byte references keep O(1) but save only 46–89 B brotli, and
cost 12 B on `ar-XB`. The owner kept the format for v1 and deferred the
question to after v1:
[stretch_goals_after_v1/names_reference_width](stretch_goals_after_v1/names_reference_width.md).

**gzip implementations** (reported, no longer gating). Rust gzip at its
maximum level against GNU `gzip -9 -n`, stripped: flate2 (miniz_oxide) +42 /
+363 / +883 / +695 B, zlib-rs +60 / +361 / +877 / +710 B (0.2–3.7 %);
compressing structure and pool separately all three agree within −5…+31 B,
so the gap is in how each splits the one stream. Which one writes the `.gz`
file is `mf2-build`'s choice (P5a); the report uses GNU gzip when installed,
else flate2.

## A9 — reader cost

**Native bench** (`bench/catalog-bench`, parser-gate style: median of 31
interleaved samples, counting allocator; load average 1.2–1.6, a fuzz run at
`nice 10` on one thread):

```sh
cargo run --release -p catalog-bench -- bench --gate \
    --md bench/catalog-bench/READER-P2.md --json bench/catalog-bench/reader-p2.json
```

| locale | `Catalog::new` stripped (P0.8) | unstripped | simple `get` + `text` (P0.8) | of which `str::from_utf8` | `get` + walk, pattern / select |
|---|---|---:|---|---:|---|
| en | **2.29 µs** (5.70; 0.40×) | 6.26 µs | **19.3 ns** (20.7; 0.93×) | 8.2 ns | 55.6 / 123.6 ns |
| pl | 2.39 µs (6.70; 0.36×) | 6.45 µs | 65.6 ns (—) | 51.6 ns | 68.7 / 343.0 ns |
| en-XA | 2.31 µs (6.90; 0.33×) | — | 77.8 ns (—) | — | — |
| ar-XB | 2.34 µs (6.10; 0.38×) | — | 28.6 ns (28.4; 1.01×) | — | — |

Loading allocates **0** bytes and copies **0** (the pointer and length of the
moved-in buffer are unchanged) for all 8 catalogs; every lookup sweep
allocates 0. `cargo test --workspace` enforces the 0/0 on the real workload.
Gate: pass. Unstripped loads cost ≈ 4 µs more: the IDS walk.

**Finding for Phase 3.** On non-ASCII text the per-access UTF-8 check is
80–85 % of a simple lookup (pl 65.6, en-XA 77.8 ns against en's 19.3); under
heavier load en-XA reached 107 ns. B10 (≤ 100 ns) is stated for the reference
`en`, which meets it with room; a non-Latin locale is close to the line.

**B12** (`bench/b12/check.sh`, CI job `b12`): `no_std` `cdylib` harnesses for
`wasm32-unknown-unknown`, `wasm-release` (fat LTO, `opt-level = "z"`,
`panic = "abort"`), then `wasm-opt -Oz`. The reader harness takes the catalog
from an import and exercises every public reader API and a full view walk;
its `#[panic_handler]` calls the import `b12::b12_panic_reachable`.

* The import is **absent** from the reader harness after LTO + `wasm-opt`;
  twiggy on the symbol-keeping build finds 0 `core::fmt` and 0 panic or
  alloc-failure symbols; 0 panic message strings in the data.
* A control harness with a deliberate `bytes[i]` and a `write!` is caught (its
  import present, fmt and panic symbols found), and one `bytes[i]` added to
  the reader harness made the check exit 1.
* **Size**: reader harness 14,085 B raw / 7,122 B gz against a base of 472 /
  335 → **Δ 13,613 B raw / 6,787 B gz** — the reader *plus* a walk over every
  API, so an upper bound for the reader's share of B1. By twiggy:
  `Catalog::new` 3,373 B (P0.3's: 1.9 KB; the difference is the load
  validation Phase 2 added — NAMES, FALLBACK, IDS restarts, plural
  structure), other reader functions 4,390 B, `str::from_utf8` 698 B, the
  harness's walk 5,311 B. IDS validation and `lookup` are linked although
  production catalogs strip IDS — a B1 saving for Phase 3 if needed.

## A10 — skew and flags

`crates/mf2-catalog/tests/skew.rs`: F6 — changing a slot, a markup name, the
function set or an id changes the hash, and a catalog under another hash is
`ManifestMismatch`, while a translation that changes text, selectors and
variants (and uses a subset of the variables) keeps it; F7 — 300 messages with
two fallback locales round-trip through the binary search; F9 — majors 0, 2
and 255 are `Version`, a later minor loads; stripping — the whole workload
decodes identically stripped and unstripped. L3 checks the stripped catalog
against the formatting-relevant model on every suite test and on the
generated input (§A6).

## Carried forward

* **Owner decisions**: the remote host (the CI `runs-on` label); spec
  license (#1112); B5/B9 restatements (B7 was restated on brotli, §A8);
  whether the D1 gate runs on every push; the `fixed_decimal` panic paths
  (before P3 exits).
* **For Phase 3** (in [10](10-phase-3-work-order.md)): the per-access UTF-8
  cost on non-Latin text against B10; IDS code linked into stripped clients;
  the view API is what the evaluator walks — functions are resolved per call
  (P0.3) unless measured otherwise; the open note `stripped-formats-identically`
  is due at P3.
* The B12 job's binaryen download could not be exercised from the
  development machine (its network rule); its pinned URL and checksum are
  checked on the first CI run.
