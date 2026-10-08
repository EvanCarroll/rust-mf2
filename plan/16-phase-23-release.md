# Phase 23 — release 3.0.0

The last phase (`plan/01-size-and-features.md` §7,
`plan/08-dates-and-what-ships-where.md` §8). It publishes the crates at
3.0.0 to crates.io, tags the release, and makes the documents say so. There
are seventeen: task 16.1 gave the ICU4X date backend's browser side a crate of
its own, `mf2-fn-datetime-web-icu`, which `mf2-fn-datetime` depends on for
`wasm32-unknown-unknown` (the dry run of 2026-10-07 packaged all seventeen).

Publishing cannot be undone, so this phase has one stop for the owner's word
(23.2). Everything before it is a rehearsal that changes nothing outside the
machine.

It was Phase 20 until the order changed (owner, 2026-10-03: all the code
first, then compile, test and measure; `plan/08` §8). Its first task, the
plan pointers in the code, went to the code phases as 19.7.

## State

* **In flight:** nothing.
  A worktree made for a task is removed once its work is merged.
* **Next:** 23.1

## Done

* **23.0b** The two tests that count `UnknownLocale` warnings take a test-only
  turnstile in `warn`, and the bound test empties that kind's keys and lines
  before and after filling the budget, so neither depends on the thread order.
  `KEYS_PER_KIND`, `once_for` and what an application sees are unchanged; the
  bound is still proved with 100 keys. `ci` green, all 65 steps.
* **23.0a** `cargo xtask api` puts back a public re-export of a hidden trait
  as it does one of a macro, renumbering into the listing's own build the ids
  the trait names; the records are rewritten. They predated every Phase 22
  fix, and nothing moved in them but the two `mf2::CorpusHost` lines in each
  of the seven `mf2` listings. `ci` green, all 65 steps, after one run hit
  23.0b's flake.
* **23.0** The generator emits `Self` inside `impl Locale` and the trait impls
  for it, `#[must_use]` on the five value-returning `pub fn`s, `ok_or_else` in
  `from_str`, an `# Errors` section on `install_from_directory`, and a one-line
  first doc paragraph throughout; no group allowance was added. The fixture has
  its own strict `[lints.clippy]`, and `codegen-matrix` lints every set.
  Committed as `9e4b756` with one of `ci`'s 65 steps red, `api --check`, which
  `main` was already red on before this task (see 23.0a); the owner was told.
  Size not measured: `ok_or_else` is the only byte-level change, and 23.1's
  comparison shows it.

## Before this phase

* Phases 11 to 22 are done, each with its exit recorded, and the owner's
  answer on the number split is in `plan/08` §2.7. The one exception is 23.0,
  the generator fix, which the task says may run while Phase 22's exit is open.
* The owner's crates.io credentials are set up on this machine
  (`cargo login`), as they were for 2.0.0.

## Standing rules

* Never read, search or list `plan/archive/`.
* This phase is run by the coordinator itself, step by step; it starts a
  task agent only for a fix that 23.1 turns up.
* Network: crates.io only. No push, no force, no rewritten history unless
  the owner says so (`CLAUDE.md`).
* One build at a time, `CARGO_BUILD_JOBS=3`.
* From here on no commit is made with `cargo xtask ci` red (`CLAUDE.md`).

## Tasks

### 23.0 The generated module under a strict clippy

Found 2026-10-05 in a trial port of a terminal application to 2.0, with no mf2
code changed: an application that includes the generated module with
`mf2::include_generated!()` and runs clippy with `pedantic` and `nursery` at
`-D warnings` gets 41 errors from code it did not write, all pointing into the
`mf2_generated.rs` that `mf2-build` writes to `OUT_DIR` — `use_self` 31,
`too_long_first_doc_paragraph` 7, `must_use_candidate` 2, `or_fun_call` 1. Code
pulled in with `include!` is linted as the including crate's own code, so the
application cannot fix them in its own source and its CI is red until it wraps
mf2's include in allowances. Nothing in this workspace sees them: mf2's
`[workspace.lints.clippy]` enables `pedantic` only and allows
`must_use_candidate`, `nursery` is enabled nowhere, and `cargo xtask
codegen-matrix` builds `tools/i18n-fixture` with `cargo check`.

This task runs before the phase's gate, with Phase 22's exit still open: it is a
generator fix that has to land before the pre-flight, and the only figure it can
move is a few bytes from `ok_or_else`. What it moves, 23.1's
`compare.sh p21 p23` flags.

