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

* **In flight:** nothing. The phase is closed: all four exit steps are done
  and recorded below.
* **Next:** Phase 12 (`plan/03-phase-12-native-host.md`).
* A worktree made for a task is removed once its work is merged.

## Done

* **Phase exit, steps 1 and 3** (coordinator, 2026-10-02).

  Step 1: `bash tools/checks/run.sh p11b --against v2` — **all 21 checks
  pass** on `1560a0c`. Sizes against the baseline: `b1=26938` (+215),
  `app=42017` (+121), `rlib=46316` (-4), `b5=8.1`, `b5v=10.4`, every `b7.*`
  identical, `tui=1812352`, `allocs=1329/1329/1329/1328`, `b13=+13573`,
  `tests=933` (+1, 11.4's), conformance `suite=612 ledger=612 gaps=0`. Frame
  time 346.9 µs against a 348.8 µs baseline, taken under load and so shown,
  not judged. The +215 B is 3.0.0's own crate identity, accounted for by
  11.5; nothing else moved.

  **Later phases compare with `--against p11b`, never `p11`.** A second
  session, Phase 12's coordinator, read this session as dead and ran its own
  `run.sh p11` into the same label at the same time: both runs appended to
  every check log, so figures printed twice and `scenarios`, `api` and both
  `e2e` read FAIL — a fixture edit racing the other run's restore, a garbled
  `api.txt`, 6 of 654 browser asserts lost to port contention. None of it was
  real; the directory is kept as `target/p10-checks/p11-contaminated/`. Two
  check suites must never run at once.

  Step 3: the owner was told that F1 is a defect in 2.0.0 — with `intl` on,
  a browser application formats no number at all — and that task 14.0 fixes
  it.

  Step 2: `CARGO_BUILD_JOBS=3 cargo xtask release --allow-dirty` — **passed**
  (exit 0, "the dry run of 3.0.0 passed"), on the owner's say-so, since the
  command reaches crates.io. `cargo-semver-checks` ran 21 times against the
  published 2.0.0 — the sixteen crates, with `mf2` once per mode (axum, core,
  csr, hydrate, native, ratatui, ssr) — and every one reported "no semver
  update required"; `mf2-macros` is skipped by design, being a proc-macro
  crate. `cargo publish --workspace --dry-run` then packaged all sixteen at
  3.0.0.

  One caveat on that run, so nobody reads it as more than it is: Phase 12's
  commits (`b8945c7` through `a7571a9`) landed on `main` while it was in
  flight, and `--allow-dirty` reads the working tree, so it is not a snapshot
  of one commit. It is good evidence that 3.0.0 is publishable and that the
  version bump did what 11.1 intended; the record that counts for the release
  is the release phase's pre-flight (now Phase 20), which runs the same command
  on a quiet tree.

  **Phase 11 is closed.**

* **11.5** The phase's +215 B gz on B1 (26723 → 26938; whole app 41896 →
  42017) is 133 B raw in the 1860-site `tr` wasm's code section, and no new
  code: reverting only the 3.0.0 version and the `links` name to 2.x
  measures the old figures exactly (`run.sh p11base --only sizes`), and the
  133 B is 1072 permuted bodies — `-C metadata`. Nothing to take out (§8).

* **11.4** `cargo xtask native-canaries` links `tools/native-canary` (a new
  workspace of its own, the smallest MF2 application) once per row,
  unstripped, in the release profile, and reads its symbols by owning crate.
  Positive control green; no forbidding rows yet. `native` links jiff,
  unicode-normalization and ryu, and neither sha2 nor sys-locale (§8).

* **11.3** `xtask/src/feature_sets.rs` holds all 87 sets once — packages,
  features, target, builds-or-refused, the commands that use them; `ci`,
  `codegen-matrix`, `msrv` and `refusals` read it, and the capture of their
  cargo command lines is byte-identical to the one taken first. New nightly
  `cargo xtask feature-sets` (`--list` prints the table): all 19 alone, green.

* **11.1** The workspace is 3.0.0: the version and the fifteen pins, `mf2`'s
  `links = "mf2-v3"` with `DEP_MF2_V3_*` through `mf2-build`, the CLI's
  `mf2@3`, a `## 3.0.0` changelog entry, 2.0.0 marked published, and a
  `baseline."2.0.0"` table of the seven modes in place of `"1.0.0"`'s.
  `cargo xtask api` and `package` rewrote nothing: neither records a version.

* **11.0** The tree compiles again. `tests/xliff.rs` and
  `src/workspace_tests.rs` no longer read the moved `plans/05-tooling.md`:
  the import codes are a literal list in the test, held against
  `Finding::ALL` (length and codes) by `workspace_tests`, whose three tests
  keep their "a test is named after it" half. No `plans/` left in `mf2-cli`.

* **11.2** F1–F3 answered in `plan/01` §8. `intl` formats no number in a
  browser today (no host with `Host::numbers`); jiff's feature names hold and
  `tzdb-bundle-always` only matters where there is no system copy; both
  catalog writers see every key and name, normalized in `encode_all`'s pass.
  Read-only: no build run.

## Before this phase

* The owner's pending work is committed: the move of `plans/` to
  `plan/archive/`, the changes to `CLAUDE.md`, `README.md`,
  `docs/accessibility.md` and `xtask/src/package.rs`, and `plan/01` to
  `plan/07`. `cargo xtask release` refuses a dirty tree.
* The coordinator runs the check suite once on that tree, as the comparison
  point for every later phase: `bash tools/checks/run.sh v2`.
* That run found the tree red: the move of `plans/` broke the build, so the
  baseline could not be taken on it. Task 11.0 repairs it first and the
  baseline is taken on the repaired tree. The repair touches a test only, so
  no size moves and `v2` stays the comparison point the later phases use.
