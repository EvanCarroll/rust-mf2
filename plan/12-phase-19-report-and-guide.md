# Phase 19 — the report, the costs and the guide

The last phase of `plan/08-dates-and-what-ships-where.md` before the
release. After it, `mf2 stats` says what ships where, the cost table and the
canaries hold the figures of Phases 16 to 18 every night, the guide presents
the date families, and a developer who has only the guide has built a
terminal application and a Leptos application with dates.

It takes over Phase 15's tasks 15.4 and 15.5 (`plan/06`), as 19.5 and 19.6.

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then takes the tasks in order: it sets "In flight" to the task,
starts one `mf2-task` agent with a brief of three lines (the task's number,
this file, the design section the task names), waits for its report, and
goes on. It reads no code itself. Task 19.6 it runs itself. After the last
task it carries out "Phase exit", then stops: the next phase starts in a
fresh session.

## State

* **In flight:** nothing. A worktree made for a task is removed once its work
  is merged.
* **Next:** 19.1

## Done

(nothing yet)

## Before this phase

* Phase 18 is done, and the owner's answer on the number split is in
  `plan/08` §2.7.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One task, one `mf2-task` agent, one coherent commit by path, with
  `cargo xtask ci` green. A task that changes the guide also runs
  `bash tools/checks/run.sh <label> --only docs`, and one that changes
  rustdoc runs `cargo xtask docs-rs`.
* **A figure that moves the wrong way is the owner's to see**
  (`plan/08` §2.1). The agent puts it in its report with the command; the
  coordinator tells the owner in plain English before the next task starts.
* The guide is for application developers: plain English, no task numbers,
  no reference to `plan/`. Every figure in it comes from 19.2's table. The
  rendered book is never committed (`mdbook build` writes to
  `target/mdbook`).
* If the design does not settle a choice, or a budget or gate would break,
  stop at a safe point and report to the coordinator. The owner decides.

## Tasks

### 19.1 `mf2 stats` says what ships where

Design: `plan/08` §7.

Build: for each language, one line per bundle: the messages, each section a
catalog can leave out, each LOCALE entry, the server-only table's entries.
Each line has its bytes, who reads it (the browser, native code), and where
it ships (the catalog, the server-only table). A closing line gives the
bytes a browser downloads and never reads. `--format json` has the same.

Starts at: `crates/mf2-cli/src/stats.rs`; `crates/mf2-build/src/catalog.rs`.

Done when: a test over a corpus with dates and numbers under three feature
sets (`intl` in the browser, `icu` in the browser, `number-intl`);
`docs/command-line.md` shows the output; `cargo xtask ci` green.

### 19.2 The cost table

Design: `plan/08` §7.

Build, in `cargo xtask feature-costs`:

* the date rows under the family names: the date functions with `iso`,
  `intl` and `icu` in the browser;
* native rows: `native-datetime-iso` and `native-datetime-icu` against no
  date feature;
* a native row for `native-datetime-icu` in an application with no date and
  one plain placeholder, so that what a date feature costs when nothing
  shows a date stays visible;
* the same for the browser: `leptos-client-datetime-intl` in a client with
  no date and one plain placeholder. It has never been measured;
* per language, the brotli bytes the date slice adds to a browser catalog
  with `icu`, and with `intl` (0);
* the number split's row, if the owner chose it.

Starts at: `xtask/src/feature_costs.rs`; `tools/native-canary/`.

Done when: `docs/feature-costs.md` is rewritten by the command, the nightly
check holds it, and `cargo xtask ci` is green. The report compares each new
row with `plan/08` §1.2 and names any that differs by more than 10 %.

### 19.3 The canaries

Design: `plan/08` §7.

Build, in `cargo xtask native-canaries`:

* no date feature, a plain placeholder in the corpus: no symbol of `jiff`,
  `icu_*` or `mf2_fn_datetime`;
* `native-datetime-iso`: no symbol of `icu_*`;
* `native-datetime-icu`: a size ceiling that ICU4X's compiled-in data would
  break.

And for the client, in the check that already greps the wasm for locale
data: with `leptos-client-datetime-intl`, no ICU4X symbol, and no `icu_*`
crate in `cargo tree` for the browser target.

