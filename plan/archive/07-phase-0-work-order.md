# 07 — Phase 0 work order

Part of the [master plan](00-master-plan.md) (§9, P0). RFC 2119 keywords apply.

Phase 0 bootstraps the repository and replaces every *estimate* and *audit*
figure in the plans with an in-tree measurement. It ends in a go/no-go. **No
product crate is written in Phase 0**; probe code is throwaway.

Work orders are written one phase at a time: each phase's findings change the
next phase's tasks. The last task here writes the Phase 1 work order.

## State at the start

| Already in the tree | Where |
|---|---|
| Plans | `plans/` |
| WG spec + test suite at the pin (`5c4ddb27`, 462 tests / 16 files) | `third_party/message-format-wg/` |
| Pin (only) for the W3C Message Resource draft — license unconfirmed, nothing vendored | `third_party/w3c-message-resource/PIN` |
| Planning-audit scratch code: `Tr` prototype, `RenderEffect` bench, `ox_mf2_parser` bench + conformance run, plural size probes | `probes/audit/` (read its README) |

Not yet present: git repository, Cargo workspace, `xtask`, CLDR inputs, CI.

Tooling checked on the development machine (2026-09-20): `cargo`, `rustup`,
`wasm-opt`, `twiggy`, `cargo-leptos`, `wasm-bindgen`, `wasmtime`, `trunk`,
`cargo-fuzz`, `rust-analyzer`, `node`/`npx`, `podman`, `buildah` present; a
`stable` toolchain and target `wasm32-unknown-unknown` installed; Playwright
browser builds (Chromium, Firefox) present in the user cache. **Missing**: target
`wasm32-wasip1` (`rustup target add`); the `brotli` CLI (not needed — compress
with the `brotli` crate from `xtask`); the Playwright **package** (install it
project-locally under `tools/e2e/` with npm — allowed by the boundary rule). The
machine's default toolchain is a nightly; the workspace pins **stable** in
`rust-toolchain.toml`.

### The owner's part

Three things no agent can settle; none blocks A1–A5, A7 or the probes:

1. **Commits and the remote.** Whether agents may commit, and where the remote
   lives (it decides CI details). Until there is a remote, "CI green" means
   `cargo xtask ci` passes locally.
2. **License of the W3C Message Resource draft** (`third_party/w3c-message-resource/PIN`):
   the repository states none. Until it is confirmed, nothing from it is
   vendored; we work from the grammar in [05](05-tooling.md) §2.
3. **Go/no-go** at the end (C3).

## Part A — bootstrap (A1 → A2, then two tracks: A3 → A4 → A5 → A6, and A7 alone)

