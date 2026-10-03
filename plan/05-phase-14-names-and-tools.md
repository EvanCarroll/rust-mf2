# Phase 14 — names and tools

The fourth of six phases (`plan/01-size-and-features.md` §7). After it, the
feature names are the 3.0 names, the build warns about a feature that is on
and unused, `mf2 check` and `mf2 init` write the feature list, which is what
stands in for a default set (owner decision 2), and the equivalence helper a
custom function calls cannot answer outside its domain (decision 6).

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then runs every task not yet in "Done", and then "Phase exit", as
**one Workflow** (owner, 2026-10-03): one fresh `mf2-task` agent per task
(agentType `mf2-task`), strictly one after another, each given a brief of
three lines (the task's number, this file, the design section the task
names). Each agent, after its task's commit, adds its Done entry, moves
"In flight"/"Next" along, and commits this file by path. An agent that must
stop for the owner returns `blocked` with the question; the workflow then
stops, runs nothing further, and the coordinator relays the question in
plain English. The coordinator reads no code and keeps its own context to
the workflow's final summary. The next phase starts in a fresh session.

If "In flight" names a task whose commit is not on `main` and the working
tree holds uncommitted changes to its files, a previous session's agent may
still be running: check `ps` for `xtask`/`cargo` before starting, and do
not start a task over a dirty tree.

## State

* **In flight:** nothing. A worktree made for a task is removed once its work
  is merged.
* **Next:** Phase exit

## Done

* **14.0** (92f621c): `mf2-host-web` has `ZONES_NUMBERS_HOST` and
  `INTL_NUMBERS_HOST` beside `NUMBERS_HOST`; `__use_host!` takes `numbers`,
  named by codegen only when `intl` is on and the corpus reaches a number.
  Browser check `intl-host` (`tools/e2e/intl-host/`, outside the workspace)
  passes 7/7 in Chromium and Firefox (WebKit not installed) and fails 4/7
  with the arm removed. No refused feature set became valid. Checked and closed (owner,
  2026-10-03): `Intl.PluralRules` gets the same digit options as
  `Intl.NumberFormat`, so `1.0` displays `1` and selects `one`, and with
  `minimumFractionDigits=1` displays `1.0` and selects `other` (node 23),
  as the Rust path does (`mf2-runtime/tests/format.rs`). No defect.
* **14.1** (73eab6e): `mf2`'s `intl` is `number-intl`; `mf2-fn-number/intl`
  is gone (its users name `mf2-runtime/intl`); `Features::number_intl()`;
  the 2.0.0 baseline keeps `intl`. The `mf2-host-web` statics keep their
  names: they name the date host they wrap, and their gate
  (`mf2-host-web/intl`) is unchanged. Owner, 2026-10-03:
  `INTL_NUMBERS_HOST` (number host over the `datetime-intl` date host) is
  renamed `INTL_DATES_NUMBERS_HOST`; task 14.1a does it, after 14.2.
* **14.2** (0492664): lint `unused-feature` (warn), once per family per
  corpus: date family on with no `:datetime`/`:date`/`:time`; `fn-number`/
  `number-intl` on with no numeric function, plural selection or plain
  placeholder. Silent in `mf2 check` when it assumes the features. Allowed in
  the i18n fixture's three `mf2.toml` (size gates build every feature set).
