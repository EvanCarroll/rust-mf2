# Phase 21 — everything passes

The second of the three phases that follow the code (`plan/08` §8; owner,
2026-10-03). Phase 20 left a tree that compiles and has run no test. After
this phase, `cargo xtask ci` and the whole check suite pass on it, the WG
suite passes at every layer, and a developer who has only the guide has
built a terminal application and a Leptos application with dates.

**The long runs happen once.** A full `ci` or a full suite is never run
again to confirm one fix. A fix is confirmed by the narrowest command that
shows the failure, and the full run is repeated once, at the phase's exit,
only for the checks whose inputs changed after they last passed.

Figures are Phase 22's. This phase reads a check's result, not its sizes.

## How to run this phase

The session given this file is the coordinator. It reads no code. It takes
the steps in order. Each long command it starts itself, in the background
with its output in a file, and waits for; it reads the file's tail and its
failures, never the whole log. A failure gets one `mf2-task` agent with a
narrow brief (see "Fixing a failure"). Step 21.6 it runs itself. After the
last step it carries out "Phase exit" and goes on to Phase 22 (`plan/15`),
in the same session or a fresh one.

## State

* **In flight:** the phase exit (`p21b`). A worktree made for a task is removed once its work
  is merged.
* **Next:** the phase exit

## Done

* 21.1 (2026-10-04): `ci --keep-going` in 10 min 57 s (`target/p21/ci.log`). Tests: 182.5 s
  over 166 binaries, against 1,721 s at `p14`; none over two minutes (largest
  `generated_l4` 32.9 s). Failed: the workspace's tests (8), `docs --no-build`,
  `api --check`, `package --check`.