| Task | Deliverable | Done when |
|---|---|---|
| **A1** Repository | `git init`; `.gitignore` (`target/`, `Cargo.lock`, `node_modules/`, `*.wasm` outside fixtures, `dist/`); `LICENSE` (MIT); root `README.md` stub. Ask the owner before the first commit and for the remote's host (it decides CI details, A6). | `git status` clean of build output |
| **A2** Workspace | Root `Cargo.toml`: `resolver = "3"`, `exclude = ["probes"]`, `[workspace.package]` (edition 2024, license MIT), `[workspace.dependencies]`, `[workspace.lints]` (`unsafe_code = "deny"` workspace-wide — product crates add `#![forbid(unsafe_code)]` themselves; the bench crates need one `unsafe impl GlobalAlloc` for the counting allocator and `allow` it locally; clippy pedantic subset). Profiles: `release` stays cargo's default (`opt-level = 3`, used by native benches such as the parser gate) plus fat LTO; a separate **`wasm-release`** profile carries `opt-level="z"`, fat LTO, `codegen-units=1`, `panic="abort"`, `strip`. `rust-toolchain.toml`: stable + `wasm32-unknown-unknown` + `wasm32-wasip1`. Members: an `xtask` stub crate (clap, no subcommands yet). | `cargo check --workspace` passes; `cargo check` still works inside `probes/audit/tr-prototype` |
| **A3** `xtask` | clap binary with subcommands `spec-sync [--rev] [--check]`, `cldr-sync`, `resource-sync`, `conformance-report [--init]`, `ci`, `size`, `gen-workload`. Implemented in Phase 0: `spec-sync`, `cldr-sync`, `conformance-report` (A5), `ci` (runs locally exactly what the workflow runs), `gen-workload` (A7); the rest are stubs. `--check` verifies `third_party/message-format-wg` against its `PIN` by re-fetching that commit and comparing trees. | `cargo xtask spec-sync --check` exits 0 |
| **A4** CLDR inputs | Per `third_party/cldr-json/PIN` (already present: tag **48.2.1**; upstream ships `-full` packages only): `cldr-core/supplemental/{plurals,ordinals,numberingSystems,currencyData}.json` (the last two added when P0.5 needed native digits and currency fraction digits); for the probe locale panel (`en es de fr ar he ja hi ru pl cy`) `cldr-numbers-full/main/<loc>/{numbers,currencies}.json` and `cldr-units-full/main/<loc>/units.json`; upstream `LICENSE`. (The all-locales compact tables of [05](05-tooling.md) §7 are Phase 3/4 work, not this.) | files present; `PIN`'s `vendored` field updated |
| **A5** Ledger skeleton | `conformance/` crate `mf2-conformance`: loads the vendored suite, applies `defaultTestProperties`, computes each test's key (`file`, `hash`, `nth`); `cargo xtask conformance-report --init` writes `conformance/ledger.toml` exactly as specified in [01](01-conformance.md) §4 — `current_phase = "P0"`, the `n/a` matrix for error tests, every applicable cell `xfail` with `until` from the layer → phase table in 01 §3, plus the `L4d`/`L5d`/`L6d` columns; one test asserts ledger ↔ suite is a bijection on the key. | `cargo test -p mf2-conformance` green; deleting a ledger entry, or duplicating a suite test, makes it red |
| **A6** CI skeleton | `.forgejo/workflows/ci.yml` whose only job step is `cargo xtask ci` (fmt, clippy, test, conformance report) plus uploading the report (Forgejo `upload-artifact` fork). Follow the owner's CI conventions in `CLAUDE.md`; check `https://code.forgejo.org/actions` for a maintained fork before using any action; fetch sources with plain `git` rather than `actions/checkout`. | the YAML parses, `cargo xtask ci` is green locally; a green hosted run is required only once the owner has provided a remote |
| **A7** Workload generator (needs only A2; run it in parallel with A3–A6 — it gates P0.1, P0.7, P0.9, P0.12) | `bench/workload-gen`: seeded, deterministic. Emits (a) `locales/<tag>/*.mf2` following the working grammar in [05](05-tooling.md) §2 **and the same messages as flat JSON** (`id → source`), with the shape of [06](06-size-and-perf.md) §2, plus `en-XA`/`ar-XB` pseudo-locales and canary strings (B6); (b) a cargo-leptos app crate with M call sites spread over K components and R `#[lazy]` routes, in the seven call-site shapes and proportions of 06 §2, with a pluggable call-site template so probes can swap implementations; (c) knobs N, M, L, R, seed, and "add `:number`/`:datetime` messages"; (d) commits the default-seed corpus as `bench/corpora/workload-1600.json` and the suite's `src` list as `bench/corpora/suite.json`. | same seed ⇒ byte-identical output; shape within the tolerances stated in 06 §2 |

## Part B — probes (parallel after A2; A7 where noted)

Each probe lives in `probes/p0-NN-<name>/` as a standalone crate (own
`[workspace]`), and writes `RESULT.md` beside its code: the question, the exact
commands, the numbers, the conclusion, caveats. Probes never edit `plans/`
directly — C1 merges them — so parallel agents do not collide. Definitions,
methods and thresholds are in [06](06-size-and-perf.md) §5; this table adds
order, inputs and starting points.

