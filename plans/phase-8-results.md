# Phase 8 results — migration and interchange

What Phase 8 built and what it measured, against
[16-phase-8-work-order](16-phase-8-work-order.md). Every figure comes with
the command that produced it; where a figure moved a budget or a ledger
status, the commit that moved it says why (master plan §11).

Each task's own record — what was built, how it was tested, what it found —
is in the work order under the task's heading (§A0 … §A7). This file gathers
them at the phase exit: the status, the figures a reader will look for
first, what the phase found that it did not fix, and where each finding goes.

## Status at exit

| Exit criterion | Verdict |
|---|---|
| Leptos 0.9 the default, 0.8 an opt-in built and tested in CI | **met** (A0) — `leptos-0-9` default, `leptos-0-8` opt-in on `leptos-mf2` and `mf2-axum`; `cargo xtask leptos-0-8` a nightly job, not allowed to fail, with its negative control (E0050/E0425) |
| a Fluent corpus of reference-workload shape converts with zero unmapped constructs | **met** (A1, A2) — 6,464 entries in four locales, no error and no warning; `mf2 check` 0 errors |
| converted catalogs format as the Fluent originals, or differ only in owner-approved classes, each counted | **met** (A3; owner question 5) — four classes, no pair unexplained; counts in the work order §A3 |
| the call-site codemod and the migration guide, the guide's samples compiled | **met** (A4) — the reference application rewritten byte for byte as `fluent-converted` in all 61 files with a call site; `cargo xtask fluent-migrate` green; `cargo xtask docs` green with the guide |
| the `leptos-fluent` A/B measured, reported, its snapshot committed with the commit it measured | **met** (A5) — `bench/fluent-ab/SNAPSHOT.md`, of `391ef2a` |
| XLIFF 2 vendored, mapping designed, export and import built and validated against the schema | **met** (A6; owner questions 3 and 7) — XLIFF 2.1; every export validates with `xmllint`; the reference workload round-trips byte-identical |
| dates in the reader's time zone, asserted in a browser | **met** (A7; owner question 2) — `zone.mjs` 40/40 in Chromium and Firefox |
| `cargo xtask ci` green; the harness green at `current_phase = "P8"` | **open** — the exit commit, next (Phase 8 adds no layer; every column stays as Phase 7 left it) |
| this file and the Phase 9 work order | **met** — [17](17-phase-9-work-order.md) (A8), with four owner answers about the release |

## The figures

**The `leptos-fluent` A/B** (A5; `cargo xtask fluent-ab`; the reference
application of 1,600 messages and 1,860 call sites, both sides on Leptos
0.8 because `leptos-fluent` 0.3.1 requires it; Chromium 143 and Firefox
155, 10 alternating runs a side, load 3.2). The whole snapshot, with every
minimum and maximum, is [`bench/fluent-ab/SNAPSHOT.md`](../bench/fluent-ab/SNAPSHOT.md).

| | `leptos-fluent` | mf2 |
|---|---:|---:|
| client wasm, whole (gz) | 975,597 B | 613,952 B |
| a first visit in `en` (gz; mf2: wasm + JS + the `en` catalog) | 983,963 B | 642,980 B |
| each added locale | +66,042 B gz in every visitor's wasm | a 21–27 KB gz catalog, for its own readers only |
| first translated frame after the wasm's load (Chromium median) | 158.8 ms | 129.8 ms |
| a simple format / one argument / a select | 1.18 / 2.03 / 3.14 µs | 0.09 / 0.75 / 2.41 µs |
| a locale switch with 2,000 live nodes | 82.4 ms | 10.3 ms |

Firefox is the same way round. The same-text check compared 53,622
characters of hydrated `<main>` over four routes and two locales, equal in
both engines.

**Converted catalogs against `fluent-bundle`** (A3;
`cargo test -p mf2-cli --test fluent_oracle`): 64,088 pairs per workload
setting, 868 per construct setting; every difference is in one of the four
classes the owner accepted — numbers localized where `fluent-bundle` prints
`f64`'s text, MF2's Default Bidi Strategy for the isolates, selection on
the decimal as `:number` rounds it, current CLDR plural rules. No pair
unexplained.

**XLIFF 2** (A6; `cargo test -p mf2-cli --test xliff`): the reference
workload's three target locales export as 799,126 (`pl`), 824,039
(`en-XA`) and 802,692 B (`ar-XB`); each validates against
`xliff_core_2.0.xsd` and imports back to byte-identical resources.
`ar-XB` offers 56 empty units: its 14 selects' Arabic forms beyond
English's (owner question 7).

**Size.** B1 moved once in the phase, with Leptos: 22,102 → 25,891 B gz
(A0; tachys 0.3 and `leptos/lazy`, not measured apart), all budgets met;
B5 12.6 → 8.4 B gz a site. A7's time zone added nothing to B1 (the gated
build has no `fn-datetime`; its client is byte-identical, SHA-256
`5602cefe…`) and +2,312 B gz to `demo-ssr`'s main module, which has it.
B1 is fitted from two gzipped builds, so it reads a little differently in
another tree state (25,835 at A7's gate run); compare it within one tree.

## What the phase found and did not fix

| Finding | Where found | Goes to |
|---|---|---|
| `DateTimeValue::with_zone` relabels a wall time instead of converting an instant; `timeZone=input` then shows the UTC wall time labelled with the zone | A7 | Phase 9 API review (A2) — a 1.0 cannot ship it |
| `mf2 check` is silent about an option a function does not have (`dateStyle` on `:datetime`), which MF2 ignores at run time | A7 | Phase 9 A2, a lint |
| a date inside an island is built and linted but not asserted in a browser (no example has one; adding one moves `islands-zero`'s figure) | A7 | recorded in the Phase 9 order; not tasked |
| `mf2-cli`'s unit tests `include_str!` `plans/05-tooling.md` from outside the crate, so its tests do not travel in a published package | A1, A4, A6 (the tests that hold each code against its plan table) | Phase 9 A4 (packaging) |
| WebKit is not installed on the development machine; every Phase 8 browser figure is Chromium and Firefox | all | recorded; a release note (Phase 9 A6) |
| a screen reader was not run (none installed, outside the permitted network) | Phase 7 A11, unchanged | recorded |
| `leptos-fluent` 0.3.1 requires Leptos < 0.9, so the A/B and the `fluent-view` / `fluent-converted` templates are on 0.8 | A4, A5 | recorded in the snapshot; re-run only when the owner asks |
| the Phase 7 documentation said an instant is shown in its own zone; it never was (the formatting context's zone is `timeZone`'s default) | A7 | fixed in 03 §6.1 and `docs/call-sites.md` |
| `dateStyle` / `timeStyle` in `demo-ssr` and the docs were `Intl`'s names, ignored by MF2 | A7 | fixed (`dateLength=long`) |

## Owner questions of the phase

Seven, all answered and recorded in the work order (§"Owner questions") and
the documents each corrects: dev hot reload deferred until after v1; the
time-zone cookie built; XLIFF 2 vendored; the A/B on the reference
application; the four classes of formatting difference accepted; Leptos 0.9
the default and 0.8 an opt-in; the target language's plural forms offered
to translators. Four more, about the release, were answered writing the
Phase 9 order (2026-09-25) and are recorded there.
