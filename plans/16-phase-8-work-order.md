# 16 — Phase 8 work order: migration and interchange

Part of the [master plan](00-master-plan.md) (§9, P8). RFC 2119 keywords
apply. Written at the close of Phase 7 (A10) from
[phase-7-results](phase-7-results.md), the Phase 7 work order's task records
([15](15-phase-7-work-order.md)), the prior-art audit of
[04](04-leptos-integration.md) §11, and the four owner answers below.

Phases 1–7 built the library. Phase 8 builds the way **into** it: a Fluent
project's messages converted to MF2, its call sites rewritten to `tr!`, a
translation team's files exchanged as XLIFF 2 — and, once, the measured
answer to the question a Fluent user will ask first: how does this compare
with `leptos-fluent`? It also builds the one piece of the runtime design that
three phases left unbuilt: dates in the reader's time zone.

## State at the start (Phase 7's exit)

| In the tree | Where |
|---|---|
| Every delivery mode: SSR + hydrate, lazy routes, islands, client-only; L6, L7, L7c green, L6d/L7d/L7cd with the default configuration's documented degradations | `crates/leptos-mf2`, `crates/mf2-axum`, `conformance/`, `examples/demo-{ssr,islands,csr}` |
| 612 suite tests (462 WG, 150 ours), every normative statement covered | `conformance/coverage.toml`, `COVERAGE.md` |
| The pipeline and the CLI: `init check compile fmt stats dump pseudo export import watch`; `export` / `import` speak **flat JSON** only | `crates/mf2-build`, `crates/mf2-cli` (`exchange.rs`) |
| The reference-workload generator: `.mf2` resources and flat JSON, call-site templates `tr`, `tr-view`, `idlit`, `idlit-view`, `dummy`, … | `bench/workload-gen` |
| The size gates (`cargo xtask size`, `b5 --view`, `islands-zero`, `churn`) | `xtask/` |
| User documentation, every sample compiled | `docs/`, `cargo xtask docs` |
| Dates: `:datetime` / `:date` / `:time`, both backends; an instant formats in its own zone, else `Setup::with_time_zone`'s, else UTC — the reader's zone is never learnt | `crates/mf2-fn-datetime`, [03](03-runtime.md) §6 |
| **No Fluent anywhere**: no `.ftl` reader or writer, no Fluent dependency | — |
| **No XLIFF**: the standard is not vendored | — |

## What Phase 7 left, and where it goes

* **The reader's time zone** (03 §6: a cookie, and a re-render after
  hydration) was planned for Phase 6 and never built; Phase 7 A13 found it
  writing the documentation. **A7**, by owner question 2.
* **Dev hot reload** — `mf2 watch` pushing a recompiled catalog to open
  pages through an `mf2-axum` dev endpoint — was in the master plan's P7
  list and was not built; Phase 7 A6 made the rebuild loop work instead
  (`watch-additional-files`; 4.0 s from edit to served on `demo-ssr`, 5.1 s
  on `demo-islands`, debug). **Deferred until after v1** by owner question
  1; the master plan's "Later" list says so.
* **Not run in Phase 7, recorded rather than tasked:** a screen reader (none
  installed, and installing one is outside the permitted network — A11);
  WebKit (its build is no longer installed on the development machine, so
  every Phase 7 browser figure is Chromium and Firefox); `cargo xtask
  islands-zero` after the switcher stopped being an island (it runs
  nightly). None of them blocks Phase 8.
* **Leptos 0.9** is still a beta (0.9.0-beta / tachys 0.3.0-beta2 at
  2026-09-24), tracked nightly by `cargo xtask leptos-beta` — until A0 made
  it the default and that job became `cargo xtask leptos-0-8`. See
  §"Standing".

## Owner questions

1. **Whether Phase 8 builds the dev mode that pushes a saved translation
   into the open page without a rebuild** — **answered (owner, 2026-09-24):
   later, after v1.** Developers keep today's loop (save; cargo rebuilds;
   the page reloads), which Phase 7 made work and measured. Recorded in the
   master plan (§9 P7, "Later") and 05 §4 and §6. The question as it was
   put: the push was planned for Phase 7 and not built; the options were to
   build it now (a dev-only endpoint and a browser listener that must never
   ship in a release build), defer it past v1, or drop it.

2. **How dates follow the reader's time zone** — **answered (owner,
   2026-09-24): build the cookie design.** On a first visit the server
   formats in UTC and the page corrects its dates after it loads and
   remembers the reader's zone in a cookie; later visits are right from the
   server. Readers get local times with no application code; a first visit
   sees dates change once. Recorded in 03 §6. The question as it was put:
   build the cookie design, have the application supply the zone and drop
   the cookie, or defer past v1.

3. **Whether the XLIFF 2 standard may be added to the repository as a new
   pinned upstream** — **answered (owner, 2026-09-24): yes, vendor it.** The
   core specification and its schema go under `third_party/` with a sync
   command, like the MF2 specification, so that XLIFF export and import can
   be built and checked against the schema. Recorded in the master plan (D13,
   §9 P8). The question as it was put: the standard is not in the tree and
   the boundary forbids fetching it; the options were to vendor it, keep
   flat JSON only and move XLIFF past v1, or drop XLIFF.

