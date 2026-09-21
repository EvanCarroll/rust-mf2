# P0.1 — call-site cost (go/no-go) — RESULT

This probe answers P0.1 of `plans/06-size-and-perf.md` §5 and
`plans/07-phase-0-work-order.md` (budget B5, master plan §2 row 6). The code is
throwaway.

## Question

The concrete-type design of `plans/04-leptos-integration.md` §2–3 is one
`Tr` / `TrArgs` value per call site, with rendering, attribute, `From` and
`to_string` implemented once in the library. What does it cost per call site
in the client wasm? The sites follow the seven shapes and the mix of 06 §2,
with and without arguments, and the cost is measured after `wasm-opt -Oz` and
gzip -9, as a delta against the same app with literals. It is compared with
the closure-per-site control. **Threshold: ≤ 40 B gz per site, weighted by
06 §2's mix; no-go above that with no credible fix.**

## Verdict: **met → go**

| Baseline for the B5 delta | tr, marginal B gz/site (weighted by 06 §2's mix, fixed cost excluded) | tr, whole-app average at M = 1,860 (fixed cost included) | closure control, marginal |
|---|---|---|---|
| **`dummy`**: the same literal at every site (the baseline requested for this probe) | **35.7** | 40.2 | 108.5 |
| `idlit`: each site's MsgId as a short literal (recommended; see "Baselines") | **24.5** | 28.5 | 97.3 |
| `literal`: each site's source text (plans/06 §3 wording) | 17.7 | 20.5 | 90.5 |

* **Threshold (≤ 40 B gz/site): met with every baseline.** The design costs
  about **one third of the closure-per-site control** in gz (35.7 vs 108.5
  against `dummy`), and 1/5 to 1/14 of it in raw bytes (90 vs 492 raw
  against `dummy`, 31 vs 432 against `idlit`).
* **Stretch (25 B gz): met against `idlit` and `literal`, not against
  `dummy`.**
* The weighted average is the M-delta of whole apps generated in 06 §2's mix,
  so it is weighted by construction. The explicit per-shape weighting gives
  28.0 B gz/site against `idlit` (the per-shape table is noisier; see
  "Per shape").
* The fixed cost of the `tr` library (part of B1) is **7.5 KB gz / 27.4 KB
  raw** after `wasm-opt -Oz` (M-delta intercept against `idlit`; 8.3 KB gz
  against `dummy`).
* **B6 canary grep: clean.** No `app.canary.zq7-canary-msg`,
  `zq7_canary_var` or `ZQ7-CANARY-TEXT` appears in the `tr` wasm (bindgen
  and `-Oz` outputs, M = 1,860 and 3,720) or its JS glue. The `closure`
  control ships the id and the variable name; `literal` ships the text and
  the variable name. A wider check found 0 of the 627 message texts ≥ 16 B
  and 0 of the 1,481 ids ≥ 16 chars in the `tr` wasm.
* **The audit's 14 B/site is reproduced in raw bytes but not in gz** (see
  "Audit figure").

## Commands

Everything runs from the repository root, on the shared 8-core machine with
`CARGO_BUILD_JOBS=3`, one build at a time. Size figures do not depend on
load.

