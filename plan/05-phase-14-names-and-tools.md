# Phase 14 — names and tools

The fourth of six phases (`plan/01-size-and-features.md` §7). After it, the
feature names are the 3.0 names, the build warns about a feature that is on
and unused, and `mf2 check` and `mf2 init` write the feature list, which is
what stands in for a default set (owner decision 2).

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
* **Next:** 14.1

## Done

(nothing yet)

## Before this phase

* Phase 13 is done.
* `plan/01` §8 has the answer F1 (whether `intl` works in an application).

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

### 14.1 `intl` becomes `number-intl`

Design: `plan/01` §3.4, §3.5 and §8 F1.

Build:

* `crates/mf2/Cargo.toml`: the feature is `number-intl =
  ["mf2-runtime/intl", "mf2-host-web?/intl"]`. `mf2-fn-number`'s `intl`
  feature only forwarded to `mf2-runtime/intl` and is removed. The sub-crate
  feature names (`mf2-runtime/intl`, `mf2-host-web/intl`) stay.
* Everything that spells the old name: `crates/mf2-build/src/features.rs`
  (the `intl()` reader), the feature-set table in `xtask`, `tools/i18n-fixture`,
  `conformance/`, `bench/`, the feature table in `crates/mf2/src/lib.rs`.
  Find them with `rg -n '"intl"|/intl\b|features.*\bintl\b' crates xtask
  tools conformance bench examples`.
* If F1 says an application with the feature never reaches
  `mf2_host_web::NUMBERS_HOST`: the generated host names it. `__use_host!`
  (`crates/mf2/src/__generated.rs`) gets the arms, and `IntlNumbers` wraps
  whichever date host the build would otherwise name. A browser test proves a
  number is formatted by `Intl.NumberFormat` (the existing `l4-web` or e2e
  harness).
* The baseline table `[package.metadata.api.baseline."2.0.0"]` (task 11.1)
  keeps 2.0.0's spelling, `intl`.

Done when: `cargo xtask ci`, `cargo xtask refusals` and
`cargo xtask codegen-matrix` are green.

### 14.2 The lint `unused-feature`

Design: `plan/01` §5.

Build: a lint in the table of `crates/mf2-build/src/lint.rs` (beside
`gated-function` and `neutral-numbers`), warn by default, raised from the
build where `gated-function` is (`functions` in
`crates/mf2-build/src/check.rs`). It fires, once per feature, when:

* `fn-datetime` (or a date backend) is on and no catalog uses `:datetime`,
  `:date` or `:time`. The message says that a date may still be handed to a
  plain placeholder, and that with the feature on every plain placeholder
  links the date code and time zones.
* `fn-number` or `number-intl` is on and no catalog formats or selects on a
  number (the build already knows: `formats_numbers` and the plural needs in
  `crates/mf2-build/src/slice.rs`).

The message says "on for this build": in a workspace another crate may have
turned the feature on. The level is set in `mf2.toml` like any lint.

Check every corpus in the tree (`examples/`, `tools/i18n-fixture`, `bench/`,
`conformance/`): a warning there is either fixed by dropping the feature or
allowed in that corpus's `mf2.toml`, with the reason.

Done when: tests cover both rules and the silent cases; `cargo xtask ci` is
green with no new warning in the tree.

### 14.3 `mf2 check` prints the feature list

Design: `plan/01` §5.

Build, in `crates/mf2-cli/src/check.rs` (the report) with the features it
already resolves through `crates/mf2-cli/src/cargo.rs`:

* a short block in the text report, and a field in the JSON report: the
  function and data features the corpus needs, those that are on, those on
  and unused, and the dependency line to write with the modes left as they
  are;
* when both `datetime-icu` and `datetime-intl` are on, which one each target
  uses;
* when the features could not be resolved, the note says the list is the
  corpus's needs only. The fallback that assumes `fn-number,fn-datetime`
  stays for the lints.

`mf2-cli`'s API listing is its command tree and report shape: regenerate it.

Done when: `cargo xtask ci` is green and the block reads correctly for
`examples/tui` and `examples/demo-ssr/i18n`.

### 14.4 `mf2 init` writes 3.0 lists

Build, in `crates/mf2-cli/src/init.rs`: `Mode::features()` (used by
`cargo add` for an existing crate) and the templates (`native_toml`,
`leptos_toml`, `csr_toml`) disagree today: the templates add `fn-number` for
native applications and `features()` does not. Make one source for both, with
the 3.0 names. `ratatui` implies `native`, so a terminal application names
`ratatui` alone.

Done when: the `init` tests in `crates/mf2-cli/tests/commands.rs` cover every
mode's list, `cargo xtask ci` is green, and
`bash tools/checks/run.sh p14-init --only docs` is green (the guide's
projects are made with `mf2 init` and built).

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p14 --against p13`. Nothing should move in
   size, unless F1's fix changed what a `number-intl` client links; report
   that client's bytes if so.
2. `cargo xtask release --allow-dirty`: the dry run must still pass against
   the 2.0.0 baseline now that a feature is renamed.
3. Add a Done entry for the exit.
