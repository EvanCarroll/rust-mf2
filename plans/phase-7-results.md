# Phase 7 results — islands, CSR, lazy routes (layer L7)

What Phase 7 built and what it measured, against
[15-phase-7-work-order](15-phase-7-work-order.md). Every figure comes with
the command that produced it; where a figure moved a budget or a ledger
status, the commit that moved it says why (master plan §11).

The tasks' own records (A1–A9) are in the work order, under each task's
heading; this file gathers them at the phase exit. It starts with the one
document the work order asks to be written *here*: A11's audit.

## A11 — the WCAG 2.2 AA audit of the examples

**Result: the examples pass WCAG 2.2 AA as audited, with one part not
done — a screen reader (below).** Audited 2026-09-24: `examples/demo-ssr`
(`/` and `/lazy`, `cargo leptos build --split`, debug), `examples/demo-islands`
(`cargo leptos build`, debug) and `examples/demo-csr` (`trunk build`), each in
`en`, `fr` and `ar` (right to left), light and dark, in Chromium 143 and
Firefox 155 (Playwright 1.63.0). WebKit was not run: its build is no longer
installed on the development machine.

### How

* **Automated:** `tools/e2e/checks/a11y.mjs` — `node run.mjs a11y
  --base-url http://127.0.0.1:3702 --browser chromium,firefox` with
  demo-islands on port 3704. axe-core 4.13.0 (pinned in
  `tools/e2e/package.json`; MPL-2.0, a test dependency only) with the
  `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22a` and `wcag22aa`
  rules, over 28 pages per engine (four pages × three locales × two
  schemes, and demo-ssr after a live switch to `ar` and a client
  navigation to `/lazy`, in both schemes). Beside the scan, the check
  measures what axe does not — field edges, placeholders, buttons and the
  accent colour from the computed styles; horizontal scroll at 320 CSS px —
  and asserts the structure and the switcher's keyboard behaviour below.
  **720/720** assertions in the two engines. Each detector has a negative
  control on a live page: `<html lang>` removed and body text `#ddd` →
  `html-has-lang` and `color-contrast`; the old field border `#d3d7df` and a
  `#aaa` placeholder → below 3:1 and 4.5:1; a 400 px box → a horizontal
  scroll; a one-line box hiding a wrapped sentence → clipped; a `<select>` that switches on `change` → the arrow-key probe
  fails.
* **By hand**, for what a scanner cannot see: focus order, the language of
  parts, the switcher from the keyboard, the accessibility tree
  (Playwright's `ariaSnapshot`, standing in for a screen reader). The hand
  audit ran at 33a9621, before the fixes; what it found is the table below,
  and each fix is asserted by the check, not by a second hand pass.

### Findings, and what became of each

| # | Criterion | Found (before) | Now | Asserted by |
|---|---|---|---|---|
| 1 | **3.2.2 On Input** (F37) | The switcher switched on the `<select>`'s `change`, which the keyboard fires on every arrow key: one ArrowDown switched demo-ssr to `fr` live, and reloaded demo-islands in `fr` with focus on `<body>`; `ar` was unreachable from `en` without passing `fr` | **Fixed in the library** (owner question 9): `<LocaleSwitcher>` is a `<form method="get">` — the `<select name="lang">` inside its `<label>`, and a submit button whose text is the application's message (`button`). An arrow key changes the select only. Under `hydrate`/`csr` the submit is intercepted and is the live `set_locale`, focus kept on the button; with no client code it is the form's `GET ?lang=`, which `mf2-axum`'s `QueryParam` negotiates and the cookie sink remembers. The option of the page's locale is `selected` in the markup, so the form is right before any code runs | `render.rs` `the_switcher_is_a_get_form_applied_by_a_button`; `a11y.mjs`: ArrowDown changes the select and not the page (no switch, no reload) on all three examples, Tab reaches the button, Enter switches — live on demo-ssr and demo-csr, by navigation on demo-islands and on demo-ssr with the wasm blocked |
| 2 | 4.1.1 / 1.3.1 (a page with two switchers) | A fixed `id="mf2-locale"` and `<label for>`: a second switcher duplicated the id and lost its label | **Fixed:** no `id`; the select is inside its label | `render.rs` (no `id`, no `for`); `a11y.mjs` (no `id` in the switcher, no duplicate `id`, the combobox named by its label) |
| 3 | **1.4.11 Non-text Contrast** | Field and select borders `--line` against the page: 1.44:1 light, 1.58:1 dark | **Fixed:** a `--field` colour for control edges — 4.19:1 light (`#767c89`), 4.26:1 dark (`#737b8c`); `--line` stays for decorative rules | `a11y.mjs` field edges ≥ 3:1 on every page |
| 4 | **1.4.3 Contrast (Minimum)** | Placeholders in the engines' colours: Chromium `#757575` 3.93:1 on the dark page; Firefox ≈ 3.95:1 on the light one | **Fixed:** `::placeholder { color: var(--muted); opacity: 1 }` — 8.21:1 light, 9.82:1 dark | `a11y.mjs` placeholder ≥ 4.5:1 |
| 5 | 1.3.1, 2.4.1 (landmarks) | `<header>`, `<nav>` and `<footer>` inside `<main>` on all three examples: no banner, no contentinfo, "skip to main" landing on the header | **Fixed:** banner, navigation, main and contentinfo are siblings inside the schema.org `WebPage` wrapper | `a11y.mjs` one `main`, the three outside it; axe's landmark rules |
| 6 | 2.4.6 Headings and Labels | demo-ssr's echo field was labelled "Search", like the search field above it, and is not a search | **Fixed:** its own message, `echo-label` ("Type something") in en/fr/ar | `a11y.mjs` no two fields share a label |
| 7 | 2.4.2 Page Titled | demo-ssr's `/` and `/lazy` had one `<title>`; a client navigation changed nothing | **Fixed:** `/lazy` sets its own (`lazy.page-title`), served and on a client navigation, and leaving restores the home title | `a11y.mjs` (served title, navigated title, restored title) |
| 8 | 4.1.3 Status Messages | "N people are here" changed on a button press, focus on the button, and nothing announced it | **Fixed:** `role="status"` on the line, in all three examples | `a11y.mjs` the counter is a `status` |
| 9 | 2.4.3 Focus Order (client navigation) | A client navigation leaves focus on the link pressed | **Recorded, not a failure:** the link stays in the navigation, and the order from it is the page's order; the new title (7) says where the reader is. Moving focus into the route is a design choice for the application, not a 2.2 AA requirement | — |
| 10 | 2.4.2 (client-only boot) | demo-csr's `index.html` says `<title>mf2</title>` until the boot sets the page's, and keeps it over an empty body when the boot fails | **Recorded, not a failure:** a page with no content has nothing to title; the failed boot logs one `mf2:` line (A2) | — |
| 11 | screen reader | Not done: none is installed on the development machine (`orca`, `espeak-ng` absent), and installing one is outside the network this repository may use | **Recorded.** The accessibility tree stands in: every control named, each option with its own `lang`. That a voice changes on an option's `lang` is not verified | — |

### Checked and passing, unchanged

* **The scan:** 0 violations and 0 incomplete on all 28 pages in both
  engines; axe's `best-practice` rules also report nothing (recorded, not
  asserted).
