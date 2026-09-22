# 06 — Size and performance: budgets, reference workload, Phase 0 probes

Part of the [master plan](00-master-plan.md). RFC 2119 keywords apply.

**The budget is the feature.** mf2-two exists because i18n stacks bloat the
wasm. Every number here is enforced by CI, or is a Phase 0 probe whose result
replaces an estimate. Anything marked *estimate* MUST be replaced by a
measurement before Phase 1 begins.

## 1. Motivating baseline (measured)

Measured on a production Leptos 0.8 SSR+hydrate application using a
Fluent-based i18n stack, two locales, by diffing the release wasm before and
after i18n was introduced:

| Quantity | Value |
|---|---|
| wasm growth from i18n | **+1,765 KB raw / +525 KB gz** (+12.4 %) |
| …from code | +386 KB gz (73 %) — does **not** scale with locale count |
| …from data | +127 KB gz (24 %) — all locale text, embedded, ≈ 55 KB gz per locale |
| Locales shipped to every visitor | all of them |
| Work at first translation | parse ≈ 190 KB of message source, in the browser |

Two conclusions drive everything:

1. **Code is the bulk.** Moving text out of the wasm recovers a quarter at most.
   The per-call-site expansion, the parser, the resolver, hash maps of arguments
   and an all-locales plural table are the real cost.
2. **Text is what scales.** At 20 locales, embedded text alone would pass 1 MB gz.
   Locale data MUST be lazy.

## 2. The reference workload

The shape below was measured on that application and is reproduced by a
**deterministic synthetic generator** (`bench/workload-gen`, seeded). The
generated app is the only application the size gate measures; nobody needs the
original.

| Property | Value |
|---|---|
| Messages per locale | 1,600, in 18 source files |
| No placeholder (simple) | 79 % |
| 1 / 2 / 3 / 4 variables | 14.7 % / 5.0 % / 1.1 % / 0.2 % (with the 79 % above: 100 %) |
| Selecting (`.match`) | 0.9 %, all plural on one count — these sit **inside** the 1-variable bucket |
| Number/date formatting functions | none in the baseline; the generator has knobs to add them |
| Message text | mean 27 B, median 19 B, p90 58 B, max 259 B; ≈ 43 KB total per locale |
| Message id | kebab-case, mean 23.5 chars |
| Translator comments | ≈ 60 % of source-file bytes (never shipped) |
| Call sites | 1,860; 21 % pass arguments; 14 % needed an explicit reactive form in the old stack |
| Call-site position (heuristic classification, ± 5 points) — **the generator's default mix and the weights of budget B5** | **45 %** non-view code needing a plain `String` (match arms, function returns, error values set from event handlers) · **20 %** text child · **8 %** HTML attribute · **8 %** component prop taking reactive text (`TextProp` / `Signal<String>`) · **4 %** component prop taking `String` · **8 %** deferred label (a closure returning `String` in a registry; becomes a `const` table entry) · **7 %** `if`/`else` between two messages inside a view |
| Inline elements in sentences | present (e.g. a `<kbd>` inside a sentence), previously faked with private-use placeholder characters — MF2 markup replaces this |

Note what the text numbers imply: real message text is ~43 KB per locale; the
rest of a source file is comments and string ids. With integer ids and no
comments, a catalog should be **well under 25 KB gz per locale**.

The generator also emits pseudo-locales (`en-XA` accented/expanded, `ar-XB`
RTL-wrapped) and can scale N messages × M sites × L locales × R lazy routes, so
the gate can also be run at 10× scale. Defaults: N = 1,600, M = 1,860, L = 2 real
+ 2 pseudo locales, R = 3. "Matches this table" means: each percentage within
± 1 percentage point, mean text length within ± 5 %. It also writes every
locale as flat JSON (`id → source`), and commits the default-seed corpus to
`bench/corpora/workload-1600.json`.

How the generator (A7, `bench/workload-gen/README.md`) reads this table:
locales `en` (source), `pl` (second real locale, own plural categories, text
≈ 1.35× the `en` bytes), `en-XA`, `ar-XB`; "text length" is the byte length of
the whole MF2 source, placeholders included; markup-only messages count as
having no variable; the plural selects declare `.input {$count :integer}` (MF2
requires an annotated selector); an `if`/`else` counts one site per branch; the
reactive-prop share is 4 % `TextProp` + 4 % `Signal<String>`; the three
4-variable messages have no call site; `--number` / `--datetime` convert
existing variable messages, leaving the shape unchanged. Seed 1 measures
within every tolerance, also at 10× ([phase-0-results](phase-0-results.md) §A).

## 3. Budgets