```sh
# the whole matrix, resumable (skips apps already in target/results.tsv), then tables:
probes/p0-01-call-site/scripts/run-all.sh
#  = cargo xtask gen-workload templates --dump closure --out probes/p0-01-call-site/target/closure-builtin
#    python3 probes/p0-01-call-site/scripts/variants.py probes/p0-01-call-site/target/templates probes/p0-01-call-site/target/closure-builtin
#    cargo xtask gen-workload all -t <every template> --out probes/p0-01-call-site/target/wl-1860
#    cargo xtask gen-workload all -t <every template> -m 3720 -n 3200 -k 120 --out probes/p0-01-call-site/target/wl-3720
#    probes/p0-01-call-site/scripts/queue.sh probes/p0-01-call-site/target/results.tsv <app dirs…>
#    python3 probes/p0-01-call-site/scripts/analyze.py > probes/p0-01-call-site/target/tables.md

# one app, the plans/06 §3 size method exactly (scripts/build-app.sh):
cd <app> && CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=probes/p0-01-call-site/target/apps-<workload> \
  cargo build --lib --no-default-features --features hydrate --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen --target web --no-typescript --out-dir <pkg> <target>/wasm32-unknown-unknown/wasm-release/<lib>.wasm
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue <pkg>/<lib>_bg.wasm -o <pkg>/opt.wasm
gzip -9 -c <pkg>/opt.wasm | wc -c

# B6:
cargo xtask gen-workload canaries --grep     # then grep -c -a -F <pattern> <pkg>/opt.wasm

# side experiments (same pipeline; apps generated into the same workload dirs):
#  tr0 = templates/tr with {{index}} replaced by 0 (one id everywhere), target/templates-x/tr0
#  Ctr = templates/tr with `features = ["static-locale"]` on mf2-probe (strategy C), target/templates-c/
#  Ctr--child--none = the variant tr--child--none with the same feature
cargo xtask gen-workload all -t probes/p0-01-call-site/target/templates-c/Ctr \
    -t probes/p0-01-call-site/target/templates-c/Ctr--child--none --out probes/p0-01-call-site/target/wl-1860
probes/p0-01-call-site/scripts/queue.sh probes/p0-01-call-site/target/results.tsv probes/p0-01-call-site/target/wl-1860/app-Ctr …
# named builds for symbol attribution (not size measurements):
probes/p0-01-call-site/scripts/names-build.sh probes/p0-01-call-site/target/wl-1860/app-idlit probes/p0-01-call-site/target/names-1860-idlit
probes/p0-01-call-site/scripts/names-build.sh probes/p0-01-call-site/target/wl-1860/app-tr--child--none probes/p0-01-call-site/target/names-1860-tr--child--none
python3 probes/p0-01-call-site/scripts/fncat.py <idlit _bg.wasm> <tr--child--none _bg.wasm>   # gz by function kind
python3 probes/p0-01-call-site/scripts/sections.py <pkg>/opt.wasm …                           # gz by wasm section
```

`wasm-opt` 120 has to be told which features Rust 1.98's
`wasm32-unknown-unknown` emits (bulk memory, etc.); without the flags it
rejects the input. Profile `wasm-release` = opt-level z, fat LTO, cgu 1,
panic abort, strip (`[profile.wasm-release]` of the generated app). The
toolchain is stable 1.98.1 with Leptos 0.8.20, tachys 0.2.18, reactive_graph
0.2.14, wasm-bindgen 0.2.128 and wasm-opt 120.

Raw data: `target/results.tsv`, with columns workload, template,
cargo-raw, bindgen-raw, bindgen-gz, opt-raw and opt-gz (bytes). Tables:
`target/tables.md`.

## What was built

* **`crate/`** (`mf2-probe`) is the concrete design, evolved from
  `probes/audit/tr-prototype`. It uses Leptos 0.8.20 (latest stable),
  `forbid(unsafe_code)`, and denies `unwrap`/`expect`/`panic`/indexing. The
  client path has no fmt (integers are formatted by hand).
  * `Tr(MsgId)` is `Copy`, 4 bytes, built by `const fn tr(id)`.
  * `TrArgs` is **one** concrete type: `MsgId` + `ArgList`, with up to 4
    `ArgValue`s inline and a boxed slice beyond. Its constructor is
    `tr_args(id, [ArgValue; N])`, generic only in the array length (three
    instances in the workload). The type is never const-generic.
  * `ArgValue` is `Str(Oco<'static, str>)` | `Int` | `Float` |
    `Reactive(Signal<ArgValue>)`. There is one `From` per signal *type*, not
    per site.
  * `glue/tachys_0_2.rs` implements `Render`, `RenderHtml` (SSR delegates
    to `<&str>`, hydrate walks the cursor as `&str` does), `AddAnyAttr` and
    `AttributeValue` for both types. Every trait method is a one-line
    forward to a shared, out-of-line function.
  * `convert.rs` implements `From<Tr | TrArgs>` for `TextProp`,
    `Signal<String>`, `Oco<'static, str>` and `String`. `Tr::to_string()` /
    `TrArgs::to_string()` track the locale when called inside an observer.
  * The catalog is a stub: text by index, arguments appended. It is filled
    at run time from `<html data-catalog>`, so every site's id reaches an
    opaque lookup and the optimiser cannot fold a site away. The server
    reads a per-request `ServerCatalog` from context at render time.
  * Node updates use **strategy B**, the registry (see P0.11). The view
    state a site holds is a 4-byte slot index. `--features effect` selects
    strategy A and `--features static-locale` selects strategy C.
