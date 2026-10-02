# Phase 11 — open 3.0 and put the guard rails in

The first of six phases (`plan/01-size-and-features.md` §7). It changes no
behaviour and no size. It makes breaking changes possible (the version), finds
out three things the later phases depend on, and adds the two checks that the
later phases fill in.

## How to run this phase

The session given this file is the coordinator. It does "Before this phase",
then takes the tasks in order: it sets "In flight" to the task, starts one
`mf2-task` agent with a brief of three lines (the task's number, this file,
the design section the task names), waits for its report, and goes on. It
reads no code itself. After the last task it carries out "Phase exit", then
stops: the next phase starts in a fresh session.

## State

* **In flight:** nothing. A worktree made for a task is removed once its work
  is merged.
* **Next:** 11.1

## Done

(nothing yet)

## Before this phase

* The owner's pending work is committed: the move of `plans/` to
  `plan/archive/`, the changes to `CLAUDE.md`, `README.md`,
  `docs/accessibility.md` and `xtask/src/package.rs`, and `plan/01` to
  `plan/07`. `cargo xtask release` refuses a dirty tree.
* The coordinator runs the check suite once on that tree, as the comparison
  point for every later phase: `bash tools/checks/run.sh v2`.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One task, one `mf2-task` agent, one coherent commit by path, with
  `cargo xtask ci` green. Compatibility with 2.x is not a goal (decision 1).
* A task that changes what an application developer sees adds a line under
  `## 3.0.0` in `CHANGELOG.md`.
* If the design does not settle a choice, or a budget or gate would break,
  stop at a safe point and report to the coordinator. The owner decides.

## Tasks

### 11.1 Version 3.0.0

Breaking changes need a new major version before they land: CI's `release`
job runs `cargo xtask release` as a dry run on every change, and its
`cargo-semver-checks` step fails a breaking change at 2.0.0.

Change:

* `Cargo.toml` (root): `[workspace.package] version` and the fifteen
  `version = "=2.0.0"` pins in `[workspace.dependencies]`.
* `xtask/src/packages.rs`: `const VERSION` and the literals in its tests.
* `crates/mf2/Cargo.toml`: `links = "mf2-v3"` and its comment.
  `crates/mf2/build.rs`: the doc comment.
* `crates/mf2-build/src/run.rs`: the two constants (`DEP_MF2_V3_FEATURES`,
  `DEP_MF2_V3_VERSION`) and the doc. `crates/mf2-build/src/error.rs`: the
  error text that names the variable.
* `crates/mf2-cli/src/init.rs`: `const MAJOR`.
* `CHANGELOG.md`: a `## 3.0.0` entry at the top with a first line of text
  (the release check needs text under it). Under `## 2.0.0`, the line
  "**Not yet published.**" is stale: 2.0.0 was published on 2026-09-30.
* `crates/mf2/Cargo.toml`: a table `[package.metadata.api.baseline."2.0.0"]`
  spelling each of 2.0.0's seven modes with 2.0.0's feature names, modelled
  on the `"1.0.0"` table below `[package.metadata.api.modes]`. Without it,
  `xtask/src/release.rs` (`spelling`) builds the published baseline with
  today's feature names, which stops working when Phase 14 renames one. The
  `"1.0.0"` table goes if nothing reads it any more.
* The stale comments "not the workspace's `=1.1.0`" in
  `crates/mf2-fn-number/Cargo.toml` and `crates/mf2-fn-datetime/Cargo.toml`.
* Regenerate what records the version: `cargo xtask api`, and the package
  listings that `cargo xtask package --check` compares.

Do not touch: `pointers/` (the two pointer crates; Phase 16 asks the owner),
the documents that say 2.0.0 is the current release (Phase 16), the
`version = "2"` samples in `docs/` (Phase 15).

Done when: `cargo xtask ci` is green and
`rg -n "2\.0\.0|mf2-v2|DEP_MF2_V2" Cargo.toml xtask crates -g '!*.txt'` shows
only the baseline table and history.

### 11.2 Three findings (no code)

Design: `plan/01` §8. Read only; write the answers into §8 in place of the
three "*Open.*" lines, at most ten lines in all, and commit `plan/01` by path.

