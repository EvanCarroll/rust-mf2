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
| The description types and their rendering: `Render`, `RenderHtml`, `AddAnyAttr`, `ToTemplate`, `AttributeValue`, `IntoProperty`, and `From` into `TextProp` / `Signal<String>` / `Oco` / `String` | `crates/leptos-mf2/src/glue/view.rs` (named `tachys_0_2.rs` until A7) |
| The node registry (D7 = B), the catalog state, `set_locale`, `hydrate_body`, `hydrate_lazy` | `crates/leptos-mf2/src/{registry,catalog,boot}.rs` |
| Markup → elements, nesting and flat handlers | `crates/leptos-mf2/src/rich.rs` |
| Negotiation, `/i18n/*`, the per-request context | `crates/mf2-axum` |
| L6 and L6d green; L6 in two engines | `conformance/src/l6.rs`, `conformance/l6-web`, `tools/e2e/checks/l6.mjs` |
| *(added by A4)* L7, L7c, L7d, L7cd green in two engines | `conformance/l7-web`, `tools/e2e/checks/l7.mjs`, `cargo xtask l7-web` |
| *(added by A5)* The churn harness, every row shape flat | `bench/churn`, `tools/e2e/checks/churn.mjs`, `cargo xtask churn`; natively `crates/leptos-mf2/tests/churn.rs` |
| *(added by A7)* The 0.9 glue (`tachys-0-3`) and its nightly beta run | `crates/leptos-mf2/src/glue/view.rs`, `cargo xtask leptos-beta`, nightly `leptos-beta` |
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
   05 §4. *Revised by question 6:* A6 measured no gain, and the wasm is
   unchanged by a translation edit in the one-crate layout too, so the
   split is the default only where it is required (client-only). The question as it was put: Phase 5a's
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
   benchmarks tracked per commit. *(The last, A15, was withdrawn by owner
   question 12.)*

6. **Whether the two-crate layout is still the default for a
   server-rendered application** — **answered (owner, 2026-09-23): no, one
   crate.** Splitting the catalogs out saves no rebuild (cargo rebuilds
   the i18n crate and the app either way) and the one-crate wasm is already
   unchanged by a translation edit; the evidence is in §"A6 — measured
   before building". Server-rendered and islands applications keep one i18n
   crate (`Emit::Both`); a client-only application keeps its catalogs apart
   (`Emit::Module` + `mf2 compile --site`), where the feature-mismatch
   check applies. A6 is rescoped accordingly. Recorded in 04 §8 and 05 §4.

7. **Whether to support the Leptos 0.9 beta before 0.9 is released** —
   **answered (owner, 2026-09-24): yes.** The line-specific code is written
   against the beta now, so the nightly beta job starts green and a new
   break in a later beta turns it red the night it lands. It is one
   glue module with the two HTML-writing methods switched by a feature, not
   a second copy of the module. Users see nothing: the feature has no
   effect on a 0.8 build and is not documented until 0.9 ships. The
   evidence is in §"A7 — measured before building". Recorded in 04 §3 and
   §10 and the master plan (D10, §9 P7, §10). The question as it was put:
   the plan said to write the 0.9 glue on release and meanwhile run an
   allowed-to-fail beta job; against today's code that job fails every
   night on the one known break and so cannot report a new one.

8. **How a translation placed straight into the page gets plain text where
   a program reads it** — **answered (owner, 2026-09-24): by the
   attribute's name.** The library leaves the isolating marks out of
   attributes a program reads (`value`, `href`, `src`, `download`, `id`,
   `name`, `data-*`, …) and keeps them in those a person reads (`title`,
   `alt`, `aria-*`, `placeholder`, …); the list is in 04 §9. A text child
   stays isolated, and `move || tr!(…).to_string()` is the documented way to
   make one plain. Recorded in 04 §9. The question as it was put: a
   description in a view position was always isolated, overridable only for
   a whole server request and not at all in the browser; the options were
   deciding by attribute name, a `plain(…)` wrapper, both, or neither with
   the closure documented.

9. **How the language switcher applies a choice** — **answered (owner,
   2026-09-24): a dropdown and a button, in a form.** `<LocaleSwitcher>`
   becomes a `<form method="get">` holding the labelled `<select
   name=LOCALE_QUERY>` and a submit button whose text the application
   supplies (a new `button` prop, a description like `label`); choosing a
   language does nothing until the button is pressed. With no client code —
   before the wasm loads, after a failed boot, or on a `static-locale`
   islands page — the form's own `GET ?lang=` is the switch, which
   `mf2-axum`'s `QueryParam` already negotiates (and the cookie follows), so
   the islands example's switcher need not be an island. Under `hydrate` and
   `csr` the submit is intercepted and becomes the live `set_locale`
   (a client-only site has no server to read `?lang=`). Readers pay one
   extra click or keypress; applications supply one more message. Recorded
   in 04 §9. The question as it was put: the switcher switched on the
   `<select>`'s `change`, which the keyboard fires on every arrow key
   (A11, below) — WCAG 3.2.2's documented failure F37; the options were
   the form, a row of language links, a warning beside the dropdown
   (G13), or leaving it and recording the failure.

10. **Whether a translation formatted to a `String` carries the isolating
    marks by default** — **answered (owner, 2026-09-24): yes, as the spec
    requires.** `to_string()` and `String::from` use the Default Bidi
    Strategy; a new `to_plain_string()` is the plain form for text a
    program consumes; `to_display_string()` stays, as a synonym. View
    positions are unchanged. Recorded in 04 §9 ("Revised (Phase 7 A12)").
    The question as it was put: A12's coverage matrix found that
    `formatting.md` makes the Default Bidi Strategy the default "when
    formatting a message as a single string", while Phase 6 (owner question
    2) had made `to_string()` plain for usability; the options were marks by
    default with a plain method, keeping it plain and recording the
    departure, or plain on the server only.

11. **Whether `mf2 check` and the build warn about non-standard or
    confusable names** — **answered (owner, 2026-09-24): build the
    warning.** `syntax.md` says linters SHOULD warn when user-chosen names
    break UAX #31 / UTS #39. A `nonstandard-name` lint, a warning by
    default, build-time only. Recorded in 05 §5. The question as it was
    put: build it, or record the SHOULD as not done.

12. **Whether size and speed are recorded for every commit** —
    **answered (owner, 2026-09-24): no; A15 is withdrawn.** No per-commit
    history is kept and no CI job is added for one. What matters is the
    A/B between this library and `leptos-fluent`, measured **once**, at
    migration (Phase 8), and committed as a snapshot with the commit it
    measured, so it can later be said "at this point, this was what we
    had" and history examined from there. A later commit is audited
    against the snapshot only when the owner asks. The existing gates
    (every push and nightly) stay as they are. Recorded in the master plan
    (§9 P7 and P8) and [06](06-size-and-perf.md) §6. The question as it
    was put: where a per-commit history would live (a data branch, git
    notes, or run attachments) and whether the half-hour whole-app size
    check would move from nightly to every push.

