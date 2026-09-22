# intl-probe — Phase 4 task A0, the `intl` client option

`plans/11-phase-4-work-order.md` §"The `intl` client option" and row A0;
the design it follows is `plans/03-runtime.md` §5.3. Results:
[`RESULTS.md`](RESULTS.md).

The question: should the client wasm leave rounding, digit output and plural
selection to the browser's `Intl` (ECMA-402), keeping MF2's semantics in
Rust, while servers keep the Rust path? The probe builds that option for the
numeric functions against today's Rust path and measures the six things A0
lists: size, speed, agreement (neutral and with locale symbols), plural
selection, the browser floor, and the L4 number files per engine.

## What is here

A standalone cargo workspace (the root workspace excludes
`bench/intl-probe`), depending on `crates/` by path:

| Crate / directory | What |
|---|---|
| `fn/` — `intl-probe-fn` | The handlers over `Intl`: `:number`, `:integer`, `:offset` (neutral output), the same localized, `:percent`, `:currency`, `:unit`. They implement the public `mf2_runtime::Function` trait; client-path rules (`no_std` + `alloc`, `forbid(unsafe_code)`, no `format!`/`Debug`/`Display`, no `unwrap` or panicking indexing). `options.rs` is **ported** from `crates/mf2-runtime/src/number/options.rs` at 3fc4735 (the option tables and ECMA-402's `SetNumberFormatDigitOptions`, which are `pub(crate)` there); `host.rs` holds **the `Host` methods the option would add**, as wasm-bindgen imports of a small inline JavaScript (the probe does not change `mf2_runtime::Host`). It links none of the runtime's digit plan, rounding, display or plural evaluator. |
| `wasm/` — `intl-probe-wasm` | The harness cdylib: the runtime with one registry per variant (a cargo feature each), `mf2-host-web` as the host, and the same wasm-bindgen exports in every variant (`load`, `format`, `parts`, `errors`, `format_all`, `bench_int`, `bench_float`, `slots`). std, as a Leptos client is: allocator, panic runtime and wasm-bindgen glue are in `base` and cancel in the deltas. |
| `native/` — `intl-probe-native` | The build side (clap CLI): compiles every catalog the page loads natively (`mf2::compile_str` for L4, multi-message catalogs through `mf2_catalog::writer::catalog` otherwise), the CLDR samples, the edge cases and the locale-symbol cases; formats those natively through the Rust registry (`loc-format`); `observe`; `catalog-data`. |
| `web/` | The page (`index.html`, `probe.mjs` → `window.mf2probe`) and `node.mjs` (the same items in node); `lib/` holds one module per item, shared by both. |
| `scripts/` | One script per item, the build, the data, the report. |
| `../../tools/e2e/checks/intl.mjs` | The Playwright check that serves the page and runs the items in Chromium, Firefox and WebKit. |

### Variants (`wasm/Cargo.toml` features, one per build)

| Variant | Registry |
|---|---|
| `base` | `:string` only (the size base) |
| `rust` | + `mf2_runtime::functions::{NUMBER, INTEGER, OFFSET}` — today's Rust path |
| `intl` | + the probe's `:number` / `:integer` / `:offset` over `Intl`, neutral output |
| `intl-loc` | the same with the catalog locale's symbols, + `:percent` |
| `intl-cu` | + `:currency`, `:unit` |
| `intl-codes` | `intl` with design A of the key (below) |
| `rust-loc` | `mf2-fn-number`'s localized `:number` / `:integer` / `:offset` and `:percent` (built when the tree has `mf2-fn-number`, Phase 4 A3, commit d56e089 on; its `:currency` / `:unit` join in A4) |

### Design points

* **Semantics in Rust, digits from `Intl`.** Resolution is the runtime's step
  for step (operand rules, inheritance and the discard lists, `select`'s
  literal-only rule, the option table, `:offset`'s exact addition, the digit
  plan and its *Bad Option* reports). `Intl.NumberFormat` gets the exact
  decimal as a string and the *resolved* digit options (so it never throws
  where MF2 reports and continues); neutral output is locale `en`,
  `numberingSystem: "latn"`, `useGrouping: false`. `Intl.PluralRules` gets
  the same digit options; the category is computed once per value, not per
  key. `:integer`'s rounded value comes from `Intl` too (maximum fraction
  digits 0). Exact keys compare with the plain integer or with the neutral
  display, as the runtime does.
