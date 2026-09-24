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
  2026-09-24), tracked nightly by `cargo xtask leptos-beta`. See §"Standing".

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

## Part A — tasks (A1 and A2 first, then A3; A4 after A1; A5 after A1, A2 and A4; A6 and A7 as convenient; A8 last)

| Task | Deliverable | Done when |
|---|---|---|
| **A1** `mf2 convert --from fluent` | The one-shot converter of [05](05-tooling.md) §6, in `mf2-cli`, parsing Fluent with the **`fluent-syntax`** crate (an existing, maintained parser — no parser of our own, so D1's rule does not arise). `.ftl` files in, `.mf2` resources out in the layout `mf2 init` makes, in `mf2 fmt`'s canonical form; Fluent comments (`#`, `##`, `###`) kept as the resource's comments and sections. **The mapping table**, written into 05 §6 before the code (below), with a stable code for every construct it cannot map. A report of those codes, as text and `--format json`, and a non-zero exit when one is found. `fluent-syntax` is a dependency of `mf2-cli` only — never of `mf2-build`, so no application's build script gains it. | the mapping written; a hand-written **construct corpus** under `crates/mf2-cli/tests/` holding every entry and expression kind of `fluent-syntax`'s AST, each converted or reported with its code, one test per code; A2's corpus converts with **zero** unmapped constructs; the converted resources pass `mf2 check` with no error; negative control: a construct removed from the mapping is reported, and the test naming it fails |
| **A2** A Fluent corpus of reference-workload shape | `bench/workload-gen` writes `.ftl` beside `.mf2` from the same model and seed: the same messages, ids, argument names and plural selections, one `.ftl` per source file per locale, pseudo-locales included. Fluent's plural selection on a count is `{ $n -> [one] … *[other] … }`; message text is identical. What the workload has and Fluent lacks — markup — follows A5's like-with-like rule (below). | `workload-gen --format ftl` reproducible (same seed, same bytes); the 6,400-message reference output parses with `fluent-syntax` with no `Junk`; `stats` on it matches 06 §2's table |
| **A3** Converted catalogs format as the Fluent originals | A harness, same process, formats each original with **`fluent-bundle`** (a dev-only oracle, never a runtime dependency) and each converted message with `mf2` from a catalog `mf2-build` wrote, over a **sampled argument set**: strings (Latin, RTL, empty), integers at every plural boundary of each locale's rules (0, 1, 2, 3, 5, 11, 21, 100, 101, …), negatives, decimals, large values. Isolation compared as a setting, both sides on and both off. *Measured before building:* where the two differ by construction — `fluent-bundle`'s `NUMBER` does not group digits or localize symbols, where `:number` with `fn-number` does; its implicit number formatting of a numeric argument; its FSI/PDI placement against the Default Bidi Strategy. **Each class of difference found goes to the owner before the exit is claimed**, with the counts, since it decides what "identically" means in the master plan's exit. | every sampled pair equal on A2's corpus and on the construct corpus's mapped part, or each difference in an owner-approved class with its count; negative control: a deliberately wrong mapping (a plural key dropped) fails the harness naming the message and the argument |
| **A4** The call-site migration | `mf2 convert --from leptos-fluent DIR`: A1 on the project's `.ftl` files, then the Rust call sites. A **codemod where mechanical** (master plan §9 P8), by byte-range edits located with `syn` and `proc-macro2` span locations, so the rest of a file is untouched: `tr!("id")` and `move_tr!("id")` → `tr!("id")`; a `{ "name" => value, … }` argument map → `name = value, …`; the context-first form (`tr!(i18n, …)`) → the context dropped; `move \|\| tr!(…)` in a view position, when the closure body is exactly the macro → `tr!(…)`; a Fluent attribute reference → the `id.attr` id A1 gave it. The macro forms are checked against `leptos-fluent`'s documentation on docs.rs before the rules are written. Everything else — the `leptos_fluent!` initializer, `I18n` context uses, language selectors, an argument name that is not a Rust identifier — is reported with file:line:column, not guessed. A diff by default, `--write` to apply. **The guide**, `docs/migrating-from-leptos-fluent.md`: what the command does, what it reports and how to finish by hand (initializer → `setup()` / `leptos_mf2::install` / `mf2_axum::install`; the language selector → `<LocaleSwitcher>`; the per-position idiom of [call sites](../docs/call-sites.md)). Its *after* samples are compiled by `cargo xtask docs`; its *before* samples are not (`leptos-fluent` is not a dependency of the workspace), which `cargo xtask docs` gains an explicit attribute for rather than an exemption. | one test per rewrite rule and per report; the `leptos-fluent` reference application of A5, converted by the command and built, has **the same call sites as the `tr-view` template generates for the same seed** (compared after `rustfmt`, differences only where documented); `cargo xtask docs` green with the guide; negative control: a rule disabled leaves its sites reported and the comparison fails |
| **A5** The `leptos-fluent` A/B — once, as a snapshot | *Owner question 4; master plan §9 P8; 06 §6.* A workload-gen template **`fluent-view`**: the reference application on the newest `leptos-fluent` release for the workspace's Leptos line, each call-site shape written in `leptos-fluent`'s own idiom for it (`tr!` where a `String` is wanted, `move_tr!` where reactive text is), its messages A2's `.ftl`. It lives in a workspace of its own, **`bench/fluent-ab/`** (as `bench/churn` does), so `leptos-fluent` never enters the main resolve. The mf2 side is **the same application converted by A1 + A4**, not a hand-written twin. **Like with like:** both sides render the same text at every site; a site whose message has markup is written the way an application without markup must (the sentence split around the element, 04 §11 item 1) on the Fluent side, and the snapshot states how many sites that is. `cargo xtask fluent-ab` builds both with the profile and `wasm-opt` flags `cargo xtask size` uses and measures: **size** — wasm and JS raw / gz / br, what a first visit downloads in one locale (wasm + JS + that locale's text), and what each added locale costs; **speed** — time from the wasm's load to the first translated frame, a simple, a one-argument and a selecting format, and a switch with 2,000 live nodes, in Chromium and Firefox, the binaries run **alternately** (the development machine's clock drifts), load noted. A browser check first asserts that both applications show the same text at a sample of sites in two locales, so neither side is measured with its translations optimized away. **The snapshot:** `bench/fluent-ab/SNAPSHOT.md` and `snapshot.json` — the commit measured, the versions (`leptos-fluent`, `fluent-bundle`, Leptos, rustc, `wasm-opt`), the date, the machine's load, the command, every figure. Not in CI and not re-run per commit; re-run at a later commit only when the owner asks. | the same-text check green on both sides; the snapshot committed, in a commit that names the commit it measured; 06 §6 and the master plan's §6 ambition line point to it |
| **A6** XLIFF 2 export and import | *Owner question 3.* **(a) Vendored:** `cargo xtask xliff-sync` fetches the newest OASIS Standard of the XLIFF 2 core — the specification and its XML schema(s) — into `third_party/xliff/` with a `PIN` (upstream, version, date, digests, licence). The licence is read **before** anything is copied; if it does not permit redistribution, the `PIN` records that and the files are cached under `target/xtask-cache/` instead, as `third_party/w3c-message-resource` does for its draft. **(b) The mapping, designed before code** in a new section of 05 (as Phase 7 A14 did): a message ↔ a `<unit>`; text ↔ `<segment>` `<source>` / `<target>`; placeholders and markup ↔ inline codes the translation tool protects, round-tripping the expression exactly; `@param`, comments and `@do-not-translate` ↔ notes and `translate="no"`; and the hard case — a `.match` message whose **target locale has other plural categories than the source** (English one/other, Arabic six) — decided and written down. A choice there that changes what a translator sees is put to the owner. **(c) Built:** `mf2 export --format xliff` and `mf2 import` of an XLIFF file, beside flat JSON, with `import`'s existing rule (the container keeps every section, comment and property; an unknown id is reported, not invented). | the design written; the reference workload exported and imported back leaves every resource **byte-identical**; every export validates against the vendored schema (`xmllint --schema` in the test, on the development machine and in CI's image); a translated target lands in the right message and variant; an edited protected code is refused with a report; negative controls for the validation and the refusal |
| **A7** The reader's time zone | *Owner question 2; [03](03-runtime.md) §6.* **Designed before code** in 03 §6 and [04](04-leptos-integration.md) §6: the cookie (`mf2_tz`, the IANA name from `Intl.DateTimeFormat().resolvedOptions().timeZone`, `CookieLocale`'s attributes), validated with `TimeZone::named`; `mf2-axum` puts it in the request's formatting context; the page states the zone it was rendered in, so the client knows whether to correct; which nodes are corrected after hydration (those whose message formats an instant without a zone of its own) and how, without a hydration mismatch (the text changes *after* hydration, through the registry, as a switch does); the order of precedence among the value's own zone, the reader's, `Setup::with_time_zone` and UTC; client-only (the reader's zone at mount, no cookie needed); islands and `static-locale` (what can be corrected, and what the documentation says is not). Every added byte behind `fn-datetime`. | a native test: the cookie sets the request's zone, a malformed or unknown one is ignored; a browser check on `demo-ssr` with Playwright's `timezoneId` in two zones, two engines: a first visit served in UTC is corrected after hydration with no `mf2:` mismatch, the cookie is written, a reload is served in the reader's zone and nothing changes after hydration; `demo-csr` mounts in the reader's zone; negative control: the correction disabled fails the first-visit assertion; `cargo xtask size` with B1 unmoved (the gated build has no `fn-datetime`), the example's delta with it recorded; `docs/call-sites.md` §"Dates" rewritten |
| **A8** The Phase 9 work order | Written from Phase 8's findings into `plans/17-phase-9-work-order.md`, with `plans/phase-8-results.md`. | written |

## Standing: Leptos 0.9

If Leptos 0.9 is released during Phase 8, `cargo xtask leptos-beta` says so
(a pin resolving to a release). D10 then moves the workspace to it, and
whether 0.8 stays supported is put to the owner (04 §10), before any other
task continues. Until then the nightly job keeps the 0.9 glue honest.

## Exit (master plan §9, P8)

- [ ] a Fluent corpus of reference-workload shape converts with a report of
      zero unmapped constructs (A1, A2)
- [ ] converted catalogs format identically to the Fluent originals on a
      sampled argument set — or differ only in classes the owner approved,
      each counted (A3)
- [ ] the call-site codemod and the migration guide, the guide's samples
      compiled (A4)
- [ ] the `leptos-fluent` A/B measured on the reference application,
      reported, and its snapshot committed with the commit it measured (A5)
- [ ] XLIFF 2 vendored, its mapping designed, export and import built and
      validated against the schema (A6)
- [ ] dates in the reader's time zone, asserted in a browser (A7)
- [ ] `cargo xtask ci` green; the conformance harness green at
      `current_phase = "P8"` (Phase 8 adds no layer, so every column stays
      as Phase 7 left it)
- [ ] `plans/phase-8-results.md` and the Phase 9 work order written (A8)
