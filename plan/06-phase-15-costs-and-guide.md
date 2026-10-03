# Phase 15 — costs and the guide

The fifth of six phases (`plan/01-size-and-features.md` §7). After it, every
feature has a measured cost that the guide publishes, the guide presents the
features as four questions with a section on a smaller build, and a 2.0
application has a page that says how to move to 3.0.

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then takes the tasks in order: it sets "In flight" to the task,
starts one `mf2-task` agent with a brief of three lines (the task's number,
this file, the design section the task names), waits for its report, and
goes on. It reads no code itself. Task 15.5 it runs itself. After the last
task it carries out "Phase exit", then stops: the next phase starts in a
fresh session.

## State

* **In flight:** nothing. A worktree made for a task is removed once its work
  is merged.
* **Next:** 15.3

## Done

* **15.1** `cargo xtask feature-costs` writes `docs/feature-costs.md` (12 rows, 2026-10-03); nightly
  `b5` job runs `--check` (off when > 10 % and > 128 B). Client: workload with 150 `:number`/150
  `:datetime` messages, functions through the i18n crate. Canary gained `tui`/`cli`. Findings:
  `compile` adds 8.35 MB native, `datetime-icu` 99.9 KB gz; `number-intl` and `ratatui` save
  bytes; `tzdb-bundled` unused costs 0. `static-locale`/`mark-fallback-lang` not measured (no Leptos layer).
* **15.2** `docs/features.md` rewritten around the four questions, includes the cost table, adds
  "Time zones" and "A smaller build"; lib.rs table (+`axum`, `clap`, `tzdb-bundled`), README, ecosystem,
  native-apps and Cargo comments aligned. Negatives explained: `number-intl` replaces Rust code with `Intl`
  calls; `ratatui`'s baseline `Line::from(String)` links unicode-width tables (~9 KB, by symbol diff).
  Code, not §3.4, settles both date backends on: `datetime-icu` formats everywhere.

## Before this phase

* Phase 14 is done: the feature names are final.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One task, one `mf2-task` agent, one coherent commit by path, with
  `cargo xtask ci` green. A task that changes the guide also runs
  `bash tools/checks/run.sh <label> --only docs`, and one that changes
  rustdoc runs `cargo xtask docs-rs`.
* The guide is for application developers: plain English, no task numbers,
  no reference to `plan/`. The rendered book is never committed
  (`mdbook build` writes to `target/mdbook`).
* If the design does not settle a choice, or a budget or gate would break,
  stop at a safe point and report to the coordinator. The owner decides.

## Tasks

### 15.1 `cargo xtask feature-costs`

Design: `plan/01` §6.4.

Build: a command that measures what each function and data feature adds, and
writes one Markdown table that the guide includes.

* **Client:** gzip bytes of the reference workload's wasm, built as
  `cargo xtask size` builds it (`xtask/src/size.rs`), with and without the
  feature: `fn-number`, `number-intl`, `fn-datetime`, `datetime-icu`,
  `datetime-intl`. The workload has no Leptos layer, so `static-locale` and
  `mark-fallback-lang` are measured on a demo's client wasm
  (`examples/demo-islands`, `examples/demo-csr`), or left out with a line
  that says so.
* **Native:** stripped bytes of a small binary (the fixtures of
  `cargo xtask native-canaries`), with and without: `fn-number`,
  `fn-datetime`, `tzdb-bundled`, `compile`, `ratatui`, `clap`.
* Each cost is measured on a corpus that uses the feature, and the table says
  so; a feature that is on and unused should cost nothing, and a row that
  shows otherwise is a finding to report.
* The table records the command and the date. It is many builds: it runs in
  `.forgejo/workflows/nightly.yml`, not in `cargo xtask ci`, one build at a
  time. The nightly step fails when a committed figure is off by more than a
  stated tolerance.

Done when: the table is committed where the guide can include it and
`cargo xtask ci` is green.

### 15.2 The features page

