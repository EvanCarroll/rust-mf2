# Phase 17 — data goes only where it is read (code)

The second phase of `plan/08-dates-and-what-ships-where.md`. After it, the
code is written by which a browser that formats dates with `Intl` downloads
no date data, a browser under `number-intl` downloads no number data, the
build cannot write an entry where nothing reads it, and a server that writes
ISO dates has them localized once the page hydrates.

**Code only** (`plan/08` §8; owner, 2026-10-03). Nothing is compiled, tested
or measured here. Phase 20 compiles it, Phase 21 tests it, Phase 22 measures
it.

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then takes the tasks in order: it sets "In flight" to the task,
starts one `mf2-task` agent with a brief of four lines (the task's number,
this file, the design section the task names, and "nothing is compiled in
this phase: the file's standing rules replace `cargo xtask ci`"), waits for
its report, and goes on. It reads no code itself and runs no build. After the
last task it adds a Done line for the phase and goes straight on to Phase 18
(`plan/11`) in the same session. It stops only for an owner question.

## State

* **In flight:** 17.1. A worktree made for a task is removed once its work
  is merged.
* **Next:** 17.1

## Done

(nothing yet)

## Before this phase

* Phase 16's tasks are committed: the family features exist in the code and
  the old names are gone from it.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* **Nothing is compiled.** No `cargo build`, `check`, `clippy`, `test`,
  `run`, `doc` or `bench`, no `cargo xtask`, no `cargo leptos`, nothing under
  `tools/checks/`, no browser run. Three commands are allowed, because they
  compile nothing: `cargo fmt` (it also finds syntax errors),
  `cargo metadata` and `cargo tree` (they show that the manifests resolve).
* One task, one `mf2-task` agent, one coherent commit by path. The commit is
  made when the task's code and tests are written, `cargo fmt --all --check`
  passes and `cargo metadata --format-version 1` resolves. A file outside
  the workspace is formatted with `rustfmt --edition 2024 <file>`.
* **Write for a compiler that runs later.** Read the definition of everything
  the task calls or changes, and every caller of what it changes (`rg -n`).
  A caller left as it was is Phase 20's compile error.
* **Tests are written, not run.** Each task's "Tests" are written in the
  task and first run in Phase 21. No task weakens or removes a test.
* **Figures are Phase 22's.** A task takes no baseline and measures nothing.
  Its report says in one line what it expects to move, and which way.
* **The client's bytes must not move** in 17.1, 17.2 and 17.4: `server-data`
  is off in a client build, and nothing these tasks add may be reachable
  from one. Phase 22 measures it once, for all of them.
* What a task could not confirm by reading goes under "For Phase 20", one
  line: what, and where.
* The guide (`docs/`) is Phase 19's.
* If the design does not settle a choice, stop at a safe point and report to
  the coordinator. The owner decides.

## Tasks

### 17.1 The server-only table

Design: `plan/08` §4.1, §4.2.

Build:

* `mf2-catalog`: the feature `server-data`. A catalog may carry a second
  source of LOCALE entries, in the section's own encoding, which
  `locale_entry` falls back to. Off, the field does not exist.
* `mf2-build`: each entry gets its place by §4.1's rule. The entries for
  native code alone are written beside the catalog; the catalog's name and
  hash cover what a browser downloads. `mf2 compile --site` writes no
  server-only table.
* The generated module embeds the table for a server; the Leptos server and
  `mf2::axum` hand it to their catalogs. A native application keeps
  everything in its catalog.
* The date slice follows the rule: in the catalog when the browser's
  formatter is `icu` or there is no browser side, in the table otherwise.
* `mf2 stats` lists the table's entries beside the catalog's.

Starts at: `crates/mf2-catalog/src/reader.rs` (`locale_entry`, `Bytes`),
`writer.rs`; `crates/mf2-build/src/catalog.rs` (`write`), `slice.rs`
(`icu_entry`), `codegen.rs` (`embedded`, `catalogs`), `features.rs`
(`CATALOG_FEATURES`); `crates/mf2/src/leptos/catalog.rs`,
`crates/mf2/src/axum/serve.rs`; `crates/mf2-cli/src/stats.rs`.

Tests (written here, run in Phase 21):

* with `leptos-client-datetime-intl` and `leptos-server-datetime-icu`, a
  catalog has no entry 48 and is byte for byte the catalog of the same
  corpus built with no date formatter in native code;
* with `leptos-client-datetime-icu`, the slice is in the catalog and no
  table is written;
* the server formats the date goldens as before
  (`conformance/goldens/dates.tsv`): the existing test, unchanged.

Phase 22 measures: the client's bytes (unchanged); a catalog's brotli bytes
with the slice in it and with the slice in the table, for a corpus with a
zone name. `plan/08` §1.2 measured 15,607 to 17,781 B of slice per language.

### 17.2 `number-intl` sends no number data

Design: `plan/08` §4.1.

First read the browser path under `number-intl` and list which LOCALE
entries it still reads. The September probe says none; if one is read, it
stays in the catalog and the report names it.

Build: with `number-intl` on, the entries the browser does not read go to
the server-only table. `Catalog`'s plural spans come from whichever source
holds them.

Starts at: `crates/mf2-runtime/src/number.rs`, `number/intl.rs`,
`plural.rs`; `crates/mf2-fn-number/src/`; `crates/mf2-build/src/slice.rs`.

Tests (written here, run in Phase 21): the placement of each entry, with
`number-intl` on and off. Phase 21 also runs `cargo xtask l4-web` with the
`intl` entries.

Phase 22 measures: a catalog's bytes with `number-intl` on and off, for a
corpus with a variable currency code.

### 17.3 ISO on the server, another formatter in the browser

Design: `plan/08` §4.3.

Build: the page states the server's date formatter on the preload link; the
client compares it with its own, and when they differ rewrites every date it
hydrated, after hydration, through the queue the zone correction keeps.
Islands behave as they do for the zone. With the same formatter on both
sides, or ICU4X on the server and `Intl` in the browser, nothing changes.

Starts at: `crates/mf2/src/leptos/zone.rs` (`correction`), `links.rs`
(`ZONE_ATTR`), `registry.rs`; the browser harness of `cargo xtask l7-web`.

Tests (written here, run in Phase 21): a browser test in the three engines:
a page rendered with `leptos-server-datetime-iso` shows the browser's
localized date after hydration, and one rendered with
`leptos-server-datetime-icu` is not rewritten.

Phase 22 measures: what this task adds to the client. It is the one task of
this phase that may move the client's bytes, and the owner is told by how
much.

### 17.4 Nothing is written unread

Design: `plan/08` §7, `unread-data`.

Build: after slicing, `mf2-build` checks each entry's place against its
readers and fails with `unread-data` if an entry would go where none of them
looks.

Starts at: `crates/mf2-build/src/catalog.rs`, `lint.rs`;
`xtask/src/feature_sets.rs`.

Tests (written here, run in Phase 21): the check over every feature set of
`xtask/src/feature_sets.rs` and every corpus of the fixtures; and a
deliberately wrong placement, which must fail it.

## For Phase 20

One line per task, only for what could not be confirmed by reading.

(nothing yet)

## Phase exit (coordinator)

No run. The suite against the baseline is Phase 21's, and what the owner is
told — a browser catalog's bytes with and without a zone name, the server
binary's bytes, what 17.3 added to the client — is in Phase 22's report.

1. Add a Done line for the phase.
2. Go on to Phase 18 (`plan/11-phase-18-icu-and-numbers.md`).
