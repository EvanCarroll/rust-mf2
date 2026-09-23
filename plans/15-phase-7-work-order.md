# 15 — Phase 7 work order: islands, CSR, lazy routes, dev loop (layer L7)

Part of the [master plan](00-master-plan.md) (§9, P7). RFC 2119 keywords
apply. Written at the close of Phase 6 from
[phase-6-results](phase-6-results.md), the Phase 0 probes P0.2 (a `#[lazy]`
route under `--split` renders `Tr` and switches live) and P0.11 (the node
registry under churn), and the prior-art audit of
[04](04-leptos-integration.md) §11 item 4.

Phase 6 made the description render, hydrate and follow the locale in the
one delivery mode that matters most (SSR + hydrate). Phase 7 makes it true
in the **other three** — islands, CSR, and code-split routes — and makes the
development loop bearable. It turns **L7** green: the same suite, in a page
whose interactive parts are islands.

## State at the start (Phase 6's exit)

| In the tree | Where |
|---|---|
| The description types and their rendering: `Render`, `RenderHtml`, `AddAnyAttr`, `ToTemplate`, `AttributeValue`, `IntoProperty`, and `From` into `TextProp` / `Signal<String>` / `Oco` / `String` | `crates/leptos-mf2/src/glue/tachys_0_2.rs` |
| The node registry (D7 = B), the catalog state, `set_locale`, `hydrate_body`, `hydrate_lazy` | `crates/leptos-mf2/src/{registry,catalog,boot}.rs` |
| Markup → elements, nesting and flat handlers | `crates/leptos-mf2/src/rich.rs` |
| Negotiation, `/i18n/*`, the per-request context | `crates/mf2-axum` |
| L6 and L6d green; L6 in two engines | `conformance/src/l6.rs`, `conformance/l6-web`, `tools/e2e/checks/l6.mjs` |
| The example, and the browser checks that drive it | `examples/demo-ssr`, `tools/e2e/checks/demo.mjs` |
| The whole-app size gate | `cargo xtask size` |

## What Phase 6 settled, and what it left

* **The client state is a `thread_local!`, not a context** ([04](04-leptos-integration.md) §5).
  That was chosen partly *for* islands (§11 item 4: the audited prior art
  cannot support them because its loader and context live in the shell), and
  Phase 7 is where the choice is either vindicated or paid for. It has not
  been tried yet.
* **`hydrate_lazy` exists and is untested.** P0.2 proved a `#[lazy]` route
  under `cargo leptos --split` renders `Tr` and switches live, with registry
  slots freed inside the chunk — but that was the probe's glue, not this
  library's. Phase 6's example is a single route.
* **A rich message inside an island needs its catalog before the island
  hydrates** ([04](04-leptos-integration.md) §7): its node *structure* comes
  from the catalog, and `hydrate_islands` cannot be gated the way
  `hydrate_body` is. This is a known work item, not a discovery to make late.
* **CSR has no boot.** `boot.rs`'s `report_boot_failure` is `hydrate`-only,
  and `catalog_url` reads a page that CSR does not server-render. §8 says
  where a CSR locale comes from (storage, `navigator.languages`) and what
  replaces the preload link (a generated `i18n/index.json`); none of it is
  built.
* **The dev loop is untouched.** A translation edit rebuilds the i18n crate
  and everything downstream of it; Phase 5a's owner question 1 (catalogs
  emitted apart, `Emit::Catalogs`) exists precisely so that it need not, and
  no measurement has been made of what the two arrangements cost a
  `cargo leptos watch`.

## Owner questions

1. **Which delivery mode is the default in the documentation.** Phase 6
   documents SSR + hydrate. Islands change the trade: server-only components
   cost **zero** wasm, which is the strongest size story this project has,
   but strategy C (`static-locale`: a switch is a cookie and a navigation)
   is their natural fit, and that is a different user experience. The
   question is which one the README leads with.
2. **Whether a translation edit may invalidate the wasm.** Phase 5a's
   `Emit::Catalogs` keeps the generated module free of catalog names so that
   an edit leaves the client untouched; Phase 6 did not use it, because the
   example is one crate. If the answer is "never", the split becomes the
   documented default and the example changes to match.
3. **How much of the CSR story to build** — **answered (owner,
   2026-09-23): build it, in this phase.** A2 is therefore in scope as
   written: a client-only application has no server to negotiate with, so
   it reads the locale from storage and `navigator.languages`, remembers
   the choice locally, and finds its catalogs through a generated index
   rather than a preload link the server wrote.

