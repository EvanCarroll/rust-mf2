# Phase 12 — the native host: time zones and float text

The second of six phases (`plan/01-size-and-features.md` §7). After it, a
native or server binary with no date in any message links no jiff and no
`ryu`, and a native application with dates reads the system's time-zone
database. Estimated saving for trippy: 308 KiB and 12.8 KiB (`plan/01` §1.4).

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then takes the tasks in order: it sets "In flight" to the task,
starts one `mf2-task` agent with a brief of three lines (the task's number,
this file, the design section the task names), waits for its report, and
goes on. It reads no code itself. After the last task it carries out "Phase
exit", then stops: the next phase starts in a fresh session.

## State

* **In flight:** nothing of this phase's own. Phase 11's exit is carried out
  first (`plan/02` State): the `p11b` run it asks for is the comparison point
  the exit below needs, and it had never been taken.
* **Next:** 12.1
* A worktree made for a task is removed once its work is merged.

## Done

(nothing yet)

## Before this phase

* Phase 11 is done: the version is 3.0.0, `plan/01` §8 has the answer F2
  (jiff's feature names), `cargo xtask native-canaries` exists.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One task, one `mf2-task` agent, one coherent commit by path, with
  `cargo xtask ci` green. Compatibility with 2.x is not a goal (decision 1).
* A task that changes what an application developer sees adds a line under
  `## 3.0.0` in `CHANGELOG.md`, and regenerates the API listings
  (`cargo xtask api`) when the public API moves.
* If the design does not settle a choice, or a budget or gate would break,
  stop at a safe point and report to the coordinator. The owner decides.

## Tasks

Design for 12.1 to 12.3: `plan/01` §4.1 and the manifests in §3.5. They are
three steps so that `cargo xtask ci` is green after each.

### 12.1 Zones leave `HOST`

Build:

* `crates/mf2-host-std`: a feature `time-zones` that makes jiff optional
  (for now still with `tzdb-bundle-always`, so no answer changes in this
  task), and a second static, `ZONES_HOST`, that adds `zone_offset` and
  hands `f64_to_text` and `nfc` to `StdHost`. `HOST` no longer overrides
  `zone_offset`.
* `crates/mf2/Cargo.toml`: `fn-datetime` adds `"mf2-host-std?/time-zones"`.
* Every place in `mf2` and `mf2-build` that names the native host names
  `ZONES_HOST` when `fn-datetime` is on and `HOST` otherwise: the `host-std`
  arm of `__use_host!` (`crates/mf2/src/__generated.rs`), `native_host` in
  `crates/mf2-build/src/codegen.rs`, `Catalogs::load` in
  `crates/mf2/src/native/catalogs.rs`, and the Leptos server setup
  (`crates/mf2/src/leptos/state.rs`). Find the rest with
  `rg -n "host_std::HOST" crates conformance bench tools`.
* Native code reads the system's zone only with `fn-datetime`:
  `Catalogs::load` and `refresh` in `crates/mf2/src/native/store.rs` use UTC
  without it.

Leave for 12.3: `mf2`'s own jiff dependency and `native/zone.rs`.

Done when: every date test passes unchanged and `cargo xtask ci` is green.
The Done entry says whether `cargo xtask native-canaries` still reports
`jiff` in a `native,fn-number` binary. It should not; if it does, say what
still refers to it, for 12.3.

### 12.2 The system's database, unless the bundle is asked for

Owner decision 3. Use the feature names from `plan/01` §8 F2.

Build:

* `crates/mf2-host-std`: `time-zones` enables jiff's system database (and
  jiff's own bundle on a platform that has none); a weak feature
  `tzdb-bundled = ["jiff?/tzdb-bundle-always"]`. `ZONES_HOST` asks the
  bundled database with `tzdb-bundled` and the system's without it; the
  POSIX-rule fallback stays in both.
* `crates/mf2/Cargo.toml`: `tzdb-bundled = ["mf2-host-std?/tzdb-bundled"]`;
  `ssr` and `axum` add `"tzdb-bundled"`.
* `crates/mf2-build/src/features.rs` knows the new name where it lists
  names.
* Tests: with `tzdb-bundled`, the answers of 2.0 (the existing tests);
  without it, a lookup that reads a database the test supplies, and a named
  zone that is not there is *Bad Option*, not a panic.
* The feature-set table (`xtask`, task 11.3): rows for `native,fn-datetime`
  and `native,fn-datetime,tzdb-bundled`.

Done when: a server build (`axum` or `ssr`, with dates) still has the bundle;
a `native,fn-datetime` build has none on a platform with a system database;
`cargo xtask ci` is green.

### 12.3 `mf2` has no jiff of its own

Build:

* The system-zone reader moves from `crates/mf2/src/native/zone.rs` into
  `mf2-host-std`, behind `time-zones`, returning `mf2-runtime`'s `TimeZone`.
  `native/zone.rs` is deleted.
* `native` in `crates/mf2/Cargo.toml` drops `dep:jiff`, `jiff/std` and
  `jiff/tz-system`; `mf2` drops the dependency.
* The `IntoArg` impls for jiff's types (`mod with_jiff` in
  `crates/mf2/src/into_arg.rs`) are gated on
  `all(feature = "host-std", feature = "fn-datetime")` and use jiff through a
  re-export from `mf2-host-std`.
* The generated module names the date host only when `fn-datetime` is on
  **and** the corpus can reach a date: it uses `:datetime`, `:date` or
  `:time`, or has a plain placeholder. `host` and `native_host` in
  `crates/mf2-build/src/codegen.rs` apply the same rule (the module already
  knows `m.functions` and `m.unannotated`). `Catalogs` takes its host from
  the generated corpus instead of naming a static.
* `[package.metadata.api.modes]` and the docs.rs feature set in
  `crates/mf2/Cargo.toml` if they named anything that moved.

Done when: `rg -n "jiff" crates/mf2/Cargo.toml` is empty, the API listings
are regenerated, `cargo xtask ci` and `cargo xtask docs-rs` are green.

### 12.4 Float text from `core`

Design: `plan/01` §4.2.

Build: `StdHost::f64_to_text` (`crates/mf2-host-std/src/lib.rs`) formats with
`core` into the 32-byte buffer; `ryu` leaves the crate's dependencies. First
a test, with `ryu` as a dev-dependency: for the edge values (zero and its
sign, subnormals, the largest and smallest, the integers around 2^53, values
either side of where each formatter switches to an exponent) and a large
random sample, `Number::from_f64` gives the same `Number` through both, and
no text overflows the buffer.

Fallback: if the test or the conformance suite disagrees and the difference
cannot be closed in the host, keep `ryu` and report.

Done when: the test and `cargo xtask ci` are green.

### 12.5 The canary rows

Design: `plan/01` §6.1.

Build: rows in `cargo xtask native-canaries` that forbid `jiff` and `ryu`
symbols in the sets `native`, `native,fn-number`, `ratatui,fn-number` and
`axum`; the positive control stays. If `tzdb-bundled` leaves a recognisable
symbol, a row that forbids it in `native,fn-datetime`.

Done when: the command and `cargo xtask ci` are green.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p12 --against p11b`. Expected: `tui-mf2` smaller;
   B1 and B5 within their bands (the client is not touched).
2. Lower `SIZE_LIMIT` in `xtask/src/tui_gate.rs` and the same number in
   `tools/checks/compare.sh` to the measured size of `tui-mf2`. One commit;
   its message gives the old and new bytes and the command.
3. Add a Done entry for the exit: the bytes before and after.
