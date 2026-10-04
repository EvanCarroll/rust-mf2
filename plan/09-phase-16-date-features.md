# Phase 16 — the date features

The first of the four phases of `plan/08-dates-and-what-ships-where.md`.
After it, a date formatter is chosen per side, under the framework's name;
`fn-datetime`, `datetime-icu` and `datetime-intl` are gone; no build links
ICU4X's compiled-in data; and a build with dates and no formatter says what
to write.

The date slice still goes into every catalog in this phase. Phase 17 moves
it.

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
* **Next:** 16.1

## Done

(nothing yet)

## Before this phase

* Phase 15's tasks 15.1 to 15.3 are done, and its exit is recorded
  (`plan/06`): the label `p15` exists for `tools/checks/run.sh`.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One task, one `mf2-task` agent, one coherent commit by path, with
  `cargo xtask ci` green. One build at a time, `CARGO_BUILD_JOBS=3`.
* **A figure that moves the wrong way is the owner's to see**
  (`plan/08` §2.1). The agent puts it in its report with the command; the
  coordinator tells the owner in plain English before the next task starts.
  A figure that was not measured is reported as not measured.
* The guide (`docs/`) is Phase 19's. A task here changes a page only where a
  check would otherwise fail, and says so in its report.
* If the design does not settle a choice, or a budget or gate would break,
  stop at a safe point and report to the coordinator. The owner decides.

## Tasks

### 16.1 One formatter per side

Design: `plan/08` §3.1, §3.2, §3.4.

Build:

* `mf2-fn-datetime`: the features `std-icu`, `web-icu` and `web-intl`, and
  the default backend §3.4 gives for each target. The compile-time checks of
  "both on" become checks of the strongest-wins rule. `Icu<_, _, Compiled>`
  moves behind a feature of this crate that nothing in `mf2` turns on.
* ICU4X as a dependency of one side: a browser build compiles `icu_*` only
  with `web-icu`, a native build only with `std-icu`.
* `mf2`: the feature `datetime` and the fourteen family features of §3.4.
  `fn-datetime`, `datetime-icu` and `datetime-intl` stay for this task, as
  the 3.0 lines of §3.6 write them, so that the tree keeps building; 16.2
  removes them.
* `mf2-build`: `Features` answers "which formatter, on which side". The
  slice is cut when either side's formatter is `icu`, and `icu-blob` is
  required then (`run.rs`, `slice.rs`). It still goes into the catalog.

Starts at: `crates/mf2-fn-datetime/Cargo.toml`, `src/lib.rs`, `src/icu.rs`;
`crates/mf2/Cargo.toml`, `src/__generated.rs` (`__use_host!`);
`crates/mf2-build/src/features.rs`, `run.rs`, `slice.rs`.

Done when: a test for each rule of §3.2 on each side;
`cargo tree -p mf2 --target wasm32-unknown-unknown -e normal` lists no
`icu_*` crate with `host-web-datetime-intl` and `host-std-datetime-icu` on;
the native canary with `native-datetime-icu` is within 1 % of 893,136 B and
nowhere near 5 MB; `cargo xtask ci` is green. The report gives both figures.

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
rewrites; `cargo xtask ci`, `cargo xtask docs-rs` and `cargo xtask api` are
green.

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
* `mf2 init`: the starters name no date feature; a test holds that.

Starts at: `crates/mf2-build/src/lint.rs`, `error.rs`;
`crates/mf2-cli/src/feature_list.rs`, `init.rs`; `docs/lints.md` for the two
lint entries only.

Done when: a test per message; `cargo xtask ci` green.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p16 --against p15`.
2. Tell the owner, in plain English, what moved: the client's bytes with
   `intl` against 15.2a's 703,921 B; the native canary with `icu` and with
   `iso`; that the browser still downloads the slice until Phase 17.
3. Add a Done entry for the exit.
