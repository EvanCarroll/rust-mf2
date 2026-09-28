# rust-mf2 — master plan

**Unicode MessageFormat 2 for Rust applications: full spec, lazy-loaded web locales, native CLI and TUI support.**

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted
as described in RFC 2119. This document is the single source of truth; the
companion documents elaborate it and MUST NOT contradict it.

| Doc | Subject |
|---|---|
| [01-conformance](01-conformance.md) | the WG test suite through six layers; the ledger |
| [02-catalog-format](02-catalog-format.md) | the `.mf2b` binary catalog |
| [03-runtime](03-runtime.md) | evaluator, functions, feature flags, locale data |
| [04-leptos-integration](04-leptos-integration.md) | `tr!`, rendering, SSR→hydrate, switching, a11y |
| [05-tooling](05-tooling.md) | parser, build pipeline, manifest, CLI, conventions |
| [06-size-and-perf](06-size-and-perf.md) | budgets, reference workload, Phase 0 probes, size gate |
| [07-phase-0-work-order](07-phase-0-work-order.md) | Phase 0 work order (done) |
| [phase-0-results](phase-0-results.md) | Phase 0 measurements, go/no-go recommendation |
| [08-phase-1-work-order](08-phase-1-work-order.md) | Phase 1 work order (done); frozen `mf2-model` types |
| [phase-1-results](phase-1-results.md) | Phase 1 measurements: L1/L2, the D1 gate, generated input, fuzzing |
| [09-phase-2-work-order](09-phase-2-work-order.md) | Phase 2 work order (done); the frozen `mf2-catalog` API |
| [phase-2-results](phase-2-results.md) | Phase 2 measurements: format v1, L3, B7, reader cost and B12, fuzzing |
| [10-phase-3-work-order](10-phase-3-work-order.md) | Phase 3 work order (done) |
| [phase-3-results](phase-3-results.md) | Phase 3 measurements: the runtime core, D15, L4 |
| [11-phase-4-work-order](11-phase-4-work-order.md) | Phase 4 work order (done) |
| [phase-4-results](phase-4-results.md) | Phase 4 measurements: the function families, the `intl` option |
| [12-phase-5a-work-order](12-phase-5a-work-order.md) | Phase 5a work order (done) |
| [phase-5a-results](phase-5a-results.md) | Phase 5a measurements: the build pipeline, the CLI, B7/B8, both owner questions |
| [13-phase-5b-work-order](13-phase-5b-work-order.md) | Phase 5b work order (done): the macros |
| [phase-5b-results](phase-5b-results.md) | Phase 5b measurements: the call-site types, the macro, L5, B5 |
| [14-phase-6-work-order](14-phase-6-work-order.md) | Phase 6 work order (done): Leptos and Axum, layer L6 |
| [phase-6-results](phase-6-results.md) | Phase 6 measurements: rendering, hydration, the registry, `mf2-axum`, L6, the size gate |
| [15-phase-7-work-order](15-phase-7-work-order.md) | Phase 7 work order: islands, CSR, lazy routes, layer L7, accessibility, documentation |
| [phase-7-results](phase-7-results.md) | Phase 7 measurements: the WCAG 2.2 AA audit, spec coverage, the documentation, `mark-fallback-lang` |
| [16-phase-8-work-order](16-phase-8-work-order.md) | Phase 8 work order: migration from Fluent, XLIFF 2, the `leptos-fluent` A/B, the reader's time zone |
| [phase-8-results](phase-8-results.md) | Phase 8 record: status at exit, the A/B and oracle figures, findings carried to Phase 9 |
| [17-phase-9-work-order](17-phase-9-work-order.md) | task-level work order for the next phase: the release — 1.0.0, the specification text out of the tree, `cargo xtask release` |

---

## 1. Goals

1. **MF2 in Rust applications** — the *entire* specification: syntax, data model, errors,
   pattern selection, fallback, bidi, `u:` options, markup, format-to-parts,
   every default function, custom functions. Never a subset.
2. **Lazy-loaded locales** — a visitor downloads one locale's data, when needed,
   cached independently of the wasm. Adding a locale adds **zero** bytes of wasm.
3. **Minimal wasm** — every known technique, enforced by budgets in CI. The
   budget is the feature.
4. **Seamless, efficient Leptos integration** — one macro that works as text,
   attribute, prop or `String`; compile-time checked ids and arguments; no
   hydration mismatch; works with lazy routes and islands; tens of bytes per
   call site (B5 ≤ 40 B gz; P0.1 measured 24.5); no allocation for the common
   message.
5. **Proof, not claims** — the official `unicode-org/message-format-wg` test
   suite runs at *every* layer of the stack as each layer is built
   ([01](01-conformance.md)).

Non-goals: a translation-management system; supporting UI frameworks other than
Leptos (though `mf2-runtime` is framework-free); ICU MessageFormat 1 syntax; a
stable public binary format.

## 2. Assessment of the earlier draft plan

The draft this plan supersedes had the diagnosis right and several gaps.

**Kept** — code, not text, is the bulk of the cost and text is what scales with
locale count; parse at build time and ship a binary catalog; integer message
ids; no per-call-site closure; flattened fallbacks; content-hashed immutable
catalogs; a CI size gate; a go/no-go probe before real work.

**Changed**