1. `crates/mf2-build/src/codegen.rs` emits code that is clean under `pedantic`
   and `nursery`: `Self` inside `impl Locale` (`ALL`, `SOURCE`, the `match` arms
   and the `from_str` return type), `#[must_use]` on the generated `has_locale`
   and `registry`, `ok_or_else` in `from_str`, and a one-line first paragraph on
   every generated doc comment, the rest after a blank line. No blanket group
   allowance on the generated items: it would hide the same lints from mf2's own
   checks and would do nothing for an application that enables `restriction`
   lints one by one. A targeted `allow` beside the code it covers, as
   `enum_variant_names` is today, only where a lint cannot reasonably be
   satisfied — and it is named in the commit.
2. The string assertions in that file's own tests, and the generated snippets
   quoted in `docs/`, say what the generator now emits.
3. `tools/i18n-fixture` carries its own `[lints.clippy]` instead of inheriting
   the workspace's — `pedantic` and `nursery` at warn, `must_use_candidate` not
   allowed — and its hand-written source is clean under them. `cargo xtask
   codegen-matrix` runs `clippy ... -- -D warnings` in place of `check` for at
   least the server, native and client sets, so the generator cannot regress
   silently.
4. `CHANGELOG.md`'s 3.0.0 entry names it: the generated module was not clean
   under an application's strict clippy, and is now.
5. Check: `cargo xtask codegen-matrix`, then `cargo xtask ci`. Commit by path
   with `ci` green.

### 23.0a The API records and the hidden host trait

`cargo xtask api --check` is red, and was before 23.0: `mf2::CorpusHost` is a
`pub use` (`crates/mf2/src/lib.rs:319`) of a `#[doc(hidden)]` trait added by
`bdd27b8` (22.15, 2026-10-04), while `crates/mf2/api/*.txt` were last written at
`bcc73f7` (21.2). rustdoc drops a hidden item, so the lister loses the
re-export; it already puts such a re-export back for one macro
(`restore_hidden_reexports`, `xtask/src/api.rs`, for `mf2::leptos::islands_gate!`)
and does not know the trait kind.

**The trait stays hidden** (owner, 2026-10-05). It carries one constant,
`SYSTEM_ZONE`, from the host to the corpus so that an application with no dates
links neither the reading of the machine's zone nor a time-zone database; the
only method that takes it, `Corpus::with_host`, is `#[doc(hidden)]` too, and the
only caller is the generated module. Documenting it would promise a name whose
only use is hidden. Bringing a host of one's own to a native corpus therefore
stays an undocumented path in 3.0, though `mf2::Host` is documented
(`crates/mf2/src/lib.rs:454`).

1. `xtask/src/api.rs` restores a public re-export of a hidden **trait**, as it
   does a macro, with a test beside the macro's if there is one.
2. `cargo xtask api` rewrites the records. Read the whole diff: the records
   predate every Phase 22 fix, so anything in it besides `mf2::CorpusHost` is a
   public-API change nobody has seen. Report each one; do not absorb it.
3. Check: `cargo xtask api --check`, then `cargo xtask ci` — all 65 steps, since
   23.0 committed with this one red. Commit by path with `ci` green.

### 23.0b The warning test that saturates the key cap

A test in `mf2`'s lib binary fails intermittently and can turn the pre-flight,
or `cargo xtask release`'s own `ci`, red for no reason. It failed once during
15.1 (got 0, expected 1) and then passed; this is the mechanism, read from the
code, not inferred from a run.

`crate::warn`'s keyed warnings are capped per kind: `once_for` drops a warning
once 32 keys exist for that kind (`KEYS_PER_KIND`, `crates/mf2/src/warn.rs:52`,
`:83`), and `SEEN` is one `Mutex<Vec<_>>` for the process. `warn.rs`'s own test
`a_keyed_warning_is_given_once_per_key_and_the_keys_are_bounded`
(`crates/mf2/src/warn.rs:146`) pushes **100** keys of `Kind::UnknownLocale` to
prove the bound holds, which saturates that kind for the whole test process.
`leptos::catalog::tests::an_unknown_language_provided_is_named_once`
(`crates/mf2/src/leptos/catalog.rs:608`) then asserts that `"tlh-test"` was
warned once — and gets nothing, because the cap was reached. Both are unit tests
of the same crate, so they are in one binary and run on parallel threads: which
one wins is the thread order, which is why it is intermittent. Any later test
that expects an `UnknownLocale` warning has the same exposure.