* **Two key designs**, both measured (`fn/src/host.rs`): B (default) sends
  `name=value` pairs taken from the option tables the wasm already holds (MF2's
  option names are Intl's), so the JavaScript is generic; A (feature
  `key-codes`, variant `intl-codes`) sends 17 one-character codes and the
  JavaScript holds Intl's names. The JavaScript caches one `NumberFormat` and
  one `PluralRules` per key (FIFO, 256 keys).
* **Probe-only costs**, which an option inside the runtime would not have: a
  resolved value is a `Value::Boxed` (the runtime's `Number` is opaque), one
  allocation per numeric resolution; the probe's own exact-decimal text code
  (`fn/src/text.rs`) stands in for the runtime's private `Decimal::add`.
* **Unit coverage is Intl's**: a well-formed unit `Intl` does not sanction
  (it sanctions 45 units and their `-per-` compounds) is *Unsupported
  Operation*; `usage` is *Unsupported Operation* (no conversion).

## Prerequisites

* The repository's toolchain (`rust-toolchain.toml`) with target
  `wasm32-unknown-unknown`; `wasm-bindgen` CLI **0.2.128** (equal to the
  pinned library: `cargo install wasm-bindgen-cli --version 0.2.128`),
  `wasm-opt` (binaryen), `twiggy`, `gzip`, `node` (≥ 20).
* `tools/e2e`: `npm install`, and the browser builds (its README; WebKit
  goes to `target/ms-playwright`, see there).
* The machine is memory-tight: every script builds with `CARGO_BUILD_JOBS=3`
  unless it is set.

## Run

From the repository root. Every script takes `INTL_PROBE_REV=<rev>` to build
against commit `<rev>`'s crates instead of the working tree (a snapshot under
`target/intl-probe/snap-<sha>/`, the probe copied on top — so a measurement
pins the runtime it measures while `crates/` moves):

```sh
bench/intl-probe/scripts/build.sh      # item 1 (= 1-size.sh): every variant, sizes → target/intl-probe/size.md; the page's modules → target/intl-probe/pkg/
bench/intl-probe/scripts/data.sh       # the data every item reads → target/intl-probe/data/ (includes runtime-bench numbers ecma 100000)
bench/intl-probe/scripts/5-floor.sh    # item 5: feature detection
bench/intl-probe/scripts/6-l4.sh       # item 6: the L4 number files per engine
bench/intl-probe/scripts/4-plural.sh   # item 4: the CLDR samples through Intl.PluralRules
bench/intl-probe/scripts/3-agreement.sh  # item 3: ECMA-402 cases, edge cases, locale-symbol cases
bench/intl-probe/scripts/2-speed.sh    # item 2: rust vs intl (and rust-loc vs intl-loc), alternated; Chromium also at 4× throttle
bench/intl-probe/scripts/all.sh        # all of the above in order
node bench/intl-probe/scripts/report.mjs [size speed ecma edge loc plural floor l4]   # the tables → target/intl-probe/report.md
```

Items 2–6 run `tools/e2e`'s check in every browser (`node run.mjs intl
--browser all`, items chosen by `MF2_INTL_ITEMS`) and, except speed, in node
(`node bench/intl-probe/web/node.mjs <item>`), then print the report.
Pieces on their own:

```sh
cd tools/e2e && MF2_INTL_ITEMS=l4,floor node run.mjs intl --browser chromium
node bench/intl-probe/web/node.mjs ecmaHandlers --out /tmp/x.json
target/intl-probe/snap-<sha>/src/bench/intl-probe/target/release/intl-probe-native observe   # the :offset zero-sign observation
```

Outputs (all generated, none committed): `target/intl-probe/size*.md`,
`twiggy-<variant>.{txt,csv}`, `pkg/`, `data/`, `results/<browser>.json`,
`results/node-<item>.json`, `results/loc-<engine>.json`,
`results/speed-<browser>-<stamp>.json` (every speed run is kept; the report
gives the range over runs), `report.md`.

Timings: the development machine's clock drifts (1.3–4.1 GHz seen) and other
agents build on it; the variants are therefore alternated inside one page
run, the MHz and the load are recorded with every run, and figures are given
as ranges over runs.