4. **Which application the `leptos-fluent` comparison measures** —
   **answered (owner, 2026-09-24): the generated reference application**
   (1,600 messages, 1,860 call sites — [06](06-size-and-perf.md) §2), built
   once on `leptos-fluent` and once *converted by this phase's own
   migration tools*, so the snapshot describes an application of realistic
   shape and the migration path is exercised end to end. Recorded in the
   master plan (§9 P8) and 06 §6. The question as it was put: the reference
   application, the small demo example ported to `leptos-fluent`, or both.

5. **Which differences between a converted message and its Fluent original
   count as "formats identically"** — **answered (owner, 2026-09-24): all
   four classes A3 measured are accepted**, each with its count (§A3 below):
   numbers are localized where `fluent-bundle` prints `f64`'s text; the
   invisible bidi isolates follow MF2's Default Bidi Strategy; a decimal
   with more than three places selects its variant as `:number` rounds it
   (the shown number and the wording agree); current CLDR plural rules,
   not `intl_pluralrules`' older ones. Nothing is changed to imitate
   Fluent. Recorded in the master plan (§9 P8, exit) and 05 §6.1. The
   question as it was put, class by class: accept, or imitate Fluent's raw
   numbers / change the isolation strategy / have the converter reproduce
   Fluent's variant choice / have the converter warn about selects whose
   default is not `other`.

6. **Which Leptos line is the default** — **answered (owner, 2026-09-24):
   Leptos 0.9, beta or not — "that was supposed to be the default"; and
   0.8 stays supported as an opt-in.** Applications get 0.9 without doing
   anything; one still on 0.8 turns on a feature and keeps working; both
   lines are built and tested in CI. Recorded in the master plan (D10) and
   04 §3, §10; the work is **A0**. The questions as they were put: keep 0.8
   the default until 0.9 is released, make the beta the default now, or
   support both equally; then, with 0.9 the default, drop 0.8 or keep it as
   an opt-in.

7. **Which variants a message that changes with a number offers a
   translator whose language has other plural forms than the source's** —
   **answered (owner, 2026-09-25): the target language's forms.** A Polish
   translator of an English one/other message gets four entries, an Arabic
   one six, each showing the English text MF2 would pick for it; an
   existing translation keeps its own variants, plus empty entries for the
   forms it lacks. Translators need no MF2; English text repeats, and two
   counts multiply (up to 36 entries in Arabic). Recorded in 05 §6.3. The
   question as it was put: the target language's forms, English's forms
   only (a translator cannot add the forms their language needs), or the
   whole message in one entry with its select syntax locked (translators
   must know MF2).

## Part A — tasks (A1 and A2 first, then A3; A0 before any other task; A4 after A1; A5 after A1, A2 and A4; A6 and A7 as convenient; A8 last)

**A0 and A5 are done** (2026-09-25; what was built is below the table).
**A6 (a) and (b) are done** (2026-09-25): XLIFF 2.1 vendored, the mapping
designed in 05 §6.3 with owner question 7 answered (below the table); (c),
the build, is next.
**A1, A2, A3 and A4 are done** (2026-09-24): A2's corpus converts with no
finding, which was A1's last criterion; converted catalogs format as the
originals but for four owner-approved classes (owner question 5); the
reference application on `leptos-fluent` converts to exactly the expected
call sites and builds. What was built is below the table.

