# B12 — the catalog reader has no panic path and no `core::fmt`

Task A9 of Phase 2 (`plans/09-phase-2-work-order.md`), budget B12 and the
reader's share of B1 (`plans/06-size-and-perf.md` §3), and the client-path
rules of `plans/05-tooling.md` §8.

A standalone workspace (the root workspace excludes `bench/b12`) of three
`#![no_std]` `cdylib`s for `wasm32-unknown-unknown`:

| Crate | What it links |
|---|---|
| `b12-harness` (rlib) | Shared scaffolding: the imports (module `b12`), a `#[panic_handler]` that calls the import `b12_panic_reachable`, a bump `#[global_allocator]`, host input into a real allocation that escapes to the host. Panic-free itself. |
| `b12-base` | The scaffolding, reading the same inputs and feeding the same sinks: the size base, with an allocation kept alive. |
| `b12-reader` | The base plus `mf2-catalog` with no features (the client path): `Catalog::new` on bytes read from an import, then every public reader API — the header accessors, `sections`, `function`/`function_count`, `locale_entry`, `get` / `text` / `names` / `fallback_locale` for every id plus one past the end plus one from the host, `lookup` with a host key — and a full view walk: declarations (iterated, then `Declarations::body`, and `MsgView::body`), patterns and parts, expressions, functions, options, markup, selectors, variants, keys, `Names` with one past each end. Every result is folded into an accumulator that goes to an imported sink. |
| `b12-control` | The base plus a deliberate bounds check and a deliberate `write!`: the check must see its panic import and its fmt symbols, which proves the check can fail. |

## Command

```sh
bench/b12/check.sh          # exit 0: B12 clean; 1: B12 fails (or the control shows the check is broken); 2: tool missing
```

It needs `cargo` (the toolchain of `rust-toolchain.toml`, with the
`wasm32-unknown-unknown` target), `wasm-opt` and `wasm-dis` (binaryen),
`twiggy` and `gzip`. It builds with `CARGO_BUILD_JOBS=3` unless set. What it
does, per harness:

1. Build with profile `wasm-release` (opt-level z, fat LTO, 1 CGU,
   `panic = "abort"`, strip — the 06 §3 method) and with `wasm-syms` (the same,
   keeping the name section); then `wasm-opt -Oz` with the probes' feature
   flags (`--debuginfo` for the `wasm-syms` build).
2. **Panic reachability**: the imports of the stripped optimised module
   (`wasm-dis`). `b12::b12_panic_reachable` must be absent from `b12-reader`
   and `b12-base` and present in `b12-control`. An import from any other module
   is an undefined symbol and fails.
3. **Symbols**: `twiggy top` over the non-stripped build, before and after
   `wasm-opt`: no `core::fmt`, `alloc::fmt`, `Formatter`, `Arguments`, `Debug`,
   `Display` symbols, and no `panic`, `unwrap`, `expect`, `bounds`, `overflow`,
   `unreachable`, `capacity`, `handle_alloc_error`, `oom` symbols (twiggy
   demangles v0 names as `core[…]::fmt::…`; the mangled `4core3fmt` spelling
   is matched too). `b12-control` must show both kinds.
4. **Data**: no panic message text in the stripped optimised module.
5. **Size**: raw and `gzip -9 -n` of the stripped optimised modules, and the
   delta `b12-reader` − `b12-base`, plus a twiggy breakdown by crate.

Output: `bench/b12/target/b12/b12.txt` (the report) and `size.tsv`.

## Measured (2026-09-21)

rustc 1.98.1, wasm-opt 120, twiggy 0.8.0, gzip 1.13.

**B12: clean.** The panic import is absent from `b12-reader` after LTO +
`wasm-opt -Oz`; 0 fmt and 0 panic symbols before and after `wasm-opt`
(71 / 74 items); 0 panic strings; the only imports are the harness's five.
The control shows the import and 6 fmt + 3 panic symbols. Mutation check: a
single `bytes[i]` added to `b12-reader` turns the check red (exit 1).

