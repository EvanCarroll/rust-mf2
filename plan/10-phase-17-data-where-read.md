# Phase 17 — data goes only where it is read

The second phase of `plan/08-dates-and-what-ships-where.md`. After it, a
browser that formats dates with `Intl` downloads no date data, a browser
under `number-intl` downloads no number data, the build cannot write an
entry where nothing reads it, and a server that writes ISO dates has them
localized once the page hydrates.

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then takes the tasks in order: it sets "In flight" to the task,
starts one `mf2-task` agent with a brief of three lines (the task's number,
this file, the design section the task names), waits for its report, and
goes on. It reads no code itself. After the last task it carries out "Phase
exit", then stops: the next phase starts in a fresh session.

## State

* **In flight:** nothing. A worktree made for a task is removed once its work
  is merged.
* **Next:** 17.1

## Done

(nothing yet)

## Before this phase

* Phase 16 is done: the family features exist and the old names are gone.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One task, one `mf2-task` agent, one coherent commit by path, with
  `cargo xtask ci` green. One build at a time, `CARGO_BUILD_JOBS=3`.
* **A figure that moves the wrong way is the owner's to see**
  (`plan/08` §2.1). The agent puts it in its report with the command; the
  coordinator tells the owner in plain English before the next task starts.
* **The client's bytes do not move** in 17.1, 17.2 and 17.4. `cargo xtask
  size` before and after is in each report.
* The guide (`docs/`) is Phase 19's.
* If the design does not settle a choice, or a budget or gate would break,
  stop at a safe point and report to the coordinator. The owner decides.

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

Done when:

* with `leptos-client-datetime-intl` and `leptos-server-datetime-icu`, a
  catalog has no entry 48 and is byte for byte the catalog of the same
  corpus built with no date formatter in native code;
* the server formats the date goldens as before
  (`conformance/goldens/dates.tsv`);
* with `leptos-client-datetime-icu`, the slice is in the catalog and no
  table is written;
* the client's bytes are unchanged; `cargo xtask ci` is green.

The report gives a catalog's brotli bytes before and after for a corpus
with a zone name. `plan/08` §1.2 measured 15,607 to 17,781 B of slice per
language.

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

Done when: a test of the placement for each entry; `cargo xtask l4-web`
passes with the `intl` entries; `cargo xtask ci` green. The report gives the
catalog's bytes before and after for a corpus with a variable currency code.

### 17.3 ISO on the server, another formatter in the browser

Design: `plan/08` §4.3.

Build: the page states the server's date formatter on the preload link; the
client compares it with its own, and when they differ rewrites every date it
hydrated, after hydration, through the queue the zone correction keeps.
Islands behave as they do for the zone. With the same formatter on both
sides, or ICU4X on the server and `Intl` in the browser, nothing changes.

Starts at: `crates/mf2/src/leptos/zone.rs` (`correction`), `links.rs`
(`ZONE_ATTR`), `registry.rs`; the browser harness of `cargo xtask l7-web`.

Done when: a browser test in the three engines: a page rendered with
`leptos-server-datetime-iso` shows the browser's localized date after
hydration, and one rendered with `leptos-server-datetime-icu` is not
rewritten. The report gives the client's bytes before and after; this task
may move them, and the coordinator tells the owner by how much.

### 17.4 Nothing is written unread

Design: `plan/08` §7, `unread-data`.

Build: after slicing, `mf2-build` checks each entry's place against its
readers and fails with `unread-data` if an entry would go where none of them
looks. A test runs the check over every feature set of
`xtask/src/feature_sets.rs` and every corpus of the fixtures.

Starts at: `crates/mf2-build/src/catalog.rs`, `lint.rs`;
`xtask/src/feature_sets.rs`.

Done when: the test passes, a deliberately wrong placement fails it, and
`cargo xtask ci` is green.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p17 --against p16`.
2. Tell the owner, in plain English: a browser catalog's bytes before and
   after, with and without a zone name; the server binary's bytes; what 17.3
   added to the client.
3. Add a Done entry for the exit.
