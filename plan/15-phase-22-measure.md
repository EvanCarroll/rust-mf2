# Phase 22 — every figure, and the owner's report

The last of the three phases that follow the code (`plan/08` §8; owner,
2026-10-03). Phase 21 left a tree on which every check passes. This phase
takes every measurement that Phases 16 to 19 owed, once, on that tree; tells
the owner all of it in one message, with everything that moved the wrong
way; and asks the one open question, the number split, with its figures.

No task in Phases 16 to 19 measured anything, so there is no figure per
task. A change is measured against what it replaced by switching it off in
this tree (a feature, a setting), or against a figure recorded before the
code went in ("The baseline"). Where neither exists, the report says
**not measured**.

## How to run this phase

The session given this file is the coordinator. It reads no code. It takes
the tasks in order: it sets "In flight" to the task, starts one `mf2-task`
agent with a brief of four lines (the task's number, this file, "measure and
report; change nothing but the results file the task names", and the
standing rules), waits for its report, and goes on. 22.1 and 22.11 it does
itself. After 22.11 it **stops for the owner**. The answer may add tasks;
when they are done it carries out "Phase exit". The release, Phase 23,
starts in a fresh session.

## State

* **In flight:** nothing. A worktree made for a task is removed once its work
  is merged.
* **Next:** 22.9–22.10, the cost table again, 22.11

## Done

* 22.1 (2026-10-04, `bash tools/checks/compare.sh p14 p21b`, the exit run of Phase 21): `b1`
  26,876 → 26,842 (−34), `app` 42,080 → 42,028 (−52), `b5`/`b5v`/`b7` unchanged, `b13` avoided
  +13,573 → +13,581; **`tui` 1,328,664 → 1,331,976 (+3,312)**, frame time unchanged against the
  baseline binary (367.2 vs 369.2 µs); **`rlib` 46,316 → 46,520 (+204)**; the second `rlib`,
  603,534, is 21.5a's new dated corpus. Tests 956 → 1,027. To 22.2: `tui`, `rlib`, and 17.3.
* 22.2 `tui` (+3,312 B; `CARGO_PROFILE_RELEASE_STRIP=symbols cargo build --release --bins
  --manifest-path examples/tui/Cargo.toml`, bisect and cherry-picks, `target/attr-sizes.txt`):
  3014fd9 (17.1, `server-data` through `host-std`) +496 B, code for a table a terminal UI never
  has; 23a80ce (17.5, the date walk and `with_dates` gone) +2,816 B, the part not isolated;
  548eca5 0 B. Calls for 22.12 and 22.13.
* 22.2 `rlib` (+204 B; `cargo build -p mf2-i18n-fixture --no-default-features --features hydrate
  --target wasm32-unknown-unknown --release`, `ar tv`, `target/rl-sizes.txt`): 23a80ce +170 B, the
  `formats_dates` default method each function in the generated table now carries (code an
  application keeps); 548eca5 +32 B of metadata only; 3014fd9 +2 B. To 22.13 with the TUI's.
* 22.2 17.3 (`cargo xtask size`; demo-ssr: `cargo leptos build --release --split --frontend-only`,
  `node tools/checks/measure-demo.mjs`): 0 B in `b1`/`app` (no dates there); demo-ssr +466 B gz
  (wasm +451), though its pair — ICU4X server, `Intl` browser — is never rewritten; +21 B raw a
  page (`data-mf2-dates`). Calls for 22.14.
