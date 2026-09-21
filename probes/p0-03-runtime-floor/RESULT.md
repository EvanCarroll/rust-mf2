# P0.3 — runtime floor: RESULT

**Question** (plans/07, plans/06 §5): how big is a `no_std`, fmt-free,
panic-free evaluator skeleton over P0.7's catalog encoding? Does it fit
**≤ 12 KB gz** (B1 credibility), with **no `core::fmt` and no panic machinery**
(B12)? And does it actually work, natively, on every workload message in all
four locales?

Toolchain: rustc/cargo 1.98.1 (stable, the repo's `rust-toolchain.toml`),
wasm-opt 120, twiggy 0.8.0. Inputs: P0.7's recommended encoding
(`probes/p0-07-catalog-encoding`, layout `nul-planes-split-stripped-sorted`;
see its RESULT.md) and P0.4's plural evaluator (`probes/p0-04-plural/eval`).
Both are used read-only through path dependencies.

## Verdicts

| Threshold | Measured | Verdict |
|---|---|---|
| Runtime floor ≤ 12 KB gz | **7,408 B gz** (14,768 B raw) as a no_std delta; **7,252 B gz** (14,591 B raw) as a std delta against a fair base (allocation kept alive). With P0.8's recommended UTF-8 strategy (validate on access): **7,022 / 6,825 B gz** | **met** (about 60 % of the budget) |
| B12: no `core::fmt`, no panic machinery | twiggy finds **0** fmt symbols and **0** panic, bounds or alloc-failure symbols, both before and after `wasm-opt`. The panic handler's import `p03_panic_reachable` is **absent** after LTO + `wasm-opt`, which proves no panic path survives. There are no panic strings in the module. The same holds for the per-access build | **met** |
| Works | **40,544 / 40,544 checks** (`out/check.txt`), 0 failures. That covers every message × 4 locales × up to 11 argument sets (**26,968 formats**), compared with an independent reference formatter. It also covers parts vs string, error sets, spot checks, and a mini corpus of 25 cases for paths the workload lacks. The **mutation run** loaded and fully formatted 1,292 of 3,000 corrupted catalogs (4.1 M formats); `Catalog::new` rejected the other 1,708. **0 panics.** Both UTF-8 strategies were checked | **met** |

## What the skeleton covers (`rt/`, crate `p03-rt`)

It is `#![no_std]` + `alloc` with `#![forbid(unsafe_code)]`. It denies
`unwrap_used`, `expect_used`, `indexing_slicing`, `panic` and `unreachable`.
There is no `format!`, `Debug` or `Display` on any path (the error enums derive
them for host use only; they are unreachable, and B12 shows it). About 1,190 lines.

* **`Catalog::new(Vec<u8>, manifest_hash)`**, which validates once (F4):
  * magic, version, the layout flags, and `manifest_hash` (F6);
  * the section table: in order, non-overlapping, in bounds, no duplicates,
    unknown kinds skipped, required kinds present, STRINGS last and ending at EOF;
  * INDEX size and entries: SIMPLE offsets inside the pool, and
    PATTERN/SELECT offsets strictly increasing inside MESSAGES (monotonicity);
  * the LOCALE container walk (it finds `plural.cardinal` / `plural.ordinal`);
  * the FUNCS table, and that every function name and the locale tag are pool strings;
  * UTF-8. The default strategy copies the structure head out with **fallible**
    allocation, moves the pool to the buffer's front, and runs one
    `String::from_utf8` over it. The `utf8-per-access` feature keeps the buffer
    untouched: zero copies and zero allocations. P0.8 recommends that one.
* **`get(MsgId)`** is O(1): four byte-plane reads. The chunk bits are checked.
  Pool strings are NUL-terminated, and a hand-written SWAR scan finds the end.
  (`str::find` kept a slice-index panic path alive; B12 caught it.)
* **Simple fast path**: `Formatter::simple(id) -> Option<&str>`, with no
  evaluator and no allocation.
* **Pattern formatting** writes into `&mut dyn Sink` (`push_str` only).
  Arguments are positional: `Arg::{Str, Int, Unset}`.
* **Declarations**: `.input` and `.local` are resolved lazily in place, with no
  table. A declaration only sees the ones before it, so corrupt data cannot
  recurse forever.
* **Functions**: `:string`, `:integer` and `:number`, over integers and integer
  literals, with neutral ASCII digits. `select=plural|ordinal|exact`. Unknown
  functions give *unknown-function* + fallback, and bad operands give
  *bad-operand* + fallback. Names are resolved when called: a few short compares
  against the FUNCS table, with no allocation.
* **Selection** follows the spec algorithm with no allocation (up to 8
  selectors). It filters with SelectorsMatch and keeps the best by
  SelectorsCompare, so the first best in source order wins. Plural keys use
  P0.4's evaluator on the catalog's `plural.*` entry. For `:number` and
  `:integer`, an exact numeric key beats its category key. `:string` compares
  bytes (NFC through `Host` is Phase 3). A selector without annotation raises
  *bad-selector* and matches only `*`.
* **Parts output** goes through `&mut dyn PartSink`. The part kinds are `Text`,
  `BidiIsolation`, `Expression{kind, value, dir}`, `Fallback(Source)`, and
  `Markup{kind: open|standalone|close, name, options}`. Options are a lazy
  iterator (literal or argument). String output and parts output share one
  walker behind a `dyn Out`, so there is a single copy of the code.
* **Fallback output**: `{$name}` takes its name from NAMES, `{|lit|}` escapes
  `\` and `|`, `{:fn}` covers function-only expressions, and `{�}` marks corrupt
  data or a missing message.
* **Default Bidi Strategy** uses the message dir from the catalog header.
  `:integer` / `:number` values are LTR: plain in LTR messages, LRI…PDI in RTL
  ones. Strings, literals, unannotated values and fallbacks are unknown, so they
  get FSI…PDI. `BidiStrategy::None` is also offered.

**Not in the skeleton** (Phase 3 scope, and not needed for the floor): `u:id`
and `u:dir`, decimals and floats, and every `:number` digit option. Also out:
NFC key comparison through `Host`, a custom-function trait with dyn registry,
the missing-message policy, and the `write_named` API. So the floor measures
the reader + evaluator + selection + plural + bidi + parts core. It does not
include the numeric formatter (P0.5 owns that; the budget is ≤ 6 KB gz) or the
Leptos glue.

## Commands (from `probes/p0-03-runtime-floor/`)

```sh
CARGO_BUILD_JOBS=3 cargo build --release && ./target/release/p03          # native check  -> out/check.txt
CARGO_BUILD_JOBS=3 cargo build --release -p p03-check --features p03-rt/utf8-per-access \
  && ./target/release/p03                                                   # same, per-access UTF-8
scripts/size.sh                                                             # wasm sizes    -> out/size.tsv
scripts/b12.sh                                                              # B12           -> out/b12.txt
```

`size.sh` follows the 06 §3 method. It builds with `cargo build --target
wasm32-unknown-unknown --profile wasm-release` (opt-level z, fat LTO, 1 CGU,
panic abort, strip), then runs `wasm-opt -Oz` with the bulk-memory,
nontrapping-float-to-int, sign-ext, mutable-globals, reference-types and
multivalue features, then `gzip -9 -n`. There are four harnesses (`wasm/*`),
each a cdylib with the same six exports: `alloc`, `load`, `simple`, `format`,
`parts` and `out_ptr`. Every runtime entry point is reachable, and the markup
options and fallback sources are consumed.

* `base` / `floor`: `no_std`, a bump `#[global_allocator]`, and a fixed static
  output buffer. The panic handler calls the import `p03::p03_panic_reachable`.
  The harness is panic-free itself: `alloc` goes straight to the allocator and
  the output copy is bounds-checked. So a surviving import can only come from
  the runtime.
* `std-base` / `std-floor`: the same exports on std. **`std-base` keeps a real
  allocation alive** (`black_box`), so the std allocator and panic runtime are
  in the base. P0.4 showed that without this the delta silently includes about
  5 KB gz.

## Size (`out/size.tsv`)

| Harness | raw B | gz B | base | **Δ raw** | **Δ gz** |
|---|---|---|---|---|---|
| `base` (no_std scaffolding) | 590 | 403 | — | — | — |
| `floor` (eager UTF-8) | 15,358 | 7,811 | base | **14,768** | **7,408** |
| `floor`, `utf8-per-access` | 14,635 | 7,425 | base | **14,045** | **7,022** |
| `std-base` (fair: allocation alive) | 12,434 | 5,558 | — | — | — |
| `std-floor` (eager UTF-8) | 27,025 | 12,810 | std-base | **14,591** | **7,252** |
| `std-floor`, `utf8-per-access` | 26,209 | 12,383 | std-base | **13,775** | **6,825** |

The largest items in the optimised module (`twiggy top`, `out/b12.txt`) are
`Formatter::run` at 2.4 KB and `load` (inlined `Catalog::new`) at 1.9 KB. Next
come `Eval::expr` at 1.4 KB and `parse_int` at 0.75 KB. `Source::write`
(fallback escaping) is 0.7 KB and `core::str::from_utf8` is 0.7 KB (gone in the
per-access build's load path). `Eval::category` is 0.6 KB; it contains P0.4's
plural evaluator.

## B12 (`out/b12.txt`)

* Symbol names, from twiggy over the non-stripped build (profile `wasm-syms`)
  and over `wasm-opt -Oz --debuginfo`: **0** matches of
  `core::fmt|alloc::fmt|fmt::Formatter|fmt::Arguments|Debug|Display`. **0**
  matches of `panic|unwrap|expect|bounds|overflow|unreachable|capacity|handle_alloc_error|oom`.
* Panic reachability: the import `p03_panic_reachable` is **absent** after LTO +
  `wasm-opt`, for both UTF-8 variants.
* Data: no panic, bounds or overflow strings in the stripped optimised module.
* Things that failed B12 during the probe and were fixed (coding notes for Phase 3):
  1. `Vec::with_capacity` / `to_vec` / `push` in `Catalog::new` kept
     `capacity_overflow` and `handle_alloc_error` alive. The fix: fallible
     `try_reserve_exact` + `extend_from_slice`, and resolving function names at
     call time instead of building a `Vec` at load.
  2. `str::find('\0')` (core's memchr `CharSearcher`) kept
     `slice_index_fail → panic_fmt` alive. It was replaced by a hand-written,
     panic-free SWAR NUL scan, which is also faster.
  3. The harness's own `copy_from_slice` pulled in `len_mismatch_fail`. That
     was harness noise, replaced by a zipped copy.
  4. Undefined wasm imports need `#[link(wasm_import_module = "…")]`, or
     rust-lld refuses to link.

## Native correctness run (`out/check.txt`)

`check/` (bin `p03`) builds the four workload catalogs with P0.7's writer, loads
them, and formats every message:

* **Reference comparison.** Every message × 4 locales is formatted with
  every argument set: 10 counts for `$count` messages, one set otherwise, plus
  one all-`Unset` set for the fallback path. The runtime reads from the catalog.
  The **reference formatter** walks the *parsed data model* instead, using P0.4's
  independent rational-arithmetic plural reference. The two must produce the
  identical string, including every bidi control, and the same error set. The
  parts output must concatenate to the string output. Result: 0 mismatches in
  26,968 formats.
* **Spot checks** with hand-written expectations. `pl` picks `one/few/many`
  (0→many, 1→one, 2→few, 22→few, 25→many), while `en`, `en-XA` and `ar-XB`
  pick `one/*`. In the RTL `ar-XB`, `{$count :integer}` renders as LRI 1 PDI
  inside the RLO…PDF text. Markup gives
  `text | open:strong | text | close:strong | text`. The canary works. Bogus
  ids (1600, 1<<24, u32::MAX) give an empty output and *missing-message*.
* **Mini corpus** (14 messages, `en` and `ar` catalogs, P0.4 cardinal + ordinal
  entries) for paths the workload lacks: unknown function with `$x`, a quoted
  literal needing escapes, and function-only fallbacks; `.local` over an input
  and chained locals; `select=exact`; exact beating category; `:string`
  selection; *bad-selector*; two selectors with the spec ordering; ordinal
  1st/2nd/23rd/11th; bidi in LTR and RTL messages and with `BidiStrategy::None`;
  *bad-operand*; markup options with literal and variable values; manifest
  mismatch rejected.
* **Mutation run.** 3,000 copies of the `en` catalog each get 1–4 random bit
  flips, 90 % of them in the structure. Each is loaded, and when it loads, every
  message is formatted as string and as parts with 4 argument kinds. Result:
  **0 panics**.

## Caveats

* This is a skeleton, not the product evaluator. The size is the floor of
  catalog reader + evaluator + selection + plural + bidi + parts. The numeric
  formatter (P0.5), the `Host`/NFC glue, `u:` options, the custom-function
  registry and the Leptos glue come on top of it inside B1 (≤ 30 KB gz).
* `NumMode` only handles `i64`. Plural operands for decimals come from P0.5's
  formatter; P0.4 defined the contract.
* `Catalog` holds plain buffers, not `Arc<[u8]>` + `Arc<str>` as in F2's wording.
  Sharing is meant to be `Rc`/`Arc<Catalog>` at the owner. Converting the
  buffers to `Arc` would cost one more copy of the pool.
* The eager strategy keeps the fetched buffer's capacity for the pool, so about
  12 KB of slack (the structure's size) stays allocated. The per-access strategy
  has no slack and no copy.

## Recommendations for Phase 2 / Phase 3

1. **Evaluator shape.** Walk the catalog bytes in place, with lazy declaration
   resolution and allocation-free selection (rank per key compared on the fly).
   Use one walker behind `dyn Out` for both string and parts output. This keeps
   the floor at ≈ 7 KB gz and makes both outputs identical by construction.
2. **Client-path coding rules** to add to 05 §8, each learned from a B12 failure
   here: no infallible `Vec`/`String` growth in client crates (use
   `try_reserve*` or let the caller's `Sink` own growth); no `str::find` /
   `str::split*` on the client path (use hand-written scans); no
   `copy_from_slice` without a proven length; no `checked_pow` (P0.4); mark
   undefined imports with `wasm_import_module`.
3. **Make the B12 CI gate the P0.4/P0.3 pair**: a panic-free no_std harness
   whose panic handler calls an import, plus a twiggy name grep. Both are cheap,
   and the import check is a proof rather than a heuristic.
4. **Resolve functions at call time.** Name lookup against the closed-world
   registry is a handful of short compares per call and needs no load-time
   allocation. Plans/02 §2.2 says "once per catalog load". Keep that only if
   Phase 3 measures a real cost.

## Departures (plans not edited)

* 02 F2 says the buffers are `Arc<[u8]>` + `Arc<str>`. This probe uses `Vec` +
  `String` owned by `Catalog`, shared as `Arc<Catalog>`. That avoids a copy.
* 02 §2.2 resolves the function table "once per catalog load". Here it is
  resolved per call (see recommendation 4).

## Owner questions

None.