| # | Gap in the draft | Resolution |
|---|---|---|
| 1 | **Subsetted MF2** ("ship the smallest function set"). | The library is full-spec. Apps stay small through *closed-world linking* (only functions the corpus uses are linked) and explicit opt-in features for costly families ([03](03-runtime.md) §3, §5). |
| 2 | **No markup, no format-to-parts.** Both are in the spec and the suite, and markup is the most valuable Leptos feature (inline elements in a sentence without baking in word order). | First-class: parts API in the runtime, `TrRich` in Leptos ([04](04-leptos-integration.md) §7). |
| 3 | **Conformance at one layer, skips by name allowed.** | Six layers, one ledger, no silent skips, tags are not a skip reason ([01](01-conformance.md)). |
| 4 | **Bound to one application** (its counts, its migration, its phases). | A deterministic synthetic *reference workload* reproduces the shape; budgets are per-unit; migration tooling is generic ([06](06-size-and-perf.md) §2). |
| 5 | **Parser = an exact-pinned alpha crate**, and the client would decode that crate's snapshot format (a CST dump with no compatibility promise). | Own `mf2-syntax` — gated on measurably matching or beating that crate — and own catalog; the alpha crate is a dev-only differential oracle ([05](05-tooling.md) §1). |
| 6 | **Call-site design ignored where call sites are.** Measured: ≈ 45 % are non-view code needing a `String`, ≈ 20 % text children (mostly wrapped in a closure only to be reactive), ≈ 20 % props and attributes, the rest registry closures and `if`/`else` choices. "A plain function returning `String`" serves the first group and loses reactivity in all the others — or brings the closures back. | One concrete `Tr` description type — `Copy`, `const`-constructible — with rendering, attribute, `From` and `to_string` paths implemented once ([04](04-leptos-integration.md) §2–3). P0.1: 24.5 B gz/site weighted by the real mix vs 97 B for the closure-per-site control; the `String` path costs ≈ 0, view positions 46–102 B gz. |
| 7 | **Argument names shipped in the wasm** (`&[(&'static str, Value)]`) and compared at runtime. | Positional slots fixed by the manifest; names live only in the lazy catalog ([05](05-tooling.md) §3). |
| 8 | **Build orchestration unspecified** — how macros find ids, invalidation, cargo-leptos' double build, multi-crate apps, rust-analyzer. | i18n crate + `build.rs` + generated `tr!` wrapper with the manifest path and hash baked in ([05](05-tooling.md) §4); verified by P0.9 under cargo-leptos, a second crate and rust-analyzer (D8). |
| 9 | **Spec obligations missing**: ordinal rules, plural operands from the *formatted* number, NFC in `:string` selection, Default Bidi Strategy, option inheritance, fallback names. | Listed and owned ([01](01-conformance.md) §6, [03](03-runtime.md) §7). |
| 10 | **"One implementation" made non-negotiable without measurement**; dates deferred indefinitely; time zone (an SSR problem under any backend) unmentioned. | Function *semantics* are implemented once in Rust; number/date *internationalization* is an opt-in client feature with stated costs; hydration was shown not to compare text ([03](03-runtime.md) §5–6). |
| 11 | **Deploy skew** between the wasm's id table and cached catalogs. | `manifest_hash` in both; mismatch ⇒ reject and reload ([02](02-catalog-format.md) F6). |
| 12 | **Catalog budget 60 KB gz/locale** — measured message text is only ≈ 43 KB raw; the rest of a source file is comments and ids. | ≤ 25 KB gz, restated in Phase 2 on brotli, the encoding actually served: ≤ 23,296 B br ([06](06-size-and-perf.md) B7). |
| 13 | **Reactive-graph hazard unnoticed**: one effect per node on a rarely-firing locale signal leaks dead subscribers under node churn. | Library-owned node registry ([04](04-leptos-integration.md) §4); P0.11 measured the leak (+72 B per churned node) and settled D7. Phase 7 A5 found it again in the conversions and in a node's argument effect (+69.8 B per churned row), closed both, and gates every row shape (`cargo xtask churn`, nightly; `leptos-mf2`'s `tests/churn.rs`, every push). |
| 14 | **Accessibility/SEO absent**: fallback-language text inside a page marked with another `lang` (WCAG 3.1.2), `hreflang`, `Content-Language`/`Vary`. | [04](04-leptos-integration.md) §9; catalog flags fallback messages. |
| 15 | **No fmt-free / panic-free discipline**, the usual hidden 10–20 KB. | Budget B12 with a symbol check in CI. |
| 16 | **Phase order** put the runtime before the things it needs (parser, catalog writer) and integration risk last. | Reordered; Phase 0 includes a throwaway vertical slice (§9). |
| 17 | Dev loop (hot reload), pseudo-locales, missing-message policy, error policy in production: unspecified. | [05](05-tooling.md) §6, [03](03-runtime.md) §8. |

## 3. Architecture