The question this order held back for A8 — whether an agent may post a
proposal to the tachys maintainers or only draft it — did not arise: A8
found nothing to propose (§"A8").

## Part A — tasks (A1–A3 in order; A4–A9 and A11–A15 as their inputs exist; A10 last)

**A1, A2, A3, A4, A5 and A6 are done** (2026-09-23), **A7**, **A8**, **A9**, **A11**, **A12**, **A13**, **A14** and **A10** (2026-09-24); what they found is below the table. **A15 is withdrawn** (owner question 12). **The exit is met** (2026-09-24, below; [phase-7-results](phase-7-results.md) §"Status at exit").

| Task | Deliverable | Done when |
|---|---|---|
| **A1** Islands — **done** | `hydrate_islands` with the catalog loaded **alongside** rather than before it (it cannot be gated), and the rule for a rich message inside an island: either the island waits for the catalog or the message is not rich. `static-locale` as the documented default for islands. *As built: the entry point cannot be gated, but the island walk can — an empty first island that waits (below), so the island waits.* | an islands build of the example renders and switches; a server-only component contributes **zero** bytes to the wasm, measured |
| **A2** CSR — **done** | The locale from storage → `navigator.languages` → default; the catalog URL from a generated `i18n/index.json` preloaded by `index.html`; `mount_to_body` with the same boot gate. | a `trunk` build of the example renders, switches and reloads into the same locale |
| **A3** Lazy routes — **done** | `hydrate_lazy` exercised by the example under `cargo leptos --split`: a route in its own chunk, rendering descriptions, switching live, and freeing its registry slots when it unmounts. | P0.2's lazy-route assertions, against this library rather than the probe's glue |
| **A4** Layer L7 — **done** | The suite in a page whose interactive parts are islands, **and** in a client-only page: the same 297 cases, the same twin switch, with the ledger columns of owner question 4 (`L7`/`L7d` islands, `L7c`/`L7cd` client-only). The ledger checker currently *rejects* an `L7` column (`conformance/tests/ledger.rs`); that test changes with the columns. *As built: all 324 runtime-valid tests, not 297 — one page per locale the suite uses (below).* | L7 and L7c green in both configurations, every L7d and L7cd cell `pass` or `degraded` |
| **A5** The churn follow-up — **done** | P0.11 left one thing to Phase 6 and Phase 6 left it here (A3 found and fixed a leak in the same family — below): what the **conversions** (`TextProp`, `Signal<String>`, `to_string()` under an observer) cost inside a list that churns. The registry is flat under churn; a derived conversion subscribes to the locale trigger and is dropped with its component, which is the same shape as strategy A's leak. | measured under P0.11's churn, and either flat or documented with its cost *(flat, below: all three conversions and the argument effect leaked ≈ 70 B a churned row, and now leave nothing)* |
| **A6** The dev loop — **done** | *Rescoped by owner question 6.* The two numbers (measured: below). `watch-additional-files = ["i18n/locales"]` in `demo-ssr` and `demo-islands`, which ignore a locale edit under `cargo leptos watch` today. Server-rendered apps and `mf2 init` stay on one crate (`Emit::Both`); `mf2 init` says a client-only app publishes with `mf2 compile --site`. **The mismatch check:** `mf2 compile --site` fails when its function features (`fn-number`, `fn-datetime`, `datetime-icu`) differ from those cargo resolves for the i18n crate (`cargo metadata`), and without `--features` takes cargo's — so `demo-csr`'s `Trunk.toml` no longer repeats the list. [05](05-tooling.md) §4 and §6 updated; `phase-5a-results` gets a pointer to the correction. | both numbers recorded (done); the examples watch `locales/`, asserted by an edit under `cargo leptos watch`; a mismatched `--features` fails `mf2 compile --site` with both lists named, and has a test |
| **A7** Leptos 0.9 — **done** | *Rescoped by owner question 7.* A `tachys-0-3` feature of `leptos-mf2` that switches the two `to_html_with_buf` impls (`Tr`/`TrArgs`/`TrDyn` and `TrRich`) to 0.9's `RenderFlags` form, in the existing glue module; everything else stays shared. The glue module is renamed to name no line (`glue/view.rs`), and `glue.rs`' doc says why. `cargo xtask leptos-beta`: copies the tracked tree to `target/leptos-beta`, pins the workspace's `leptos`, `tachys`, `reactive_graph`, `leptos_axum`, `leptos_meta` and `leptos_router` to the newest 0.9 / 0.3 pre-releases, then checks `leptos-mf2` for `ssr`, `csr` and `hydrate` and runs its `render` (ssr) and `churn` (csr) tests, all with `tachys-0-3`. The working tree is never edited. A nightly job, `leptos-beta`, runs it with `continue-on-error`. `tachys-0-3` with 0.8's dependencies is a compile error that names the fix. At 0.9's release, the workspace moves to it (D10: latest stable), and whether 0.8 stays supported is a question for the owner then. | the xtask passes on today's beta (and fails on it without `tachys-0-3`, as the negative control); the nightly job exists; `cargo xtask ci` green on 0.8 |
| **A8** The tachys leaf hook — **done: the reason not to** | What P0.1 asked Phase 6 to *propose* and Phase 6 only gathered evidence for: a tachys leaf that lets a description reuse `&str`'s state and async path. Phase 6 §A7 has the case — a 197 KB gz intercept against the leanest baseline, and an application crate that takes over two hours to compile where the `String` path takes minutes, both from instantiating tachys' view machinery per site. With it, P0.1's `--cfg erase_components` figure. *As found: neither half of the case survived measurement. The 197 KB was the server's host linked into the benchmark's client (a template bug, fixed); the fixed cost is 28 KB gz, the margin 11.9 B gz, and ≈ 0 under `erase_components`; the two-hour compile did not reproduce (below).* | the proposal written and put to the tachys maintainers, or the reason not to |
| **A9** The bidi override in a view — **done**, *decided by owner question 8* | Phase 6 answered owner question 2 for every position and gave the `String` direction an override (`to_display_string`); a **view** position could only be overridden per request. *Decided: an attribute's strategy follows its name* ([04](04-leptos-integration.md) §9's second table). `AttributeValue`'s `to_html`, `build` and `hydrate` and the registry's `Target::Attribute` rewrite on a switch all choose `TextUse` from the key through **one** function (ASCII case-insensitive; `data-` by prefix); `IntoProperty`, text children and markup are unchanged. The `TextUse` and `Target` docs and the `with_bidi` doc say so. The closure form for a plain text child goes in the user documentation (A13). | a native `ssr` render test: a message with an argument in `value=` and `data-x=` has no U+2066–U+2069, in `title=` it has them, and the name match ignores case; a browser check on an example page asserts the same after hydration **and** after a live switch (the registry path); negative control: with the rule forced to `Displayed`, both fail; `cargo xtask size` passes, with the B1 change recorded |
| **A11** The WCAG 2.2 AA audit — **done** | The master plan's exit: every example page (`demo-ssr` both routes, `demo-islands`, `demo-csr`) in every locale, RTL included, audited against WCAG 2.2 AA — automated (an axe-style scan in `tools/e2e`) and by hand for what a scanner cannot see (focus order, `lang` of parts, the switcher with a screen reader). | the audit written, every finding fixed or recorded with its reason, the automated part a browser check. *Measured before building (below); what remains:* the switcher of owner question 9 (with a render test and browser assertions that an arrow key changes nothing, the button switches — live under `hydrate`/`csr`, by navigation with the wasm blocked — and that no fixed `id` is left); the example fixes the section lists; `tools/e2e/checks/a11y.mjs` asserting the scan, the contrast figures, reflow, and the switcher's keyboard behaviour in two engines, each with a negative control; the written audit in `plans/phase-7-results.md` |
| **A12** Spec coverage — **done** (§"A12" below; [phase-7-results](phase-7-results.md) §A12) | The master plan's exit: no normative statement of the pinned spec without a covering test ([01](01-conformance.md) §5's coverage matrix, complete). A statement the WG suite does not cover gets a test in `conformance/extra/`. *Added by owner questions 10 and 11:* `to_string()` / `String::from` isolated by default and a new `to_plain_string()` (04 §9); the `nonstandard-name` lint (05 §5). | the matrix complete; zero uncovered normative statements; `cargo xtask conformance-report` checks it (and writes `COVERAGE.md`); the two additions built, each with a test and a negative control |
| **A13** User documentation — **done** (§"A13" below) | What a user needs to adopt the library, leading with SSR + hydrate and then islands (owner question 1), with one i18n crate for server-rendered apps and catalogs published apart for client-only ones (owner questions 2 and 6): install, `mf2 init`, the call site, the delivery modes, the switcher, accessibility. | written, and every code sample in it compiled by CI |
| **A14** `mark-fallback-lang` — **done** (§"A14 — design" below; [phase-7-results](phase-7-results.md) §A14) | WCAG 3.1.2: text the catalog borrowed from a fallback locale renders inside `<span lang>`, identically on server and client — declared since Phase 6, doing nothing ([04](04-leptos-integration.md) §9). It changes a message's rendered *structure*, so it needs its own design before code. | designed (done), built, and asserted in a browser (hydration included) |
| **A15** Benchmarks per commit — **withdrawn** (owner question 12) | ~~The size and speed numbers of [06](06-size-and-perf.md) recorded for every commit in CI.~~ Replaced by the `leptos-fluent` A/B, measured once at migration and committed as a snapshot (master plan §9 P8); A10 carries it into the Phase 8 work order. | — |
| **A10** The Phase 8 work order — **done** ([16](16-phase-8-work-order.md)) | Written from Phase 7's findings into `plans/16-phase-8-work-order.md`, including the `leptos-fluent` A/B of owner question 12. *Four owner questions came up writing it and are answered there (2026-09-24): dev hot reload deferred until after v1, the time-zone cookie built, XLIFF 2 vendored, the A/B on the reference application.* | written |

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

