# Phase 18 — a smaller, faster ICU4X, and the number split (code)

The third phase of `plan/08-dates-and-what-ships-where.md`. After it, the
code is written by which an application links only the ICU4X its messages
need and a date is not formatted by rebuilding the formatter; the number
split is built behind features that are off; and the two harnesses exist
that measure a date's speed in a browser and the number split.

**Code only** (`plan/08` §8; owner, 2026-10-03). Nothing is compiled, tested
or measured here. Phase 20 compiles it, Phase 21 tests it, and Phase 22
measures it and puts the number split to the owner with the figures.

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then takes the tasks in order: it sets "In flight" to the task,
starts one `mf2-task` agent with a brief of four lines (the task's number,
this file, the design section the task names, and "nothing is compiled in
this phase: the file's standing rules replace `cargo xtask ci`"), waits for
its report, and goes on. It reads no code itself and runs no build. After the
last task it adds a Done line for the phase and goes straight on to Phase 19
(`plan/12`) in the same session. It stops only for an owner question.

## State

* **In flight:** nothing. A worktree made for a task is removed once its work
  is merged.
* **Next:** 18.1

## Done

(nothing yet)

## Before this phase

* Phase 17's tasks are committed: the code puts the date slice where its
  readers are.

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
  task and first run in Phase 21. No task weakens or removes a test, and no
  ledger entry moves to `xfail`: the WG suite passes at every layer.
* **Figures are Phase 22's.** A task takes no baseline and measures nothing.
  Its report says in one line what it expects to move, and which way.
* **Each change can be measured against what it replaces, in one tree.**
  Phase 22 has no build of the old code to compare with, so 18.1, 18.2 and
  18.4 each keep a switch that gives the old behaviour, as their text says.
* What a task could not confirm by reading goes under "For Phase 20", one
  line: what, and where.
* If the design does not settle a choice, stop at a safe point and report to
  the coordinator. The owner decides.

## Tasks

### 18.1 The narrowest ICU4X per corpus

Design: `plan/08` §5.1.

Build:

* The build works out whether the corpus needs zone names and whether it
  needs a calendar other than Gregorian, and the generated module states
  both. A macro of `mf2` turns them into the date statics of that form.
* The slice is cut for that form alone.
* `mf2.toml`'s `[dates]`: `calendars` and `zone-names`, each `"auto"` by
  default. `calendars = "all"` with `zone-names = true` is the widest form,
  which is what every build has today: Phase 22 measures the narrow forms
  against it.
* `mf2 check` prints the form and the reason.
* Find out, by reading, how a date argument carries a calendar, and say in
  the report whether the build can see it. If it cannot, §5.1's fallback
  stands.

Starts at: `crates/mf2-locale-data/src/icu_blob.rs` (`DateNeeds`,
`IcuBlobSpec`); `crates/mf2-build/src/slice.rs`, `codegen.rs`
(`builtin_path`, `registry`), `config.rs`; `crates/mf2/src/__generated.rs`;
`crates/mf2-fn-datetime/src/icu.rs` (`Variant`).

Tests (written here, run in Phase 21): one for each of the four forms; the
date goldens under the form each language gets. Phase 21 also holds
`bench/b12/check.sh` to its limits.

Phase 22 measures, for a corpus of Gregorian languages with no zone name,
the narrow form against the widest: the client's gzip bytes with
`leptos-client-datetime-icu` (the harness measured 42,820 against 83,028 B),
the native canary with `native-datetime-icu`, and a slice's bytes.

### 18.2 The formatter cache

Design: `plan/08` §5.2.

Build: the provider is built once per catalog and the formatter once per
language and shape; `supports` and `format` share one. A catalog that is
loaded again must not be served another's formatter. The cache is a feature
of `mf2-fn-datetime` that the native host turns on. No browser build turns
it on in this task: Phase 22 measures what it would add to one, and it goes
in only under 1 KB of gzip.

`bench/runtime-bench`'s `date_cost` builds with the cache and without it (a
feature of the bench crate), so that Phase 22 can alternate the two
binaries.

Starts at: `crates/mf2-fn-datetime/src/icu.rs` (`Blob::with`, `run`),
`Cargo.toml`; `bench/runtime-bench/examples/date_cost.rs`.

Tests (written here, run in Phase 21): the date goldens are byte for byte
what they were (the existing test, unchanged); a test formats through two
catalogs of one language in turn.

Phase 22 measures: `date_cost` with the cache and without, by alternating
the two binaries (the baseline: 2.55 to 3.94 µs, 27.07 µs with a zone name);
the native canary's bytes with and without; the browser build with
`leptos-client-datetime-icu` with the cache added.

### 18.3 The speed of a date in a browser: the harness

Design: `plan/08` §9.

Build the harness; run nothing. One page that times one date placeholder
through `Intl` and through ICU4X, for a date, a date and time, and one with
a zone name, alternating the two builds in the page as the number probe does
(`bench/intl-probe/scripts/2-speed.sh`); and one script that builds both,
runs the page in Chromium, Firefox and WebKit, and writes the figures and
its own command to a results file beside the harness.

Starts at: `tools/e2e/datetime/`, `tools/e2e/checks/datetime.mjs`;
`bench/intl-probe/` for the method.

Done when: the page and the script are committed. Phase 22 runs the script.
No claim about date speed is made before it has.

### 18.4 The number split

Design: `plan/08` §6.

Build, behind features of the sub-crates and not of `mf2`: in a browser
build, `:currency` and `:unit` take their names from `Intl.NumberFormat`;
digits, rounding and plural selection stay in Rust. The currency and unit
entries then go to the server-only table. With the features off, nothing
changes: that is the build Phase 21 tests, and the Rust path Phase 22
measures against.

And one script beside the probe that makes the four measurements against the
Rust path, on the probe's corpus and language panel: the client's gzip
bytes; a catalog's brotli bytes per language; the time per placeholder in
three engines; where the text differs. It writes them, with its own command,
to a results file.

Starts at: `bench/intl-probe/RESULTS.md` §7 and §8 for the baseline and the
method; `crates/mf2-fn-number/src/`; `crates/mf2-host-web/src/numbers.rs`.

Tests (written here, run in Phase 21): the split's own, under its features.
`cargo xtask ci` runs with the features off.

Done when: the code, the tests and the script are committed. The task makes
no default and adds no feature to `mf2`. Phase 22 runs the script and asks
the owner.

## For Phase 20

One line per task, only for what could not be confirmed by reading.

(nothing yet)

## Phase exit (coordinator)

No run, and no question yet: the owner is asked about the number split in
Phase 22, with 18.4's four figures, and is told 18.1's, 18.2's and 18.3's
there too.

1. Add a Done line for the phase.
2. Go on to Phase 19 (`plan/12-phase-19-report-and-guide.md`).
