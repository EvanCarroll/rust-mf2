# Phase 0 results

Part of the [master plan](00-master-plan.md) (§9, P0); task C1 of the
[Phase 0 work order](07-phase-0-work-order.md). One section per probe, merged
from the probes' `RESULT.md` files, each ending in **threshold met / not met**.
The probe code under `probes/` is throwaway (C5 deletes it); this document is
the permanent record, so each section keeps the method, the numbers and the
caveats that later phases depend on.

Measured 2026-09-20/21 on one machine (Intel Core i7-1165G7, 4 cores / 8
threads, 16 GB, Linux 6.12), toolchain **stable 1.98.1** (pinned by
`rust-toolchain.toml`), wasm-opt 120, wasm-bindgen 0.2.128, twiggy 0.8.0,
cargo-leptos 0.3.7, node 23.11, Playwright 1.63.0 (Chromium 143, Firefox 155).
Up to six agents built concurrently: **sizes and allocation counts do not
depend on load; wall-clock times do** and are marked where taken under load.

**Size method** throughout (06 §3): `wasm32-unknown-unknown`, profile
`wasm-release` (opt-level `z`, fat LTO, 1 CGU, `panic = "abort"`, `strip`) →
`wasm-bindgen` where applicable → `wasm-opt -Oz` → `gzip -9`, always as a
**delta against a base crate built the same way**. Method lesson (P0.4): a std
base whose allocation LLVM deletes also drops the allocator and std's panic
runtime (≈ 5.1 KB gz), so every delta against it is inflated — bases must keep
an allocation alive (`black_box`). The audit's plural and per-site deltas were
taken against such a base.

**B12 method** (P0.4): besides a `twiggy` symbol check on a non-stripped
build, make the `#[panic_handler]` call an imported `extern` function; if that
import is absent after LTO + `wasm-opt`, no panic path survives. This proves
panic-freedom rather than inferring it from names, and is proposed for the CI
B12 gate.

## Summary

| Item | Gate | Verdict | Headline |
|---|---|---|---|
| A1–A7 | bootstrap | **done** | `cargo xtask ci` green; ledger 462/462; workload shape within tolerance |
| **P0.1** call-site cost | **go/no-go** | **met** | 24.5 B gz/site (`idlit` baseline), 35.7 against the harshest; closure control 97–109; view positions cost 46–102 B gz/site |
| **P0.2** vertical slice | **go/no-go** | **met** | 67/67 e2e checks, Chromium + Firefox, debug + release; D9 verified with a narrowed fallback |
| P0.3 runtime floor | B1 credibility | **met** | 7.0 KB gz ≤ 12; B12 clean; 40,544/40,544 checks, 0 panics in 3,000 corrupted catalogs |
| P0.4 plural | D3 | **met** | 15,041/15,041 CLDR samples; 0.43 KB gz code; ≤ 84 B data/locale |
| P0.5 numbers | B1 share, B2, B3, B8 | **partly met** | core 70/70 suite, 0/95,675 ECMA-402 diffs; numeric share **10.2 KB gz > 6**; B2 1.7, B3 2.9 KB gz |
| P0.6 dates | B4 | **partly met** | `datetime-intl` **5.4 KB gz > 3**; `datetime-icu` 93 KB gz (≈ 100 confirmed) |
| P0.7 catalog encoding | B7 | **met for the reference; pl over by 1 KB** | en 20.5 KB gz; 600 layouts lossless; NUL-terminated strings, byte-plane INDEX; B7 restated to scale with text |
| P0.8 load + lookup | B9, B10 | **met (warm); cold install 1.12 ms** | load 0.08 ms at 4×; simple 20.7 ns / 0 allocs; 1-arg 93.5 ns; UTF-8 validated per access |
| **P0.9** build orchestration | **go/no-go**, D8 | **met** | every edit scenario rebuilds correctly under cargo and cargo-leptos; macro overhead +0.2–0.3 s per 2,000 sites; rust-analyzer expands it; 4 amendments to D8 |
| P0.10 hydration tolerance | risk sizing | **confirmed / documented** | text differences silent; a structural difference makes stock tachys panic |
| P0.11 node update | D7 | **met — D7 = B** | registry: 44.8 B/node, flat heap under 100k churn; switch 6.8 ms at 4× (effects: 427 B/node, +72 B/node leak) |
| P0.12 parser baseline | D1 baseline | **done** | ox baseline re-measured on the committed corpora; ox 460/462 |

## C3 — go/no-go recommendation

**Recommendation: go.** None of the three no-go conditions of the work order
holds:

| No-go condition | Evidence |
|---|---|
| P0.1 above 40 B gz/site with no credible fix | 24.5 B gz/site (35.7 against the harshest baseline); the closure control is 97–109 |
| P0.2 cannot hydrate cleanly, or the catalog cannot be fetched in parallel with the wasm | 67/67 checks with zero warnings in two browsers and two build modes; the catalog finishes before the wasm, through the preload, with no extra request |
| P0.9 cannot give correct incremental rebuilds under cargo-leptos | every edit scenario correct under `cargo leptos build` and `watch`; identical manifests in both builds |

D3 confirmed, D7 settled (B), D8 settled with amendments, D9 verified with a
narrower fallback than planned. Risks carried into later phases, none of which
blocks Phase 1:

1. **View-position call sites cost 46–102 B gz** (tachys' async hydration code
   around a non-`&str` leaf); the weighted B5 passes only because String/prop
   sites are ≈ 0, and the margin against the harshest baseline is 4 B — P6.
2. **`fixed_decimal` keeps panic paths reachable** (B12 on the numeric path) —
   an owner decision before P3 exits.
3. **Budgets raised**: the numeric share of B1 (≤ 10 KB gz) and
   `datetime-intl` (≤ 6 KB gz); B1's total of 30 KB gz is not yet measured as a
   whole (parts: floor 7.0 + numeric 10.2 + call-site library 7.5).