```
 authoring                 build time (native)                              runtime
 ─────────                 ───────────────────                              ───────
 locales/<tag>/*.mf2 ─▶ mf2-build                                  ┌─ server: catalogs embedded,
 third_party/cldr-json ─▶  · mf2-resource → mf2-syntax → data model│   per-request Arc<Catalog> in context,
                           · validate + lint (mf2 check)           │   mf2-axum serves /i18n/<tag>.<hash>.mf2b
                           · manifest: ids→MsgId, arg slots,       │   (br/gz, immutable) + <link rel=preload>
                             markup names, functions used, hash    │
                           · flatten fallbacks                     ├─ client wasm: NO text, NO names, NO rules;
                           · slice locale data (plural rules, …)   │   fetch preloaded catalog → validate →
                           · mf2-catalog writer → .mf2b + br/gz ───┘   thread-local; mf2-runtime formats
                           · generated Rust: MANIFEST_HASH,            from the catalog; Tr renders
                             registry(), tr! wrapper ─▶ mf2-macros ─▶ call sites: MsgId + positional args
```

Invariants: one evaluation path (from the catalog); function semantics written
once; the wasm holds no locale data; everything the wasm and a catalog must
agree on is covered by `manifest_hash`.

## 4. Monorepo layout

One Cargo workspace; every crate lives here.

```
rust-mf2/
├── Cargo.toml                  workspace: edition 2024, resolver 3, shared lints,
│                               [workspace.dependencies] at latest versions, wasm size profile
├── rust-toolchain.toml         stable + wasm32-unknown-unknown (+ wasm32-wasip1 for L4)
├── README.md · LICENSE         MIT
├── CLAUDE.md                   rules for agents (boundary, non-negotiables, conventions)
├── plans/                      this directory
├── docs/                       user docs; resource-format.md; migration guides
├── third_party/                vendored, read-only, pinned
│   ├── message-format-wg/      test/ + LICENSE + PIN (spec/ fetched into a cache; D13)
│   ├── w3c-message-resource/   resource-format draft: explainer, ABNF, data model, schema + PIN
│   ├── cldr-json/              the subset we consume + PIN
│   └── xliff/                  XLIFF 2.1 core (OASIS Standard): specification + schemas/ + PIN
├── crates/
│   ├── mf2-model/              data model, MsgId, shared error enums        no_std
│   ├── mf2-syntax/             parser, CST, lowering, validation, serializer no_std
│   ├── mf2-resource/           W3C Message Resource container: parser, serializer, data model no_std
│   ├── mf2-catalog/            .mf2b reader (client) / writer, decode (features) no_std
│   ├── mf2-runtime/            evaluator, selection, bidi, parts, registry, :string, plural eval no_std
│   ├── mf2-fn-number/          numeric family                               no_std
│   ├── mf2-fn-datetime/        date/time family; backends icu | intl
│   ├── mf2-host-web/           browser Host: normalize(), optional Intl glue
│   ├── mf2-host-std/           native Host: NFC, ICU4X pieces
│   ├── mf2-native/             native apps: one generated Corpus, app-owned locale
│   ├── mf2-ratatui/            optional Ratatui text, MF2 markup as styles
│   ├── mf2-locale-data/        CLDR JSON → LOCALE section (build side)
│   ├── mf2-build/              loaders, manifest, lints, catalogs, codegen
│   ├── mf2-macros/             tr! proc-macro
│   ├── mf2-cli/                `mf2` binary (clap)
│   ├── mf2-axum/               negotiation, catalog serving, preload headers
│   ├── leptos-mf2/             Tr types, glue/view.rs, context, hydrate entrypoints, switcher
│   └── mf2/                    facade: re-exports + feature flags apps actually touch
├── conformance/                crate mf2-conformance: L1–L7 harnesses, ledger.toml,
│                               extra/ (WG schema), goldens/, REPORT.md, COVERAGE.md;
│                               l4-runner/ (L4's client side, also wasm32-wasip1),
│                               l4-web/ (L4 in the browser for the `intl` build)
├── bench/
│   ├── workload-gen/           deterministic reference-workload generator
│   ├── corpora/                committed inputs: suite.json, workload-1600.json
│   ├── parser-gate/            the D1 gate: mf2-syntax vs ox_mf2_parser, same process
│   └── benches/                criterion + wasm timing harness
├── examples/
│   ├── demo-ssr/               cargo-leptos + Axum: 4 locales incl. RTL, lazy route, switcher, markup
│   ├── demo-csr/               trunk
│   └── demo-islands/
├── fuzz/                       cargo-fuzz: parser, catalog decoder
├── tools/e2e/                  Playwright harness (project-local npm install)
├── tools/oracle/               optional JS reference-implementation differ (dev only)
├── probes/                     Phase 0 throwaway experiments (deleted in Phase 2, C5; in the first commit)
├── xtask/                      spec-sync, cldr-sync, resource-sync, xliff-sync, conformance-report, l4-wasi, l4-web, size, gen-workload
└── .forgejo/workflows/         CI
```

