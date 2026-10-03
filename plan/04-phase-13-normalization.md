# Phase 13 — normalization as a catalog fact

The third of six phases (`plan/01-size-and-features.md` §7). After it, the
runtime decides canonical equivalence with a catalog's keys from a small map
the catalog carries, `Host::nfc` is gone, and no native binary links the
normalization tables unless it compiles messages at run time. Estimated
saving for trippy: 118.5 KiB (`plan/01` §1.4).

This phase replaces an existing crate with code of our own, so it has a gate
and a fallback (`plan/01` §4.3, at the end). Tasks 13.1 and 13.2 are the
gate; nothing is removed before they pass.

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
* **Next:** the phase exit

## Done

* **13.1** The builder (`mf2-catalog/src/writer/nfc_map.rs`), the map it writes
  (`mf2-catalog/src/nfc_map.rs`: fixed-width tables, binary search) and the
  check (`mf2-runtime/src/nfc.rs`), nothing calling them yet. The lookup
  computes Hangul syllables instead of listing them; a key set nothing at or
  above U+0300 reaches gets no map. Oracle `unicode-normalization`: every code
  point, and mark sequences, over seven key sets.
* **13.2** The differential fuzz target (`fuzz/fuzz_targets/nfc.rs`, in the
  `long-runs` job and `fuzz/README.md`): index bytes split on `0xFF` draw a key
  set and a candidate from decomposable characters and marks; every pair is
  checked against NFD equality. Seeds walk the alphabet in blocks (54 files).
  A 241 s run (3,640 execs, 15/s — the map build dominates) was clean.
* **13.3** Section kind NFC (9), written by `catalog` from the keys and slot
  names `encode_all` collects, read as `Catalog::nfc_map` and validated at
  load (`CatalogError::Nfc`). The format's major version is 2, so a 1.x/2.x
  catalog is refused. `mf2 stats` lists the map per locale, `mf2 dump` ahead
  of the messages. B7 holds: the reference corpora's keys are ASCII, so no
  catalog there carries the section at all.
* **13.4** `:string` matching and `slot_map` ask the catalog's map
  (`FnContext::equivalent`, the helper a custom function uses); `Host::nfc`
  is left only in the oracles and the L4 runner (13.5). The differential test
  (`conformance/tests/nfc.rs`) runs both paths over every string of the
  suite and the key sets of 13.1, and they agree. No ledger entry moved.
* **13.5** `Host::nfc` is gone, from the trait and every host and stub:
  `mf2-host-std` drops `unicode-normalization`, `mf2-host-web` the `normalize`
  glue. The L4 runner, `conformance/tests/nfc.rs` and the `format` fuzz target
  now normalize with `unicode-normalization` themselves. The canaries forbid
  the tables in the four prebuilt sets and require them in `native,compile`.

## Before this phase

* Phase 12 is done.
* `plan/01` §8 has the answer F3 (whether the one-message writer sees every
  key and name). If the answer is no, task 13.3 starts by reporting to the
  coordinator.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* One task, one `mf2-task` agent, one coherent commit by path, with
  `cargo xtask ci` green. Compatibility with 2.x is not a goal (decision 1).
* A task that changes what an application developer sees adds a line under
  `## 3.0.0` in `CHANGELOG.md`, and regenerates the API listings
  (`cargo xtask api`) when the public API moves.
* Never quote specification text (Unicode, MF2) into the repository;
  paraphrase it.
* If the design does not settle a choice, or a budget or gate would break,
  stop at a safe point and report to the coordinator. The owner decides.

## Tasks

Design for all five: `plan/01` §4.3.

### 13.1 The check and the map, tested

Build, with nothing using them yet:

* **The map builder**, build side, in `mf2-catalog`'s writer (which already
  depends on `unicode-normalization`): from a set of keys and names, the code
  points whose full canonical decomposition uses only characters of those
  keys in NFD, each with its decomposition, and the combining classes of the
  characters involved. Hangul syllables decompose by arithmetic; say in the
  Done entry whether the map lists them or the check computes them.
* **The check**, in `mf2-runtime`, client-path code (`no_std`,
  `forbid(unsafe_code)`, no `fmt`, no `unwrap`, no panicking index): is this
  string canonically equivalent to this key, given the map? The quick path
  first (`nfc_quick` in `crates/mf2-runtime/src/text.rs`, then bytes).
* **The exhaustive test**, with `unicode-normalization` as the oracle: for
  every code point and a corpus of key sets, the check agrees with comparing
  NFC forms. The key sets include: ASCII only (the map must be empty), Latin
  with precomposed and combining accents, several marks in both orders,
  Greek with tonos, Hangul, a key with a singleton decomposition, and an
  empty set.

