# Phase 7 results — islands, CSR, lazy routes (layer L7)

What Phase 7 built and what it measured, against
[15-phase-7-work-order](15-phase-7-work-order.md). Every figure comes with
the command that produced it; where a figure moved a budget or a ledger
status, the commit that moved it says why (master plan §11).

The tasks' own records (A1–A9) are in the work order, under each task's
heading; this file gathers them at the phase exit. After the exit's status
it starts with the one document the work order asks to be written *here*:
A11's audit.

## Status at exit

**Every exit criterion is met** (2026-09-24), and `current_phase = "P7"` is
in the exit commit with the harness green.

| Exit criterion | Verdict |
|---|---|
| L7 and L7c green in both configurations, every L7d and L7cd cell recorded, none `xfail`; `current_phase = "P7"` with the harness green | **met** — L7 and L7c 444/444, L7d and L7cd 325/444 + 119 documented degradations (L6d's), in Chromium and Firefox, `l7: 34/34 assertions` over 16 pages per engine (`cargo xtask l7-web --browser chromium,firefox`, verify only); L6 in the same two engines `20/20` (`cargo xtask l6-web`); `cargo xtask ci` green, its report 612 entries at P7, no `xfail` |
| the WCAG 2.2 AA audit of the examples passes | **met**, one part not done — a screen reader (§A11) |
| no normative spec statement without a covering test | **met** — 164 statements, 0 gaps (§A12, `COVERAGE.md`) |
| user documentation and `mark-fallback-lang` | **met** — §A13, §A14; per-commit benchmarks (A15) withdrawn by owner question 12 |
| islands, CSR and lazy routes demonstrated by the example and asserted in a browser | **met** — `islands.mjs`, `csr.mjs`, `lazy.mjs`, Chromium and Firefox (work order A1–A3) |
| a server-only component costs zero wasm | **met** — `cargo xtask islands-zero`: identical code, data within 16 B (work order A1) |
| owner questions 1–12 answered and recorded | **met** — 2026-09-23 and 2026-09-24, in 04, 05, 01 and the master plan |
| this file and the Phase 8 work order | **met** — [16](16-phase-8-work-order.md) (A10) |

The L7 and L6 runs at exit are the first since A13 and A14; they moved no
cell. Every browser figure is
Chromium and Firefox; WebKit is not installed on the development machine.

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
  the examples had none at this audit. *Since A14:* `demo-ssr` and
  `demo-csr` leave one sentence untranslated in `ar` on purpose, rendered
  inside `<span lang="en" dir="ltr">`, and the scan still passes (§A14).
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

## A12 — spec coverage

The master plan's exit asks for no normative statement of the pinned spec
without a covering test. Built: the statement rule, the matrix, its checker,
127 new tests in the WG schema, two defects fixed, and owner questions 10 and
11 built. [01](01-conformance.md) §5 describes the matrix as built.

### The matrix

* **164 normative statements** at the pin (`syntax.md` 40, `formatting.md`
  45, `errors.md` 9, `u-namespace.md` 5, `data-model/README.md` 7,
  `functions/README.md` 19, `functions/number.md` 22,
  `functions/datetime.md` 16, `functions/string.md` 1), every one with an
  entry in `conformance/coverage.toml`, rendered to `conformance/COVERAGE.md`.
  `cargo xtask conformance-report`: *164 normative statements, 0 gap(s)*.
* An entry is either `tests` or `na = { kind, reason }`; `na` is for a
  statement that binds no implementation (addressed to message authors or
  function authors, or a MAY not taken — `kind = "permission"`, allowed only
  on a MAY). The checker rejects an entry naming a test that does not exist,
  so every citation is live.
* `conformance/tests/coverage.rs`: the matrix complete, `COVERAGE.md`
  current, the sentence splitter lossless (a paragraph scan finds no key
  word outside an extracted statement) and one mutation per kind of gap.

### New tests (`conformance/extra/`, WG schema)

127, in `syntax.json` (8), `formatting.json` (9), `u-options.json` (6),
`functions/accept.json` (21: every value of every REQUIRED option of the
numeric functions) and `functions/numeric-options.json` (83: outputs for the
options the WG suite asserts none for, ordinal selection, exact-match
serialization). Every layer runs them; the suite is now 612 tests in 22
files (462 the WG's, 150 ours).

| Layer | pass / applicable | degraded |
|---|---:|---:|
| L1, L4, L5 | 612 / 612 | |
| L2 | 470 / 470 | |
| L3, L6, L7, L7c | 444 / 444 | |
| L4d, L5d | 493 / 612 | 119 (+50) |
| L6d, L7d, L7cd | 325 / 444 | 119 (+50) |

The 50 new degradations are the default configuration's two known ones —
Unknown Function for a gated function, neutral digits without `fn-number`
— on numeric-option tests. Commands: `cargo xtask conformance-report`
(L1–L6d); `cargo xtask l7-web --browser chromium,firefox --promote` (L7
columns, 480 cells promoted from `xfail`; `l7: 34/34 assertions`);
`cargo xtask l6-web` (`l6: 20/20 assertions`, both engines).

### Defects the matrix found, fixed

1. **A repeated attribute name could not be written as the data model.**
   `{a @c @c=d}` is valid (unique attribute names are only a SHOULD; all but
   the last are ignored), but `mf2-model`'s JSON serializer refused it, so
   L2 would have failed on it. It now writes the last occurrence, and
   `Attributes::get` returns the last
   (`a_repeated_attribute_name_serializes_its_last_occurrence`).
2. **The harness's `:test:function` refused a `:test:function` value as its
   `decimalPlaces` option**, which `test/README.md` says resolves to its
   input (`conformance/l4-runner/src/test_functions.rs`; `formatting.json`
   #0 covers it).
3. **L6 could not judge markup on a dynamic call site.** `formatting.json` #2
   (`{#tag foo=$x}content{/tag}`, `$x` unset) has to go through `TrDyn`,
   which has no rich form, so L6 reported its markup as missing. The harness
   now lowers a `TrDyn` to slot order by the catalog's NAMES — a slot no
   argument names is `ArgValue::Unset` — and renders that through
   `tr_rich`; the markup it judges is the same message with the same values.
   L6 443 → 444 of 444 (`cargo run -p mf2-conformance --example failures --
   L6`); the run before the fix is the negative control. This was the
   harness, not the library: a `TrDyn` with markup renders its text; only
   a rich view of it is missing, and nothing asks for one.

### Rust tests for what no WG-schema test can see

`mf2-runtime/tests/additions.rs::a_declaration_is_resolved_once_and_a_handler_sees_no_u_options`,
`mf2-runtime/tests/format.rs::u_options_are_removed_before_the_handler`
(negative control: `u:` options pushed to the handler's list — both fail),
`mf2-fn-datetime/tests/format.rs::the_time_zone_option_on_date`.

### A reading recorded: `u:id`

`u-namespace.md` requires `u:id`'s value to be a literal or a variable whose
resolved value is a string, or turns into one without an error. rust-mf2 reads it narrowly (`str_of` in
`crates/mf2-runtime/src/eval.rs`): a string, an exact decimal given as its
text, or a custom value that exposes a string. An integer or float argument
is a Bad Option and `u:id` is dropped — the formatted number is not the
resolved value, and taking it would make an id depend on the locale.
`extra/u-options.json` covers the string case (U1) and a value that cannot
become a string (U2, a `:test:select` value); **no test asserts the number
case**, which is this reading rather than a requirement.

### Owner question 10: a string is isolated by default

`to_string()` and `String::from` on `Tr`, `TrArgs`, `TrRich` and `TrDyn` now
use the Default Bidi Strategy; `to_plain_string()` is new and plain;
`to_display_string()` stays, as a synonym of `to_string()` (04 §9, revised).
View positions are unchanged. Test:
`crates/leptos-mf2/tests/render.rs::a_string_is_isolated_and_a_plain_string_is_not`
(`to_string`, `String::from` and `to_display_string` isolated,
`to_plain_string` plain); it replaces
`a_string_is_plain_and_a_text_child_is_isolated`, which asserted the
opposite. Negative control, run: `to_string()` put back to plain, the new
test fails at its `to_string` assertion (`"Hello, Ada!"` against the
isolated form). Nothing in the examples
or `tools/e2e` consumed a plain `String`: `demo-ssr`'s `echo` puts its
`to_string()` back into the page, where isolation is right.

**Size** (`cargo xtask size`, whose template formats every site to a
`String` through `to_string()`, so it is the path that changed): B1 22,108 →
**22,100 B gz** (−8), B5 **12.6 B gz** a site (unchanged), whole app at
1,860 sites **45,519 B gz** (unchanged) — all met. The only difference on
the client is which of the two formatters the method passes.

### Owner question 11: the `nonstandard-name` lint

`crates/mf2-build/src/lint.rs` / `check.rs`, a warning by default, `allow`
permitted (05 §5). It checks every variable, option, function, markup and
attribute name of a message, on its NFC form, each side of a namespace
alone:

1. a UAX #31 identifier under MF2's profile — Start is XID_Start plus `_`,
   Continue is XID_Continue plus `-` and `.` (`unicode-ident`);
2. every character Identifier_Status=Allowed (UTS #39 General Security
   Profile, `unicode-security`);
3. a single script by UTS #39's resolved script set, so kana with kanji is
   one script and Latin with a Cyrillic `а` is two (`unicode-security`).

Tests: the seeded drift in `tests/drift.rs` (`$nаme` with U+0430 in both
locales: the lint fires, nothing else errors, and `allow` silences it); unit
tests in `check.rs` for names that pass (`a-b.c`, `_x`, `número`, `名前`,
`ひらがな漢字`, `имя`) and one per reason (U+2140, U+01C5, U+0430). Negative
control: the check forced to find nothing — the drift test fails
("nonstandard-name … did not fire"). The 6,400-message reference workload
raises none (`the_reference_workload_is_clean`). *Added to the plan's list:*
attribute names (05 §5 says so).

**What `unicode-security` costs** (05 §5 asked for it to "measure
acceptable"): it is a dependency of `mf2-build` only, so of the build and
the `mf2` CLI, never of a client crate. `cargo build --release -p mf2-cli`
with the lint as built and with its two UTS #39 checks and the dependency
removed: `target/release/mf2` 11,848,664 → **11,890,464 B (+41,800,
+0.35 %)**. Its compile time was not measured separately. Kept.

## A13 — user documentation

**What was written.** `docs/` — [getting started](../docs/getting-started.md),
[call sites](../docs/call-sites.md), [delivery modes](../docs/delivery-modes.md),
[switching language](../docs/switching.md), [accessibility](../docs/accessibility.md)
and an index — and a new root `README.md` (the old one said "Phase 2 is
next"). The order is owner question 1's: SSR + hydrate first, islands second
with `cargo xtask islands-zero`'s figures, then client-only; one i18n crate
for a server-rendered application and catalogs published apart
(`Emit::Module` + `mf2 compile --site`) for a client-only one (questions 2
and 6). The pages name the crates as a release will (`mf2 = "0.1"`) and say
that, until then, a path into a checkout replaces the version.

**Every sample compiled — `cargo xtask docs`.** A `rust`, `toml` or `mf2`
block names its file (`file=<project>/<path>`); the blocks of a project, in
page order, are that project. `merge` merges a `toml` block into the file a
project inherits (the islands manifest shows two lines, not seventy);
`generated` marks a file a command wrote, compared byte for byte; `run=`
holds the `mf2` commands a page runs, so the i18n crate is made by the
documented `mf2 init`. A code block with no `file=`, an unknown attribute,
or a page under `docs/` the xtask does not list is an error. Dependencies on
this repository's crates are pointed at the working tree. Five applications
(`hello`, `calls`, `lazy`, `islands`, `csr`) are checked natively with `ssr`
and on `wasm32-unknown-unknown` with `hydrate` (or `csr`), `RUSTFLAGS="-D
warnings"`, one shared target directory; `csr`'s catalogs are then
published with `mf2 compile --site` and must include `index.json`.

* 2026-09-24: 68 blocks on 6 pages, 60 of them files, all compiled; the
  build scripts' lints silent (the first run found `missing-plural-category`
  on the French plural, and the sample now names `many`); the first run
  also failed on a `cfg(feature = "csr")` in an application that declares
  no `csr`, now fixed in the sample.
* Negative controls, run: a line of `mf2 init`'s `src/lib.rs` changed on
  the page — `--no-build` fails naming the page line and both texts; a
  misspelt id in a sample (`welcom`) — the `calls` check fails with the
  macro's "did you mean `welcome`?".
* CI: a new `docs` job runs `cargo xtask docs`; `cargo xtask ci` runs the
  assembly without the builds.

**`mf2 init` changed** so that what the pages show verbatim works: the
scaffolded crate forwards `mf2`'s `ssr`/`hydrate`/`csr` and has a `setup()`
(every example had added both by hand), and `datetime-icu` /
`datetime-intl` imply `fn-datetime`, without which the build script does not
consider dates available. Its "next steps" name both installs.

**Found and fixed: a live switch was not remembered.** 04 §6 says a switch
updates the cookie; under `hydrate` without `static-locale`, `set_locale`
wrote none, and a `?lang=` in the address (which the switcher's no-script
form and any link produce) outranks the cookie anyway. Observed before the
fix, `demo-ssr` (debug `--split`), Chromium and Firefox: after a live switch
to `fr` the cookie held another tag, the address kept `?lang=en`, and a
reload came back in `en` — `demo.mjs` 138/144, the three new assertions
failing in each engine. Now the live switch writes the `mf2_locale` cookie
with `CookieLocale`'s default attributes (the code the `static-locale`
switch already had, shared) and removes `lang` from the address with
`history.replaceState` (web-sys `History`). After: `demo.mjs` 144/144,
`lazy.mjs` 66/66, `islands.mjs` 58/58, `csr.mjs` 78/78, `a11y.mjs`
720/720, both engines. WebKit was not run (not installed). Size, `cargo
xtask size`: B1 22,100 → **22,102 B gz** (+2, the cookie write and the
query removal now on the live path), B5 12.6 B gz a site (unchanged), whole
app at 1,860 sites **45,517 B gz** — all met.

**Found, not built: the reader's time zone.** 03 §6 plans a zone cookie and
a re-render after hydration; neither exists. An instant formats in its own
zone, else `Setup::with_time_zone`'s, else UTC, identically on both sides.
The call-site page says so. Left to the Phase 8 work order.

## A14 — `mark-fallback-lang`

Built to the design in [15](15-phase-7-work-order.md) §"A14 — design",
without departures. WCAG 3.1.2: a message the catalog in force borrowed
from another locale renders, in a view position, inside
`<span lang="{lender}">`, with `dir` when the lender's direction differs
from the catalog's. An own message is a bare text node, as before.

**Where it lives.** `crates/leptos-mf2/src/lang.rs` (new): the lender
(`Catalog::fallback_locale` plus the lender's direction from `LOCALES`,
found once where the catalog is already in hand), the server's opening
tag, the rich fragment's wrapper (one tachys `span` around the builder's
fragment), and the client's `Wrapper`, an `Rc<Cell<Option<Element>>>`
that the view state and the registry's `Target::Text` share. The glue
(`glue/view.rs`) writes, builds and hydrates the text types through it
under the feature, and leaves the feature-off bodies as they were. The
registry's four "format, then write" sites became one `Target::render`,
which under the feature fits the wrapper, so a switch, a rebuild (also
under `static-locale`) and an argument effect all take the same path.

**Tests.**

* Native, `ssr` — `crates/leptos-mf2/tests/fallback_lang.rs`, catalogs
  written with a FALLBACK entry (`mf2_catalog::writer`): 7 tests. A
  borrowed plain message is `<span lang="en">Save</span>`; borrowed into
  `ar` it gains `dir="ltr"`; an own message has no span; a borrowed rich
  message is wrapped whole; `a<!>Save<!>b` becomes
  `a<!><span lang="en">Save</span><!>b` (the separators do not move); an
  empty borrowed text keeps tachys' `' '`; `title=` and `to_string()` are
  unmarked. Negative control, run: the same file with the feature off —
  5 fail (every span assertion), the 2 "unchanged" tests pass.
* Browser, `demo-ssr` (debug, `--split`; Chromium and Firefox; WebKit not
  installed). The note on the home page, `note-label` then `untranslated`,
  is left untranslated in `ar` on purpose (said in the corpus and the
  view). `demo.mjs` 144 → **170/170**: served `?lang=ar` has
  `<span lang="en" dir="ltr">`, `en` and `fr` have none; hydration adopts
  the same span and text node and logs nothing; a switch to `fr` removes
  the span and keeps the text node; back to `ar` wraps that node again,
  with the hydrated page's text. Negative control, run: `Wrapper::adopt`
  refusing an element — Chromium 82/85, failing
  `hydration-reports-no-mismatch` on the `mf2:` mismatch line, and the two
  that follow from it. `a11y.mjs` 720/720 and `lazy.mjs` 66/66 with the
  note in place.
* Browser, `demo-csr` (the build path; `trunk build`). `csr.mjs` 78 →
  **88/88**: no span in `fr`; a switch to `ar` wraps the text node `fr`
  built; mounting in `ar` after a reload builds it wrapped; a switch back
  to `fr` unwraps it and keeps the node.
* CI: `cargo xtask ci` lints the feature's client half (`hydrate`, and
  `csr,static-locale`) and its server half with the test, and runs the
  test. `cargo xtask leptos-beta` also checks `hydrate` with the feature and
  runs `fallback_lang` on the 0.9 pre-release, where the separator follows
  `flags.hydrate` — run: passes on leptos 0.9.0-beta / tachys 0.3.0-beta2,
  `fallback_lang` 7/7. `cargo xtask docs`: every sample compiled, the new
  one included.

**Size.** `cargo xtask size` (the feature is off in the gated build): B1
**22,102 B gz**, unchanged from A13; B5 12.6 B gz a site; the whole app at
1,860 sites 45,517 B gz — all met. The example's client with the feature
on, `demo-ssr` `cargo leptos build --release --split`, measured by
alternating the one feature in its manifest:

| file | off (raw / gz / br) | on (raw / gz / br) | Δ br |
|---|---:|---:|---:|
| `demo_ssr.wasm` | 727,792 / 306,012 / 243,536 | 733,706 / 308,696 / 245,427 | +1,891 |
| the lazy route's chunk | 13,506 / 6,858 / 5,993 | 13,315 / 6,728 / 5,875 | −118 |
| `demo_ssr.js` | 22,497 / 6,611 / 5,750 | 22,613 / 6,636 / 5,773 | +23 |
| **total** | 763,795 / 319,481 / 255,279 | 769,634 / 322,060 / 257,075 | **+1,796** |

gzip −9 and brotli q11 (Node's zlib). A third build with only the rich
wrapper compiled out puts the main module's share at +933 B br for the
text path and +958 B br for the rich wrapper (a new element type behind
`AnyView`). Opt-in, so no budget moves; an application that turns it on
pays about 2.6 KB gz, inside B1's 30 KB with 22.1 KB used.

**Found and fixed: the example's release build did not compile.**
`cargo leptos build --split --release` in `examples/demo-ssr` (the
README's command) failed at the committed tree with "queries overflow
the depth limit" (the app's view type inside `hydrate_lazy`'s future);
debug builds, which every browser check uses, were unaffected. The
compiler's suggestion, `#![recursion_limit = "256"]`, fixes it; committed
apart, before this change.

**Documented.** `docs/accessibility.md` §"Untranslated text" (how to turn
it on, what it renders, and what cannot be marked: attributes, strings,
`<title>` and `<textarea>`); its sample is compiled by `cargo xtask docs`
as a `merge` into the `calls` project, so the docs build the feature for
`ssr` and `hydrate`. The feature's comments in `leptos-mf2`'s and `mf2`'s
manifests, and 04 §9.