* **`templates/tr/`** is the `tr` template for workload-gen (data only).
  It covers the 7 shapes × modes `none` / `plain` / `signal` / `get`, plus the
  `const` table form for deferred labels (`[deferred] type =
  "mf2_probe::Tr"`, `static` rows of `tr(i)`). Markup (`rich`) sites are
  rendered as plain `tr(i)`, because `TrRich` is outside this probe. A
  `signal_prop` site in `get` mode becomes `Signal::derive(move ||
  tr_args(..).to_string())`, because `Signal<String>` has no
  `From<closure>`.
* **`templates/dummy/`** is the requested baseline (the same literal
  everywhere). **`templates/idlit/`** is the second no-text baseline: each
  site is a literal holding its MsgId, about 3.5 B per distinct message.
* **`scripts/variants.py`** writes 30 per-shape variant templates, also data
  only. `<impl>--<shape>` renders one shape with `tr` or `closure` and every
  other site with `idlit`; `<impl>--<shape>--none` does so only for that
  shape's sites without arguments.

## Method

* **Scales.** Two workloads are generated with the same knob ratios: the
  default **M = 1,860** sites (N = 1,600 messages, K = 60 components, R = 3
  lazy routes; 391 sites with arguments) and **M = 3,720** (N = 3,200,
  K = 120; 781 with arguments). The shape counts come from `sites.json`.
  Every app is built with the same profile and pipeline, and all apps of a
  workload share one cargo target dir.
* **Marginal per site** = (Δ@3,720 − Δ@1,860) / (sites@3,720 −
  sites@1,860), where Δ = app − baseline app of the same workload.
  **Fixed** = Δ@1,860 − 1,860 × marginal. Fixed costs (the library, the
  generic instantiations for its types) cancel in the marginal. B5 is the
  marginal; the fixed part belongs to B1.
* **Per shape**: the same M-delta over the shape's own site count, applied to
  the `<impl>--<shape>` variants (vs `idlit`). Arguments-only cost is
  (`--shape` − `--shape--none`) over the arg-site count.
* **Baselines.** `dummy` makes every site identical. LLVM and wasm-opt then
  merge code that differs only by its message: the helper `match` arms of
  the String shape fold into one arm, and identical per-block functions are
  deduplicated. The `dummy` app ends up **11.4 B gz / 59 B raw per site
  smaller than `idlit`**, even though both carry almost no text. That
  merging never happens in a real app, whose sites are distinct, so `dummy`
  overstates every implementation's cost by about 11 B gz/site. `idlit`
  keeps sites distinct with almost no text (+6.8 B gz/site vs `literal`,
  which is the text itself). This is why `idlit` is recommended as the B5
  baseline (a plan change, owner question 1).
* **Before / after `wasm-opt`.** "bindgen" = the `wasm-bindgen` output
  (before `wasm-opt`), "opt" = after `wasm-opt -Oz`. Note that `-Oz`
  **shrinks raw by 13–16 % but grows gz by 4.6–5.0 %** on every app here
  (e.g. `tr` @1,860: 2,442,089 → 2,102,439 raw, 577,038 → 603,295 gz).

## Results

### Whole apps (bytes)