## A6 — measured before building: the split saves nothing under cargo

**Owner question 6 is answered (below): one crate stays the default for
server-rendered apps.** Nothing was built for A6 before the answer; what
follows is what was observed, and the A6 row says what remains.

* **The recompile happens either way.** In `examples/demo-csr`, whose i18n
  crate already emits `Emit::Module`, a one-line edit to a French text,
  then `cargo build -v --target wasm32-unknown-unknown`: `Dirty
  demo-csr-i18n: the file i18n/locales has changed`, the i18n crate
  recompiled, then `Dirty demo-csr: the dependency demo-csr-i18n was
  rebuilt`. The build script has to rerun to see the edit, and cargo
  rebuilds a crate whose build script reran, and everything above it,
  whatever the script wrote (no early cut-off — P0.9 said so for the
  one-crate layout). Phase 5a's "cargo recompiles neither it nor anything
  above it" (05 §4, phase-5a-results owner question 1) was reasoned from the
  unchanged `OUT_DIR` and never timed; it is wrong.
* **Both numbers**, `examples/demo-ssr` under `cargo leptos watch`
  (debug, `CARGO_BUILD_JOBS=2`, load 1.2–2.8), from writing the edit to the
  restarted server listening; runs alternated one-crate, split, one-crate:

  | Layout | four edits (s) | recompiled per edit |
  |---|---|---|
  | one crate (`Emit::Both`), run 1 | 3.47, 3.12, 2.91, 2.93 | `demo-i18n` ×2, `demo-ssr` ×2 |
  | catalogs apart (`Module` + `Catalogs`) | 4.79, 3.78, 3.68, 2.95 | `demo-i18n` ×2, `demo-ssr` ×2, `demo-catalogs` ×1 |
  | one crate, run 2 | 2.97, 2.93, 2.91, 2.81 | `demo-i18n` ×2, `demo-ssr` ×2 |

  Every edit was served (`curl '/?lang=fr'`). The split did the same work
  plus one crate. The split layout was a temporary change (a `catalogs/`
  crate, `Emit::Module` in `i18n/build.rs`, `demo_catalogs::CATALOGS` in
  `main.rs`), reverted. Script: a `cargo leptos watch` under `ts`, a `sed`
  on `reset = …` in `fr/main.mf2`, waiting for the next `listening on`.
* **The wasm was byte-identical in every run and in both layouts** —
  `demo_ssr.wasm` sha256 `faea698c1dea…` before and after all twelve
  edits, one-crate and split alike. The one-crate module already puts every
  catalog name behind `ssr` (P0.9 found the same), so a translation edit
  never changed the wasm there either: owner question 2's goal holds
  without the split, for an application with a server.
* **Also found:** neither `demo-ssr` nor `demo-islands` sets
  `watch-additional-files`, so `cargo leptos watch` ignores a locale edit in
  both (it had to be added for the measurement). That needs fixing whatever
  the answer.
* Where the split *is* needed: a client-only application, which has no
  server to embed catalogs in (`demo-csr`, `mf2 compile --site`), and so
  where the feature-mismatch check owner question 2 asked for matters.

**Owner question 6 — answered (owner, 2026-09-23): no.** One crate stays
the default for server-rendered and islands apps; the two-crate layout
stays where it is required (client-only). The alternative that would really
cut the rebuild — the i18n crate not rerunning on a translation edit at all
— needs the functions a translation may use to be known without reading the
translations; that is a design change, not part of A6, and not scheduled.

## A6 — the dev loop: what was built and measured