| Crate | Depends on | In client wasm? |
|---|---|---|
| `mf2-model` (data model, `MsgId`, error enums, `Frontend` trait) | — | types only |
| `mf2-syntax` | `mf2-model` | **never** |
| `mf2-resource` | `mf2-model` | **never** |
| `mf2-catalog` (reader; features `writer`, `decode`, `manifest`) | `mf2-model` | reader only |
| `mf2-runtime` (incl. core numeric semantics) | `mf2-model`, `mf2-catalog`, `fixed_decimal` | **yes — budget B1** |
| `mf2-fn-number` (number localization, `:percent`, `:currency`, `:unit`) | `mf2-runtime` | feature `fn-number`, when used |
| `mf2-fn-datetime` | `mf2-runtime`; optional `icu_datetime` (`datetime-icu`). Reaches `Intl` only through the `Host` trait | feature `fn-datetime`, when used |
| `mf2-host-web` | `mf2-runtime`, `js-sys`, `web-sys` | yes (small) |
| `mf2-host-std` | `mf2-runtime`, an NFC crate | **never** (server, tests) |
| `mf2-native` | `mf2[host-std]`, `sys-locale`, `jiff[tz-system]` | **never** (native CLI/TUI) |
| `mf2-ratatui` | `mf2-native`, `ratatui-core` | **never** (optional TUI adapter) |
| `mf2-locale-data` | `mf2-model`, `mf2-catalog[writer]`; ships compact CLDR tables | **never** |
| `mf2-build` | `mf2-syntax`, `mf2-resource`, `mf2-catalog[writer,manifest]`, `mf2-locale-data`, compression crates | **never** |
| `mf2-macros` | `syn`, `quote`, `mf2-catalog[manifest]` | expansion only |
| `mf2-cli` | `mf2-build`, `clap` | **never** |
| `mf2-axum` | `axum`, `mf2-catalog` | **never** |
| `leptos-mf2` | `leptos`, `mf2-runtime`, `mf2-host-web` (client) / `mf2-host-std` (server) | yes |
| `mf2` (facade, [05](05-tooling.md) §9) | re-exports the above by feature | re-exports only |

CI asserts the "never" column with `cargo tree -e normal --target
wasm32-unknown-unknown` on the demo app.

## 5. Client feature flags (through the `mf2` facade)

| Feature | Default | Effect |
|---|---|---|
| *(core)* | — | `:string`; `:number` / `:integer` / `:offset` with complete semantics (digit options, rounding, `select` = exact / plural / ordinal) and **neutral** symbols; markup; bidi; fallback |
| `fn-number` | off | number *localization* (symbols, grouping, numbering systems) and `:percent`, `:currency`, `:unit` |
| `fn-datetime` + `datetime-icu` \| `datetime-intl` | off | date/time family; exact-but-large vs. tiny-but-host-dependent |
| `intl` *(adopted 2026-09-22)* | off | client only: numbers, plural selection and dates through the browser's `Intl` ([03](03-runtime.md) §5.3), requiring `Intl.NumberFormat` v3; the server keeps the Rust path. As built (Phase 4): +2.2 KB gz for core numbers, +0.4 with `fn-number`, −3.6 with `:currency` + `:unit`, 1.7–6× slower per numeric placeholder; L4 green in Chromium, Firefox and WebKit ([06](06-size-and-perf.md) §3) |
| `static-locale` | off | no live switching: switch = cookie + navigation; no per-node bookkeeping |
| `mark-fallback-lang` | off | wrap fallback-language text in `<span lang>` |
| `diagnostics` | off (on in dev) | readable errors, ids, spans |
| `ssr` / `hydrate` / `csr` / `islands` | — | mirror Leptos |

Features belong to the **application**, declared once on its i18n crate, and
apply to its server and wasm builds alike, so SSR output always matches what the
client would produce. A function that exists only behind a feature (`:percent`,
`:currency`, `:unit`, `:datetime`, `:date`, `:time`) and is used while that
feature is off is a **build error** naming the file and line; numbers formatted
without `fn-number` are a `check` warning (`neutral-numbers`). With a feature on,
unused handlers are still eliminated.

## 6. Budgets (summary — full table in [06](06-size-and-perf.md))

Fixed client cost ≤ 30 KB gz · ≤ 40 B gz per call site · **0** locale bytes in
the wasm · catalog ≤ 23,296 B brotli (the served encoding) for the
1,600-message reference locale, and ≤ 0.91 × (0.5 × source bytes + 1 KB) for
any locale · 0 extra round trips before
hydration · no `core::fmt` reachable from the client runtime · unused functions
absent. Reference-workload ambition: ≈ 105 KB gz total against
a measured 525 KB gz for the stack it replaces, and +0 per added locale.
Measured against `leptos-fluent` once, at migration:
[`bench/fluent-ab/SNAPSHOT.md`](../bench/fluent-ab/SNAPSHOT.md) (Phase 8 A5).

## 7. Conformance (summary — full text in [01](01-conformance.md))

Upstream pinned at `5c4ddb27` (2026-08-31; LDML 48.2 + 7 commits, 3 of them
normative); the suite vendored. 462 tests, 16 files. Since upstream #1112 (inside the
pin) the **spec text** may not be redistributed publicly without Unicode's
permission; the tests remain Unicode-3.0 — so the spec text leaves the tree and
is fetched on demand by `spec-sync` (owner, 2026-09-25; [01](01-conformance.md)
§1, [17](17-phase-9-work-order.md) A0). L1 syntax → L2 data model → L3 catalog (lossless) → L4 runtime
from the catalog, native **and** wasm → L5 macros → L6 Leptos SSR + hydrate. One
ledger; a phase cannot exit with its layer red.

## 8. Decisions

