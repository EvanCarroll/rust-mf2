# 10 — Phase 3 work order: the runtime core (layer L4, core files)

Part of the [master plan](00-master-plan.md) (§9, P3). RFC 2119 keywords apply.
Written at the close of Phase 2 from [phase-2-results](phase-2-results.md).
The runtime's structure is specified in [03-runtime](03-runtime.md); this
document orders the work, fixes the API the later phases build on, and sets the
exit criteria.

Phase 3 builds `mf2-runtime` — the evaluator that formats **from a catalog**:
resolution, declarations, selection, fallback, the Default Bidi Strategy,
parts, markup, `u:` options, the registry and the custom-function API,
`:string`, the core numeric semantics (`:number` / `:integer` / `:offset`
over `fixed_decimal`, neutral symbols) and the plural / ordinal evaluator —
plus `mf2-host-std`, a minimal `mf2-host-web`, the plural part of
`mf2-locale-data`, and the `mf2` facade with `compile_str`. It proves them on
every L4 suite file except `functions/{percent,currency,date,time,datetime}.json`,
on native **and** `wasm32-wasip1`, byte-identical, and within B1 (runtime
part), B10 and B12.

## State at the start

| In the tree | Where |
|---|---|
| The data model and parser (Phase 1) | `crates/mf2-model`, `crates/mf2-syntax` |
| The catalog, format version 1 frozen: reader, views, writer (`single`, `catalog`), decoder, manifest | `crates/mf2-catalog`; format in [02](02-catalog-format.md) §2, API in [09](09-phase-2-work-order.md) §"Status at exit" |
| L1/L2/L3 harnesses; ledger at `current_phase = "P2"`; 417 L4 cells `xfail until = "P3"`, the 45 of `functions/{percent,currency,date,time,datetime}.json` `until = "P4"`, every L4d cell `until = "P4"`; the open note `stripped-formats-identically`, `until = "P3"` | `conformance/` |
| Fuzz targets `parse`, `catalog`; seeds by `cargo xtask fuzz-seed` | `fuzz/` |
| Size and reader-cost harnesses: B7 report, reader bench, the B12 wasm check | `bench/catalog-bench`, `bench/b12` |
| CLDR 48.2.1 JSON: plurals, ordinals, numbering systems, the panel's numbers | `third_party/cldr-json/` |
| Phase 0 code to port, **from history only** (`probes/` was deleted at the end of Phase 2; `git show b15b6d6:probes/…`): P0.3's evaluator skeleton (`p0-03-runtime-floor/rt/src/format.rs`, `num.rs`) — patterns, lazy declarations, selection, parts, fallback, bidi over the prototype views; P0.4's plural rule parser, encoder and evaluator with the CLDR sample runner (`p0-04-plural/{rules,eval}`); P0.5's `:number` semantics over `fixed_decimal` (`p0-05-numbers`) | commit `b15b6d6` |

Figures Phase 3 re-measures and must not lose (phase-0-results, phase-2-results):
P0.3's runtime floor 7.0 KB gz (reader + evaluator + selection + plural +
bidi + parts; the reader alone now measures ≤ 6.8 KB gz *with* a walk over its
whole API, `Catalog::new` 3.4 KB raw); P0.4's evaluator 0.43 KB gz, 15,041 /
15,041 CLDR samples; P0.5's core numeric semantics 10.2 KB gz (8.5 `no_std`),
70/70 suite, 0 ECMA-402 diffs; P0.8's simple lookup 20.7 ns / 0 allocs and
1-argument pattern 93.5 ns / 0 allocs (reused `String`), select 317 ns.

Carried — **owner decisions**:

1. **`fixed_decimal`'s panic paths** (03 §5.2): P0.5 found six panic entry
   points reachable through `fixed_decimal` 0.7 and `smallvec`, which break B12
   on the numeric path. Accept (and restate B12), fix upstream, or an own
   panic-free digit buffer (which D1's rule requires to come with a baseline, a
   gate and a fallback). **Phase 3 cannot exit with B12 red**, so this is needed
   before A5 lands. — **Decided (owner, 2026-09-21): the own buffer (D15)**,
   A/B-tested against `fixed_decimal` behind the same interface (A5b) and
   adopted only if it is at least as good.
2. Unchanged: the remote host (the CI `runs-on` label); the spec license
   (#1112); B5/B9 restatements (B7 was restated on brotli at the end of Phase
   2); whether the D1 gate runs on every push; the tz database (P4).
3. **New, decided (owner, 2026-09-21):** `syntax.json` #90 needs French
   number symbols (Phase 4), so its L4 cell is due at P4, not P3 — the one
   exception outside the five function files (01 §3, master plan P3 exit).