* **1.4.10 Reflow and 1.4.12 Text Spacing:** at 320 CSS px, with and
  without the spacing applied (line height 1.5, letter 0.12 em, word
  0.16 em, paragraph 2 em), no page scrolls sideways and no box that hides
  its overflow cuts text off — every page, every locale (asserted).
* **Buttons and the focus ring:** button text 6.77:1 light and 8.47:1 dark;
  the accent (focus ring, links) the same against the page; a 3 px ring with
  a 2 px offset on every control (2.4.7, 2.4.11).
* **2.4.3 Focus Order** follows the visual order in `ar` (right to left) and
  `en` (by hand, before the fixes, which added one control after the select
  and moved no other); focus stays on the switcher's button across a live switch (asserted).
* **3.1.1 Language of Page:** `<html lang dir>` right on the server and after
  every switch (the `demo`, `lazy`, `csr` checks, and here).
* **3.1.2 Language of Parts:** each language named in its own language with
  its own `lang`. The Latin inside Arabic text — "Leptos", "wasm", "Esc",
  "Ada" — is proper names and technical terms, which 3.1.2 exempts. Text a
  catalog borrows from a fallback locale is A14's (`mark-fallback-lang`);
  the examples have none.
* **2.5.8 Target Size (Minimum):** axe's rule passes.

### What the fix cost

* **A reader:** one more action — choose, then press the button — where the
  `change` switched at once.
* **An application:** one more message (the button's text) and one more
  prop. The three examples have it (`language.apply`: Apply / Appliquer /
  تطبيق).
* **demo-islands' client:** the switcher is no longer an island. Its size
  effect was not measured here; `cargo xtask islands-zero` (nightly) keeps
  measuring the server-only claim, and was not re-run for this change.
  `cargo xtask size` was not re-run either: its template renders no
  switcher.
* **The re-run checks**, Chromium and Firefox, debug builds: `demo.mjs`
  134/134, `lazy.mjs` 66/66, `islands.mjs` 58/58 (was 56: the switcher is
  now asserted to be a `GET` form and not an island), `csr.mjs` 78/78. All
  four now switch through the button (`chooseLocale` in `lib/browser.mjs`).

### Observed, not changed

* After a switch with no client code the address carries `?lang=…`, which
  `QueryParam` ranks above the cookie; a later **live** switch does not
  remove it (a live switch never wrote the cookie either, since Phase 6), so
  a reload returns to the language in the address. Under `static-locale` the
  switch removes it before navigating, as before.
