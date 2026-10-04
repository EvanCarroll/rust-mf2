# Phase 16 — the date features (code)

The first of the four code phases of
`plan/08-dates-and-what-ships-where.md`. After it, the code is written by
which a date formatter is chosen per side, under the framework's name;
`fn-datetime`, `datetime-icu` and `datetime-intl` are gone; no build links
ICU4X's compiled-in data; and a build with dates and no formatter says what
to write.

The date slice still goes into every catalog in this phase. Phase 17 moves
it.

**Code only** (`plan/08` §8; owner, 2026-10-03). Nothing is compiled, tested
or measured here. Phase 20 compiles it, Phase 21 tests it, Phase 22 measures
it. Its first task, 16.0, is the tooling those three phases use.

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then takes the tasks in order: it sets "In flight" to the task,
starts one `mf2-task` agent with a brief of four lines (the task's number,
this file, the design section the task names, and "nothing is compiled in
this phase: the file's standing rules replace `cargo xtask ci`"), waits for
its report, and goes on. It reads no code itself and runs no build. After the
last task it adds a Done line for the phase and goes straight on to Phase 17
(`plan/10`) in the same session. It stops only for an owner question.

## State

* **In flight:** 16.3. A worktree made for a task is removed once its work
  is merged.
* **Next:** 16.3

## Done

* 16.0 (2026-10-03): `[profile.dev] opt-level = 1`; `generated_l3`/`_l4`
  split their cases over scoped threads (`mf2_conformance::parallel`)
  (f566b3b). `cargo xtask ci --compile` (badfb66) and `--keep-going`
  (1a81c34, `Error::CiStepsFailed`). Unit tests for both modes and the
  splitter; nothing compiled.
* 16.1 (2026-10-03): `mf2-fn-datetime` `std-icu`/`web-icu`/`web-intl`, strongest wins per
  side; `Compiled` behind `compiled-data` (a ci set). Browser ICU4X via the new published
  `mf2-fn-datetime-web-icu` (renames refused by Cargo; §3.4). `mf2` `datetime` + 14 family
  features, 2.0 names as aliases; `mf2-build` `Side`/`DateFormatter`, `cuts_date_slice`.
  `cargo tree` on wasm: no `icu_*` with `host-web-datetime-intl,host-std-datetime-icu`.
* 16.2 (2026-10-03): `fn-datetime`, `datetime-icu`, `datetime-intl` gone from `mf2`; every
  user moved per §3.6 (test and bench crates forward the `host-*` families, the demos the
  Leptos ones). `Features`: `datetime` gates the date functions under any formatter's name;
  `for_catalogs` compares `fn-number`/`datetime`/`icu-blob`. Left for 21.2: the ledger,
  `api*.txt`; kept: `mf2-host-web`'s own `datetime-intl`, the 2.0.0 baseline.

## Before this phase

* Phase 15's tasks 15.1 to 15.3 are done (`plan/06`). Its exit took no run:
  the baseline is the label `p14` of `tools/checks/run.sh`, with the figures
  of `plan/08` §1.2 (`plan/15`, "The baseline").
* No tracked file has an uncommitted change (`git status --short`).

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
* What a task could not confirm by reading goes under "For Phase 20", one
  line: what, and where.
* The guide (`docs/`) is Phase 19's. A task here changes a page only where a
  check would otherwise fail, and says so in its report.
* If the design does not settle a choice, stop at a safe point and report to
  the coordinator. The owner decides.

## Tasks

### 16.0 The tools of the new order

Design: `plan/08` §8.

Three commits, in this order.

**1. Tests that take minutes.** A full run of the check suite took 73
minutes at `p14` (2026-10-03), and `cargo xtask ci` was 31 of them. On that
warm tree cargo reported 4 seconds of compiling, and the tests ran for 1,721
of `ci`'s 1,878 seconds: the time is the tests themselves, built without
optimization (`bash tools/checks/test-times.sh target/p10-checks/p14/ci.log`).
Seven test binaries are 1,559 s of it:

| Test binary | s |
|---|---:|
| `conformance/tests/generated_l4.rs` (3,000 generated messages, one thread) | 594 |
| `conformance/tests/generated_l3.rs` (the same) | 491 |
| the conformance crate's unit tests | 137 |
| `conformance/tests/layers.rs` | 135 |
| `crates/mf2-catalog/tests/roundtrip.rs` | 76 |
| `conformance/tests/goldens.rs` | 73 |
| `conformance/tests/build_differential.rs` | 54 |

Build:

* `[profile.dev] opt-level = 1` in the workspace's `Cargo.toml`, with a
  comment that says why. Debug assertions and overflow checks stay on.
  Measured on 2026-10-03 on the conformance crate alone, beside another
  project's build: its tests ran in 297 s against 1,369 s (`generated_l4`
  75 s, `generated_l3` 74 s, the unit tests 67 s, `layers` 58 s), all
  passing; building them from nothing took 287 s
  (`CARGO_PROFILE_DEV_OPT_LEVEL=1 cargo test -p mf2-conformance`).
* `generated_l3` and `generated_l4` run their cases on scoped threads, one
  per `std::thread::available_parallelism`: the same cases from the same
  seed, the counts merged before the closing assertions, a failing case
  still named with its number and its message.

The workspace's total has not been measured. Phase 21 measures it (21.1)
against 1,721 s, and names any binary that still takes two minutes.

**2. `cargo xtask ci --compile`.** The cargo steps of `ci` with nothing run:
`fmt --check`, every `clippy` step as it is, every `test` step with
`--no-run`; none of the closing checks (`refusals`, `docs`,
`conformance-report`, `api`, `package`). Phase 20 runs it.