Design: `plan/01` §3.2 to §3.4.

Rewrite `docs/features.md` around the four questions:

1. Where does it run?
2. What can messages do?
3. Who supplies locale data: the same answer everywhere, or the platform's
   and a smaller build?
4. Behaviour and tools.

With:

* the cost table of 15.1;
* **"A smaller build"**: each lever with its cost and what is given up
  (`number-intl` and `datetime-intl` in the browser; `fn-datetime` with no
  backend; leaving `tzdb-bundled` off; leaving out a function feature the
  corpus does not use), and the two tools that say what to write
  (`mf2 check`, the `unused-feature` warning);
* **time zones**: where the rules come from in each kind of application,
  and that a container with no tzdata needs `tzdb-bundled`;
* `host-std` and `host-web` as the way to use `mf2` with no framework;
* that nothing is on by default, and why `default-features = false` is
  never needed.

Bring into line: the feature table in `crates/mf2/src/lib.rs` (it omits
`clap`), the table in `docs/README.md`, `docs/ecosystem.md`,
`docs/native-apps.md` where it describes `native`, and the comments in
`crates/mf2/Cargo.toml`'s `[features]`.

Done when: `cargo xtask docs`, `cargo xtask docs-rs` and `cargo xtask ci` are
green.

### 15.3 Upgrading from 2.0, and the promise

Build:

* `docs/upgrading.md`: a section "2.0 to 3.0" (the page is all 1.x to 2.0
  today), written from the `## 3.0.0` entry in `CHANGELOG.md` and
  `plan/01` §3.4: the renamed feature, time zones (what changes for a native
  application and for a server, and `tzdb-bundled`), `Host::nfc` for anyone
  who wrote a host, jiff's `IntoArg` impls, the catalog format (rebuild; old
  catalog files are refused), the `links` name.
* `docs/versioning.md`: `mf2`'s feature names are part of the promise; the
  sub-crates' features are not. Remove what is stale: the passage that says
  "2.0.0 is the next release, not yet published".
* `CHANGELOG.md`: the `## 3.0.0` entry put in order, breaking changes first.

Done when: the checks of the standing rules are green.

### 15.4 Samples and examples on 3.0

Build:

* The dependency samples that say `version = "2"`: `docs/native-apps.md`,
  `docs/mf2-for-developers.md`, `docs/getting-started.md`,
  `docs/delivery-modes.md`, `docs/call-sites.md`, `docs/upgrading.md`,
  `docs/accessibility.md`. Find them with `rg -n 'version = "2"|mf2@2' docs
  README.md`. The sentence in `docs/getting-started.md` that says 2.0.0 is on
  crates.io is Phase 16's.
* The feature lists in those samples, in `examples/*/Cargo.toml`,
  `examples/*/i18n/Cargo.toml` and `bench/`, checked against what
  `mf2 check` prints for each corpus.
* `[package.metadata.docs.rs]` in `crates/mf2/Cargo.toml`: the feature set
  docs.rs builds.

Done when: `bash tools/checks/run.sh p15-samples --only docs,demos` and
`cargo xtask ci` are green.

### 15.5 A cold start

The coordinator, not a task agent, starts one `mf2-user` agent
(`.claude/agents/mf2-user.md`): a developer who has only the guide. Ask it to
build, from the guide alone, a terminal application with a date in a message
and a Leptos application with numbers, choosing features as the guide tells
it, and to run `mf2 check`. 3.0.0 is not on crates.io yet, so its projects
point at this tree's crates; say so in its instructions.

Each stumble it reports about features, time zones or the upgrade page
becomes a small task in this phase (15.6, 15.7, …), added under Tasks before
it is started. Stumbles about other things go to the owner as a list.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p15 --against p14`. Nothing should move in
   size.
2. Read `docs/features.md` and `docs/upgrading.md` once as a user would, and
   confirm that every figure in them comes from 15.1's table.
3. Add a Done entry for the exit.
