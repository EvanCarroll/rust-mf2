# runtime-bench — Phase 3's runtime measurements

`plans/10-phase-3-work-order.md` tasks A5, A5b and A11 (native half).

| Command | What |
|---|---|
| `cargo run --release -p runtime-bench -- numbers corpus 100000` | P0.5's random `:number` corpus through the runtime, one line per case (display, errors, cardinal and ordinal selection) |
| `cargo run --release -p runtime-bench -- numbers ecma 100000 \| node bench/runtime-bench/ecma-diff.cjs` | P0.5's ECMA-402 differential against `Intl.NumberFormat` (node is a local tool; no network) |
| `cargo run --release -p runtime-bench -- numbers speed` | ns, allocations and bytes per `:number` format |
| `bash bench/runtime-bench/number-ab.sh` | the D15 A/B: all of the above built with and without feature `fixed-decimal`, compared; report in `NUMBER-AB-P3.md` |

The size and B12 rows of the A/B come from `bench/b12/check.sh`
(`b12-runtime` against `b12-runtime-fixed`). The binary links
`catalog-bench`'s counting allocator.