| workload | template | bindgen raw | bindgen gz | opt raw | opt gz |
|---|---|---|---|---|---|
| wl-1860 | idlit | 2,333,995 | 524,264 | 2,017,470 | 550,270 |
| wl-1860 | dummy | 2,308,474 | 510,474 | 1,910,641 | 528,569 |
| wl-1860 | literal | 2,372,018 | 539,775 | 2,053,500 | 565,097 |
| wl-1860 | **tr** | 2,442,089 | 577,038 | 2,102,439 | 603,295 |
| wl-1860 | tr, strategy C (`static-locale`) | 2,436,094 | 574,689 | 2,096,267 | 599,775 |
| wl-1860 | closure | 3,314,849 | 697,628 | 2,813,459 | 729,688 |
| wl-3720 | idlit | 4,354,036 | 889,921 | 3,696,247 | 934,589 |
| wl-3720 | dummy | 4,302,068 | 863,079 | 3,479,020 | 891,996 |
| wl-3720 | literal | 4,429,774 | 918,507 | 3,766,483 | 962,035 |
| wl-3720 | **tr** | 4,548,743 | 985,015 | 3,838,799 | 1,033,128 |
| wl-3720 | tr, strategy C | 4,541,554 | 981,518 | 3,831,116 | 1,026,267 |
| wl-3720 | closure | 6,286,224 | 1,222,735 | 5,296,385 | 1,295,016 |

### Marginal per site (M-delta 1,860 → 3,720) and fixed cost

| impl | baseline | bindgen raw | bindgen gz | **opt raw** | **opt gz** | fixed opt raw | fixed opt gz |
|---|---|---|---|---|---|---|---|
| **tr** | dummy | 60.8 | 29.8 | 90.3 | **35.7** | 23,817 | 8,320 |
| **tr** | idlit | 46.6 | 22.8 | 31.0 | **24.5** | 27,386 | 7,511 |
| **tr** | literal | 26.3 | 15.7 | 12.6 | **17.7** | 25,562 | 5,303 |
| tr, strategy C | idlit | 45.9 | 22.1 | 30.1 | 22.7 | 22,725 | 7,332 |
| closure | dummy | 525.7 | 92.7 | 491.7 | 108.5 | ≈ 0 | ≈ 0 |
| closure | idlit | 511.5 | 85.7 | 432.3 | 97.3 | ≈ 0 | ≈ 0 |
| closure | literal | 491.2 | 78.7 | 413.9 | 90.5 | ≈ 0 | ≈ 0 |
| literal | idlit | 20.3 | 7.0 | 18.4 | 6.8 | — | — |
| dummy | idlit | −14.2 | −7.0 | −59.4 | −11.2 | — | — |

The closure control has no fixed cost of its own worth mentioning: its
`lookup` is a few hundred bytes, and the intercepts are −0.8 to −1.6 KB gz,
which is noise.

### Per shape and argument mode

Marginal bytes per site (M-delta 1,860 → 3,720) of the `<impl>--<shape>` variants against `idlit`. "no args" = the shape's sites without arguments (`--none` variant); "args only" = (all − no args) over the argument sites. Mix weights (06 §2): string 45 %, child 20 %, attr 8 %, text_prop 4 %, signal_prop 4 %, string_prop 4 %, deferred 8 %, if_else 7 %.