* **The examples watch `locales/`.** `demo-ssr` and `demo-islands` set
  `watch-additional-files = ["i18n/locales"]`. Asserted by
  `tools/watch-edit.sh EXAMPLE PORT` (2026-09-23): it starts `cargo leptos
  watch`, edits fr's `reset` text, requires the restarted server to serve
  the edit (`?lang=fr`), then the revert. demo-ssr: served 4.0 s after the
  edit; demo-islands: 5.1 s (debug, `CARGO_BUILD_JOBS=2`, under load — the
  first run also rebuilt what the new `mf2-build` touched). The control,
  demo-ssr with the line removed (`… expect-ignored`): no rebuild in 90 s
  after the same edit. Not in `cargo xtask ci`; it needs a full cargo-leptos
  build of each example.
* **`mf2 compile --site` takes the i18n crate's features from cargo.** It
  runs `cargo metadata` on the crate in `-C DIR` and builds the catalogs for
  the features the resolve gives it. `--features`, if given, must agree on
  `fn-number`, `fn-datetime` and `datetime-icu` (`mf2_build::CATALOG_FEATURES`
  — the only ones `mf2-build` reads for a catalog); otherwise it fails with
  both lists and the crate's name, before writing anything:
  `--features names [fn-datetime, fn-number] but cargo resolves [fn-number]
  for demo-csr-i18n; …`. A `DIR` that is not a cargo package fails too.
  `mf2 compile` without `--site` is unchanged. Test:
  `compile_site_builds_for_the_i18n_crates_features_and_rejects_others`
  (`crates/mf2-cli/tests/commands.rs`); negative control, the comparison
  disabled: the test fails at the mismatch assertion.
* **The list is written once.** `demo-csr`'s `Trunk.toml` hook and `cargo
  xtask churn` no longer pass `--features`. `trunk build` of demo-csr
  published the same catalogs as before (`fr.87bf7771890c4b05.mf2b`, …) and
  `csr.mjs` passed 78/78 in Chromium and Firefox; the churn harness's
  catalogs, published with and without `--features fn-number`, are
  byte-identical.
* **`mf2 init`** prints a fourth step for a client-only application:
  `Emit::Module` in `build.rs`, and `mf2 -C <dir> compile --site
  <site>/i18n`. 05 §4 had said it *writes* `watch-additional-files`; it
  prints it (the application's manifest is not its to edit), and 05 now
  says so.
* **Plans.** 05 §4 and §6 updated; `phase-5a-results` owner question 1
  points to the correction.

## A7 — measured before building: the beta breaks two methods

Observed 2026-09-24. Nothing was built for A7 before owner question 7 was
answered; the A7 row says what remains.

* **What is published.** crates.io: `leptos` 0.9.0-beta and `tachys` /
  `reactive_graph` 0.3.0-beta2 (2026-07-18), `leptos_axum` / `leptos_meta`
  0.9.0-beta, `leptos_router` 0.9.0-beta1 (2026-07-21). No release since. The
  latest 0.8 line is still what the workspace pins.
* **The workspace resolves on the beta.** In a scratch copy of the tracked
  tree, those six pins rewritten to the betas: `cargo check -p leptos-mf2`
  fails for `ssr`, `csr` and `hydrate` alike with the same eight errors,
  all in the glue: the two `to_html_with_buf` impls (`render_description!`
  and `TrRich`) have 0.2's parameters `escape: bool, mark_branches: bool`
  where 0.3 has `flags: RenderFlags`. That is the break 04 §1 predicted, and
  the only one: the registry, the catalog state, the boot, the conversions
  and every `reactive_graph` use compile unchanged.
* **With those two methods adapted** (the parameter replaced and passed
  through to tachys' own `&str` / fragment impls), all three targets
  check, `tests/render.rs` passes 12/12 under `ssr` (12/12 on 0.8 too), and
  `tests/churn.rs` passes under `csr`. The churn test's plain-`track()`
  control still leaks on `reactive_graph` 0.3.0-beta2, so A5's workaround is
  still needed there.
* Not tried on the beta: the examples, `mf2-axum`, the browser checks.

## A7 — Leptos 0.9: what was built and measured

* **One glue module, two forms of one method.** `glue/tachys_0_2.rs` is now
  `glue/view.rs`; `glue.rs` says why it names no line. The `tachys-0-3`
  feature (`= ["leptos"]`) switches the two `to_html_with_buf` impls — the
  `render_description!` one (`Tr`, `TrArgs`, `TrDyn`) and `TrRich`'s — to
  0.3's `flags: RenderFlags`, passed through to tachys' own `&str` and
  fragment impls as the 0.2 form passes `escape` and `mark_branches`. Nothing
  else in the crate is `cfg`'d on it.
* **On Leptos 0.8 it is a compile error that names the fix.** The first
  error is always the `RenderFlags` import (rustc continues to type-check and
  reports eight more), and rustc prints that import's line, so the line
  carries the fix: ``use tachys::view::RenderFlags; // `tachys-0-3` needs
  Leptos 0.9; for Leptos 0.8, turn it off``. A custom
  `#[diagnostic::on_unimplemented]` trait on the method's arity was tried
  first and dropped: rustc reports a closure-arity mismatch as E0593 and
  ignores the custom message.
* **`cargo xtask leptos-beta`** (`xtask/src/leptos_beta.rs`): (1) in the
  working tree, `tachys-0-3` with `ssr` must fail and its first error must
  contain that sentence; (2) every tracked file, as it is in the working
  tree, copied to `target/leptos-beta/tree`, the six workspace pins
  rewritten to `^0.9.0-alpha` / `^0.3.0-alpha` so cargo picks the newest
  pre-release, no lock file, builds in `target/leptos-beta/target`; (3)
  `leptos-mf2` checked for `ssr` natively and for `hydrate` and `csr` on
  `wasm32-unknown-unknown`, then `render` (`ssr`) and `churn` (`csr`), all
  with `tachys-0-3`. It prints what each pin resolved to, and says so when
  one resolves to a release (the cue to move the workspace, D10).
  `--no-tachys-0-3` is the negative control. The working tree is never
  written; `git status` after both runs showed only this change.
* **Result, 2026-09-24**, resolved to leptos 0.9.0-beta, tachys and
  reactive_graph 0.3.0-beta2, leptos_axum and leptos_meta 0.9.0-beta,
  leptos_router 0.9.0-beta1: all three checks pass, `render` 12/12, `churn`
  1/1 (so its plain-`track()` control still leaks on 0.3 — A5's workaround
  is still needed). **Negative control**, `--no-tachys-0-3`: the `ssr` check
  fails with the eight errors the measurement before building found (four
  E0050, four E0061), all in the two methods.
* **The nightly job** `leptos-beta` runs the xtask, its step
  `continue-on-error`, with the wasm32 target installed.
* **`cargo xtask ci`** on Leptos 0.8: green, every step (2026-09-24). It does not
  build `tachys-0-3`; that is the nightly's.
* Not tried on the beta, as before: the examples, `mf2-axum`, the browser
  checks.

## A8 — the tachys leaf hook: the reason not to

Measured 2026-09-24, before anything was designed. Phase 6's case for the
hook had two halves, and neither survived.