**3. `cargo xtask ci --keep-going`.** Every step runs, a failed one does not
stop the rest, `cargo test` gets `--no-fail-fast`, and the failed steps are
listed at the end with exit 1. Phase 21 runs it, so that one run finds every
failure.

Plain `cargo xtask ci`, which `.forgejo/workflows/ci.yml` runs, does what it
does today.

Starts at: `Cargo.toml` (`[profile.release]`);
`conformance/tests/generated_l3.rs`, `generated_l4.rs`; `xtask/src/ci.rs`
(`steps`, `run`), `xtask/src/main.rs` (`Command::Ci`), `xtask/src/error.rs`.

Done when: the three commits are made.

### 16.1 One formatter per side

Design: `plan/08` §3.1, §3.2, §3.4.

Build:

* `mf2-fn-datetime`: the features `std-icu`, `web-icu` and `web-intl`, and
  the default backend §3.4 gives for each target. The compile-time checks of
  "both on" become checks of the strongest-wins rule. `Icu<_, _, Compiled>`
  moves behind a feature of this crate that nothing in `mf2` turns on.
* ICU4X as a dependency of one side: a browser build compiles `icu_*` only
  with `web-icu`, a native build only with `std-icu`. Try §3.4's first way
  (one package under two dependency names, one per target) with
  `cargo tree`, which compiles nothing; if Cargo refuses it, the ICU4X
  backend gets a crate of its own.
* `mf2`: the feature `datetime` and the fourteen family features of §3.4.
  `fn-datetime`, `datetime-icu` and `datetime-intl` stay for this task, as
  the 3.0 lines of §3.6 write them; 16.2 removes them.
* `mf2-build`: `Features` answers "which formatter, on which side". The
  slice is cut when either side's formatter is `icu`, and `icu-blob` is
  required then (`run.rs`, `slice.rs`). It still goes into the catalog.

Starts at: `crates/mf2-fn-datetime/Cargo.toml`, `src/lib.rs`, `src/icu.rs`;
`crates/mf2/Cargo.toml`, `src/__generated.rs` (`__use_host!`);
`crates/mf2-build/src/features.rs`, `run.rs`, `slice.rs`.

Tests (written here, run in Phase 21): one for each rule of §3.2 on each
side.

Done when: the code and the tests are committed, and
`cargo tree -p mf2 --target wasm32-unknown-unknown -e normal` lists no
`icu_*` crate with `host-web-datetime-intl` and `host-std-datetime-icu` on.

Phase 22 measures: the native canary with `native-datetime-icu`, which must
be within 1 % of 893,136 B and nowhere near 5 MB.

### 16.2 The old names go

Design: `plan/08` §3.6.

Build: every user in the tree moves to the family features, and
`fn-datetime`, `datetime-icu` and `datetime-intl` are deleted from `mf2`.

* The code's own `cfg`s (`feature = "fn-datetime"` becomes
  `feature = "datetime"`), the conversions of date types, the Leptos layer's
  reader zone.
* `xtask/src/feature_sets.rs`, and what reads it (`ci`, `codegen-matrix`,
  `msrv`, `refusals`); `feature_costs.rs` keeps measuring the same builds
  under the new names.
* `examples/`, `tools/`, `bench/`, `conformance/`, `fuzz/`,
  `.forgejo/workflows/`: manifests and scripts.
* `[package.metadata.docs.rs]` of `mf2` and `mf2-fn-datetime`.

Find them with `rg -n 'fn-datetime|datetime-icu|datetime-intl' crates xtask
tools conformance bench examples fuzz .forgejo .github`. About forty files
name each. `docs/` and `README.md` are Phase 19's.

Done when: that `rg` finds only generated records that their generator
rewrites, and the report lists them. Phase 21 runs the generators (21.2).

### 16.3 The build says what to write

Design: `plan/08` §3.3, §3.5.

Build:

* `gated-function` for a date function: the families of the frameworks that
  are on, the recommended formatter for each side, and what each costs. A
  server-rendered build with a client formatter and no server one gets this
  error.
* `several-date-formatters`, a warning: more than one choice on a side, and
  which one formats.
* `unused-feature` for a date formatter that no message can reach, and a
  line when a family's feature is on without its framework.
* `icu` in native code without `mf2-build`'s `icu-blob`: the error names the
  line to add.
* `mf2 check`: per side, the formatter in force, and the features to write.
  `--format json` has the same. The "both backends" lines go.
* `mf2 init`: the starters name no date feature.

Starts at: `crates/mf2-build/src/lint.rs`, `error.rs`;
`crates/mf2-cli/src/feature_list.rs`, `init.rs`; `docs/lints.md` for the two
lint entries only.

Tests (written here, run in Phase 21): one per message; and one that holds
the starters of `mf2 init` to naming no date feature.

## For Phase 20

One line per task, only for what could not be confirmed by reading.

* 16.1: `use mf2_fn_datetime_web_icu::{icu_calendar, …}` then `use icu_calendar::…` in
  `mf2-fn-datetime/src/icu.rs` on wasm; `api.txt` of the new crate (Phase 21 generates it).
* 16.2: `unexpected_cfgs` for the `host-*-datetime-icu` `cfg` of `conformance/l5/shared.rs`
  in every crate that includes it (l5 sets, l5-generated, l6-web, l7 sets declare both).

## Phase exit (coordinator)

No run. The suite against the baseline is Phase 21's, and what the owner is
told — the client's bytes with `intl` against 15.2a's 703,921 B, the native
canary with `icu` and with `iso` — is in Phase 22's report.

1. Add a Done line for the phase.
2. Go on to Phase 17 (`plan/10-phase-17-data-where-read.md`).
