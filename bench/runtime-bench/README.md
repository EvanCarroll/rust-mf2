# runtime-bench — the runtime measurements (Phases 3 and 4)

`plans/10-phase-3-work-order.md` tasks A5, A5b and A11 (native half).

| Command | What |
|---|---|
| `cargo run --release -p runtime-bench -- numbers corpus 100000` | P0.5's random `:number` corpus through the runtime, one line per case (display, errors, cardinal and ordinal selection) |
| `cargo run --release -p runtime-bench -- numbers ecma 100000 \| node bench/runtime-bench/ecma-diff.cjs` | P0.5's ECMA-402 differential against `Intl.NumberFormat` (node is a local tool; no network) |
| `cargo run --release -p runtime-bench -- numbers locale 100000 \| node bench/runtime-bench/loc-diff.cjs` | Phase 4 (A3): the same corpus localized by `mf2-fn-number` over the locale panel and two non-Latin numbering systems (`ar-EG`, `hi-u-nu-deva`), `:number` and `:percent`, a `useGrouping` value per case, against `Intl.NumberFormat`; differences classified (spaces U+00A0/U+202F, bidi marks, the rest by locale) |
| `cargo run --release -p runtime-bench -- numbers speed` | ns, allocations and bytes per `:number` format |
| `bash bench/runtime-bench/number-ab.sh` | the D15 A/B: all of the above built with and without feature `fixed-decimal`, compared; report in `NUMBER-AB-P3.md` |
| `cargo run --release -p runtime-bench -- b10 --gate --md bench/runtime-bench/B10-P3.md --json bench/runtime-bench/b10-p3.json` | B10 (A11): simple, 1-argument pattern and select on the four production catalogs, P0.8's figures alongside, per-call vs load-time function resolution; exit 1 unless B10 holds on `en` (`--only <row>` runs one row, for profiling) |
| `cargo run --release -p runtime-bench --example select_cost` | what a select costs, piece by piece (unannotated integer, `:integer`, `.input`, select) |

Timings on the development machine drift with its clock (the report
records the MHz): compare two builds by alternating their binaries, never
one run after another.

The size and B12 rows of the A/B come from `bench/b12/check.sh`
(`b12-runtime` against `b12-runtime-fixed`). The binary links
`catalog-bench`'s counting allocator.
