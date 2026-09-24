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
| *(added by A4)* L7, L7c, L7d, L7cd green in two engines | `conformance/l7-web`, `tools/e2e/checks/l7.mjs`, `cargo xtask l7-web` |
| *(added by A5)* The churn harness, every row shape flat | `bench/churn`, `tools/e2e/checks/churn.mjs`, `cargo xtask churn`; natively `crates/leptos-mf2/tests/churn.rs` |
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

1. **Which delivery mode is the default in the documentation** —
   **answered (owner, 2026-09-23): SSR + hydrate leads.** The live,
   no-reload switch is the headline and most Leptos applications are built
   that way; islands come second, presented as the smallest download, with
   `cargo xtask islands-zero`'s numbers. Recorded in 04 §8. The question
   as it was put: Phase 6
   documents SSR + hydrate. Islands change the trade: server-only components
   cost **zero** wasm, which is the strongest size story this project has,
   but strategy C (`static-locale`: a switch is a cookie and a navigation)
   is their natural fit, and that is a different user experience. The
   question is which one the README leads with.
2. **Whether a translation edit may invalidate the wasm** — **answered
   (owner, 2026-09-23): never.** Catalogs emitted apart from the module
   (`Emit::Catalogs` for a server, `mf2 compile --site` for a static host)
   become the documented default and the examples change to match, so a
   translation fix leaves the wasm — and every reader's cached copy of it —
   untouched. The extra build step must fail loudly when its features and
   the i18n crate's disagree, rather than publish catalogs the wasm rejects.
   A6 now measures the gain rather than deciding. Recorded in 04 §8 and
   05 §4. The question as it was put: Phase 5a's
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
4. **How the suite outside SSR + hydrate is recorded** — **answered
   (owner, 2026-09-23): in columns of its own**, not by widening L6's, so
   the report shows each delivery mode tested on its own. The master plan
   (§9 P7) and [01](01-conformance.md) §3 said "L6 in every delivery mode"
   and were changed with this answer. The column names are this document's:
   `L7` / `L7d` for the islands page, `L7c` / `L7cd` for the client-only
   page.