| Harness | raw B | gz B | Δ raw | **Δ gz** |
|---|---:|---:|---:|---:|
| `b12-base` | 472 | 335 | — | — |
| `b12-reader` | 14,085 | 7,122 | **13,613** | **6,787** |

The delta is the reader **plus the harness's walk over every API**: a
client's own walk (the Phase 3 evaluator) replaces the latter. Shallow code
bytes after `wasm-opt` (twiggy, names kept): `Catalog::new` (inlined into the
harness's `load`) 3,373; the other `mf2_catalog` functions 4,390 (largest:
`Catalog::lookup` 796, `Declarations::body` 384, `Declarations::next` 369,
`MsgView::names` 366, `view::expr` 334, `Catalog::text` 318); `core` 784
(`str::from_utf8` 698); the harness walk 5,311, into which the small reader
accessors (`get`, `plane_entry`, `fallback_locale`, `locale_entry`, …) are
inlined; data 205. So the reader alone is ≥ 8.5 KB raw of the 13.6.

Phase 0 for comparison (P0.3, `phase-0-results.md`): the whole runtime floor —
reader + evaluator + selection + plural + bidi + parts, per-access UTF-8 — was
14,045 B raw / **7,022 B gz**, and its `Catalog::new` 1.9 KB raw. The frozen
reader validates more at load (NAMES, FALLBACK, IDS restarts, the plural
entries' structure), hence `Catalog::new` at 3.4 KB; `lookup` (and IDS
validation inside `Catalog::new`) serve dev/tool builds only, since production
catalogs strip IDS.

## The `intl` client option (Phase 4, owner decision 4)

`check.sh` item 8. Three harnesses with the `intl` features, built in a cargo
invocation of their own so that the features never reach the others:

| Crate | What it links |
|---|---|
| `b12-runtime-intl` | `b12-runtime`'s registry (the core's `:number`, `:integer`, `:offset`) with `mf2-runtime/intl`, walked over a stub number formatter (`b12_runtime_walk::run_intl`: the host chooses its answers, every request field goes to a sink) |
| `b12-runtime-fn-number-intl` | `b12-runtime-fn-number-measure`'s registry (the whole localized family, `:currency` and `:unit` included) with the `intl` features, over the same stub |
| `b12-runtime-intl-unused` | the `intl` features on (`mf2-fn-number` linked), a corpus without numbers: `b12-runtime-nonum`'s registry and host |