| impl | shape | sites M1/M2 | opt raw/site | **opt gz/site** | bindgen raw/site | bindgen gz/site | fixed opt gz |
|---|---|---|---|---|---|---|---|
| tr | string | 837/1674 | 5.3 | 0.2 | 8.1 | 0.7 | 6116 |
| tr | string (no args) | 636/1304 | -6.1 | -2.7 | -7.5 | -0.2 | 4326 |
| tr | string (args only) | 201/370 | 50.5 | 11.5 | 70.0 | 4.6 | 1335 |
| tr | child | 372/744 | 135.8 | 101.9 | 191.4 | 106.7 | 6618 |
| tr | child (no args) | 278/569 | -76.4 | 46.4 | -64.7 | 42.9 | 3908 |
| tr | child (args only) | 94/175 | 898.5 | 301.4 | 1111.4 | 335.8 | -604 |
| tr | attr | 149/298 | 75.4 | 85.6 | 75.9 | 76.5 | 1084 |
| tr | attr (no args) | 125/227 | 30.4 | 48.3 | 37.5 | 55.8 | 2680 |
| tr | attr (args only) | 24/71 | 172.9 | 166.7 | 159.4 | 121.5 | 1127 |
| tr | text_prop | 75/149 | -11.6 | -11.8 | -8.4 | -1.4 | 7002 |
| tr | text_prop (no args) | 49/120 | -6.5 | -20.4 | -4.1 | 14.2 | 5388 |
| tr | text_prop (args only) | 26/29 | -133.3 | 192.3 | -111.3 | -371.3 | -3271 |
| tr | signal_prop | 74/149 | 10.9 | -2.2 | 22.2 | 10.9 | 5874 |
| tr | signal_prop (no args) | 62/118 | -6.6 | -27.3 | -1.3 | 21.2 | 6198 |
| tr | signal_prop (args only) | 12/31 | 62.5 | 71.8 | 91.3 | -19.5 | 345 |
| tr | string_prop | 74/149 | 1.2 | -6.4 | 11.1 | 8.6 | 5275 |
| tr | string_prop (no args) | 62/112 | -6.8 | -26.8 | 1.2 | 23.2 | 5848 |
| tr | string_prop (args only) | 12/37 | 17.3 | 34.4 | 31.1 | -20.7 | 202 |
| tr | deferred | 149/297 | -3.8 | -10.8 | -3.4 | 0.6 | 6319 |
| tr | if_else | 130/260 | -23.4 | 33.2 | 12.4 | 35.2 | 5161 |
| tr | if_else (no args) | 108/192 | -74.0 | 14.8 | -47.4 | 2.9 | 5169 |
| tr | if_else (args only) | 22/68 | 69.0 | 66.9 | 121.6 | 94.2 | 1244 |
| closure | string | 837/1674 | 33.0 | 9.6 | 38.9 | 11.9 | 8541 |
| closure | string (no args) | 636/1304 | 19.4 | 6.7 | 23.6 | 7.6 | 3510 |
| closure | string (args only) | 201/370 | 87.0 | 21.2 | 99.5 | 28.7 | 4568 |
| closure | child | 372/744 | 1906.2 | 407.0 | 2245.3 | 363.9 | 5386 |
| closure | child (no args) | 278/569 | 1856.2 | 403.0 | 2223.6 | 361.7 | 3075 |
| closure | child (args only) | 94/175 | 2085.7 | 421.4 | 2323.0 | 371.9 | 2071 |
| closure | attr | 149/298 | 193.5 | 90.8 | 244.3 | 67.2 | 2020 |
| closure | attr (no args) | 125/227 | 174.0 | 127.7 | 204.2 | 69.4 | -3332 |
| closure | attr (args only) | 24/71 | 235.9 | 10.5 | 331.5 | 62.3 | 2656 |
| closure | text_prop | 75/149 | 184.1 | 50.4 | 216.9 | 61.6 | 6629 |
| closure | text_prop (no args) | 49/120 | 180.9 | 59.5 | 212.5 | 61.9 | 3441 |
| closure | text_prop (args only) | 26/29 | 260.7 | -163.7 | 321.7 | 54.3 | 8311 |
| closure | signal_prop | 74/149 | 184.1 | 43.4 | 231.1 | 49.4 | 5244 |
| closure | signal_prop (no args) | 62/118 | 159.5 | 34.9 | 194.1 | 42.4 | 3653 |
| closure | signal_prop (args only) | 12/31 | 256.6 | 68.5 | 340.1 | 70.2 | 1819 |
| closure | string_prop | 74/149 | 36.2 | 23.9 | 42.7 | 33.9 | 3833 |
| closure | string_prop (no args) | 62/112 | 18.7 | -2.1 | 20.9 | 11.0 | 3556 |
| closure | string_prop (args only) | 12/37 | 71.3 | 75.9 | 86.4 | 79.7 | 1264 |
| closure | deferred | 149/297 | 30.8 | 4.6 | 34.2 | 13.3 | 3905 |
| closure | if_else | 130/260 | 52.7 | 34.1 | 66.6 | 35.6 | 791 |
| closure | if_else (no args) | 108/192 | -0.9 | 3.9 | 2.9 | -0.8 | 2848 |
| closure | if_else (args only) | 22/68 | 150.6 | 89.1 | 183.0 | 102.1 | -12 |

Weighted by the mix (per-shape marginals, all modes):