| Task | Deliverable | Done when |
|---|---|---|
| **A0** Leptos 0.9 the default | *Owner question 6; D10; 04 §3, §10.* The workspace's `leptos`, `leptos_axum`, `leptos_router`, `leptos_meta` and `tachys` move to the newest 0.9 / 0.3 pre-releases (what `cargo xtask leptos-beta` pins today), with `any_spawner` and whatever else must move with them. The glue switch inverts: 0.3 is the default path, an opt-in feature (named in the task, documented) selects the 0.2 line; `tachys-0-3` goes. Everything is rebuilt and re-run on 0.9 — L6, L6d, L7, L7c, L7cd, the examples, `cargo xtask size` (a moved budget figure is recorded with its cause, master plan §11) and `cargo xtask docs`. The docs' `Cargo.toml` samples show 0.9 and say how to stay on 0.8. `cargo xtask leptos-beta` becomes a job that builds and tests the **0.8** feature (renamed to say so), run in CI, not allowed-to-fail. | `cargo xtask ci` green on 0.9; the 0.8 feature's build and the L6 layer green in CI; a negative control: the 0.8 glue's `to_html_with_buf` given the 0.3 signature fails the 0.8 job; a new Leptos 0.9 pre-release is picked up by the ordinary dependency update, with no plan change |
| **A1** `mf2 convert --from fluent` | The one-shot converter of [05](05-tooling.md) §6, in `mf2-cli`, parsing Fluent with the **`fluent-syntax`** crate (an existing, maintained parser — no parser of our own, so D1's rule does not arise). `.ftl` files in, `.mf2` resources out in the layout `mf2 init` makes, in `mf2 fmt`'s canonical form; Fluent comments (`#`, `##`, `###`) kept as the resource's comments — a group comment as a detached comment, not a `[section]`, since a section would rename the ids under it (05 §6.1). **The mapping table**, written into 05 §6.1 before the code (*written 2026-09-24*), with a stable code for every construct it cannot map. A report of those codes, as text and `--format json`, and a non-zero exit when one is found. `fluent-syntax` is a dependency of `mf2-cli` only — never of `mf2-build`, so no application's build script gains it. | the mapping written; a hand-written **construct corpus** under `crates/mf2-cli/tests/` holding every entry and expression kind of `fluent-syntax`'s AST, each converted or reported with its code, one test per code; A2's corpus converts with **zero** unmapped constructs; the converted resources pass `mf2 check` with no error; negative control: a construct removed from the mapping is reported, and the test naming it fails |
| **A2** A Fluent corpus of reference-workload shape | `bench/workload-gen` writes `.ftl` beside `.mf2` from the same model and seed: the same messages, ids, argument names and plural selections, one `.ftl` per source file per locale, pseudo-locales included. Fluent's plural selection on a count is `{ $n -> [one] … *[other] … }`; message text is identical. What the workload has and Fluent lacks — markup — follows A5's like-with-like rule (below). | `workload-gen --format ftl` reproducible (same seed, same bytes); the 6,400-message reference output parses with `fluent-syntax` with no `Junk`; `stats` on it matches 06 §2's table |
| **A3** Converted catalogs format as the Fluent originals | A harness, same process, formats each original with **`fluent-bundle`** (a dev-only oracle, never a runtime dependency) and each converted message with `mf2` from a catalog `mf2-build` wrote, over a **sampled argument set**: strings (Latin, RTL, empty), integers at every plural boundary of each locale's rules (0, 1, 2, 3, 5, 11, 21, 100, 101, …), negatives, decimals, large values. Isolation compared as a setting, both sides on and both off. *Measured before building:* where the two differ by construction — `fluent-bundle`'s `NUMBER` does not group digits or localize symbols, where `:number` with `fn-number` does; its implicit number formatting of a numeric argument; its FSI/PDI placement against the Default Bidi Strategy. **Each class of difference found goes to the owner before the exit is claimed**, with the counts, since it decides what "identically" means in the master plan's exit. | every sampled pair equal on A2's corpus and on the construct corpus's mapped part, or each difference in an owner-approved class with its count; negative control: a deliberately wrong mapping (a plural key dropped) fails the harness naming the message and the argument |
| **A4** The call-site migration | `mf2 convert --from leptos-fluent DIR`: A1 on the project's `.ftl` files, then the Rust call sites. A **codemod where mechanical** (master plan §9 P8), by byte-range edits located with `syn` and `proc-macro2` span locations, so the rest of a file is untouched: `tr!("id")` and `move_tr!("id")` → `tr!("id")`; a `{ "name" => value, … }` argument map → `name = value, …`; the context-first form (`tr!(i18n, …)`) → the context dropped; `move \|\| tr!(…)` in a view position, when the closure body is exactly the macro → `tr!(…)`; a Fluent attribute reference → the `id.attr` id A1 gave it. The macro forms are checked against `leptos-fluent`'s documentation on docs.rs before the rules are written. Everything else — the `leptos_fluent!` initializer, `I18n` context uses, language selectors — is reported with file:line:column, not guessed. *(Designed: 05 §6.2, 2026-09-24. An argument name that is not a Rust identifier is written quoted, `"a-b" = value`, which `tr!` accepts — mechanical, so not reported.)* A diff by default, `--write` to apply. **The guide**, `docs/migrating-from-leptos-fluent.md`: what the command does, what it reports and how to finish by hand (initializer → `setup()` / `leptos_mf2::install` / `mf2_axum::install`; the language selector → `<LocaleSwitcher>`; the per-position idiom of [call sites](../docs/call-sites.md)). Its *after* samples are compiled by `cargo xtask docs`; its *before* samples are not (`leptos-fluent` is not a dependency of the workspace), which `cargo xtask docs` gains an explicit attribute for rather than an exemption. | one test per rewrite rule and per report; the `leptos-fluent` reference application of A5, converted by the command and built, has **the same call sites as the `tr-view` template generates for the same seed** (differences only where documented — *as designed:* compared token by token, since `rustfmt` does not format inside `view!`, against a template `fluent-converted` that is `tr-view` with each documented difference written as its own row); `cargo xtask docs` green with the guide; negative control: a rule disabled leaves its sites reported and the comparison fails |
| **A5** The `leptos-fluent` A/B — once, as a snapshot | *Owner question 4; master plan §9 P8; 06 §6.* A workload-gen template **`fluent-view`**: the reference application on the newest `leptos-fluent` release for the workspace's Leptos line, each call-site shape written in `leptos-fluent`'s own idiom for it (`tr!` where a `String` is wanted, `move_tr!` where reactive text is), its messages A2's `.ftl`. It lives in a workspace of its own, **`bench/fluent-ab/`** (as `bench/churn` does), so `leptos-fluent` never enters the main resolve. The mf2 side is **the same application converted by A1 + A4**, not a hand-written twin. **Like with like:** both sides render the same text at every site; a site whose message has markup is written the way an application without markup must (the sentence split around the element, 04 §11 item 1) on the Fluent side, and the snapshot states how many sites that is. `cargo xtask fluent-ab` builds both with the profile and `wasm-opt` flags `cargo xtask size` uses and measures: **size** — wasm and JS raw / gz / br, what a first visit downloads in one locale (wasm + JS + that locale's text), and what each added locale costs; **speed** — time from the wasm's load to the first translated frame, a simple, a one-argument and a selecting format, and a switch with 2,000 live nodes, in Chromium and Firefox, the binaries run **alternately** (the development machine's clock drifts), load noted. A browser check first asserts that both applications show the same text at a sample of sites in two locales, so neither side is measured with its translations optimized away. **The snapshot:** `bench/fluent-ab/SNAPSHOT.md` and `snapshot.json` — the commit measured, the versions (`leptos-fluent`, `fluent-bundle`, Leptos, rustc, `wasm-opt`), the date, the machine's load, the command, every figure. Not in CI and not re-run per commit; re-run at a later commit only when the owner asks. | the same-text check green on both sides; the snapshot committed, in a commit that names the commit it measured; 06 §6 and the master plan's §6 ambition line point to it |
| **A6** XLIFF 2 export and import | *Owner question 3.* **(a) Vendored:** `cargo xtask xliff-sync` fetches the newest OASIS Standard of the XLIFF 2 core — the specification and its XML schema(s) — into `third_party/xliff/` with a `PIN` (upstream, version, date, digests, licence). The licence is read **before** anything is copied; if it does not permit redistribution, the `PIN` records that and the files are cached under `target/xtask-cache/` instead, as `third_party/w3c-message-resource` does for its draft. **(b) The mapping, designed before code** in a new section of 05 (as Phase 7 A14 did): a message ↔ a `<unit>`; text ↔ `<segment>` `<source>` / `<target>`; placeholders and markup ↔ inline codes the translation tool protects, round-tripping the expression exactly; `@param`, comments and `@do-not-translate` ↔ notes and `translate="no"`; and the hard case — a `.match` message whose **target locale has other plural categories than the source** (English one/other, Arabic six) — decided and written down. A choice there that changes what a translator sees is put to the owner. **(c) Built:** `mf2 export --format xliff` and `mf2 import` of an XLIFF file, beside flat JSON, with `import`'s existing rule (the container keeps every section, comment and property; an unknown id is reported, not invented). | the design written; the reference workload exported and imported back leaves every resource **byte-identical**; every export validates against the vendored schema (`xmllint --schema` in the test, on the development machine and in CI's image); a translated target lands in the right message and variant; an edited protected code is refused with a report; negative controls for the validation and the refusal |
| **A7** The reader's time zone | *Owner question 2; [03](03-runtime.md) §6.* **Designed before code** in 03 §6 and [04](04-leptos-integration.md) §6: the cookie (`mf2_tz`, the IANA name from `Intl.DateTimeFormat().resolvedOptions().timeZone`, `CookieLocale`'s attributes), validated with `TimeZone::named`; `mf2-axum` puts it in the request's formatting context; the page states the zone it was rendered in, so the client knows whether to correct; which nodes are corrected after hydration (those whose message formats an instant without a zone of its own) and how, without a hydration mismatch (the text changes *after* hydration, through the registry, as a switch does); the order of precedence among the value's own zone, the reader's, `Setup::with_time_zone` and UTC; client-only (the reader's zone at mount, no cookie needed); islands and `static-locale` (what can be corrected, and what the documentation says is not). Every added byte behind `fn-datetime`. | a native test: the cookie sets the request's zone, a malformed or unknown one is ignored; a browser check on `demo-ssr` with Playwright's `timezoneId` in two zones, two engines: a first visit served in UTC is corrected after hydration with no `mf2:` mismatch, the cookie is written, a reload is served in the reader's zone and nothing changes after hydration; `demo-csr` mounts in the reader's zone; negative control: the correction disabled fails the first-visit assertion; `cargo xtask size` with B1 unmoved (the gated build has no `fn-datetime`), the example's delta with it recorded; `docs/call-sites.md` §"Dates" rewritten |
| **A8** The Phase 9 work order | Written from Phase 8's findings into `plans/17-phase-9-work-order.md`, with `plans/phase-8-results.md`. | written |