Starts at: `xtask/src/native_canaries.rs`, `tools/native-canary/`;
`xtask/src/size.rs`.

Done when: each row passes, each fails when its feature is swapped for the
one it forbids, and `cargo xtask ci` is green.

### 19.4 The guide

Design: `plan/08` §3, §4, §5.1.

Rewrite, for an application developer:

* `docs/features.md`: the date families as a table per kind of application
  (a server-rendered Leptos application, a client-only one, a command-line
  tool, a terminal UI, an Axum server); what each formatter costs, from
  19.2's table; one formatter per build and which one formats when two are
  on; that a server with ISO dates shows them until the page hydrates, and
  to readers without JavaScript always; that a server's ICU4X text and a
  browser's `Intl` text can differ, with the known cases; what ships where.
* `docs/configuration.md`: `[dates]`.
* `docs/lints.md`: `several-date-formatters`, `unread-data`, the new text of
  `gated-function` and `unused-feature`.
* `docs/getting-started.md`, `docs/call-sites.md`, `docs/native-apps.md`,
  `docs/delivery-modes.md`, `docs/README.md`, `README.md`: wherever a date
  feature is named.
* `crates/mf2/src/lib.rs`'s feature table, the comments of
  `crates/mf2/Cargo.toml`'s `[features]`, `crates/mf2-fn-datetime`'s front
  page.
* `CHANGELOG.md`, `## 3.0.0`: the date features renamed, with §3.6's table;
  a server no longer links ICU4X's data for every language; a browser that
  formats with `Intl` downloads no date data; and, under Fixed, that 2.0.0
  with both date features on put ICU4X in the browser.

Find the old names with `rg -n 'fn-datetime|datetime-icu|datetime-intl' docs
README.md CHANGELOG.md crates/mf2/src/lib.rs`.

Done when: that `rg` finds the names only in the changelog's table;
`cargo xtask docs`, `cargo xtask docs-rs`,
`bash tools/checks/run.sh p19-guide --only docs` and `cargo xtask ci` are
green.

### 19.5 Samples and examples on 3.0

Build:

* The dependency samples that say `version = "2"`: `docs/native-apps.md`,
  `docs/mf2-for-developers.md`, `docs/getting-started.md`,
  `docs/delivery-modes.md`, `docs/call-sites.md`, `docs/upgrading.md`,
  `docs/accessibility.md`. Find them with `rg -n 'version = "2"|mf2@2' docs
  README.md`. The sentence in `docs/getting-started.md` that says 2.0.0 is on
  crates.io is Phase 20's.
* The feature lists in those samples, in `examples/*/Cargo.toml`,
  `examples/*/i18n/Cargo.toml` and `bench/`, checked against what
  `mf2 check` prints for each corpus. `examples/demo-ssr` formats with
  `Intl` in the browser and ICU4X on the server.
* `[package.metadata.docs.rs]` in `crates/mf2/Cargo.toml`: the feature set
  docs.rs builds.

Done when: `bash tools/checks/run.sh p19-samples --only docs,demos` and
`cargo xtask ci` are green.

### 19.6 A cold start

The coordinator, not a task agent, starts one `mf2-user` agent
(`.claude/agents/mf2-user.md`): a developer who has only the guide. Ask it
to build, from the guide alone, a terminal application with a date in a
message and a server-rendered Leptos application with numbers and a date,
choosing features as the guide and the build's errors tell it, and to run
`mf2 check` and `mf2 stats`. 3.0.0 is not on crates.io yet, so its projects
point at this tree's crates; say so in its instructions.

Each stumble it reports about features, dates, time zones or what ships
where becomes a small task in this phase (19.7, 19.8, …), added under Tasks
before it is started. Stumbles about other things go to the owner as a list.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p19 --against p18`. Nothing should move in
   size.
2. Read `docs/features.md` once as a user would, and confirm that every
   figure in it comes from 19.2's table.
3. Tell the owner, in one message, the figures of `plan/08` §1.2 beside what
   they are now: the client with dates, a browser catalog, a server binary,
   a command-line tool with and without dates, and a date's time.
4. Add a Done entry for the exit.
