# Phase 20 — everything compiles

The first of the three phases that follow the code (`plan/08` §8; owner,
2026-10-03). Phases 16 to 19 wrote their code without compiling it. After
this phase, every build that `ci` makes succeeds with warnings denied, the
test binaries are built, and **no test has been run**. Tests are Phase 21's,
figures Phase 22's.

## How to run this phase

The session given this file is the coordinator. It reads no code. It takes
the steps in order. Each step is one command, which the coordinator starts
itself in the background with its output in a file, and waits for; it reads
the file's tail and its `error` lines, never the whole log.

A step that passes is ticked under Done, and the next starts. A step that
fails gets fix agents, one at a time, until its command passes (see "Fixing
a step"). After the last step the coordinator carries out "Phase exit" and
goes on to Phase 21 (`plan/14`), in the same session or a fresh one.

## State

* **In flight:** step 4. A worktree made for a task is removed once its work
  is merged.
* **Next:** step 4

## Done

* Before this phase (2026-10-04): Phases 16–19 committed; 246 GB free; another project's
  `cargo leptos watch` was running (outside this tree); the session's rust-analyzer was
  stopped.
* Step 1, formatting: passed at once (`target/p20/1-fmt.log`).
* Step 2, the workspace type-checks: one fix commit (9f36701, 17.5's borrow in `mf2-build`),
  then passed in 24 s (`target/p20/2-check-r2.log`); the session's rust-analyzer had already
  checked most of it. Its two warnings (18.1's `date_forms.rs`, 16.1's `tests/icu.rs`) are
  fixed before step 3, which denies warnings.
* Step 3, everything `ci` builds: the first run (6 min) stopped at clippy; a41e74a (the two
  warnings) and four fix commits — a42ed5a (`mf2-runtime`, 17.5), 5723c29 (`mf2-fn-datetime`,
  17.5/18.2a), e6b0425 (`mf2-conformance` 16.0, `mf2-build` test 17.5), a2f72b8 (`xtask`'s
  error boxed, 19.2) — found by running its clippy step alone with `--keep-going`. Then
  `cargo xtask ci --compile --keep-going` passed in 21 min 33 s
  (`target/p20/3-ci-compile-r2.log`).

## Before this phase

* Phases 16 to 19 are committed, each with its Done line.
* No other build is running: `pgrep -af 'cargo|rustc|rust-analyzer|checks/run.sh'`.
  A build of another project on this machine does not stop the phase, but it
  slows it: say in the Done entry that one ran.
* `df -h .`: everything that depends on `mf2` is built again in this phase.
  Under 100 GB free, ask the owner before clearing anything. **Never
  `cargo clean`:** `target/p10-checks/p14` is the baseline Phases 21 and 22
  compare with, and the specification text's cache is under `target/` too.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* **No test is run in this phase**, not one. `cargo test` appears only with
  `--no-run`.
* One build at a time, `CARGO_BUILD_JOBS=3`. A long command runs in the
  background and is waited for; an agent that returns while its build runs
  kills it. No LSP tool: its `rust-analyzer` builds into the same `target/`.
* **A fix makes the code do what its task says.** The task that wrote a line
  is named by `git log --oneline -3 -- <file>`; its text is in `plan/09` to
  `plan/12`, and it names its design section in `plan/08`. A fix never
  silences: no `#[allow]`, no test or check removed or weakened, no feature
  dropped to get past an error.
* If an error shows that the design cannot be built as written, stop at a
  safe point and report to the coordinator. The owner decides.
* Commits are by path, and say which task's code they correct. Until this
  phase's exit a commit need not compile as a whole (`CLAUDE.md`,
  Conventions): it must compile further than the one before it.

## Steps

Logs go to `target/p20/`.

1. **Formatting.** `cargo fmt --all --check`
2. **The workspace type-checks.**
   `CARGO_BUILD_JOBS=3 cargo check --workspace --all-targets --keep-going`
3. **Everything `ci` builds, built.** `CARGO_BUILD_JOBS=3 cargo xtask ci --compile`
   (task 16.0: `ci`'s `fmt` and `clippy` steps as they are, its `test` steps
   with `--no-run`, none of its closing checks). Every feature set of
   `xtask/src/feature_sets.rs` is compiled here, for the browser and for the
   host, with warnings denied.
4. **The three checks of the suite that only build.**
   `bash tools/checks/run.sh p20 --only docs-rs,codegen-matrix,msrv`

The standalone workspaces (the demos, the terminal UI, the native canary,
the browser harnesses, the size workloads) are built by their own checks in
Phase 21, each just before it runs. A build failure there is fixed there, by
this phase's rules.

## Fixing a step

For a failed step the coordinator names the crates that failed
(`rg -n 'could not compile|^error' <log> | head -40`) and starts one
`mf2-task` agent per crate, in the order cargo built them, with a brief of
four lines: the step's command, the log's path, the crate, and this file.

The agent:

* reads its crate's errors from the log (`rg -n -A14 '^error' <log>`), and
  the lines under "For Phase 20" in `plan/09` to `plan/12` that name its
  crate;
* fixes them under the standing rules, checking with the smallest build that
  shows the error: `cargo check -p <crate>` with the features and the target
  of the failing step;
* stops when its crate checks, or after about forty tool calls, whichever
  comes first; commits by path what it has; and reports in at most ten
  lines: the commits, what is left, and any owner question. It does not edit
  this file: the coordinator records the step.

The coordinator then runs the step's command again. When it passes, the step
is done; when it fails, the next agent starts from the new log.

## Phase exit (coordinator)

1. All four steps pass on one commit, with a clean tree
   (`git status --porcelain`).
2. Add a Done entry: for each step, whether it passed at once or how many
   fix commits it took, and how long the phase's builds ran.
3. Go on to Phase 21 (`plan/14-phase-21-test.md`).