## A0 — Leptos 0.9 the default: what was built

* **The switch** (04 §3, §10 "As built"): `leptos-mf2` and `mf2-axum` have
  `leptos-0-9` (default) and `leptos-0-8`; 0.8 is `default-features =
  false, features = ["leptos-0-8"]` on both. One source, both lines in one
  lock file (the 0.8 crates renamed back at each crate root); both lines
  or none is a `compile_error!` naming the fix. `tachys-0-3` is gone.
* **Moved to 0.9.0-beta / tachys 0.3.0-beta:** the workspace, the three
  examples, `conformance/l6-web`, `conformance/l7-web` and its sets,
  `bench/churn`, the workload generator's apps (a template may say
  `leptos = "0.8"`; `fluent-view` and `fluent-converted` do, since
  `leptos-fluent` 0.3.1 requires Leptos < 0.9) and the docs' samples.
  The requirement `0.3.0-beta` already resolves to `0.3.0-beta2`: a new
  pre-release is taken by `cargo update`, with no plan change.
* **Found on the way — Leptos 0.9 hydrates a lazy route only with its new
  `lazy` feature** (`leptos/lazy` → `tachys/lazy`; without it tachys
  panics when hydration reaches the route, and `demo.mjs`/`lazy.mjs`
  timed out). 0.8 has no such feature. It is the application's to turn
  on, so an application without lazy routes does not pay for it:
  `demo-ssr`, the generated reference application (0.9 templates) and
  `docs/delivery-modes.md` §"Lazy routes" (a compiled `merge` block) now
  do; `hydrate_lazy`'s documentation says so.