| impl | opt raw/site | **opt gz/site** | bindgen raw/site | bindgen gz/site |
|---|---|---|---|---|
| tr | 33.7 | 28.0 | 49.6 | 31.0 |
| closure | 433.9 | 100.4 | 513.1 | 92.9 |

Reading the table:

* **Against the control, per shape (opt gz/site, tr vs closure):** string
  0.2 vs 9.6; text child 101.9 vs 407.0 (no args: 46.4 vs 403.0); attribute
  85.6 vs 90.8 (no args: 48.3 vs 127.7); `TextProp` −11.8 vs 50.4;
  `Signal<String>` −2.2 vs 43.4; `String` prop −6.4 vs 23.9; deferred rows
  −10.8 vs 4.6; `if`/`else` 33.2 vs 34.1. `tr` is cheaper or equal in every
  shape, and 4–25× cheaper in raw bytes wherever the control creates a
  closure.
* **String path (45 % of sites): ≈ 0 B gz/site** (0.2; −2.7 without
  arguments, 11.5 with them). `tr(i).to_string()` is a constant plus one
  call, cheaper in raw bytes than `String::from("…")`. The audit never
  measured this path; it is the cheapest shape.
* **Props and deferred rows: ≈ 0 ± 20.** These shapes have 74–149 sites per
  scale, and gz noise of a few hundred bytes gives ±20 B/site (hence the
  negative values). The conversions are single library functions.
* **The cost sits in view positions.** Text child 101.9 B gz/site (46.4
  without arguments), attribute 85.6 (48.3), `if`/`else` 33.2 (14.8). With
  arguments, a child costs about 300 B gz and an attribute about 170. That
  figure mixes the three modes at roughly a third each. **`get`** is the
  user writing `move || tr_args(…, x.get())`, one closure type per site that
  costs what the closure control does. **`signal`** passes a
  `Signal<ArgValue>` and costs one library effect, with no per-site closure.
  **`plain`** passes values. The counts (≈ 30 per mode and scale) are too
  small to split reliably.
* **What makes view positions cost gz but not raw bytes.** Without
  arguments, a `Tr` child site is *smaller* in raw bytes than the `idlit`
  literal (−76 B raw/site after `-Oz`) and yet 46 B larger in gz. Symbol
  pairing of named builds (`tr--child--none` vs `idlit`, M = 1,860) puts
  the difference in tachys' per-block **`hydrate_async`** state machines:
  +6.3 KB raw but +12.1 KB gz for 278 sites. Every other category is
  equal or smaller (`into_owned` −5.6 KB, since `&str` becomes a `String`
  there; `build` −4.2 KB). The async frames get larger (field offsets
  3,680 vs 2,784 in the same block) and contain extra `memory.copy`s. The
  code is no bigger; it compresses worse. It is not the update strategy:
  strategy C (no per-node state, no `Drop`) removes only ≈ 8 B gz per child
  site (38.8 vs 46.4). It is mostly not id entropy either: with the *same*
  id at every site (`tr0`), the app is still +37.5 KB gz over `idlit` at
  M = 1,860, and distinct ids add a further 8.3 B gz/site.

### Audit figure (14 B raw / ≈ 3 B gz per site, `Tr` as text child or attribute, no args, before `wasm-opt`)

| | bindgen raw | bindgen gz | opt raw | opt gz |
|---|---|---|---|---|
| audit (prototype, delta 200 → 400 sites) | 14 | ≈ 3 | — | — |
| this probe: text child, no args (vs `idlit`, M-delta) | −64.7 | 42.9 | −76.4 | 46.4 |
| this probe: attribute, no args | 37.5 | 55.8 | 30.4 | 48.3 |

**Raw: reproduced.** The per-site code is a few tens of bytes, and below the
literal for text children. **Gz: not reproduced.** Measured as the delta of
the whole gzipped file at scale, a view-position site costs 43–56 B gz
before `-Oz` and 46–48 B after. The audit's "≈ 3 B gz" was not a whole-file
measurement. The weighted average still passes, because the String path
(45 %) and the props/rows (24 %) cost almost nothing.

