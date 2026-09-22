# D15 — the own digit buffer against `fixed_decimal` (Phase 3, A5b)

Owner decision 1 of `plans/10-phase-3-work-order.md` (recorded as D15 in the
master plan): the core numeric semantics run over an own panic-free,
allocation-free digit buffer (`crates/mf2-runtime/src/number/decimal/own.rs`),
under D1's rule — `fixed_decimal` 0.7.2 stays behind the same internal
interface (`number/decimal/fixed.rs`, feature `fixed-decimal`) as the baseline
and the fallback, and the own buffer is kept only if it is no worse on every
row. Measured 2026-09-21 on the Phase 0 machine (i7-1165G7), rustc 1.98.1,
node v23.11.0 (ICU 76.1), wasm-opt 120; timings under load (load average
1.8–3.2).

```sh
bash bench/runtime-bench/number-ab.sh      # output, ECMA-402, speed and allocations
bash bench/b12/check.sh                    # size and B12 (b12-runtime vs b12-runtime-fixed)
```

| Row | Own buffer | `fixed_decimal` 0.7.2 | Verdict |
|---|---|---|---|
| Output on P0.5's corpus (100,000 value × option sets: display, errors, cardinal and ordinal selection with exact keys) | — | — | **identical**, line for line |
| ECMA-402 differential (`Intl.NumberFormat('en', {useGrouping: false})`) | 95,675 identical, 0 different; the 4,325 sets `Intl` rejects are all *Bad Option* | the same | equal (P0.5's figures reproduced) |
| Suite (`functions/{number,integer,offset}.json` at L4) | 70/70 | 70/70 (L4 as a whole: 416/462, the same cells — `cargo run -p mf2-conformance --example failures --features mf2-runtime/fixed-decimal -- L4`) | equal |
| Size: the core numeric semantics' share of B1 (`b12-runtime` − `b12-runtime-nonum`, `wasm-opt -Oz`, gzip -9) | **10,227 B raw / 5,142 B gz** | 15,039 B raw / 7,305 B gz | own −2,163 B gz |
| B12 (panic import after LTO + `wasm-opt -Oz`; fmt and panic symbols) | **absent**; 0 and 0 | present; 0 fmt, 8 panic / alloc-failure symbols | own |
| Allocations per `:number` format (20,000 cases) | **0** | 0.50 (8.7 B) | own |
| Speed per `:number` format, interleaved, 5 rounds (median of 21 passes each) | 929.7, 923.3, 841.4, 988.7, 830.2 ns — median **923.3** | 934.5, 1,002.3, 928.3, 914.1, 906.9 ns — median 928.3 | equal within noise (own ahead in 3 of 5 pairs) |
| Operands | up to 40 significant digits, exponent ±9,999; past that *Unsupported Operation* | any length; `:offset` only through `i64` (17 digits: P0.5's `add_small`) | own `:offset` handles 40 digits |

**Verdict: the own buffer is no worse on any row and better on size, B12
and allocations — it stays (D15).** `fixed_decimal` remains behind the
`fixed-decimal` feature as the fallback and as the baseline this report is
re-run against.