* **F1.** With `intl` on, compiling for `wasm32-unknown-unknown`: which host
  does an application's formatter hold, and what does a numeric function do
  when `Host::numbers()` returns `None`? Start at `__use_host!` in
  `crates/mf2/src/__generated.rs`, `NUMBERS_HOST` in
  `crates/mf2-host-web/src/numbers.rs`, `crates/mf2-runtime/src/number/intl.rs`
  and the `Setup` in `crates/mf2/src/leptos/state.rs`. Say whether `intl`
  works in an application today, and if not, the smallest fix. Task 14.1
  builds on the answer.
* **F2.** On docs.rs, for the jiff version in the workspace: the feature
  names that give the system's time-zone database, the bundle on platforms
  with no system database, and system-zone detection; the call that looks a
  zone up in the system's database; and whether `tzdb-bundle-always` changes
  what that call returns. Task 12.2 builds on the answer.
* **F3.** Do `writer::catalog` and `writer::single`
  (`crates/mf2-catalog/src/writer.rs`), as `compile_str` calls the latter
  (`crates/mf2/src/compile.rs`), each see every variant key and argument name
  of what they write? Task 13.3 builds on the answer.

Network: docs.rs and crates.io only.

### 11.3 One table of feature sets in `xtask`

Design: `plan/01` §6.3. The sets that CI builds are hand-listed in four
places: `xtask/src/ci.rs` (about twenty `-p mf2 --features` runs),
`xtask/src/codegen_matrix.rs` (`SERVER`, `NATIVE`, `CLIENT`),
`xtask/src/msrv.rs` (`SERVER_FEATURES` and its steps) and
`xtask/src/refusals.rs` (the thirteen that must not compile).

Build: one module in `xtask/src/` holding every set once, with its target
(host or `wasm32-unknown-unknown`), whether it must build or must be refused,
and which commands use it. The four commands read it.

This is a refactor. Before changing anything, capture the cargo command lines
each of the four commands runs into a file under `target/`; afterwards the
same capture must be identical.

Also:

* a nightly step (`.forgejo/workflows/nightly.yml`) that runs `cargo check`
  for each `mf2` feature alone, with the line or mode it needs to be valid;
  the table says what each needs;
* the comment in `.forgejo/workflows/ci.yml` that still says "13 feature
  combinations (6 server, 7 client)".

Done when: the captures match and `cargo xtask ci` is green.

### 11.4 `cargo xtask native-canaries`

Design: `plan/01` §6.1.

Build: a command that links a small native binary for each row of a table,
unstripped, in the profile releases use, reads its symbols, and fails when a
crate that the row forbids has a symbol in it, or one the row requires has
none. One build at a time (`CARGO_BUILD_JOBS=3`); keep the number of binaries
to what the rows need. Reuse `tools/i18n-fixture` if it can link a native
binary for a set; otherwise add one small fixture beside it. Read symbols
with a tool the CI image already has, or the smallest dependency that does it.

It lands with:

* one **positive control**: a set with `native,fn-datetime` requires `jiff`
  symbols, which proves the reader sees them;
* no forbidden rows yet (they would fail today; Phases 12 and 13 add them);
* a report, printed on every run, of which of `jiff`,
  `unicode_normalization`, `ryu`, `sha2` and `sys_locale` are present in each
  binary. This answers `plan/01` §8, "`native` links by use": put the result
  there in one line.

Wire it in: a job in `.forgejo/workflows/ci.yml` beside `tui-gate`, and a
check in `tools/checks/run.sh` (and its README).

Done when: the command passes, the report is in §8, `cargo xtask ci` is green.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p11 --against v2`. Nothing should have moved:
   this phase changes no size.
2. `cargo xtask release --allow-dirty` (the dry run; it reaches crates.io).
   It must pass its `cargo-semver-checks` step for every crate against the
   published 2.0.0. If it does not, that is a task for this phase, not the
   next.
3. If F1 says `intl` does not work in an application, tell the owner in one
   paragraph: it is a defect in 2.0.0, fixed by task 14.1.
4. Add a Done entry for the exit: the two commands and their results.