* **`cargo xtask leptos-0-8`** (was `leptos-beta`): both lines at once
  refused with the fix named; on 0.8, `leptos-mf2` linted for ssr,
  hydrate and csr, its `render`, `churn` and `fallback_lang` tests,
  `mf2-axum`'s tests and layer L6. Its nightly job is not allowed to
  fail. `--negative-control` (the 0.9 `to_html_with_buf` under
  `leptos-0-8`) fails with E0050/E0425. `cargo xtask docs` also compiles
  Getting started's application on 0.8 (`hello-0-8`) from the page's own
  instructions.
* **Re-run on 0.9** (2026-09-25, Chromium and Firefox, debug builds):
  `cargo xtask ci` green; `cargo xtask docs` 7 applications; L6 20/20;
  L7 444/444, L7c 444/444, L7d and L7cd 325/444 with the ledger's 119
  documented degradations (the ledger's columns hold); churn 84/84, no
  shape grows the heap; `demo.mjs` 170/170, `lazy.mjs` 66/66,
  `islands.mjs` 58/58, `csr.mjs` 88/88, `a11y.mjs` 720/720;
  `fluent-migrate` and `islands-zero` green. The nightly CI jobs
  themselves were not run (nothing is pushed).
* **Size** (`cargo xtask size`, taken under load — sizes are
  load-independent): B1 22,102 → **25,891 B gz** (+3,789), B5 12.6 →
  **8.4 B gz** a site, the whole app at 1,860 sites 45,517 → **41,511 B
  gz** (−4,006) — all met. The change between the two measurements is
  the Leptos line (tachys 0.3), plus `leptos/lazy`, which is on in all
  three of the gate's templates; the two were not measured apart. The
  islands example's client (`islands-zero`): code section 185,925 →
  165,151 B, shipped 94,904 → 85,446 B gz.

## A1 — the Fluent converter: what was built

* **`mf2 convert --from fluent DIR`** in `crates/mf2-cli/src/convert.rs`
  (discovery, layout, never overwriting), `convert/fluent.rs` (inlining and
  selectors), `convert/fluent/hoist.rs` (the `.match` product),
  `convert/fluent/number.rs` (a field-for-field replica of `fluent-bundle`'s
  `FluentNumberOptions` and its `merge`, because key reachability is
  `FluentNumber` equality) and `convert/fluent/datetime.rs`. Messages are
  built as `mf2-model` values and written by `mf2-syntax`'s serializer, so
  escaping and the `{{…}}` quoting of a leading `.` are the serializer's; a
  debug assertion reads every message back and compares.
* **The construct corpus**, `crates/mf2-cli/tests/fluent/constructs/`: every
  entry and expression kind of `fluent-syntax` 0.12's AST, three locales
  (`pt_BR` for the tag rule, `fr` for `*[0]` before `[one]`), a
  subdirectory. Its expected output is `constructs.expected/`; the only
  findings are the seven `fluent-datetime-approximate` warnings its
  `DATETIME`s must give. The output passes `mf2 check --features
  fn-number,fn-datetime` with no error and `mf2 fmt --check` with no change.
* **Tests:** one per code, named after it (`tests/convert.rs`; a unit test
  fails if a code of `report.rs` has no test or is not in 05 §6.1); the
  golden comparison and **the negative control** in `src/convert.rs`
  (`NUMBER` removed from the mapping: its sites are reported as
  `fluent-unknown-function` and the comparison fails); never overwriting;
  errors leaving only their entry out.
* **Decisions the mapping did not state**, now in 05 §6.1 (*as built*): a
  `NUMBER` selector keeps only `select=ordinal` and `minimumFractionDigits`;
  a select with only a default stays a one-variant `.match` so its argument
  survives; a number key that is not an MF2 `number-literal` is written as
  its value; comment tabs become spaces; `fluent-junk` also covers U+0000.
  Two more known differences for A3: a selector's `.input` formats a bare
  `{$n}` through it, and `fluent-bundle`'s term-in-term argument reset is
  not reproduced.

## A2 — the Fluent corpus: what was built

* **`workload-gen --format ftl`** (`all`, `locales`; `--format mf2,ftl` for
  both) writes `ftl/<tag>/<ns>.ftl` from the same message bodies the
  `.mf2` files render (`bench/workload-gen/src/fluent.rs`; the locales now
  produce bodies once, `locale::bodies`). The `.mf2`, JSON and app output
  is byte-identical to the previous commit's (checked at three knob sets
  against a binary built from it).