5. **What the master plan gives Phase 7 that this order had left out** —
   **answered (owner, 2026-09-23): all of it stays in Phase 7.** Added as
   A11–A15: the WCAG 2.2 AA audit and the spec-coverage check (both in the
   master plan's exit), user documentation, `mark-fallback-lang`, and
   benchmarks tracked per commit.

Still to ask when it comes up: A8 ends with a proposal *put to the tachys
maintainers*, which is outward-facing — whether an agent may post it or
only draft it for the owner to post.

## Part A — tasks (A1–A3 in order; A4–A9 and A11–A15 as their inputs exist; A10 last)

**A1, A2, A3, A4 and A5 are done** (2026-09-23); what they found is below the table.

| Task | Deliverable | Done when |
|---|---|---|
| **A1** Islands — **done** | `hydrate_islands` with the catalog loaded **alongside** rather than before it (it cannot be gated), and the rule for a rich message inside an island: either the island waits for the catalog or the message is not rich. `static-locale` as the documented default for islands. *As built: the entry point cannot be gated, but the island walk can — an empty first island that waits (below), so the island waits.* | an islands build of the example renders and switches; a server-only component contributes **zero** bytes to the wasm, measured |
| **A2** CSR — **done** | The locale from storage → `navigator.languages` → default; the catalog URL from a generated `i18n/index.json` preloaded by `index.html`; `mount_to_body` with the same boot gate. | a `trunk` build of the example renders, switches and reloads into the same locale |
| **A3** Lazy routes — **done** | `hydrate_lazy` exercised by the example under `cargo leptos --split`: a route in its own chunk, rendering descriptions, switching live, and freeing its registry slots when it unmounts. | P0.2's lazy-route assertions, against this library rather than the probe's glue |
| **A4** Layer L7 — **done** | The suite in a page whose interactive parts are islands, **and** in a client-only page: the same 297 cases, the same twin switch, with the ledger columns of owner question 4 (`L7`/`L7d` islands, `L7c`/`L7cd` client-only). The ledger checker currently *rejects* an `L7` column (`conformance/tests/ledger.rs`); that test changes with the columns. *As built: all 324 runtime-valid tests, not 297 — one page per locale the suite uses (below).* | L7 and L7c green in both configurations, every L7d and L7cd cell `pass` or `degraded` |
| **A5** The churn follow-up — **done** | P0.11 left one thing to Phase 6 and Phase 6 left it here (A3 found and fixed a leak in the same family — below): what the **conversions** (`TextProp`, `Signal<String>`, `to_string()` under an observer) cost inside a list that churns. The registry is flat under churn; a derived conversion subscribes to the locale trigger and is dropped with its component, which is the same shape as strategy A's leak. | measured under P0.11's churn, and either flat or documented with its cost *(flat, below: all three conversions and the argument effect leaked ≈ 70 B a churned row, and now leave nothing)* |
| **A6** The dev loop | What a translation edit costs a running `cargo leptos watch`, with and without `Emit::Catalogs` — and the split made the default regardless (owner question 2: a translation edit never invalidates the wasm): `demo-ssr` and `demo-islands` emit their catalogs apart, `mf2 init` scaffolds it, and a feature mismatch between the build step and the i18n crate is an error at build time. | both numbers; the examples on the split; [05](05-tooling.md) §4 updated |
| **A7** `tachys_0_3` | Leptos 0.9's glue beside `tachys_0_2.rs`, behind a feature, when 0.9 is released; 0.9 betas tracked in CI as allowed-to-fail from now. | the 0.9 beta job runs; the module exists when 0.9 does |
| **A8** The tachys leaf hook | What P0.1 asked Phase 6 to *propose* and Phase 6 only gathered evidence for: a tachys leaf that lets a description reuse `&str`'s state and async path. Phase 6 §A7 has the case — a 197 KB gz intercept against the leanest baseline, and an application crate that takes over two hours to compile where the `String` path takes minutes, both from instantiating tachys' view machinery per site. With it, P0.1's `--cfg erase_components` figure. | the proposal written and put to the tachys maintainers, or the reason not to |
| **A9** The bidi override in a view | Phase 6 answered owner question 2 for every position and gave the `String` direction an override (`to_display_string`); a **view** position can only be overridden per request. If a call site needs it per site, `Plain<D>` is the shape ([04](04-leptos-integration.md) §9). | decided, and built if the answer is yes |
| **A11** The WCAG 2.2 AA audit | The master plan's exit: every example page (`demo-ssr` both routes, `demo-islands`, `demo-csr`) in every locale, RTL included, audited against WCAG 2.2 AA — automated (an axe-style scan in `tools/e2e`) and by hand for what a scanner cannot see (focus order, `lang` of parts, the switcher with a screen reader). | the audit written, every finding fixed or recorded with its reason, the automated part a browser check |
| **A12** Spec coverage | The master plan's exit: no normative statement of the pinned spec without a covering test ([01](01-conformance.md) §5's coverage matrix, complete). A statement the WG suite does not cover gets a test in `conformance/extra/`. | the matrix complete; zero uncovered normative statements |
| **A13** User documentation | What a user needs to adopt the library, leading with SSR + hydrate and then islands (owner question 1), with catalogs emitted apart as the default (owner question 2): install, `mf2 init`, the call site, the delivery modes, the switcher, accessibility. | written, and every code sample in it compiled by CI |
| **A14** `mark-fallback-lang` | WCAG 3.1.2: text the catalog borrowed from a fallback locale renders inside `<span lang>`, identically on server and client — declared since Phase 6, doing nothing ([04](04-leptos-integration.md) §9). It changes a message's rendered *structure*, so it needs its own design before code. | designed, built, and asserted in a browser (hydration included) |
| **A15** Benchmarks per commit | The size and speed numbers of [06](06-size-and-perf.md) recorded for every commit in CI, so a regression is seen when it lands rather than at a phase exit. | the CI job runs and keeps its history |
| **A10** The Phase 8 work order | Written from Phase 7's findings into `plans/16-phase-8-work-order.md`. | written |

## A1 — islands: what was built and measured