Done when: the test passes and `cargo xtask ci` is green.

### 13.2 The differential fuzz target

Build: a target in `fuzz/` (declared in `fuzz/Cargo.toml` as the existing
five are) that draws a key set and a string from decomposable characters and
combining marks, builds the map, and fails when the check and full NFC
comparison disagree. Seeds through `cargo xtask fuzz-seed`. Add it to the
fuzz step of the `long-runs` job in `.forgejo/workflows/nightly.yml` and to
`fuzz/README.md`.

Done when: the target builds and a short run is clean. The long run is the
phase exit's.

### 13.3 The catalog section

Build:

* `crates/mf2-catalog/src/format.rs`: a new section kind for the map, and the
  format's version moved so that a 3.0 reader refuses a catalog written
  before the rule existed. An absent section means an empty map.
* Both writers (`writer::catalog`, `writer::single`) emit it when it is not
  empty; the reader exposes it.
* `mf2 dump` and `mf2 stats` (`crates/mf2-cli`) show it.
* `cargo xtask catalog-size` (the brotli budget on the catalog files, B7):
  the corpora grow by the section. If the budget breaks, stop and report.

No keys or names go into the client wasm: the section is catalog data.

Done when: catalogs round-trip, the B6 canary check (`cargo xtask
codegen-matrix`) and `cargo xtask ci` are green.

### 13.4 The runtime uses it

Build:

* `:string` matching (`matches` in
  `crates/mf2-runtime/src/functions/string.rs`) and named arguments
  (`slot_map` in `crates/mf2-runtime/src/format.rs`) call the check with the
  catalog's map instead of `Host::nfc`.
* A helper on the function context (`FnContext`,
  `crates/mf2-runtime/src/function.rs`) that gives a custom function the same
  comparison.
* Tests that run both paths, the new check and `Host::nfc`, over the
  conformance inputs and the key sets of 13.1, and require the same answer.
  `Host::nfc` still exists in this task so that it can be the oracle.

Done when: the WG suite passes at every layer with no ledger entry
(`conformance/ledger.toml`) moving to `xfail`, and `cargo xtask ci` is green.

### 13.5 `Host::nfc` goes

Build:

* `nfc` is removed from the `Host` trait (`crates/mf2-runtime/src/host.rs`)
  and from every host: `mf2-host-std` (which drops `unicode-normalization`),
  `mf2-host-web` (the `normalize` glue), the hosts in `conformance/`, `bench/`
  and tests. Tests that need full NFC use `unicode-normalization` directly.
* A row in `cargo xtask native-canaries`: no `unicode_normalization` symbol
  in `native`, `native,fn-number`, `ratatui,fn-number` and `axum`; and a
  positive control with `compile`.
* The API listings (`cargo xtask api`).

Done when: `cargo xtask ci`, `cargo xtask docs-rs` and `bash
bench/b12/check.sh` (the client's no-panic, no-`fmt` check) are green.

## Gate and fallback

The phase stops, and the coordinator asks the owner, if:

* 13.1 or 13.2 finds a disagreement that is a flaw in the method, not a bug
  in the code;
* a client budget breaks (B1: 30 KB gzip fixed; B5: 40 B gzip per call site;
  B12) or the catalog budget (B7) breaks in 13.3.

The fallback is `plan/01` §4.3's last paragraph: `Host::nfc` stays as a
provided method, and the full tables go behind a feature `nfc` on
`mf2-host-std`, enabled by `compile`. If it is taken, `plan/01` §3.3, §3.4
and §4.3 change in the same commit.

## Phase exit (coordinator)

1. `bash tools/checks/run.sh p13 --against p12`. Expected: `tui-mf2` smaller;
   B1 moves by the check's code less the removed `normalize` glue. Report B1
   before and after.
2. The fuzz exit run: the new target for at least one hour, clean, with no
   build running on the machine meanwhile (`fuzz/README.md` has the command).
3. Lower `SIZE_LIMIT` in `xtask/src/tui_gate.rs` and `tools/checks/compare.sh`
   to the measured size. One commit, with the old and new bytes.
4. Ask the owner, in plain English, whether to measure trippy again now: it
   means switching the checkout in `vendor/trippy` to its `mf2` branch and
   pointing it at this tree. If yes, build `mf2-trip` and add the bytes to
   `plan/01` §1.4 as a new row, beside the 10,222,768 it started from.
5. Add a Done entry for the exit: the sizes, and the fuzz run's length.