* **Decisions the task did not state** (the generator's README, §Fluent):
  a Fluent id is the dotted id with `-` for `.` (Fluent identifiers have no
  `.`; generation fails if two ids meet) and a section is a group comment
  naming it; a sentence with an element is **split into one message with
  three attributes**, `.before`, `.<element>`, `.after` (always all three,
  `{ "" }` when empty, trimmed at the split — the view puts the element and
  the spaces), 8 of 1,600 messages; `@param` becomes the Fluent
  `# Variables:` comment block; comments are re-sized so they are 60 % of
  the Fluent `en` bytes.
* **`stats --format ftl`** (and `--ftl DIR` for files on disk) parses every
  file with `fluent-syntax` — an error or `Junk` fails it — and measures 06
  §2's table on the parse. Seed 1: every checked row in tolerance (1 / 2 /
  3 / 4 variables 14.69 / 5.00 / 1.12 / 0.19 %, selects 0.88 %, text mean
  27.77 B, id mean 23.53, comments 60.04 % of `en`).
* **Converted** (`crates/mf2-cli/tests/convert.rs`,
  `the_reference_workload_converts_with_nothing_unmapped`): four locales,
  6,464 entries (1,600 − 8 + 3 × 8 each), **no error and no warning**;
  `mf2 check` 0 errors (the same 18 warnings the `.mf2` workload has);
  `mf2 fmt --check` unchanged. Exported, 1,578 messages per locale are the
  `.mf2` source byte for byte; the 14 selects differ only in declaring
  `:number` for `:integer`; the 8 split sentences are their parts. With
  `--number 20 --datetime 10`, the report is the 40
  `fluent-datetime-approximate` warnings the mapping promises, nothing else.
* **For the tasks after it:** A3 — a converted select annotates `$count`
  `:number`, not `:integer`, so a decimal count formats differently in its
  variant text (in the sampled argument set). A4 — the converted call
  sites name the Fluent ids (`chat-input-send`, and `….before` / `….after`
  around an element), not `tr-view`'s dotted ids: a documented difference
  of the comparison. A5 — the B6 grep pattern for the canary id is dotted;
  on either side of the A/B the id is `app-canary-zq7-canary-msg`.

## A3 — the `fluent-bundle` oracle: what was built

* **`crates/mf2-cli/tests/fluent_oracle.rs`**, in one process: each
  original formatted with `fluent-bundle` 0.16.0 (a dev-dependency of
  `mf2-cli`, with `unic-langid` for its locale; nothing else depends on
  it), each converted message with `mf2` from catalogs `mf2-build` wrote
  from `mf2 convert --from fluent`'s output. Corpora: A2's reference
  workload (1,616 entries × 4 locales) and the construct corpus's mapped
  part. Settings: isolation on both sides / off on both, and the catalogs
  built with no function feature (the core `:number`) or with `fn-number`
  (the construct corpus needs it, so it has only the latter). 8 s, debug.
* **The argument set.** A variable Fluent reads as a number (a `NUMBER`
  operand, or a bare selector with a number or category key) gets 41
  numbers: integers at the plural boundaries of `en`, `pl`, `fr`, `pt` and
  `ar` (0 … 6, 10 … 15, 21, 22, 25, 99 … 103, 111, 112, 1000, 1001, 12345,
  10⁶, 1234567, 12345678901), negatives, decimals (0.5, 1.5, 2.25, −1.5,
  0.1, 1234.5, 1e−7, 1.0004). Any other variable gets those and three
  strings (Latin, RTL, empty). Each variable sees every value of its set.
  A `DATETIME` message has no oracle (7 units of the construct corpus,
  counted).