* **Done.** The baseline was taken on the repaired tree (`cfa19a6`): twenty
  checks, every one a pass, under `target/p10-checks/v2/`. The figures the
  later phases are compared against are `b1=26723`, `app=41896`, `b5=8.2`,
  `b5v=10.4`, `b7.en=18072`, `b7.pl=24137`, `b7.en-XA=21537`,
  `b7.ar-XB=18423`, `rlib=46320`, `tui=1812352`,
  `allocs=1329/1329/1329/1328`, `files=30`, `b13=+13573`, `tests=932` and
  conformance `suite=612 ledger=612 gaps=0`. Frame time was 374.8 µs, taken
  under load and so shown, not judged. A later phase compares with
  `bash tools/checks/run.sh LABEL --against v2`; do not take this baseline
  again.

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

### 11.0 Make the tree compile again

Found by the baseline run, not planned: `cargo clippy --workspace
--all-targets` fails, so every `cargo xtask ci` on this tree is red and no
task may commit.

`crates/mf2-cli/tests/xliff.rs` has `include_str!("../../../plans/05-tooling.md")`
in `every_code_has_a_test`, which reads §6.3's import table out of the
document and asserts each `xliff-<code>` in it has a test in the file. The
owner's move of `plans/` to `plan/archive/` deleted that path.

The test may not be made to read `plan/archive/`: the archive is history and
CLAUDE.md forbids reading it. Make the test stand on its own — the six codes
belong to the CLI's own contract, so take them from the code that defines
them, or failing that from a literal list in the test — and keep what it
checks: every import code has a test named after it, and the count is the
whole set. The same file's two stale `plans/05-tooling.md` mentions (its
module comment and the string near line 54) point into the archive; reword
them so they do not.

If the honest repair is to drop the test rather than keep it, stop and report
to the coordinator instead: that is the owner's call.

Done differently: emptying the `rg` reaches past that one test, so the stale
`plans/` mentions in the crate's other comments and in `Cargo.toml` are
reworded too, and with them the one generated line — `package.txt`'s header,
in `xtask/src/package.rs` and in all 16 committed `package.txt` files. Text
only: no file list and no size moves.

Done when: `cargo xtask ci` is green and
`rg -n "plans/" crates/mf2-cli` is empty.

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

Do not touch: `pointers/` (the two pointer crates; the release phase asks the
owner), the documents that say 2.0.0 is the current release (the release
phase), the
`version = "2"` samples in `docs/` (Phase 15). Departure: `cargo xtask ci`'s
docs check compares `mf2 init`'s output with the generated blocks of
`docs/native-apps.md`, so those four lines had to say `"3"`; the page's other
samples and the rest of `docs/` wait for Phase 15.

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

### 11.5 Attribute the 215 bytes

Found by the Phase exit, not planned. The exit's check run says this phase
moved the client's fixed cost: on the quiet re-run (`target/p10-checks/p11b`,
`run.sh p11b --against v2`) `b1 = 26938` against the baseline's `26723`
(+215 B), `app = 42017` against `41896` (+121 B), `rlib = 46316` against
`46320` (-4 B), `b5 = 8.1` against `8.2`. The catalogs are byte-identical
(every `b7.*` unchanged), so it is the wasm. Every size gate passed, so this
is drift inside the budget, not a breach — but this phase claims to move no
size, and Phases 12 to 16 compare against `v2`.

Known already; do not redo it:

* The only shipping files changed since the baseline `cfa19a6` are
  `crates/mf2-build/src/{run,error}.rs`, `crates/mf2-cli/src/init.rs`,
  `crates/mf2-cli/tests/commands.rs`, `crates/mf2/{Cargo.toml,build.rs}` and
  the two `mf2-fn-*` comments. None of them is client-path code.
* No dependency moved: nothing was fetched into the cargo cache after the
  baseline run, so the `Cargo.lock` rewrite at 03:01 was the internal pins.
* It reproduces: two runs at `HEAD` both measure 26938.
* The first exit run (`target/p10-checks/p11`) had `scenarios`, `api` and
  both `e2e` red because a second session ran its own suite into the same
  label at the same time. The clean re-run passes all 21 checks. Spend no
  time on those four.

Measure, do not reason: `bash tools/checks/run.sh LABEL --only sizes` at
`cfa19a6` and at `HEAD`, one build at a time, nothing else running, the
baseline measured in a detached checkout or a worktree that you restore —
never leave the repository off `main`. Then narrow: the candidates are the
version literal wherever generated code carries it, the `links` key and its
`DEP_MF2_V3_*` metadata, and the `mf2` manifest's new baseline table.

Then either take the cost back out, or record it: one line in `plan/01` §8
naming what the 215 bytes are, and the figures in this task's Done entry. If
3.0.0 simply costs 215 B more on the client and nothing can be done about it,
say that plainly — it is a result, not a failure. If the cause is not found
by measurement, stop and report rather than guess.

Done when: the cause is named with the measurement that shows it, the figures
are recorded in `plan/01` §8, and `cargo xtask ci` is green.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p11 --against v2`. Nothing should have moved:
   this phase changes no size.
2. `cargo xtask release --allow-dirty` (the dry run; it reaches crates.io).
   It must pass its `cargo-semver-checks` step for every crate against the
   published 2.0.0. If it does not, that is a task for this phase, not the
   next.
3. Done ahead of the exit: F1 found `intl` broken, the owner was told on
   2026-10-02 and confirmed the intent (`plan/01` §2 decision 4). Task 14.0
   fixes it, so there is nothing left to report here.
4. Add a Done entry for the exit: the two commands and their results.