* 21.2: `conformance-report`, `cldr-sync` + `locale-data` (the CLDR cache was absent), and
  `api` after 3f637f8 (18.4's helper hidden at its re-export) and `package` after c26c617
  (16.1's LICENSE a symlink). A review traced every changed API item to a task but 17
  `mf2-build` helpers (16.1, 16.3, 18.4), put to the owner. Records: bcc73f7 and the
  listing commit after it. `rg plans/` finds only the bench records (left by the owner, or
  rewritten when their benchmarks next run) and nothing else.
* 21.3: 8277d8e (`mf2-build`: 17.4's `look_in` let a browser-read entry into the
  server-only table — a real bug; two tests of 17.1's and 8614ca8's replaced behaviour),
  877c8ee (build differential asks the widest date form, 18.1), 3d5d1ac (17.5's bare-date
  assertion), acb5840 (release test counts the published list, 16.1), 936c404 (`mf2 check`
  and `mf2 stats` samples; 19.1's `bundles` named the browser in a corpus without one).
  The coverage and locale-data table tests pass on the new records.
* 21.4 (2026-10-04): the suite in 53 min 55 s (`target/p10-checks/p21/`): 19 pass; `l6-web`
  and `l7-web` failed, then passed after cf7eec5 (16.1's browser ICU4X imports in
  `icu/cache.rs`; the L5 gate's four formatters; `mf2-l4-runner`'s `std-icu` off for the L7
  sets, so the ISO page's server is ISO; `dates-formatter.mjs` judged by date function, the
  owner's rule): `l6-web` 20/20, `l7-web` 70/70. `e2e-0-8` took 21.3 min (11 at `p14`):
  Phase 22's. 892a345 hides the 19 `mf2-build` helpers (owner, 2026-10-04).
* 21.5: `feature-sets`, `msrv --below`, the parser gate, `islands-zero`, the demos' debug
  builds and `fmt-check` pass; `l4-wasi` passes once `wasmtime` 49.0.0 is in `target/tools`
  (47,600 records identical); `l4-web` (chromium, firefox — no WebKit on this machine) passes
  after 70092b8 (16.1 left the browser L4 on the ISO stub). Owed: the canaries fail with
  their forbidden features; `mf2 stats`' page corrected (e91b7db); `mf2 check` recommends
  every manifest's features. 49618f0 (21.5a): `intl_links_no_icu` gets a corpus with a date,
  so its symbol search can fail.
* 21.6: the cold start built and ran both applications from the guide; five stumbles
  became 21.7–21.11, one goes to the owner (`getting-started.md`'s 2.0.0 line).
* 21.7–21.12: 643aa46 (21.7, `mf2 check` stops with the build's `IcuBlob` error),
  92af0d4 (21.8, the ICU4X form line, tested), e31192d (21.9, `mf2-cli` turns on `icu-blob`
  by default — owner: the tool's +5.34 MB is not a budget; `mf2 stats` matches the build's
  files), 5cbb1cd (21.10, date lines of the frameworks that are on), 26da8ce (21.11, the
  first visit's zone in `call-sites.md`), 548eca5 (21.12, the generated `install_with`).
  Exit: the fixes after the suite touch crates nearly every check builds, so the whole
  suite runs once more under a fresh label, `p21b` (a second run into `p21` would append
  to its logs), with the affected checks of 21.5.

## Before this phase

* Phase 20's exit is recorded: its four steps pass on one commit.
* `target/p10-checks/p14/summary.tsv` exists. It is the baseline (2026-10-03,
  `f68832e`, the exit of Phase 14): Phase 15 took no run of its own
  (`plan/06`). If it is gone, `plan/15` has its figures, and `--against p14`
  is left off.
* No other build is running: `pgrep -af 'cargo|rustc|rust-analyzer|checks/run.sh'`,
  and `ListAgents` for a live session in this repository. Two runs of
  `tools/checks/run.sh` at once spoil both.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One build at a time, `CARGO_BUILD_JOBS=3` (`cargo leptos`: 2). A long
  command runs in the background and is waited for; an agent that returns
  while its build runs kills it. No LSP tool.
* **The WG suite passes at every layer, and no ledger entry moves to
  `xfail`.** A fix makes the code do what its task says (`plan/09` to
  `plan/12`, and the design section the task names). A test is changed only
  where it tests the old behaviour that a task replaced, and the report says
  which task. No test, check, budget or gate is removed or loosened.
* A failure that shows the design cannot be built as written, or that a
  budget or gate would break, goes to the coordinator, and the owner
  decides.
* Commits are by path, and say what failed and why. Until this phase's exit
  a commit is made on the narrow command's pass, not on a full `ci`
  (`CLAUDE.md`, Conventions).

## Steps

Logs go to `target/p21/`, and the suite's to `target/p10-checks/p21/`.

### 21.1 Every failure of `ci`, in one run

`CARGO_BUILD_JOBS=3 cargo xtask ci --keep-going` (task 16.0: every step
runs, `cargo test` with `--no-fail-fast`, and the failed steps are listed at
the end).

Then `bash tools/checks/test-times.sh target/p21/ci.log`: the test binaries
by time. Put its total in the Done entry beside 1,721 s, which is what the
same tests took before task 16.0 (`p14`, 2026-10-03). A binary that still
takes more than two minutes is named there too.

### 21.2 The records the generators write

Some of 21.1's failures are records that the code phases could not rewrite,
because rewriting them is a build. Run each generator whose check failed,
and no other:

* `cargo xtask api` — the public API listings (`api.txt`);
* `cargo xtask package` — the packages' file lists (`package.txt`);
* `cargo xtask locale-data` — `crates/mf2-locale-data/data/*.txt`;
* `cargo xtask conformance-report` — `conformance/REPORT.md` and
  `COVERAGE.md`.

One agent then reads the diff of what they wrote against the tasks, and
reports anything in the public API that no task of `plan/09` to `plan/12`
asked for. Then one commit of the records, by path. Then
`rg -n "plans/" crates xtask tools conformance bench examples fuzz docs
.forgejo .github` must be empty (19.7).

### 21.3 The failures, fixed

One agent per failed step of 21.1 that is left, by "Fixing a failure". `ci`
is not run again here.

### 21.4 The suite

`bash tools/checks/run.sh p21 --against p14`

It runs `ci` first, which is the full run that confirms 21.3, then the
twenty other checks, and carries on after a failure. A check that fails is
fixed by "Fixing a failure" and run again alone:
`bash tools/checks/run.sh p21 --only <check>`; the label keeps the other
checks' rows. If `sizes` fails on a workload that names an old feature,
`bash tools/checks/regen.sh` rewrites the workloads.

A check that passes with a figure that moved is not a failure here.
`compare.sh` and what moved are Phase 22's.

### 21.5 The checks that are not in the suite

Each once, in this order; a failure is fixed by "Fixing a failure":

* `cargo xtask feature-sets` — every feature of `mf2` compiled alone. The
  date features were renamed, so this is the first run that matters.
* `cargo xtask msrv --below`
* `cargo xtask l4-wasi --generated 20000`
* `cargo xtask l4-web` — the `intl` build (17.2).
* `cargo run --release -p parser-gate -- --gate`
* `cargo xtask islands-zero`
* `bash tools/fmt-check.sh` over the three demos' wasm files, as the
  `fmt-check` job of `.forgejo/workflows/nightly.yml` calls it.

And what the code phases owed:

* **19.3:** each new canary row is run with its feature swapped for the one
  it forbids, and must fail.
* **19.1:** `mf2 stats` over its test's corpus prints what
  `docs/command-line.md` shows; the page is corrected if not.
* **19.5:** for each example, and each application of the guide under
  `target/docs/projects`, `mf2 check` recommends the date features its
  manifest has.

### 21.6 A cold start

The coordinator, not a task agent, starts one `mf2-user` agent
(`.claude/agents/mf2-user.md`): a developer who has only the guide. Ask it
to build, from the guide alone, a terminal application with a date in a
message and a server-rendered Leptos application with numbers and a date,
choosing features as the guide and the build's errors tell it, and to run
`mf2 check` and `mf2 stats`. 3.0.0 is not on crates.io yet, so its projects
point at this tree's crates; say so in its instructions.

Each stumble it reports about features, dates, time zones or what ships
where becomes a small task in this phase (21.7, 21.8, …), added under Steps
before it is started; each names the narrow check that confirms it.
Stumbles about other things go to the owner as a list.
### 21.7 to 21.11 What the cold start found (2026-10-04)

Both applications of 21.6 built and ran from the guide (a terminal UI with a date, and a
server-rendered Leptos application with numbers and a date: the server's ICU4X text, the
browser's rewrite into the reader's zone, one 585 B catalog downloaded). Its stumbles about
features, dates, zones and what ships where:

* **21.7** `mf2 check` passes with an ICU4X formatter on and no `icu-blob` on `mf2-build`;
  the build then fails. Check must report what the build reports (16.3's `IcuBlob`).
  Confirmed by a `mf2 check` test over such a manifest.
* **21.8** `mf2 check` prints no line for the ICU4X form the build chose and why (18.1), in
  either application, though `features.md` and `command-line.md` say it does. Confirmed by
  the `mf2 check` test of that output and the docs check.
* **21.9** `mf2 stats` leaves out the ICU4X date slice: in the terminal UI it reports
  catalogs of 697/785 B under other names than the 1,243/1,257 B files the build wrote; in
  the Leptos application no server-only table row shows the server's date data. Stats must
  list every bundle where it ships, matching the build's files (19.1, `plan/08` §7). If the
  fix costs the `mf2` binary bytes (e.g. `icu-blob` in the command-line tool), stop: that is
  the owner's call. Confirmed by `stats_says_what_ships_where` with an ICU4X formatter.
* **21.10** before a formatter is chosen, `mf2 check` in a Leptos application lists the date
  line of every kind of application, not of the frameworks that are on (16.3). Confirmed by
  a `mf2 check` test.
* **21.11** `call-sites.md` names `setup().with_time_zone(…)` for the first visit's zone
  without saying what `setup()` is, where it is called and on which side. Confirmed by the
  docs check.

Owner, 2026-10-04, on 21.9 (and so 21.8): `mf2-cli` turns on `icu-blob` by default; the
tool's size does not matter, only what applications build. And a new task:

* **21.12** the generated module offers `install_with(setup)` (owner, `plan/08` §4.3), and
  `call-sites.md` uses it for the first visit's zone; `install()` is `install_with(setup())`.
  Confirmed by the codegen tests and the docs check.

For the owner, not a task here: `getting-started.md` says its manifests name 2.0.0 while
they say 3 (the release phase's sentence, 23.3).


## Fixing a failure

The coordinator gives one `mf2-task` agent a brief of four lines: the
command that failed, narrowed to one test binary or one check
(`cargo test -p <crate> --test <name>`, `cargo xtask <check>`), the log's
path, the task that wrote the code if the log shows it, and this file.

The agent:

* reads the failure from the log (`rg -n` for `FAILED`, `panicked`,
  `error`), never the whole log;
* fixes it under the standing rules, and runs only the narrow command;
* asks first whether the test fails without optimization too
  (`CARGO_PROFILE_DEV_OPT_LEVEL=0`), when the failure looks like one of
  timing or of counted allocations: task 16.0 turned optimization on for
  tests, and a test that fails only with it is reported as 16.0's;
* commits by path when the narrow command passes, and reports in at most
  ten lines. It does not edit this file: the coordinator records the step.

## Phase exit (coordinator)

1. Every check of 21.4 and 21.5 has passed since the last change to what it
   builds. Where a fix landed after a check last passed, run that check
   again, once, now: `bash tools/checks/run.sh p21 --only <checks>`. A fix
   under `crates/` after `ci` last passed means `ci` is one of them.
2. The tree is clean, and `bash tools/checks/run.sh p21 --summarize` shows
   every check `pass`.
3. Add a Done entry: the fix commits by task, the test time of 21.1, and how
   long the phase's runs took.
4. Go on to Phase 22 (`plan/15-phase-22-measure.md`).
