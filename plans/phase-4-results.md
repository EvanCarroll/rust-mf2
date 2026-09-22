# Phase 4 results

Part of the [master plan](00-master-plan.md) (§9, P4); the record behind the
exit checklist of the [Phase 4 work order](11-phase-4-work-order.md). Every
figure names the command that produced it. Measured 2026-09-22 on the Phase 0
machine (Intel Core i7-1165G7, 4 cores / 8 threads, Linux 6.12), toolchain
stable 1.98.1; wasm-opt 120, twiggy 0.8.0, GNU gzip 1.13, wasmtime 49.0.0
(pinned, in `target/tools`), node v23.11.0 (ICU 76.1, CLDR 46) as the local
`Intl` reference. **Sizes do not depend on load; timings do**, and the clock
drifts (1.8–2.3 GHz within the day): timings are ranges, and two builds are
compared only by alternating their binaries.

*In progress: this file grows with the phase; the summary table is written at
exit.*

## A4 — `:currency` and `:unit`

L4 485/485 in the all-features configuration since A4: `functions/currency.json`
(12) and `conformance/extra/functions/unit.json` (23, ours, WG schema) moved
`xfail` → `pass` (`cargo test -p mf2-conformance --test layers`). The panel
goldens (`conformance/goldens/currency.tsv`, 1,170 cases; `units.tsv`, 1,053)
were compared before committing with node's `Intl.NumberFormat`: **2,106 of
2,106** comparable cases identical (the 117 `currencyDisplay=never` cases have
no `Intl` counterpart); `cargo xtask l4-wasi` formats all 4,095 golden cases
identically on native and `wasm32-wasip1`.

## A11 — B2, B3, B1′ (2026-09-22)

`bash bench/b12/check.sh` (wasm-release, `wasm-opt -Oz`, `gzip -9 -n`; each
figure a delta between two harnesses that differ only in their registry):

| Budget | Harnesses | Δ raw | **Δ gz** | Limit |
|---|---|---:|---:|---|
| B2: `fn-number` on and used (localized `:number` / `:integer` / `:offset`, `:percent`, unannotated numbers) | `runtime-fn-number` − `runtime` | 4,101 | **2,069** | ≤ 3,072 |
| B3: + `:currency`, `:unit` | `runtime-fn-number-measure` − `runtime-fn-number` | 11,770 | **5,432** | ≤ 5,632 (restated; was 4,096) |
| B1′: `fn-number` linked, unused | `runtime-fn-number-unused` − `runtime` | 0 | **0** | +0 |

B12 holds for all three (no panic import after LTO + `wasm-opt -Oz`, no
`core::fmt` symbol). **B3 as first built measured 6,048 B gz.** A twiggy diff
of the two harnesses (`twiggy top` on the `wasm-syms` builds) put it in the
two functions' option loops, their writers and the catalog views' record
readers; what was duplicated or over-general came out without changing any
output — one options loop for both functions over keyword tables, one
`fill` over `dyn FnMut` arguments instead of five monomorphized copies,
blanks split by byte instead of Unicode `is_whitespace` / `trim`, the currency
spacing decided once, the runtime's digit-size parser (`Value::digit_size`) —
6,048 → 5,432 (B2 2,151 → 2,069 with it). **The owner restated B3 to
≤ 5.5 KB gz** (decision 5 of the work order; 06 §3): P0.5's 2.9 KB probe had a
plural stub and `en-US` data only, while the functions as built carry
plural-form currency and unit names, the `…alphaNextToNumber` patterns and
currency spacing, currency-specific patterns and separators, accounting and
narrow symbols, unit-width fallback and `X-per-Y` composition. The option not
taken — the catalog writer precomputing each currency's affixes per display
and sign style and composing literal compound units — was estimated at −0.9
to −1.4 KB gz for a larger catalog and literal-only compound units.