All wasm numbers: `wasm32-unknown-unknown`, release profile
(`opt-level="z"`, `lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip=true`),
then `wasm-opt -Oz`, then gzip -9. Measured as a **delta** against the same
generated app with every call site replaced by a short literal **distinct per
site** (its MsgId: workload-gen's `idlit` baseline). Why that baseline (P0.1):
one identical literal everywhere lets LLVM and wasm-opt merge per-site code and
understates the baseline by ≈ 11 B gz/site; each site's source text subtracts
text that moves into the catalog anyway, flattering the design by ≈ 7 B gz/site.
The size gate also reports the `dummy` (identical literal) figure as a
conservative bound. Per-site costs are **marginal**: the M-delta between the
default workload and one twice its size, so fixed library cost (B1) cancels.
Note that `wasm-opt -Oz` shrinks raw bytes by 13–16 % but grows gz by ≈ 5 % on
these apps; the size gate records both, and which optimisation level delivers
the smallest compressed wasm is decided in P6 (the budgets stay on `-Oz` until
then, for comparability).

| # | Budget | Target | Stretch |
|---|---|---|---|
| B1 | Fixed client cost: catalog reader + evaluator + `:string` + selection + plural + bidi + parts/markup + Leptos glue + fetch/boot. Of it, the **core numeric semantics** (`:number`/`:integer`/`:offset`, linked only when the corpus uses them) ≤ 10 KB gz | ≤ 30 KB gz | 20 KB gz |
| B2 | Feature `fn-number` on **and** used: locale-aware numeric family incl. `:percent` | ≤ 3 KB gz | — |
| B3 | …plus `:currency` + `:unit`, when used | ≤ 5.5 KB gz more *(restated in Phase 4; was ≤ 4)* | — |
| B4 | Feature `fn-datetime` on **and** used | `datetime-intl`: ≤ 6 KB gz wasm + ≤ 1 KB gz JS glue (of which ≤ 3.5 KB gz date semantics every backend needs); `datetime-icu`: ≤ 95 KB gz Gregorian, ≤ 105 KB gz any calendar, `icu.blob` ≤ 3 KB gz per locale without zone names, ≤ 25 KB gz with | — |
| B1′ | Any feature on but **unused** by the corpus | +0 B over B1 | — |
| B5 | Per call site, marginal, weighted by §2's mix (the reference workload) | ≤ 40 B gz (P0.1: 24.5; 35.7 against the `dummy` bound) | 25 B gz |
| B6 | Locale bytes in the wasm (text, names, rules, symbols) | **0** | — |
| B7 | Catalog on the wire (production: COLD and IDS stripped), **brotli 11** — the `.br` file the build writes and serves | reference workload `en`: ≤ 0.91 × 25 KB = **23,296 B br**; **every locale**: br ≤ 0.91 × (0.5 × MF2 source bytes + 1 KB), and raw ≤ 1.25 × source bytes + 8 B/message | — |
| B8 | Locale data inside the catalog: plural + number symbols | ≤ 0.5 KB gz per locale | — |
| B9 | Catalog load (validate + index) for 1,600 messages, 4× CPU throttle, warm code. (The cold first install — ≈ 1 ms of one-off engine warm-up in isolation — is counted in the app's boot, measured from P6 on.) | ≤ 1 ms | 0.2 ms |
| B10 | Format: simple message / 1-argument pattern (native) | ≤ 100 ns, 0 allocs / ≤ 500 ns, ≤ 1 alloc | — |
| B11 | Extra round trips before hydration | 0 — the catalog is preloaded and downloads in parallel with the wasm | — |
| B12 | `core::fmt` and panic-formatting machinery reachable from the client runtime crates | **absent** | — |
| B13 | Unused functions in the wasm | **absent** — an app whose corpus never uses `:datetime` links no date code | — |

**Budgets moved by Phase 0** (evidence in [phase-0-results](phase-0-results.md)):
the numeric share of B1 was ≤ 6 KB gz and measured 10.2 KB gz (8.5 `no_std`;
P0.5) — raised to ≤ 10, with two known savings (float → text through the
`Host`, −2.8 KB gz; build-time option interning, ≈ −0.5) to recover inside B1.
B2 was ≤ 15 KB gz *(estimate)* and measured 1.7 — tightened to ≤ 3. B3 was
≤ 5 *(estimate)*, measured 2.9 — tightened to ≤ 4. B4-intl was ≤ 3 *(estimate)*
and measured 5.4 + 0.6 JS, 3.4 of it semantics every backend needs — raised.
B4-icu ≈ 100 *(audit)* is confirmed (93 / 105 KB gz) and now carries the blob
sizes. B7 did not account for locales whose text is longer than the reference:
the synthetic `pl` (1.35× the `en` bytes) misses 25 KB gz by 1 KB under every
layout while `en` measures 20.5 KB — so B7 now scales with source bytes (all
four generated locales meet it; P0.7). B8 was ≤ 2 KB gz and measured ≤ 0.2 —
tightened to ≤ 0.5. B9 measured 0.08 ms warm at 4×; the cold first install
(1.12 ms in an isolated page) is engine warm-up of code, not catalog work, so
B9 names the warm load and the boot budget (P6) carries the rest. B5's value
is unchanged but its baseline is now defined (distinct per-site literals) and
its figure is marginal, both for the reasons given under the method above.

**Whole-app ambition** for the reference workload: ≈ 30 KB fixed + 1,860 × 40 B
≈ **105 KB gz**, against 525 KB gz — and **+0 bytes of wasm per added locale**.
Phase 0 measured the parts separately — runtime floor 7.0 KB gz, core numeric
semantics 10.2, call-site library 7.5, 24.5 B gz per site — not the whole; the
size gate measures the whole from P5 on.

**Budget moved by Phase 2** (owner, 2026-09-21; evidence in
[phase-2-results](phase-2-results.md) §A8): **B7 is stated on brotli**, the
encoding nearly every visitor downloads — the build writes a `.br` file beside
each catalog and `mf2-axum` serves it to any client that accepts brotli
([02](02-catalog-format.md) §3); the `.gz` file only reaches clients that do
not. The limits are the gzip ones scaled by **0.91**, the *worst* brotli/gzip
ratio measured on the four locales (en-XA: 0.909, the same in P0.7 and in
Phase 2), rounded up — so no locale is held tighter than it was under gzip, and
every locale, and P0.7's own figures, pass. gzip is still reported (the
fallback path), not gated; which gzip implementation produces the `.gz` file is
`mf2-build`'s choice (P5a). The wasm budgets stay on gzip -9: the app's server
or CDN chooses their encoding, so gzip -9 remains the reproducible stand-in.

### How B6, B12, B13 are checked (not just hoped for)

* **B6 canary.** Every generated locale contains a unique canary string, a canary
  variable name and a canary message id. CI greps the final wasm for all three;
  any hit fails the build.
* **B12 symbol check.** `twiggy`/`wasm-objdump` over a probe that links only the
  client runtime crates MUST show no `core::fmt::` symbols and no panic message
  strings. Rules that make this true: `#![no_std]`, no `format!`/`Debug`/
  `Display` on the client path, no indexing or `unwrap` that can panic (use
  `get()` and return the fallback), integer/decimal to text by hand-written
  routines. Error *types* still use `thiserror`; their `Display` impls exist but
  MUST be unreachable from the client build.
* **B13 closed world.** `mf2-build` records which functions the corpus uses and
  generates the registry constructor with only those handlers. Handlers are
  plain `fn` items referenced only from that constructor, so dead-code
  elimination removes the rest. CI builds the reference app with and without a
  single `:datetime` message and asserts the symbol sets differ accordingly.

### Phase 0 measurements (2026-09-20/21)

Deltas against a fair base (allocation kept alive), `wasm-release`, `wasm-opt
-Oz`, gzip -9. Full tables, methods and caveats: [phase-0-results](phase-0-results.md).

| Item | Δ raw / Δ gz | Probe |
|---|---|---|
| Runtime floor: reader + evaluator + selection + plural + bidi + parts, `no_std`, per-access UTF-8 | 14.0 KB / **7.0 KB** | P0.3 |
| Own plural evaluator (correct on all 15,041 CLDR samples) | 0.67 KB / **0.43 KB** | P0.4 |
| `icu_plurals` 2.3 + blob / compiled data | 29.8 KB / 14.5 KB · 39.8 KB / 17.9 KB | P0.4 |
| Plural data per locale: ours cardinal + ordinal / ICU4X payload cardinal, ordinal | 0–84 B / 5–188 B, 5–125 B | P0.4 |
| Core numeric semantics over `fixed_decimal` (std app) / `no_std` absolute | 21.7 KB / **10.2 KB** · 17.1 KB / 8.5 KB | P0.5 |
| `fn-number` layer incl. `:percent` / + `:currency` + `:unit` | 3.5 KB / **1.7 KB** · +6.5 KB / +2.9 KB | P0.5 |
| `icu_decimal` + blob / `Intl.NumberFormat` glue (over the core) | 35.8 KB / 15.9 KB · 4.0 KB / 1.6 KB (+0.6 KB JS) | P0.5 |
| MF2 date semantics / + `Intl.DateTimeFormat` glue | 6.2 KB / 3.4 KB · 11.2 KB / **5.4 KB** (+0.6 KB JS) | P0.6 |
| `icu_datetime` + blob: Gregorian all options / no zone styles / any calendar | 221 KB / **93 KB** · 146 KB / 64 KB · 257 KB / 105 KB | P0.6 |
| Catalog, reference `en`, stripped | 50.9 KB raw / **20.5 KB gz** / 17.9 KB br | P0.7 |

The planning audit's plural and per-site deltas were taken against a base whose
allocation LLVM deleted, so they included ≈ 5.1 KB gz of allocator and panic
runtime; its absolute ICU4X sizes reproduce.

Per call site (P0.1: Leptos 0.8.20 `hydrate`, `wasm-release`, after `wasm-opt
-Oz`, marginal between M = 1,860 and 3,720, against `idlit`; B gz/site, raw in
brackets; shapes with < 150 sites carry ± 20 B of noise):

| Call-site shape | concrete `Tr` / `TrArgs` | closure-per-site control |
|---|---|---|
| non-view `String` (45 %) | 0.2 [5] (11.5 with args) | 9.6 [33] |
| text child (20 %) | 101.9 [136] (46.4 without args) | 407 [1,906] |
| HTML attribute (8 %) | 85.6 [75] (48.3 without args) | 90.8 [194] |
| `TextProp` / `Signal<String>` / `String` prop, deferred row (24 %) | ≈ 0 | 5–50 |
| `if`/`else` (7 %) | 33.2 | 34.1 |
| **weighted (whole apps)** | **24.5 [31]** | **97.3 [432]** |

The planning audit's "14 B raw / ≈ 3 B gz" for a view site reproduces in raw
bytes only; at whole-file scale a view site costs 43–56 B gz (tachys' per-block
async hydration code compresses worse around a non-`&str` leaf — a P6 item,
[04](04-leptos-integration.md) §1). Fixed cost of the call-site library: 7.5 KB
gz (inside B1).

Reactive-graph costs (P0.11): a `RenderEffect` per node costs 427 B and 10
allocations on wasm32 (797 B native) and leaks +72 B per churned node until the
next switch; the registry costs 44.8 B and ≈ 0 allocations per node with a flat
heap; a switch with 2,000 live nodes takes 6.8 ms of script at 4× CPU throttle
(registry) vs 12–17 ms (effects) — hence D7 = B.

Comparable prior art: `leptos_i18n`'s lazy mode removes the strings but, by its
own documentation, "the code to render each key is still baked in" — the
per-key code is exactly the cost technique 2 below exists to avoid.

### Phase 2 measurements (2026-09-21)

Details and commands: [phase-2-results](phase-2-results.md).

| Item | Figure | Harness |
|---|---|---|
| B7, catalogs stripped, brotli 11: en / pl / en-XA / ar-XB | **18,072** / 24,137 / 21,537 / 18,423 B br (limits 23,296 for `en`; 0.91 × (0.5 × source + 1 KB): 20,610 / 27,557 / 44,192 / 25,725) | `cargo xtask catalog-size` |
| The same, GNU gzip -9 -n (reported, not gated) | 20,592 / 26,706 / 23,686 / 20,597 B gz | same |
| Against P0.7's layout, brotli / gzip | +143 / +69 / +60 / −68 B br; +56 … +79 B gz; the pool identical, the delta is NAMES' `str32` | same |
| Reader: `Catalog::new` native, en stripped / unstripped | **2.29 µs** / 6.26 µs, 0 allocations, 0 copies (P0.8: 5.7 µs) | `catalog-bench bench` |
| Simple `get` + `text`, native: en / pl / en-XA / ar-XB | **19.3** / 65.6 / 77.8 / 28.6 ns (P0.8 en: 20.7) — UTF-8 per access is 80–85 % on non-Latin text | same |
| B12, reader: panic import after LTO + `wasm-opt -Oz`; `core::fmt` | **absent**; none | `bench/b12/check.sh` |
| Reader + a walk over its whole API (part of B1), Δ against the base | 13,613 B raw / **6,787 B gz** (`Catalog::new` 3.4 KB raw) | same |

**Budget moved by Phase 4** (owner, 2026-09-22; evidence in
[phase-4-results](phase-4-results.md)): **B3 is ≤ 5.5 KB gz** (was ≤ 4). The
built `:currency` + `:unit` measured 6,048 B gz over `fn-number`'s registry,
5,432 after taking out what was duplicated (one options loop for both, one
`fill`, byte-level blanks, the currency spacing decided once, the runtime's
digit-size parser; B2 fell 2,151 → 2,069 with it) — `bench/b12/check.sh`,
harness `b12-runtime-fn-number-measure`. P0.5's 2.9 KB probe had a plural stub
and `en-US` data only; the functions as built carry what CLDR and ECMA-402
do and it did not: plural-form currency and unit names, the
`…alphaNextToNumber` patterns and currency spacing, currency-specific
patterns and separators, accounting and narrow symbols, unit-width fallback
and `X-per-Y` composition. Moving the currency presentation and the unit
composition into the catalog writer was estimated at −0.9 to −1.4 KB gz for
a larger catalog and literal-only compound units; not taken.

### Phase 4 measurements (2026-09-22)

Details and commands: [phase-4-results](phase-4-results.md). Sizes on the
merged tree (`bash bench/b12/check.sh`; wasm-release, `wasm-opt -Oz`,
`gzip -9 -n`; each a delta between two harnesses differing only in their
registry or host).

| Item | Figure | Harness |
|---|---|---|
| B1, runtime part | 37,772 B raw / **18,864 B gz** (Phase 3: 18,428; +349 for the Phase 4 API hooks, +15 for `Host::numbers`, +72 for the date ones) | `b12-runtime` |
| B1's numeric share | 5,377 B gz (≤ 10 KB) | `b12-runtime` − `b12-runtime-nonum` |
| **B2** `fn-number` on and used (`:number` / `:integer` / `:offset` localized, `:percent`, unannotated numbers) | 4,062 / **2,034 B gz** (≤ 3,072) | `b12-runtime-fn-number` |
| **B3** + `:currency`, `:unit` | 11,779 / **5,454 B gz** (≤ 5,632, restated above) | `b12-runtime-fn-number-measure` |
| **B4** `datetime-icu`, Gregorian with zone styles | 154,958 / **69,641 B gz** (≤ 97,280); without zone styles 42,820 | `b12-dates-icu-greg-zones` |
| **B4** `datetime-icu`, any calendar with zone styles | 213,235 / **83,028 B gz** (≤ 107,520); without 55,476 | `b12-dates-icu-any-zones` |
| **B4** `datetime-intl` | wasm **5,131 B gz** (≤ 6,144), JS glue **668 B gz** (≤ 1,024) | `b12-dates-intl` − `b12-dates-web-base` |
| … of which the date semantics every backend needs | 7,600 / **3,577 B gz** (the row's note: ≤ 3,584) | `b12-dates-semantics` − `b12-dates-base` |
| `icu.blob` per locale, gzip -9 | **0.61–0.91 KB** without zone names (`th`, any calendar 1.05; ≤ 3), **18.3–20.9 KB** with them (≤ 25); `:datetime`'s defaults alone 0.28–0.45 KB | `cargo test -p mf2-locale-data --features icu-blob --test icu_blob sizes -- --nocapture` |
| **B8** plural + `number.symbols` (+ the percent pattern) per panel locale | **47–85 B gz** (≤ 512); four currencies with every display 136–252 B gz, five units × three widths 200–310 B gz | `cargo test -p mf2-locale-data --test numbers b8 -- --nocapture`, 02 §4.8 |
| **B1′** each feature on and unused: `fn-number`, `fn-datetime`, `mf2-host-web`'s date features, `intl` | **0**, **0**, **0** (JS 0), **−69 B gz** | `*-unused` harnesses |
| **B12** | clean for the reader, the runtime, the numeric and date functions and the `intl` path; reported, not gated, for the ICU4X harnesses (7 fmt / 11–24 panic symbols, ICU4X's) and `mf2-host-web`'s own `wasm-bindgen` / `js-sys` glue (a panic path, with or without the date features) | `bench/b12/check.sh` |
| **B13** | 0 numeric symbols without the numeric handlers, 0 date symbols without a date function | same |

### Phase 5a measurements (2026-09-22)

Details and commands: [phase-5a-results](phase-5a-results.md). These are the
catalogs an application actually serves — the manifest built from the source
locale, fallbacks flattened, COLD and IDS stripped, and the LOCALE entries the
corpus needs — rather than catalogs written straight from a corpus file.

| Item | Figure | Harness |
|---|---|---|
| **B7**, the build's own catalogs, brotli 11: en / pl / en-XA / ar-XB | **17,992** / 24,109 / 21,502 / 18,498 B br (same limits as Phase 2) | `cargo test -p mf2-build --test sizes -- --nocapture` |
| The same, raw | 51,516 / 67,945 / 101,170 / 61,444 B (limits 66,862 / 85,947 / 131,647 / 80,913) | same |
| What the locale data costs on the wire | **−80 / −28 / −35 / +75 B br** — three of the four locales are *smaller* with 12–24 B of number symbols in them | same |
| **B8** plural + `number.symbols`, the build's catalogs | reference workload 17–45 B; a corpus using every numeric function, over the 11-locale panel, 12–47 B (≤ 512) | `cargo test -p mf2-build --test slicing -- --nocapture` |
| **B6** on the generated module | clean: no canary text and no catalog name in the client artifact | `cargo xtask codegen-matrix` |
| One `build.rs` pass, reference workload (1,600 × 4) | **369 ms** cold / 358 ms warm release, 2.15 s debug; peak 21.9 / 26.5 MB | `cargo run --release -p build-cost` |
| `icu.blob` at build time | +0 (315 ms with, 350 ms without, on a corpus with 160 `:datetime` messages); ≈500 B per catalog | same, `--features datetime-icu` |

Built *without* `fn-number` the same corpus gives exactly Phase 2's brotli
figures (18,072 / 24,137 / 21,537 / 18,423), which is what says the
difference above is the number data and nothing else.

**B1′ and B13 on the generated module**, as byte deltas on the client wasm
`tools/i18n-fixture` builds for `wasm32-unknown-unknown` under the
`wasm-release` profile (§3's size method), the corpus edited and put back:

| | Corpus | Features | `.wasm` |
|---|---|---|---:|
| **B13** | `:integer` and markup | `hydrate,fn-number` | 364,235 |
| | plus `:currency`, `:unit`, `:percent` | `hydrate,fn-number` | 377,834 → **+13,599 B** |
| **B1′** | nothing unannotated, no numeric or date function | `hydrate` | 349,171 |
| | the same | `hydrate,fn-number,fn-datetime` | **349,171 → +0 B** |

So a corpus that uses none of `:currency`, `:unit`, `:percent` pays nothing
for them (13.6 KB avoided), and two function crates linked but unreachable
from the generated registry cost nothing at all. Both were taken by hand;
a `b12-generated` harness pair that makes them a gate is Phase 5b's to add.

### Phase 4: the `intl` client option (2026-09-22)

Owner decision 4 ([03](03-runtime.md) §5.3). The option is opt-in and has no
budget of its own; what it costs is measured against the Rust path in the
same harness, and B1′ and B12 hold for it. Tree 79c4d7f.

| Item | Figure | Command |
|---|---|---|
| B1′: `intl` on, no number in the corpus | **−34 B raw / −69 B gz** against `b12-runtime-nonum` (a resolved number keeps its digit plan, not its rounded digits: every `Value` is smaller); gated ≤ +0 | `bench/b12/check.sh` (`b12-runtime-intl-unused`) |
| B1 moved by `Host::numbers` (one vtable slot, every client) | +17 B raw / **+15 B gz** of the runtime part (18,849 → 18,864 B gz Δ) — the price of B1′ = +0 | same (`b12-runtime`) |
| B12 for the `intl` path; the Rust rounding, display and plural evaluator not linked | **absent**; none linked (`b12-runtime`: 4 such symbols) | same (`b12-runtime-intl`, `b12-runtime-fn-number-intl`) |
| Size of the option, B gz of wasm + JS over the harness's base: core numbers | **7,613** (6,630 + 983 JS) against the Rust path's 5,408: **+2,205** | `bench/intl-probe/scripts/build.sh` (`rt-intl` vs `rust`) |
| … with `fn-number` (symbols, grouping, `:percent`) | **7,604** against 7,215: **+389** | same (`rt-intl-loc` vs `rust-loc`) |
| … with `:currency`, `:unit` too | **9,016** against 12,659: **−3,643** | same (`rt-intl-cu` vs `rust-cu`) |
| JS glue | **983 B gz** (the inline module 779) | same |
| Per numeric placeholder, `intl` / Rust path | 2.2–2.7× `:number`, 4.0–4.2× `:integer` (Chromium 143); 2.2–3.3×, 3.9–6.1× (Firefox 155); 2.4–2.8×, 4.9–5.1× (WebKit 26.6); +1.1 to +7.1 µs a format, +4 to +27 µs at 4× (Chromium) | `bench/intl-probe/scripts/2-speed.sh`, three runs, variants alternated |
| Per select (1 and 3 keys) | 1.7–2.4× (Chromium), 2.3–3.5× (Firefox), 1.9–2.8× (WebKit); a select that also formats its number 3.4–5.4× | same |
| Against A0's probe handlers, same runs | faster in nearly every row (e.g. Chromium select 2.35–2.43× against 2.65–2.72×) | same (`rt-intl` vs `intl`) |
| L4 in the engines | **324 / 324** runtime tests in Chromium, Firefox and WebKit; no suite test differs from the Rust path | `cargo xtask l4-web` |

### Phase 3 measurements (2026-09-21)

Details and commands: [phase-3-results](phase-3-results.md). Timings on the
Phase 0 machine, whose clock drifts (1.8–2.3 GHz): ranges are over the day's
runs, and builds are compared by alternating their binaries.

| Item | Figure | Harness |
|---|---|---|
| B1, runtime part: reader + evaluator + `:string` + core numbers + plural + bidi + parts, Δ against the base | 37,126 B raw / **18,428 B gz** — with P0.1's call-site library (7.5 KB gz) 25.9 of B1's 30 KB, 4.1 KB left for fetch/boot (P6) | `bench/b12/check.sh` (`b12-runtime`) |
| The same without the numeric handlers | 26,899 B raw / 13,291 B gz (P0.3's skeleton, without `u:` options, decimals, the registry, NFC, named arguments or the frozen reader's validation: 7.0 KB gz) | same (`b12-runtime-nonum`) |
| B1's numeric share (≤ 10 KB gz) | **5,137 B gz** (10,227 raw); over `fixed_decimal` 0.7.2 7,311 B gz | same, and `NUMBER-AB-P3.md` |
| B12, runtime: panic import after LTO + `wasm-opt -Oz`; `core::fmt` | **absent**; none (over `fixed_decimal`: 8 panic / alloc-failure symbols) | same |
| B13: without the numeric handlers | **0** numeric symbols linked (14 with them) | same |
| B10 simple, `write`: en / pl / en-XA / ar-XB | **20.1–34.5** / 69.6–81.3 / 83.3–93.2 / 33.6–43.6 ns, 0 allocations (`Formatter::simple`: 13.0–22.3 ns on `en`; P0.8 20.7) | `runtime-bench b10` |
| B10 1-argument pattern, reused `String` | **118–137** / 133–149 / 155–175 / 130–150 ns, 0 allocations; a new `String::with_capacity(128)`: 1.02 allocations (P0.8 `en`: 93.5 / 106.3) | same |
| Select (not a B10 row), `en` | 433–514 ns, 4 allocations (P0.8: 317 ns on P0.3's integer-only `:integer`; the full numeric semantics cost ~110 ns over an unannotated integer) | same; `examples/select_cost.rs` |
| Function resolution per call / from a load-time table | 12.6 / 2.8 ns — stays per call | same |
| `:number` differential against ECMA-402 (P0.5's 100,000 cases) | 95,675 identical, **0 different**; the 4,325 sets `Intl` rejects are all *Bad Option* | `runtime-bench numbers ecma` |

## 4. Techniques, ordered by expected effect

1. **No parser in the client.** Parse at build time; ship binary catalogs.
2. **One concrete call-site type.** `tr!` expands to constructing a small
   non-generic value (`MsgId` + positional args). Rendering as a text child, as
   an attribute, or converting into `Signal<String>` / `TextProp` / `String` is
   implemented **once, inside the library**. No per-site closure type, no
   per-site monomorphisation of reactive plumbing.
   See [04-leptos-integration](04-leptos-integration.md).
3. **Ids are integers, arguments are positional slots**, both resolved at compile
   time. The wasm contains no message ids and no argument names.
4. **Closed-world linking** of functions and locale data (B13).
5. **fmt-free, panic-free runtime** (B12).
6. **Simple-message fast path**: 79 % of messages never enter the evaluator; text
   goes from the catalog buffer to the DOM / SSR buffer with no `String`.
7. **Locale data is sliced and lazy**: this locale's plural rules and symbols
   travel in this locale's catalog. Adding a locale adds no code.
8. **Host services instead of tables**: NFC normalization always, and date
   formatting when the application picks `datetime-intl`, are delegated to the
   JS host on the client.
9. **Flattened fallbacks**: every catalog carries every id; one fetch, no chain
   walking, no second catalog in memory.
10. **Immutable, content-hashed, precompressed catalogs**, decoupled from the
    wasm hash.
11. **No JSON, no serde on the client.** Boot data is two short strings.
12. **Shared scratch buffer** for pattern formatting; one allocation only when a
    caller really needs an owned `String`.

## 5. Phase 0 probes

Throwaway code under `probes/` (not workspace members of the real crates; deleted
or archived at Phase 0 exit). Each probe has a threshold and gates a decision in
the master plan §8. Results are written to `plans/phase-0-results.md`.

| # | Probe | Method | Threshold | Gates |
|---|---|---|---|---|
| **P0.1** | **Call-site cost — go/no-go** | Generate 2,000 sites in the **seven shapes of §2, in §2's proportions** (non-view `String`, text child, HTML attribute, reactive-text prop, `String` prop, `const` table entry, `if`/`else`), 21 % of them with 1–3 args (plain, and signal-valued vs. an enclosing `move \|\|` closure). Compare a closure-per-site control against the concrete-type design, **after `wasm-opt -Oz`**. Report per shape and the weighted average. The audit's 14 B/site (view shapes, no args) is the number to reproduce. | ≤ 40 B gz/site, weighted by §2's mix | The whole project. If the concrete-type design cannot beat the control decisively, stop and rethink. |
| **P0.2** | **Vertical slice — go/no-go** | Hand-built catalog → concrete type → SSR (including a streamed `Suspense` boundary, to prove the catalog context is reachable whenever `to_html_with_buf` runs) → preload → fetch → hydrate → switch locale, in a cargo-leptos app with one `#[lazy]` route. | zero hydration warnings; catalog request overlaps the wasm request; lazy route sees the i18n state; SSR context lookup works under streaming | D9 (render-time lookup vs. capture), minimum Leptos version, `forbid(unsafe_code)` feasibility in the client crates |
| P0.3 | Runtime floor | `no_std` evaluator skeleton (catalog reader + pattern + select + parts), fmt-free. | ≤ 12 KB gz | B1 credibility |
| P0.4 | Plural | Own rule parser + encoder + evaluator, run against **every CLDR `@integer`/`@decimal` sample for every locale** (correctness is the open question; D3 itself is decided). Size comparison against `icu_plurals` (runtime blob, compiled data) on `en`, `ar`, `ru` only to reproduce the audit. | 100 % of samples; ≤ 1.5 KB gz code; ≤ 300 B data/locale | confirms D3; fixes the `plural.*` catalog entries for P2 |
| P0.5 | Numbers | Core numeric semantics over `fixed_decimal` (all REQUIRED `:number` options incl. rounding modes/increments) with neutral output → its cost inside B1; then the `fn-number` localization layer with catalog-borne symbols → B2; then `:currency` and `:unit` code plus **per-locale data size** for "used" vs. "all" currencies/units → B3. `icu_decimal` + blob measured once for comparison. | B1 (numeric share ≤ 6 KB gz), B2, B3 | budgets only — D4 is decided |
| P0.6 | Dates | (a) `icu_datetime` + blob (semantic skeleton subset the spec needs), (b) `Intl.DateTimeFormat` glue. | sets B4 | D4 |
| P0.7 | Catalog encoding | Encode the reference workload; raw/gz/br; single vs split string pools; fixed vs varint index. | B7 | Format details in 02 §6 |
| P0.8 | Load + lookup speed | wasm in a browser, desktop and 4× CPU-throttled; uses P0.7's encoding and P0.3's evaluator. | B9, B10 | cost of the load step (pool split + UTF-8 pass) |
| P0.9 | Build orchestration | i18n crate with `build.rs` → manifest + generated `tr!` wrapper → proc-macro reads manifest. Check: cargo-leptos dual (ssr + hydrate) build, incremental rebuild after editing one message, multi-crate workspace, rust-analyzer expansion, wall-clock of 2,000 expansions. | edit→rebuild correctness; ≤ 2 s macro overhead per 2,000 sites | D8 |
| P0.10 | Hydration tolerance | Source reading says hydration from server HTML never compares or sets text/attribute content. Confirm in a browser: render different text on server and client for one node, in debug and release; then do the same with a *structural* difference (markup). | text: no warning, server text stays until next update; structure: documented failure mode | Risk sizing for `datetime-intl`; hydration gate rules for markup |
| P0.11 | Node update strategy | 2,000 live translated nodes + a virtual list that churns 100k nodes: (A) `RenderEffect` per node vs (B) library registry. Heap growth, switch latency, bytes. | B: flat heap under churn; switch ≤ 1 frame budget ×2 | D7 |
| P0.12 | Parser baseline | Create `bench/parser-gate/` (a permanent workspace member, not a probe) and the committed corpora; re-measure `ox_mf2_parser` on them under the comparison rules of [05-tooling](05-tooling.md) §1 (fresh and reused state). These numbers replace the audit table as the baseline. | harness + corpora committed; baseline recorded | The D1 gate at P1 exit |

P0.1, P0.2 and P0.9 are the three that can kill or reshape the project (the
no-go conditions are in the [work order](07-phase-0-work-order.md), C3); P0.1 and
P0.2 run first.

## 6. CI size gate (from Phase 5 on)

`cargo xtask size` builds the generated reference app, computes every budget in
§3, writes `target/size-report.json` + a Markdown table, and fails on regression
beyond a small tolerance (default 1 % or 256 B, whichever is larger). The report
is attached to every CI run; the numbers in this file are updated only by a
commit that also explains the change.

Benchmarks (`criterion` native; a small wasm timing harness in the browser) cover
load, simple lookup, pattern format, select format, and locale switch with 2,000
live nodes.