* 22.12 (326061e): `server-data` only with `ssr`/`axum`; the TUI 1,331,976 → 1,330,408 B (−1,568).
  22.13 (b4bdd8d): `formats_dates` gone, a date function is one whose parts are `datetime`; the
  TUI → 1,327,528 B (−2,880; much of it clap's layout, which another change can move), the
  fixture rlib 46,520 → 46,280 B. The TUI is now 1,136 B under `p14`. (`STRIP=symbols cargo build
  --release --bins --manifest-path examples/tui/Cargo.toml`.)
* 22.14 (94405c5, 24d66d1): the cfg `mf2_date_rewrite` only for a pair that can rewrite; demo-ssr
  248,067 → 247,768 B gz (−299, not −466: about 210 B of 17.3 remain, not attributed); the ICU4X
  page states no formatter (`icu/states-none`); `l7-web` 70/70; `stated_date_formatter` hidden.
  (`cargo leptos build --release --split --frontend-only`, `node tools/checks/measure-demo.mjs`.)
* 22.3 (`cargo xtask feature-costs`, `--check` holds; absolutes rebuilt the xtask's way, `target/abs.sh`):
  client gz iso / icu / intl 699,516 / 758,909 / 699,832; native iso-less / iso / icu 419,784 / 587,104 /
  748,592. >10 %: browser icu 99,887 → 59,272; native `fn-number` 8,080 → 9,920 (canary gained a
  placeholder); native icu 893,136 → 748,592 (§1.2). **Unused and not 0: `native-datetime-icu` +146,080 B**
  (client `intl` unused −2 B). Slice brotli ar-XB/en/en-XA/pl: 316/466/298/317 with icu, 0 with intl.
* 22.15 (bdd27b8): the 146,080 B were jiff's system-zone code, read whenever `datetime` was on;
  now only the dates host (`ZonesStdHost`, named only when a message calls a date function)
  reads the zone. The row is −40 B (`STRIP=symbols cargo build --release --manifest-path
  tools/native-canary/Cargo.toml --features native[,native-datetime-icu,no-date-message]`).
  Without a date message `time_zone()` answers UTC.
* 22.4: demo-ssr's server, stripped, 11,939,208 B at 2cbbcac (`fn-datetime` + `datetime-icu`) →
  7,429,888 B now (`leptos-server-datetime-icu` + `leptos-client-datetime-intl`), −4,509,320 B
  (−37.8 %). `cargo leptos build --release --server-only` then `strip -o`, the old one from
  `git archive 2cbbcac` with the tree's `Cargo.lock` files (`target/b224.sh`).
* 22.5 (`mf2 stats -C <corpus> --features fn-number,leptos-server-datetime-icu,leptos-client-
  datetime-{intl|icu}[,number-intl] --format json`, nine languages): browser catalog br with `Intl`
  93–206 B, the date slice in the server table (247–1,652 B; 28,657–41,885 with a zone name); with
  ICU4X in the browser 281–853 B (15,719–17,958 with a zone name). `number-intl` moves 1,951–2,894
  B of number data to the server (catalog 1,099–1,536 → 110–113 B). Unread by a browser: 0, all 16.
* 22.6 (narrow = `auto`, widest = `[dates] calendars = "all"`, `zone-names = true`;
  `target/p22-6/`): browser client with `leptos-client-datetime-icu` 758,900 vs 801,143 B gz
  (−42,243); native canary 748,456 vs 900,384 B stripped (−151,928); slice 14–17 B smaller a
  language; `mf2 check`: "ICU4X dates: the Gregorian calendar only …; no zone names …".
* 22.7 (`target/p22-7/`): `date_cost` uncached → cached, µs over 6 alternate runs: en `:datetime`
  3.01–3.14 → 0.62–0.66; ja 2.25–2.66 → 0.59–0.74; de 1.90–2.26 → 0.63–0.79; a zone name 27.60–
  32.14 → 1.25–1.55; ISO 0.23–0.53 both. Native canary: the cache costs 13,032 B stripped, and no
  setting turns it off. Browser: +1,764 B gz, over 1 KB, so the browser does not get it.
* 22.8 (`tools/e2e/datetime/speed.sh`, under load; Chromium + Firefox, no WebKit installed): ICU4X
  / `Intl` ns, en pl ar: a date 1.55–2.60, date and time 1.85–2.91, a zone name 6.00–8.13 (4×
  throttle alike). Cached ICU4X / `Intl`: 0.42–0.69, a zone name 0.75–0.98; the cache is 3.5–4.3×
  (7.1–9.0× with a zone name), but 22.7 keeps it out of the browser. Script ran unchanged.

## Before this phase

* Phase 21's exit is recorded: every check passes, the tree is clean.
* No other build is running, in this repository or in another project on
  the machine: `pgrep -af 'cargo|rustc|rust-analyzer|checks/run.sh'`. A
  timing taken beside one is marked "taken under load", and taken again if
  the owner's question turns on it.

## Standing rules

* Never read, search or list `plan/archive/`. Restrict every `rg` to named
  directories.
* **Every figure comes with the command that produced it**, in the Done
  entry and in the report. A figure that was not taken is listed as not
  measured.
* **A figure that moved the wrong way is the owner's to see** (`plan/08`
  §2.1), with its measurement and its command, in 22.11's one message. So
  is any known departure from the smallest or fastest option.
* One build at a time, `CARGO_BUILD_JOBS=3` (`cargo leptos`: 2). A long
  command runs in the background and is waited for. No LSP tool.
* Timings on this machine drift by up to 30 % within a day. Compare two
  builds by building both, keeping both binaries, and running them
  alternately; give the range over the runs.
* A measuring task commits its results file by path and nothing else. It
  changes no code. What a figure calls for is a new task (22.12, …), which
  the coordinator writes under Tasks before starting it, with the narrow
  check that confirms it.
* A Done entry is at most five lines.

## The baseline

Recorded before any code of Phases 16 to 19 went in.

**The suite's table, `p14`** (2026-10-03, `f68832e`, the exit of Phase 14;
`target/p10-checks/p14/summary.tsv`). Phase 15 took no run of its own. Its
one change to shipped code is 15.2a, which touched only a build with both
old date features on.