4. **Spec license** (upstream #1112): `spec/` cannot stay vendored in a public
   repository without Unicode's permission — decide before any public remote.
5. **Named time zones on the server need a tz database** (P4).
6. **A translation edit recompiles the app** in dev until P7's hot reload.

---

## Part A — bootstrap

| Task | Outcome |
|---|---|
| A1 | `git init` (branch `main`), `.gitignore`, `LICENSE` (MIT), `README.md`. No commit yet (owner). |
| A2 | Root `Cargo.toml` (resolver 3, `exclude = ["probes"]`, shared lints: `unsafe_code = "deny"`, clippy pedantic minus noise), profiles `release` (+ fat LTO) and `wasm-release`; `rust-toolchain.toml` → stable (resolved to 1.98.1) + `wasm32-unknown-unknown` + `wasm32-wasip1`; `cargo xtask` alias. |
| A3 | `xtask`: `spec-sync [--rev] [--check]` (blobless fetch into `target/xtask-cache/`, byte-for-byte tree compare), `cldr-sync` (sparse checkout, verifies the tag still names the pinned commit), `conformance-report [--init [--force]] [--ledger] [--report]`, `ci` (fmt, clippy `-D warnings`, test, conformance report — no network), `gen-workload` (pass-through), stubs `resource-sync` and `size` (exit 1 with a reason). `spec-sync --check` exits 0 (33 files match). |
| A4 | `third_party/cldr-json/`: `cldr-core/supplemental/{plurals,ordinals,numberingSystems,currencyData}.json`, and for the panel `en es de fr ar he ja hi ru pl cy` `cldr-numbers-full/main/<loc>/{numbers,currencies}.json` and `cldr-units-full/main/<loc>/units.json`, upstream `LICENSE` — 38 files, upstream-relative paths (the `cldr-json/` prefix stripped). `numberingSystems.json` and `currencyData.json` were added after P0.5 needed them. |
| A5 | `mf2-conformance`: loads 462 tests / 16 files, applies `defaultTestProperties`, key = (`file`, `hash`, `nth`). Hash encoding (now in 01 §4). `conformance/ledger.toml`: `current_phase = "P0"`, every applicable cell `xfail` with the §3 deadline; 136 syntax-error tests and 25 data-model-error tests take the `n/a` pattern. 26 tests incl. mutation tests (a deleted entry, a duplicated suite test, an early `until`, a skip-by-tag, misplaced `n/a` … all go red). |
| A6 | `.forgejo/workflows/ci.yml`: plain `git` fetch, `rustup toolchain install`, `cargo xtask ci`, upload of `conformance/REPORT.md` with `https://code.forgejo.org/forgejo/upload-artifact@v5` (the Forgejo-patched fork; the `code.forgejo.org/actions` mirror states its v4+ does not work on Forgejo). `runs-on` is a placeholder until the owner names the remote. |
| A7 | `bench/workload-gen` (own SplitMix64 PRNG, quantile tables, word lists and JSON writer, so no dependency bump changes output). Seed 1: every checked shape row within tolerance (table below). Committed `bench/corpora/workload-1600.json` and `bench/corpora/suite.json` (462 `{file, index, src}`). Built-in call-site templates `literal` and `closure` both build (SSR check + `wasm-release` hydrate + `--split`). |

Workload shape, seed 1 (06 §2 targets; ±1 pp / ±5 % checks all pass, also at 10×):

| Property | Target | Generated |
|---|---|---|
| simple / 1 / 2 / 3 / 4 variables | 79 / 14.7 / 5.0 / 1.1 / 0.2 % | 79.00 / 14.69 / 5.00 / 1.12 / 0.19 % |
| `.match` | 0.9 % | 0.88 % |
| text mean / median / p90 / max | 27 / 19 / 58 / 259 B | 27.03 / 19 / 58 / 259 B (43.2 KB) |
| id mean (full dotted id) | 23.5 | 23.53 |
| files; comment share (`en`) | 18; ≈ 60 % | 18; 60.00 % |
| call sites String / child / attr / reactive prop / String prop / deferred / if-else | 45 / 20 / 8 / 8 / 4 / 8 / 7 % | 45.00 / 20.00 / 8.01 / 8.01 / 3.98 / 8.01 / 6.99 % |
| sites with arguments | 21 % | 21.02 % (plain 288 / signal 52 / enclosing closure 51) |

Generator choices the plan now records (06 §2): locales `en`, `pl` (second real
locale, plural `one few many other`), `en-XA`, `ar-XB`; markup messages (0.5 %)
count as "no variable"; text length is the byte length of the whole MF2
source; the plural selects declare `.input {$count :integer}` (MF2 requires an
annotated selector); each `if`/`else` branch is one site; the reactive-prop
share is split 4 % `TextProp` / 4 % `Signal<String>`; `--number`/`--datetime`
convert existing variable messages (shape unchanged). Canaries (B6): id
`app.canary.zq7-canary-msg`, variable `zq7_canary_var`, text
`ZQ7-CANARY-TEXT-<TAG>` (`workload-gen canaries --grep`). The placeholder-free
subset used by the parser gate is "no `{` and not starting with `.`": 1,256
messages. Wasm of the whole generated app (hydrate, `wasm-release`, `wasm-opt
-Oz`): `literal` 565,097 B gz, `closure` 729,688 B gz.

---

## P0.1 — call-site cost (go/no-go)

**Question.** What does the concrete-type design (04 §2–3: one `Tr`/`TrArgs`
value per site, every rendering path implemented once in the library) cost per
call site, in 06 §2's seven shapes and mix, with and without arguments, after
`wasm-opt -Oz` — against the closure-per-site control? Threshold ≤ 40 B gz per
site, weighted by the mix; no-go above it with no credible fix.

**Built.** `mf2-probe` (evolved from the audit prototype; Leptos 0.8.20,
`forbid(unsafe_code)`, no fmt on the client path): `Tr(MsgId)` (`Copy`, 4 B,
`const fn`); `TrArgs` = `MsgId` + `ArgList` (4 inline `ArgValue`s, boxed
beyond; `ArgValue` = `Str(Oco)` | `Int` | `Float` | `Reactive(Signal<ArgValue>)`);
tachys 0.2 glue where every trait method forwards to one shared out-of-line
function; `From` into `TextProp`/`Signal<String>`/`Oco`/`String`; strategy B
registry (view state = a 4-byte slot index). The catalog is a stub filled at
run time from the page, so every id reaches an opaque lookup. A data-only `tr`
template covers the 7 shapes × `none`/`plain`/`signal`/`get` modes and the
`const` table form for deferred labels.

**Method.** Two workloads with equal knob ratios, M = 1,860 (N = 1,600, K = 60,
R = 3) and M = 3,720 (N = 3,200, K = 120); every app built with the 06 §3
pipeline. **Marginal per site** = (Δ@3,720 − Δ@1,860) / 1,860, where Δ = app −
baseline app of the same workload; **fixed** = Δ@1,860 − 1,860 × marginal
(belongs to B1). Weighted by the mix by construction (whole generated apps).
Per shape: 30 data-only variant templates render one shape with the
implementation under test and every other site with the `idlit` baseline.
Three baselines:

* `dummy` — the same literal at every site. LLVM and wasm-opt then merge code
  that differs only by message (identical `match` arms, deduplicated per-block
  functions): the `dummy` app is 11.4 B gz/site **smaller than `idlit`**
  although neither carries text, so `dummy` overstates every implementation by
  ≈ 11 B gz/site.
* `idlit` — each site a short literal holding its MsgId: distinct sites, almost
  no text. Isolates per-site code. **Adopted as the B5 baseline** (06 §3).
* `literal` — each site's source text (06 §3's old wording): subtracts the text
  that moves into the catalog, flattering the design by ≈ 7 B gz/site.

**Numbers** (after `wasm-opt -Oz`, gzip -9; whole apps in bytes: `tr`
603,295 / 1,033,128 gz, `idlit` 550,270 / 934,589, `dummy` 528,569 / 891,996,
`closure` 729,688 / 1,295,016 at M = 1,860 / 3,720):