| # | Decision | Status |
|---|---|---|
| D1 | Parser: own `mf2-syntax`, **conditional on a measured gate** — at least as fast and as lean as `ox_mf2_parser` on every benchmark row, and more correct (462/462). Fails the gate at P1 exit ⇒ adopt ox behind the `Frontend` trait. ox is the dev-only oracle either way ([05](05-tooling.md) §1) | **settled: own `mf2-syntax`** — decided by owner; the P1 gate holds on every row (3.2–15.9× faster, fewer allocations, 462/462 vs 460/462; [phase-1-results](phase-1-results.md) §A9) |
| D2 | Source container: the **W3C Message Resource draft** (the MF2 editor's resource format, incubated by the W3C i18n WG; Unicode itself defines none), in new crate `mf2-resource`, behind a `Loader` trait; flat JSON import/export ([05](05-tooling.md) §2) | **decided by owner**; draft status is a tracked risk |
| D3 | Plural: own evaluator, per-locale rules baked into the catalog (0.43 KB gz vs 14.5–17.9 KB gz for `icu_plurals`; 0–84 B of data per locale) | **settled** — P0.4: 15,041/15,041 CLDR 48 samples, 224 locales, agrees with ICU4X; encoding in [02](02-catalog-format.md) §4.1 |
| D4 | Number/date internationalization on the client is opt-in via cargo features, off by default; both date backends offered. **Amendment requested by the owner (2026-09-21)**: an `intl` client option — numbers, plural selection and dates through the browser's `Intl`, Rust on the server ([03](03-runtime.md) §5.3) | **decided by owner**; the `intl` amendment **adopted (owner, 2026-09-22) as an opt-in client feature, off by default, requiring `Intl.NumberFormat` v3**, after the A0 probe measured it larger than the Rust path for core numbers (+2.2 KB gz), 2–6× slower per numeric placeholder, and bound to each engine's locale coverage ([03](03-runtime.md) §5.3; `bench/intl-probe/RESULTS.md`) |
| D5 | Fallbacks flattened at build time; fallback origin flagged per message | decided |
| D6 | One catalog per locale; `MsgId` chunk bits reserved for per-route catalogs later | decided |
| D7 | Node update strategy: library registry (B) over effect-per-node (A) | **settled: B** — P0.11: 44.8 vs 427 B per node on wasm32, flat heap vs +72 B per churned node, switch 6.8 vs 12–17 ms at 4× throttle |
| D8 | Build orchestration: i18n crate + `build.rs` + generated `tr!` wrapper (manifest path **and hash** baked in; relocation fallback; catalog names only under `ssr`) | **settled** — P0.9: correct rebuilds for every edit scenario under cargo, `cargo leptos build` and `watch`; +0.2–0.3 s per 2,000 sites; rust-analyzer expands it ([05](05-tooling.md) §4) |
| D9 | Catalog storage: client thread-local; server per-request context looked up **at render time**, so `Tr` is the same 4-byte `Copy` value everywhere. Conversions to derived reactive types (`TextProp`, `Signal<String>`) capture the request catalog under `ssr`, because third parties (leptos_meta's `<Title>`) may evaluate them outside the request owner | **verified** — P0.2: render-time lookup reached the context in all four `SsrMode`s; the narrowed capture fixed the one miss ([04](04-leptos-integration.md) §5) |
| D10 | Leptos: **0.9 is the default line, beta or not** (0.9.0-beta at 2026-09-24); 0.8 kept as an opt-in feature, built and tested in CI beside it; glue isolated in one module with the line-specific methods switched by feature (owner, 2026-09-24; replaces "latest stable, 0.9 tracked against its betas") | decided |
| D11 | License: **MIT**. Publish to crates.io late (P9) — **1.0.0**, at the end of P9, on the Leptos 0.9 beta, every crate versioned together, published by the owner with one command (owner, 2026-09-25; [17](17-phase-9-work-order.md)). Dependencies and vendored material MUST be MIT-compatible (ICU4X and the WG suite are Unicode-3.0 — fine; GPL code is excluded, including as a test oracle) | **decided by owner** |
| D12 | Crate names as in §4. The original 16 library names were checked free on 2026-09-25; `mf2-native` and `mf2-ratatui` were added later for native apps. Verify all 18 names at release; unrelated `mf2_i18n*` crates exist. | working assumption; check at release ([17](17-phase-9-work-order.md) A1, A7) |
| D13 | Spec and CLDR inputs vendored and pinned, synced by xtask; the XLIFF 2 core specification and schema likewise, for Phase 8's export and import (owner, 2026-09-24): XLIFF 2.1, the newest OASIS Standard (2.2 is a Committee Specification), `third_party/xliff/`, `cargo xtask xliff-sync` — OASIS's notice permits verbatim copies with the notice kept and forbids modifying them, which a read-only `third_party/` satisfies | decided; `spec/` itself (upstream license change #1112 — [01](01-conformance.md) §1) **leaves the tree and is fetched on demand** into a git-ignored cache; `test/` stays vendored (owner, 2026-09-25; [17](17-phase-9-work-order.md) A0) |
| D14 | Catalog is a lossless data-model encoding, not a bytecode | decided |
| D15 | Numeric digits: an **own panic-free, allocation-free digit buffer** in `mf2-runtime` instead of `fixed_decimal` 0.7 (whose six panic paths break B12), under D1's rule — `fixed_decimal` stays behind the same internal interface as the A/B baseline and the fallback ([03](03-runtime.md) §5.2) | **settled: own buffer** — decided by owner (2026-09-21); the A5b gate holds on every row (identical output on 100,000 cases, 5,142 vs 7,305 B gz, B12 clean vs a panic import, 0 vs 0.5 allocations; [phase-3-results](phase-3-results.md) §A5b) |

## 9. Phases

Each phase lists its work, the conformance layer it must turn green, and its
exit criterion. Work inside a phase is ordered; phases overlap as drawn.

```
P0 ─▶ P1 ─▶ P2 ─┬─▶ P3 ─▶ P4 ─┐
                └─▶ P5a ──────┴─▶ P5b ─▶ P6 ─▶ P7 ─▶ P8 ─▶ P9
```

P5a (build pipeline, CLI) needs only P1 + P2 and runs alongside P3 and P4.
P5b needs P5a **and P3 + P4**: its exit is "the whole suite through `tr!`", which
cannot pass without the runtime and every function. P6 needs P5b (and so
everything before it).

### P0 — Bootstrap, probes, go/no-go
* Workspace skeleton, toolchain, lint policy, release/wasm profiles, CI skeleton
  in `.forgejo/workflows`, `xtask` skeleton.
* Vendor the spec + suite, the CLDR subset and the W3C Message Resource draft
  (license permitting) with `PIN` files.
* Ledger generator: every test present, every layer `xfail until=<phase>`.
* `bench/workload-gen` (needed by the probes).
* Probes P0.1–P0.12 ([06](06-size-and-perf.md) §5) under `probes/`; results in
  `plans/phase-0-results.md`; estimates in these documents replaced by numbers.
* **Exit**: P0.1 (call-site cost) and P0.2 (vertical slice: SSR → preload →
  hydrate → switch, zero warnings, one lazy route, context reachable under
  streaming) pass; P0.9 gives correct incremental rebuilds; D3 confirmed, D7 and
  D8 settled, D9 verified; the `ox_mf2_parser` baseline (P0.12) re-measured on the
  committed corpora. If P0.1, P0.2 or P0.9 fails with no credible fix, stop and
  rethink.

### P1 — Syntax and data model · layers **L1, L2** — *done* ([phase-1-results](phase-1-results.md))
* `mf2-model`: data model types, serde JSON matching the WG schema, error enums,
  the `Frontend` trait (its public types are frozen in the Phase 1 work order).
* `mf2-syntax`: lexer/parser to lossless CST with recovery and spans; lowering;
  the six data-model validations; serializer; variable/markup/function analysis.
* ABNF-driven generator; parser fuzz target; differential oracle wired in.
* `bench/parser-gate/` (created in P0.12) gains `mf2-syntax` beside ox — the D1
  gate, run in CI from here on.
* **Exit**: L1 and L2 100 % over all 462 `src`; lossless CST and model
  round-trip properties hold on generated input; fuzz run clean; **the D1 gate
  holds on every benchmark row** (≥ ox speed, ≤ ox allocations; target ≥ 3× and
  zero allocations for placeholder-free messages). If it does not hold, switch
  the `Frontend` to `ox_mf2_parser` and carry on — the lowering is kept.

### P2 — Binary catalog · layer **L3** — *done* ([phase-2-results](phase-2-results.md))
* `mf2-catalog` writer (incl. `writer::single`, the one-message compile used by
  every later layer), reader, model-rebuilding decoder, manifest reader/writer;
  format spec frozen at exit (version 1). The LOCALE section is frozen as a
  *container* plus the two `plural.*` entries (from P0.4); other entries are
  additive in P4.
* Validation pass, skew check, fallback flags, stripping; decoder fuzz target.
* **Exit**: L3 100 % lossless; stripped ≡ unstripped formatting (checked once L4
  exists, tracked in ledger); deterministic output; budget B7 met on the
  committed reference corpus (`bench/corpora/workload-1600.json` — flat JSON, so
  no resource parser is needed yet).

### P3 — Runtime core · layer **L4** (core files) — *done* ([phase-3-results](phase-3-results.md))
* `mf2-runtime`: resolution, declarations, selection, fallback, Default Bidi
  Strategy, parts, markup, `u:` options, registry, custom-function API,
  `:string`, the **core numeric semantics** (`:number` / `:integer` / `:offset`
  over the own digit buffer, D15, neutral symbols), plural/ordinal evaluator, `Host` trait;
  `mf2-host-std`, minimal `mf2-host-web`.
* `mf2-locale-data`, **plural part only**: UTS #35 rule parser, encoder into the
  `plural.*` entries, all-locale rule tables, CLDR samples as tests.
* The `mf2` facade (re-exports, `compile_str`).
* `:test:*` functions in the harness via the public API.
* **Exit**: L4 green for every suite file except
  `functions/{percent,currency,date,time,datetime}.json` — i.e. including
  `functions/{string,number,integer,offset}.json` — and except `syntax.json`
  #90, whose French decimal comma needs Phase 4's number symbols (owner,
  2026-09-21; [01](01-conformance.md) §3); on native **and** wasm
  (`wasm32-wasip1`), byte-identical; budgets B1 (runtime part), B10, B12 met
  in a client-only probe; CLDR plural samples pass for every locale.

### P4 — Functions and locale data · layer **L4** (all files) — *done* ([phase-4-results](phase-4-results.md))
* The `intl` client option (owner request, 2026-09-21): probed first; adopted
  as an opt-in feature only if its numbers hold ([03](03-runtime.md) §5.3).
* `mf2-locale-data`, the rest: number, currency and unit tables for all locales;
  LOCALE section slicing; the additive entry kinds.
* `mf2-fn-number`: localization layer (symbols, grouping, numbering systems),
  `:percent`, `:currency`, `:unit` — with `:unit` tests written under
  `conformance/extra/functions/unit.json`, since the suite has none; `mf2-fn-datetime` with `icu` (server + optional client) and `intl`
  (client) backends; time-zone context.
* Locale-output goldens for the locale panel.
* **Exit**: L4 100 % in the **all-features** configuration; the **default**
  configuration's documented degradations recorded in the ledger; B2–B4, B8,
  B13 measured and met; goldens identical on both targets for Rust backends.

### P5a — Build pipeline and CLI — *done* ([phase-5a-results](phase-5a-results.md))
* `mf2-resource` (W3C Message Resource draft: parser, serializer, data model,
  schema-validated JSON, ABNF-driven tests) + loader; JSON loader; metadata
  (`@param`, `@do-not-translate`, comments) carried to lints and exports;
  manifest; lints; flattening;
  codegen; embedding for the server; `mf2-cli` (`init check compile fmt stats
  dump pseudo export import watch`).
* **Exit**: reference workload builds reproducibly; `check` catches seeded drift
  of every lint class; edit-one-message rebuilds only what it must.
* **Result**: met. The manifest the pipeline derives is P0.7's
  (`0x43e0dc12eeb05ef1`); the `mf2-build` ≡ `compile_str` differential is green
  on all 485 suite messages in both configurations; a translation-only edit
  leaves the client wasm byte-identical; B7 and B8 hold on the catalogs the
  build writes. Owner question 1 is answered with a supported split
  (`Emit::Module` / `Emit::Catalogs`), owner question 2 with "one catalog".
  A native application uses `Emit::Native` (catalogs embedded) or
  `Emit::NativeFiles` (shipped beside it): the native host, nothing behind
  `ssr`, and one generated `CORPUS` for `mf2-native` (Phase 9, 2026-09-27).
  cargo-leptos is not exercised — there is no Leptos application before P6 —
  and B1′/B13 are held by construction rather than by a wasm size delta.

### P5b — Macros · layer **L5**
* `mf2-macros`: `tr!` forms, diagnostics with did-you-mean, manifest cache;
  generated suite crate; `trybuild` compile-fail set.
* Needs P5a, P3 and P4.
* **Exit**: L5 100 % in both feature configurations; 2,000-site synthetic build inside B5; rust-analyzer expands
  the macro; macro overhead within P0.9's threshold.

### P6 — Leptos and Axum · layer **L6**
* `leptos-mf2`: `Tr`, `TrArgs`, `TrRich`, glue for tachys 0.2, conversions,
  registry, hydrate entrypoints (`hydrate_body`, `hydrate_lazy`), `set_locale`,
  `preload_locale`, time-zone cookie, error sinks.
* `mf2-axum`: negotiation strategies, embedded catalog service, precompression,
  cache headers, preload link/header.
* `examples/demo-ssr`; Playwright e2e.
* CI size gate switched on against the reference workload.
* **Exit**: L6 100 % (SSR text + markup structure; hydrate page with zero
  warnings; switch and back); demo renders `es`, hydrates, switches to `ar`
  (RTL) without reload; works with a `#[lazy]` route under `--split`; **all §6
  budgets met**.

### P7 — Delivery modes and hardening
* CSR (`demo-csr`), islands (`demo-islands`, including rich messages inside
  islands), `static-locale`, `mark-fallback-lang`, `<LocaleSwitcher>`, head
  helpers, the dev loop (`cargo leptos watch` sees a locale edit; *the push
  of a recompiled catalog to open pages — `mf2 watch` + an `mf2-axum` dev
  endpoint — was not built, and is deferred until after v1, owner
  2026-09-24*), Leptos 0.9 glue (against the betas until released), user documentation,
  coverage matrix complete
  ([01](01-conformance.md) §5).
* **Exit**: L6 green (SSR + hydrate, lazy routes) and **L7** green — the
  suite in an islands page and in a client-only page, each in columns of
  its own (owner, 2026-09-23; [01](01-conformance.md) §3); WCAG 2.2 AA
  audit of the demo passes; no normative spec statement without a covering
  test.

### P8 — Migration and interchange
* **First:** Leptos 0.9 made the default line, 0.8 an opt-in (D10; owner,
  2026-09-24) — [16](16-phase-8-work-order.md) §A0.
* `mf2 convert --from fluent`; a call-site migration guide (and codemod where
  mechanical) from closure-per-site macros to `tr!`; XLIFF 2 export/import,
  against the standard vendored under `third_party/` (owner, 2026-09-24).
* The reader's time zone ([03](03-runtime.md) §6's cookie and re-render
  after hydration), planned for P6 and not built there; the cookie design
  confirmed (owner, 2026-09-24).
* **The A/B against `leptos-fluent`**, measured **once**, at migration: the
  reference-workload application ([06](06-size-and-perf.md) §2), built on
  `leptos-fluent` and converted by this phase's tools (owner, 2026-09-24) —
  its client size (and the speed figures that apply) on
  `leptos-fluent` and on this library, reported with the commit it measured
  and committed as a snapshot — "at this commit, this is what we had". It
  is not re-run per commit; a later commit is audited against the snapshot
  only **when the owner asks**, by re-running the same command there
  (owner, 2026-09-24, replacing Phase 7's per-commit benchmark history).
* **Exit**: a Fluent corpus of reference-workload shape converts with a report of
  zero unmapped constructs; converted catalogs format identically to the Fluent
  originals on a sampled argument set, but for the classes of difference the
  owner approved (2026-09-24: localized numbers, bidi isolates, selection on
  a rounded decimal, current CLDR plural rules — [16](16-phase-8-work-order.md)
  §A3); the `leptos-fluent` A/B measured,
  reported and its snapshot committed; XLIFF 2 export and import validated
  against the vendored schema; dates in the reader's time zone. Work order:
  [16](16-phase-8-work-order.md).

### P9 — Release
* Name verification, API review, semver policy tied to Leptos lines, docs.rs,
  changelog, MSRV statement, release automation.
* Decided (owner, 2026-09-25): the first release is **1.0.0**, published at
  the end of this phase on the Leptos 0.9 beta; the specification text
  leaves the tree first; `cargo xtask release` checks everything and runs as
  a dry run in CI, and the publish itself is the owner's, by that command.
* **Exit**: every task of [17](17-phase-9-work-order.md) done, the release
  dry run green in CI, `current_phase = "P9"` with the harness green. Work
  order: [17](17-phase-9-work-order.md).

### Later, deliberately not now
Per-route catalog chunks (bits already reserved); dev hot reload — `mf2
watch` pushing a recompiled catalog to open pages without a rebuild (owner,
2026-09-24; Phase 7 A6 measured the rebuild loop at 4–5 s on the example);
editor tooling (tree-sitter
grammar, LSP diagnostics from `mf2-syntax`); ICU MessageFormat 1 import; server
push of catalog updates in production; catalog text as JS strings — a lazy
`JsString` cache, then possibly a v2 catalog container using the JS String
Builtins' imported string constants
([stretch_goals_after_v1/prob_builtin_strings](stretch_goals_after_v1/prob_builtin_strings.md);
v1 keeps the four seams listed there); narrower string references in the
catalog's NAMES section, up to 1.1 % of a catalog's brotli size
([stretch_goals_after_v1/names_reference_width](stretch_goals_after_v1/names_reference_width.md)).

## 10. Risks

| Risk | Mitigation |
|---|---|
| We own an MF2 evaluator's correctness. | The suite at six layers, the coverage matrix, ABNF generation, a JS reference oracle. |
| tachys trait churn (0.9 changes a signature). | All coupling in one glue module, the line-specific methods switched by feature and tracked against the betas nightly; writes delegated to tachys' own `&str` impls. |
| The prototype `Tr` type-checks but has not run in a browser. | P0.2 is an exit gate for Phase 0. |
| Own number formatter is more work than it looks (rounding priority, significant digits, grouping strategies). | `fixed_decimal` carries the arithmetic; suite + goldens define done; feature-gated so it cannot hold up core. |
| Draft functions (`:datetime` family, `:unit`) change upstream. | Pinned spec; `spec-sync` shows the diff; ledger forces the update to be explicit. |
| CLDR drift between our baked rules and a browser's `Intl` (only with `datetime-intl`). | Documented trade; hydration does not compare text; plural selection never uses the host. |
| Proc-macro reading a manifest file is fragile across tools. | P0.9 tests cargo-leptos, rust-analyzer, incremental and multi-crate builds before anything depends on it. |
| Code splitting tooling is young. | Catalog state is a plain thread-local shared by chunks; the e2e test runs under `--split`. |
| The W3C Message Resource format is an incubation draft and may change; its repo states no license. | Confined to `mf2-resource` behind the `Loader` trait; pinned; `mf2 fmt` migrates sources; license confirmed before vendoring, otherwise only a `PIN` and our own grammar. |
| Own parser turns out slower than the existing one. | A measured gate at P1 exit with an explicit fallback to `ox_mf2_parser` behind the `Frontend` trait. *Retired at P1: 3.2–15.9× faster on every row; the gate stays in CI.* |
| Scope. | Phase 0 is a genuine go/no-go; phases exit on measurable criteria only. |

## 11. Working rules

* The repository is **self-contained**. The boundary and network rule is stated
  once, in the root `CLAUDE.md` ("Boundary"). If a requirement seems missing, ask
  the owner.
* **Work orders are rolling-wave.** Each phase gets a task-level work order
  (`plans/NN-phase-N-work-order.md`) written at the close of the previous phase,
  because each phase's findings change the next one's tasks. Written so far:
  [07-phase-0-work-order](07-phase-0-work-order.md) (done),
  [08-phase-1-work-order](08-phase-1-work-order.md) (done),
  [09-phase-2-work-order](09-phase-2-work-order.md) (done),
  [10-phase-3-work-order](10-phase-3-work-order.md) (done),
  [11-phase-4-work-order](11-phase-4-work-order.md) (done),
  [12-phase-5a-work-order](12-phase-5a-work-order.md) (done),
  [13-phase-5b-work-order](13-phase-5b-work-order.md) (done),
  [14-phase-6-work-order](14-phase-6-work-order.md) (done),
  [15-phase-7-work-order](15-phase-7-work-order.md) (done),
  [16-phase-8-work-order](16-phase-8-work-order.md) and
  [17-phase-9-work-order](17-phase-9-work-order.md) (next).
* Conventions are in [05-tooling](05-tooling.md) §8.
* A change that moves a budget or a ledger status says why in its commit.