1. The bound is still proved, without spending another test's budget: either
   the bounded-keys test uses a kind nothing else asserts on, or `warn` gains a
   test-only reset of `SEEN` that it calls when it is finished, or the two tests
   are made one. Whichever is chosen, no test may depend on another's thread
   order.
2. Check: `cargo test -p mf2` with the feature set that compiles both tests,
   run three times so an order-dependent pass is visible, then `cargo xtask ci`.
   Commit by path with `ci` green.

### 23.1 Pre-flight

This is the full run on the final commit. Each check runs once in it: the
dry run of step 5 runs `ci`, `docs-rs` and `msrv` itself, so step 3 leaves
them out.

1. The tree is clean: `git status --porcelain --untracked-files=all` prints
   nothing (`vendor/`, `comparison.md` and the owner's two root notes,
   `mf2-generated-code-clippy.md` and the date-backends synopsis, are excluded
   in `.git/info/exclude`; owner, 2026-10-05).
2. `CHANGELOG.md` has a complete `## 3.0.0` entry, breaking changes first,
   and no "not yet published" line.
3. The suite, against Phase 21's run:
   `bash tools/checks/run.sh p23 --against p21 --only sizes,demos,docs,codegen-matrix,scenarios,leptos-0-8,churn,l6-web,l7-web,e2e-0-9,e2e-0-8,browser-no-fmt,browser-pay-for-use,conformance-report,api,refusals,tui-allocs-vs-pseudotrippy,native-no-heavy-crates`.
   Every check passes, and `bash tools/checks/compare.sh p21 p23` flags
   nothing that Phase 22's report did not tell the owner.
4. `cargo xtask tui-allocs-vs-pseudotrippy --gate` passes.
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

### 23.2 Publish — the owner's word first

Stop and ask the owner, in plain English, in one message:

* what will be published: seventeen crates at 3.0.0, named;
* that the dry run passed, with the date and the commit;
* the measured sizes: `tui-mf2` before and after, the client's fixed cost
  (B1) before and after, and trippy if it was measured;
* the figures of Phase 22's report: the client with dates, a browser
  catalog, a server binary, a command-line tool with and without dates, and
  anything that is still larger or slower than the best known option, by how
  much;
* that a published version cannot be removed, only yanked;
* the two open questions of 23.4.

On yes:

1. `cargo xtask release --publish`. It refuses when `CI` is set, publishes
   in dependency order, waits on crates.io's rate limit, and can be run again
   if it stops: crates already published with identical content are skipped.
2. `git tag -a v3.0.0 -m "rust-mf2 3.0.0"` (the command it prints).
3. Confirm on crates.io that all seventeen show 3.0.0.

On no, or on any answer that is not a clear yes: stop here and record the
answer under Done.

### 23.3 The documents say so

One commit, after the publish:

* `README.md`: the sentences that say the crates are on crates.io at 2.0.0
  and what 2.x promises.
* `docs/getting-started.md`: the note under the first dependency sample.
* `docs/versioning.md`: which `mf2` goes with which `mf2-build`.
* `CLAUDE.md`, "Start here": 3.0.0 is on crates.io, with the date and the
  tag, and phases 11 to 23 are done; and under Conventions, the sentence
  that let Phases 16 to 21 commit before `ci` goes.
* `plan/01-size-and-features.md`: the status line.

Find the rest with `rg -n "2\.0\.0|2\.x" README.md CLAUDE.md docs`.
`cargo xtask ci` green before the commit, as always.

Then tell the owner the commit and the tag exist locally and are not pushed.
Push, and the push of the tag, only on the owner's word.

### 23.4 Questions for the owner

Asked with 23.2, answered before this phase closes:

* **The two pointer crates** (`pointers/`: `leptos-mf2` and `mf2-axum`, at
  2.0.0, outside the workspace): should they get a 3.0.0 that points at
  `mf2` 3.0, or stay as they are?
  **Answered (owner, 2026-10-08): neither.** The 2.0.0 pointers were never
  published, and nothing uses either crate: `mf2-axum` is deleted from
  crates.io, and `pointers/` is removed. `leptos-mf2` cannot be deleted,
  `mf2` 1.0.0 requiring it, so the owner yanks it with `mf2` 1.0.0.
* **The plan files:** `plan/01` to `plan/16` are finished work once this
  phase is done. Move them to `plan/archive/` now, or leave them until the
  next plan is written? If they move, the citations of them in the code are
  repaired by 19.7's rule, in 23.3's commit.

## Phase exit

A Done entry: the publish date, the commit, the tag, whether it was pushed,
and the owner's answers to 23.4.