### Fixed cost of the `tr` library (part of B1)

7.5 KB gz / 27.4 KB raw after `-Oz` (intercept against `idlit`; 8.3 KB gz
against `dummy`, 5.3 KB against `literal`). Strategy C's intercept is
7.3 KB gz. In a named build, the library's own functions come to ≈ 7 KB raw
before `-Oz`. The rest is generic instantiations over its types:
`RenderEffect<TrText>` for user closures, and the `Signal<ArgValue>` /
`MemoInner<ArgValue>` read paths, which are reachable from `set_catalog`'s
formatter even in an app without reactive arguments.

## Conclusion

**Met.** The design costs 35.7 B gz per site against the requested `dummy`
baseline, 24.5 against `idlit` and 17.7 against the source-text literal,
weighted by 06 §2's mix, after `wasm-opt -Oz`. That is ≤ 40 in every case
and 3–4× below the closure-per-site control. The margin against `dummy` is
4.3 B. This is enough for a go, but not enough to be careless about in
Phase 6.

## Caveats

* The catalog and formatter are stubs. Real formatting (MF2 patterns,
  numbers) is fixed cost (B1), not per-site. Real pattern formatting could
  add per-site cost only if it changed the call-site expansion, which it
  does not.
* `TrRich` (markup) is not implemented. The 8 `rich` sites were rendered as
  `Tr`.
* The server paths type-check (`cargo clippy --features ssr` on the crate)
  but were not run. P0.2 ran an equivalent `Tr` end to end.
* Per-shape figures for shapes with < 150 sites per scale carry about
  ±20 B gz/site of noise. The per-mode split of arguments is not resolved.
* Timings were not part of P0.1. Sizes are exact and reproducible, and do
  not depend on load.
* The M = 2,000 variant of the plan's wording was not built separately.
  Whole-app averages at 1,860 and 3,720 bracket it (tr vs `dummy`: 40.2 and
  37.9 B gz/site including the fixed cost), and the marginal does not
  depend on M.

## Recommendations for the real `leptos-mf2`

1. **Keep the concrete-type design.** It passes the threshold even against
   the harshest baseline. Implement every trait method as a forward to one
   out-of-line function, as here. The String path and the conversions are
   then almost free per site.
2. **Look at view positions in Phase 6.** Their gz cost comes from tachys'
   generic async hydration (`hydrate_async` per block, instantiated because
   of lazy routes) around a non-`&str` leaf. Options, in order of
   expected effect:
   (a) measure with Leptos' `--cfg erase_components`;
   (b) propose a tachys hook, so that a leaf can reuse `&str`'s state and
   async path;
   (c) re-measure on tachys 0.3 (Leptos 0.9) before building workarounds.
3. **Steer users to `tr!(…, count = count)`** (signal-valued) rather than
   `move || tr!(…, count = count.get())`. The latter is a closure per site
   and costs what the old stack did.
4. **Keep the view state at 4 bytes** (a registry slot). Strategy B costs
   about 8 B gz per child site over C. That is acceptable for live
   switching, and C is available through `static-locale`.
5. **Trim the fixed cost.** Make `Reactive` argument formatting reachable
   only from reactive sites: the registry walk stores and formats only
   plain arguments, and signal-valued nodes re-render through their own
   effect. The `Signal<ArgValue>` machinery then drops out of apps without
   reactive arguments.
6. **Size method.** `wasm-opt -Oz` raises gz by ≈ 5 % on these apps. The
   size gate (06 §6) should record both, and the owner should decide
   whether B-budgets are measured after `-Oz`, after `-Os`, or on the
   bindgen output.

## Departures from the plan

* **Baseline.** The primary B5 baseline is `dummy`, as instructed, and
  `idlit` and `literal` are reported as well. I recommend `idlit` as the B5
  baseline in plans/06 §3, for the reason under "Baselines". C1 decides.
* **M = 1,860**, the generator default, rather than 2,000.
* **`TrArgs` constructor.** `tr_args` is generic over the array length N.
  The type stays concrete, as plans/04 §2 asks, but the constructor has 3
  instances. Plans/04 §2 should say so, or use arity-specific constructors.