| Probe | Needs | Start from | Notes |
|---|---|---|---|
| **P0.1 call-site cost — go/no-go** | A7 | `probes/audit/tr-prototype` | Reproduce 14 B/site **after `wasm-opt -Oz`**; then all seven shapes of [06](06-size-and-perf.md) §2 in its proportions — note that ~45 % of real sites are the non-view `String` path, which the audit never measured — with args and signal-valued args vs. an enclosing closure. Report per shape and the weighted average. Run first. |
| **P0.2 vertical slice — go/no-go** | A2 | `probes/audit/tr-prototype` | cargo-leptos + Axum app, hand-built two-locale catalog, shell `<link rel=preload>`, fetch → install → hydrate, `set_locale`, one `#[lazy]` route under `--split`, and one streamed `Suspense` boundary containing a `tr!` (does the render-time catalog lookup see the request's context? — D9). Browser-driven check (Playwright under `tools/e2e/`): zero console warnings, catalog request overlaps the wasm request. Fold **P0.10** (hydration tolerance: text difference, then structural difference) into this app. Run first. |
| P0.3 runtime floor | A2, P0.7's encoding sketch (same stream) | — | `no_std` skeleton: catalog reader for P0.7's encoding + pattern + select + parts. Record gz size and the `core::fmt` symbol check (B12). |
| P0.4 plural | A4 | `probes/audit/plural-size`, `plural-payload` | Turn the `hand` evaluator into a correct one: parse UTS #35 rules from `plurals.json`/`ordinals.json`, encode, evaluate; run **every CLDR `@integer`/`@decimal` sample for every locale**. Report code size, data size, and failures. Settles D3. |
| P0.5 numbers | A4 | — | Own formatter over `fixed_decimal` with symbols passed as data vs. `icu_decimal` + blob vs. `Intl.NumberFormat` glue; every REQUIRED `:number` option. Sets B2/B3. |
| P0.6 dates | A2 | — | `icu_datetime` + blob vs. `Intl.DateTimeFormat` glue. Sets B4. |
| P0.7 catalog encoding | A7 | — | Encode the generated corpus per [02](02-catalog-format.md) §2 (simple + pattern + select only); raw/gz/br; single vs. split string pools; fixed vs. varint index. Sets B7. |
| P0.8 load + lookup | P0.7, P0.3 | — | wasm, desktop and 4× CPU throttle. Sets B9/B10; decides UTF-8 validation strategy. |
| P0.9 build orchestration | A7 | — | i18n crate + `build.rs` + generated `tr!` wrapper with baked manifest path → proc-macro. Check cargo-leptos double build, incremental rebuild after editing one message, a second crate depending on the i18n crate, rust-analyzer expansion, wall-clock of 2,000 expansions. Settles D8. |
| P0.11 node update strategy | P0.1 | `probes/audit/render-effect-bench` | Effect-per-node (A) vs. library registry (B): 2,000 live nodes + a list churning 100k nodes; heap growth, switch latency, bytes. Settles D7. |
| P0.12 parser baseline | A7 | `probes/audit/ox-parser-bench`, `ox-conformance` | Reproduce the D1 baseline table; note the one-`SourceStore`-per-message caveat and also measure a shared store. Build it as the permanent crate `bench/parser-gate/` over the committed corpora, under the comparison rules of [05](05-tooling.md) §1, so Phase 1 only has to add `mf2-syntax` beside ox. It is **not** a probe and survives C5. |

Suggested agent split: one agent for Part A; then P0.1 + P0.11 (same code) ·
P0.2 (+P0.10) · P0.3 + P0.7 + P0.8 · P0.4 · P0.5 + P0.6 · P0.9 · P0.12 — seven
independent streams.

## Part C — close-out (sequential; after all probes)

| Task | Deliverable |
|---|---|
| **C1** Results | `plans/phase-0-results.md`: one section per probe, merged from the `RESULT.md` files, each ending in *threshold met / not met*. |
| **C2** Re-baseline the plans | Replace every figure marked *estimate* or *audit* in `plans/` with the measured one; adjust budgets B1–B13 where the evidence says so, stating why; record D3, D7, D8 as settled with their evidence. |
| **C3** Go/no-go | A short recommendation to the owner. **No-go conditions**: P0.1 above 40 B gz/site with no credible fix; P0.2 cannot hydrate cleanly or the catalog cannot be fetched in parallel with the wasm; P0.9 cannot give correct incremental rebuilds under cargo-leptos. |
| **C4** Phase 1 work order | `plans/08-phase-1-work-order.md`, including the **frozen public types of `mf2-model`** (so P2/P5a can be planned against them) and the `Frontend` trait. |
| **C5** Clean up | Delete `probes/` (its `RESULT.md` files are already merged into `plans/phase-0-results.md`). `bench/`, `conformance/`, `xtask/`, `tools/e2e/` stay. |

## Exit checklist (master plan §9, P0)

- [x] A1–A7 done; `cargo xtask ci` green (no remote yet, so no hosted run)
- [x] P0.1 and P0.2 meet their thresholds (and P0.9)
- [x] every other probe has a `RESULT.md`, threshold met or the plan adjusted
      (merged into [phase-0-results](phase-0-results.md))
- [x] D3, D7, D8 settled with evidence (D9 verified)
- [x] no *estimate* / *audit* figure remains in `plans/`
- [x] Phase 1 work order written ([08](08-phase-1-work-order.md))
- [x] owner has given go/no-go — **go** (recommended in
      [phase-0-results](phase-0-results.md) §C3; Phase 1 started and finished
      2026-09-21)
- [ ] C5 (delete `probes/`) — unblocked: the first commit holds the probe
      code; scheduled in [09](09-phase-2-work-order.md) (after Phase 2 has
      ported from P0.3 and P0.7)