| Figure | `p14` |
|---|---|
| `b1`, the client's fixed cost, B gz | 26,876 |
| `app`, the whole app, B gz | 42,080 |
| `b5` / `b5v`, B gz a site | 8.2 / 10.4 |
| `b7`, a catalog, B brotli: en / pl / en-XA / ar-XB | 17,962 / 24,081 / 21,531 / 18,417 |
| `rlib` (codegen-matrix), B | 46,316 |
| `b1p` / `b13` (b12-generated), B | +0 / +13,573 |
| `tui`, stripped, B | 1,328,664 |
| `allocs` a frame, en/de/es/fr | 1329 / 1329 / 1329 / 1328 |
| demo files | 31 |
| tests in `ci` | 956 |
| the tests' own run time in `ci`, s | 1,721 |

**The date figures:** `plan/08` §1.2, and the cost table as it was:
`git show 2cbbcac:docs/feature-costs.md`.

## Tasks

### 22.1 What moved in the suite's table (coordinator)

`bash tools/checks/compare.sh p14 p21`

Expected to differ: the demo files (their features changed), the canaries'
count of sets, the counts of tests. Every other flag is a figure for the
report. One that no task explains goes to 22.2. If `p14` is gone, compare
`target/p10-checks/p21/summary.tsv` with the table above by hand.

### 22.2 A figure that nothing explains

Only if 22.1 has one. One agent per figure. It finds the change that moved
it: on a scratch branch, revert the suspected task's commits
(`git switch -c attribute`, `git revert <commits>`), build only the workload
that gives the figure, read it, and go back (`git switch main`,
`git branch -D attribute`). If the revert does not apply or the tree does
not build without the commit, the figure is reported as moved and **not
attributed**.

17.3 is the first suspect for the Leptos client: it is the one task of
Phase 17 allowed to move the client's bytes, and its cost is wanted in any
case. Measure it this way even if nothing is flagged.

### 22.3 The cost table

`CARGO_BUILD_JOBS=3 cargo xtask feature-costs`

It is many builds, one at a time, and it rewrites `docs/feature-costs.md`
(19.2's rows). Compare each row with the table as it was and with
`plan/08` §1.2, and name any that differs by more than 10 %. Then
`cargo xtask feature-costs --check` passes. Commit the table.

The report takes from it: the client with `intl`, `icu` and `iso` (15.2a
measured 703,921 B gz with both old features on, the same workload); a
native binary with no date feature, with `iso` and with `icu` (427,240,
595,616 and 893,136 B; and nowhere near the 5,041,944 B of ICU4X's
compiled-in data); a date feature that is on in a client and in a native
binary that show no date; the slice's brotli bytes per language.

### 22.4 The server binary

`examples/demo-ssr`'s server binary, stripped, as it is now; and as it was
at `2cbbcac`. Its earlier size was never recorded, so build it once from an
export of that commit (`mkdir -p target/before && git archive 2cbbcac |
tar -x -C target/before`, built there, into its own target directory, with
the tree's own `Cargo.lock` files copied in so that both builds resolve the
same versions), and remove `target/before` afterwards. Give both figures and
the two commands.

### 22.5 What ships where

With `mf2 stats` (19.1), for the corpora of `plan/08` §1.2 (one date shape;
seven shapes; seven shapes and a zone name) and one with a variable currency
code:

* a browser catalog's brotli bytes with `intl` in the browser and with
  `icu`, and the server-only table's bytes beside it (17.1);
* the same with `number-intl` on and off (17.2);
* the closing line: the bytes a browser downloads and never reads. It must
  be 0 under every feature set of the test.

### 22.6 The narrowest ICU4X (18.1)

For a corpus of Gregorian languages with no zone name, the narrow form
against the widest (`[dates]` `calendars = "all"`, `zone-names = true`): the
client's gzip bytes with `leptos-client-datetime-icu` (the harness measured
42,820 against 83,028 B); the native canary with `native-datetime-icu`; a
slice's bytes. And `mf2 check`'s line for the form it chose.

### 22.7 The formatter cache (18.2)

