# Phase 9 results — release

What Phase 9 built and what it measured, against
[17-phase-9-work-order](17-phase-9-work-order.md). Every figure comes with
the command that produced it; where a figure moved a budget or a ledger
status, the commit that moved it says why (master plan §11).

Each task's own record — what was built, how it was tested, what it found —
is in the work order under the task's heading (§A0 … §A7, §B1 … §B13).
This file gathers them at the phase exit: the status, the figures a reader
will look for first, the book's re-verification (B13's verdicts), what the
phase found and did not fix, and the master plan's "Later" list put in an
order for after the release.

## Status at exit

**Every exit criterion of the work order is met** (2026-09-28), and
`current_phase = "P9"` is in the exit commit with the harness green. One
caveat against the master plan's wording (§9 P9, "the release dry run
green in CI"): the dry run is green locally, and CI runs it in its
`release` job, but no runner has been attached to the repository, so that
job has never run. The publish itself is the owner's (owner question 4):
what stands between the tree and crates.io is the owner's `cargo xtask
release --publish`.

| Exit criterion | Verdict |
|---|---|
| the specification text out of the tree, fetched on demand; the suite still vendored | **met** (A0) — `cargo xtask spec-sync` fetches `spec/` into a git-ignored cache against 14 digests in the `PIN`; every reader refuses a missing or stale cache naming the command; nothing committed quotes the text. The old commits still hold it: rewriting them is the owner's, if the history is made public |
| the names verified and every package's metadata complete | **met** (A1) — 16 names at the time, 18 with `mf2-native` and `mf2-ratatui`; description, README, keywords, categories, licence (`MIT`; `MIT AND Unicode-3.0` for `mf2-locale-data`), exact `=` requirements between our crates, held by `xtask/src/packages.rs` |
| every public item reviewed, listed and documented; `with_zone` fixed; the unknown-option lint | **met** (A2) — `crates/<name>/api.txt` for each crate, held by `cargo xtask api --check` in CI |
| the version policy written, the MSRV measured and held in CI | **met** (A3) — `docs/versioning.md`; MSRV **1.88**, `cargo xtask msrv` and `--below` |
| the packages audited and verified from crates.io's point of view | **met** (A4) — `crates/<name>/package.txt`, `cargo xtask package --check --test`, `cargo publish --workspace --dry-run` |
| the documentation builds as docs.rs builds it | **met** (A5) — `cargo xtask docs-rs`, `-D warnings`, every front page names the user guide |
| the changelog with 1.0.0 and its known limitations | **met** (A6) — `CHANGELOG.md`, and 1.1.0 above it |
| `cargo xtask release` green as a dry run; the publish the owner's | **met** (A7) — green on the clean tree (304 s, 2026-09-25); the `release` job is in `ci.yml` but has never run on a runner. The owner's first publish (2026-09-26) put up five crates at 1.0.0 before crates.io's new-crate rate limit; the command now resumes such a publish. **1.1.0 is the first full release** (owner, 2026-09-27) |
| Part B: the book's refuted claims fixed in code or text, each with its guard | **met** (B1–B12) |
| Part B: the book re-verified by running it | **met** (B13, below) — every refuted row verified or now described as it behaves; three more sentences corrected |
| `cargo xtask ci` green; the harness green at `current_phase = "P9"` | **met** (2026-09-28) — see the exit commit |
| this file | **met** |

## The figures

| What | Figure | Command, date |
|---|---|---|
| MSRV | Rust **1.88** (the 18, native and `wasm32`, five feature sets); 1.87 refused | `cargo xtask msrv`, `cargo xtask msrv --below` (2026-09-28) |
| the promised API | 8,374 lines of `cargo public-api -s` before the review → 3,512 after (15 libraries and the CLI's command tree); 137 missing docs → 0 | `cargo xtask api` (A2, 2026-09-25) |
| the largest package | `mf2-locale-data`, 1,377,249 B (7,838,847 B unpacked), 14 % of crates.io's 10 MB | `cargo xtask package` (A4, 2026-09-25) |
| the release dry run | 304 s on the clean tree, warm caches | `cargo xtask release` (A7, 2026-09-25) |
| B1 / B5 / the whole app | 25,875 B gz / 8.4 B gz a site / 41,466 B gz at 1,860 sites; B7 `en` 18,072 B br. No budget moved in the phase; `cargo xtask ci`'s size gates held through Part B | `cargo xtask size`, `cargo xtask catalog-size` (A6, 2026-09-25) |
| a server-only component in an islands page | code 165,705 B / 864 functions with and without it, data 23,446 B both, 85,726 B gz both | `cargo xtask islands-zero` (B13, 2026-09-28) |
| conversions under churn | 84/84; no row shape grows the heap in Chromium or Firefox | `cargo xtask churn` (B13, 2026-09-28) |

## The book, re-verified (B13's verdicts)

On 2026-09-28 the corrected book was assembled by `cargo xtask docs`, its
sample applications built as a reader builds them (`cargo leptos build`,
`trunk build`, `cargo build`), and driven with Playwright in Chromium 143
and Firefox 155, curl, and native runs, with the review's variants (a
second build as "another deploy", the page's path-prefix samples pasted
into an app, islands without `static-locale`, lazy routes without
`--split`, `NativeFiles`). The full record, with what each probe asserted,
is the work order's §B13.

| Refuted in the review | Now |
|---|---|
| a catalog from another deploy during a switch reloads | **verified** — one navigation into the new language, the choice remembered, one `mf2:` line; at boot, one reload (13/13, both engines) |
| a lazy route behaves like the rest of the page | **verified** — counts and follows a switch loaded directly and after `<A href>` (8/8, both) |
| without `--split` a lazy route works the same | **as the book now says** — nothing hydrates (`__wasm_split_placeholder__`) |
| islands follow a live switch without `static-locale` | **as the book now says** — a `GET ?lang=`, the island's state starts again (7/7, both) |
| the migrated server honours the initializer's cookie | **as the book now says** — it does not until the report's extra source is added; then it does |
| the path-prefix negotiator with the switcher | **verified** — `303` to `/fr/…`; with and without the wasm the switch lands there (6/6, both) |
| a native catalog from another build is an error | **verified** — `ContentMismatch`, for a text-only and for a message-set change (the page had said `ManifestMismatch` for the second; corrected) |
| `msg_id!` + `TrDyn` for tools | **verified** — `TrDyn::new`, compiled sample and test |
| `class=` takes a message | **as the book now says** — E0277 for `class` and `style`; other attributes compile |
| `mf2 check` runs the build's checks | **verified** — the same report as the build on `hello` and on the date messages |
| islands-zero's figures | **re-measured** — no section grew; the page gives this run |

Every partly-right row holds as the book now words it (the work order
lists each). **Corrected by B13**, in text only: the `ManifestMismatch`
sentence (`native-apps.md`); "a `Secure` cookie is not stored over plain
HTTP" — both engines store it from `127.0.0.1`, and neither from another
address (`switching.md`); the islands-zero figures (`delivery-modes.md`).

## What the phase found and did not fix

| Finding | Where found | Goes to |
|---|---|---|
| the old commits hold the MF2 specification text, which may not be redistributed | A0 | the owner, if the history is ever made public (owner question 1) |
| item docs (not front pages) cite `plans/` documents, section numbers and phase codes, which docs.rs readers cannot follow (about 190 places at A5; 357 doc-comment lines match a looser search on 2026-09-28) | A5 | the post-1.0 order, first group |
| `mf2-locale-data`'s packaged test `tests/icu_blob.rs` pins the zones vector's length (16,857 B), which moves with every `icu_time_data` patch; with no lock file committed, CI and a package's own tests meet the next one as a failure | A0, left to A4 and not taken up there | the post-1.0 order, first group |
| a markup message with a signal argument follows it only when something around it re-runs; in a lazy route reached by a link it would re-run the route (read, not run) | B1 | the post-1.0 order, first group: run it, then decide what the book promises |
| `html_marked` checks only a text-only element's first child; a borrowed message after other text in a `<textarea>` would still be wrapped (no sample has one) | B6 | recorded |
| Chromium keeps a bare `?` in the address after `?lang=` is removed | B2, B13 | recorded; harmless to the server |
| the `release` CI job has never run on a runner | A7 | the owner's CI; the dry run is green locally |
| WebKit and a screen reader were not run; a date inside an island is not asserted in a browser | Phases 7–8, unchanged | the changelog's known limitations; the post-1.0 order, first group |

## What follows v1 — the "Later" list in order

The master plan's §9 "Later" list, reviewed into the order below
(proposed at A8, 2026-09-28; the owner may reorder). None of it needs a
2.0: the catalog format and the manifest are outside 1.x's promise
(`docs/versioning.md`), so even a new catalog container is a minor
release, as long as a server and its client are built together.

Outside the order, and first when they happen: the owner's publish of
1.1.0, and Leptos 0.9's release, taken as a patch (A3's policy).

1. **Hygiene after the release** (patches). The item docs' plan
   citations rewritten for docs.rs readers; the zones vector test made
   robust to time-zone data releases; the markup-with-a-signal case run
   and the book's promise decided; a date inside an island asserted in a
   browser; WebKit and a screen reader when a machine has them. Small,
   and each is something a user of 1.1.0 can meet.
2. **Dev hot reload** — `mf2 watch` pushing a recompiled catalog to open
   pages (owner, 2026-09-24: after v1). The rebuild loop is 4–5 s today
   (Phase 7 A6); this is the largest day-to-day improvement for a
   translator or a developer, is additive, and needs no format change.
3. **Editor tooling** — LSP diagnostics from `mf2-syntax` (its analysis
   and detail codes exist and are promised API), then a tree-sitter
   grammar. Additive, separate crates or repositories.
4. **Per-route catalog chunks** — the `MsgId` chunk bits are reserved
   (D6). The largest size lever left for a big application with lazy
   routes; it needs the reference workload measured per route first, and
   a gate against today's one-catalog-per-locale.
5. **A lazy `JsString` cache** — form A of
   [prob_builtin_strings](stretch_goals_after_v1/prob_builtin_strings.md):
   client-only, no format change, the four seams v1 kept; measured
   against its own gate before it is kept.
6. **ICU MessageFormat 1 import** — a second migration path beside
   Fluent, in `mf2 convert`. Additive.
7. **Server push of catalog updates in production** — after (2), which
   builds the mechanism for development; it needs a design for how a
   pushed catalog relates to the manifest hash a page was built with.
8. **Narrower NAMES references** —
   [names_reference_width](stretch_goals_after_v1/names_reference_width.md):
   up to 1.1 % of a catalog's brotli size, for a new `format_version`;
   worth doing only together with another format change.
9. **A catalog v2 container with imported string constants** — form B of
   [prob_builtin_strings](stretch_goals_after_v1/prob_builtin_strings.md):
   a format change that depends on browser support, and only if (5)'s
   measurement says the strings are worth it.

## Owner questions of the phase

Fourteen, all answered and recorded in the work order (§"Owner
questions") and the documents each changes: five about the release (the
specification text downloaded on demand; publish on the Leptos 0.9 beta;
1.0.0; the owner presses publish; only what applications use is
promised), and nine from the book's verification (the fixes folded into
1.1.0; `TrDyn::new`; the switcher learns URLs; `class=` a documented limit;
a switch reloads on deploy skew; the cookie only on an explicit choice;
`mf2 check` asks cargo for the features; `mf2 fmt` keeps some blank lines;
all four book additions).