* **The 197 KB gz intercept was the server's host in the client.** The
  `tr-view` template depended on the benchmark's i18n crate
  (`templates/tr/i18n`) with default features, and that crate's default is
  `ssr`. With `ssr` on, the generated module's `host::HOST` is
  `host_std::HOST`, and `tr-view`'s boot names it — so the `hydrate` client
  linked jiff with its bundled time-zone database, Unicode normalisation
  and `std::io`'s error strings. `twiggy diff` of a 120-site pair
  (`idlit-view` → `tr-view`, names kept): four data segments of 43,147,
  42,452, 31,211 and 17,129 B found only in `tr-view`, 11,004 B of
  `jiff::tz` code, and the zone-name table (`Africa/Abidjan…`, 8,504 B) in
  the wasm; `cargo tree -i jiff` on the client names the path. No message
  text: the canary is absent from that wasm. The `tr` template has the same
  declaration but picks its host by the app's own feature, so it linked
  nothing extra.
* **The fix:** `default-features = false` on the i18n crate in both
  templates. Nothing in the examples or `mf2 init` has the slip — no
  example's i18n crate defaults to `ssr`. For `tr` it is a no-op, measured:
  `cargo xtask size` passes (B1 22,108 B gz, B5 12.6, whole app 45,519),
  and the 1,860-site `tr` client built with and without the line has the
  same length and is 2 B gz apart. (Against the last recorded run those
  figures are +0.2 KB; the line is not the cause, and the commits since
  Phase 6 were not bisected.)