* **Classification** (the test's module comment): a pair that differs is
  explained by isolation marks removed; else by numbers read as tokens
  (and a currency mark beside one); else, for a different variant, by one
  of two causes each **checked**, not assumed — Fluent given the value
  rounded as `:number` rounds it agrees, or a probe message on each side
  puts the number in different plural categories. Anything else is
  *unexplained* and fails the test, as does a class outside the approved
  list. **Negative control:** a Polish `one` variant dropped from one
  converted message fails it, naming the message and `$count=Int(1)`.
* **Counts** (pairs; 64,088 per workload setting, 868 per construct
  setting). *Workload, core:* 42 number rendering (a decimal of more than
  three places shown rounded), 70 selection by a rounded decimal, and with
  isolation on 56,204 isolation. *Workload, `fn-number`:* 17,242 number
  rendering (grouping 11,093; decimal separator 2,951; grouping + decimal
  separator 500; a sign with a bidi mark in `ar-XB` 1,312; grouping + sign
  798; at a selector's placeholder 588), 70 selection by a rounded decimal,
  with isolation on 39,004 isolation. *Constructs, `fn-number`:* 238
  number rendering (`NUMBER` options, percent and currency that
  `fluent-bundle` does not apply among them), 9 selection by a rounded
  decimal, 1 cardinal rules (`fr` 1,000,000: `many` in current CLDR,
  `other` in `fluent-bundle`), 5 ordinal of a non-integer (`en` 1.5:
  `one` there, `other` in CLDR), with isolation on 309 isolation. No pair
  unexplained.
* **Found beyond 05 §6.1's list** (now in it): the rounded-decimal
  selection, and the plural-rule differences. The workload's usual
  `*[other]` default hides the latter: only a select whose default is not
  `other` shows it.

## A4 — the call-site codemod: what was built

* **The rules, before code** (05 §6.2, from `leptos-fluent` 0.3.1's
  docs.rs pages — which also confirmed §6.1's `locales/<lang>/*.ftl`).
  A call is classified by where it stands: in a **view position** (a
  `view!` child or attribute/prop value that is the whole call) `tr!` and
  `move_tr!` become the description, `tr!(…)`; in a **`String` position**
  `tr!(…)` becomes `tr!(…).to_string()`; a `move_tr!` elsewhere, or with an
  argument that may change (not a literal or a path), becomes its own
  documented expansion `Signal::derive(move || tr!(…).to_string())`. Every
  type and every evaluation of an argument stays as it was. Nine rules,
  twelve report codes, every one an error ("a migration is finished when
  the report is empty").
* **`mf2 convert --from leptos-fluent APP_DIR`** in
  `crates/mf2-cli/src/convert/leptos_fluent.rs` (discovery: the `.ftl`
  directory from `--locales`, the initializer's `locales:`, or
  `APP_DIR/locales`; the source locale's converted messages, against which
  every call is checked; the diff by default, `--write` to apply),
  `…/call.rs` (the macro forms, parsed with `syn`) and `…/rewrite.rs` (the
  walk over `proc-macro2` tokens — every macro's arguments, `view!`
  included — and the byte-range edits). A file that does not name
  `leptos_fluent` is not touched, so a second run changes nothing. `mf2 init
  --no-messages` makes the translation crate without the starter messages
  a conversion would collide with.
* **Departures, recorded in 05 §6.2 and the table above:** an argument
  name that is not a Rust identifier is written quoted (`tr!` accepts it),
  not reported; the reference comparison is exact, against a template, not
  "after rustfmt" (which does not format inside `view!`).
* **The reference application.** Two templates in
  `bench/workload-gen/templates/`: `fluent-view` (the workload's call
  sites in `leptos-fluent`'s idiom, `leptos_fluent!` in a provider — a new
  template field, `provider`, wraps the router in it; ids by a new
  placeholder, `{{fluent_id}}`) and `fluent-converted` (`tr-view` with the
  five documented differences as its own rows: Fluent ids; unquoted
  argument names; an argument read from a signal keeps `move_tr!`'s
  `Signal::derive` expansion — 103 of 1,860 sites; deferred labels are
  `fn() -> String` closures — 149; a sentence with an element is its three
  messages — 8). Existing templates' output is byte-identical to the
  previous commit's (checked against a binary built from it).
  `fluent-view` type-checks against `leptos-fluent` 0.3.1 for the server
  and for wasm (checked once, at 120 sites, in a scratch directory; A5
  builds it for real).
* **Tests.** `cargo test -p mf2-cli`: the whole reference application,
  in memory, rewritten, **byte for byte** `fluent-converted`'s in every
  file with a call site (61 files), the only findings the initializer and
  the `use` beside it; **negative control:** with `move-tr-view` switched
  off its sites are reported (`leptos-fluent-call`) and the comparison
  fails. One test per rule (`rule_*`) and per code (`leptos_fluent_*`) in
  `tests/convert.rs`, each list checked against the enum and 05 §6.2.
* **`cargo xtask fluent-migrate`** (nightly, after the size gate): the
  same end to end through the command line on disk — the report must be
  exactly the initializer, its `use` and three manifest lines — then the
  hand-finishing the guide describes (the manifest, the initializer's
  module, the entry points, taken from `fluent-converted`), after which the
  migrated application is `fluent-converted` in every file, and the build:
  client (`wasm32`, `hydrate`) and server (`ssr`). Run 2026-09-24: green —
  the report as expected, 64 files byte for byte, both builds.
* **The guide**, `docs/migrating-from-leptos-fluent.md`: Getting started's
  application as it would be on `leptos-fluent`, the commands, the
  rewritten file, the report and each code's hand-finish, the finished
  file, and what changes for the reader (A3's accepted differences, terms
  copied). `cargo xtask docs` gained two attributes: **`before`** — code
  before a migration, never compiled, but only accepted in a project whose
  `run=` commands convert it, and written before they run, so the page's
  `generated` block must be exactly what `mf2 convert` wrote — and
  **`status=N`** for a `run=` block whose commands must exit with N (the
  conversion exits 1 while work is left). The finished application is
  checked for the server and for wasm like every other page's.

## A5 — the `leptos-fluent` A/B: what was built

* **`cargo xtask fluent-ab`** (`xtask/src/fluent_ab.rs`,
  `tools/e2e/checks/fluent-ab.mjs`, `bench/fluent-ab/README.md`). The
  reference workload with `fluent-view`; the mf2 side is a copy of it put
  through the guide's commands (`mf2 init --no-messages`, `mf2 convert
  --from leptos-fluent --write`, the report checked as `fluent-migrate`
  checks it) and finished as the guide says, from `bench/fluent-ab/mf2/`
  (manifest, entry points, server) and the shell edited in place. Both on
  **Leptos 0.8**: `leptos-fluent` 0.3.1 (newest, 2025-12-29) requires
  < 0.9 (§"Standing").
* **Departure, recorded in the README:** the sizes are the size gate's own
  pipeline, but the application the browser times is `cargo leptos build
  --release --split`. The generated applications' lazy routes are
  `wasm_split` imports (`__wasm_split_placeholder__`) that only the split
  step resolves, so the gate's unsplit client does not run in a browser —
  found on the first trial, where neither side hydrated. The snapshot
  gives both: the whole client from the gate's pipeline, and what a first
  visit to `/` downloaded of the split build.
* **Timing hooks** behind an `ab-bench` feature that the sized build
  leaves off: a `performance.mark` after hydration, `ab_mount`,
  `ab_format`, `ab_switch`, `ab_preload` — each side through its own
  library's API, the same message ids on both (chosen from the workload).
* **The same-text check** compares the whole hydrated `<main>` of `/`,
  `/r1`, `/r2`, `/r3` in `en` and `pl` (53,622 characters), bidi marks
  aside, with a control that it sees a locale difference; equal in both
  engines. 8 sites are the split sentence on both sides.
* **The snapshot:** `bench/fluent-ab/SNAPSHOT.md` and `snapshot.json`, of
  commit `391ef2a` (2026-09-25, Chromium 143 and Firefox 155, 10 runs a
  side, load 3.2, 60/60 assertions). The whole client: 975,597 → 613,952
  B gz wasm; a first visit in `en` 983,963 B gz on `leptos-fluent` against
  642,980 B gz (wasm + JS + the `en` catalog) on mf2; each added locale
  66,042 B gz in every visitor's wasm on `leptos-fluent`, a 21–27 KB gz
  catalog for its own readers only on mf2. Chromium medians: first frame
  after the wasm's load 158.8 → 129.8 ms; a simple format 1.18 → 0.09 µs,
  one argument 2.03 → 0.75, a select 3.14 → 2.41; a switch with 2,000
  live nodes 82.4 → 10.3 ms. Firefox the same way round (the snapshot has
  every figure, with its minimum and maximum).

## A6 — XLIFF 2: what was built

* **(a) Vendored** (2026-09-25): `cargo xtask xliff-sync [--list]
  [--check]` (`xtask/src/xliff_sync.rs`). `--list` reads OASIS's index:
  v2.0 and v2.1 have an `os` (OASIS Standard) stage, v2.2 only up to
  `cs01` — so the newest OASIS Standard is **XLIFF 2.1** (13 February
  2018; its core schema keeps the 2.0 namespace, `xliff_core_2.0.xsd`).
  The licence was read from the cache before anything was copied (the
  OASIS notice: verbatim copies with the notice, no modification) and is
  quoted in `third_party/xliff/PIN` with `redistribute = yes`; without
  that field the command stops after fetching. **Departure:** the files
  are taken from the release's ZIP, not fetched one by one — the `.html`
  served beside it differs on every request (the CDN rewrites each e-mail
  address), found when the first `--check` failed. The PIN pins the ZIP's
  and every vendored file's SHA-256; a changed digest is refused, and
  `--check` compares the vendored tree byte for byte (both negative
  controls run: a digest altered in the PIN, a vendored file edited).
  Vendored: the specification (`.html`, and the `.xml` OASIS calls
  authoritative), `schemas/` with W3C's `xml.xsd` the core schema imports;
  not `change_tracking.xsd` (no owner or notice, imported by nothing core)
  or the `.pdf`. 25 files, 1,953,918 bytes.
* **(b) Designed** (2026-09-25), 05 §6.3: core only (validates against
  `xliff_core_2.0.xsd` alone); a resource file ↔ `<file>`, a section ↔
  `<group type="mf2:section">`, a message ↔ `<unit>`, a `.match` message ↔
  `<group type="mf2:select">` with a unit per variant; every expression an
  inline code whose `<data>` is its exact MF2 text (`ph`; markup `pc`, or
  isolated `sc`/`ec`); comments, `@param` and other properties ↔ notes;
  `@do-not-translate` ↔ `translate="no"`; declarations and selectors not in
  the document at all. Import re-exports the tree in memory and refuses,
  per unit and with a code, what does not match it (six codes); an
  unchanged data model keeps its bytes. The hard case went to the owner
  (question 7): the target language's plural forms. `quick-xml` is the one
  new dependency, of `mf2-cli` only.

## Standing: Leptos 0.9

*Superseded by owner question 6 (2026-09-24):* 0.9 is the default now,
beta or not, and 0.8 an opt-in tested in CI — A0. A5 uses the newest
`leptos-fluent` for Leptos 0.9; if none exists, the A/B runs both sides on
the 0.8 feature and the snapshot says so. (Was: wait for 0.9's release,
then move and ask about 0.8.)

## Exit (master plan §9, P8)

- [x] Leptos 0.9 the default, 0.8 an opt-in built and tested in CI (A0)
- [x] a Fluent corpus of reference-workload shape converts with a report of
      zero unmapped constructs (A1, A2)
- [x] converted catalogs format identically to the Fluent originals on a
      sampled argument set — or differ only in classes the owner approved,
      each counted (A3; owner question 5)
- [x] the call-site codemod and the migration guide, the guide's samples
      compiled (A4)
- [x] the `leptos-fluent` A/B measured on the reference application,
      reported, and its snapshot committed with the commit it measured (A5)
- [ ] XLIFF 2 vendored, its mapping designed, export and import built and
      validated against the schema (A6)
- [ ] dates in the reader's time zone, asserted in a browser (A7)
- [ ] `cargo xtask ci` green; the conformance harness green at
      `current_phase = "P8"` (Phase 8 adds no layer, so every column stays
      as Phase 7 left it)
- [ ] `plans/phase-8-results.md` and the Phase 9 work order written (A8)