## Part A — tasks (A1–A3 in order; A4–A10 as their inputs exist)

**A1 has a draft.** An unbuilt, untested start on A1 is in the local
`git stash` as "A1 islands draft" (`git stash list`; `git stash pop` to
restore it). The owner (2026-09-23) is content for it to be reused where
it serves A1. Before reusing it, check it for three things:

* `hydrate_body` was changed to skip hydration on **any** load failure,
  not only `ManifestMismatch`. That departs from Phase 6's behaviour, so
  either justify it here and in [04](04-leptos-integration.md) §6 and
  re-run the demo's browser checks, or revert it.
* A doc link names `crate::CatalogInline`, which does not exist yet.
* The inline catalog (`<script type="application/mf2-catalog">`, base64,
  decoded with `atob`) needs a server-side writer in `mf2-axum` / the
  example, a size measurement of the page bytes it adds, and a browser
  check.

| Task | Deliverable | Done when |
|---|---|---|
| **A1** Islands | `hydrate_islands` with the catalog loaded **alongside** rather than before it (it cannot be gated), and the rule for a rich message inside an island: either the island waits for the catalog or the message is not rich. `static-locale` as the documented default for islands. | an islands build of the example renders and switches; a server-only component contributes **zero** bytes to the wasm, measured |
| **A2** CSR | The locale from storage → `navigator.languages` → default; the catalog URL from a generated `i18n/index.json` preloaded by `index.html`; `mount_to_body` with the same boot gate. | a `trunk` build of the example renders, switches and reloads into the same locale |
| **A3** Lazy routes | `hydrate_lazy` exercised by the example under `cargo leptos --split`: a route in its own chunk, rendering descriptions, switching live, and freeing its registry slots when it unmounts. | P0.2's lazy-route assertions, against this library rather than the probe's glue |
| **A4** Layer L7 | The suite in a page whose interactive parts are islands: the same 297 cases, the same twin switch, with the L7 and L7d ledger columns. | L7 green in both configurations, every L7d cell `pass` or `degraded` |
| **A5** The churn follow-up | P0.11 left one thing to Phase 6 and Phase 6 left it here: what the **conversions** (`TextProp`, `Signal<String>`, `to_string()` under an observer) cost inside a list that churns. The registry is flat under churn; a derived conversion subscribes to the locale trigger and is dropped with its component, which is the same shape as strategy A's leak. | measured under P0.11's churn, and either flat or documented with its cost |
| **A6** The dev loop | What a translation edit costs a running `cargo leptos watch`, with and without `Emit::Catalogs`; the split made the default if it wins. Owner question 2. | both numbers, and the answer in [05](05-tooling.md) §4 |
| **A7** `tachys_0_3` | Leptos 0.9's glue beside `tachys_0_2.rs`, behind a feature, when 0.9 is released; 0.9 betas tracked in CI as allowed-to-fail from now. | the 0.9 beta job runs; the module exists when 0.9 does |
| **A8** The tachys leaf hook | What P0.1 asked Phase 6 to *propose* and Phase 6 only gathered evidence for: a tachys leaf that lets a description reuse `&str`'s state and async path. Phase 6 §A7 has the case — a 197 KB gz intercept against the leanest baseline, and an application crate that takes over two hours to compile where the `String` path takes minutes, both from instantiating tachys' view machinery per site. With it, P0.1's `--cfg erase_components` figure. | the proposal written and put to the tachys maintainers, or the reason not to |
| **A9** The bidi override in a view | Phase 6 answered owner question 2 for every position and gave the `String` direction an override (`to_display_string`); a **view** position can only be overridden per request. If a call site needs it per site, `Plain<D>` is the shape ([04](04-leptos-integration.md) §9). | decided, and built if the answer is yes |
| **A10** The Phase 8 work order | Written from Phase 7's findings into `plans/16-phase-8-work-order.md`. | written |

## Exit (master plan §9, P7)

- [ ] L7 green in both configurations, every L7d cell recorded — `pass`, or
      `degraded` with its kind — none `xfail`; `current_phase = "P7"` in the
      exit commit with the harness green
- [ ] islands, CSR and lazy routes each demonstrated by the example and
      asserted by a browser check
- [ ] a server-only component's wasm cost measured at zero
- [ ] owner questions 1–3 answered and recorded in 04
- [ ] `plans/phase-7-results.md` and the Phase 8 work order written