* **Refuted on the way:** that the intercept came from the *variety* of leaf
  types (`Tr` and `TrArgs` making more distinct block types than `&str`
  does). At 465 sites, every no-argument view leaf converted to `TrArgs`:
  445,999 → 445,106 B gz, −0.2 %. The size was a fixed cost from the start:
  tr-view − idlit-view was 196.9, 199.8, 205.1 and 215.0 KB gz at 120, 465,
  930 and 1,860 sites; with the fix, 31.0, 33.5, 38.6 and 49.4 KB. (These
  ad-hoc figures are `gzip -9` of the file and are compared only with each
  other; the xtask's gzip differs by ≈ 2–3 KB on the same file.)
* **B5 re-measured, `cargo xtask b5 --view`** (1,860 / 3,720 sites, fixed
  template), 17 minutes for all six builds — Phase 6's run took about
  twelve hours:

  | workload | template | bindgen gz | opt raw | opt gz |
  |---|---|---:|---:|---:|
  | 1,860 sites | tr-view | 580,969 | 2,071,085 | 600,670 |
  | 1,860 sites | idlit-view | 527,540 | 2,012,739 | 550,187 |
  | 1,860 sites | dummy | 627,842 | 2,443,752 | 643,780 |
  | 3,720 sites | tr-view | 971,681 | 3,739,896 | 1,006,726 |
  | 3,720 sites | idlit-view | 895,064 | 3,689,170 | 934,183 |
  | 3,720 sites | dummy | 1,090,396 | 4,553,690 | 1,130,013 |

  | baseline | marginal B gz/site | fixed B gz | budget |
  |---|---:|---:|---|
  | **`idlit-view`** | **11.9** | **28,423** | ≤ 40 — met |
  | `dummy` | −43.1 | 37,067 | not a bound for this pair (phase-6-results §A7) |

  The baseline reproduces Phase 6's to the byte at 1,860 sites (550,187).
  The margin moves 11.4 → 11.9 because the fix took 167,864 B gz off
  tr-view at 1,860 sites and 166,920 at 3,720 (Phase 6: 768,534 and
  1,173,646) — 944 B over 1,860 sites. The fixed cost is now of B1's
  order (22.1 KB on the `String` path, plus the view glue), not ten times
  it.
* **The compile time did not reproduce.** The app crate alone (touched and
  rebuilt; fat LTO, `CARGO_BUILD_JOBS=3`, one run each, load 1.9–5.6):

  | sites | idlit-view | tr-view, Phase 6's template | tr-view, fixed |
  |---:|---:|---:|---:|
  | 120 | 3.8 s | 4.2 s | 4.4 s |
  | 465 | 8.7 s | 9.7 s | 10.3 s |
  | 930 | 17.4 s | 18.9 s | 30.8 s (load 5.6) |
  | 1,860 | 29.3 s | 37.5 s | 43.8 s |

  Linear in the sites, and within 1.5× of the `&str` baseline under load —
  not two hours. Phase 6's tr-view wasm reproduces within 403 B raw
  (2,461,523 against 2,461,120), so it was the same build; why it took hours
  then was not established, and nothing here depends on it.
* **`--cfg erase_components`** (P0.1's figure), `RUSTFLAGS="--cfg
  erase_components" cargo xtask b5 --view --out …` (fixed template; 337 s
  for the six builds):

  | workload | template | bindgen gz | opt raw | opt gz |
  |---|---|---:|---:|---:|
  | 1,860 sites | tr-view | 236,325 | 701,562 | 244,007 |
  | 1,860 sites | idlit-view | 200,218 | 629,702 | 211,499 |
  | 1,860 sites | dummy | 198,367 | 679,068 | 210,687 |
  | 3,720 sites | tr-view | 301,627 | 972,373 | 307,272 |
  | 3,720 sites | idlit-view | 259,890 | 901,651 | 274,255 |
  | 3,720 sites | dummy | 252,901 | 997,247 | 269,844 |

  | baseline | marginal B gz/site | fixed B gz |
  |---|---:|---:|
  | **`idlit-view`** | **0.3** | 31,999 |
  | `dummy` | 2.2 | 29,212 |

  With components erased, a description in a view position costs what a
  `&str` does, and every application is 59–76 % smaller (tr-view at 1,860
  sites: 600,670 → 244,007 B gz). So what is left of the 11.9 B lives in
  the typed (non-erased) view path, and Leptos' own switch removes it.
* **Why no proposal.** A leaf hook could remove at most part of the 11.9 B
  gz a site costs on the whole mix — the formatting and the registry are
  ours whatever tachys offers — against a budget of 40; the compile time it
  was to cut is not there; and under `erase_components` a description
  already costs what a `&str` does (0.3 B gz). There is nothing measured
  to put to the tachys maintainers, so nothing is put. The nightly `b5` job keeps measuring
  the view margin, and the workspace's move to Leptos 0.9 (A7) will show
  any change tachys 0.3 makes to it.
* **Also fixed:** `cargo xtask b5 --out` with a relative path failed in the
  i18n crate's build script, which runs in another directory; the path is
  made absolute first.
* **Corrected elsewhere:** phase-6-results §A7 (a pointer here), 04 §1, 06
  §3 (B5's row).

## A9 — an attribute's bidi strategy by its name: what was built and measured

* **The rule** is `state::attribute_use(key)` in `leptos-mf2`: plain for
  the sixteen names of 04 §9's second table and any `data-*`, isolated for
  every other name, ASCII case-insensitive. It is asked in all four places
  an attribute's text is made — `AttributeValue::to_html` (the server),
  `build` and `hydrate` (the client), and `Target::text_use` (the
  registry's rewrite on a switch, and the unregistered rebuild under
  `static-locale`). `IntoProperty` stays plain, text children and markup
  stay isolated. The docs of `TextUse`, `Target::Attribute` and
  `RequestI18n::with_bidi` say so.
* **Test**, `crates/leptos-mf2/tests/render.rs`
  `an_attribute_is_isolated_or_plain_by_its_name` (`--features ssr`, 13/13):
  `Hello, {$name}!` is plain in `value`, `data-x`, `href`, `download`,
  `VALUE`, `Data-X`, and isolated in `title`, `aria-label`, `placeholder`,
  `alt`, and in `database` and `values` (a near miss is not a match).
* **The example.** demo-ssr's home page has a prefilled "Message" field
  (`message-label`, added in en/fr/ar) whose `value=`, `title=` and
  `data-greeting=` are the corpus's `greeting` with a name, which was in the
  catalog and unused. `tools/e2e/checks/demo.mjs` asserts, at four stages —
  the server's HTML, as hydrated, after a live switch to `fr` (written by
  the registry), and after the switch to `ar` (a Latin name in an RTL
  sentence) — that `value` has no U+2066–U+2069, `data-greeting` equals it,
  `title` has U+2068 `Ada` U+2069, and the two differ only by the marks.
  Chromium and Firefox, debug `--split` build, 2026-09-24: `demo.mjs`
  134/134 (was 100: 17 new assertions per engine), `lazy.mjs` 66/66.
  WebKit was not run (not installed).
* **Negative controls**, `attribute_use` forced to `Displayed`: the render
  test fails at `value= is plain`; `demo.mjs` fails 118/134 — the `value`
  and differ-only-by-the-marks assertions at all four stages, in both
  engines (the `data` one passes because it compares with `value`, and
  `title` is isolated either way).
* **Size.** `cargo xtask size` passes unchanged to the byte (B1 22,108 B gz,
  B5 12.6, whole app 45,519, as at A8). That gate's `tr` template formats
  every site to a `String`, so the attribute glue is not in it; the rule's
  cost is in the view path: `cargo xtask b5 --view`, against A8's run
  of the previous commit (the `idlit-view` and `dummy` baselines reproduce
  it to the byte, so the difference is this change): `tr-view` at 1,860
  sites 600,670 → 601,079 B gz (+409; raw 2,071,085 → 2,071,615, +530), at
  3,720 sites 1,006,726 → 1,007,111 (+385). **Fixed 28,423 → 28,856 B gz
  (+433), marginal 11.9 → 11.8 B gz a site** — a fixed cost, nothing per
  call site, within the ≤ 40 budget.
* `cargo xtask ci` green (2026-09-24).

## A11 — measured before building: what a scanner sees, and what it does not

Observed 2026-09-24 at 33a9621, all three examples rebuilt (`demo-ssr`
with `--split`, debug), Chromium and Firefox. Nothing was changed in the
tree; the draft scan was not kept.

* **The scan is clean.** axe-core 4.13.0 (npm, pinned; MPL-2.0, a test
  dependency of `tools/e2e` only), tags `wcag2a`, `wcag2aa`, `wcag21a`,
  `wcag21aa`, `wcag22a`, `wcag22aa`, plus `best-practice` recorded
  separately, over demo-ssr `/` and `/lazy`, demo-islands `/` and demo-csr
  `/` (served from `dist/`, the locale from `localStorage`), each in `en`,
  `fr` and `ar`, light and dark (`colorScheme`), and demo-ssr after a live
  switch to `ar` and a client navigation to `/lazy`: **0 violations, 0
  incomplete, 0 best-practice** on all 28 pages in both engines, and no
  horizontal scroll at 320 CSS px (1.4.10). Control: the same page with
  `<html lang>` removed and body text `#ddd` reports `html-has-lang` and
  `color-contrast`, so the scan runs. (axe-core was not in the harness
  before; Phase 6's "the switcher passes an axe run" has no run recorded.)
* **Found by hand — the switcher (library): WCAG 3.2.2, F37.** A focused,
  closed `<select>` changes value on ArrowDown and fires `change`, in both
  engines. demo-ssr: one ArrowDown switched the page `en` → `fr` live,
  focus kept. demo-islands: one ArrowDown reloaded the page in `fr`, focus
  on `<body>`. A keyboard reader cannot reach `ar` from `en` without passing
  through `fr`. Owner question 9 decides the fix. Also in the component: a
  fixed `id="mf2-locale"`, so two switchers on one page (header and
  footer) duplicate an id and break the second `<label for>`.
* **Found by hand — the examples' CSS (1.4.11, 1.4.3).** Input and select
  borders `--line` against the page: 1.44:1 light, 1.58:1 dark (3:1
  needed). Placeholder text, the engines' defaults: Chromium `#757575`,
  4.61:1 light and **3.93:1 dark**; Firefox 54 % of the text colour, ≈ 3.95:1
  light and ≈ 5.4:1 dark (4.5:1 needed). Buttons 6.77:1 and 8.47:1, focus
  ring the accent colour with a 2 px offset on the page — pass.
* **Found by hand — the examples' structure.**
  * Landmarks: `<header>`, `<nav>` and `<footer>` are inside `<main>`, so
    there is no banner or contentinfo and "skip to main" lands on the
    header (1.3.1, 2.4.1 by landmarks) — all three examples.
  * demo-ssr's echo field is labelled "Search", the same as the search
    field above it, and is not a search (2.4.6).
  * demo-ssr's `/` and `/lazy` have the same `<title>` (2.4.2); a client
    navigation changes neither title nor focus.
  * The counter's "N people are here" changes on a button press with focus
    on the button and nothing announces it (4.1.3; `role="status"`).
  * demo-csr's `index.html` title is `mf2` until the boot, and stays so, over
    an empty body, when the boot fails — recorded, not a 2.2 AA failure of a
    page with content.
* **Checked and passing.** Focus order follows the visual order in `ar`
  (right to left) and `en`; focus stays on the switcher across a live
  switch; the focus ring is visible on every control; text spacing (1.4.12:
  line height 1.5, letter 0.12 em, word 0.16 em, paragraph 2 em) at 320 px
  clips nothing and scrolls nothing; target size passes (axe); the
  accessibility tree names every control, and each option has its own
  `lang`. Latin inside Arabic text — "Leptos", "wasm", "Esc", "Ada" — is
  proper names and technical terms, which 3.1.2 exempts.
* **Not done: a screen reader.** None is installed on the development
  machine (`orca`, `espeak-ng` absent), and installing one is outside the
  network this repository may use. The accessibility tree (Playwright's
  `ariaSnapshot`) stands in for it; that the voice changes on an option's
  `lang` is not verified.

## A11 — the audit: what was built and measured

The written audit — every finding, what became of it, and what was not
done — is [phase-7-results](phase-7-results.md) §A11. In short:

* **The switcher (owner question 9).** `<LocaleSwitcher>` is a `<form
  method="get">`: the `<select name="lang">` inside its `<label>` (no `id`),
  and a submit button whose text is a new `button` prop. The option of the
  page's locale is `selected` in the markup, so the form is right before any
  code runs. Under `hydrate`/`csr` the submit is intercepted and is
  `set_locale`, focus kept; without code it is `GET ?lang=`. demo-islands'
  switcher is no longer an island. Test: `render.rs`
  `the_switcher_is_a_get_form_applied_by_a_button` (14/14); negative
  control, `selected` never set: it fails at "the page's locale is selected
  in the markup".
* **The example fixes:** a `--field` colour for control edges (4.19:1 /
  4.26:1), placeholders in `--muted` (8.21:1 / 9.82:1), banner, navigation,
  main and contentinfo as siblings, the counter a `status`, demo-ssr's echo
  field its own label, `/lazy` its own title; `language.apply` in every
  example and locale.
* **The check:** `tools/e2e/checks/a11y.mjs`, 720/720 in Chromium and
  Firefox, 2026-09-24: the axe-core scan over 28 pages, the contrast figures,
  reflow and text spacing, the structure, and the switcher from the keyboard
  (an arrow key changes nothing but the select; the button switches — live
  on demo-ssr and demo-csr, by navigation on demo-islands and on demo-ssr
  with the wasm blocked). Negative controls in the page for each detector.
  The static host `csr.mjs` had is `lib/static.mjs` now, shared.
* **Re-run**, because every check switched on `change`: `demo.mjs` 134/134,
  `lazy.mjs` 66/66, `islands.mjs` 58/58, `csr.mjs` 78/78, both engines.
  WebKit was not run (not installed).
* **Not done:** a screen reader (none installed; outside the permitted
  network). The accessibility tree stands in for it.

## A12 — done (2026-09-24)

The whole record is [phase-7-results](phase-7-results.md) §A12. In short:

* **The matrix:** 164 normative statements at the pin, every one covered or
  `na` with a reason; `cargo xtask conformance-report` checks
  `conformance/coverage.toml` and writes `conformance/COVERAGE.md` —
  *0 gap(s)*. [01](01-conformance.md) §5 describes it as built.
* **127 new tests** in the WG schema under `conformance/extra/`; the suite
  is 612 tests in 22 files. L1–L7c pass everything applicable, in Chromium
  and Firefox for the browser columns; the default configuration's
  degradations are 119 (+50, the two known kinds). No `xfail` is left in
  the ledger.
* **Three defects fixed:** a repeated attribute name the data-model
  serializer refused; the harness's `:test:function` refusing its own value
  as `decimalPlaces`; L6 unable to judge markup on a dynamic call site (the
  harness now lowers `TrDyn` to slot order for its rich render).
* **Owner question 10 built:** `to_string()` / `String::from` isolated,
  `to_plain_string()` plain; `cargo xtask size` B1 22,108 → 22,100 B gz.
* **Owner question 11 built:** the `nonstandard-name` lint (UAX #31 under
  MF2's profile, UTS #39's Identifier_Status and single-script), attribute
  names included; `unicode-security` adds 41,800 B to the `mf2` CLI and
  nothing to the client.
* **A reading recorded:** `u:id` takes a string (or an exact decimal's text,
  or a custom value exposing one); an integer or float is Bad Option. No
  test asserts the number half.

## A13 — done (2026-09-24)

The whole record is [phase-7-results](phase-7-results.md) §A13. In short:

* **The pages:** `docs/` — getting started (install, `mf2 init`, the
  messages, a server-rendered application that hydrates and switches), call
  sites, delivery modes (SSR + hydrate first, then lazy routes, islands with
  `islands-zero`'s figures, client-only with `mf2 compile --site`),
  switching language, accessibility. The root README leads the same way
  and was rewritten (it still said "Phase 2 is next").
* **Every sample compiled:** `cargo xtask docs` assembles the `rust`,
  `toml` and `mf2` blocks of the pages and the README into five
  applications (`file=<project>/<path>` in the info string; `merge` for a
  manifest a variant changes; `generated` for a file `mf2 init` wrote,
  compared byte for byte; `run=` for the `mf2` commands a page runs), and
  checks each for the server and for wasm32 with warnings denied; the
  client-only one also publishes its catalogs. A code block with no
  `file=` is refused. New CI job `docs`; `cargo xtask ci` runs the
  assembly (`--no-build`).
* **`mf2 init` scaffolds a crate that works with Leptos as written**
  (`ssr`/`hydrate`/`csr` forwarding `mf2`'s, `setup()`, the date features
  implying `fn-datetime`); every example had added these by hand.
* **Found and fixed: a live switch was not remembered.** Under `hydrate`,
  `set_locale` wrote no cookie (04 §6 said it did) and left `?lang=` in
  the address, so a reload came back in the old locale. It now writes the
  cookie and drops the query; `demo.mjs` asserts it.
* **Found, not built:** the reader's time zone (03 §6's cookie) — an
  instant formats in UTC unless the value or `Setup` names a zone. The
  documentation says so; it is left to the Phase 8 order (A10).

## A14 — design (2026-09-24; built as written — [phase-7-results](phase-7-results.md) §A14)

Written before any code, as the row asks. What exists: the catalog's
FALLBACK section (02 §2.6) names, per message, the locale its text came
from — the source locale included, since `missing = "fallback"` ends every
chain there (`mf2-build` `catalog::write`); `Catalog::fallback_locale(id)`
reads it (binary search), `Catalog::dir()` is the page's direction, and the
generated `LOCALES` (in `Setup::locales`) holds every lender's direction,
because a lender is always one of the build's locales. The feature
`mark-fallback-lang` is declared in `leptos-mf2` and `mf2` and does
nothing.

**What is marked.** A description in a **view position** whose message
the catalog in force borrowed: the text child (`Tr`, `TrArgs`, `TrDyn`)
and the rich fragment (`TrRich`). It renders as
`<span lang="{lender}">…</span>`, with `dir="{ltr|rtl}"` added when the
lender's direction differs from the catalog's (English borrowed into an
Arabic page must not be laid out right to left; WCAG does not ask for it,
correct rendering does). The tag is the lender's as written, even where it
shares a language with the page (`fr` inside `fr-CA` is valid and says what
the catalog knows). A message the catalog did not borrow renders exactly as
today — a bare text node, no element — so the feature costs a page
nothing until a translation is missing.

*Rejected: a `<span>` around every message*, with `lang` set only when
borrowed. It would make the structure independent of the catalog, but
put an element around every translated string on every page (bytes, CSS
selectors such as `p > span`, and RCDATA breakage — below — on every
message instead of only on borrowed ones).

**What cannot be marked, and is documented rather than built.** HTML marks
the language of an attribute only through its element's `lang`, which
would re-label the element's content too; and a `String` carries no markup.
So `title=`, `aria-label=`, `alt=`, `placeholder=`, properties,
`to_string()`/`String::from`, `TextProp`, `Signal<String>` and `Oco` stay
unmarked. `docs/accessibility.md` says so and points at
`missing-translation` and `mf2 stats`, which remain the way to see the gap.
Two elements hold only text: in `<title>` and `<textarea>` (RCDATA) a span
is shown as literal markup, so there the string forms are the way
(`leptos_meta`'s `<Title text=…>`, `prop:value`), and the docs say so.
`<option>` is safe either way: a parser that drops the span leaves the
text, which hydration adopts (below). `<LocaleOption>`'s name is the one
message whose language the element knows better than the catalog (its
`lang=tag` is right, a lender's tag would not be); it is left to the
lint — the names are meant to be defined in every locale, and
`missing-translation` warns when one is not.

**The server** (`to_html_with_buf`, both tachys lines). Unborrowed: as
today. Borrowed: the `<!>` separator exactly when tachys would write one
before a text (position `NextChildAfterText`), then
`<span lang=… [dir=…]>`, the text escaped by the same rule
`<&str as RenderHtml>` uses inside it with position `FirstChild` (an empty
text writes the same `' '`), `</span>`, and the position left at
`NextChildAfterText` — **the same position a text leaves**, so whatever
follows writes and walks identically whether the description was wrapped
or not. `TrRich`: the builder's root fragment becomes one tachys
`span().attr("lang").attr("dir")` view holding it; tachys writes and
hydrates that, as it does the handlers' elements.

**Hydration adopts the shape the server wrote; it does not ask the
catalog** (§3's rule for text, kept). `adopt_text` walks the cursor as it
does now (child or sibling, then over the separator) and looks at the node
it lands on: a `Text` is adopted as today; an `Element` is adopted as the
wrapper, its first child as the text node (created and appended when the
span is empty in the DOM); anything else is today's one-line mismatch and
degradation. The position is set to `NextChildAfterText` in every case,
matching the server. A client that builds (CSR, a template clone,
`FROM_SERVER = false`, a list row added later) reads the catalog and
wraps or not. `TrRich` hydrates through tachys, against the same catalog
the boot gate guarantees for its structure already (§6).

**The retained state and a switch.** The text node keeps its identity for
the life of the view: the wrapper comes and goes **around** it. Under the
feature, `TrState` and the registry's `Target::Text` share one
`Rc<Cell<Option<Element>>>` (the wrapper, if any); the slot's write
becomes "set the text, then fit the wrapper" against the new catalog's
`fallback_locale(msg_id)`:

| before → after | DOM change |
|---|---|
| none → none | the text only (today) |
| none → borrowed | a span is created, inserted before the text in its parent (if mounted), and the text moved into it |
| borrowed → borrowed | `lang` / `dir` set or removed if they changed |
| borrowed → none | the text inserted before the span, the span removed |

`Mountable` (`mount`, `unmount`, `insert_before_this`) acts on the outer
node — the wrapper if present, else the text. `rebuild` (a closure that
switched messages) takes the same path, so it works under `static-locale`
too, where there is no switch but a rebuilt description can change lender.
`TrRich`'s switch is already a fragment rebuild; a changed root (wrapped ↔
not) is a different `AnyView` type, which tachys replaces. Nothing here
changes a build without the feature: every added field, branch and import
is behind `cfg(feature = "mark-fallback-lang")`, and `cargo xtask size`
must show B1 unmoved.

**One lookup.** The text functions gain a form that hands the body the
lender beside the text — `Option<Lender>`, the lender's tag and a `dir`
that is `Some` only when it differs from the catalog's — computed where the catalog is
already in hand (`format_with`, the registry's `Slot::apply`), so no
position looks the catalog up twice.

**Tests (the row's "done when").**

* **native, `ssr`** (`crates/leptos-mf2/tests/render.rs`, the feature on):
  a catalog with `fr` borrowing one plain and one rich message from `en`
  renders both inside `<span lang="en">`; an `ar` catalog borrowing from
  `en` adds `dir="ltr"`; an own message has no span; the text around a
  wrapped message separates exactly as around an unwrapped one (a wrapped
  message between two text children); an attribute and `to_string()` are
  unmarked. Negative control: the same test with the feature off fails.
* **browser** (`tools/e2e/checks/demo.mjs`, two engines): `demo-ssr` turns
  the feature on and gains one sentence left untranslated in `ar` on
  purpose — a demonstration of the feature, said so in its source; if the
  example's check denies `missing-translation`, that one message is
  allowed explicitly. Assert: served `?lang=ar` has the span with
  `lang="en" dir="ltr"`; hydration logs no `mf2:` mismatch and the span is
  the same node after hydration; a live switch to `fr` removes the span
  and keeps the text node; back to `ar` restores it; the a11y scan still
  passes. Negative control: with `adopt_text`'s element branch removed,
  the check fails on the mismatch line. `demo-csr` (the build path): the
  same sentence in `ar` is wrapped after mount and after a switch.
* **size:** `cargo xtask size` unchanged with the feature off (it is off
  in the gated build); the example client's delta with it on, measured and
  recorded in the results.

`docs/accessibility.md` §"Untranslated text" is rewritten when it is
built (it says the feature does nothing), and the feature's comment in
`crates/leptos-mf2/Cargo.toml` with it.

## Exit (master plan §9, P7)

- [x] L7 and L7c green in both configurations, every L7d and L7cd cell
      recorded — `pass`, or `degraded` with its kind — none `xfail`;
      `current_phase = "P7"` in the exit commit with the harness green
      *(L7, L7c 444/444; L7d, L7cd 325/444 + 119 documented degradations;
      `cargo xtask ci` and `cargo xtask l7-web` green at P7, 2026-09-24)*
- [x] the WCAG 2.2 AA audit of the examples passes (A11; the audit is in
      [phase-7-results](phase-7-results.md), a screen reader not run)
- [x] no normative spec statement without a covering test (A12; 164
      statements, 0 gaps)
- [x] user documentation (A13) and `mark-fallback-lang` (A14) done
      *(per-commit benchmarks, A15, withdrawn by owner question 12)*
- [x] islands, CSR and lazy routes each demonstrated by the example and
      asserted by a browser check (A1 `islands.mjs`, A2 `csr.mjs`, A3
      `lazy.mjs`, all in Chromium and Firefox)
- [x] a server-only component's wasm cost measured at zero (A1, `cargo
      xtask islands-zero`: identical code, data within 16 B)
- [x] owner questions 1–12 answered (2026-09-23; 7–12 on 2026-09-24) and recorded in 04, 05, 01 and the master plan
- [x] `plans/phase-7-results.md` and the Phase 8 work order written
      ([16](16-phase-8-work-order.md), A10)
