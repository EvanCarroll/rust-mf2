# 14 — Phase 6 work order: Leptos (layer L6)

Part of the [master plan](00-master-plan.md) (§9, P6). RFC 2119 keywords
apply. Written at the close of Phase 5b from
[phase-5b-results](phase-5b-results.md), the Phase 0 probes P0.2 (SSR →
hydrate → switch, 67/67), P0.10 (what hydration compares) and P0.11 (the node
update strategies), and the prior-art audit of
[04](04-leptos-integration.md) §11. The design is
[04-leptos-integration](04-leptos-integration.md) §§3–9; this document orders
the work and sets the exit criteria.

Phase 6 makes the description render. `leptos-mf2` adds the rendering, the
catalog context and the reactive argument on top of the Leptos-free core the
facade already has; `mf2-axum` negotiates the locale and serves the catalogs.
It turns **L6** green — the whole suite rendered through the view type and
hydrated in a browser, in both feature configurations.

## State at the start (Phase 5b's exit)

| In the tree | Where |
|---|---|
| The call-site core: `Tr` (4 B, `Copy`, `const`), `TrArgs`, `TrRich`, `TrDyn`, `ArgValue` and the two extension traits | `crates/mf2/src/{tr,arg,dynamic}.rs`, [04](04-leptos-integration.md) §2.1 |
| `tr!` and `msg_id!`: checked against the manifest, spanned at the call site, no id string, argument name or markup name in the expansion | `crates/mf2-macros`, [05](05-tooling.md) §4 |
| L1–L5 green, L5d recorded; the suite as four i18n crates built from the vendored suite | `conformance/`, `conformance/l5/` |
| The build pipeline, the generated module, `mf2 check`, `mf2-cli` | Phase 5a |
| B5's Phase 5b half (the description and the `String` path), B1′ and B13 as gates | [phase-5b-results](phase-5b-results.md) §A6, §A10 |

## What Phase 5b settled, and what it left

* **The expansion contract is fixed** ([04](04-leptos-integration.md) §2).
  Phase 6 changes nothing about what `tr!` emits: `tr(…)`, `tr_args1…4`,
  `tr_args_n`, `tr_rich(…, [(key, markup(h))])`. What it *adds* is the
  meaning of `markup(h)` for a view closure, and `impl From<Signal<T>> for
  ArgValue` through `ArgSource`.
* **`markup(h)` needs a home.** Phase 5b's core takes anything that already
  implements `MarkupHandler`; a blanket impl for closures is impossible in
  `leptos-mf2` (a foreign trait for a bare `F`). A1 below decides how the
  facade re-exports the Leptos one without breaking the core's.
* **Handlers are found by the hash of the markup name**, in the message's
  ascending markup order (`TrRich::handler(name)`). The renderer has the name
  from the catalog at render time; the wasm never does.
* **B5's view half is not measured yet.** Phase 5b measured the description
  and the `String` path — 45 % of real sites — because without a renderer
  every template formats to a `String`. P0.1 measured the whole mix at
  **24.5 B gz** with a `Tr` leaf, and found the view positions cost 46–102 B
  gz of *tachys*, not of us. A6 below re-measures with the real leaf.

## Owner questions

1. **How the client learns another locale's hashed URL**
   ([04](04-leptos-integration.md) §6). P0.2 used `GET /i18n/<tag>` → `307`
   to the immutable URL: one extra round trip, at switch time only, nothing
   in the page. The alternative is a tag → URL map emitted into the DOM: no
   round trip, a few bytes per locale in every page. Phase 6 must measure
   both on the reference workload and put the numbers in front of the owner.
2. **Which bidi strategy applies in which position**
   ([04](04-leptos-integration.md) §9, from the prior-art audit). The
   isolating marks belong in displayed text; in a value the user or another
   program consumes as plain text — `prop:value`, the clipboard, a `String`
   handed to a server function — they are invisible junk. `BidiStrategy` is a
   `FormatContext` field, so a renderer can hold one formatter per strategy
   and pick by position at no per-call-site cost. The question is the default
   for each position, and whether a call site may override it.
3. **Which Leptos 0.8.x releases are supported**
   ([04](04-leptos-integration.md) §10). Everything was verified on 0.8.20
   and its siblings; older 0.8.x were not tested.

## Part A — tasks (A1 first; A2–A5 in order; A6–A11 as their inputs exist)