Gates: B12 for all three; `b12-runtime-intl` links none of the Rust
rounding, digit display or plural evaluator (`number::display::`,
`Decimal>::round`, `plural::select`, `OperandsBuilder`; `b12-runtime` shows
them, so the grep can fail); B1′ for `intl`: `b12-runtime-intl-unused` ≤
`b12-runtime-nonum` in raw bytes. Measured 2026-09-22 (the tree of
`plans/06-size-and-perf.md` §3's `intl` rows): B12 clean in all three; B1′
**−34 B raw / −69 B gz** (a resolved number keeps its digit plan instead of
its rounded digits, so every `Value` is smaller); with the stub formatter
the core is +368 B raw / +110 B gz over `b12-runtime` and the whole family
−12,157 / −5,731 over `b12-runtime-fn-number-measure` — without the
browser's glue, which `bench/intl-probe` measures (the size of the option).
## Dates (Phase 4, A11: B4, B12, B13, B1′ for `mf2-fn-datetime`)

`check.sh` also builds the date harnesses (step 9 of its header). They share
`b12-dates-walk`: `b12-runtime-walk`'s client path with date/time arguments
(an instant, a floating value, an instant at an offset) and the formatting
context's zone (UTC, an offset, a named zone) chosen by the host, over a host
the harness passes — a stub whose `zone_offset` answers a host number for the
native harnesses, `mf2-host-web`'s for the web ones.

| Crate | Registry / host | Measures |
|---|---|---|
| `b12-dates-base` | the core functions; stub host | the base of the date deltas |
| `b12-dates-semantics` | + `:datetime`, `:date`, `:time`, unannotated dates over a backend that writes one byte of the plan | the date semantics every backend needs; B12-gated |
| `b12-dates-neutral` | the same over the neutral stub backend | B12-gated |
| `b12-dates-icu-{greg,any}-{nozones,zones}` | over `Icu<GregorianOnly \| AnyCalendar, NoZones \| WithZones>` (ICU4X from the catalog's `icu.blob`) | B4 `datetime-icu`; B12 reported (ICU4X's own `core::fmt` and panic paths) |
| `b12-dates-web-base` | the core functions; `mf2_host_web::HOST` with the date features on; through `wasm-bindgen` | the base of B4 `datetime-intl`; B1′ for `mf2-host-web` (its JS loads no date glue) |
| `b12-dates-web-plain` | `b12-dates-web-base`'s source, built alone without `mf2-host-web`'s date features | B1′ for those features: the web base must be the same size, wasm and JS |
| `b12-dates-intl` | + the date functions over `Intl`; `mf2_host_web::INTL_HOST`; through `wasm-bindgen` | B4 `datetime-intl`, wasm and JS; adds no fmt / panic symbol, import or text to the web base |
| `b12-dates-unused` | `b12-runtime`'s registry and walk, `mf2-fn-datetime` linked with both backends' features | B1′ (= `b12-runtime`, +0 B), B13 |

Measured 2026-09-22 on the merged tree (rustc 1.98.1, wasm-opt 120, twiggy
0.8.0, wasm-bindgen 0.2.128, gzip 1.13; `bench/b12/check.sh`). The date
semantics are 11 B gz under the note they missed by 4 before the `intl`
option's numeric rework landed beneath them:

| Figure | Δ raw | **Δ gz** | Limit |
|---|---:|---:|---|
| date semantics (`dates-semantics` − `dates-base`) | 7,600 | **3,577** | 06 B4's note: ≤ 3,584 |
| + the neutral backend (`dates-neutral` − `dates-base`) | 8,582 | 4,032 | — |
| B4 `datetime-icu`, Gregorian, zone styles (`dates-icu-greg-zones` − `dates-base`) | 154,958 | **69,641** | ≤ 97,280 |
| … Gregorian, no zone styles | 93,591 | 42,820 | — |
| B4 `datetime-icu`, any calendar, zone styles | 213,235 | **83,028** | ≤ 107,520 |
| … any calendar, no zone styles | 151,113 | 55,476 | — |
| B4 `datetime-intl`, wasm (`dates-intl` − `dates-web-base`) | 10,830 | **5,131** | ≤ 6,144 |
| B4 `datetime-intl`, JS glue (3,132 − 2,464 B gz) | — | **668** | ≤ 1,024 |
| B1′: `fn-datetime` on, unused (`dates-unused` − `runtime`) | 0 | **0** | +0 |
| B1′: `mf2-host-web`'s date features on, `HOST` named (`dates-web-base` − `dates-web-plain`) | 0 | **0** (JS 0) | +0 |

B12: `b12-dates-semantics`, `-neutral`, `-unused` and `-base` clean (no panic
import, no fmt or panic symbol, no panic text); `b12-dates-intl` adds none to
its web base. `mf2-host-web`'s own glue — in the web base too — keeps a panic
path (`alloc::raw_vec::capacity_overflow`, `core::panicking::panic_nounwind_fmt`,
retained through the function table by `wasm-bindgen`'s closure glue and
`js-sys`'s futures, with or without the date features): reported here, for
the Leptos layer's B12 (P6). The ICU4X harnesses show 7 fmt and 11–24 panic symbols, ICU4X's.
B13: 0 date symbols in `b12-runtime`, `b12-dates-base` and `b12-dates-unused`;
20 in `b12-dates-semantics`.