* **The islands gate.** Leptos' island script does not await the entry
  point, but it does await an island that returns a promise, one island at a
  time in document order. `<IslandsGate/>` is such an island, first in
  `<body>`; `leptos_mf2::islands_gate!()` exports it from the application
  (this crate forbids the `unsafe` a `#[wasm_bindgen]` export expands to);
  `hydrate_islands()` sets the owner and starts the catalog load it waits
  on. So the rule for a rich message inside an island is the first
  alternative — **the island waits** — at no page bytes and no extra request.
  An earlier draft inlined the whole catalog into every page instead; it was
  dropped (04 §8 says why).
* **`static-locale` is the islands default, and was broken.** It registered
  nothing, so a signal-valued argument never re-formatted and a rebuilt
  description never rewrote its node. Now only a node with a reactive
  argument registers (the counter's line: `mf2_live_nodes() == 1` on the
  islands page), and an unregistered node's rebuild writes straight to it.
  Its switch is the `mf2_locale` cookie, the `?lang=` removed from the
  address, and a navigation. `static-locale` had never been linted: six
  dead-code warnings went with the fix.
* **A failed boot no longer hydrates**, in every mode: the page stays as
  served (04 §6). Phase 6 hydrated against no catalog, which traps on a page
  with a markup message.
* **Measured** — `cargo xtask islands-zero` (nightly, beside the size gate):
  the example's client with and without `more-server`, a server-only
  component with a call site in every position. Code section 185,925 B and
  1,019 functions in both; data 25,824 vs 25,823 B, the same constants in a
  different order; shipped 220,634 vs 220,633 B raw, 94,904 B gz both. The
  gate: identical code, data within 16 B of padding.
* **Browser checks**, `examples/demo-islands` (`tools/e2e/checks/islands.mjs`):
  56/56 in Chromium and Firefox. They include the gate holding a delayed
  catalog (no island hydrates until it lands) and the control: the same page
  with the gate removed traps on the island's markup message. WebKit was not
  run; its build is no longer installed on the development machine.
* **Found in the harness:** `page.waitForFunction` does not await an async
  predicate, so `demo.mjs`'s "hydrated" wait returned at once and passed
  only because hydration is fast. Both checks now poll with `until` from
  `lib/browser.mjs`. `demo.mjs` gained the failed-boot assertions: 100/100
  in two engines.

## A2 — CSR: what was built and measured

* **The boot.** `leptos_mf2::mount_to_body(App)` (feature `csr`) chooses
  the locale — `localStorage` `mf2_locale`, then `navigator.languages`,
  then `navigator.language`, then the source locale — loads
  `i18n/index.json` through `index.html`'s
  `<link rel=preload … data-mf2-index>`, fetches and installs that locale's
  catalog, sets `<html lang dir>`, and only then mounts. A failed boot logs
  one `mf2:` line and mounts nothing; a manifest mismatch reloads. The
  browser parses the index, so the wasm has no JSON parser.
* **One matcher.** `mf2-axum`'s RFC 4647 lookup moved into `leptos-mf2` as
  `lookup_locale`; the server's `Accept-Language` and the client's
  `navigator.languages` are matched by the same code (`fr-CA` → `fr`).
* **A switch** is live and writes `localStorage`, so a reload comes back
  in it. Under `csr` + `static-locale` a switch writes the tag and reloads
  (there is no cookie reader). Both configurations are linted by `cargo
  xtask ci` now; neither was built before.
* **Publishing.** `mf2 compile --site DIR` writes the catalogs (with `.br`
  and `.gz`) and `index.json` and nothing else (`Outcome::publish` in
  `mf2-build`). `examples/demo-csr` runs it as a trunk `post_build` hook into
  the staged site; its i18n crate emits `Emit::Module`, so the wasm names no
  catalog and a translation edit leaves it alone. The hook's `--features`
  must match the i18n crate's, and the example says so in both places.
* **Browser checks**, `tools/e2e/checks/csr.mjs` (serves `dist/` itself as a
  static host: catalogs immutable, index `no-cache`): 78/78 in Chromium and
  Firefox, on a debug build and on `trunk build --release`. It covers the
  first visit from the reader's languages (`fr-FR`, `fr-CA`, `ar-EG`,
  `de-DE` → `en`), an unknown remembered tag being ignored, one index request
  from the preload plus one catalog, the first frame in the chosen locale
  with its markup element, a live switch reaching text, attribute,
  `<title>`, a signal argument and `inLanguage` (with RTL), the choice
  surviving a reload and outranking the reader's languages, switching back
  matching a boot in that locale, two failed boots, and no message text in
  the bundle. The one console message it tolerates is Chromium's note that
  it ignores `integrity` on trunk's own wasm preload (crbug.com/981419).
  WebKit was not run (not installed).