| Baseline | `tr` marginal B gz/site (raw) | `tr` fixed, gz | closure control marginal B gz/site (raw) |
|---|---:|---:|---:|
| `dummy` | **35.7** (90) | 8.3 KB | 108.5 (492) |
| **`idlit`** | **24.5** (31) | 7.5 KB | 97.3 (432) |
| `literal` | 17.7 (13) | 5.3 KB | 90.5 (414) |

Per shape, `tr` vs closure, B gz/site against `idlit` (± 20 B noise for shapes
with < 150 sites): String path (45 %) **0.2** vs 9.6 (11.5 with arguments);
text child (20 %) **101.9** vs 407 (46.4 without arguments; ≈ 300 with);
attribute 85.6 vs 90.8 (48.3 without arguments); `if`/`else` 33.2 vs 34.1;
`TextProp`, `Signal<String>`, `String` prop and deferred rows ≈ 0 vs 5–50.
Explicitly weighted: 28.0 (`tr`) vs 100.4 (closure). `tr` is cheaper or equal
in every shape, and 4–25× smaller in raw bytes wherever the control creates a
closure. The `get` argument mode (user writes `move || tr!(…, x.get())`) is a
closure per site and costs what the control does; the `signal` mode (a
`Signal<ArgValue>` argument) costs one library effect and no per-site closure.

**Where the view-position cost comes from.** A `Tr` text child without
arguments is 76 B *smaller* in raw bytes than the `idlit` literal yet 46 B
larger in gz. Symbol pairing of named builds places the difference in tachys'
per-block `hydrate_async` state machines (instantiated because of the lazy
routes): their frames grow (3,680 vs 2,784 B offsets in the same block) and
compress worse; other code shrinks. It is not the update strategy (strategy C,
no per-node state, saves only ≈ 8 B gz per child site) and mostly not id
entropy (one id at every site still costs +37.5 KB gz; distinct ids add
8.3 B gz/site). **The audit's "14 B raw / ≈ 3 B gz per view site" reproduces
in raw bytes only**: at whole-file scale a view site costs 43–56 B gz before
`wasm-opt` and 46–48 B after. The weighted average passes because the String
path and props/rows (69 % of sites) cost almost nothing.

Also: the fixed cost of the `tr` library is 7.5 KB gz / 27.4 KB raw (≈ 7 KB raw
of its own functions, the rest generic instantiations such as
`Signal<ArgValue>` read paths reachable from the switch formatter even in apps
without reactive arguments — trimmable). **B6 canary grep clean** on the `tr`
wasm and JS at both scales (0 of 627 texts ≥ 16 B and 0 of 1,481 ids ≥ 16 chars
found). `wasm-opt -Oz` shrinks raw by 13–16 % but **grows gz by 4.6–5.0 %** on
every app. `tr_args` is generic over the array length (3 instances); the
`TrArgs` type stays concrete.

**Threshold** — ≤ 40 B gz/site weighted: **met** against every baseline
(24.5 against `idlit`, 35.7 against the harshest, `dummy`), about a third of
the closure control. Stretch 25 B gz: met against `idlit`. **Go.** Phase 6
carries the view-position cost (04 §1): measure with `--cfg
erase_components`, propose a tachys leaf hook reusing `&str`'s state and
async path, re-measure on tachys 0.3; steer users to signal-valued arguments;
trim the fixed cost.

---

## P0.2 — vertical slice (go/no-go)

**Question.** Does the whole path work in a browser: SSR with a per-request
catalog → `<link rel=preload>` → fetch in parallel with the wasm → validate →
install → hydrate → switch locale, with a `#[lazy]` route under `--split` and
streamed `Suspense`? Is the catalog reachable at render time under streaming
(D9)? Is `forbid(unsafe_code)` feasible in the client crate?

**Built.** A cargo-leptos + Axum 0.8.9 app on Leptos **0.8.20** (leptos_axum
0.8.10, leptos_router 0.8.15, leptos_meta 0.8.6, tachys 0.2.18, reactive_graph
0.2.14), two hand-built catalogs (`en` ltr, `ar` rtl, 32 simple messages), and
a library crate `tr` (`#![forbid(unsafe_code)]`, denies unwrap/expect/panic/
indexing): a 4-byte `Copy` `Tr` with tachys 0.2 impls delegating to `&str`
(`Render`, `RenderHtml`, `AttributeValue`, `no_attrs!`, `From` into
`TextProp`/`Signal<String>`/`String`/`Oco`); **strategy B** registry (slab slot
per node, freed on `Drop`); a boot gate reading `<link data-mf2>` and `<html
lang>`; `set_locale`; SSR context looked up at render time with a
default-locale fallback. All leptos_axum `_with_context` entry points are wired
(route-list generation, routes + server functions, file/error handler). A
Playwright harness (`tools/e2e/`, permanent) runs 67 checks.

**Numbers.**

| Build | Chromium 143 | Firefox 155 |
|---|---|---|
| debug, `--split` | 67/67 | 67/67 |
| release (`wasm-release` + `wasm-opt -Oz`), `--split` | 67/67 | 67/67 |
| release, throttled (150 ms RTT, 1.6 Mbps) | 67/67 | — |
| debug, render-time lookup only (`d9-no-capture`) | 63/67 (the 4 title assertions below) | — |

0 console warnings and 0 errors across load, hydration, lazy-route navigation,
switch to `ar` and back, four streamed pages and a direct `/lazy` load.

| Run | catalog start → end (ms) | main wasm start → end (ms) |
|---|---|---|
| release, Chromium | 12.5 → 20.4 | 13.0 → 25.1 |
| release, Firefox | 118 → 340 | 119 → 344 |
| release, Chromium throttled | 168.8 → 329.0 | 168.9 → 3,199.5 |

Exactly one catalog request (`initiatorType: "link"`): the gate's `fetch()`
reuses the preload, so the gate adds no round trip (B11). After switching and
back, the client text of home and lazy pages is byte-identical to the SSR text
in that locale. Registry slots 20 → 15 → 20 across home → `/lazy` → home. A
catalog with a different manifest hash is rejected. No message text, id or
autonym is in the release wasm/JS. Reference size of the whole demo app (not
an i18n cost): main module 559,101 B after `wasm-opt -Oz` (220,318 B gz), lazy
chunk 6,975 B.

**D9.** Render-time lookup works for every `Tr` that tachys renders (text and
attribute) in **all four** `SsrMode`s, including `Suspend` chunks streamed
later. It fails for a derived value that a third party evaluates outside the
request owner: leptos_meta reads `<Title text>` after rendering, so under
`PartiallyBlocked` and `Async` the SSR `<title>` came out in the default locale
(1 miss per request). Fix, implemented and verified: under `ssr`, `From<Tr>`
for `TextProp` and `Signal<String>` captures the request's `Arc<Catalog>` at
conversion time. `Tr` stays a 4-byte `Copy` value; the larger server-side `Tr`
D9 anticipated is not needed. The `additional_context` provider must tolerate
missing request `Parts` (the file handler calls it in a bare owner).