| Task | Deliverable | Done when |
|---|---|---|
| **A1** `leptos-mf2`, the crate | The crate, its feature flags, and the two extension points wired: `impl From<Signal<T>> for ArgValue` through `ArgSource` (one instance per signal *type*, not per call site), and the view-closure `markup(h)` — with the facade's re-export settled so that a rich call site compiles with the `leptos` feature and the core's `markup` stays available without it. All tachys coupling in one module (`glue/tachys_0_2.rs`). | a rich call site and a signal-valued argument compile and render; 04 §2.1's expansion table still describes what the macro emits |
| **A2** Rendering | `Render`, `RenderHtml`, `AddAnyAttr` (`no_attrs!`), `AttributeValue`, `IntoProperty` for the three description types, and the `From` conversions into `TextProp` / `Signal<String>` / `Oco<'static, str>` / `String` — each **one function in the library**, never one per call site (04 §3). `simple` messages hand tachys the borrowed `&str`; patterns format into a reused scratch `String`. | P0.2's 67 browser checks pass against the real crates |
| **A3** Hydration that cannot break the page | The cursor walked as `&str` does, the existing `Text` adopted, **no panic** on a node that is not a `Text` (stock tachys traps and the page goes inert, P0.10): one diagnostic, then degrade. | a deliberately mismatched node logs and hydrates; the wasm does not trap |
| **A4** The catalog and the locale switch | The client's `thread_local!` active catalog and notifier; the server's per-request context, with the `ssr` capture of 04 §5 (leptos_meta reads `<Title text>` outside the request owner); D7's library registry (P0.11: 44.8 B and ≈ 0 allocations per node, against 427 B for an effect per node); `set_locale` → fetch → validate against `MANIFEST_HASH` → swap → notify. **A lookup with no catalog falls back to the default locale and never panics** — the failure mode the prior-art audit found most expensive (04 §11). | a locale switch with 2,000 live nodes inside P0.11's budget; no `expect_context` on any path a call site can reach |
| **A5** `mf2-axum` | Negotiation as an ordered list of typed **sources** and **sinks** (never a boolean matrix, 04 §11): cookie, `Accept-Language`, path prefix, each a strategy behind a trait; `Content-Language` and `Vary`; `/i18n/*` served from the embedded catalogs with the precompressed variant and `immutable`; the preload link in the shell. The negotiated locale is **serialized into the page** (`<html lang>` and the preload URL) and the client trusts it rather than re-negotiating. | a request in each strategy returns the right catalog and the right headers; the client never re-negotiates |
| **A6** L6 | (a) **SSR**: every runtime-valid suite message rendered through the view type; text equals `exp`; markup rendered with the **flat recorder handler**, one marker element per markup part, whose order, kind, name and options equal `expParts`. (b) **Hydrate**: a generated page of every such message, server-rendered, hydrated headless, **zero** hydration warnings, identical `innerText` before and after; then a switch to a twin locale and back, `innerText` still equal to `exp`. Per-test `bidiIsolation` through the provider. L6 and L6d ledger columns. | L6 324/324 both configurations; every L6d cell `pass` or `degraded` with its kind, none `xfail` |
| **A7** B5, the whole mix | P0.1's method with the real leaf: the `tr` template's view positions rendering a description rather than a `String`, against `idlit` at two scales. Also P0.1's open P6 item: measure with `--cfg erase_components`, and propose the tachys leaf hook that would let a description reuse `&str`'s state and async path. | B5 ≤ 40 B gz on the full mix, or restated with the owner; the `erase_components` figure recorded |
| **A8** Accessibility and SEO | `<html lang dir>` correct and updated on switch; `mark-fallback-lang` (WCAG 3.1.2); the reference `<LocaleSwitcher>` (labelled native control, each language in its own language with its own `lang`, flexbox, external SVG); `hreflang` helpers; the schema.org `inLanguage` guidance. Owner question 2 decided and implemented. | the switcher passes an axe run at WCAG 2.2 AA; the bidi decision recorded in 04 §9 |
| **A9** The size gate | `cargo xtask size` — the whole-app ambition of 06 §3 measured end to end (fixed + per site) on the reference workload, as a CI gate rather than a one-off. | the gate runs in CI and fails on a regression |
| **A10** Examples | One example application: SSR + hydrate, a locale switch, a markup message rendering real elements, a date and a plural. Flexbox, WCAG 2.2 AA, schema.org considered, SVGs external. | it builds and runs from a clean checkout with the documented commands |
| **A11** The Phase 7 work order | Written from Phase 6's findings into `plans/15-phase-7-work-order.md`: islands, CSR, lazy routes, dev hot reload. | written |

## Exit (master plan §9, P6)

- [ ] L6 green in both configurations, SSR **and** hydrate, every L6d cell
      recorded — `pass`, or `degraded` with its kind — none `xfail`;
      `current_phase = "P6"` in the exit commit with the harness green
- [ ] the browser run is in CI (or a nightly job CI gates on), in at least
      two engines
- [ ] B5 met on the full mix, or restated with the owner
- [ ] owner questions 1–3 answered and recorded in 04
- [ ] the size gate runs in CI
- [ ] `plans/phase-6-results.md` and the Phase 7 work order written