* **Found:** trunk's release build fails with the system `wasm-opt` 120,
  which does not assume the bulk-memory ops rustc now emits;
  `index.html` passes the same feature flags `cargo xtask size` does.
* **Size, for the record — not a budget.** `trunk build --release`
  (opt-level z, fat LTO, `wasm-opt -Oz`), 2026-09-23: wasm 183,372 B raw /
  79,890 B gz; JS glue 38,007 B / 7,164 B gz (`gzip -9`). Catalogs 477 /
  563 / 714 B raw for en / fr / ar; the index 98 B.
  Boot costs two serial requests (index, then catalog) where SSR costs one;
  the index starts downloading with the wasm.

## A3 — lazy routes: what was built and measured

* **The example has two routes.** `examples/demo-ssr` now has a router: `/`
  is the Phase 6 page, and `/lazy` is a `#[lazy_route]` with a text, an
  attribute (`title`), a markup message and the current locale. The client
  boots with `leptos_mf2::hydrate_lazy(App)`. Built with `cargo leptos build
  --split`, the route is `pkg/split_…lazy_page_view….wasm`: 11,255 B raw /
  5,617 B gz in the release build (`wasm-release`), for the record — not a
  budget. `leptos-mf2` needed no change for chunks: they share the main
  module's memory and thread-locals, as P0.2 found with its own glue.
* **Found and fixed: an attribute leaked its registry slot.** `TrAttrState`
  — a description in an attribute or a property — had no `Drop`, so every
  time its element unmounted, its slot stayed live, holding the detached
  element and rewriting it on every switch. `TrState` and `TrRichState`
  free theirs; nothing before this task ever unmounted an attribute (the
  Phase 6 page is one route and its switch checks compare counts across a
  switch, not a navigation). Measured with the new check: home 17 slots →
  `/lazy` 13 → home 19, and 29 after five more round trips; with the `Drop`,
  17 → 12 → 17 → 17. The count also exposed it as the difference between a
  client-built `/lazy` (13) and a hydrated one (12). A5's churn measurement
  should include attributes in the churning rows for this reason.
* **Browser checks**, `tools/e2e/checks/lazy.mjs` against `demo-ssr`: P0.2's
  C, D, E and H groups against this library — chunk not fetched on home,
  fetched once on navigation, no message text in it; text, attribute,
  markup and locale in the chunk; a switch to RTL on the lazy route equal to
  the server's page; the page built on the way back equal to the server's;
  slots freed on leaving and flat over five round trips; a direct `/lazy`
  load hydrating with the chunk, changing no text, registering what a
  client-built one does, switching and freeing its slots; a silent console.
  Chromium and Firefox: 66/66 on the debug `--split` build; 64/64 on the
  release `--split` build, which ran before the last two assertions
  (`inLanguage`, below) were added; `demo.mjs` 100/100 on both builds,
  unchanged by the router. WebKit was not run (not installed).
* **Also changed in the example:** schema.org `inLanguage` now follows a
  switch (it read the catalog once; `demo-csr` had fixed this for itself),
  through the same helper the lazy route uses to show the locale — asserted
  by `lazy.mjs`.

## A4 — layer L7: what was built and measured

* **All 324, not 297.** 297 is the `en-US` page of L6(b); the ledger's
  runtime-valid tests are 324, and 27 of them are in `und` (22), `fr` (1)
  and `ar` (4). A column whose cells are claimed must test every one, so L7
  has **one page per locale the suite uses**. A page is in one locale at a
  time and a generated module belongs to the crate that includes it, so the
  corpus is four **set** crates (`conformance/l7-web/sets/*`, built by
  `set-build`), each with the twin `en-GB`; one wasm carries all four and
  installs the set its page names (`<html data-l7-set>`).
