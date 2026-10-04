# Phase 20 — release 3.0.0

The last phase (`plan/01-size-and-features.md` §7,
`plan/08-dates-and-what-ships-where.md` §8). It publishes the crates at
3.0.0 to crates.io, tags the release, and makes the documents say so. There
are sixteen, or seventeen if task 16.1 gave the ICU4X date backend a crate of
its own; where this file says sixteen, read the count `cargo xtask release`
prints.

Publishing cannot be undone, so this phase has one stop for the owner's word
(20.2). Everything before it is a rehearsal that changes nothing outside the
machine.

## State

* **In flight:** nothing. A worktree made for a task is removed once its work
  is merged.
* **Next:** 20.0, then 20.1

## Done

(nothing yet)

## Before this phase

* Phases 11 to 19 are done, each with its exit recorded.
* The owner's crates.io credentials are set up on this machine
  (`cargo login`), as they were for 2.0.0.

## Standing rules

* Never read, search or list `plan/archive/`.
* This phase is run by the coordinator itself, step by step; it starts a
  task agent only for a fix that 20.1 turns up.
* Network: crates.io only. No push, no force, no rewritten history unless
  the owner says so (`CLAUDE.md`).
* One build at a time, `CARGO_BUILD_JOBS=3`.

## Tasks

### 20.0 Plan pointers in the code — audit, then repair

The tree cites the planning documents by path, and the old `plans/`
directory is now `plan/archive/`: every one of those citations names a path
that no longer exists. Measured on 2026-10-02, before Phase 11's work:
**705 matches of `plans/` in about 300 files**, across `crates/`, `xtask/`,
`conformance/`, `bench/`, `examples/`, `fuzz/`, `tools/` and
`.forgejo/workflows/` — mostly module headers and doc comments of the shape
"plans/03-runtime.md §4". Task 11.0 swept `crates/mf2-cli` only, and 11.3
reworded three more in passing. 3.0.0 should not go out with them.

Audit first, change nothing: count the matches by shape — a bare path in a
header, a path with a section number, a path inside a generated file — and
report the counts to the coordinator.

Then repair them under one rule. A citation may **not** be repointed at
`plan/archive/`: `CLAUDE.md` forbids referencing the archive, and a path
the reader is not allowed to open tells them nothing. So each citation
either says on its own what it meant — the rule, the budget, the invariant,
or the code that defines it — or it goes. Where the comment around it
already says that, dropping the path is the whole repair.

Make it a reviewed pass over the matches of the `rg` below, one commit by
path. Generated text changes through its generator and is regenerated, by
hand never: `crates/mf2-locale-data/data/*.txt`, `conformance/REPORT.md`,
`conformance/COVERAGE.md`, `conformance/ledger.toml`, the records under
`bench/`, and the `package.txt` header (11.0 changed that generator
already).

Citations of `plan/01` to `plan/13` are a different thing and stay: those
documents still exist, and 20.4's second question decides whether they move
to the archive. If the answer moves them, the same rule applies to them, in
20.3's commit.

If a citation carries a fact found nowhere else in the tree, or the pass
cannot be made mechanical, stop at a safe point and report: dropping the
fact is the owner's call, not the agent's.

Text only: no behaviour and no size moves, which is why this runs before
the pre-flight rather than after it.

Done when: `rg -n "plans/" crates xtask tools conformance bench examples
fuzz docs .forgejo .github` is empty, `cargo xtask ci` is green, and the
audit's counts are in the Done entry.

### 20.1 Pre-flight

1. The tree is clean: `git status --porcelain --untracked-files=all` prints
   nothing (`vendor/` and `comparison.md` are excluded locally).
2. `CHANGELOG.md` has a complete `## 3.0.0` entry, breaking changes first,
   and no "not yet published" line.
3. `bash tools/checks/run.sh p20 --against p19`: every check passes, nothing
   moved in size.
4. `cargo xtask native-canaries` and `cargo xtask tui-gate --gate` pass.
5. `cargo xtask release` (no flag: the dry run). It checks the tree, the
   changelog, the crates' metadata, the names on crates.io, the API against
   the published 2.0.0 with `cargo-semver-checks`, then runs `ci`, the
   package tests, `docs-rs`, `msrv` and `cargo publish --workspace --dry-run`.
6. `plan/01-size-and-features.md`: the status line says the design is built,
   and §1.4 has the measured sizes (from the exits of Phases 12 and 13)
   beside the estimates. `plan/08-dates-and-what-ships-where.md`: the status
   line says the same, and nothing in its §9 is still unverified.

A failure here is fixed by a task agent, with a new task written under this
heading, and the pre-flight is run again from step 1.

### 20.2 Publish — the owner's word first

Stop and ask the owner, in plain English, in one message:

* what will be published: sixteen crates at 3.0.0, named;
* that the dry run passed, with the date and the commit;
* the measured sizes: `tui-mf2` before and after, the client's fixed cost
  (B1) before and after, and trippy if it was measured;
* the figures of Phase 19's exit: the client with dates, a browser catalog,
  a server binary, a command-line tool with and without dates, and anything
  that is still larger or slower than the best known option, by how much;
* that a published version cannot be removed, only yanked;
* the two open questions of 20.4.

On yes:

1. `cargo xtask release --publish`. It refuses when `CI` is set, publishes
   in dependency order, waits on crates.io's rate limit, and can be run again
   if it stops: crates already published with identical content are skipped.
2. `git tag -a v3.0.0 -m "rust-mf2 3.0.0"` (the command it prints).
3. Confirm on crates.io that all sixteen show 3.0.0.

On no, or on any answer that is not a clear yes: stop here and record the
answer under Done.

### 20.3 The documents say so

One commit, after the publish:

* `README.md`: the sentences that say the crates are on crates.io at 2.0.0
  and what 2.x promises.
* `docs/getting-started.md`: the note under the first dependency sample.
* `docs/versioning.md`: which `mf2` goes with which `mf2-build`.
* `CLAUDE.md`, "Start here": 3.0.0 is on crates.io, with the date and the
  tag, and phases 11 to 20 are done.
* `plan/01-size-and-features.md`: the status line.

Find the rest with `rg -n "2\.0\.0|2\.x" README.md CLAUDE.md docs`.
`cargo xtask ci` green before the commit, as always.

Then tell the owner the commit and the tag exist locally and are not pushed.
Push, and the push of the tag, only on the owner's word.

### 20.4 Questions for the owner

Asked with 20.2, answered before this phase closes:

* **The two pointer crates** (`pointers/`: `leptos-mf2` and `mf2-axum`, at
  2.0.0, outside the workspace): should they get a 3.0.0 that points at
  `mf2` 3.0, or stay as they are?
* **The plan files:** `plan/01` to `plan/13` are finished work once this
  phase is done. Move them to `plan/archive/` now, or leave them until the
  next plan is written?

## Phase exit

A Done entry: the publish date, the commit, the tag, whether it was pushed,
and the owner's answers to 20.4.
