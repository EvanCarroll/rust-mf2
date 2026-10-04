# Phase 19 — the report, the costs and the guide (code)

The last of the four code phases of `plan/08-dates-and-what-ships-where.md`.
After it, the code is written by which `mf2 stats` says what ships where and
the cost table and the canaries hold the figures of Phases 16 to 18 every
night; the guide presents the date families; the samples are on 3.0; and no
source file cites a planning document that no longer exists.

**Code only** (`plan/08` §8; owner, 2026-10-03). Nothing is compiled, tested
or measured here. Phase 20 compiles it, Phase 21 tests it and runs the cold
start, Phase 22 measures it.

It takes over Phase 15's task 15.4 (`plan/06`) as 19.5, and the release
phase's audit of plan pointers as 19.7. Phase 15's 15.5, the cold start, was
19.6 here and is now Phase 21's 21.6: it needs a tree that builds.

## How to run this phase

The session given this file is the coordinator. It checks "Before this
phase", then takes the tasks in order: it sets "In flight" to the task,
starts one `mf2-task` agent with a brief of four lines (the task's number,
this file, the design section the task names, and "nothing is compiled in
this phase: the file's standing rules replace `cargo xtask ci`"), waits for
its report, and goes on. It reads no code itself and runs no build. After the
last task it adds a Done line for the phase and goes on to Phase 20
(`plan/13`), in the same session or a fresh one. It stops only for an owner
question.

## State

* **In flight:** 19.5. A worktree made for a task is removed once its work
  is merged.
* **Next:** 19.5

## Done

* 19.1: `Catalog::bundles` (`mf2-build`) lists messages, the optional
  sections (cold, ids, fallback, nfc), LOCALE and server-only entries with
  readers and place; `mf2 stats` prints them and the unread bytes, text and
  JSON; test `stats_says_what_ships_where`; `docs/command-line.md`. Choice:
  the optional sections take the messages' readers (one shared reader).
  Then 9b14037: COLD and IDS count as read by native code alone (stripped by default; a
  browser reads FALLBACK and NFC), so turning `strip` off shows in the unread line.
* 19.2: `feature-costs` rows under `leptos-client-datetime-*` and
  `native-datetime-{iso,icu}`, a no-date row per side (`unused`), a per-language
  slice table (brotli, `icu`/`intl`) built in-process (`mf2-build` now a normal
  xtask dependency); row ids include the corpus. Canary: plain placeholder in
  the base corpus, canary-only `no-date-message`.
* 19.3: `native-canaries` forbids `icu_*` (a prefix entry) and `mf2_fn_datetime`
  in every dateless row and ICU4X in the ISO row; a `native-datetime-icu` row
  requires ICU4X, ceiling 2,000,000 B stripped (`strip -o`). The client check
  is in `codegen_matrix.rs` (the B6 grep; `size.rs` greps nothing): `cargo tree`
  and the wasm's names for `hydrate,host-web-datetime-intl`.
* 19.4: the guide, `[dates]`, `date-mismatch` and `unread-data` (a non-lint
  section of `lints.md`), a heading per date feature in `features.md` (its
  reference test), the 3.0.0 changelog. Old names remain only in the
  changelog (table, 2.0 history, the Fixed entry), the 2.0.0 api baseline and
  the generated `feature-costs.md` / `api.txt`.

## Before this phase

* Phase 18's tasks are committed.
* The number split is **not** decided: Phase 22 asks the owner, with the
  figures. The cost table and the guide are written for what is true until
  then: `fn-number` formats in Rust everywhere, and `number-intl` is the
  opt-in. If the owner chooses the split, Phase 22 adds its row and its
  paragraph.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* **Nothing is compiled.** No `cargo build`, `check`, `clippy`, `test`,
  `run`, `doc` or `bench`, no `cargo xtask`, no `cargo leptos`, no `mdbook`,
  nothing under `tools/checks/`, no browser run. Three commands are allowed,
  because they compile nothing: `cargo fmt` (it also finds syntax errors),
  `cargo metadata` and `cargo tree` (they show that the manifests resolve).
* One task, one `mf2-task` agent, one coherent commit by path. The commit is
  made when the task's code and tests are written, `cargo fmt --all --check`
  passes and `cargo metadata --format-version 1` resolves. A file outside
  the workspace is formatted with `rustfmt --edition 2024 <file>`.
* **Write for a compiler that runs later.** Read the definition of everything
  the task calls or changes, and every caller of what it changes (`rg -n`).
  A caller left as it was is Phase 20's compile error.
* **Tests are written, not run.** Each task's "Tests" are written in the
  task and first run in Phase 21. No task weakens or removes a test.
* **Figures are Phase 22's.** A task takes no baseline and measures nothing.
* The guide is for application developers: plain English, no task numbers,
  no reference to `plan/`. The rendered book is never committed.
* **A figure in the guide comes from the cost table.** `docs/features.md`
  includes `docs/feature-costs.md`, which Phase 22 rewrites. Prefer the
  included table to a figure in a sentence. A figure that a sentence must
  state is taken from `plan/08` §1.2, and Phase 22 checks each one against
  the new table.
* What a task could not confirm by reading goes under "For Phase 20", one
  line: what, and where.
* If the design does not settle a choice, stop at a safe point and report to
  the coordinator. The owner decides.

## Tasks

### 19.1 `mf2 stats` says what ships where

Design: `plan/08` §7.

Build: for each language, one line per bundle: the messages, each section a
catalog can leave out, each LOCALE entry, the server-only table's entries.
Each line has its bytes, who reads it (the browser, native code), and where
it ships (the catalog, the server-only table). A closing line gives the
bytes a browser downloads and never reads. `--format json` has the same.

`docs/command-line.md` shows the output, written from the code that prints
it. Phase 21 runs the command over the test's corpus and corrects the page
where the two differ.

Starts at: `crates/mf2-cli/src/stats.rs`; `crates/mf2-build/src/catalog.rs`.

Tests (written here, run in Phase 21): a corpus with dates and numbers under
three feature sets (`intl` in the browser, `icu` in the browser,
`number-intl`).

### 19.2 The cost table's rows

Design: `plan/08` §7.

Build, in `cargo xtask feature-costs`:

* the date rows under the family names: the date functions with `iso`,
  `intl` and `icu` in the browser;
* native rows: `native-datetime-iso` and `native-datetime-icu` against no
  date feature;
* a native row for `native-datetime-icu` in an application with no date and
  one plain placeholder, so that what a date feature costs when nothing
  shows a date stays visible;
* the same for the browser: `leptos-client-datetime-intl` in a client with
  no date and one plain placeholder. It has never been measured;
* per language, the brotli bytes the date slice adds to a browser catalog
  with `icu`, and with `intl` (0).

The command is not run here, and `docs/feature-costs.md` is not edited by
hand: Phase 22 runs the command, which rewrites the file.

Starts at: `xtask/src/feature_costs.rs`; `tools/native-canary/`.

Done when: the rows are committed. Phase 22 compares each new row with
`plan/08` §1.2 and names any that differs by more than 10 %.

### 19.3 The canaries

Design: `plan/08` §7.

Build, in `cargo xtask native-canaries`:

* no date feature, a plain placeholder in the corpus: no symbol of `jiff`,
  `icu_*` or `mf2_fn_datetime`;
* `native-datetime-iso`: no symbol of `icu_*`;
* `native-datetime-icu`: a size ceiling that ICU4X's compiled-in data would
  break.

And for the client, in the check that already greps the wasm for locale
data: with `leptos-client-datetime-intl`, no ICU4X symbol, and no `icu_*`
crate in `cargo tree` for the browser target.

Starts at: `xtask/src/native_canaries.rs`, `tools/native-canary/`;
`xtask/src/size.rs`.

Done when: the rows are committed. Phase 21 runs them, and for each swaps
its feature for the one it forbids, which must fail it.

### 19.4 The guide

Design: `plan/08` §3, §4, §5.1.

Rewrite, for an application developer:

* `docs/features.md`: the date families as a table per kind of application
  (a server-rendered Leptos application, a client-only one, a command-line
  tool, a terminal UI, an Axum server); what each formatter costs, from the
  cost table; one formatter per build and which one formats when two are
  on; that a server with ISO dates shows them until the page hydrates, and
  to readers without JavaScript always; that a server's ICU4X text and a
  browser's `Intl` text can differ, with the known cases; what ships where.
* `docs/configuration.md`: `[dates]`.
* A date needs `:date`, `:time` or `:datetime` in the message: a bare
  placeholder given a date is an error (17.5), with the build's check of a
  variable that is a date in one language and bare in another, and how an
  application marks its own date function.
* `number-intl` (17.2): both builds must turn it on, on the `mf2` dependency line and
  not under the application's `hydrate` feature, or the two builds write different
  catalogs and the browser asks for a file the server does not serve.
* `docs/lints.md`: `several-date-formatters`, `unread-data`, the new text of
  `gated-function` and `unused-feature`.
* `docs/getting-started.md`, `docs/call-sites.md`, `docs/native-apps.md`,
  `docs/delivery-modes.md`, `docs/README.md`, `README.md`: wherever a date
  feature is named.
* `crates/mf2/src/lib.rs`'s feature table, the comments of
  `crates/mf2/Cargo.toml`'s `[features]`, `crates/mf2-fn-datetime`'s front
  page.
* `CHANGELOG.md`, `## 3.0.0`: the date features renamed, with §3.6's table;
  a server no longer links ICU4X's data for every language; a browser that
  formats with `Intl` downloads no date data; and, under Fixed, that 2.0.0
  with both date features on put ICU4X in the browser.

Find the old names with `rg -n 'fn-datetime|datetime-icu|datetime-intl' docs
README.md CHANGELOG.md crates/mf2/src/lib.rs`.

Done when: that `rg` finds the names only in the changelog's table. Phase 21
runs `cargo xtask docs`, `cargo xtask docs-rs` and the suite's `docs` check
over it.

### 19.5 Samples and examples on 3.0

Build:

* The dependency samples that say `version = "2"`: `docs/native-apps.md`,
  `docs/mf2-for-developers.md`, `docs/getting-started.md`,
  `docs/delivery-modes.md`, `docs/call-sites.md`, `docs/upgrading.md`,
  `docs/accessibility.md`. Find them with `rg -n 'version = "2"|mf2@2' docs
  README.md`. The sentence in `docs/getting-started.md` that says 2.0.0 is on
  crates.io is the release phase's.
* The feature lists in those samples, in `examples/*/Cargo.toml`,
  `examples/*/i18n/Cargo.toml` and `bench/`, written by `plan/08` §3.5's
  rule for each corpus. `examples/demo-ssr` formats with `Intl` in the
  browser and ICU4X on the server. Phase 21 checks each list against what
  `mf2 check` prints.
* `[package.metadata.docs.rs]` in `crates/mf2/Cargo.toml`: the feature set
  docs.rs builds.

Done when: the samples and manifests are committed. Phase 21 runs the
suite's `docs` and `demos` checks over them.

### 19.6 A cold start

Moved to Phase 21, as 21.6 (`plan/14-phase-21-test.md`): the trial builds
two applications from the guide, so it needs a tree that compiles and passes.

### 19.7 Plan pointers in the code — audit, then repair

Moved here from the release phase, where it was 20.0. It changes comments in
about 300 source files, and a source file changed after Phase 20 is
everything built again: so it is the last task before the first build.

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
path. Generated text changes through its generator, by hand never:
`crates/mf2-locale-data/data/*.txt`, `conformance/REPORT.md`,
`conformance/COVERAGE.md`, `conformance/ledger.toml`, the records under
`bench/`, and the `package.txt` header (11.0 changed that generator
already). This task changes the generators; Phase 21 runs them (21.2), and
the generated files change there.

Citations of `plan/01` to `plan/16` are a different thing and stay: those
documents still exist, and the release phase's second question decides
whether they move to the archive.

If a citation carries a fact found nowhere else in the tree, or the pass
cannot be made mechanical, stop at a safe point and report: dropping the
fact is the owner's call, not the agent's.

Text only: no behaviour and no size moves.

Done when: `rg -n "plans/" crates xtask tools conformance bench examples
fuzz docs .forgejo .github` finds only generated files whose generator is
changed, the report lists those files, and the audit's counts are in the
Done entry.

## For Phase 20

One line per task, only for what could not be confirmed by reading.

* 19.1: the byte figures in `docs/command-line.md`'s stats sample are
  written from the code, not a run; `docs/translating.md`'s generated stats
  sample still shows the old "entry by entry" lines and needs regenerating.
* 19.2: that `Build::check` fills `Catalog::br` and cuts the `icu` slice
  in-process as the i18n build script does (`xtask/src/feature_costs.rs`
  `catalogs`); xtask's first build now compiles `mf2-build` with `icu-blob`.
* 19.3: that the ICU ceiling sits between the two real stripped sizes, and that
  `mf2-i18n-client` built with `CARGO_PROFILE_RELEASE_STRIP=none` keeps names
  and links the date formatter through `registry()` (`xtask/src/codegen_matrix.rs`).
* 19.4: `unused-feature`'s date text (`crates/mf2-build/src/check.rs`) still says a date may
  reach a plain placeholder, and the guide's "a formatter links the date code for every plain
  placeholder" (F10) is unconfirmed since bare dates are Bad Operands; `translating.md`'s stats
  figures are from the code, not a run.

## Phase exit (coordinator)

No run. The suite is Phase 21's; reading `docs/features.md` against the cost
table, and the owner's figures, are Phase 22's.

1. Add a Done line for the phase.
2. Go on to Phase 20 (`plan/13-phase-20-compile.md`).