* **The pages.** Islands (`L7`, `L7d`): server-rendered with Leptos' own
  `HydrationScripts islands=true`, `<IslandsGate/>` first, and every call
  site an island of its own, so each hydrates separately behind the gate
  and follows a **live** switch from inside its island — "the same twin
  switch" as L6(b), which exercises the registry inside islands (the
  thread-local choice of §"What Phase 6 settled"; it held). Not
  `static-locale`: under it a switch is a navigation to a page the server
  renders in the twin, which is L6's path again, and `demo-islands`'
  browser check covers that mode. Client-only (`L7c`, `L7cd`): an empty
  body, the index preload, `mount_to_body`.
* **What is asserted, per case, in each engine:** islands — the server's
  text (read at `DOMContentLoaded`) is what the server renders alone,
  hydration changes none of it; client-only — the mounted text is what the
  server *would* render (the `l7-page` binary writes it beside the page);
  both — after the switch every case is the server's twin text, after
  switching back the first text exactly, `<html lang>` follows, the registry
  ends where it started, the console is silent. A page-level failure fails
  every case on it.
* **Judged by `cargo xtask l7-web`**, the columns' harness (they cannot run in
  `cargo test`; `BROWSER_HARNESSED` in `conformance/src/matrix.rs`, rule 7 of
  01 §4). A cell passes when L6 (or L6d) passes the test *and* every engine
  agreed on its case — a delivery mode is never greener than the render it
  delivers. In the default configuration a page holds only what the default
  build accepts (`mf2_l5_gen::Configuration::Default`), so the 68 tests L6d
  records as `build-reject` are that at L7d/L7cd too; the one
  `neutral-numbers` test (`syntax.json` #90, `fr`) is on the page and must
  agree. The ledger is held to the result by the same `verify`/`promote` as
  every other column; `--promote` rewrote it and the report.
* **Result**, Chromium and Firefox, `cargo xtask l7-web` (2026-09-23): L7
  324/324, L7c 324/324, L7d and L7cd 255/324 + 69 documented degradations —
  exactly L6d's. 16 pages per engine. Negative controls: a wrong expected
  text in a page's data fails the case in the check, and fails the promoted
  `pass` cells in the xtask with the engine, the stage and both texts.
  WebKit was not run (not installed). A nightly job (`l7-web`) runs it in
  both engines.
* **Found:** a client-only application's reader in `en-US`, where the build
  has `fr` (source) and `en-GB`, gets `en-GB` — `lookup_locale` falls back
  to a locale of the same language before the source locale, as designed
  (A2). The check therefore starts each client-only page from a remembered
  choice of the set's locale, the path the boot ranks first.
* **Found:** `conformance/l5/shared.rs`'s `date_time` named
  `mf2::fn_datetime` unconditionally, so no crate including it could be
  built without `fn-datetime`; it is now `cfg`-gated on that feature (every
  existing includer has it on). And `l6.mjs` still waited for hydration with
  an async `waitForFunction`, which returns at once (A1's finding, fixed
  there in `demo.mjs` only); it polls with `until` now.

## A5 — the conversions under churn: what was built and measured

* **The harness.** `bench/churn` is P0.11's churning list on `leptos-mf2`
  itself (a `csr` cdylib, a workspace of its own): 2,000 live rows, then
  10,000 churned rows of warm-up and 100,000 more, 50 per round under a
  round owner that is cleaned (a keyed list's shape), with a counting
  allocator. One fresh page per row shape: `text`, `attr` (text plus a
  `title`, as A3 asked), `args` (a signal-valued argument), `textprop`,
  `signal`, `to-string` (`move || tr!(…).to_string()`) and `oco` (the
  control: a value). `cargo xtask churn` builds it (release, `opt-level =
  "z"`, fat LTO), publishes the catalogs with `mf2 compile --site`, and runs
  `tools/e2e/checks/churn.mjs`: the heap may grow at most 64 KiB over the
  100,000 rows, the registry must end where it started, and after the churn
  every live row follows a switch to `fr` and back (the `Oco` rows keep
  their text, as documented). It runs nightly in the `l7-web` job.
* **Measured before the fix**, Chromium, 2026-09-23, at d22875c (load
  1.3–1.6; heap figures are load-independent):

  | Row | heap over 100,000 churned rows | released by the next locale notify |
  |---|---:|---|
  | `text`, `attr`, `oco` | +0 B | — |
  | `textprop`, `signal`, `to-string` | **+6,979,072 B (69.8 B a row), linear** | yes: 7,692,544 B, warm-up included; the notify took 4.4–7.0 ms, against 0.1 ms |
  | `args` | **+6,979,072 B (69.8 B a row), linear** | **no** |

  The conversions leaked exactly as the work order expected: strategy A's
  leak, through the consumer's effect. The `args` row was not expected. The
  work order's "the registry is flat under churn" held for P0.11's rows,
  but P0.11 left rows with a signal-valued argument out of its churn. Such a
  node's argument effect stays in the *application's* signal's subscriber
  set when the node is dropped. That signal may never change, so a locale
  switch does not release it and nothing else does either.
* **The fix, in `leptos-mf2`.**
  * `track_locale()` (client, public; `catalog.rs`) subscribes the running
    observer to the locale trigger and registers an owner cleanup that
    removes it. Every `reactive_graph` observer runs under an owner of its
    own, which is cleaned before each re-run and when it is dropped. The
    three conversions go through it (`convert.rs`, `text.rs`).
  * `changed().track()` still works, but its documentation now points to
    `track_locale()`, and both examples use `track_locale()` now.
  * The registry's argument effect is an `ArgsEffect`, whose `Drop` clears
    the effect's sources. That covers a slot freed and a slot whose
    description a rebuild replaced.
* **Measured after**, Chromium and Firefox (`cargo xtask churn`, 84/84):
  every row shape +0 B over the 100,000 rows. The trigger's notify after the
  churn takes 0.2–0.4 ms in Chromium; Firefox reports 0 ms at its timer's
  resolution. The heap figures are identical in the two engines. **The
  cost** is paid by each *live* consumer:

  | Row, live | before | after |
  |---|---:|---:|
  | `textprop` | 518.1 B, 14.0 allocations | 582.1 B, 17.0 |
  | `signal` | 550.6 B, 16.0 | 614.6 B, 19.0 |
  | `to-string` | 498.1 B, 12.0 | 546.1 B, 14.0 |
  | `args`, `text`, `attr`, `oco` | 683.9, 112.8, 154.6, 120.0 B | unchanged |

  So a live consumer costs 48–64 B more, where before every consumer that
  had been dropped cost 70 B, without bound. Firefox was measured after the
  fix only.
* **In `cargo xtask ci`**, natively (no DOM):
  `crates/leptos-mf2/tests/churn.rs`, run with `--features csr` because
  `--workspace` unifies `ssr`. It churns 20,000 consumers of each conversion
  and requires at most 16 KiB of growth. A plain-`track()` control must
  still leak: without the fix it left 3,140,864 B (157 B a row natively;
  P0.11 measured 151). If the control stops leaking, `reactive_graph`
  cleans up dropped subscribers itself and the workaround can go. The test
  also checks that a live consumer re-subscribes after every run: three
  switches, three re-runs.
  **Negative control:** with the owner cleanup removed, the test fails with
  `TextProp: 20000 churned rows left 3140864 B behind`.
* **Re-checked, because the conversions changed:** `demo.mjs` 100/100 and
  `lazy.mjs` 66/66 on demo-ssr (debug, `--split`), and `csr.mjs` 78/78 on
  demo-csr. All three ran in Chromium and Firefox. L6 and L7 were not
  re-run: their pages use no conversion, and demo-islands' `<Title>` is
  rendered by the server. WebKit was not run (not installed).

## Exit (master plan §9, P7)

- [ ] L7 and L7c green in both configurations, every L7d and L7cd cell
      recorded — `pass`, or `degraded` with its kind — none `xfail`;
      `current_phase = "P7"` in the exit commit with the harness green
      *(A4: every cell recorded, none `xfail`; the bump is the exit's)*
- [ ] the WCAG 2.2 AA audit of the examples passes (A11)
- [ ] no normative spec statement without a covering test (A12)
- [ ] user documentation (A13), `mark-fallback-lang` (A14) and per-commit
      benchmarks (A15) done
- [ ] islands, CSR and lazy routes each demonstrated by the example and
      asserted by a browser check
- [ ] a server-only component's wasm cost measured at zero
- [x] owner questions 1–5 answered (2026-09-23) and recorded in 04, 01 and the master plan
- [ ] `plans/phase-7-results.md` and the Phase 8 work order written