**Threshold** — zero hydration warnings: **met**; catalog request overlaps the
wasm request: **met**; lazy route sees the i18n state: **met**; SSR context
lookup under streaming: **met** (with the narrowed D9 fallback);
`forbid(unsafe_code)` in the client crate: **feasible** (every
`#[wasm_bindgen]` export lives in the application). Minimum Leptos version
verified: **0.8.20** only.

Caveats: 32 simple messages, no patterns/selectors/markup; catalogs not
precompressed; B12 not assessed (the app links `console_error_panic_hook`);
localhost timings plus one CDP-throttled run. `set_locale` found the other
locale's hashed URL through `GET /i18n/<tag>` → `307` (one extra round trip at
switch time only) — 04 §6 did not specify this; owner decision.

## P0.10 — hydration tolerance

| Case | Console | Outcome |
|---|---|---|
| text differs (static text, reactive text, attribute) | nothing, debug or release | server text kept; a reactive node shows client text after its next update; static text never changes |
| structure differs, stock tachys (`&str` meets `<b>`) | debug: hydration error with source location, then `Unrecoverable hydration error` panic; release: `entered unreachable code` | **wasm traps, hydration aborts, the page is inert** |
| structure differs, probe `Tr` glue | one `mf2:` error | hydration continues; that `Tr` binds to a detached node |

Identical in Chromium and Firefox. **Threshold** — text: **confirmed** (no
warning; server text stays until the next update); structure: **documented**.
Consequences: `datetime-intl` text differences are harmless; a `TrRich` whose
structure differs between server and client would kill the app with stock
tachys, so `leptos-mf2` must walk the hydration cursor itself and never panic
(tachys' `failed_to_cast_text_node` is `pub(crate)`), and the hydration gate
(same catalog, same manifest hash) is what keeps structures equal. Server and
client must be the same build mode (debug SSR emits extra markers).

---

## P0.3 — runtime floor

**Question.** How big is a `no_std`, fmt-free, panic-free evaluator skeleton
over P0.7's encoding — reader + pattern + select + plural + parts + bidi — and
does it work on every workload message?

**Built.** ≈ 1,190 lines, `#![no_std]` + `alloc`, `forbid(unsafe_code)`,
denies unwrap/expect/indexing/panic/unreachable: `Catalog::new` validating
once (magic, version, `manifest_hash`, section table ordered/in bounds/unknown
kinds skipped/STRINGS last, INDEX monotonicity, LOCALE walk, FUNCS); O(1)
`get` over the byte-plane INDEX; `Formatter::simple`; pattern formatting into
`&mut dyn Sink` with positional `Arg::{Str, Int, Unset}`; lazy in-place
`.input`/`.local`; `:string`/`:integer`/`:number` on integers with neutral
digits and `select=plural|ordinal|exact`; allocation-free spec selection
(filter + best-by-compare, first best in source order) with P0.4's evaluator;
parts output (`Text`, `BidiIsolation`, `Expression`, `Fallback`, `Markup`)
sharing one walker with string output behind `dyn Out`; fallbacks from NAMES;
Default Bidi Strategy from the header's `dir`, and `None`. Not included (Phase
3): `u:` options, decimals and the `:number` digit options (P0.5), NFC through
`Host`, the custom-function registry, `write_named`.