Seam kept for catalog text as JS strings
([stretch_goals_after_v1/prob_builtin_strings](stretch_goals_after_v1/prob_builtin_strings.md)
§7) — Phase 3's: **`Sink` has a catalog-text method**
(`push_catalog_text(&Catalog, StrRef)`, defaulting to
`push_str(catalog.text(r))`), and `Formatter::simple` has a variant that
yields the `StrRef`. The evaluator writes catalog text only through it.

## What Phase 2 changes here

* **Walk the views; never decode.** The evaluator reads a message through
  `mf2_catalog::{MsgView, Declarations, PatternView, …}` (frozen with the
  format). `decode` is build-side (feature `decode`) and never linked into a
  client. There is no "format from a model" path: `compile_str` and the L4
  harness go `parse_model` → `validate` → `analyze` → `writer::single` →
  `Catalog::new` → format.
* **What MESSAGES gives the evaluator** (02 §2.2): names, keys and function /
  option / markup identifiers already in NFC, so option names compare with
  plain `==` against `"minimumFractionDigits"`, `"u:dir"`, …; keys are NFC, so
  `:string` selection only normalizes the *runtime* operand (quick check, then
  `Host`); attributes are not there (the spec says they have no effect).
* **Variables.** `VarRef::External(slot)` is the call site's argument
  `args[slot]`; `VarRef::Local(i)` is the `i`-th `.local`. An `.input {$x …}`
  is a declaration whose operand is `External(slot)`; later references to `$x`
  are also `External(slot)` — the evaluator MUST map a slot to its `.input`
  (the spec: the declaration's value replaces the argument). A `.local` that
  shadows by name is a Duplicate Declaration the build rejects; the format
  still resolves references to the latest binding.
* **Names for fallback output** come from `MsgView::names()` in O(1)
  (`Names::var(VarRef)`), the local names as declared, the slot names in NFC.
* **Functions are per-call** `catalog.function(index)` → `ns:name` → registry.
  Resolve once per catalog load only if A11 measures a per-call cost worth a
  load-time table (02 §2.2).
* **Malformed data costs one message.** Views yield `Err(Malformed)` and end;
  `text()` returns `None` for a bad string. The evaluator then formats that
  message's fallback and reports an error; nothing panics. Per 02 §2 "Cost
  bounds", formatting a message is linear in the bytes it reads.
* **UTF-8 per access dominates non-Latin lookups.** A simple `get` + `text` is
  19.3 ns on `en` but 65.6 ns on `pl` and 77.8 ns on `en-XA` (up to 107 ns under
  load); `str::from_utf8` is 80–85 % of it. B10's ≤ 100 ns is stated for the
  reference `en`; A11 measures the evaluator on all four locales and, if a
  non-Latin locale misses, weighs the options (a faster validator, validating
  the pool once lazily, a cached validity bit) against F2/F4 — with numbers.
* **B1 accounting.** The reader is ≤ 6.8 KB gz with a walk over its whole API.
  `Catalog::new` validates IDS and the client links `lookup`, although
  production catalogs strip IDS; IDS validation at load can be dropped (every
  IDS access is bounds-checked) if B1 needs the bytes.
* **The open note** `stripped-formats-identically` is due at P3: L4 formats
  every applicable test from the stripped catalog too and requires identical
  output, then the note closes (`status` → a fact, or removed).
* **Plural data.** `bench/catalog-bench` hard-codes P0.4's `plural.cardinal`
  bytes for its four locales; `mf2-locale-data` (A6) replaces them.
* The L3 harness's `formatting_model` (`conformance/src/l3.rs`) is the model
  a stripped catalog formats; reuse it in A9's stripped check.

## API of `mf2-runtime` (written in A1, frozen at exit)

Phases 4 (functions), 5b (macros) and 6 (Leptos) are written against it.
Methods may be added; changing one after exit needs this document (or its
successor) changed in the same commit. Crate rules: `#![no_std]` + `alloc`,
`#![forbid(unsafe_code)]`, client-path code (no `format!`, `Debug`/`Display`
use, `unwrap`, panicking indexing; `try_reserve*`; the 05 §8 rules). A1 writes
the exact signatures into 03 §2 before code; the shape, from 03 §2 and what
Phase 2 fixed:

```rust
pub struct Formatter<'c> { /* &'c Catalog, &'c Registry, &'c FormatContext */ }
pub struct FormatContext { pub bidi: BidiStrategy, pub host: &'static dyn Host, /* time zone: P4 */ }

impl<'c> Formatter<'c> {
    pub fn new(catalog: &'c Catalog, registry: &'c Registry, cx: &'c FormatContext) -> Self;
    pub fn simple(&self, id: MsgId) -> Option<&'c str>;          // no evaluator, no allocation
    pub fn simple_ref(&self, id: MsgId) -> Option<StrRef>;       // the seam
    pub fn write(&self, id: MsgId, args: &[Arg<'_>], out: &mut dyn Sink, errs: &mut dyn ErrorSink);
    pub fn parts(&self, id: MsgId, args: &[Arg<'_>], out: &mut dyn PartSink, errs: &mut dyn ErrorSink);
    pub fn write_named(&self, id: MsgId, args: &[(&str, Arg<'_>)], out: &mut dyn Sink, errs: &mut dyn ErrorSink);
}

pub trait Sink { fn push_str(&mut self, s: &str);
                 fn push_catalog_text(&mut self, cat: &Catalog, r: StrRef) { /* default via push_str */ } }
pub trait PartSink { fn part(&mut self, p: Part<'_>); }      // Text, BidiIsolation, Expression, Markup, Fallback
pub trait ErrorSink { fn error(&mut self, e: FormatError); } // FormatError: the suite's kinds, a plain enum
pub enum Arg<'a> { Str(&'a str), Int(i64), Float(f64), Decimal(&'a str), Custom(&'a dyn CustomValue), Unset }
                                                              // DateTime: P4, additive
pub trait Host { fn nfc<'a>(&self, s: &'a str, buf: &'a mut String) -> &'a str; }
pub struct Registry { /* closed world: only the handlers the corpus uses */ }
pub trait Function: Sync { /* resolve → a ResolvedValue: write, parts, matches, better_than, dir */ }
```

`Arg` is small on purpose (03 §2: every variant is a code path in the wasm).
The three `:test:*` functions of the suite live in the conformance crate and
are written against the public `Function` trait only (03 §3): if they cannot
be, the trait is wrong.

## Part A — tasks (A1 first; A2–A5 in order; A6–A12 as their inputs exist)

| Task | Deliverable | Done when |
|---|---|---|
| **A1** API | The exact public API of `mf2-runtime` (above) into 03 §2 before code: types, traits, the `Part` shape `expParts` asserts, `FormatError` ↔ the suite's error names, the `Host` trait, the registry's construction (closed world, B13), the `:test:*` feasibility check against `Function`. | 03 §2 has no "sketch" left |
| **A2** Evaluator | `crates/mf2-runtime`: resolution over the views — literals, variables (slots, `.input` mapping, locals, lazy evaluation of each declaration at most once), function calls through the registry, option resolution (literal / variable), fallback representations (`{$name}`, `{\|lit\|}`, `{:fn}`) from NAMES, errors collected (formatting never fails); markup (no text in string output; parts with name, options, `u:id`; `u:dir` on markup = Bad Option); the `u:` options of the pinned spec (`u:id`, `u:dir`). Port P0.3 from history. | unit tests; `cargo build -p mf2-runtime --target wasm32-unknown-unknown`; clippy denies the panic lints |
| **A3** Selection | The spec's algorithm: resolve selectors, filter by `matches`, sort by `BetterThan`, first best in source order; `:string` selection under NFC (quick check, then `Host`); bad-selector / bad-variant-key errors; missing fallback handled per spec. | `pattern-selection.json` green at L4 |
| **A4** Bidi and parts | Default Bidi Strategy (message `dir` from the catalog header, each value's `dir`), `None`; parts output (`Text`, `BidiIsolation`, `Expression`, `Markup`, `Fallback`) sharing one walker with string output; the parts concatenate to the string. | `bidi.json`, `u-options.json`, `fallback.json` green at L4 |
| **A5** `:string` and core numbers | `:string`; `:number` / `:integer` / `:offset` over `fixed_decimal` with every digit and rounding option, `signDisplay`, `select` = `exact` / `plural` / `ordinal`, option inheritance, operand rules, neutral symbols. Port P0.5 from history. **Needs owner decision 1.** | `functions/{string,number,integer,offset}.json` green at L4; P0.5's ECMA-402 differential re-run |
| **A5b** Digit buffer A/B (D15) | The numeric code over the own buffer and over `fixed_decimal` (feature `fixed-decimal`, same interface): the suite, a differential on one random corpus of values × options (display, plural operands, exact-match text), wasm size of the core numeric semantics, allocations, B12, speed. Record the result; keep the own buffer only if it is no worse on every row. | the A/B committed (`bench/`), the decision recorded in 03 §5.2 |
| **A6** Plural and `mf2-locale-data` | The P0.4 evaluator in `mf2-runtime` (plural operands from the *formatted* number); `crates/mf2-locale-data` (plural part): UTS #35 rule parser, the canonical encoder of 02 §4.1, all-locale tables from `third_party/cldr-json` (subtag truncation, then root), the CLDR `@integer`/`@decimal` samples as tests; `bench/catalog-bench` takes its plural entries from it. | 15,041 / 15,041 CLDR samples, cardinal and ordinal, every locale; the catalog-bench bytes unchanged |
| **A7** Hosts | `crates/mf2-host-std` (NFC via `unicode-normalization`; the host for native and `wasm32-wasip1`); a minimal `crates/mf2-host-web` (`String.prototype.normalize` through `js-sys`). | used by A9 (native, wasip1) and A10 (web build only) |
| **A8** Facade | `crates/mf2`: re-exports, the client feature flags of the master plan §5 that exist by now, `mf2::compile_str` (std: parse + validate — rejecting syntax and data-model errors with their kinds — + analyze + `writer::single`, returning the catalog and its manifest). | used by A9; doc examples compile |
| **A9** Conformance L4 | `conformance/src/l4.rs`: every L4-applicable test (01 §3, §4): error tests — the one-message compile rejects with that kind; the others — format from the catalog with `params`, `locale`, `bidiIsolation`; assert `exp`, `expParts`, `expErrors` (absent ⇒ none); `dyn` tests through `write_named`; the `:test:*` functions via the public trait. The same from the **stripped** catalog, output identical — then close the `stripped-formats-identically` note. **wasm32-wasip1**: suite catalogs compiled natively, embedded, formatted under wasmtime (a current version; the machine's 0.28 is too old — install via the toolchain's means, pinned), byte-identical to native. `HARNESSED` gains L4; `--promote`. | L4 green for every file except `functions/{percent,currency,date,time,datetime}.json` (those stay `xfail until = "P4"`), native and wasip1 |
| **A10** Generated input and fuzzing | L4 on ABNF-generated messages with generated arguments: never panics, output deterministic, native = wasip1 (a sampled run under wasmtime). Fuzz target `format`: catalog bytes (as in `catalog`) + argument bytes → `write` and `parts` on every message: no panic, a time budget per byte. | bounded in `cargo test`, 1,000,000 nightly; a ≥ 1 h fuzz run clean |
| **A11** Budgets | **B1** (runtime part) and **B12** in a client-only harness (extend `bench/b12`: the reader + evaluator + `:string` + core numbers + plural + bidi + parts, delta against the base; fmt and panic imports absent); **B13**: a harness built without the numeric handlers links no numeric code. **B10** native, parser-gate style (`bench/`): simple ≤ 100 ns / 0 allocs, 1-argument pattern ≤ 500 ns / ≤ 1 alloc on all four locales, against P0.8 — and the non-Latin UTF-8 finding above decided with numbers. Per-call vs load-time function resolution measured. | B1 runtime part, B10 (`en`), B12, B13 met or the budget restated with the owner; the reports committed |
| **A12** Ledger hygiene | The L4d (default-features) columns stay `xfail until = "P4"`; every L4 xfail carries a reason; `current_phase = "P3"` in the exit commit with the harness green and no open note due. | `cargo xtask conformance-report` green at P3 |

Order: A1 unblocks everything; A2 → A3 → A4 are the core; A5 needs owner
decision 1 and A2; A6 alongside A3 (selection needs plural); A7 and A8 as
soon as A2 exists; A9 grows with A2–A6; A10–A11 alongside.

## Exit (master plan §9, P3)

- [ ] L4 green for every suite file except
      `functions/{percent,currency,date,time,datetime}.json` — including
      `functions/{string,number,integer,offset}.json` — native and
      `wasm32-wasip1`, byte-identical; `current_phase = "P3"` bumped in the exit
      commit and the harness green
- [ ] stripped and unstripped catalogs format identically on the whole suite;
      the `stripped-formats-identically` note closed
- [ ] CLDR plural samples pass for every locale (cardinal and ordinal)
- [ ] B1 (runtime part), B10, B12 met in a client-only harness (B12 with owner
      decision 1 settled); B13 shown
- [ ] L4 on generated input (1,000,000) and a ≥ 1 h `format` fuzz run clean
- [ ] the `mf2-runtime` API frozen in 03 §2 (or this document changed in the
      same commit, with the reason)
- [ ] the Phase 4 work order written from Phase 3's findings
