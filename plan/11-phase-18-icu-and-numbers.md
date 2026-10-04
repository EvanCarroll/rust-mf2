# Phase 18 — a smaller, faster ICU4X, and the number split

The third phase of `plan/08-dates-and-what-ships-where.md`. After it, an
application links only the ICU4X its messages need, a date is not formatted
by rebuilding the formatter, the speed of `Intl` against ICU4X for a date is
measured, and the owner has the figures to decide how a browser writes
currency and unit names.

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
* **Next:** 18.1

## Done

(nothing yet)

## Before this phase

* Phase 17 is done: the date slice is where its readers are.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One task, one `mf2-task` agent, one coherent commit by path, with
  `cargo xtask ci` green. One build at a time, `CARGO_BUILD_JOBS=3`.
* **A figure that moves the wrong way is the owner's to see**
  (`plan/08` §2.1). The agent puts it in its report with the command; the
  coordinator tells the owner in plain English before the next task starts.
* Timings on this machine drift: compare two builds by alternating their
  binaries, and give the range over the runs.
* The WG suite passes at every layer, and no ledger entry moves to `xfail`.
* If the design does not settle a choice, or a budget or gate would break,
  stop at a safe point and report to the coordinator. The owner decides.

## Tasks

### 18.1 The narrowest ICU4X per corpus

Design: `plan/08` §5.1.

Build:

* The build works out whether the corpus needs zone names and whether it
  needs a calendar other than Gregorian, and the generated module states
  both. A macro of `mf2` turns them into the date statics of that form.
* The slice is cut for that form alone.
* `mf2.toml`'s `[dates]`: `calendars` and `zone-names`, each `"auto"` by
  default.
* `mf2 check` prints the form and the reason.
* Find out how a date argument carries a calendar, and say in the report
  whether the build can see it. If it cannot, §5.1's fallback stands.

Starts at: `crates/mf2-locale-data/src/icu_blob.rs` (`DateNeeds`,
`IcuBlobSpec`); `crates/mf2-build/src/slice.rs`, `codegen.rs`
(`builtin_path`, `registry`), `config.rs`; `crates/mf2/src/__generated.rs`;
`crates/mf2-fn-datetime/src/icu.rs` (`Variant`).

Done when: a test for each of the four forms; the date goldens pass under
the form each language gets; `bench/b12/check.sh` holds its limits;
`cargo xtask ci` green. The report gives, for a corpus of Gregorian
languages with no zone name: the client's gzip bytes with
`leptos-client-datetime-icu` before and after (the harness measured 83,028
against 42,820 B), the native canary with `native-datetime-icu` before and
after, and a slice's bytes before and after.

### 18.2 The formatter cache

Design: `plan/08` §5.2.

Build: the provider is built once per catalog and the formatter once per
language and shape; `supports` and `format` share one. A catalog that is
loaded again must not be served another's formatter. The cache is a feature
of `mf2-fn-datetime` that the native host turns on.

Starts at: `crates/mf2-fn-datetime/src/icu.rs` (`Blob::with`, `run`),
`Cargo.toml`; `bench/runtime-bench/examples/date_cost.rs`.

Done when: the date goldens are byte for byte what they were; a test formats
through two catalogs of one language in turn; `cargo xtask ci` green. The
report gives `date_cost` before and after by alternating the two binaries
(the baseline: 2.55 to 3.94 µs, 27.07 µs with a zone name), and the native
canary's bytes before and after. Then measure the browser build with
`leptos-client-datetime-icu` with the cache on: if it adds more than 1 KB of
gzip, the browser does not get it, and the report says so.

### 18.3 The speed of a date in a browser

Design: `plan/08` §9.

Measurement only. In Chromium, Firefox and WebKit, time one date placeholder
through `Intl` and through ICU4X, for a date, a date and time, and one with
a zone name, alternating the two builds in one page as the number probe does
(`bench/intl-probe/scripts/2-speed.sh`).

Starts at: `tools/e2e/datetime/`, `tools/e2e/checks/datetime.mjs`;
`bench/intl-probe/` for the method.

Done when: the figures and the command are in a results file beside the
harness, and in the report. No claim about date speed is made before this.

### 18.4 The number split, measured

Design: `plan/08` §6.

Build, behind features of the sub-crates and not of `mf2`: in a browser
build, `:currency` and `:unit` take their names from `Intl.NumberFormat`;
digits, rounding and plural selection stay in Rust. The currency and unit
entries then go to the server-only table.

Measure against the Rust path, on the probe's corpus and language panel:
the client's gzip bytes; a catalog's brotli bytes per language; the time per
placeholder in three engines; where the text differs.

Starts at: `bench/intl-probe/RESULTS.md` §7 and §8 for the baseline and the
method; `crates/mf2-fn-number/src/`; `crates/mf2-host-web/src/numbers.rs`.

Done when: the four measurements are in the report with their commands;
`cargo xtask ci` green with the features off. The task makes no default and
adds no feature to `mf2`.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p18 --against p17`.
2. Ask the owner, in plain English, with 18.4's four figures: should a
   browser write currency and unit names through `Intl` by default, as an
   opt-in, or not at all. Say what each means for an application: its bytes,
   its speed, and the languages a browser has no names for. Write the answer
   into `plan/08` §2.7, and into Phase 19 as a task if it needs code.
3. Tell the owner 18.1's, 18.2's and 18.3's figures.
4. Add a Done entry for the exit.