* **14.1a** (df81666): `INTL_NUMBERS_HOST` is `INTL_DATES_NUMBERS_HOST`
  (static, `__use_host!` arm, API listing; 14.0's changelog line amended).
  Cold ci after `cargo clean` (bash `time`; no `/usr/bin/time` here): wall
  3589 s, user+sys 6272 s (5880 + 392), `target/` 34 GB after; it stopped at
  `api --check` (listing order), the last step but one. Rerun green.
* **14.3** (26535b0): `mf2 check` prints needs / on / on and unused / the
  `features = [...]` to write (modes as the crate writes them on `mf2`) and,
  with both backends, that `datetime-icu` formats everywhere; JSON `features`.
  Warm ci (bash `time`): wall 1897 s, user+sys 2771 s (2722 + 50), `target/`
  42 GB after; it stopped at the docs samples (the new block). Rerun green.
* **14.4** (d75dcec): `Mode::features()` is the one list per mode, used by
  `cargo add` and rendered into every template: cli `native, fn-number`, tui
  `ratatui, fn-number`, ssr `leptos, fn-number`, islands adds `static-locale`,
  csr `leptos, csr, fn-number`. New test covers both paths for every mode.
  ci green (first run stopped at `fmt`), `--only docs` green.
* **14.5** (c635ba1): `FnContext::equivalent` returns `Option<bool>`, `None`
  when the key holds a character the catalog's map does not reach
  (`nfc::check`); the runtime's own sites keep the `bool` `nfc::equivalent`.
  ci green, docs-rs clean, B12 clean. B1 26,834 (Phase 13) -> 26,876 B gz,
  inside the 64 B noise band (`run.sh p14-5 --only sizes`).

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

### 14.0 `intl` reaches `Host::numbers`

Design: `plan/01` §8 F1 and §2 decision 4 (owner, 2026-10-02): a build with
the feature is meant to get both `Intl.NumberFormat` and `Intl.PluralRules`
through `Host::numbers`, and gets neither. It is a defect in 2.0.0, found by
task 11.2, and it is fixed before the feature is renamed so that one commit
fixes behaviour and the next only renames.

What is already built: `crates/mf2-host-web/src/numbers.rs` has
`IntlNumbers`, whose `numbers()` hands out an `Intl` whose `format` is
`Intl.NumberFormat` and whose `plural` is `Intl.PluralRules`, given out only
on an engine that passes the `Intl.NumberFormat` v3 probe, and
`pub static NUMBERS_HOST: IntlNumbers = IntlNumbers(&HOST)`. Nothing names
it. `__use_host!` (`crates/mf2/src/__generated.rs`) can emit only
`host_web::{HOST, ZONES_HOST, INTL_HOST}`, so `Host::numbers` keeps its
`None` default (`crates/mf2-runtime/src/host.rs`) and `check_host` refuses
every numeric function and plural selector.

Build:

* `mf2-host-web`: an `IntlNumbers` static over **each** date host an
  application can be given, not `HOST` alone — a build that formats dates
  and numbers must reach both. The wrapped field is `pub`, so a static is
  enough; no new JS is needed.
* `crates/mf2/src/__generated.rs`, and whatever writes it: `intl` arms that
  name the matching static for every combination the feature structure
  allows (§3.4, §3.5).
* A browser test that proves both halves on a real engine, where the Rust
  path cannot be mistaken for the host's: a number whose grouping or
  currency only `Intl.NumberFormat` produces, and a variant only
  `Intl.PluralRules` selects. Put it with the existing web asserts (`l4-web`
  or the e2e harness). It must fail if the new arms are removed.
* If a feature set that was refused becomes valid, say so in the one feature
  table (task 11.3) rather than in `refusals.rs`.

This changes behaviour, so it earns a `## 3.0.0` line in `CHANGELOG.md`:
`intl` formatted no number and selected no plural in a browser in 2.0.0.

Done when: `cargo xtask ci`, `cargo xtask refusals` and
`cargo xtask codegen-matrix` are green, the browser test passes, and it fails
with the arms removed.

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
* The wiring is task 14.0's, landed before this one: rename what it added
  along with everything else.
* The baseline table `[package.metadata.api.baseline."2.0.0"]` (task 11.1)
  keeps 2.0.0's spelling, `intl`.

Done when: `cargo xtask ci`, `cargo xtask refusals` and
`cargo xtask codegen-matrix` are green.

### 14.1a `INTL_NUMBERS_HOST` becomes `INTL_DATES_NUMBERS_HOST`

Owner, 2026-10-03 (14.1's Done entry): the `INTL` in the static's name meant
the `datetime-intl` date host it wraps, but read as "the `Intl` number host".
Rename the static in `crates/mf2-host-web` and every place that names it
(the `__use_host!` arms in `crates/mf2/src/__generated.rs` and whatever
writes them, tests, docs). `NUMBERS_HOST` and `ZONES_NUMBERS_HOST` keep
their names. Regenerate the API listings (`cargo xtask api`) and add a
`## 3.0.0` line in `CHANGELOG.md`.

Done when: `cargo xtask ci` and `cargo xtask codegen-matrix` are green and
`rg -n INTL_NUMBERS_HOST crates xtask tools conformance bench examples docs`
finds nothing.

First, a one-off measurement (owner, 2026-10-03). Warm `cargo xtask ci`
runs in this phase took about 30–45 min with load below 2, while `target/`
held 247 GB (178 GB debug, 4256 incremental dirs, 119 copies of `mf2`),
all under 3 days old. Before editing anything, with no other build running
(`pgrep -af 'xtask|cargo'`): `cargo clean`, then `cargo xtask spec-sync`
(and `uts35-sync`/`cldr-sync` if a step asks). Run this task's ci as
`/usr/bin/time -v cargo xtask ci` (the cold run) and report its wall
time, user+sys CPU and the size of `target/` after it in the Done entry.
Task 14.3 runs its first ci the same way (the warm run, at no extra cost)
and reports the same three figures. No further runs.

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

### 14.5 The equivalence helper cannot answer outside its domain

Design: `plan/01` §4.3 (the helper bullet) and §2 decision 6.

Phase 13 replaced `Host::nfc` with the map a catalog carries. The map holds
every code point whose full canonical decomposition stays inside the
characters of the catalog's keys and names, so the check is exact for any
string whose NFD characters are in that set — and every key's and name's are,
by construction. A custom selector comparing against a string of its own may
hold a character the map does not reach; `nfc::equivalent` then returns
`false`, which may be wrong. Spotting it needs no new data: the walk already
reaches `Step::Unmapped` (`crates/mf2-runtime/src/nfc.rs`).

Build:

* `crates/mf2-runtime/src/nfc.rs`: a second entry point that separates "not
  equivalent" from "this key holds a character the map does not reach".
  `equivalent` keeps its `bool` signature for the runtime's own two call
  sites (`functions/string.rs`, `format.rs`), which always pass a key the
  catalog holds, so the client path grows no branch.
* `crates/mf2-runtime/src/function.rs`: `FnContext::equivalent` returns
  `Option<bool>`, `None` for a key outside the map. Its documentation says
  what a custom selector may do then: byte equality, which is sound but
  incomplete, or treating the key as unsupported.
* Tests: a key holding a character the catalog never saw gives `None`, and
  the same key present in the catalog gives `Some`; 13.1's oracle test and
  13.4's both-paths test keep passing.
* `CHANGELOG.md` under `## 3.0.0`, and `cargo xtask api` — the helper's
  signature is public API.

No new data and no new dependency: no client-path crate gains
`unicode-normalization` (`cargo xtask native-canaries` stays green), and B1
must not move.

Done when: `cargo xtask ci` is green, `bash bench/b12/check.sh` is clean, and
B1 is reported before and after.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p14 --against p13`. Nothing should move in
   size, unless F1's fix changed what a `number-intl` client links; report
   that client's bytes if so.
2. `cargo xtask release --allow-dirty`: the dry run must still pass against
   the 2.0.0 baseline now that a feature is renamed.
3. Add a Done entry for the exit.