| Harness (Δ after `wasm-opt -Oz`) | Δ raw | **Δ gz** |
|---|---:|---:|
| `no_std`, eager UTF-8 | 14,768 | **7,408** |
| `no_std`, UTF-8 per access (P0.8's choice) | 14,045 | **7,022** |
| std, fair base (allocation kept alive), eager | 14,591 | 7,252 |
| std, fair base, per access | 13,775 | 6,825 |

Largest items: `Formatter::run` 2.4 KB, `Catalog::new` 1.9 KB, `Eval::expr`
1.4 KB, integer parsing 0.75 KB, fallback escaping 0.7 KB, the plural path
0.6 KB.

Correctness: **40,544 / 40,544** checks — every message × 4 locales × up to 11
argument sets (26,968 formats) byte-identical, bidi controls included, to an
independent reference formatter that walks the parsed data model; parts
concatenate to the string; spot checks (`pl` one/few/many, LRI…PDI inside RTL
text, markup parts, bogus ids → *missing-message*); a 14-message mini corpus
for constructs the workload lacks. Mutation run: 3,000 bit-flipped catalogs,
1,708 rejected by `Catalog::new`, 1,292 loaded and fully formatted (4.1 M
formats), **0 panics**.

B12 failures found and fixed during the probe — now client-path coding rules
(05 §8): infallible `Vec`/`String` growth keeps `capacity_overflow` /
`handle_alloc_error` alive (use `try_reserve*`, or let the caller's `Sink` own
growth); `str::find`/`split*` keep `slice_index_fail → panic_fmt` alive (use
hand-written scans; the NUL scan is SWAR); `copy_from_slice` without a proven
length pulls `len_mismatch_fail`; undefined wasm imports need
`#[link(wasm_import_module = …)]`.

The probe resolves function names per call (a few short compares against
FUNCS, no load-time allocation) instead of 02 §2.2's "once per catalog load";
Phase 3 keeps load-time resolution only if it measures a real cost.

**Threshold** — ≤ 12 KB gz: **met** (7.0 KB gz, ≈ 60 %); B12: **met** (0 fmt,
0 panic symbols, panic import absent, both UTF-8 variants).

---

## P0.7 — catalog encoding

**Question.** Encode the workload per 02 §2 — size per locale raw / gzip -9 /
brotli against B7; settle 02 §6 (single vs split pools, fixed vs varint INDEX,
stripping); and resolve the conflict P0.2 found: a pool of "varint length +
UTF-8" strings cannot pass F4's single `str::from_utf8`.

**Built.** A throwaway parser for the workload constructs, the manifest (05 §3;
`manifest_hash` FNV-1a 64), a writer with every layout variant, and a
model-rebuilding decoder. **All 600 catalogs (150 layouts × 4 locales) decode
back to the parsed data model: lossless.** Compression: GNU `gzip -9 -n`,
brotli crate quality 11 / window 22.

Recommended layout, production (stripped):

| locale | MF2 source bytes | raw (limit 1.25 × src + 8 × 1,600) | **gzip -9** | brotli | B7 raw | B7 gz ≤ 25,600 B |
|---|---:|---:|---:|---:|---|---|
| en | 43,250 | 50,867 (66,862) | **20,536** | 17,929 | met | met |
| pl | 58,518 | 67,302 (85,947) | **26,647** | 24,068 | met | **not met** (+1,047 B) |
| en-XA | 95,078 | 100,528 (131,647) | **23,620** | 21,477 | met | met |
| ar-XB | 54,491 | 60,790 (80,913) | **20,518** | 18,491 | met | met |

The plan as written (length-prefixed strings, row-major index, first-use pool)
gives 23,083 / 29,609 / 28,433 / 24,833 B gz — failing pl and en-XA; the
recommended layout saves 2.5–4.8 KB gz per locale. pl misses 25 KB gz under
every layout measured (best 25,905 B, not O(1)): its pool alone is 18.6 KB gz,
because the synthetic Polish has 1.35× the `en` bytes. The structure (all but
STRINGS) is 6.9–7.3 KB gz; the pool 12.8–18.6 KB gz. Unstripped (with IDS)
catalogs are 29–36 KB gz.

String lengths (the F4 conflict), Δ gz against NUL termination: length prefix
encoded as one UTF-8 scalar +1.5…2.7 KB; offset + length in the structure
+2.2…3.2 KB; index + end-offset table +2.3…2.8 KB. **Choice: NUL-terminated
strings** — the pool stays valid UTF-8, a constant terminator compresses to
nearly nothing, and MF2 syntax cannot contain U+0000 (the ABNF excludes
`%x00`; the one suite test with it is a syntax error), so the writer rejects
NUL only in data models built in code (a documented F1 limit).

02 §6, settled (Δ gz for en / pl / en-XA / ar-XB against the recommendation):

| Open point | Result | Decision |
|---|---|---|
| single vs split pools | first-use order +0.6…1.5 KB; identifiers grouped first +0.4…1.4 KB; sorted only +0.03…0.2 KB; a separate section compresses identically | **one STRINGS section**, writer groups identifiers first and sorts each group bytewise |
| fixed vs varint INDEX | row-major u32 +0.5…1.4 KB; varint delta +0.2…0.9 KB (not O(1)); blocked delta +0.7…1.3 KB | **fixed 4-byte entries stored as 4 byte planes** (still one O(1) read) |
| stripping | COLD + IDS +8.6…9.8 KB gz (COLD empty here) | strip COLD and IDS for clients |
| deduplication | off: +2.5…4.3 KB gz | keep |

02 §2 also lacked a section for the function table §2.2 refers to: **FUNCS**
(sorted `ns:name` StrRefs). LOCALE container: `varint n · (varint key · varint
len · payload)*`, key 1 = `plural.cardinal`, 2 = `plural.ordinal`, unknown keys
skipped by length. The section layout is now in 02 §2. The MESSAGES byte
grammar as prototyped (read by P0.3's runtime; Phase 2 freezes its own):

```text
MESSAGES   per non-simple message, in MsgId order:
           varint names_ref (0 = none, else NAMES offset + 1) · varint decl_count · Decl* · Body
Decl     := u8 head (bit 7 local; bits 3–4 operand kind; bit 5 has function; bit 6 has cold)
            [input: varint slot] operand? FunctionRef? ColdRef?
Pattern  := varint n · Part*
Part     := u8 head (bits 0–2: text | expr | open | standalone | close; expr bits as Decl;
            bit 7 markup options) + StrRef / operand / FunctionRef / name + options / ColdRef
Select   := varint nsel · VarRef* · varint nvar · (Key{nsel} · varint pattern_len · Pattern)*
Key      := u8 0 = `*` | 1 StrRef | 2 StrRef(NFC) ColdRef(original spelling)
VarRef   := varint (index << 1 | is_local)       ; slot for externals, order for locals
Value    := varint 0 · StrRef (literal) | varint (VarRef << 1 | 1)
FunctionRef := varint fn_idx · varint n · (StrRef name · Value)*
StrRef   := varint offset into STRINGS (absolute)
NAMES    := deduplicated entries: varint n_ext · StrRef* (slot order) · varint n_local · StrRef*
IDS      := front-coded: (varint shared_prefix · varint suffix_len · suffix)*
```

Section sizes for `en`: header 90 B, INDEX 6,400, MESSAGES 4,568, NAMES 701,
FUNCS 2, LOCALE 8, STRINGS 39,098. Suffix sharing (a NUL-terminated string that
is a suffix of another pointing into it) is possible but was not measured. The
size gate should report structure gz and pool gz separately; the structure is
what the format controls.

**Threshold** — B7 raw: **met** for all four locales; B7 gz ≤ 25 KB: **met**
for en (the reference, 20.5 KB), en-XA and ar-XB, **not met** for pl (26.0 KB;
met under brotli). B7 is restated in 06 §3 to scale with text volume: ≤ 25 KB
gz for the reference `en`, and gz ≤ 0.5 × MF2 source bytes + 1 KB for any
locale (all four meet it: ratios 0.475 / 0.455 / 0.248 / 0.377).

Caveats: the workload has only `:integer` (in the 14 selects), no literals,
options or attributes, markup in 8 messages — those encodings round-trip but
their size share at scale is untested; pl/en-XA/ar-XB are synthetic.

---

## P0.8 — load + lookup speed

**Question.** `Catalog::new` time for 1,600 messages against B9 (≤ 1 ms at 4×
CPU throttle); simple and 1-argument lookup against B10 (native); which UTF-8
strategy.

**Numbers** (P0.3 runtime over P0.7 catalogs; **taken under load**, load
average 2–5; repeated criterion runs varied up to 2×, rankings never changed):

| | eager: one `from_utf8` over the pool | **per access** (chosen) |
|---|---|---|
| native `Catalog::new` en / pl / en-XA / ar-XB | 12.0 / 143.5 / 184.4 / 100.1 µs | **5.7 / 6.7 / 6.9 / 6.1 µs** |
| Chromium 4× `Catalog::new` | 0.110 / 0.783 / **1.103** / 0.263 ms | **0.079 / 0.078 / 0.075 / 0.073 ms** |
| first locale switch (copy + load), 4× | 2.280 ms | **0.125 ms** |
| allocations / copies at load | 1 alloc (≈ 12 KB) + pool memmove | **0 / 0** |
| native simple lookup | 22.8 ns | 20.7 ns, 0 allocs |
| native 1-arg pattern, reused `String` | 100.2 ns | 93.5 ns, 0 allocs |
| native 1-arg pattern, new `String::with_capacity(128)` | 117.4 ns | 106.3 ns, 1.02 allocs |
| native select (plural), reused `String` | 236 ns | 317 ns, 0 allocs |
| runtime size (P0.3, `no_std` Δ gz) | 7,408 B | 7,022 B |
| corrupt pool byte | whole catalog rejected | that string falls back |

Whole-pool validation is cheap on ASCII but 60–300 µs natively (up to 1.1 ms
at 4×) on non-ASCII text, which defeats `from_utf8`'s ASCII fast path;
per-access validation adds 0–45 ns per string read at 4× (≈ 0.1 ms for the
1,860 sites of a hydration). **Decision: validate UTF-8 per string on access**;
the fetched buffer is the catalog (no split copy, no `Arc<str>`). F2/F4 are
updated in 02 §1.

Cold first install in a fresh page: 0.160 ms copy + **1.120 ms**
`Catalog::new` at 4× (median of 9, range 0.25–2.3 ms), although the next
install takes 0.125 ms — ≈ 1 ms is one-off V8 first-execution cost of the
code, not load work. Browser lookups at 4× (informative): simple 143 ns, 1-arg
1.30 µs, select 5.38 µs. A `String::new()` target costs ≈ 3 allocations per
1-arg format through growth; the `to_string` path must pre-size (128 B covers
98 % of outputs).

**Threshold** — B10: **met** (20.7 ns / 0 allocs; 93.5 ns / 0 allocs, 1 into
an owned `String`); B9 as the load work (validate + index): **met** (≤ 0.08 ms
at 4×); B9 as a cold first install: **not met** (1.12 ms, attributed to code
warm-up). B9 is restated in 06 §3 as the warm load, with the cold first install
tracked in the app's boot budget (P6).

---

## P0.4 — plural (D3)

**Question.** Can our own UTS #35 rule parser (build side), a compact encoding
of the `plural.cardinal`/`plural.ordinal` LOCALE entries and a `no_std`,
fmt-free, panic-free evaluator pass every CLDR `@integer`/`@decimal` sample for
every locale, within ≤ 1.5 KB gz of code and ≤ 300 B of data per locale?

**Numbers.** CLDR 48.2.1: 224 cardinal locales (12,396 expanded samples) and
108 ordinal locales (2,645 samples), five assertions each (operand extraction
vs a reference, encoded-path category, an independent rational-arithmetic
reference evaluator, exclusivity through both paths): **15,041 / 15,041, 0
failures**. ICU4X 2.3, given data generated from the same CLDR files, agrees on
every sample plus 904,368 extra numbers. The same run on `wasm32-wasip1` under
wasmtime gives the same result. Five deliberate evaluator bugs are all caught.
The audit's `hand` evaluator was wrong (it treated the `c`/`e` operand as 0:
54 samples fail in 9 locales).

| Code (Δ after `wasm-opt -Oz`) | raw | gz |
|---|---:|---:|
| our evaluator, `no_std` | 667 | **429** |
| our evaluator, in a std module | 764 | 462 |
| `icu_plurals` 2.3 + blob provider (fair std base) | 29,838 | 14,451 |
| `icu_plurals` 2.3, compiled data (fair std base) | 39,779 | 17,865 |

The audit's absolute ICU4X sizes reproduce (41.6 / 19.6 and 51.6 / 23.1 KB gz);
its base was degenerate (5.1 KB gz too small).

Data per locale, cardinal + ordinal: **min 0 / median 3.5 / max 84 B** (`kw`);
panel (raw bytes): en 20, es 15, de 5, fr 19, ar 17, he 14, ja 0, hi 18, ru 31,
pl 32, cy 35. Distinct rule sets: **40 cardinal, 25 ordinal**; all distinct
entries together are 937 B. 116 of 224 locales have no ordinal rules
(subtag truncation, then root = empty, resolves every CLDR 48 locale). ICU4X
2.3's compiled data lacks 75 cardinal and 5 ordinal CLDR locales (it answers
`other` for them) — another reason to build from CLDR JSON (05 §7).