* `date_cost` with the cache and without, the two binaries run alternately
  (the baseline: 2.55 to 3.94 µs, 27.07 µs with a zone name; ISO 0.32 to
  0.47 µs).
* The native canary's bytes with the cache and without.
* The browser build with `leptos-client-datetime-icu`, as it is and with the
  cache added for the measurement. If the cache adds 1 KB of gzip or less,
  write a task that turns it on for that build; if more, the browser does
  not get it, and the report says so with the figure.

### 22.8 The speed of a date in a browser (18.3)

Run 18.3's script: Chromium, Firefox and WebKit; `Intl` against ICU4X; a
date, a date and time, and one with a zone name. The results file beside the
harness is committed. This is the first measurement of it: `plan/08` §9.

### 22.9 The number split (18.4)

Run 18.4's script against the Rust path, on the probe's corpus and language
panel: the client's gzip bytes; a catalog's brotli bytes per language; the
time per placeholder in three engines; where the text differs. The results
file beside the probe is committed.

### 22.10 The guide's figures

Read `docs/features.md` once as a user would, with the new table in it.
Every figure a sentence states, there and in the other pages 19.4 rewrote,
is the table's; correct those that are not. Then
`bash tools/checks/run.sh p22-docs --only docs`. One commit.

### 22.11 The report, and the question (coordinator)

One message to the owner, in plain English, with no task numbers:

1. **Before and now**, each with its command: the client with dates; a
   browser catalog, with and without a zone name; the server binary; a
   command-line tool with and without dates; the terminal UI; a date's time
   natively and in a browser; the client's fixed cost and the whole app; how
   long the tests take.
2. **Everything that moved the wrong way**, and every known departure from
   the smallest or fastest option, by how much, with its command. What 17.3
   added to the client. Whether the browser gets the formatter cache.
3. **Everything not measured**, and why.
4. **The question:** should a browser write currency and unit names through
   `Intl` by default, as an opt-in, or not at all. With 22.9's four figures,
   say what each answer means for an application: its bytes, its speed, and
   the languages a browser has no names for.

Then stop. Write the answer into `plan/08` §2.7. If it needs code — a
feature of `mf2`, the cost table's row, the guide's paragraph — that is a
task here, with its narrow checks, and `cargo xtask feature-costs` is run
again for its row.
### 22.12 No server-only table in a native build (from 22.2)

A native build never writes the server-only table (17.1), yet `host-std` turns on
`server-data`, so a terminal UI links its reader (+496 B). Turn it on only where a build can
have the table: a server whose browser side reads a different catalog. Confirmed by the
TUI's stripped size (−496 B against c63af17) and 17.1's slicing tests.

### 22.13 What 17.5 added to a terminal UI (from 22.2)

17.5 added 2,816 B to the TUI, which formats no date. Find which part a native build links
(the date walk over FUNCS has only a Leptos caller; the bare-placeholder path changed for
every placeholder; `formats_dates` is a method in every function's vtable, +170 B in the
fixture's rlib) and remove what a build without dates never runs, keeping 17.5's
behaviour. Confirmed by the TUI's stripped size and 17.5's tests; what cannot go is said,
with its bytes.

### 22.14 No rewrite code where the pair never rewrites (from 22.2)

The build knows both sides' date formatters. Where the pair never rewrites (the same
formatter on both sides, or ICU4X on the server and `Intl` in the browser: `plan/08` §4.3),
the browser links no rewrite code and the page states no formatter. demo-ssr pays +466 B gz
and 21 B a page for code that never runs. Confirmed by demo-ssr's gzip bytes (−466 B) and
17.3's browser test (the ISO page is still rewritten, the ICU4X one is not).

### 22.15 No ICU4X in a native binary that shows no date (from 22.3)

With `native-datetime-icu` on and no message calling a date function, the native canary is
still 146,080 B bigger (`cargo xtask feature-costs`' finding, 2026-10-05), though the build
knows no message formats a date (8614ca8 names no date host then). Find what still links
ICU4X (a symbol diff of the two binaries) and leave it out, keeping every date test. The cost
table runs once more after the code tasks, before the report. Confirmed by that row: 0 B.


## Phase exit (coordinator)

1. The owner has answered, and every task the answer or a figure added is
   done, each confirmed by its narrow check.
2. `plan/08`: nothing in §9 is still unverified, and §1.2's figures have the
   new ones beside them.
3. Add a Done entry for the exit: the commit, and the report's table in at
   most ten lines.
4. Stop. Phase 23 (`plan/16-phase-23-release.md`) starts in a fresh session.
   Its pre-flight is the full run on the final commit.