The byte encoding (v1, ready for Phase 2 to freeze) is now specified in
[02](02-catalog-format.md) §4.1. Operands contract for Phase 3: the formatter
supplies `{ i, f, t: u64, v, w, e: u32 }`; values ≥ 10¹⁸ are stored as
10¹⁸ + (value mod 10¹⁸), exact for every CLDR 48 modulus and literal (the
encoder asserts it). MF2's `:number` has no compact notation, so `e` is always
0 from MF2 core; the evaluator supports it for full CLDR conformance.
Client-path coding note: on wasm32 `u64::checked_pow`/`checked_mul` pull in
`__multi3` (128-bit multiply).

**Threshold** — 100 % of samples: **met**; ≤ 1.5 KB gz code: **met** (0.43);
≤ 300 B data/locale: **met** (≤ 84); B12: **met** (0 fmt, 0 panic symbols,
panic import absent). **D3 confirmed.**

---

## P0.5 — numbers

**Question.** The complete `:number`/`:integer`/`:offset` semantics over
`fixed_decimal`, `no_std`, fmt-free, panic-free — cost (plan: ≤ 6 KB gz of
B1); the `fn-number` layer with catalog-borne data (B2), `:currency`/`:unit`
(B3), per-locale data (B8); comparisons.

**Correctness.** Core, neutral output: suite **70/70** (`number` 41, `integer`
13, `offset` 16 — none needs locale data). ECMA-402 differential, 100,000
random value × option sets vs `Intl.NumberFormat('en', {useGrouping:false})`:
**95,675 identical, 0 different**; the 4,325 sets `Intl` rejects are exactly
those reported as *Bad Option*. With `fn-number` and en-US data: suite **95/95**
(adds `percent` 13/13 and `currency` 12/12). Against `icu_decimal` (236 values ×
4 grouping strategies × 11 locales + 2 native-digit variants): 12,228 identical,
44 different — all `useGrouping=always` in es/pl, where we match ECMA-402 and
ICU4X documents `Always` as behaving like `Auto` (so `icu_decimal` cannot
implement MF2's `always`). Against node `Intl` (CLDR 46): `:percent` 198/198;
`:currency`/`:unit` differences are all the harness's plural stub or CLDR 46→48
data changes.

| Build (Δ, gz) | Δ raw | **Δ gz** |
|---|---:|---:|
| core semantics (std app base) | +21,673 | **+10,233** |
| + `Float(f64)` operands (`ryu`) | +26,233 | +13,065 |
| core, `no_std` absolute (incl. bump allocator) | 17,117 | 8,450 |
| `fn-number` (symbols, grouping, digits, `:percent`) over core | +3,514 | **+1,728** |
| + `:currency` + `:unit` over `fn-number` | +6,522 | **+2,935** |
| `Intl.NumberFormat` glue over core (+614 B gz JS) | +3,981 | +1,597 |
| `icu_decimal` + blob over core | +35,759 | +15,885 |

Per-locale data (raw / standalone gz): `number.symbols` 31–117 / 63–124 B;
currencies *used* (4 codes) 145–585 B raw vs *all* 4.7–9.6 KB gz; units *used*
(5 × 3 widths) 195–363 B gz vs *all* 4.4–9.2 KB gz. B8 (plural + symbols):
≤ 0.2 KB gz per locale.

**B12.** Our crates: 0 `core::fmt` symbols, no panic strings. But the panic
import **survives**: six panic entry points, all traced to `fixed_decimal` 0.7.2
and its `smallvec` (inlined bounds checks, `SmallVec` growth `unwrap`/`expect`,
slicing in `Decimal::try_from_utf8`); in a std build they drag std's panic
formatting in (+3.6 KB raw of fmt). Owner decision recorded as an open question.

**Threshold** — B1 numeric share ≤ 6 KB gz: **not met** (10.2 KB gz; 8.5
`no_std`); B2: **met** (1.7 vs ≤ 15); B3: **met** (2.9 vs ≤ 5); B8: **met**
(≤ 0.2 vs ≤ 2); B12: **met for our code, not met for `fixed_decimal`** in std
builds. Ways to close B1, not taken: `Float(f64)` → text via the `Host`
(JavaScript `String(x)`, −2.8 KB gz), build-time interning of option
names/values (≈ −0.5 KB gz), a panic-free digit buffer (needs a D1-style
baseline, gate and fallback). Budgets adjusted in 06 §3.

Caveats: plural categories in the harness were stubs (P0.4's evaluator slots
in); not implemented: compound units beyond CLDR ids, unit `usage` conversion
(*Unsupported Operation*), cash digits; `:offset` passes an inherited `select`
on without error.

---

## P0.6 — dates

**Question.** What does the MF2 date family cost with each D4 backend —
ICU4X `icu_datetime` with a per-locale blob, or `Intl.DateTimeFormat` glue — and
how big is the restricted ICU4X blob?

| Variant (Δ vs base 7,063 B gz) | Δ raw | **Δ gz** | JS Δ gz |
|---|---:|---:|---:|
| MF2 date semantics only (literal parsing, options, zones; `no_std`) | +6,175 | **+3,364** | 0 |
| semantics + `Intl.DateTimeFormat` via js-sys | +11,233 | **+5,438** | +628 |
| semantics + one inline-JS import | +10,416 | +5,077 | +240 |
| ICU4X blob, Gregorian, all options incl. `timeZoneStyle` | +221,056 | **+93,025** | +17 |
| ICU4X blob, Gregorian, no time-zone styles | +146,361 | **+64,416** | +29 |
| ICU4X blob, any calendar | +256,555 | +105,254 | +13 |
| ICU4X compiled data (all locales) | +3.4 MB | +1,071,401 | — |

Per-locale ICU4X blob (markers the binary requests; raw / gz): with zone names
16.8–23.0 KB gz (en/ar/ja/ru); without 2.2–2.6 KB gz; a corpus using only the
default options 1.6–2.0 KB gz. Zone names are ≈ 85 % of the with-zones blob,
although the ICU4X backend can only display offsets (no tz rules).

Checks: suite `functions/{date,time,datetime}.json` error expectations **20/20**
(the suite asserts no successful date output). Blob ≡ compiled data on 93/93
option sets for each of four locales. ICU4X vs `Intl` on zone-free samples
28/28 identical **only after mapping U+202F → U+0020** (V8 emits a plain space
where CLDR has a narrow no-break space) — the P0.10 text tolerance is required
for `datetime-intl`, not optional. B12: date semantics 0 fmt / 0 panic symbols;
`datetime-icu` brings 72 `core::fmt` symbols (7.9 KB) into any app enabling it.

Server path: ICU4X 2.3 has no time-zone transition rules, so named zones
(including the visitor's zone from the cookie, 03 §6) need a tz database beside
it (owner question). For `datetime-icu` byte identity the server formats from
the same blobs as the catalog's `icu.blob`. ICU4X 2.3.0 bug met on the way: the
iterable variant of `icu_time_data`'s time-zone-periods macro does not compile.

**Threshold** — B4 `datetime-intl` ≤ 3 KB gz: **not met** (5.4 KB gz + 0.6 KB
gz JS; 3.4 of it is semantics every backend needs); B4 `datetime-icu` ≈ 100 KB
gz: **confirmed** (93 KB gz Gregorian; 64 without `timeZoneStyle`); B12 for the
date semantics: **met**. Budgets adjusted in 06 §3; `mf2-build` links the
no-zone field set when the corpus never uses `timeZoneStyle` (03 §5.2).

---

## P0.7 — catalog encoding

*Pending.*

---

## P0.8 — load + lookup speed

*Pending.*

---

## P0.9 — build orchestration (go/no-go, D8)

**Question.** Does 05 §4's design — an i18n crate whose `build.rs` writes a
manifest and catalogs to `OUT_DIR`, plus a generated module whose exported
`tr!` `macro_rules!` forwards to a proc-macro with the manifest's absolute path
baked in — work under cargo-leptos's dual build, with a second crate and
rust-analyzer; does it rebuild correctly after every kind of locale edit; is
the macro overhead ≤ 2 s per 2,000 sites?

**Built.** A workspace laid out as the target design with stand-ins: a catalog
crate (manifest `manifest.mf2m`, FNV-1a `manifest_hash` per 02 §3), a build
crate with a loader for the working grammar of 05 §2 and a small MF2 scanner
(manifest from `en`, translations flattened and checked against it,
content-hashed catalogs, generated module written only when changed), the
proc-macro (manifest read once per rustc process, id check with did-you-mean,
argument set = variables, positional expansion `$crate::__mf2::tr(MsgId(n))` /
`tr_args(…)`), a facade, the i18n crate over workload-gen's 4 locales × 1,600
messages, a second crate calling `tr!` (in a `static` table too), and the
generated cargo-leptos app with **2,000 `tr!` sites** plus two timing controls
(a trivial proc-macro; hand-expanded, no macro). Manifest 61,361 B; generated
module 2,280 B; `build.rs` 60–420 ms.

**Rebuild correctness** (full logs per scenario, `cargo leptos build` and plain
cargo):

| Scenario | Observed | Met |
|---|---|---|
| no change | nothing compiles, no build script runs | ✔ |
| `touch` a locale file | i18n + dependents recompile in both builds (8–23 s debug); server and wasm **byte-identical** | ✔ (cost noted) |
| edit one `en` text | manifest unchanged, `en` catalog renamed, new text served; **wasm byte-identical** | ✔ |
| add `{$extra}` to a used message | spanned error at the second crate's call site, then at both app call sites (`needs argument extra …`) | ✔ |
| revert | original hashes; server binary bit-identical | ✔ |
| unknown id at a new call site | spanned `unknown message id` | ✔ |
| add that message | builds; later MsgIds shift and still render their own text; browser: wasm hash = server hash | ✔ |
| edit `pl` only | `pl` catalog renamed, manifest and wasm unchanged | ✔ |

cargo-leptos builds server and wasm in separate target dirs (`--split` adds a
third), so there are two or three OUT_DIRs and baked paths — all held a
**byte-identical manifest** at every step. In Chromium, a `--split` build logs
`mf2: manifest ok <hash>` on `/` and on a lazy route. The second crate saw every
change and failed first where it should. **`cargo leptos watch` did not rebuild
within 151 s after a locale edit** until `watch-additional-files =
["<i18n crate>/locales"]` was set (then ≈ 10 s, edit served).
**rust-analyzer** (`rust-analyzer diagnostics` over the 3 × 2,000-site
workspace, 414 s, ≈ 5 GB peak): 0 errors, no unresolved macro; with a misspelt
id and argument seeded, it reports exactly the macro's own errors at the right
ranges — it runs the proc-macro against the manifest.

**Macro overhead** (medians, interleaved; **taken under load**, load average
4–5):

| Measurement | `tr!` | trivial macro | no macro | Δ |
|---|---:|---:|---:|---:|
| `cargo check`, native, 7 runs | 5.46 s | 5.12 s | 5.14 s | **+0.32 s** |
| `cargo check`, native, final macro, 5 runs | 5.59 s | 5.38 s | 5.37 s | +0.22 s |
| `cargo check`, wasm32, 5 runs | 4.47 s | 3.82 s | 3.56 s | +0.65 / +0.91 s |
| debug `cargo build`, 5 runs | 43.2 s | 43.0 s | 52.6 s | within noise |

The proc-macro's own time is 0.12–0.45 s per 2,000 expansions.

**Fragility found, with fixes.** (1) A **moved `target/`** (CI cache restored
elsewhere) left the i18n crate fresh and the baked path dead — 2,003 errors;
fixed by a relocation fallback in the macro (the same `build/…/out/manifest.mf2m`
under rustc's `-L dependency=` directories, verified against the baked hash):
0 errors. (2) **Inline mode** (manifest bytes in the wrapper) survives any
relocation; costs +0.5 s per 2,000 sites, growing with manifest size × sites —
opt-in for remote-execution builds, not the default; a first version that
re-parsed the literal with syn cost +4.7 s. (3) Several `compile_error!`s in one
expansion misparse unless wrapped in a block. (4) Any locale change — even a
translation-only edit or a touch — recompiles the i18n crate and every
dependent in both builds (cargo has no early cut-off); outputs are
deterministic. (5) Plain cargo and cargo-leptos do not share artifacts (a
switch cost a 4.6-minute rebuild). Not tested: sccache, `cargo package`, a live
editor session (reasoning in the RESULT: correctness holds by construction; an
absolute path costs sccache hits across checkouts).

**Threshold** — edit → rebuild correctness: **met** (plain cargo, `cargo leptos
build`, `cargo leptos watch` with the watch path); ≤ 2 s macro overhead per
2,000 sites: **met** (+0.2–0.3 s native, ≤ 0.9 s wasm). **D8 settled** as
designed, with the amendments now in 05 §4. The C3 no-go condition does not
apply.

---

## P0.11 — node update strategy (D7)

**Question.** On a locale switch, (A) one `RenderEffect` per node tracking a
global trigger, or (B) a library-owned registry — a slab slot per node, freed
in O(1) on drop, walked on switch? 2,000 live nodes plus a list churning
100,000 nodes: heap growth, switch latency, bytes per node.

**Built.** A native model on the real `reactive_graph` 0.2.14 with a counting
allocator (fake DOM), and a browser harness on **P0.1's real library** in CSR
mode, `wasm-release`, built as A and as B; 2,000 `<span>{tr(i)}</span>` plus
`<li>` rows churned 50 at a time under per-round owners cleaned per batch (as a
keyed list does); Chromium via the `tools/e2e` Playwright install, 4× through
CDP. **Taken under load** (load average 1.5–3.5); heap and allocation counts
are load-independent.

| | A (effect per node) | B (registry) |
|---|---|---|
| live heap per node, wasm32 / native | 427 B / 797 B | **44.8 B / 37 B** |
| allocations per node, wasm32 | 10.02 | **0.01** |
| heap growth over 100k churned nodes, wasm32 / native | **+72 B / +151 B per node, linear**, released only by the next switch | **+64 KiB once (slab growth), then flat** / +1,877 B total |
| switch script, 2,000 live nodes, 1× / 4× (median of 21) | 3.9–5.7 / 12.2–17.5 ms | **2.3–3.5 / 6.8–6.9 ms** |
| first switch after churn, 4× | 31.7–43.5 ms | 6.9–7.7 ms |
| forced style/layout after the switch, 4× (same DOM work) | 55–80 ms | 57–59 ms |
| native switch, 2,000 live | 1.31 ms | 22.5 µs |
| harness code, gz | 30,042 B | 28,128 B (−1.9 KB) |

A leaks because a dropped `RenderEffect` stays in the trigger's subscriber set
until the trigger next fires — and locale switches are rare, so under a
churning list (chat log, virtual list) the leak is effectively unbounded. A
first browser timing method ("switch → next macrotask") made B look 6× slower
because B happened to meet a rendering step; it was diagnosed and replaced by
separate script and forced-layout timings (both strategies pay the same
layout). No console errors in any run.

**Threshold** — B flat heap under churn: **met**; switch ≤ 2 × 16.7 ms: **met
for the update mechanism** (≤ 7 ms at 4×); the page's total frame at 4× (≈ 65
ms) is dominated by re-laying out 2,000 changed texts, which any strategy — a
full remount included — pays. **D7 settled: B.** Design points for
`leptos-mf2`: the slot index is the whole view state (4 bytes, one-call drop
glue); `set_locale` walks the slab synchronously, then notifies the
conversions; a node with signal-valued arguments owns one effect that tracks
only its arguments; conversions (`TextProp`, `Signal<String>`, `to_string()`
under an observer) still subscribe to the one trigger — their behaviour inside
churning rows is a Phase 6 follow-up.

---

## P0.12 — parser baseline (the D1 gate harness)

Not a probe: `bench/parser-gate/` is a permanent workspace member. Both
parsers (ox now; `mf2-syntax` in Phase 1, one adapter) run in one process with
samples taken in rotation over all 12 rows, reversing order each round;
thread-local counting allocator; median of ≥ 31 samples; `--gate` applies 05
§1's rules (with ox alone it prints *baseline only*). The run aborts if
`suite.json` no longer matches the vendored suite. Command:

```sh
CARGO_BUILD_JOBS=3 cargo run --release -p parser-gate -- --gate --json bench/parser-gate/baseline.json
```

The baseline table now in [05](05-tooling.md) §1 was measured at a 1-minute
load average of 2.9 → 2.7 (other agents building); at loads of 5–12 medians
were 1.5–3× higher. Allocation figures are exact and load-independent — they
match the audit **byte for byte** on the suite rows, where the input is
identical. Times are ≈ 30 % below the audit's; the audit's own probe, re-run in
the same window, agrees with the harness, so the gap is the machine, not the
method. `bench/parser-gate/BASELINE.md` holds per-pass limits, best samples
and IQRs.

The fresh-state caveat is large: with a new `SourceStore` per message most of
ox's allocations are per-call setup (`parse_source` builds a new
`ParseWorkspace` each call). With reused state (ox's session API with a shared
workspace — its leanest reuse), the CST rows fall to ≈ 1 allocation/message.
For "+ model", ox can only share the store (its model API needs an owned
parse result), which saves one allocation but is 6–11 % slower.

ox correctness: **460/462** exact on syntax + data-model errors (both misses
report `missing-fallback-variant` alongside the expected `variant-key-mismatch`,
`data-model-errors.json` #0 and #1); no errors on the 1,600 workload messages.
A test pins the 460 figure.

**Threshold** — harness + corpora committed, baseline recorded: **met**.
