# 18 — Phase 10 work order: 2.0, the user experience

Part of the [master plan](00-master-plan.md) (§9, P10). RFC 2119 keywords
apply. Written on 2026-09-28, at the owner's request, from four sources:
- a UX review of the library, written 2026-09-27. It is kept outside the tree (`comparison.md`,
  untracked), so its findings are transcribed below;
- a report on why `mf2` always depends on `leptos-mf2`;
- the owner's staged MF2 port of a real Ratatui application (`vendor/trippy`, also untracked);
- the thirteen owner answers below.

Phases 1–9 built the library and released it as 1.0.0. Phase 10 rebuilds
the way people **use** it, for a 2.0.0 release:
- **Ratatui first:** best in class, unless that costs performance or executable size;
- **the web too:** its setup cut down;
- **what failed silently fixed:** the tooling gaps the review found;
- **a book that teaches:** MF2 itself, not only this library.

What comes after (hot reload, editor tooling, per-route catalogs, …) is
kept open by each decision, not built.

## State at the start (commit `7c8d1c0`)

| In the tree or on crates.io | Where |
|---|---|
| All 16 original crates published at **1.0.0** (2026-09-26; 11–37 downloads each on 2026-09-28, likely mirrors). The workspace says `1.1.0`. `mf2-native` and `mf2-ratatui` were never published. 1.1.0 is not published, and won't be (owner question 1) | crates.io API; `Cargo.toml` `[workspace.package]` |
| `docs/versioning.md` "Where the releases stand", `README.md` "Status", `docs/getting-started.md` and the 1.1.0 changelog entry still say that only five crates reached 1.0.0 | those files |
| The call-site types (`Tr`, `TrArgs`, `TrRich`, `TrDyn`, `ArgValue`, `DateTimeValue`, `Text`, the markup traits) are declared in `leptos-mf2` (`tr.rs`, `arg.rs`, `dynamic.rs`, `markup.rs`). They sit there because of the orphan rule, and `mf2` re-exports them | [04](04-leptos-integration.md) §2.1; `crates/mf2/src/lib.rs` |
| `mf2` depends on `leptos-mf2` always (without its default features). Its `ssr`/`hydrate`/`csr` pick no Leptos line; the line comes from an application's own `leptos-mf2` dependency | `crates/mf2/Cargo.toml`, root `Cargo.toml` |
| The Leptos 0.8 line is renamed back to `leptos` at `leptos-mf2`'s and `mf2-axum`'s roots (`extern crate leptos_0_8 as leptos;`). This is so that the paths `view!` and `#[component]` write into the crate reach the active line. Only `components.rs` uses those macros: 14 uses, six components | `crates/leptos-mf2/src/lib.rs:95-104`, `components.rs` |
| `to_string()` / `to_plain_string()` / `From<Tr> for String` exist only under Leptos, and there is no `Display` (B12's rule for the client). Only `Tr` derives `Debug` | `crates/leptos-mf2/src/glue/view.rs:651-699`, `tr.rs:35` |
| `ArgValue` converts from `&str` (copied into `Arc<str>`), `String`, `char`, `i8`…`i64`, `u8`…`u32`, `usize`, `f32`/`f64` and date values. Nothing for `u64`, `i128`, `bool`, `Cow<str>`, `Path`, `SystemTime` or jiff types. A wrong type is an error about `ArgValue`, which the user never wrote | `crates/leptos-mf2/src/arg.rs:230-322`; `crates/mf2-macros/src/expand.rs:219` |
| Native: `mf2-native` holds an app-owned `NativeI18n`, with `set_locale(&mut self, &str)` and `format(&self, &impl Message) -> String`. Every locale's catalog file is required, and an unnamed system zone becomes a frozen offset. `mf2-ratatui`: `line`/`text(&NativeI18n, &impl Message, &MarkupStyles)`; `MarkupStyles` is a `Vec<(String, Style)>`, and each call allocates per span and per markup name | `crates/mf2-native/src/native.rs`, `crates/mf2-ratatui/src/lib.rs` |
| The book's native page: a separate translation crate (17-line `Cargo.toml` with feature forwarding, 8-line `build.rs`, a `mf2.toml` that is all defaults, 1-line `lib.rs`) in a two-member workspace, and `&i18n` passed to every call. Its `main` never reaches the TUI | `docs/native-apps.md` |
| `mf2 init` scaffolds a web translation crate only. On a native build its module would not compile (`host_web`) | `crates/mf2-cli/src/init.rs` |
| The generated module gates `CATALOGS` and the host on the including crate's own `ssr` / `datetime-*` features, which is why each translation crate declares and forwards them. The build reads that crate's `CARGO_FEATURE_*` | `crates/mf2-build/src/codegen.rs`, `features.rs:167-188` |
| `tr!` is a `#[macro_export]` in an `include!`d file. Inside its own crate it can be called only unqualified, before any `mod` item (rustc #52234) | [05](05-tooling.md) §4 |
| Two locale matchers. Both truncate `zh-Hant-TW` to `zh-Hant` and then to `zh`, before `zh-TW` is ever tried. The web's (`lookup_locale`) then allows any locale of the same language; the native one refuses that for 14 multi-script languages | `crates/leptos-mf2/src/state.rs:209-246`, `crates/mf2-native/src/locale.rs:18-80` |
| `Negotiator::default()` is cookie, then `Accept-Language`. The switcher hard-codes the query name `lang` | `crates/mf2-axum/src/negotiate.rs`, `crates/leptos-mf2/src/components.rs` |
| `mf2-axum` requires Leptos (default `leptos-0-9`). Direct Leptos use is confined to `context.rs`; the rest uses `leptos-mf2` items that are gated on `leptos` but whose content is neutral | `crates/mf2-axum/` |
| `api.txt` and the release's semver check see one feature set per crate (docs.rs's, i.e. `ssr`). The client-only API (`set_locale`, `hydrate_body`, …) is neither listed nor checked | `xtask/src/api.rs`, `release.rs` |
| Budgets as of Phase 9: B1 25,875 B gz; B5 8.4 B gz a site; the whole app 41,466 B gz at 1,860 sites; B7 `en` 18,072 B br; MSRV 1.88; ledger at `current_phase = "P9"`, and `Phase` ends at P9 | [phase-9-results](phase-9-results.md); `conformance/src/matrix.rs` |

**The owner's trippy port** (`vendor/trippy`: Apache-2.0, untracked, staged and uncommitted on upstream `12aca14`):
- **Scope:** `crates/trippy-tui` moved from an in-tree `t!` over a TOML catalog to MF2. 162 call sites, 109 keys, 12 locales.
- **API:** it pins an older `mf2-native` API (`from_embedded` with six arguments, `=1.0.1`).
- **The last build failed with 34 errors** (2026-09-27, before `Emit::Native` existed). Still true of today's design:
  - `$crate::tr!` used inside the crate that generates it (rustc #52234);
  - no `ArgValue` from a `Display` type (`KeyBinding`) or from `Cow`;
  - it needed its own `thread_local! RefCell<NativeI18n>` and a `t!` wrapper;
  - every message became an owned `String`, where upstream borrowed constant text.
- **Tests:** it also deleted upstream's 22 locale tests.
- **Upstream trippy's bugs,** each a class MF2 prevents:
  - a key typo (`t!("chart")` against the catalog's `title_chart`);
  - a placeholder typo (`%{plural_flow}` in two locales);
  - an English-only plural (`> 1`);
  - word order assembled with `format!` (one of them renders "AS awaited: <awaited>");
  - key hints bolded by slicing the translated word (`[h]aide`).
- **Per-frame cost upstream:** every `t!` clones the locale `String`.

## Where the work stands (handoff, 2026-09-28)

Kept current so that any task can be picked up cold, from this file and the
commits. The first session (2026-09-28) ran Part A, C3's data half and E1–E3 as
parallel agents; three were stopped by the account's session limit and were
finished by new agents. **Part A is done**: A9, which the owner added
afterwards, is done, and **A8's design is written and approved by the owner**
([19](19-native-and-terminal.md); question 17). A8's two opening questions, on the
matcher, were answered (questions 15 and 16), and C3's text half read the
matching rules from the specification before A8 stated them. The next tasks
start from fresh sessions or fresh agents, one at a time, as listed under
"Next"; the owner is asked only when a task finds a question or needs a review.

**Done** — records below, in this file:
- **A1** (`8f6569e`, `3a296a9`, `cc2416c`): the release statements; the 1.x baselines; `examples/tui`
  and `cargo xtask tui-gate`; the 1.x binaries in `target/p10-baseline/`; the UX table's 1.x column.
- **A2, A3, A6** (`5a660d1`): `links` + cfg macros adopted; the in-crate `tr!` chosen (a hidden
  exported wrapper re-exported as `tr`, a crate `prelude`); one crate works for the web.
- **A4** (`678bd6a`): the ambient store meets its gate; text borrowed through a hidden runtime
  seam (`probes/p10-ambient/seam.patch`).
- **C3's data half** (`acdbc01`): CLDR's `languageMatching` and `territoryContainment` vendored;
  the evidence for question 11.
- **C3's text half** (`f54f39f`): UTS #35 Part 1 fetched into the cache by `cargo xtask uts35-sync`
  (`third_party/uts35/PIN`: the CLDR repository at `release-48-2`, `docs/ldml/tr35.md`), never
  committed. What §4.3–§4.4 say, paraphrased, is in its record below:
  - the threshold is the implementation's, below the default script distance, so question 15's
    pairs are refused whatever it is;
  - `419` counts inside `$americas`;
  - demotion is the implementation's, a step a little above the default region distance.

  The evidence re-run changed no verdict.
- **E1–E3** (`a72d057`, `3f1eed9`, `894c0c4`): `dropped-markup`, an error by default (floor
  `allow`); `@do-not-translate` messages neither missing nor covered; `mf2 import` checks what it
  would write and writes nothing on an error it brings, and JSON import names XLIFF for new ids.
- **A5** (probe branch `p10-a5-display`; the variant kept as `probes/p10-display/display.patch`):
  `Display` on the four descriptions, the always-on inherent `to_string` / `to_plain_string` and
  `Debug` everywhere are **adopted** — no `Display`, `Debug` or `core::fmt` reaches a client
  wasm; B1 26,400 → 26,317 B gz (gzip noise on 79 fewer raw bytes), B5 +0.03 B, B12 clean.
- **A7** (probe branch `p10-a7-names`; `probes/p10-names/`, with the crate changes as
  `helper-crates.patch`): `mf2::leptos` with no root rename, the components in per-line helper
  crates, hydration green on both lines (the six browser checks on 0.9, and on 0.8 through
  copies of the demos); the coherence rules confirmed with their errors. The table's cost: see B1
  below.
- **A9** (`probes/p10-display-cost/`, run in A5's worktree; its outputs in that worktree's
  `target/a9/`): **`{}` costs 0.15–0.6 KB gz** once per description type, in every client; **`{:?}`
  on a description with arguments costs 11.8–16.5 KB gz** (core's float formatter, with its panic
  paths), and `unwrap()`, `assert_eq!` or a derived `Debug` reach it too. S2 (`Debug` through
  `write_str`) cuts that by 92–93 %; S1 (`write_str` for `Display`) gains nothing. R3 (`Display`
  refusing in a browser build, with our message) costs 0 B and puts 1.x's wrapper `.to_string()`
  calls back on the fmt-free method. 13 silent paths, on both Leptos lines; the check needs a
  debug-profile client build. **Owner question 14, asked after A9:** a lean `Display`, allowed
  everywhere (S3: `Display` pads the text `to_string()` builds; `{}` then costs 25–70 B gz), and
  `Debug` through `write_str` (S2). A8 states both in the design.
- **A8, approved by the owner** (`7ba59ef`, the probe `probes/p10-args/`; `1f94646`, the design;
  the review: question 17): [19](19-native-and-terminal.md), with the web side's
  design in [04](04-leptos-integration.md) §12 and [05](05-tooling.md) §4.1, §6.4, §9.1.
  - **The four samples' exact code** (19 §1): a one-file CLI, a trippy-shaped TUI, a two-crate
    workspace, and the Leptos `hello` in one crate.
  - **The UX targets** (19 §2; method §2's table below): every row falls. Setup lines go 35 → 13,
    48 → 24, 48 → 19 and 96 → 22; crates named go to `mf2` + `mf2-build` everywhere; concepts
    18 → 8, 22 → 14, 22 → 16 and 28 → 18; the Leptos page needs 3 commands, none of them this
    library's.
  - **The gate table** (19 §14).
  - **One probe** (`probes/p10-args/`): a `tr!` argument takes `IntoArg` by value, else any
    `Display` as its text, else our own E0277 message at the argument. 13 of 13 accepted cases
    took the expected step; 3 refused cases gave our message.
  - **17 choices no answer settled** (19 §15). The owner confirmed the first two by name (any
    `Display` type is an argument, as its text; `install()` returns nothing) and approved the rest.
- **B1** (`1023d57`; the scripts in `probes/p10-b1/`, their outputs in the measurement worktree's
  `target/p10-b1/`): the call-site types and the Leptos layer are `mf2`'s (`mf2::leptos`, each
  line through `crate::line`); the six components in `mf2-leptos-ui-0-9` / `-0-8` by **static
  dispatch** (a `Layer` trait `mf2` implements), which held the demos where A7's function table did
  not — demo-ssr −136, demo-csr +11, demo-islands +1 B gz, against +339 / +183 / +7 in the same
  tree; `leptos-mf2` a shim; the tests in `crates/mf2/tests/`. B1 26,332 → 26,329 B gz, B5 −0.004 B
  a site, B7 byte-identical, B12 clean; `ci`, `docs`, `docs-rs`, `codegen-matrix`, `scenarios`,
  `leptos-0-8`, `l6-web`, `l7-web`, `churn`, `msrv` and the six browser checks on both Leptos lines
  green; the ledger unchanged. `Display` (S3) and `Debug` (S2) on the moved types: 19 §14's third
  row holds but in demo-islands, an owner question (below).
- **B2** (`90c8b2a`, whose three renames went in one commit early, with `46303b9`; the scripts in
  `probes/p10-b2/`, their outputs in the measurement worktree's `target/p10-b2/`): `mf2-native`'s
  code is `mf2::native`, behind `native` (which implies `host-std`, and beside `hydrate` or `csr`
  is a `compile_error!`, for `wasm32` only since question 24); `LocaleSource` keeps 1.x's name
  (B2's `LocaleOrigin` undone by B1's review fixes, question 23);
  `NativeI18n` and `NativeError` keep 1.x's names for C2; `mf2-native` a shim with 1.x's names;
  the tests in `crates/mf2/tests/native.rs`. **Every web artifact byte-identical** (B1 26,344 B gz,
  B5 8.334 B a site, B7's catalogs, the three demos' wasm, `b5 --view`), B12 clean; `ci` (with
  two new steps for `native` alone), `docs`, `docs-rs`, `codegen-matrix`, `scenarios`,
  `leptos-0-8`, `l6-web`, `l7-web`, `churn`, `msrv` and the six browser checks on both Leptos
  lines green; the ledger unchanged. Natively, `tui-mf2`'s allocations per frame identical and
  +176 B stripped.
- **B1's review fixes** (items 1–9: `418b189`, `09aed9a`, `f5e72fd`, `5eb881e`, `c7dd54d`,
  `1552c1d`, `f51b30e`, `f1ce0d0`; record below): the patches and the script behind B1's figures,
  re-measured (+38 / +56 B gz; the buffer's build byte-identical to B1's own); "wraps", not
  "re-exports"; two modes with a line **confirmed** (the helper's sentence, alone) and fixed, `cargo
  xtask refusals` in `ci`; the shim's layer paths **confirmed** lost when only `mf2` has the mode
  (11 errors) and fixed, with a test in `ci`; `mf2::native::LocaleSource` again; B2's demo hashes
  from a committed script; `mf2`'s implicit feature `jiff` **confirmed** and gone; `native` beside
  `csr` across a workspace **confirmed**, an owner question (below). `ci`, `leptos-0-8`, `l7-web`,
  `docs`, `docs-rs` green; every client wasm byte-identical (the size workloads, `b5 --view`, the
  demos), B12 clean.
- **The browser-only refusal** (question 24; `1dd5058`; record below): `mf2` refuses `native`
  beside `hydrate` or `csr` only when compiling for `wasm32`, in a sentence that says what a
  browser build needs; on the host the two compile together, so `unify.sh`'s `--workspace` case
  exits 0 (101 before). `cargo xtask refusals` checks both sides: the nine refusals, and three
  host combinations compiling, 1.x's workspace among them (with a negative control). `ci` and
  `docs-rs` green; every client wasm byte-identical (the size workloads, `b5 --view`, the demos),
  B12 clean.
- **B3** (`afd3811`; record below): `mf2-ratatui`'s code is `mf2::ratatui`, behind `ratatui`
  (which implies `native`; `ratatui-core` 0.1.2, the latest release); `MarkupStyles`, `line` and
  `text` keep 1.x's names for C5 and C8; `mf2-ratatui` a shim with 1.x's names, held to `mf2`'s
  items by a test; the tests in `crates/mf2/tests/ratatui.rs`. Beside `hydrate` or `csr` it is
  refused for `wasm32` only, in a sentence of its own that names `ratatui`, and compiles on the
  host; `cargo xtask refusals` checks both sides (11 refused, 6 host). **Every web artifact
  byte-identical** (the size workloads, `b5 --view`, B7's catalogs, the demos but demo-ssr's
  `__wasm_split` loader), B12 clean; `ci`, `docs`, `docs-rs`, `codegen-matrix`, `scenarios`,
  `msrv`, `churn` green; the ledger unchanged. Natively, `tui-mf2`'s allocations per frame
  identical and −240 B stripped.
- **B4** (`6f3a87b`; record below): the repository's own users name `mf2`, its features and
  `mf2::leptos`: conformance (`conformance/`, `l6-web`, `l7-web` and its sets), `bench/churn`,
  `bench/fluent-ab/mf2` with `cargo xtask fluent-ab`'s shell edit, and the templates `tr-view` and
  `fluent-converted` (`tr` named `mf2` alone already). The examples, the book and `mf2 init` stay
  on the shims (C8, D5, D6). B1 26,720 B gz, B5 8.167 B a site, the whole app and the size
  workloads' wasm unchanged; `b5 --view` 24,634 → 24,636 B gz fixed and 10.524 → 10.520 B a site
  (`tr-view`'s wasm the same length, only its data section's bytes moved); B7 byte-identical, the
  demos byte-identical but demo-ssr's `__wasm_split` loader, B12 clean; `conformance-report`,
  `l6-web` 20/20, `l7-web` 34/34, `ci`, `leptos-0-8`, `churn`, `fluent-migrate`, `fluent-ab` and
  the six browser checks on both Leptos lines green; the ledger unchanged.
- **B5** (`97b0cd4`; record below): `mf2`'s public API listed per mode,
  `crates/mf2/api/{core,ssr,hydrate,csr,native,ratatui}.txt`, from `[package.metadata.api]` in its
  manifest, and compared with 1.0.0 per mode by `cargo xtask release` (1.0.0's Leptos line spelled
  `leptos-mf2/leptos-0-9` there). The listings now name `mf2::leptos::islands_gate!` and, in the
  `leptos-mf2` shim's, each name its glob re-export brings. **Part B is done.** The negative control
  fails `api --check` on `api/hydrate.txt` alone; before B5 the same item passed it. Against
  1.0.0, `ssr`, `hydrate`, `csr` pass, `native` and `ratatui` are skipped (1.0.0 had neither), and
  `core`, `leptos-mf2` and `mf2-build` are refused for changes made before B5, which 2.0.0's major
  allows (routed to G2). No crate source changed, so sizes were not measured; `ci` green.
- **C1** (`232f0ac`; record below, after Part C's table): `tr!`'s arguments through
  `mf2::IntoArg`, implemented per type as 19 §7 lists (integers exact to 128 bits, `usize` without
  saturation; `bool`; `Cow<'static, str>`; paths and `SystemTime`; jiff's instants and civil
  dates; signals of any `IntoArg`; `&T` for `Copy` ones), with a message of ours at the argument
  for a type that is none of them. The dispatch has **four steps** where 19 had three: after
  `IntoArg`, 1.x's `From<T> for ArgValue` (an application's own impl, and generic code bounded on
  it), then `Display`'s text. It picks the step from the type on a zero-sized probe, in a closure,
  and passes the value straight in as 1.x's `ArgValue::from(e)` did: two earlier forms changed the
  code around every argument (+1.0 B gz a site in `b5 --view`, then stack-layout noise), this one
  compiles the size workloads to HEAD's code (the same lengths; only function numbering differs).
  B1 26,720 → 26,728 B gz, B5 8.167 → 8.160 B a site, `b5 --view` 24,636 / 10.520 → 24,682 /
  10.502, B7 byte-identical, the demos the same lengths (demo-csr byte-identical), B12 clean.
  Natively, `tui-mf2`'s allocations per frame identical and +1,760 B stripped (the exact path of
  its `usize` arguments; routed to C2). The inline short string for a `&str` was measured and not
  kept. The checks are in the record.
- **C2** (`c5b602e`; record below, after C1's): the ambient store in `mf2::native`, A4's shape —
  `install(&CORPUS)` (nothing returned; a panic naming the catalog, or a second corpus),
  `install_from_directory` (a partial set, the source language's file required), `set_locale`,
  `locale()`, `locale_source()`, `with_locale(&CORPUS, …)` (before `install` too), the bidi and
  zone settings; `Catalogs`, the explicit form, and `Error` (1.x's `NativeError`); 1.x's
  `NativeI18n` rebuilt on `Catalogs` for the shim. With `native` the descriptions have `Display`,
  `to_string()`, `to_plain_string()` and `to_cow()` (a simple message borrowed from the
  executable), through the one lookup: the request's or the page's catalog, then the thread's
  language, then the app-wide one. The system zone by name, else its POSIX rule
  (`TimeZone::rules`, evaluated by the native host), never a frozen offset; `Debug` on the
  runtime's and the catalog's 31 types. The browser checks green at HEAD, on both lines. Natively
  `tui-mf2` 1,804,776 B stripped (−160,544 against 1.x's: the frozen zone's `Timestamp::now()`
  linked jiff's error formatting), allocations identical, time not resolved from 1.x's; B10
  through the store 0 allocations for a simple message, 1 for a 1-argument one (1.x 1 and 2.27);
  the lookup's first step, with `ssr` unified, about 6–7 ns a format. The web within every budget
  (B1 26,728 → 26,733 B gz, B5 8.2, the clients' code the same with two functions in another
  order), B12 clean. The checks are in the record.

- **C3** (`40ebe16`): one locale matcher over CLDR 48's data (19 §9, its "As built"); a server carries the
  whole table, a native or client-only application its corpus's cut (`LANGUAGE_MATCHING`); `mf2` is
  `MIT AND Unicode-3.0`. B1 and the size workloads byte-identical, hydrated demos +11 / +14 B raw,
  `tui-mf2` +4,568 B; demo-csr +2,870 B gz, an owner question (waiting). `ci`, the checks and the
  browser checks on both lines green; not run: `probes/p10-b4/regen.sh` (no size template changed).
- **The check script** (`56b4398`): `probes/p10-checks/run.sh LABEL [--against EARLIER]` runs the
  whole suite, `ci` first, into one table; `compare.sh A B` flags what moved. `c4-base` (`85672a4`):
  all 20 pass in 26 min; B1 26,733 B gz, B5 8.2, `tui-mf2` 1,809,344 B, 1,815–1,817 allocs a frame.
- **C4** (`f33a3ea`): the generated module (19 §10, its "As built"): `Locale` with the matcher's `FromStr`,
  clap's parser, `format`, `name`; `install`, the locale functions, `markup::*`, a prelude; each choice
  through `mf2`'s cfg macros, one byte table for `CATALOGS` and `CORPUS`. `ci`, `codegen-matrix` (+5
  native), `scenarios`, `conformance-report` green. Departures: no generated `setup()` (an owner
  question, waiting); `format` without `native` and `axum` with D1; `tr` out of the prelude until C6.
- **C4's `setup()`** (`b46c1b5`, questions 30 and 31): generated under a Leptos mode, the matcher's data
  only with `csr`; `install()` calls it. Every hand-written one went (examples, churn, L6/L7 web,
  `mf2 init`, the book), the `tr-view` and `fluent-converted` templates install it, and the changelog
  no longer promises "compiles unchanged". `ci`, `docs`, `codegen-matrix`, `scenarios` green.
- **C5** (`b728666`): Ratatui (19 §8): `From` a description or a reference to one for `Span`, `Line`,
  `Text` through the store, borrowing through the runtime's seam S; `Styled`, `Widget`; `Theme`,
  `set_theme`, `with_theme`, `theme()`. Tested on a `Buffer`; a constant's `Span` / `Line` / `Text`
  allocate 0 / 1 / 2, a placeholder one `String`. A7's set compiles on `mf2` (`leptos`, `ssr` or
  `hydrate`, `ratatui`); `ci` green. The frame gate waits for C8's `examples/tui`.

**In flight:** C6 (the build script), started 2026-09-29 by an agent working in the main tree;
uncommitted changes there are its. Part A's probe branches and worktrees, and B1's and B2's
measurement worktrees, are removed (questions 21 and 25).

**Next, each from a fresh session or agent, one at a time, in lean mode (question 27), as the
agent `mf2-task` once a session has loaded it (question 32):**
- **C6** (the build script) first, checked with `probes/p10-checks/` (its README), as Part C's
  heading orders, then the rest of Part C in that order, each building what 19 designs. D1 (`mf2::axum`) stays
  unblocked by B4, as Part D's heading orders (D1 after B4). One task at a time: the tasks after B1
  touch the same crates and plans. D1's `axum` follows `native`'s and `ratatui`'s rule, refused
  for `wasm32` only (question 24), with its cases on both sides of `cargo xtask refusals`, and joins
  `mf2`'s listed modes (B5's record).

**Owner questions found in the work:** none waiting. C4 found one, answered as question 31 (the
generated `setup()`), with questions 30 (backward compatibility is not a priority) and 32 (task
agents at `high`). C3 found two, answered as questions 28 (the
full matcher in an application with no server) and 29 (`mf2` is MIT AND Unicode-3.0); question 27
(lean mode) was asked after C3. C2 found none. C1 found none. B5 found none.
B4 found none. B3 found none. The browser-only refusal found none. B1's review fixes found one, answered as
question 24: `native` beside a browser mode is refused only when compiling for the browser, and
B3's `ratatui` and D1's `axum` follow it; the browser-only refusal built it, and B3 built
`ratatui`'s. Questions 25 and 26 were asked with it: both measurement worktrees are removed, and
question 24's change came first in the next session.

B2 found two, answered as questions 22 and 23 below: the history with `46303b9` stays as it is;
and the native enum keeps its name, `mf2::native::LocaleSource`, because items in different
modules may share a name (B2's `LocaleOrigin` is undone by B1's review fixes). B1 found four, answered as questions 18–21
below: 19 §14's `{:?}` cap counts our code's own cost, so demo-islands holds (+929 B gz over its
`format!` control); demo-ssr's −136 B gz holds; `CLAUDE.md`'s client-path list is updated; Part
A's probe branches and worktrees are removed. Before them: C3's data half found two; they were asked when A8 started, and answered as
questions 15 and 16 below. C3's text half found none: the case it was to send back (a threshold
above the default script distance) does not arise. A8 found none; its 17 choices (19 §15) went to
the owner's review, which approved them (question 17).

**Found along the way, routed to later tasks** (details in the records):
- From C2 (its record):
  - Every change to a client-path crate: adding items no client reaches (a `Debug` impl) can move
    LLVM's inlining or function order in every client; read a names-kept build
    (`probes/p10-c2/named.sh`) before calling a web move noise.
  - C3: the native matcher copies the candidate tag into a `String` on every `set_locale` and
    `with_locale`.
  - C5 / C8: `examples/tui` still measures 1.x's API over C2's `Catalogs`; the store's frame is
    measured when C8 moves it, against C5's gate.
  - C9: the trippy port's size against upstream starts from a native build without jiff's error
    formatting (C2's −160 KB).
  - F: `with_locale(&CORPUS, …)` before C4's typed form; a second corpus is refused by `install`
    and served by `Catalogs`; before `install`, a native-only build panics, a web one shows no
    text.
- From C1 (its record):
  - ~~C2: `tui-mf2` is 1,967,192 B stripped since C1 (+1,760: the exact path its `usize` arguments
    may take on a 64-bit target), where C2's gate (19 §14) reads "≤ 1,965,320 B"; HEAD was already
    1,965,432. C2's store (A4: a CLI 5,248 B smaller) is measured against it.~~ **Met by C2** (its
    record): 1,804,776 B, the 1.x zone's `Timestamp::now()` path gone.
  - F (the book): `mf2::IntoArg` for an application's own number or date type; a `Cow` that
    borrows for less than `'static` is refused by the borrow checker (pass `&*cow`); jiff's
    `civil::Time` is its text; `SystemTime` and jiff's `Timestamp` / `Zoned` as the plain way to pass
    an instant, where `DateTimeValue::instant` returns an `Option` (the UX review's finding 9 keeps
    1.x's signature). C1 updated `docs/call-sites.md`'s table of argument types; the teaching is F's.
  - G2: `ArgValue::from(usize)` still saturates past `i64::MAX` (1.x's `From`, kept), where
    `IntoArg` is exact; the major could make `From` exact too.
  - Not scheduled: the refusal carries a note naming the hidden `KindNeither::__mf2_kind`; an error
    underlining the whole argument needs `Span::join`, which is nightly-only.
- From B5 (its record):
  - G2: against 1.0.0, `cargo xtask release`'s semver step refuses three crates at 1.1.0, a minor,
    each for a change made before B5 and allowed by 2.0.0's major: `leptos-mf2` (its items are
    `mf2`'s now, and cargo-semver-checks does not follow a re-export into another crate; the shim's
    names are held by its test and its listing), `mf2`'s `core` (`ssr`, `hydrate`, `csr` no longer
    turn on `leptos`: 19 §3's rule) and `mf2-build` (E1's `Lint::DroppedMarkup` moved the later
    variants' discriminants). HEAD's release makes the same three comparisons.
  - G1: `docs/versioning.md` says each crate commits `api.txt`, and every listing's first line says
    "1.x promises"; `mf2`'s listings are per mode in `api/` since B5.
  - D1: `axum` joins `[package.metadata.api.modes]` (`api/axum.txt`), and not
    `baseline."1.0.0"`, since 1.0.0's `mf2` had no `axum`. A glob re-export of `mf2::axum` in the
    `mf2-axum` shim is listed name by name, as `leptos-mf2`'s is.
  - D6: docs.rs shows `islands_gate!` neither in `mf2::leptos` (rustdoc drops a re-export of a
    hidden macro; the module's link to it renders as plain text) nor on the `leptos-mf2` shim's
    page, as 1.x's did; that page, through its glob, shows `mf2::leptos`'s hidden items
    (`installed`, `live_nodes`, `CatalogEntry`, the view states) instead. D6 moves the book's and
    the islands demo's `leptos_mf2::islands_gate!()` to `mf2::leptos`; G1 deletes the shim.
- From B4 (its record):
  - D6 (with D1 and D5): `docs/migrating-from-leptos-fluent.md`, which D6's row does not name,
    finishes a migration on the shims, and so do `mf2 convert --from leptos-fluent`'s
    `leptos-fluent-initializer` finding and 05 §6.2; since B4, `bench/fluent-ab/mf2` and
    `fluent-converted` name `mf2`. They move together; `mf2 init`'s printed steps too (D5).
  - D1: `ci.rs`'s comments say `--workspace` gets `ssr` into `mf2` through `mf2-axum`'s
    `leptos-mf2`; since B4 the conformance crate turns it on itself.
  - Every size A/B that changes a template: `--keep` reuses the generated applications as they
    stand; `probes/p10-b4/regen.sh` generates them again in place, each keeping its lock.
- From B3 (its record):
  - C8 (with G1): the `mf2-ratatui` shim re-exports `mf2::ratatui`'s `MarkupStyles`, `line` and
    `text`. When C8 takes them out of `mf2` (19 §8), the shim keeps its own copy until G1 deletes
    it, or goes then.
- From the browser-only refusal (its record):
  - ~~C2: `native` beside `hydrate` or `csr` now compiles on the host, so the ambient forms must
    compile, and D17's one lookup (the request or the client, then the native thread, then the
    native global) must hold, with a client mode and `native` both on. `cargo xtask refusals`'
    host cases check that `mf2` compiles so; they check no behaviour.~~ **Done by C2:**
    `tests/lookup.rs`, beside `csr` in `ci` and beside `ssr` in `--workspace`.
  - ~~B3~~, D1: `ratatui`'s and `axum`'s cases go on both sides of `cargo xtask refusals`.
    **B3's are in** (its record); D1's remain.
- From B1 (its record):
  - ~~B5: `cargo xtask api` lists neither `mf2::leptos::islands_gate!` (a hidden macro, re-exported)
    nor what the `leptos-mf2` shim re-exports by glob; `release.rs`'s semver check now leaves
    `leptos` out of `mf2`'s features too (1.0.0's needs a line from `leptos-mf2`).~~ **Done by
    B5** (its record): both listed; `mf2` compared per mode, `native` and `ratatui` (B2, B3) too.
  - ~~C2: `missing_debug_implementations` warns in `mf2`; the runtime's and the catalog's types
    remain C2's.~~ **Done by C2:** the lint warns in both, and their 31 types have `Debug`.
  - Whoever next edits `CLAUDE.md`: its client-path list names `leptos-mf2`, whose code is now
    `mf2`'s and the helpers'.
- From B2 (its record):
  - ~~C2: 19 §4's names for the native module, `Catalogs` and `Error`, are given with the API C2
    builds; B2 kept `NativeI18n` and `NativeError`, and the shim keeps 1.x's names whatever C2
    does.~~ **Done by C2:** `Catalogs` and `Error`; the shim keeps both 1.x names.
  - C2 / C8: a native size carries the build's source paths and the rlibs linked beside it:
    `tui-upstream`, which uses no MF2, moved +9,856 B with B2. Compare within one tree and one
    lock.
  - ~~B1's review fixes (item 4) / D1: of 19 §3's refusals, only "both lines" is checked by an
    xtask; `native` beside a browser mode was checked by hand (`probes/p10-b2/refusals.sh`).~~
    **Done by B1's review fixes:** `cargo xtask refusals`, a step of `ci`, checks each; B3 added
    `ratatui`'s rows (a sentence of its own), and D1 adds `axum`'s. Since the browser-only
    refusal it checks both sides: the refusals for `wasm32`, and the host combinations compiling.
  - Every task: a commit made while another task has changes staged carries them (`46303b9`
    carried B2's renames). Stage and commit by path (`git commit -- <paths>`).
- C6: a missing `mf2.toml` reruns the build script on every build (A3); `mf2 check` turns a
  failed `cargo metadata` into false `gated-function` errors (A1); `mf2 check` must see the
  function features the builds use (A6); `neutral-numbers` fires on a corpus whose only
  placeholder is a string (A3).
- ~~A8 (the `Display` / `Debug` design)~~ — **stated in 19 §6**:
  - S3 for `Display` and S2 for `Debug`, with S2's fidelity limits;
  - `Debug` on every public type;
  - the demos keep a nightly `fmt-check` (D6).
- F (the book): `.to_string()` is the leanest; `{}` costs a few dozen bytes, `{:?}` about 1 KB,
  and `unwrap()` / `assert_eq!` on a description reach `{:?}` (A9).
- F (the book): Traditional and Simplified Chinese don't fall back to each other, as CLDR's data
  says; a Traditional reader served the source language means the application needs a
  Traditional catalog, and on the web E4's warning shows it (question 15).
- D6 / CI: a `Display` / `Debug` check on the demos needs a debug-profile client build; a release
  build with names kept misses what LLVM inlines (A9). **A8: the demos keep one, nightly**
  (19 §6).
- Every size investigation that keeps names: `wasm-opt --strip-dwarf` before `-Oz`, or the
  names-kept build is not the shipped one (A9).
- ~~C1: a `&str` argument from a variable is copied into an `Arc<str>` (A4).~~ **Measured by C1,
  not kept** (its record): an inline short string saves 10 of `tui-mf2`'s 1,816 allocations a
  frame, with no time measurably saved, and costs every web client about 500 B raw.
- ~~C2: time the ambient lookup's first step when `ssr` and `native` are unified (A4); the B10
  times need a quiet machine (A1).~~ **Done by C2** (its record): about 6–7 ns a format, under load;
  B10's times are reported under load, its allocations gated (`--gate`).
- C7/C8: Ratatui without its default features needs `layout-cache` (A1).
- D5: the one-crate web starter writes `watch-additional-files = ["locales"]` (A6).
- C3, from its text half, each with a test:
  - the demotion: **A8 states it, unbounded** (19 §9). An exact match 11th in a reader's list is
    refused, and a test shows it;
  - `$!X` for a macroregion that straddles a variable (`en-001`);
  - a desired `und` is not maximized;
  - the section's worked examples.
- F (or whoever next edits `README.md`): its "Current work order" still names
  `plans/17-phase-9-work-order.md` (C3's text half).
- From A8 (19 §16):
  - ~~B2 / D1: `mf2::native::LocaleSource` (an enum) and `mf2::axum::LocaleSource` (a trait) now
    share one crate. B2 renames the native one.~~ **Settled by question 23:** no clash, no rename;
    B2's `LocaleOrigin` is undone by B1's review fixes.
  - C4: the generated names (`install`, `Locale`, `markup`, …) can collide with an application's
    own root items (E0428). A way to rename them waits until an application needs one.
  - C5: collecting descriptions into a `Line` flattens their markup (Ratatui's blanket goes
    through `Span`). The rustdoc says so, and the book recommends one message per styled line.
  - F: a `Display` argument's text is not translated, and a string selector is the MF2 way; a
    styled line is one message.
  - Not scheduled: an argument shorthand, `tr!("id", error)` for `error = error`.
- Every size gate: an A/B is valid only within one tree and one `Cargo.lock` (A4's record).
- Not scheduled: each `.match` message allocates 4 times inside the runtime (A4);
  `mf2-catalog`'s timing test `linear.rs` failed once under load 10–13 (C3 data).

## The UX review's findings, and where each goes

Transcribed from the review (2026-09-27), since its file is not in the tree.

| # | Finding | Goes to |
|---|---|---|
| 1 | A translation can drop markup (`{#link}…{/link}`) and neither `check` nor XLIFF `import` complains | E1 |
| 2 | `mf2 check` ignored the app's features | done (Phase 9 B8) |
| 3 | Missing translations counted but not named | done (Phase 9 B8) |
| 4 | `@do-not-translate` messages count as missing, which adds a warning per language for a switcher's language names | E2 |
| 5 | `mf2 import` accepts translations that fail the checks (an undeclared `$nom`), and JSON import silently skips new ids | E3 |
| 6 | A page rendered without the request's language falls back to the source language silently; with no catalogs installed, every text renders empty | E4 |
| 7 | `Negotiator::default()` ignores `?lang=`, which the switcher submits without the wasm, and the switcher hard-codes `lang` | D2 |
| 8 | Path-prefix sites and the switcher | done (Phase 9 B4) |
| 9 | Missing argument types; `DateTimeValue::instant` returns `Option`; errors name `ArgValue` | C1 (done: its record; `instant` keeps 1.x's signature, and a `SystemTime` or a jiff instant needs no `Option`) |
| 10 | Markup closures need `\|c: AnyView\|` | D4 |
| 11 | The message types can't be printed (`Display`) or derive `Debug` | A5, C2 |
| 12 | `#[cfg(feature = …)]` pairs in app code to switch language, and to read it reactively | D4 |
| 13 | Language codes are strings | C4, D4 |
| 14 | Unclear which crate an app depends on: five from the family; the facade's docs open with low-level types | B1–B4, D1, F |
| 15 | No server use without Leptos; no per-call language on a server | C2, D1 |
| 16 | About 250 lines to copy before anything runs; no starter templates | C7, D5 |
| 17 | `mf2 init` only scaffolds the web; `build.rs` boilerplate; `mf2 --help`'s summary omits `init` and `convert` | C6, C7 |
| 18 | About 1.8 s wasted per translation edit: unoptimized build scripts (fixed by `[profile.dev.build-override] opt-level = 2`, `plans/phase-5a-results.md`); maximum-quality brotli in debug builds | C6, C7 |
| 19 | Missing chapters: an MF2 guide, reference pages (`mf2.toml`, lints, features), the translator workflow, custom functions, testing, troubleshooting, deployment | F |
| 20 | First impressions: the crate map before Getting started; mdBook's playground on by default; mechanism mixed into tutorials. The landing title and the broken crate-doc sentence are already fixed | F |

## Owner questions (all answered, 2026-09-28)

1. **Release** — **answered: skip 1.1.0 and ship 2.0.0.** The redesign is
   the next release, and 1.1.0's queued fixes ship with it. No crate name
   is registered only to be retired. *As put:*
   > All 16 original crates reached crates.io as 1.0.0 on 26 September, with 11–37 downloads each
   > (probably bots and mirrors). The native and Ratatui crates were never published, so 1.1.0 isn't
   > out. Should we skip 1.1.0 and make the redesign the next release, 2.0.0?
2. **Where the call-site types and integrations live** — **answered: one
   crate, `mf2`, with features.**
   - The types move back to `mf2`.
   - `leptos-mf2`, `mf2-native` and `mf2-ratatui` become feature-gated modules.
   - An application names `mf2` plus `mf2-build`.
   - `tr!` goes straight into Ratatui widgets (`Block::bordered().title(tr!("title"))`, and
     `tr!("quit").bold()` through `Stylize`), and into Leptos views as today.

   *As put:*
   > Only the crate that defines what `tr!` returns may make it go straight into a Leptos view or a
   > Ratatui widget (Rust's orphan rule). That's why those types live in `leptos-mf2` today, and why
   > `mf2-ratatui` can only offer helper calls. Where should they live in 2.0?
3. **Native language state** — **answered: an app-wide current language
   with a per-thread override.**
   - `install()` once;
   - `set_locale(Locale::Fr)` switches the app, and the next frame draws in French;
   - `with_locale(Locale::Fr, || …)` pins one thread, so tests run in parallel;
   - `Locale::Fr.format(&tr!(…))` formats with no global at all;
   - `println!("{}", tr!(…))` works;
   - the cost is about one atomic read per message.

   *As put:*
   > A native app today creates an i18n handle and passes it to everything that makes text. The
   > trippy port had to wrap it in its own thread-local and `t!` macro. Web apps never pass one,
   > because the page or request supplies the language. Should native apps get an app-wide current
   > language?
4. **Web scope** — **answered: native and web together**, native and
   Ratatui first. *As put:*
   > Besides Ratatui and CLI apps, should 2.0 also cut the web setup's boilerplate? Candidates: no
   > feature forwarding into the translation crate, a generated setup function, `?lang=` → cookie →
   > browser as the default negotiation, typed language values, a prelude, and switching language
   > without `#[cfg]` blocks.
5. **Ratatui styles** — **answered: one app-wide theme.**
   - It is set once.
   - Common markup names (`b`, `i`, `u`, `em`, `strong` …) are styled by default.
   - The build generates a constant for every markup name the corpus uses, so a typo is a compile
     error.
   - A draw can switch themes with a scoped `with_theme`.
   - There are no per-call style arguments.
   - Messages say what a stretch is (a key, a host), and the theme says how it looks.

   *As put:*
   > In a Ratatui app, markup such as `{#key}q{/key}` or `{#ok}Connected{/ok}` has to become a style.
   > Where should an app say what each markup name looks like?
6. **Axum** — **answered: fold `mf2-axum` into `mf2`** as an `axum`
   feature. A Leptos app then names only `mf2` and `mf2-build`, and the same
   feature later serves a plain Axum application. *As put:*
   > With one crate, should the Axum server support also move into `mf2` (an `axum` feature), so a
   > Leptos app names only `mf2` and `mf2-build`?
7. **The trippy port** — **answered: finish it as 2.0's acceptance test and
   record the results.**
   - Its figures and findings go in the plans, as the leptos-fluent audit did
     ([04](04-leptos-integration.md) §11).
   - The port itself stays untracked in `vendor/`.
   - A trippy-shaped sample committed to the repository carries the repeatable gates.

   *As put:*
   > `vendor/trippy` holds your staged MF2 port of trippy's TUI (162 call sites, 12 languages). It
   > stopped at 34 build errors, several of which 2.0 removes. Should 2.0 be proven by finishing that
   > port and measuring it against upstream trippy?
8. **The Leptos module's name, and Leptos 0.8** — **answered: `mf2::leptos`,
   with 0.8 kept.** The owner, verbatim: "mf2::web is a horrible name because we may support other
   web frameworks, but I don't want to drop support of 0.8. Why can't we have a feature gate change
   the behavior of mf2::leptos for 0.8 and 0.9 (the default)?"
   - **Features:** `leptos` is the 0.9 line and the default; `leptos-0-8` is the opt-in. Changing
     the default line stays a major.
   - **The obstacle, as found (question 13 settles it):** Leptos's `view!` and `#[component]` are
     procedural macros that write `::leptos` into the crate using them. The 0.8 line, listed under
     another name, is therefore renamed back at the crate root, and that clashes with a public
     `leptos` module.

   *As put:*
   > In one crate, the Leptos layer can't be a module named `leptos` while Leptos 0.8 stays
   > supported. Supporting 0.8 means binding the name `leptos` to the 0.8 crate inside mf2, and Rust
   > rejects a module and a crate with the same name there. Which do you prefer for 2.0? — Keep 0.8,
   > name it `mf2::web`; or drop 0.8, name it `mf2::leptos`.
9. **Matching, first form** — the owner, verbatim: "I'm not sure how to answer this, if the user
   requests Traditional, and we have Traditional we should obviously serve Traditional, but I think
   if we don't we should fall back to Simplified, the same is true about es-MX vs es. If we have a
   Mexican variant we should serve it if requested, or we should fall back to spanish. If this isn't
   right please push back and give me more context about the problem." Pushed back: for some
   languages the two scripts are not mutually readable (Punjabi's Gurmukhi and Shahmukhi). That led
   to question 11. *As put:*
   > A reader whose Mac asks for Traditional Chinese for Taiwan (zh-Hant-TW) gets Simplified Chinese
   > (zh) today, on the web and natively, even when the app has zh-TW. 2.0 fixes that with one
   > matcher everywhere. The web and native also disagree when an app only has the other script: the
   > web shows it, native shows the source language. Which should 2.0 do everywhere?
10. **The silent failures** — **answered: required for 2.0** (Part E). *As put:*
    > The UX review also found tooling that fails silently. A translation can drop a link's markup
    > unnoticed; do-not-translate messages count as missing; `mf2 import` accepts broken
    > translations; and a page rendered without the request's language gives no warning. None of
    > these needs a major version. When should they be fixed?
11. **Matching** — **answered: follow CLDR's language-matching data.**
    - A requested script the app has is served. The CLDR likely subtags we already vendor fix
      `zh-Hant-TW` → `zh-TW`.
    - A missing script falls back only where CLDR says readers accept it. Expected, and confirmed
      once the file is vendored: Traditional↔Simplified and Serbian Latin↔Cyrillic yes; Punjabi's
      two scripts no. (The file confirmed Serbian and Punjabi, not Chinese: question 15.)
    - Spanish regions fall back as the owner described.
    - One matcher everywhere (C3).

    *As put:*
    > 2.0 will serve Traditional to a reader who asks for it whenever the app has it. That's a bug
    > today: zh-Hant-TW falls to zh before zh-TW is tried. The fix uses the CLDR likely-subtags data
    > we already vendor. When the requested script is missing, should the fallback follow CLDR's
    > language-matching data, or your rule for every language?
12. **The book** — **answered: the chapters, and starter templates.**
    - The 2.0 book gains an MF2 guide for developers, reference pages, the translator workflow,
      testing and troubleshooting (Part F).
    - `mf2 init` grows into a starter that makes a complete, runnable CLI, TUI or Leptos app,
      compiled by the docs check, so it cannot go stale (C7, D5).

    *As put:*
    > The review's remaining findings concern learning the library. The book is missing chapters: an
    > MF2 syntax guide for developers; reference pages for `mf2.toml`, every lint and the features;
    > the translator workflow; testing and troubleshooting. There are also no starter templates.
    > Should 2.0 include these too?
13. **The six built-in Leptos components** — **answered: one helper crate
    per Leptos line.** Rejected first: rewriting them without the macros. The owner asked why the
    Leptos macros couldn't be used and called the rewrite a bad idea. The explanation given:
    - the macros are the problem only *inside `mf2`*, only because of 0.8. Applications always use
      them, through their own `leptos` dependency;
    - a macro-free rewrite would change nothing for applications. But it would put about 300 lines
      of builder markup in the accessibility- and hydration-critical switcher, and hand-build the
      plumbing `#[component]` generates for `view!`. That plumbing is public but made for the macro,
      so a Leptos release could break it or split it per line.

    **As decided:**
    - The six components move to `mf2-leptos-ui-0-9` and `mf2-leptos-ui-0-8`: the same source, each
      crate depending on its own Leptos under the real name `leptos`, so `view!` and `#[component]`
      are used as normal.
    - `mf2::leptos` re-exports them, so applications still name only `mf2`.
    - The helpers must not depend on `mf2` (Cargo forbids the cycle). `mf2` installs a small table
      of functions at start-up for them: the languages, the current one, switching, preloading, link
      URLs.
    - The rest of `mf2`'s Leptos layer reaches each line through internal aliases, with no root
      rename.
    - A future Leptos line adds one helper.

    *As put:*
    > Leptos's `view!`/`#[component]` can't be used inside mf2 itself while one crate supports both
    > 0.8 and 0.9 under the module `mf2::leptos`. Which way should the six built-in components (the
    > switcher, its options, preload and alternate links, the islands gate) go? — Helper crate per
    > line; generated into the app; without the macros, inside mf2.
14. **`{}` on a message in a browser build** (asked after A9, 2026-09-28) — **answered: a lean
    `Display`, allowed everywhere** (A9's S3), which is what the owner proposed. To the first
    form (refuse it, or allow it at A5's cost) the owner answered, verbatim: "Can we implement our
    own Display on it that does this for us, would this raise a problem with Ux or Ergonomics to do
    that? What's the downside?" A9 built and measured that `Display`, then asked again.

    **As decided:**
    - `Display` pads the text the fmt-free inherent `to_string()` builds: one text path per
      description type, shared by `.to_string()`, `{}` and a wrapper's `.to_string()`.
    - `{}` compiles everywhere. In a browser build it costs 25–70 B gz more than `.to_string()` in
      an application that already calls `format!`, and a wrapper's `.to_string()` (a signal's read
      guard, an `Arc`, a `RefCell` borrow, `&&`) 60–100 B gz. The price: one more `String` per
      `{}`. The book says `.to_string()` is the leanest.
    - `Debug` is written through `write_str` (A9's S2, 92–93 % smaller than the derived one), as
      both options put it.
    - Rejected: refusing `{}` in browser builds (A9's R3), and A5's `Display` as written (a second
      text path per type).

    *As put, first:*
    > In browser builds, should formatting a message with `{}` (e.g. `format!("{}", tr!("title"))`)
    > be refused at compile time, or allowed?

    *Then, after the measurement:*
    > Which should browser builds get for `{}` on a message? — Lean Display, allowed; or refuse it
    > in browser builds.
15. **Traditional and Simplified Chinese** (found by C3's data half; asked when A8 started,
    2026-09-28) — **answered: follow the data.** No project rule goes on top of CLDR's.
    - **The finding.** CLDR 48 has no rule between `zh-Hant` and `zh-Hans`, so the pair takes
      the default script distance (50), exactly as Punjabi's two scripts do. Whatever threshold
      the specification sets, the two pairs fall on the same side of it. With the threshold as
      recalled (C3's text half confirms it), both are refused, in both directions, and so is
      `zh-TW` ↔ `zh-CN`, whose regions imply the scripts.
    - **What readers get.** A Traditional reader of an application that has only Simplified gets
      the next language on their list that the application has, else the source language; the
      reverse likewise. A list that also names plain `zh` gets Simplified through that entry
      (likely subtags read `zh` as `zh-Hans-CN`); by recollection, Chrome's and Firefox's lists
      for `zh-TW` do. Natively a system usually names one language, so there the change shows.
    - **What changes from 1.x** (C3 lists each case): `zh-Hant-TW` and `zh-TW` no longer reach an
      application's `zh` by truncation; the web's "any locale of the same language" step goes.
    - **What tells the developer:** on the web, E4's warning (a page rendered without the
      request's language); the fix is a Traditional catalog. The book says so (F).
    - **Rejected:** a documented project rule, Traditional → Simplified (the owner's first
      expectation, question 9) or both ways; and a per-application setting in `mf2.toml`.

    *As put,* after the background (the data confirms Serbian, Punjabi and Spanish; it scores
    Chinese as it scores Punjabi, so it cannot give both expectations; what a Traditional reader
    sees under each option; today's behaviour served Simplified by truncation):
    > When an app has Simplified Chinese but not Traditional (or the reverse), what should a
    > reader of the missing script get? — Follow the data; Traditional → Simplified (our
    > documented rule); both directions; each app decides (one line in its `mf2.toml`).
16. **The matching algorithm's text** (found by C3's data half; asked with question 15) —
    **answered: fetch it, cache only.**
    - UTS #35 Part 1's language-matching section (§4.4: the threshold, `oneway`, demotion, the
      paradigm locales, match-variable groupings) comes from a pinned upstream, at the release
      matching the vendored CLDR data. It is checked against a digest in a `PIN`, written to
      `target/xtask-cache/`, and never committed, as `spec/` is (D13).
    - The network rule reaches it through that `PIN` and a `cargo xtask *-sync` command, as it
      reaches every upstream.
    - Nothing in the tree quotes it; plans, code and tests paraphrase it and cite the section.
      C3's text half does the fetch and records what the section says.
    - **Rejected:** working from the data and ICU's behaviour as recalled. The threshold decides
      question 15's pairs, and the `419` / `$americas` reading decides whether a Mexican reader
      prefers an application's `es-419` to its `es`, so a wrong recollection would go unnoticed.

    *As put*, after the background (the data holds only distances; the rules for combining them
    are in the specification, which is not in the tree; two results already depend on it):
    > May the build fetch the Unicode specification's language-matching section, the way it
    > already fetches the MessageFormat specification? — Fetch it, cache only; or work from
    > what we know (ICU's behaviour as recalled, each assumption a test with its reason).
17. **The 2.0 design, A8's review** (2026-09-28) — **approved as written.**
    - **Any type with a `Display` is a `tr!` argument, as its text** (19 §7, choice 1):
      confirmed. Typed values keep their own conversions; the text of the rest (an `io::Error`,
      an address, trippy's `KeyBinding`) is not translated, as 1.x's `.to_string()` fix was not.
      Rejected: keeping 1.x's refusal, with a message naming `.to_string()` (C1's fallback, kept
      only for the gate).
    - **`install()` returns nothing** (19 §5, choice 2): confirmed. Embedded catalogs cannot
      fail to load in a sound build; a corrupt executable panics naming the catalog.
      `install_from_directory` returns a `Result`. Rejected: `install()?` (this file's first
      sketch, corrected).
    - **The other fifteen choices** (19 §15, choices 3–17) stand as written. The review summary
      named the switcher that lists every language, the Leptos line named on the dependency,
      `Negotiator` as a `.layer(…)`, the matcher's stated demotion, `Debug` everywhere with the
      demos' nightly check, and what "falls" means for a count of one command.

    *As put*, after a summary of what an application writes in 2.0 and the setup-line targets:
    > Should any type that can print itself (an OS error, an IP address, trippy's key bindings)
    > be accepted as a message argument, using its printed text? — Accept it; or refuse it.
    >
    > Should `install()` return nothing, or a Result the app must handle? — Return nothing; or
    > return a Result.
    >
    > Can API work start on the rest of the design as written? — Approve the rest; or hold
    > while the owner reads it.

18. **`{:?}` on a `TrArgs` in a client that formats nothing else** (found by B1; 2026-09-28) —
    **answered: 19 §14's cap counts our code's own cost.** The 1.3 KB gz cap is measured over a
    base that already formats text: in a client that formats nothing else, over a `format!`
    control (B1's `islands-control.sh`). demo-islands then reads +929 B gz (+1,353 over its base,
    +424 of it `format!`'s own machinery), and the row holds; the other four clients already
    format text, and their figures stand. An application that formats nothing else pays about
    0.4 KB more on its first `{:?}`, as on its first `format!`. Rejected: cruder `Debug` output
    (dates and decimals written less faithfully) to save about 50 B gz in such a client.
19. **demo-ssr's −136 B gz under static dispatch** (found by B1; 2026-09-28) — **answered: the
    gate holds.** After wasm-opt the module is 123 B smaller; gzip reads −136 B and brotli +154,
    the two compressors reading reordered code in opposite directions. Static dispatch stays, and
    question 13's fallback is not needed.
20. **`CLAUDE.md`'s client-path list** (found by B1; 2026-09-28) — **answered: updated.** It names
    `mf2`'s call-site types and `mf2::leptos`, and `mf2-leptos-ui-0-8` / `-0-9`, in place of
    `leptos-mf2`, a re-export since B1.
21. **Part A's probe branches and worktrees** (found by B1; 2026-09-28) — **answered: removed.**
    The branches `p10-a4-ambient` (`389b4be`), `p10-a5-display` and `p10-a5-dev` (`70e0b27`),
    `p10-a7-names` (`6014948`) and `p10-e-silent-failures` (`59cd8ac`, whose E1–E3 are on
    `main`), with their worktrees; A9's `target/a9/` went with `p10-a5-display`'s. The records
    are in this file, and the code worth keeping is in `probes/` as patches. B1's measurement
    worktree stays until B1's review fixes have taken the patches its figures need.

    *As put* (18–21 together, after B1's report):
    > Debug printing ({:?}, which unwrap() and assert_eq! also reach) was supposed to cost a
    > browser app at most 1.3 KB compressed. It measures 0.86–1.22 KB in four apps, but 1.35 KB
    > in the islands demo. That app formats no other text, so its first debug print also pulls
    > in Rust's own formatting code (0.42 KB, which any format! would pull in). How should the
    > limit read? — Count only our code; or make the output cruder.
    >
    > In the server-rendered demo the code itself got 123 bytes smaller, but gzip reads 136 bytes
    > smaller and brotli 154 bytes larger. Does that count as holding the limit? — Yes, it holds;
    > or no, check brotli first.
    >
    > The project's CLAUDE.md still lists the old Leptos crate among the browser-side crates that
    > must stay lean. Update the list? — Update it; or leave it.
    >
    > The merge has taken what it needed from the 2.0 design's experiments. Remove their
    > branches and working copies? — Remove them; or keep them.

22. **The commit that carried B2's renames** (found by B2; 2026-09-28) — **answered: the history
    stays as it is.** `46303b9`, a plans commit, committed the index with B2's three staged renames
    and the removal of `mf2-native`'s `error.rs`. It does not build on its own; `90c8b2a` completes
    it. Nothing was pushed. Rejected: rewriting the three local commits so that each builds. Since
    then the coordinating session commits by path (`git commit -- <paths>`).
23. **The native enum beside `mf2::axum::LocaleSource`** (found by B2; 2026-09-28) — **answered:
    no rename; `mf2::native::LocaleSource` keeps 1.x's name.** Items with one name in two modules
    of one crate compile side by side. `mf2::native::LocaleSource` (the enum: explicit, system,
    source) and `mf2::axum::LocaleSource` (the trait) never share a scope. 19 §16 called them "one
    name clash to settle" only because they now share a crate, and that is not a clash.
    - **A7's clash was a different one** (N1–N5): a *module* named `leptos` or `axum` at `mf2`'s
      root, against the *crates* of those names, in `mf2`'s own root scope.
    - **Where the two names do meet:** only in a scope that glob-imports both modules and writes the
      bare name (rustc then asks for the path), or in one prelude that carries both. Neither needs
      to be in the prelude.
    - **What it spares users:** native users change only `mf2_native::` → `mf2::native::`.
    - **B2's `LocaleOrigin` is undone** in B1's review fixes (item 6).
    - **The rule, for every later merge (B3, D1):** items in different modules keep their names;
      rename only for a clash in one scope.

    *As put* (22 and 23, after B2's report):
    > One of the local commits doesn't build on its own, because my plan commit swept in file
    > moves from the next commit. Should I fix the history so every commit builds? — Rewrite the 3
    > commits; or leave history as is.
    >
    > The native side has an enum saying where the current language came from … In 2.0 it shares
    > one crate with the web server's LocaleSource … so the native one needs a new name. Which
    > name? — LocaleOrigin; LocaleChoice; or LocaleSourceKind.

    The owner answered the second with a question: why a new name, when one name can sit in two
    modules of one crate? After the explanation above:
    > Should the native enum go back to its 1.x name, mf2::native::LocaleSource, beside
    > mf2::axum::LocaleSource? — Keep LocaleSource; or keep LocaleOrigin.

24. **`native` beside a browser mode across one workspace** (found by B1's review fixes, item 9;
    2026-09-29) — **answered: refuse only when compiling for the browser.**
    - **The finding:** cargo unifies features across the packages it builds together. A workspace
      with a browser client (`csr` or `hydrate` on `mf2`) and a native application on
      `mf2-native` failed `cargo check --workspace` and rust-analyzer's check with 19 §3's
      refusal, though each crate compiles alone (`bash probes/p10-b2/unify.sh`). 1.x compiled it.
    - **The rule now:** the refusal applies only to a `wasm32` build, where native-only code
      would reach a browser bundle. On the host the combination compiles, and the changelog's
      "a 1.x application compiles unchanged" stands.
    - B3's `ratatui` and D1's `axum` follow the same rule.
    - **Rejected:** keeping the refusal everywhere and qualifying the promise (mixed workspaces
      would check one package at a time, or split); dropping it (native code could reach a
      browser bundle unnoticed).
25. **B1's and B2's measurement worktrees** (2026-09-29) — **answered: removed.** Their figures
    stay in the records with the scripts and patches that reproduce them; only the raw outputs
    went.
26. **When question 24's change is built** (2026-09-29) — **answered: first in the next session,
    before B3.** The coordinating session closed after recording it.

    *As put* (24–26 together, after B1's review fixes):
    > In 2.0, mf2 refuses to compile if the native-app feature and a browser mode are both on …
    > a workspace holding a browser app and a command-line tool now fails `cargo check
    > --workspace` … What should 2.0 do? — Refuse only for browser; keep the refusal and reword
    > the promise; or drop the refusal.
    >
    > The measurement copies of the tree used by the Leptos merge and the native merge still
    > hold their raw outputs … Remove both copies? — Remove both; or keep them.
    >
    > If the answer needs a code change, when should it be made? — Now, before this session
    > closes; or next session, first.
27. **How the remaining tasks run: the token cost** (2026-09-29) — **answered: lean mode, Opus
    agents.** The seven task agents after question 26 (the browser-only refusal, B3–B5, C1–C3)
    re-read 625M tokens: each ran 118–306 steps in one context that grew to 264–812k, and C1–C3
    about 130–145M each, where the code changes were small. The method cost the tokens, not the
    code: baselines of every figure before and after, byte-level investigations, the full suite run
    step by step in every task, and long records that later agents read. From C4 on:
    - one fresh Opus agent per task reads `CLAUDE.md`, its task's row, its design section and the
      code it changes (no records, no other task's history), implements, runs `cargo xtask ci`,
      commits by path and reports in ten lines; its Done entry is at most five lines;
    - no per-task baselines or byte-level investigations. The full suite (the sizes, the demos,
      the TUI gate, B12, the conformance report, `docs`, `docs-rs`, `codegen-matrix`, `scenarios`,
      `msrv`, `churn`, `leptos-0-8`, `l6-web`, `l7-web`, the browser checks on both lines) runs as
      one quiet script after each task, started by the coordinator, and its table is compared with
      the previous one. Only a regression gets an agent, with a narrow brief;
    - the records already in this file stay; new tasks add none.

    The gates themselves are unchanged (19 §14; the budgets in [06](06-size-and-perf.md)).
    *Rejected:* small agents per phase (an implementer, a Sonnet checker, the records moved out),
    chosen first and replaced the same day; Sonnet agents; stopping to cut the scope first.
28. **Language matching in an application with no server** (found by C3; 2026-09-29) —
    **answered: the full matcher, as built.** A `csr` application matches the reader's languages
    with the same CLDR-based matcher as a server: +2,870 B gz on demo-csr (91,186 → 94,056; about
    1.5 KB of code and 1.4 KB of the application's part of CLDR's matching data). A hydrated page
    never matches, and B1 is unchanged. The data is none of what the no-locale-data rule names
    (text, ids, argument names, plural rules, symbols), and the canaries pass. *Rejected:* no data
    in the browser (other regions of the same language only, so browser and server disagree);
    fetching the data with the catalog index (another task); 1.x's matching in the browser
    (against the one-matcher decision).
29. **`mf2`'s licence** (found by C3; 2026-09-29) — **answered: "MIT AND Unicode-3.0".** `mf2` now
    ships CLDR's matching data, so it declares Unicode-3.0 beside MIT, as `mf2-locale-data`
    already does. *Rejected:* keeping `mf2` plain MIT by moving the data into `mf2-locale-data`.
30. **Backward compatibility** (2026-09-29, the owner, when C4's `setup()` question was put to
    him) — **answered: not a priority.** "2.x has no obligation to stay backwards compatible in
    any way shape or form." The changelog's promise that a 1.x application compiles unchanged is
    withdrawn. No task spends effort on 1.x compatibility from now on, and where the design and
    1.x conflict, the design wins. What is built stays (the shims, C1's step for 1.x's
    `From<T> for ArgValue`, the 1.0.0 baselines in `release.rs`); the shims go with G1 as planned,
    and the examples and the book leave them with C8 and D6. *Rejected:* removing all of it now
    (an extra task, and C8, D6 and G1 reordered).
31. **The generated `setup()`** (found by C4; 2026-09-29) — **answered: generated, as 19 §10
    designs.** It collides with the `setup()` every 1.x translation crate writes by hand. Under
    question 30 that is an upgrade step: a 1.x application deletes its own, and the upgrade guide
    says so. D3 builds on the generated one as planned. *Rejected:* not generating it (`install()`
    alone; D3's design revisited); generating it under a new name; generating it only on request.
32. **The task agents' reasoning level** (2026-09-29, with question 27) — **answered: try `high`
    and measure.** In lean mode, C4's agent re-read 33.4M tokens (141 steps, 377k at the end), a
    quarter of C1–C3's, and about 57 % of its context was its own reasoning, which stays in the
    context for the whole run. Task agents run as the project agent `mf2-task`
    (`.claude/agents/mf2-task.md`, untracked: `model: opus`, `effort: high`, lean mode's standing
    rules). A definition loads only when a session starts, so the first task agent of the next
    session is the first at `high`; until then task agents run as `general-purpose` with those
    rules in their brief. The first figure at `high` is compared with C4's.

**Decided without asking, and the owner may overturn any of them:**
- **`NativeI18n` stays** as the explicit, no-globals `mf2::native::Catalogs`. The ambient store is
  built on it; it keeps servers, tools and several message sets possible later.
- **A native-only build that formats with nothing installed panics**, and the message names
  `install()`. A build with a web mode keeps the web's rule: empty text, never a panic
  ([04](04-leptos-integration.md) §5), plus E4's warnings.
- **Two questions wait for release time (G2):** final 2.0.0 stub releases of `leptos-mf2` and
  `mf2-axum` whose `compile_error!` points to `mf2`; and whether to reserve the never-published
  `mf2-native` / `mf2-ratatui` names.
- **`vendor/` and `comparison.md` stay untracked;** every commit stages files by name.

## What 2.0 looks like

A native Ratatui application, in one crate:

```toml
[dependencies]
mf2 = { version = "2", features = ["ratatui"] }      # ratatui implies native
ratatui = "0.30"

[build-dependencies]
mf2-build = "2"
```

```rust
// build.rs
fn main() { mf2_build::run() }                        // what to emit, from mf2's features

// src/main.rs
mf2::include_generated!();                             // tr!, Locale, markup::*, install(), prelude
mod ui;

fn main() -> std::io::Result<()> {
    let args = Args::parse();                          // lang: Option<Locale>, parsed by FromStr
    install();                                         // embedded catalogs + the system's language
    mf2::ratatui::set_theme(Theme::default().style(markup::KEY, Style::new().bold().yellow()));
    if let Some(l) = args.lang { set_locale(l) }
    println!("{}", tr!("welcome"));
    ratatui::run(ui::run)
}

// src/ui.rs — no handle and no styles passed around
f.render_widget(
    Paragraph::new(tr!("status", host = h, sent = n)).block(Block::bordered().title(tr!("title"))),
    area,
);
let header = Row::new([tr!("col.host"), tr!("col.loss")]);
let hint = tr!("help");                                // `{#key}h{/key}elp`: the theme styles the key
```

*A8 wrote the exact code of every sample: [19](19-native-and-terminal.md) §1. It differs from
this sketch where the design settled a detail:*
- `install()` returns nothing, so `main` needs no `Result` for it;
- `src/ui.rs` imports the crate's prelude (A3);
- `Theme` comes from `mf2::ratatui`.

**The shape** (A8 wrote the exact code of every sample, [19](19-native-and-terminal.md) §1, and the
owner approved it: question 17; where this sketch and 19 differ, 19 is the design):

- **Crates: 18 become 16.**
  - `leptos-mf2`, `mf2-native`, `mf2-ratatui` and `mf2-axum` fold into `mf2`.
  - Two supporting crates carry the Leptos components, one per line (question 13).
  - Applications name `mf2`, and `mf2-build` in their build script; `mf2-cli` is the tool.
- **`mf2`'s features:**
  - `leptos` (the 0.9 line) or `leptos-0-8`;
  - exactly one of `ssr`, `hydrate`, `csr`;
  - `static-locale`, `mark-fallback-lang`;
  - `axum`;
  - `native`, and `ratatui` (which implies `native`);
  - the function features, `intl` and `compile`.
- **The call-site types** are defined once, with only additive impls behind features. That is the
  only arrangement that survives cargo's feature unification (`plans/phase-6-results.md`, "A hazard
  found the hard way"). The impls:
  - the Leptos glue;
  - `From` into Ratatui's `Span` / `Line` / `Text`, `Widget`, and `Styled` (so `Stylize` works);
  - `Display`, through the text the fmt-free inherent `to_string()` builds (question 14), beside
    that method, which the web client keeps;
  - `Debug` on every type, written through `write_str` (question 14; A9's S2);
  - argument conversions with a readable error.
- **The ambient store** (native):
  - an atomic active-locale index plus a thread-local override;
  - `&'static` catalogs (embedded, or files leaked once), so constant text is borrowed, not copied.
- **One ambient lookup,** used by `Display` and every conversion: the request context (ssr) or the
  client's catalog (hydrate/csr), then the native thread override, then the native global.
- **The generated module:**
  - `enum Locale` (`ALL`, `SOURCE`, `tag`, `dir`, `FromStr` through the one matcher, `Display`,
    `format`);
  - `install()` and friends; `markup::*`;
  - the web `setup()`, and a prelude.
- **The build:**
  - `links` metadata carries `mf2`'s features to the build script, so no translation crate declares
    or forwards features;
  - `mf2_build::run()` is the whole build script;
  - native applications default to one crate.
  - All three are gated by probes (A2, A3).

## Method and gates

1. **Write the applications first.** A8 writes the exact code of four
   samples, and the owner reviews it before any API work (Part C):
   - a one-file CLI (`--lang`, a plural, an error message, `println!`);
   - a trippy-shaped TUI: bordered blocks, table headers, a key-hint bar with styled keys, a status
     line with a plural and numbers, a language menu, a live switch;
   - a two-crate workspace (a library and a TUI sharing messages);
   - the Leptos `hello` application.

   They become compiled book pages as the work lands (`cargo xtask docs`).
2. **Count what users pay.** A1 fills the UX table below for 1.x, by
   written rules:
   - **setup lines:** lines that exist only for translation, plus lines naming our crates or
     forwarding features to them;
   - **crates named:** from the family, plus the application's own translation crate;
   - **concepts:** distinct API names used before the first translated output;
   - **commands:** from an empty directory to the first translated output.

   A8 sets 2.0's targets. **Every row must fall** (C8, D6). A8's reading (19 §2): setup lines,
   crates and concepts fall, commands do not rise, and fewer translation files are written by
   hand. A count of one command cannot fall. The targets are ceilings.

   | Sample | 1.x (A1, by the rules; the detail is in A1's record) | 2.0 target (A8) |
   |---|---|---|
   | one-file CLI | **35 setup lines** (a translation crate of 24, a two-member workspace); **3 + 1 crates** (`mf2`, `mf2-build`, `mf2-native`; the translation crate); **18 concepts**; **1 command**, the translation crate written by hand | **13 setup lines** (`Cargo.toml` 3, `build.rs` 3, `main.rs` 7); **2 + 0 crates**; **8 concepts**; **1 command**, `build.rs` the one translation file by hand (19 §1.1, §2) |
   | trippy-shaped TUI | **48 setup lines**: as the CLI, plus `mf2-ratatui` and a `MarkupStyles` map built for each draw; the handle in all 118 calls (48 of them `line(i18n, &tr!(…), styles)`); **4 + 1 crates**; **22 concepts**; **1 command**. The real port added **76** (a 59-line `locale.rs`) | `examples/tui` on 2.0: **24 setup lines**, no handle in any call; **2 + 0 crates**; **14 concepts**; **1 command**, 1 file by hand. The book's TUI (19 §1.2, with a menu and a live switch): 23; 2 + 0; 17; 1 (19 §2) |
   | two-crate workspace | **48 setup lines** (a third, shared translation crate; the handle as a parameter in the library); **4 + 1 crates**; **22 concepts**; **1 command** | **19 setup lines** (the library owns the corpus; no third crate, no handle); **2 + 0 crates**; **16 concepts**; **1 command**, 1 file by hand (19 §1.3, §2) |
   | Leptos `hello` | **96 setup lines**: 53 in the translation crate `mf2 init` writes (a 9-feature `Cargo.toml`, a hand-shaped `setup()`), 43 in the application (20 of them server wiring), and 3 lines changed to `_with_context` forms; **4 + 1 crates** (`mf2`, `mf2-build`, `leptos-mf2`, `mf2-axum`; the translation crate) and the `mf2` tool; **28 concepts**; **5 commands** | **22 setup lines** (one crate: `Cargo.toml` 7, `build.rs` 3, `lib.rs` 8, `main.rs` 4), none changed; **2 + 0 crates**, no tool; **18 concepts**; **3 commands**, none of them this library's (19 §1.4, §2) |

3. **Measure against what exists** (D1's rule: a baseline, a gate, a
   fallback).

   | Change | Baseline (A1) | Gate | Fallback |
   |---|---|---|---|
   | The merge (B1–B4, D1) | B1, B5, whole app, B7, B12 | B1 within ±64 B gz, B5 within ±0.2 B a site, B7 catalogs byte-identical, B12 clean | revert the offending impl (e.g. `Display` only with the std modes) |
   | Helper crates and the function table (B1) | A7 | the above, **and the demos' shipped wasm (demo-ssr, demo-csr, demo-islands) within ±64 B gz** — the size workloads render no component, and A7 measured the table at +459 / +130 / −8 B gz there; e2e green on both Leptos lines | static dispatch in place of the table (A7's record); then back to the owner with the other two options from question 13 |
   | The ambient store (C2) | 1.x `NativeI18n`; the port's `RefCell` | time per frame ≤ 1.x (alternating binaries); stripped size ≤ 1.x | the explicit `Catalogs` path; drop the thread override |
   | Ratatui conversions (C5) | 1.x `mf2-ratatui`; an **in-house re-implementation** of upstream trippy's `t!` (a TOML map, the locale `String` cloned per call, `%{x}` replace; not copied code) | allocations per frame ≤ both; time ≤ 1.x | a reusable-buffer API |
   | The matcher (C3) | both matchers' current tests | every current case still passes, except the changes decided in questions 11 and 15 (each listed); the client table measured against B1 | the client uses the server's choice |
   | `links` (C6) | today's forwarding | every A2 scenario; the wasm byte-identical across a translation edit (P0.9's scenario) | function features stay on the translation crate |
   | In-crate `tr!` (C6) | textual scope | A3 passes under cargo and rust-analyzer | textual scope, documented, with a clear error |
   | Server layer (D3) | the `_with_context` e2e results | e2e green without the context | keep the context wiring |

4. **Cold start** (G3). A fresh agent with only the book and `mf2 init` builds the CLI, the TUI and
   the Leptos application. Every stumble is fixed, and the run repeated until clean.
5. **The trippy port** (C9) is the real-world acceptance test.
6. **Keep later work open.** Each design task states what it does to:
   - hot reload: catalogs replaceable in the ambient store, the old ones leaked in development;
   - editor tooling: the manifest stays discoverable;
   - servers without Leptos: `Locale::format`, the `axum` feature;
   - per-route catalogs: the format is untouched;
   - custom functions;
   - several message sets in one process: the store keyed by corpus later.

## Part A — the plan, baselines and probes (A0 first, then A1; A2–A7 in any order; A9 after A5; A8 after A2–A7 and A9)

Probe crates live under `probes/p10-*`, which the workspace already
excludes. They are deleted at the exit; each probe's result stays in its
task record.

| Task | Deliverable | Done when |
|---|---|---|
| **A0** The plan in the tree | This work order; the master plan (D16–D24, P10, "Later", the doc table and §11); "superseded for 2.0" notes in 04 and 05; `plans/README.md`; `CLAUDE.md` "Start here" | committed (2026-09-28) |
| **A1** Stale release statements; baselines | **First commit:** `docs/versioning.md`, `README.md`, `docs/getting-started.md` and the 1.1.0 changelog intro say what is true: 16 crates at 1.0.0, 1.1.0 not published, 2.0.0 next. **Then:** web `size`, `catalog-size`, `bench/b12/check.sh`, `b12-generated`, B10. A new standalone `examples/tui` (excluded like the demos) **in its 1.x form**: the trippy-shaped frame, about 110 messages, drawn into a ratatui-core `Buffer`, plus `upstream.rs`, the in-house baseline renderer. `cargo xtask tui-gate` (`xtask/src/tui_gate.rs`): allocations per frame (counting allocator), median ns per frame over 31 runs, and stripped release sizes. The 1.x binaries kept under `target/p10-baseline/` for A/B alternation. The native book project's stripped sizes (CLI, and `--features tui`). The UX table at 1.x | two runs agree exactly on allocation counts; every figure recorded with its command |
| **A2** Probe: `links` metadata | A stand-in `mf2` with `links = "mf2-v2"` whose build script prints `cargo::metadata=features=…`. Each row is recorded pass or fail, with the output observed: <br>• a single-crate application, and a two-crate one whose translation crate sees what the *application* turned on; <br>• cargo-leptos: two builds, two values, and a second `cargo leptos build` that does nothing; <br>• rust-analyzer; <br>• a feature toggle reruns the dependent's script; <br>• `cargo metadata` carries the features (for `mf2 check`); <br>• the error a duplicate `links` gives; <br>• `cargo package` / `publish --dry-run`. <br>Also **cfg-forwarding macros** (`#[cfg(feature = "ssr")] #[macro_export] macro_rules! __if_ssr { … }` and an empty twin), so the generated module's compile-time choices live in `mf2` | D19 adopted as "`links` + cfg macros", or the fallback recorded |
| **A3** Probe: `tr!` inside its own crate | Five variants, each recorded: <br>1. today's form (the exact error and lint); <br>2. the future-incompatibility `allow` (recorded, not adopted); <br>3. a generated non-exported macro plus `pub(crate) use … as tr`, used from modules before and after the include and through a crate prelude; <br>4. an env-driven proc macro (`cargo::rustc-env=MF2_MANIFEST=…`): invalidation, a relocated target directory, rust-analyzer; <br>5. a generated `tr` against a glob-imported prelude `tr` | the in-crate mechanism chosen, and the rule for a translation crate and its consumers written for 05 §4 |
| **A4** Probe: the ambient store's cost | Four variants against the path crates: <br>(a) `NativeI18n::format`; <br>(b) the design (`OnceLock` store, an atomic index, a thread-local override, settings behind a lock with a generation counter and a per-thread copy); <br>(c) (b) reading the lock every time; <br>(d) the port's `RefCell`. <br>Time and allocations measured for a simple message, a one-argument message, markup to `Line`, and a 110-message frame, plus the stripped CLI size of (a) against (b). **Zero copy:** a simple message's `&'static str` from a `&'static Catalog`, and pattern text parts recovered as `'static` by a catalog-range check (safe code) through a hidden seam, measured against B1 because the parts sink is on the client's rich path. Parallel `with_locale` tests | D17's figures against the gates; the text-borrowing method chosen |
| **A5** Probe: `Display` / `Debug` against B12 | On a branch: `Display` for the four descriptions; an always-on inherent `to_string` / `to_plain_string`; `Debug` everywhere (hand-written for `Custom` / `Source`). Then `cargo xtask size`, `b5 --view`, `b12-generated`, `bench/b12/check.sh`, and a twiggy look for `Display` / `Debug` symbols. `clippy::inherent_to_string_shadow_display` needs an `allow` with B12 as the reason | adopted if the bytes hold (B1 ±64 B gz, B5 ±0.2 B); otherwise `Display` only with the std modes (an added impl, which survives unification) |
| **A6** Probe: a single-crate web application | Getting started's `hello` with `build.rs` and `locales/` in the application crate, checked for: <br>• both builds, and `watch` (is `watch-additional-files` still needed?); <br>• a translation-only edit leaves the wasm byte-identical (as `cargo xtask scenarios` does); <br>• `--split`; <br>• rust-analyzer; <br>• A3's in-crate `tr!` | one crate for the web too (then the web starter offers it), or why not recorded |
| **A7** Probe: names and coherence | 1. `mf2::leptos` with no root rename. The components come from per-line helper crates through a function table: on both lines, under SSR, hydrate and islands; B1 measured for the table's indirection. <br>2. No Leptos procedural macro outside the six components. <br>3. `pub mod axum` against `use axum::…` inside `mf2`. <br>4. With `leptos` and `ratatui` both on, every intended impl compiles, and the rules are recorded: <br>• never `From<Tr> for Cow<str>` (it collides through `Span`'s blanket impl); <br>• no `FromIterator<Tr> for Line`; <br>• `Cell` and `ListItem` come through their blankets over `Into<Text>` | the module and crate names confirmed; the coherence rules for 19 |
| **A8** The design, for the owner's review | A new companion, `plans/19-native-and-terminal.md`: the store and the lookup order, the Ratatui conversions and theme, the generated module, the build, the in-crate `tr!`, the argument conversion, and the per-mode API. **The exact target code of the four samples**, the UX targets and the gate table. The web side's design goes to 04 and 05 | the owner has reviewed it. This gates Part C's API work; Part B can start before |
| **A9** Probe: what `Display` / `Debug` cost the browser's wasm (added by the owner, 2026-09-28) | **What A5 left open.** In a client that formats no description, A5 found none of our `Display` / `Debug` code and no new `core::fmt`. Its positive control, one `format!("{save} {items:?}")` in the fixture client (a `Tr` through `{}`, a `TrArgs` through `{:?}`), cost **+16,604 B gz** after `wasm-opt`: both at once, and in the fixture only. The `tr` workload already holds 72 items matching B12's fmt pattern (10,354 B with names kept, from `core`, `alloc`, `std` and Leptos's stack), so what an application pays is not known. On A5's variant (the branch `p10-a5-display`, or `probes/p10-display/display.patch` at `2fb7f54`), in one tree and one lock, with A5's scripts: <br>• **the split:** nothing formatted (the base), `{}` alone, `{:?}` alone, both, and the inherent `to_string()` (the fmt-free control), over `Tr`, `TrArgs`, `TrRich` and `TrDyn`; <br>• **in applications:** the fixture client, `tr` and `tr-view` at 1,860 sites, and the three demos (`probes/p10-names/measure-demo.mjs`); <br>• **what the bytes are:** twiggy's dominators, `core::fmt`'s own code apart from our impls; <br>• **the ways to shrink it,** each measured: `Display` through `write_str` instead of `Formatter::pad` (then `{:<12}` no longer pads), and whatever the dominators point to (e.g. `Debug` written through `write_str`, without `core::fmt`'s number and string-escape code); <br>• **the ways to remove it:** A5's fallback (`Display` only with the std modes) and a target `cfg` (no `Display` on `wasm32-unknown-unknown`, whatever the features). Each brings back 1.x's compile error for `{}` in a browser build, whose text is recorded. The same choice for `Debug`, which 1.x had on `Tr` only; <br>• **the silent paths:** where a description reaches `Display` with no `format!` in the application. Generic code bounded on `ToString` or `Display` takes the blanket `ToString`, not the inherent `to_string()`. Searched in Leptos 0.8 and 0.9 and in our own crates; each path found is shown compiling, or ruled out; <br>• **a check:** whether CI can catch our types' `Display` / `Debug` in the demos' client builds (the shipped wasm has no names; e.g. A5's `twiggy.sh` over a build that keeps them) | every figure recorded here with its command; the silent paths listed; a recommendation for A8: A5's design with a rule in the book (and the check), a way to shrink it, or a way to remove it. If it changes what A5 adopted, A8 puts it to the owner |

## A1 — stale statements and 1.x baselines: what was built

* **The release statements** (commit `8f6569e`). `docs/versioning.md`
  ("Where the releases stand", the intro's example versions, the Leptos
  table), `README.md` ("Status", the install note),
  `docs/getting-started.md` (the install note, the version callout) and the
  1.1.0 changelog intro now say what is true: all 16 original crates are on
  crates.io at 1.0.0 (2026-09-26); 1.1.0 was not published and will not be,
  and its items ship in 2.0.0, the next release; `mf2-native` and
  `mf2-ratatui` were never published and are named by path until then. The
  `## 1.1.0` heading stays: the workspace is versioned 1.1.0 until G1, and
  `xtask`'s changelog test holds the tree to that entry. Full
  `cargo xtask docs` and `cargo xtask ci` green.
* **`examples/tui`** (commit `3a296a9`), excluded from the root workspace
  like the demos: a workspace of its own with its translation crate. One
  trippy-shaped frame — a header with a key-hint bar, a table of 12 hops
  under 14 column headers, the selected hop's details, chart titles,
  settings (7 tabs, 7 values), help, a language menu, flows, an event log
  and a status line — drawn into a Ratatui `Buffer` at 160 × 47 cells,
  with no terminal. **118 messages**
  in `en`, `de`, `es` and `fr`, **126 formatted per frame**. They use
  plurals (two of them on two selectors, with `many` variants where Spanish
  and French have the category), `:number`, `:integer` and `:percent` with
  `fn-number`, and markup (`key`, `host`, `ok`, `warn`, `alert`). The French
  failures message selects on another value than the English (the verb
  agrees with the failed probes); the German frozen status moves the styled
  word to the end; the language names are `@do-not-translate`.
  `mf2 check --features fn-number`: nothing to report (without
  `--features`, see the second finding below); the files are in `mf2 fmt`'s
  form. The same frame is drawn twice:
  * `src/ui.rs` — MF2 on **the 1.x API, as the user guide's native page
    has an application written**: the translation crate beside the
    application, a `NativeI18n` passed to all 10 draw functions,
    `MarkupStyles` built for each draw and passed to the 6 that draw
    markup; 48 calls `mf2_ratatui::line(i18n, &tr!(…), styles)` and 70
    `i18n.format(&tr!(…))`.
  * `src/upstream.rs` — **the baseline**: upstream trippy's approach,
    re-implemented from this work order's description, not from its code.
    A TOML table per message (122 keys) parsed once into a `HashMap`; the
    locale a thread-local `String`, cloned by every lookup; `%{name}`
    replaced one `str::replace` at a time; English plurals (`n > 1`); word
    order assembled with `format!` and spans; key hints bolded by slicing
    the translated word. Its bugs show in the frame, e.g. in French
    `[h]aide`, `Cible:` and "2411 sur 14448 (16.7%) sondes ont échoué".
  * `tests/frame.rs`: both renderers in every language; MF2's grouping,
    percent signs, plurals and the French agreement checked; no MF2
    fallback (`{…}`) in any frame. `cargo run -- --lang fr [--upstream]`
    prints a frame.
  * **Found while building it:** with Ratatui's default features off (no
    terminal backend), its layout cache is off too. Every frame then
    re-solved every layout: about 6,700 allocations and 1.2 MB a frame for
    either renderer, with counts that moved between runs (the solver's hash
    maps). The example turns on `std`, `layout-cache` and
    `underline-color` — Ratatui's defaults without the backend — and the
    counts became exact.
  * **Found while checking it:** `mf2 -C examples/tui/i18n check` reads the
    crate's features with `cargo metadata --offline`, which fails here —
    the lock holds Ratatui's optional backend, and its Windows-only
    `crossterm_winapi` has never been downloaded on this machine. The
    command then says it checks "with no function features" and reports 12
    `gated-function` errors that a build does not have (every `:percent`).
    With `--features fn-number` it passes. C6 rewrites how `check` reads
    the features; it should not turn a metadata failure into errors.
* **`cargo xtask tui-gate`** (`xtask/src/tui_gate.rs`). It builds the
  example's `tui-mf2` and `tui-upstream` in release, stripped
  (`CARGO_PROFILE_RELEASE_STRIP=symbols`), and runs them alternately,
  31 runs each by default. Each run draws two warm-up frames per language,
  counts one frame's allocations and bytes per language (a counting global
  allocator in each binary), and times 50 frames per language, switching
  language between them. It reports allocations and bytes per frame per
  language, the median of the runs' mean frame time with its range, and the
  stripped sizes, to standard output and to `target/tui-gate/report.{md,json}`.
  **A run whose counts differ from an earlier run's fails the command**
  (unit-tested, with the negative control). `--save-baseline DIR` keeps the
  binaries, with the commit in `BUILT-AT`; `--baseline DIR` puts kept
  binaries back into the rotation, so that C2 and C8 compare 2.0 with 1.x by
  alternating them; `--book` adds the user guide's native project's
  stripped sizes. No gate yet: C8 adds it.
* **The 1.x binaries are kept** in `target/p10-baseline/` (`tui-mf2`,
  `tui-upstream`, and `BUILT-AT`: built at `3a296a9`, a clean tree).
* **Not in CI yet:** nothing in `cargo xtask ci` builds `examples/tui`;
  `tui-gate` does. It stays on the `mf2-native` / `mf2-ratatui` shims until
  C8 rewrites it.

### Figures at 1.x (the crates as at `2fb7f54`; measured 2026-09-28)

| What | Figure | Command |
|---|---|---|
| B1, fixed | **26,676 B gz** (limit 30,720); the `dummy` bound 27,659 | `cargo xtask size` |
| B5, per call site | **8.2 B gz** (limit 40); the `dummy` bound 25.1 | same |
| the whole app at 1,860 sites | **41,889 B gz** (the ambition 105,120) | same |
| B7, `en` | **18,072 B br** (limit 23,296); `pl` 24,137, `en-XA` 21,537, `ar-XB` 18,423 — every locale passes | `cargo xtask catalog-size` |
| B12 | clean: no panic path, no `core::fmt` in the reader, the runtime, the numeric and the date functions; B13 shown | `bash bench/b12/check.sh` |
| B1's runtime part | 18,888 B gz (the reader 6,781; the core numbers 5,407; B2 2,022; B3 5,486) | same |
| B1′ and B13 on the generated module | +0 B; 13,573 B avoided | `cargo xtask b12-generated` |
| B10, `en` allocations | 0 (simple), 0 (1-argument, reused `String`), 1.018 (new `String`), 4.000 and 1,024 B (select) | `cargo run --release -p runtime-bench -- b10 --gate --md target/p10-a1/B10-P10-1x.md --json target/p10-a1/b10-p10-1x.json` |
| B10, `en` time, **under load** (12.0–12.3, CPU at 1,500 MHz) | 142.7 ns simple, 802.5 ns 1-argument, 2,587.8 ns select: the gate **fails** | same |
| `tui-mf2`, allocations per frame (`en` / `de` / `es` / `fr`) | **1,816 / 1,815 / 1,816 / 1,817**; bytes 171,425 / 176,838 / 175,402 / 178,333 | `cargo xtask tui-gate --save-baseline target/p10-baseline --book` |
| `tui-upstream`, allocations per frame | **1,517 / 1,519 / 1,518 / 1,526**; bytes 135,767 / 143,799 / 139,764 / 141,463 | same |
| `tui-mf2` / `tui-upstream`, median time per frame, **under load** (14.4–16.9) | 2,484.7 / 2,136.1 µs; the second run 1,908.1 / 1,721.5; the third 1,864.5 / 1,803.9 | same, then `cargo xtask tui-gate`, then `cargo xtask tui-gate --save-baseline target/p10-baseline` at `3a296a9` |
| `tui-mf2` / `tui-upstream`, stripped | **1,965,320 B** / **1,390,784 B** | same |
| the user guide's native project, stripped | `native-demo` 1,620,208 B; with `--features tui` 1,620,160 B | same (`--book`, after `cargo xtask docs`) |

* **Three runs agree exactly** on every allocation and byte count (31 runs
  of each binary in each), and a fourth, alternating with the kept
  binaries, agrees with them (`cargo xtask tui-gate --baseline
  target/p10-baseline --runs 5`).
* **B1 against Phase 9's record.** Phase 9 recorded 25,875 / 8.4 / 41,466
  (A6, 2026-09-25, before its Part B); today's tree gives 26,676 / 8.2 /
  41,889. Phase 10's gates (B1 within ±64 B gz, B5 within ±0.2 B) are
  against today's figures.
* **B10's time is not a baseline.** Every `en` time in the report is
  3.9–5.9 × Phase 3's committed `bench/runtime-bench/B10-P3.md` (load 0.84,
  1,823 MHz), including the load-time function table lookup, whose code
  has not changed (3.2 → 15.9 ns). The allocation counts equal Phase 3's;
  select's bytes per call are 1,024 against Phase 3's 928. C2's B10
  criterion needs the run repeated on a quiet machine; the reports stay
  under `target/p10-a1/`.
* **The user guide's native project** is the same size with and without
  its `tui` feature: its `main` never calls the TUI, so the linker drops it.

### The UX table's 1.x column, by the rules

Counted by method §2's rules, read this way so that C8 and D6 count 2.0
alike:

* **setup lines** — non-blank lines that are not comments, and that exist
  only for translation, or name one of our crates, or forward a feature to
  one; a line the application has anyway, in another form (Leptos's
  `_with_context` calls), is listed as *changed*, not counted;
* **crates named** — the family's crates in the manifests, plus the
  application's own translation crate; the `mf2` tool apart;
* **concepts** — distinct names of our API, generated items, features and
  `mf2.toml` keys the application's author writes before the first
  translated output; generated code the author does not write is counted
  apart;
* **commands** — what the book's page has the reader run, from an empty
  directory to the first translated output, installs included; files
  written by hand are listed beside them.

The native samples are written the way `docs/native-apps.md` has an
application written (the translation crate by hand: that page has no
`mf2 init`); the TUI is `examples/tui`'s MF2 side; the Leptos sample is
`docs/getting-started.md`'s `hello`.

| Sample | Setup lines | Crates named | Concepts | Commands |
|---|---|---|---|---|
| one-file CLI | **35**: the translation crate 24 (`Cargo.toml` 12, `build.rs` 8, `mf2.toml` 3, `lib.rs` 1); the application's `Cargo.toml` 5 (a `[workspace]` of 3 lines, `mf2-native`, the translation crate); `main.rs` 6 (the `NativeI18n` and its `set_locale`, 4; `--lang`, 2) | **3 + 1**: `mf2`, `mf2-build`, `mf2-native`; the translation crate | **18**: `Build`, `Build::new`, `emit`, `Emit::Native`, `emit_cargo`, `run`, `into_result`; features `host-std`, `fn-number`; `source_locale`, `missing`; `include_generated!`; `NativeI18n`, `embedded`, `CORPUS`, `set_locale`, `format`, `tr!` | **1** (`cargo run`); the translation crate's 4 files by hand |
| trippy-shaped TUI | **48**: as the CLI, plus `mf2-ratatui` in `Cargo.toml` (1), three `use` lines, and an 8-line `MarkupStyles` map built for each draw (+1). Every text carries the handle: 118 calls, 48 `line(i18n, &tr!(…), styles)` and 70 `i18n.format(&tr!(…))`; the 10 draw functions take `i18n`, and 6 of them `styles` too | **4 + 1**: the CLI's, and `mf2-ratatui` | **22**: the CLI's, and `MarkupStyles`, `MarkupStyles::new`, `with`, `mf2_ratatui::line` | **1**; as the CLI |
| two-crate workspace (a library and a TUI sharing messages) | **48**: the shared translation crate 24 (a third crate, as the book has it); the library 3 (`mf2-native` and the translation crate in its `Cargo.toml`, a `use`), with an `i18n: &NativeI18n` parameter on each function that makes text; the TUI 21 (`Cargo.toml` 3, the handle and `--lang` 6, `use` 3, the styles 9) | **4 + 1** | **22** | **1**; as the CLI |
| Leptos `hello` | **96**: the translation crate `mf2 init` writes, 53 (`Cargo.toml` 18 with a 9-feature block, `build.rs` 13, `mf2.toml` 10, `lib.rs` 12 with the hand-shaped `setup()`); the application 43 — `Cargo.toml` 12 (a `[workspace]` of 3, 3 dependencies, 5 forwarded features, `watch-additional-files`), `lib.rs` 11 (2 `use`, `html_lang`, the two head components, a 4-line switcher, `install`, `hydrate_body`), the server 20 (2 `use`, `install`, a 10-line negotiator, a 6-line context closure, `catalog_routes`); and 3 lines changed to their `_with_context` forms | **4 + 1**: `mf2`, `mf2-build`, `leptos-mf2`, `mf2-axum`; the translation crate; and the `mf2` tool | **28**: `tr!`, `html_lang`, `CatalogPreload`, `CatalogLinks`, `LocaleSwitcher`, `LocaleOption`, `leptos_mf2::install`, `setup`, `hydrate_body`, `mf2_axum::install`, `CATALOGS`, `Negotiator`, `empty`, `source`, `sink`, `QueryParam`, `CookieLocale`, `secure`, `AcceptLanguage`, `provide_locale`, `catalog_routes`, `leptos_routes_with_context`, `file_and_error_handler_with_context`; features `fn-number`, `fn-datetime`, `datetime-icu`; the mode forwarded to two crates; `watch-additional-files` (and 7 more in the generated `lib.rs`) | **5**: `rustup target add`, `cargo install cargo-leptos`, `cargo install mf2-cli`, `mf2 init`, `cargo leptos watch` — 2 of them this library's |

**What the owner's trippy port added** (`vendor/trippy`, counted only,
nothing copied): **76 setup lines** against the 1.0.1 API it pins — a
59-line `locale.rs` (71 with comments and blank lines) holding its own
`thread_local! RefCell<NativeI18n>`, the system-locale choice and a `t!`
wrapper; `build.rs` +9; `Cargo.toml` +4; `mf2.toml` 3; `lib.rs` +1.

**Verdict against "Done when":** met — the runs agree exactly on every
allocation count, and every figure above is recorded with its command.
B10's time, taken under load, is recorded but is not a baseline (above).

**For A8 and later** (interpretation, kept brief):
* The 1.x TUI costs +291 to +299 allocations (+19–20 %) and +33 to +37 KB
  allocated per frame, and +574,536 B of stripped executable, against the
  trippy-style baseline. C5's gate ("allocations per frame ≤ both") has
  that gap to close: 1.x allocates a `String` for each `format`, and a
  builder with owned spans for each `line`. The size gate is against 1.x,
  not against the baseline.
* A Ratatui application built without the default features needs
  `layout-cache` (and `std`) on; the 2.0 starter and book should keep
  Ratatui's defaults or say so.
* The book's native project measures nothing of the TUI until its `main`
  reaches it (C8).

## A2 — `links` metadata: what was built

Probe: `probes/p10-links/` (its README lists how to re-run each row), run on
2026-09-28 with cargo 1.98.1, cargo-leptos 0.3.9 and rust-analyzer
1.98.0-nightly (b30f3df 2026-06-11).

* **The stand-in `mf2`** (`p10-links-mf2`, named `mf2` by its dependents)
  has `links = "mf2-v2"` and the real crate's features (`ssr`, `hydrate`,
  `csr`, `leptos`, `axum`, `native`, `ratatui`, `fn-number`, `fn-datetime`,
  `datetime-icu`, `datetime-intl`, `intl`, `static-locale`,
  `mark-fallback-lang`, `compile`). Its `build.rs` prints
  `cargo::metadata=features=<sorted, comma-separated, without default>` and
  `cargo::metadata=target=$TARGET`.
* **The cfg-forwarding macros** in it: `__if_ssr!`, `__if_not_ssr!`,
  `__if_hydrate!` (each defined twice, under a `cfg` and its negation: the
  tokens pass through or vanish), and `__use_host!(HOST)` (four
  definitions: the three-way browser-host choice `codegen.rs` makes today
  with `#[cfg(all(not(feature = "ssr"), …))]` on the *including* crate's
  features).
* **The stand-in `mf2-build`**: `run()` is the whole build script. It
  reads `DEP_MF2_V2_FEATURES`, else the crate's own `CARGO_FEATURE_*`, and
  writes a module whose text is the same in every build except for what
  the script decides (`BUILD_DECIDED_EMIT`; `EMITTED_FOR_SSR` /
  `EMITTED_FOR_NATIVE` only when it saw them). Compile-time choices go
  through the macros (`CATALOGS` only under `ssr`; `host::HOST`).
  `report()` prints the build script's view next to `mf2`'s `cfg!` truth.
  With `P10_RUN_LOG` set, each script run appends a line.
* **Scenarios**, each its own workspace: a one-crate application
  (`single/`); an application with a translation crate that has no
  features and a second dependent using the macros (`two-crate/`); `mf2`
  as normal plus build dependency, and as build dependency only
  (`host-target/`); cargo-leptos's two builds (`leptos-app/`, no Leptos
  crate — see "Scope"); rust-analyzer (`ra/`, `ra-control/`); duplicate
  `links` (`dup/`); `mf2`'s own test through a translation-crate
  dev-dependency (`fixture/`).

### Rows

| # | Row | Result | Observed | Command (in `probes/p10-links/`) |
|---|---|---|---|---|
| 1 | The variable's name | **confirmed** | `DEP_MF2_V2_FEATURES` and `DEP_MF2_V2_TARGET`: the `links` value upper-cased with `-` → `_`, then the key. The dependent's warning: `p10-links-single [x86_64-unknown-linux-gnu] via links: features=[fn-number,native] … vars=DEP_MF2_V2_FEATURES=fn-number,native DEP_MF2_V2_TARGET=x86_64-unknown-linux-gnu`. A present but empty value means `mf2` with no features (`DEP_MF2_V2_FEATURES=`); absent means no `links` at all | `cd single && cargo build -v --features native` |
| 2 | One-crate application | **PASS** | `build saw [fn-number,native] via links …; mf2 compiled with [native,fn-number]; build decided … emit=native`; with `--features ssr`: `emit=web server; macros chose ssr=true … catalogs=1 host=host_std::HOST`; with `hydrate,datetime-intl`: `emit=web client; macros chose … host=host_web::INTL_HOST` | `cd single && cargo build -v --features <f> && ./target/debug/p10-links-single` |
| 3 | Two crates: the translation crate sees what the application turned on | **PASS** | The application names `mf2` with `fn-number` and turns on `mf2/ssr`; the translation crate names `mf2` with no features and has no `[features]`: `i18n: build saw [fn-number,ssr] via links …; macros chose ssr=true … catalogs=1 host=host_std::HOST`; the second dependent (no features, no build script): `widgets: server widgets; mf2 compiled with [ssr,fn-number]`. `--workspace --features p10-links-app/ssr`: the same. **But** `cargo build -p p10-links-i18n` alone: `features=[]` — that build compiles `mf2` with no features, so the script's view still equals the compiled one | `cd two-crate && cargo build -v -p p10-links-app --features ssr`; `… -p p10-links-i18n` |
| 4 | Host vs target: `mf2` as a normal dependency (`hydrate`) and a build-dependency (`native`, `compile`) | **PASS** — the target instance's | Two instances built (`--cfg feature="compile" … "native"` and `--cfg feature="hydrate"`); the script got `DEP_MF2_V2_FEATURES=hydrate DEP_MF2_V2_TARGET=x86_64-unknown-linux-gnu`; with `--target wasm32-unknown-unknown`: `hydrate` and `wasm32-unknown-unknown`. **`mf2` as a build-dependency only: no `DEP_MF2_V2_*` at all** (`via own CARGO_FEATURE_*: features=[] mf2-target=(unset) vars=`) | `cd host-target && cargo build -v -p p10-links-both [--target wasm32-unknown-unknown]`; `… -p p10-links-build-only` |
| 5 | cargo-leptos: two builds, two values | **PASS** | `cargo build --package=p10-links-leptos-app --bin=p10-links-leptos-app --no-default-features --features=ssr` → the translation crate's script saw `[fn-number,ssr]` on `x86_64-unknown-linux-gnu`; `cargo build … --lib --target-dir=…/target/front --target=wasm32-unknown-unknown --no-default-features --features=hydrate` → `[fn-number,hydrate]` on `wasm32-unknown-unknown` (cargo-leptos 0.3.9 gives the client build a target directory of its own) | `cd leptos-app && cargo leptos build` |
| 6 | A second `cargo leptos build` does nothing | **PASS** | Builds 2 and 3: no `Compiling` line, no script run (the run log stays at the first build's two lines), and `target/site/pkg/p10_links.wasm` and the server binary byte-identical (sha256 `931f868bc3fdfd25…`, `f06558561d23336c…` before and after). cargo-leptos re-runs its own wasm-bindgen step each time (≈ 160 ms), as for any application | `cd leptos-app && cargo leptos build` × 3; `sha256sum` |
| 7 | The macros' choices in cargo-leptos's builds | **PASS** | The wasm holds no `catalog bytes` (the `ssr`-only `CATALOGS`) and names only `host_web::HOST`; the server binary holds `catalog bytes` and `host_std::HOST`. The generated files differ only in the script's decisions (`BUILD_DECIDED_EMIT: "web server"` / `"web client"`; `EMITTED_FOR_SSR` in the server's) | `grep -a -c 'catalog bytes'` on both artefacts |
| 8 | rust-analyzer | **PASS** | From a deleted `target/`, `rust-analyzer analysis-stats .` ran the scripts through cargo (the file it left says `BUILD_SAW: "fn-number,ssr"` and has `EMITTED_FOR_SSR`), collected the generated items (`EMITTED_FOR_SSR`, `CATALOGS`, `MACRO_SAW_SSR`, `report`) and inferred everything: `exprs: 656, ??ty: 0`, `pats: 89, ??ty: 0`. **Negative control** — two references to items this build does not have (`MACRO_SAW_HYDRATE`, `EMITTED_FOR_NATIVE`): `exprs: 662, ??ty: 4`, `pats: 92, ??ty: 2`. `rust-analyzer diagnostics .`: only `inactive-code` hints in `mf2`, e.g. "code is inactive due to #[cfg] directives: feature = "ssr" is enabled" and "… feature = "hydrate" is disabled" — its `cfg`s for `mf2` match the build. **Two limits of the RA command line, not of `links`:** `rust-analyzer unresolved-references` panics on any crate, the three-line `ra-control/` included ("Try to use attached db, but not db is attached"); `diagnostics` does not report an unresolved value path (the negative control printed nothing where cargo gives E0425) — hence `analysis-stats` | `cd ra && rm -rf target && rust-analyzer analysis-stats .`; `rust-analyzer diagnostics .` |
| 9 | A feature toggle reruns the dependent's script, and only what it must | **PASS** | `single/`, `native` → `ssr`: `mf2` recompiled as a new unit, its script and the application's run, the application compiles; back to `native`: every unit `Fresh`, no script runs; the same for `hydrate,datetime-intl` → `ssr`. `two-crate/`, `ssr` → `hydrate`: `mf2` recompiled, `mf2`'s and the translation crate's scripts run (the latter is **not recompiled**: the same `build/p10-links-i18n-6a6f157356afc973/build-script-build` every time), the translation crate, the second dependent and the application recompile, `mf2-build` `Fresh`; back: all `Fresh`. No `rerun-if-env-changed` is printed or needed: a new feature set is a new unit of `mf2`, and the dependent's script-run unit (and its `OUT_DIR`) is new with it | `cargo build -v --features …`, in turn |
| 10 | Also: a translation-only edit; `check` and `build` alternating | **PASS** | Editing `i18n/locales/en.mf2`: `Dirty p10-links-i18n …: the file i18n/locales has changed`, only that script runs, with the same metadata (`[fn-number,ssr]`); `mf2` `Fresh`. `cargo check`, `build`, `check`: no script runs (0 in the log), only the edited crate rebuilds in each mode | `cd two-crate && cargo build -v …`; `cargo check -v …` |
| 11 | `cargo metadata` carries the features (for `mf2 check`) | **PASS, with two caveats** | `resolve.nodes[]`, `mf2`'s node: `["default","fn-number"]`; with `--features p10-links-app/ssr`: `["default","fn-number","ssr"]`. **(a)** the set is the union over every workspace member plus the given `--features` (with both `ssr` and `hydrate` passed, both listed) — it is not per build side. **(b)** host and target instances are merged: `host-target/` shows `["compile","default","hydrate","native"]` where the builds used `[hydrate]` and `[compile,native]`, also with `--filter-platform wasm32-unknown-unknown` | `cargo metadata --format-version 1 [--features …] \| jq '.resolve.nodes[] \| select(.id \| contains("p10-links-mf2@")) \| .features'` |
| 12 | A duplicate `links` | **the error, recorded** | Two packages with `links = "mf2-v2"`: the error in block (a) below. The same for two semver-incompatible versions of one package (0.1.0 and 0.2.0). The next major with `links = "mf2-v3"` resolves beside it, and a script depending on both gets `DEP_MF2_V2_*` and `DEP_MF2_V3_*` | `cd dup/app-copy && cargo build`; `dup/app-next`, `dup/app-v3` |
| 13 | `cargo package` / `cargo publish --dry-run` | **PASS** | `Packaged 6 files, 8.0KiB (3.0KiB compressed)`, verified by building; `publish --dry-run`: `Uploading p10-links-mf2 v0.1.0 … warning: aborting upload due to dry run`, exit 0; the normalized manifest keeps `build = "build.rs"` and `links = "mf2-v2"`. Still passes with the path-only dev-dependency of row 14. **`links` without a build script is a manifest error** (block (b) below) | `cargo package -p p10-links-mf2 --allow-dirty`; `cargo publish --dry-run -p p10-links-mf2 --allow-dirty`; `cd dup/mf2-nobuild && cargo build` |
| 14 | Also: `mf2`'s own test through a translation crate that depends on `mf2` (the dev-dependency cycle) | **PASS** | `cargo test -p p10-links-mf2 --test generated`: the fixture's script saw `[]`; with `--features ssr`: `[ssr]`, and `MACRO_SAW_SSR == cfg!(feature = "ssr")` holds | `cargo test -p p10-links-mf2 --test generated [--features ssr]` |
| 15 | cfg-forwarding macros under unification | **PASS** | Rows 3 and 7: the application turns `ssr` on; the translation crate's generated module (same text in every build) and the second dependent's own code follow `mf2`'s compiled `cfg`, with no `cfg(feature)` of their own. In every build observed, the script's decisions and the macros' agreed. **One finding:** a choice made *inside a function* through a macro lints differently per mode (`unused_mut` in one build, `unused_assignments` in the other); choices made at item level (a whole `static`, `fn` or `use`) do not | rows 2, 3, 5, 7 |

(a) Row 12, `dup/app-copy` (paths shortened to `<p10-links>`):

```text
error: failed to select a version for `p10-links-mf2-copy`.
    ... required by package `p10-links-dup-copy v0.0.0 (<p10-links>/dup/app-copy)`
versions that meet the requirements `*` are: 0.1.0
package `p10-links-mf2-copy` links to the native library `mf2-v2`, but it conflicts with a previous package which links to `mf2-v2` as well:
package `p10-links-mf2 v0.1.0 (<p10-links>/mf2)`
    ... which satisfies path dependency `mf2` of package `p10-links-dup-copy v0.0.0 (<p10-links>/dup/app-copy)`
note: only one package in the dependency graph may specify the same links value to ensure that only one copy of a native library is linked in the final binary
for more information, see https://doc.rust-lang.org/cargo/reference/resolver.html#links
help: try to adjust your dependencies so that only one package uses the `links = "mf2-v2"` value
failed to select a version for `p10-links-mf2-copy` which could resolve this conflict
```

(b) Row 13, `dup/mf2-nobuild`:

```text
error: failed to parse manifest at `<p10-links>/dup/mf2-nobuild/Cargo.toml`

Caused by:
  package specifies that it links to `mf2-v2` but does not have a custom build script
```

**Scope.** The cargo-leptos application has no Leptos crate in it: the
question is how the features reach the build script, and cargo-leptos runs
the same two cargo builds (features, targets, its own target directory for
the client) with or without it. Leptos's own no-op rebuild under
cargo-leptos is P0.9's and the demos'. C6 re-runs these scenarios on the
real crates, with Leptos, per its "Done when".

### Verdict

**D19's first half is adopted as "`links` + cfg macros".** Every row passes.
The fallback (the translation crate keeps its function features) is not
needed. Nothing needs the owner.

### For C6, C4 and D3 (interpretation)

- `mf2`'s `build.rs` prints `cargo::metadata=features=…`. The `cargo::` form
  needs Rust 1.77; the MSRV is 1.88. `mf2_build::run()` reads
  `DEP_MF2_V2_FEATURES`: an empty value means no features, an absent one a
  crate that does not name `mf2` directly. It then errors, or falls back to
  `CARGO_FEATURE_*` for 1.x-shaped crates.
- The crate that includes the module must name `mf2` as a **normal**
  dependency (row 4). `mf2-build` must never depend on `mf2`: `cargo
  metadata`, and so an editor's `cfg`s, would see the union of both
  instances (row 11b).
- Split the choices this way. The build script decides what to *emit*
  (native or web, server catalogs, compression) from the metadata. The
  generated text makes its *compile-time* choices at item level through
  `mf2`'s cfg macros (rows 7, 15). That puts the choices in `mf2` and keeps
  the generated file lint-clean in every mode.
- `mf2 check` reads `mf2`'s node from `cargo metadata`, with the
  application's `--features`. For a cargo-leptos application that means
  the function features, which are the same on both sides; the mode
  features differ per side, and `metadata` gives their union (row 11a).
- A build-dependency's features cannot travel this way. If `mf2`'s
  `datetime-icu` is on while `mf2-build` lacks `icu-blob`, `run()` must say
  so; it can test `cfg!(feature = "icu-blob")` in itself. This is C6's
  "clear error".
- More metadata keys cost nothing (row 1's `target`). A `version` key would
  let `run()` refuse a mismatch between `mf2` and `mf2-build`.
- A translation crate built on its own (`-p i18n`) sees only what that
  build turns on. That is correct for that build, but it is not the
  application's set (row 3).
- Cargo replays a fresh script's `cargo::warning` lines on every build (rows
  6, 9). The real build's lint warnings already work this way.

## A3 — `tr!` inside its own crate: what was built

**The probe.** `probes/p10-tr-in-crate/`, a standalone workspace with path
dependencies on the real `mf2` and `mf2-build` (the tree at `2fb7f54`), a
two-message corpus (`hello`, `greet = Hello, {$name}!`, in `en` and `fr`),
and `Emit::Native` so that no Leptos is compiled. Every variant uses the real
`mf2-build` module and the real `__tr_impl!` proc macro; only the *wrapper*
that reaches it is varied — for variant 3 the build script cuts
`mf2-build`'s generated `tr!` wrapper off the module and appends the
variant's shape, for variant 4 two stand-in proc macros forward to
`__tr_impl!`. `./run.sh` compiles every case (102, one feature set each) and
writes each compiler output to `results/<case>.txt`; the scenarios run by
hand (invalidation, relocation, rust-analyzer, a git dependency's
future-compatibility report: `gitdep.sh`) are in `results/` with their
commands. rustc 1.98.1, rust-analyzer 1.98.0-nightly (2026-06-11).

### The five variants

| # | Shape | Result | Observed |
|---|---|---|---|
| 1 | today's: `#[macro_export] macro_rules! tr` inside `include_generated!` | **unqualified only, and only after the include** | unqualified after the include (a module declared after it, or the root): PASS. Unqualified in a module declared *before* it: `error: cannot find macro `tr` in this scope` — and rustc's own help is `consider importing this macro through its public re-export: use crate::tr;`, which is the next error. `crate::tr!`, `use crate::tr;`, `super::tr!`, `self::tr!` and `$crate::tr!` in a macro of the crate, before or after the include: `error: macro-expanded `macro_export` macros from the current crate cannot be referred to by absolute paths` … `= warning: this was previously accepted by the compiler but is being phased out; it will become a hard error in a future release!` … `= note: #[deny(macro_expanded_macro_exports_accessed_by_absolute_paths)] (part of #[deny(future_incompatible)]) on by default` (rustc #52234). Another crate: `v1_today::tr!` and `use v1_today::tr;` PASS. Also tried: `#[path = concat!(env!("OUT_DIR"), "/mf2_generated.rs")] mod generated;` → `error: malformed `path` attribute input … must be of the form #[path = "file"]` |
| 2 | 1 + `#![allow(macro_expanded_macro_exports_accessed_by_absolute_paths)]` | **compiles, not adoptable** | Every path spelling compiles (unqualified before the include still fails). Every build of the crate then ends `warning: the following packages contain code that will be rejected by a future version of Rust: v1-today …`; `cargo report future-incompatibilities` shows the lint at each call site. **For a dependency that is not local** (the allowing crate committed to a git repository, `gitdep.sh`), the same warning is printed in every *consumer's* build, and the report tells the consumer to "ensure the maintainers know of this problem" or `[patch]` the dependency |
| 3a | `macro_rules! tr` (not exported) + `pub(crate) use tr;` | in-crate yes, other crates **no** | In the crate: every spelling PASS, before and after the include, except unqualified before it. An import after the include is redundant (textual scope wins): `warning: unused import: crate::tr`. A crate that never calls it: `warning: unused macro definition: tr`. Another crate: `error[E0603]: macro `tr` is private` |
| 3b | exported `tr` + `pub(crate) use tr;` at the root | **fails to compile** | `error[E0255]: the name `tr` is defined multiple times` … `tr must be defined only once in the macro namespace of this module`; and a prelude's `pub use super::tr;` is #52234 again |
| **3c** | `#[doc(hidden)] #[macro_export] macro_rules! __mf2_tr` + `pub use __mf2_tr as tr;` + `pub mod prelude { pub use super::tr; }` | **everything but one spelling, one name everywhere** | In the crate, before and after the include: `crate::tr!`, `use crate::tr;`, `use crate::prelude::*;` PASS, no lint, no warning; unqualified at the root after the include PASS. Unqualified in a module without an import: `cannot find macro `tr`` with rustc's help `use crate::tr;` — which now compiles. Another crate: `my::tr!`, `use my::tr;`, `use my::prelude::*;` PASS. A **binary** crate (a module before the include with the prelude, one after with `use crate::tr`, the root): PASS, no warning |
| 3d | 3a + `#[macro_export] macro_rules! __mf2_tr_export` re-exported as `exports::tr` | in-crate as 3a; other crates only as `my::exports::tr` | `my::tr!` from another crate: `error[E0603]: macro `tr` is private` |
| 3e | 3c + a textual `macro_rules! tr` for the code after the include | **ambiguous where both are in scope** | Unqualified after the include PASS; but `use crate::tr;` or `use crate::prelude::*;` in a module after the include: `error[E0659]: tr is ambiguous` … `ambiguous because of a conflict between a macro_rules name and a non-macro_rules name from another module` |
| 4 | an env-driven proc macro, by path (`use v4_env_macro::tr_env;`) | **works; not chosen** | `tr_env!` reads `MF2_MANIFEST` / `MF2_MANIFEST_HASH` from the build script's `cargo::rustc-env`; `tr_outdir!` reads `$OUT_DIR/manifest.mf2m`. Before, after and at the root: PASS. **Invalidation:** `$other` added to the source's `greet` → both call sites report `message greet needs argument other (its variables: $name, $other); write tr!("greet", name = …, other = …)` at the id; reverted → PASS; a translation-only edit reruns the build script and recompiles the crate, as today. **Relocated target directory** (`mv target target-moved`, a source touched, `CARGO_TARGET_DIR=target-moved`): the build script stays fresh, and both macros saw the *moved* path — the stored output still says `target/…`, so cargo rewrote the old `OUT_DIR` prefix in the replayed `rustc-env`. **rust-analyzer:** expands both; the seeded negative controls are reported at the call site (`unknown message id helo; did you mean hello?`, `message greet has no variable $nme …`), no `unresolved-macro-call` / `unresolved-proc-macro` |
| 5 | a library prelude's `tr` (glob-imported) against a generated `tr` | **conflict** | Root glob + generated `tr`, call at the root: PASS (the generated one). A module *after* the include with the glob: `error[E0659]: tr is ambiguous` (today's shape and 3a's alike). A module *before* the include with the glob: the prelude's `tr` is chosen **silently** (the stand-in's `compile_error!("the PRELUDE's tr! was chosen")` fired). Glob + explicit `use crate::tr;` after the include: today's shape → #52234; 3a → PASS with `unused import` |

**rust-analyzer** (`rust-analyzer diagnostics .` over the probe with 3c's call
sites on and seeded controls, 49 s): 3c and 4 both expand; each seeded error
is reported at its id or argument; the only other diagnostics are
`inactive-code` hints and one clippy hint in `mf2-catalog`
(`results/ra-diagnostics.txt`).

### Verdict

**The in-crate mechanism: 3c.** The generated module exports the wrapper
under a hidden name and re-exports it as `tr` (and `msg_id` alike), and adds
a `prelude`. Because `tr` is then a `use` of the exported macro — not the
macro itself — it can be named by path in the crate that includes it, which
the exported macro cannot. It is today's wrapper, proc macro, manifest path,
hash and relocation fallback, unchanged; only two lines of the module move.
Variant 4 works too (and survives relocation without a fallback), but it
gives the crate a second spelling (`mf2::tr!` inside, `my::tr!` outside), a
new public proc macro in `mf2`, and variant 5 shows that a `tr` in `mf2`'s
own prelude collides with a generated one. **Done when:** met.

**Proposed text for 05 §4** (replacing "and **inside the i18n crate itself
only unqualified** … fixture's tests are written that way"):

> **`tr!` in its own crate and in others** (2.0; Phase 10 A3). The module
> exports the wrapper under a hidden name and names it `tr` with a `use`,
> and does the same for `msg_id`:
>
> ```rust
> #[doc(hidden)] #[macro_export] macro_rules! __mf2_tr { … }   // path and hash baked in, as before
> pub use __mf2_tr as tr;
> pub mod prelude { pub use super::tr; /* and the other generated names */ }
> ```
>
> A `macro_export` macro that arrives through `include!` cannot be named by
> a path in its own crate (rustc #52234); a `use` of it can. So:
> * **in the crate that includes the module** — a one-crate application, or
>   a library with messages of its own — any module, declared before or
>   after the include, writes `use crate::tr;` or `use crate::prelude::*;`
>   (or `crate::tr!(…)`); at the root, after the include, `tr!` needs no
>   import. A module that forgets gets rustc's own suggestion, `use
>   crate::tr;`, which compiles;
> * **in a crate that depends on a translation crate** — `my_i18n::tr!(…)`,
>   `use my_i18n::tr;` or `use my_i18n::prelude::*;`, as in 1.x;
> * **three rules keep the name unambiguous:** the module defines no
>   textual `macro_rules! tr` beside the re-export (the two are ambiguous,
>   E0659, wherever both are in scope); `mf2`'s own prelude has no `tr` (a
>   glob-imported `tr` is ambiguous with the generated one after the
>   include, and silently chosen before it); and
>   `macro_expanded_macro_exports_accessed_by_absolute_paths` is never
>   allowed — a crate that allows it prints a future-incompatibility warning
>   in every build that depends on it.
>
> Upgrading from 1.x: a module declared after the include that called `tr!`
> unqualified now imports it (`use crate::tr;` or the prelude).

### Found along the way

- **A missing `mf2.toml` rebuilds the crate on every build.** `mf2-build`
  prints `cargo::rerun-if-changed=<crate>/mf2.toml` whether or not the file
  exists; cargo reports `Dirty v4-env …: the file v4-env/mf2.toml is missing`
  and reruns the build script and recompiles the crate each time, with
  nothing edited (`results/v4.no-mf2-toml-rebuilds.txt`). D19 makes
  `mf2.toml` optional, so C6 has to print the line only for a file that
  exists (or watch the directory for its creation). Observed on 1.x.
- **`neutral-numbers` fires on a corpus with no number in it:** a single
  `{$name}` placeholder draws "this locale formats numbers but fn-number is
  off" once per locale, because an unannotated placeholder may receive a
  number. Recorded; not in A3's scope.

### What it means for the design (A8, C6, C7)

- C4/C6 generate 3c's shape (`__mf2_tr` + `pub use … as tr`, `__mf2_msg_id`
  + `msg_id`) and the crate `prelude`; the 2.0 samples write `use
  crate::prelude::*;` (or `use crate::tr;`) in each module that calls
  `tr!` — the work order's `src/ui.rs` sample needs that line.
- `mf2`'s own prelude (if 2.0 has one) must not contain `tr`.
- The native one-crate default (D19) needs nothing more from the compiler;
  the in-crate part of D19 is settled without a fallback.
- C7's `mf2 init` scaffold note and the fixture's comment about
  "unqualified only" go; "Upgrading from 1.x" gains the import line.

## A4 — the ambient store's cost: what was built

* **Where.** Built on the probe branch `p10-a4-ambient` (off `2fb7f54`;
  not merged): `d6836f8`, `9c8dc82`, `d3fb379`, `07ac660`, `be498b1`,
  `aca96c4` (the runtime seam), `4e31af1`, `389b4be`. The probe is copied to
  `main` as `probes/p10-ambient/`, with the seam as
  `probes/p10-ambient/seam.patch` (the runtime on `main` is unchanged). A
  standalone workspace over the path crates (its `README.md` lists every
  command):
  * `i18n/` — a 112-message trippy-shaped corpus (63 plain, 24 with
    arguments, 10 plurals, 15 with markup) in `en` and `fr`, built by
    `mf2-build` with `Emit::Native`, laid out as the 1.x book's native page;
  * `ambient/` — variants (b) and (c): the store, the lookup, `Display`,
    `to_string` / `to_cow`, the zero-copy Ratatui `Line`, the theme;
  * `bench/` — each variant as a frame and as single-message cases, a
    counting allocator, `ambient-bench check | allocs | breakdown | time |
    time-mt | pools`, and the thread tests;
  * `cli-a/`, `cli-b/` — one three-message CLI on (a) and on (b).
* **The variants.** Each formats in `fr`, not the source locale.
  * **(a)** 1.x: an app-owned `NativeI18n` passed by reference; `format` →
    `String`; `mf2_ratatui::line` with `MarkupStyles`.
  * **(b)** the design: `install(&'static Corpus)` fills a `OnceLock` store.
    The catalogs read the executable's bytes in place and the `Catalog`
    values are leaked once, so each is `&'static`. The active locale is one
    `AtomicUsize`. A thread's override is one thread-local `Cell`, set by
    `with_locale` and restored by a guard. The settings (bidi, time zone,
    theme) sit behind an `RwLock` with a generation counter; each thread
    keeps a copy and re-reads the lock only when the generation moved.
    A simple message is borrowed (`to_cow` → `&'static str`); others are
    formatted into a reused scratch and copied out once; `Display` streams
    into the `fmt::Formatter`. Markup goes through a zero-copy sink, styled
    by a theme keyed by `markup_key`.
  * **(b-copy)** (b)'s store with 1.x's outputs (a `String` per text, 1.x's
    sink): the store's own cost, all else as (a).
  * **(c)** (b), reading the settings lock on every format.
  * **(d)** the port's shape: `thread_local! { RefCell<Option<NativeI18n>> }`
    and a `t!`-like wrapper; a `String` per message; 1.x's `line`.
* **Correct first** (`ambient-bench check`, 2026-09-28): all six variants
  render the 112 messages with (a)'s text and styles, compared character by
  character with each character's style (the zero-copy sink splits spans
  differently). In (b)'s frame, 100 spans borrow catalog text and 43 own
  theirs (34 formatted messages, 9 placeholders).

### Allocations

Exact; `ambient-bench allocs` run twice, byte-identical tables
(2026-09-28). One iteration after a warm-up. "Frame" is the 112 messages
into Ratatui `Line`s: plain ones `Line::from(Span)`, markup ones styled.
Allocations / bytes requested:

| case | (a) 1.x | (b) design | (b-copy) | (c) | (d) port |
|---|---:|---:|---:|---:|---:|
| simple → `String` | 1 / 13 | 1 / 13 | | 1 / 13 | 1 / 13 |
| simple → `&'static str` | — | **0** | | 0 | — |
| one `:integer` argument → `String` | 1 / 8 | 1 / 7 | | 1 / 7 | 1 / 8 |
| one string argument → `String` | 2 / 33 | 1 / 22 | | | 2 / 33 |
| one string argument → `println!` | 2 / 33 | **0** | | | 2 / 33 |
| markup → `Line` (`status.connected`) | 10 / 557 | **3 / 203** | 10 / 557 | 3 / 203 | 10 / 557 |
| **frame** | **413 / 27,631** | **227 / 21,483** | 386 / 27,043 | 227 / 21,483 | 413 / 27,631 |

(b)'s three ways of proving text `'static` (below) allocate alike: R1, R2
and S are each 3 / 203 and 227 / 21,483. `ToString` through `Display`
(`Show(&m).to_string()`) is 2 / 33, against the inherent `to_string`'s
1 / 22.

Where a frame's allocations go (`ambient-bench breakdown`). In (b):
* a plain simple message costs only its `Line`'s `Vec` (63 messages: 2 →
  1);
* a markup message costs its `Vec` and one `String` per placeholder (the 7
  one-key hints: 6 → 1; `status.connected`: 10 → 3).

What is left is not the store's:
* a `&str` argument from a variable is copied into an `Arc<str>`, one per
  argument (a literal is not: the macro emits `ArgValue::str_static`) —
  C1's measurement;
* each of the 10 `.match` messages allocates **4 times inside the runtime**
  (its `Scratch` working lists: declarations, inputs, selectors), in 1.x
  and in the design alike (7 → 6 a message);
* the `Line`'s own `Vec`, which any `Line` needs.

### Time

Taken **under load** (the other probes building: load average 9–16 on the
8-thread i7-1165G7), so only the ratios are the result. The variants run
round-robin in one process, each round a batch of iterations per variant
(≈ 2 ms of the first), the starting variant rotating; each row is the
median of 101 rounds' means, with their 10th–90th percentiles.
`ambient-bench time 101` (seam build), 2026-09-28, load 8.95 → 9.50:

| case | variant | median ns | p10–p90 | × (a) |
|---|---|---:|---:|---:|
| lookup: catalog + settings | (a) / (b) / (c) / (d) | 7.7 / 14.1 / 41.2 / 10.2 | | 1 / 1.82 / 5.32 / 1.32 |
| simple → `String` | (a) | 187 | 181–219 | 1.00 |
| | (b) | 144 | 136–159 | **0.77** |
| | (c) | 142 | 136–168 | 0.76 |
| | (d) | 191 | 180–209 | 1.02 |
| simple → `&'static str` | (b) / (c) | 84 / 84 | 80–89 | |
| one `:integer` argument → `String` | (a) | 1,693 | 1,584–1,860 | 1.00 |
| | (b) | 1,749 | 1,718–1,808 | 1.03 |
| | (c) | 1,754 | 1,689–1,872 | 1.04 |
| | (d) | 1,707 | 1,661–1,909 | 1.01 |
| one string argument → `String` | (a) / (b) / (d) | 861 / 862 / 887 | | 1.00 / 1.00 / 1.03 |
| | `Show(..).to_string()` | 972 | | 1.13 |
| one string argument → `println!` | (a) | 979 | 829–2,652 | 1.00 |
| | (b) | 821 | 716–2,156 | **0.84** |
| | (d) | 1,058 | 865–2,388 | 1.08 |
| markup → `Line` | (a) | 2,978 | 2,905–3,397 | 1.00 |
| | (b-copy) | 3,035 | 2,864–3,523 | 1.02 |
| | (b), R2 | 2,402 | 2,300–2,821 | **0.81** |
| | (b), R1 | 2,464 | 2,347–2,641 | 0.83 |
| | (b), S | 2,349 | 2,210–2,676 | 0.79 |
| | (c) | 2,416 | 2,301–2,693 | 0.81 |
| | (d) | 3,002 | 2,919–3,721 | 1.01 |
| **frame** (112 messages) | (a) | 136,018 | 122,203–152,422 | 1.00 |
| | (b-copy) | 136,392 | 125,486–154,422 | 1.00 |
| | (b), R2 | 120,001 | 108,225–145,001 | **0.88** |
| | (b), R1 | 118,610 | 106,902–166,914 | 0.87 |
| | (b), S | 116,591 | 104,968–143,971 | 0.86 |
| | (c) | 118,971 | 106,470–133,828 | 0.87 |
| | (d) | 140,038 | 124,360–199,932 | 1.03 |

Three earlier runs of the same binary without the seam (loads 15–22) agree:
(b)'s frame 0.85–0.87 × (a), its markup `Line` 0.79–0.82 ×, (b-copy)
0.97–1.00 ×, (d) 1.02–1.04 ×. The one-argument row is the only one where
(b) is not below (a): 1.00–1.03 × across the runs (the store's lookup plus
the scratch's copy; the same one allocation).

Frames on several threads at once (`ambient-bench time-mt THREADS 21`: each
round starts the threads on a barrier and each thread times 40 frames; the
round's figure is the median thread's). Five runs, 2026-09-28, loads
10.7–22:

| threads | (b) × (a) | (c) × (a) | (d) × (a) |
|---|---:|---:|---:|
| 8 (three runs) | 0.78 / 0.83 / 0.84 | 0.79 / 0.90 / 0.95 | 1.01 / 1.12 / 1.18 |
| 4 (two runs) | 0.81 / 0.93 | 0.78 / 0.99 | 0.93 / 1.19 |

Under this load the threaded runs cannot separate (b) from (c). The
single-thread lookup can: (c)'s read lock costs ≈ 27–31 ns a format more
than (b)'s copy (41–46 against 14–15 ns; 1.x's handle 7.4–7.7 ns); per
frame that is ≈ 3 µs of ≈ 120 µs.

### Stripped size

`cargo build --profile stripped -p cli-a -p cli-b` (release: opt-level 3,
fat LTO; symbols stripped). The same CLI both ways: the system's language
or the first argument; one simple message, one with a string argument, one
plural.

| | bytes |
|---|---:|
| (a) `NativeI18n` | 1,145,640 |
| (b) the store, printed through `Display` | **1,140,392 (−5,248)** |

The first (b) was **+3,784 B**. `nm -S --size-sort` over both showed a
`Display` impl per description type (`Show<Tr>` 3,200 B, `Show<TrArgs>`
1,976 B), each inlining the whole path; and, once they shared one function
taking `&dyn Msg`, the **parts path** (≈ 3.8 KB) — kept alive by the
vtable, which lists `Message::parts`, in a CLI that never calls it. Each
form now forwards to one non-generic function taking a trait object that
lists only what that form calls (`write` for text, `parts` for Ratatui),
and R2's pools are built by the first Ratatui conversion, not by `install`.

### Zero copy

* **Constant text.** A simple message is `&'static str` straight from the
  `&'static Catalog`: `to_cow` allocates nothing (above).
* **Pattern text parts**, recovered as `'static` in safe code (`ptr as
  usize`, `str::get`, `ptr::eq`; no `unsafe`); a part that cannot be proven
  is copied, so correctness never depends on the proof. Three methods,
  measured side by side (the tables above):
  * **R1** — a pointer-range check against the catalog's `&'static` bytes,
    then `from_utf8` on the re-slice: O(length) a part;
  * **R2** — the same check against the catalog's string pool, validated
    once as `&'static str` (2,731 B, both locales, in 1,389 ns, best of
    101: `ambient-bench pools`); `str::get` re-slices in O(1). **No change
    to the runtime**;
  * **S** — a hidden seam, `PartSink::part_catalog_text(&Catalog, StrRef)`
    in `mf2-runtime` (commit `aca96c4`), whose default is today's
    behaviour; the native sink resolves the reference against its own
    `&'static Catalog` after a `ptr::eq`. No range check, no pool.
* **S against B1 and B12.** `cargo xtask size` in this worktree on
  `2fb7f54` (`--out target/p10-a4/size-base`), then with the seam
  (`--keep`: the same generated sources at the same paths; the runtime
  rebuilt): **byte-identical**, every application at both scales (B1 fixed
  26,416 B gz, B5 8.3 B gz a site, the whole app 41,872 B gz, both runs);
  the gate's applications never format to parts. `bench/b12/check.sh`
  (its `runtime` harness formats to parts through a `PartSink`): runtime
  38,336 → 38,324 B raw (−12), 19,223 → **19,224 B gz (+1)**; **B12 clean**
  in both runs.
* Recorded for A1's reference (not this probe's A/B): this worktree's
  baseline, B1 26,416 / B5 8.3 / whole app 41,872 B gz, differs from A1's
  main-tree 26,676 / 8.2 / 41,889 at the same commit. The cause, checked
  by the coordinator: `Cargo.lock` is not committed, and the worktree
  resolved its dependencies afresh — Leptos `0.9.0-beta2` (the main tree's
  lock has `0.9.0-beta`), newer `js-sys` / `wasm-bindgen` and others. **A
  size A/B is valid only within one tree and one lock**; the ±64 B gates
  compare against a base measured under the same lock (A1's figure is the
  main tree's, until its lock is updated).

### Threads

`cargo test --release -p ambient-bench`; the probe's tests run in parallel
with each other on purpose, since the store is process-wide.
* 16 threads, half pinned to `en` and half to `fr` by `with_locale`, each
  formatting 2,000 rounds: a simple message (`to_cow` and `Display`), an
  argument message and a markup `Line`. All correct, while another test
  changes the app-wide locale, bidi and time zone.
* `set_locale`, `set_bidi` and `set_time_zone` are seen by another thread's
  next format (a worker answering over a channel).
* `with_locale` nests, and is restored when its body unwinds; an unknown
  locale is an error.
* Formatting before `install()` panics with "call install() at start-up"
  (a separate test binary).

All pass; the three-test binary passed 20 of 20 repeated runs.

### Verdict

* **The ambient store (C2)** against method §3's gate, both parts **met**:
  * time per frame ≤ 1.x: (b) is 0.86–0.88 × (a) (0.85–0.87 × in the
    earlier runs), measured interleaved in one process rather than by
    alternating binaries;
  * stripped size ≤ 1.x: −5,248 B.
  * The store's own cost, isolated by (b-copy), is within noise
    (1.00 × per frame).
  * (d), the port's `RefCell`, is 1.02–1.04 ×, and 413 allocations like
    (a).
* **The text-borrowing method chosen: S**, the hidden seam.
  * Its cost on the client is −12 B raw / +1 B gz on B12's runtime harness
    and 0 B on the whole-app gate (the gate is ±64 B gz); B12 stays clean.
  * It is the fastest of the three (within noise), needs neither a pool
    nor knowledge of the catalog's layout, and makes "catalog text arrives
    with its reference" a contract rather than an inference from pointers.
  * It is the pattern `Sink::push_catalog_text` already set.
  * **Fallback:** R2, which changes nothing in the runtime.
* **Done when** — D17's figures against the gates, and the text-borrowing
  method chosen: **met**. Nothing needs the owner.

### What it means for C2 and C5 (interpretation)

* **C2: keep (b)'s shape.** A `OnceLock` store of `&'static` catalogs, an
  atomic locale index, a thread-local override with a restoring guard, and
  settings copied per thread by generation. The per-thread copy saves
  ≈ 27–31 ns a format over the lock. The native-only panic naming
  `install()` works as decided.
* **The public forms stay thin.** `Display` for each description, and
  every `From<…>` into Ratatui, should be one-line forwarders to one
  non-generic function taking a trait object that lists only what that
  form calls. A `&dyn Message` would pull the parts path into every CLI
  (+≈ 3.8 KB here).
* **Keep an inherent `to_string`** beside `Display`. It is faster and
  allocates once (0.99–1.00 × against `ToString`'s 1.09–1.13 ×; 1 against
  2 allocations). `println!("{}", tr!(…))` streams with none. This is
  A5's `inherent_to_string_shadow_display` question.
* **C5 inherits the zero-copy sink.** Text parts are borrowed through S;
  a placeholder is one `String`; the open-element stack is inline and keyed
  by `markup_key`, so markup names never allocate; a line break becomes a
  borrowed `" "` span. With it, a markup line falls from 10 allocations to
  its `Line`'s `Vec` plus one per placeholder (and C1's argument copies),
  and the frame from 413 to 227. C5's
  allocation gate (≤ 1.x and ≤ the upstream re-implementation) should hold
  on this evidence for 1.x; the upstream baseline is A1's `upstream.rs`.
* **Not the store's, and left to their owners:**
  * the `Arc<str>` copy of a variable `&str` argument (C1);
  * the runtime's 4 `Scratch` allocations per `.match` message, which
    affect 1.x and 2.0 alike.
* **Not measured here:** the ambient lookup's first step when `ssr` is also
  on (the request context before the native store, D17's order). In a
  native-only build that step is compiled out; with both features unified
  in one workspace, every native format pays it. C2 should time it.

## A5 — `Display` / `Debug` against B12: what was built

* **Where.** The probe branch `p10-a5-display` (one commit, **`70e0b27`** on
  `2fb7f54`; not merged). On `main`: `probes/p10-display/` holds the
  variant as `display.patch`, the scripts (`measure.sh`, `named.sh`,
  `twiggy.sh`, `ours.sh`, `clippy-variant.sh`, the positive control's
  `control.sh` / `control-stripped.sh`) and the analysis (`analysis/*.py`).
  The saved outputs stay in that worktree's git-ignored `target/a5/`
  (`{base,variant}/`, `named/`, `analysis/`'s outputs) until the worktree
  is removed.
* **The variant** (`crates/leptos-mf2`, 11 files, +243 −55):
  * `src/display.rs`, new and compiled in every build, holds the string
    conversions, moved out of the Leptos-only `glue/view.rs`: the inherent
    `to_string()` (isolated), `to_plain_string()`, `to_display_string()`,
    `From<_> for String`, and **`Display` for `Tr`, `TrArgs`, `TrRich`,
    `TrDyn`**. With a Leptos mode they read the ambient catalog:
    `text::to_string`, and for `Display` a new `text::fmt_display` that
    writes the text through `Formatter::pad` (so `{:<12}` pads) with no
    `String` in between. With no Leptos mode both write nothing: a stand-in
    until C2's native store.
  * `to_string` carries `#[allow(clippy::inherent_to_string_shadow_display,
    reason = "the client's fmt-free form of what `Display` writes (B12, 06
    §3); both produce the same text")]`, in place of 1.x's
    `inherent_to_string` allow.
  * **`Debug`**: derived on `TrArgs`, `TrDyn`, `DateTimeValue`, `Stored`,
    `RequestI18n`, `Handler<H>`, `Flat<F>`, `SignalArg<S>`; hand-written for
    `TrRich` (id, arguments, number of handlers), `ArgValue` (`Custom(..)`,
    `Source(..)`), `Text` (the text, quoted), `ArgList` (a list),
    `NestingHandler(..)`, `FlatHandler(..)`. `Tr` had it already.
  * `tests/render.rs`: `{}`, `ToString::to_string` and the inherent
    `to_string()` give the same isolated text; `{:<16}` pads; the `Debug`
    shapes (`TrArgs { id: MsgId(0), args: [Str("Ada")] }`,
    `TrRich { …, handlers: 1 }`, `Custom(..)`).
* **How it was measured.** `bash target/a5/measure.sh base` at `2fb7f54`
  (2026-09-28, 04:53–06:45), then `… variant` at `70e0b27` (06:45–07:36).
  Each runs `cargo xtask size`, `cargo xtask b5 --view`,
  `cargo xtask b12-generated` and `bash bench/b12/check.sh` with
  `CARGO_BUILD_JOBS=2`, and keeps every wasm. `bash target/a5/named.sh
  base|variant` rebuilt three clients with their symbol names kept
  (`strip = false`, `wasm-opt -Oz --debuginfo`) for twiggy: `tr` at 1,860
  sites, `tr-view` at 1,860 sites (a view workload generated once into
  `target/a5wl`), and the fixture client. In the `tr` app `leptos-mf2` is
  the Leptos-free core (`workload-i18n/hydrate` turns on only
  `mf2/host-web`), and its sites format through `Tr::format`. `tr-view`
  has the Leptos layer, and its string positions call the inherent
  `to_string()` that the variant moved.
* **The A/B is valid** (checked):
  * HEAD moved to `70e0b27` at 06:37:42 (`git reflog --date=iso`), while
    the base's `check.sh` was running (06:36:30–06:45:08). B12's harnesses
    depend only on `mf2-catalog`, `mf2-runtime`, `mf2-fn-number`,
    `mf2-fn-datetime` and `mf2-host-web` (`bench/b12/*/Cargo.toml`), none
    of which the variant touches, and base and variant `b12.txt` and
    `size.tsv` are byte-identical. Every other base figure, and the base's
    named builds (05:46–06:05), finished before 06:37:42;
  * **one resolution.** Each generated app has its own `Cargo.lock`, which
    each run resolves again. The variant's named build of `tr`, in the
    target directory the base's used, rebuilt only `leptos-mf2`, `mf2`,
    `workload-i18n` and the app; no dependency unit was built (the
    `.fingerprint` files written after 07:00). The `idlit`, `idlit-view`
    and `dummy` wasm are byte-identical between the runs at both scales
    (`cmp` over `target/a5/{base,variant}/wasm/`);
  * this worktree's base (26,400 / 8.3 / 41,871) is not A1's main-tree
    figure (26,676 / 8.2 / 41,889) nor A4's worktree's (26,416 / 8.3 /
    41,872) at the same commit: a lock of its own, as A4's record says. The
    gates below compare within this tree.

### Figures

| Figure | Base `2fb7f54` | Variant `70e0b27` | Change | Gate | Command |
|---|---:|---:|---:|---|---|
| **B1, fixed** | 26,400 B gz | 26,317 | **−83** | ±64 | `cargo xtask size` |
| **B5**, per site | 8.318 B gz (shown 8.3) | 8.350 (8.3) | **+0.032** | ±0.2 | same |
| **whole app**, 1,860 sites | 41,871 B gz | 41,848 | **−23** | ambition 105,120 | same |
| the `dummy` bound: per site / fixed | 25.2 / 27,383 | 25.3 / 27,300 | | reported | same |
| `tr` at 1,860 sites: opt raw / opt gz | 2,419,654 / 693,681 | 2,419,575 / 693,658 | −79 / −23 | | same |
| `tr` at 3,720 sites | 4,455,995 / 1,203,373 | 4,455,916 / 1,203,410 | −79 / +37 | | same |
| `idlit`, `dummy`, both scales | | | byte-identical | | `cmp` |
| `b5 --view`: per site, against `idlit-view` | 10.398 B gz (10.4) | 10.398 (10.4) | 0 | ±0.2 | `cargo xtask b5 --view` |
| `b5 --view`: fixed | 24,956 B gz | 24,958 | +2 | | same |
| `tr-view` at 1,860 / 3,720: opt raw | 1,890,880 / 3,375,262 | 1,890,883 / 3,375,265 | +3 / +3 | | same |
| `tr-view` at 1,860 / 3,720: opt gz | 572,548 / 956,397 | 572,550 / 956,399 | +2 / +2 | | same |
| `idlit-view`, `dummy` | | | byte-identical | | `cmp` |
| **B1′** on the generated module (F − E) | +0 B | +0 B | | +0 | `cargo xtask b12-generated` |
| B13 (B − A) | 13,573 B avoided | 13,573 | 0 | 13,599 ± 10 % | same |
| the fixture client, raw: E / F | 349,158 / 349,158 | 349,158 / 349,158 | 0 | | same |
| the fixture client, raw: A / B | 365,010 / 378,583 | 365,001 / 378,574 | −9 / −9 | | same |
| **B12** | clean | clean; report byte-identical | | clean | `bash bench/b12/check.sh` |

B12's per-harness figures are identical in both runs: the reader 6,781 B gz;
B1's runtime part 18,888 (the core numbers 5,407); B2 2,022; B3 5,486;
B1′ for `fn-number` and `fn-datetime` 0; the date semantics 3,605; B4
`datetime-intl` 5,157 plus 672 of JS; B1′ for `intl` −63. Each gated
harness shows no panic import, 0 fmt symbols and 0 panic symbols; the control
harness shows both. The verdict line, both times: "B12: clean (no panic
path, no core::fmt in the reader, the runtime, the numeric and the date
functions); B13: shown".

### Why B1's fixed part moved −83 B gz

Observed:

1. **The arithmetic.** `b5::delta` (`xtask/src/b5.rs`) fits two scales:
   marginal = (Δ@3720 − Δ@1860) / 1,860 and fixed = Δ@1860 − 1,860 ×
   marginal, where Δ = `tr` − `idlit`. Because 3,720 = 2 × 1,860, that is
   **fixed = 2·Δ@1860 − Δ@3720**, and the whole app is Δ@1860. `idlit` did
   not change, so the fixed part moved 2 × (−23) − (+37) = **−83**, the
   whole app −23, and B5 (37 + 23) / 1,860 = +0.032.
2. **The delivered bytes.** The variant's `tr` wasm is **79 B smaller at
   both scales**, as a fixed change should be. The code section is −78 B
   and the function section −1 B: one function fewer (9,009 → 9,008;
   15,924 → 15,923). The data section (124,732 B), the element section and
   every other section are the same size, so no string constant came in (a
   reachable `Debug` impl would bring its names). Command:
   `python3 target/a5/analysis/sizes.py BASE VARIANT` over the kept
   `opt.wasm` files.
3. **Why gzip reads −23 at one scale and +37 at the other.** With one
   function fewer, every function after it is renumbered. At 1,860 sites
   272 bodies (1,394,833 B, 62 % of the code) differ in their bytes; at
   3,720 sites 602 (2,763,348 B, 66 %) do. In each case all but the seven
   below keep their length (`python3 target/a5/analysis/bodies.py BASE
   VARIANT`, with call targets normalised). The compressor's matches change
   with them. The same files read differently with each compressor:

   | | raw | flate2 level 9 (the gate) | `gzip -9 -n` | `brotli -q 11` |
   |---|---:|---:|---:|---:|
   | `tr` at 1,860 sites | −79 | −23 | −30 | −130 |
   | `tr` at 3,720 sites | −79 | +37 | +127 | −473 |
   | **the fixed part** (2·δ₁ − δ₂) | −79 | **−83** | −187 | +213 |
   | `tr-view` at 1,860 / 3,720 sites | +3 / +3 | +2 / +2 | +7 / 0 | −273 / −759 |

   (`gzip -9 -n -c F | wc -c` and `brotli -q 11 -c F | wc -c` over
   `target/a5/{base,variant}/wasm/…/opt.wasm`; the gate's column is
   `cargo xtask size`'s.)
4. **Which code.** At the function level the `tr` change is **seven bodies
   (717 B) replaced by six (639 B)**, the same sets at both scales, and all
   of them destructors. Method: `wasm-dis` over the measured `opt.wasm`,
   then `python3 target/a5/analysis/match.py`, which normalises every
   index, name and constant and names each changed function by its text in
   the named builds.
   * `drop_glue::<ArgValue>` 162 → 165 B; `Arc<DateTimeValue>::drop_slow`
     105 → 66 B.
   * `drop_glue::<Text>` (141 B) and `drop_glue::<Arc<str>>` (126 B) are
     functions of their own in the variant. Both named builds have them;
     the measured base does not.
   * Three base bodies (171, 98, 94 B) and one variant body (132 B) have no
     twin in the named builds.
   * One copy fewer of two shared drop shapes (the text of
     `drop_glue::<SplitLoaderFuture>`, 50 B; a generic
     `drop_glue::<Arc<…>>`, 37 B), and one copy more of a 9 B `Weak::drop`
     shape.

   In `tr-view` one function changed: `drop_glue::<ArgValue>`, 162 → 165 B.
   It is the same `Arc` release, written with `ptr + 4` held in a local and
   an `if` in place of a `br_if`. So the moved `to_string()`, which
   `tr-view`'s string positions call, compiles to the same code.
5. **Where that code runs** (`twiggy paths -d 3 -r 12` on the named base
   `tr`). `Arc<dyn ArgSource>::drop_slow` and `Arc<DateTimeValue>::drop_slow`
   are reached only through `drop_glue::<[ArgValue]>` and
   `drop_glue::<ArgValue>`, from the template's one formatting helper
   `support::s::<TrArgs>` and from
   `tr::with_args::<(), TrArgs::write::{closure}>`. That is the destructor
   of a `TrArgs` after it has been formatted: one copy per application,
   hence the same −79 B at both scales.

Interpretation (brief; not established): the variant adds non-generic items
that rustc compiles whether or not anything calls them. `From<TrArgs> for
String` and its siblings take a description by value and drop it, and the
`Debug` impls walk `ArgValue`. Fat LTO removes them as unused, but the
destructors they share were optimised while they were present, and were
inlined differently. The named builds (`strip = false`, `--debuginfo`) show
−160 B where the measured show −79, which fits an effect that depends on
layout.

### What twiggy sees

* **Nothing of ours is `Display`, `Debug` or `core::fmt`** in the variant's
  three named builds (`bash target/a5/twiggy.sh variant`;
  `bash target/a5/ours.sh target/a5/named/variant/size-wl-1860-tr`). The
  items that match B12's fmt pattern are **identical in name and size** to
  the base's, once binaryen's `.N` suffixes are dropped: `tr` 72 items /
  10,354 B, `tr-view` 72 / 10,354 B, the fixture 32 / 3,272 B. All of them
  belong to `core`, `alloc`, `std` and Leptos's stack (`hydration_context`,
  `leptos`, `wasm_bindgen`, `wasm_split_helpers`, `async_once_cell`). The
  list of every `Display` / `Debug` impl in `tr` is the same in both.
* **`twiggy diff`** on the named builds
  (`twiggy diff -n 60 target/a5/named/base/X/opt.wasm target/a5/named/variant/X/opt.wasm`):
  * **`tr`, +53 B:** the name section +213 and the code −160, all
    destructors. `Arc<dyn ArgSource>::drop_slow` −96;
    `Weak<dyn ArgSource>::drop` −89 against `Weak<dyn SharedContext>::drop`
    +89 (one merged body under another name);
    `Arc<DateTimeValue>::drop_slow` −39;
    `drop_glue::<Arc<oneshot::Inner<Option<Owner>>>>` −39;
    `Weak<oneshot::Inner<…>>::drop` +11; `drop_glue::<ArgValue>` +3.
  * **`tr-view`, +2 B:** `drop_glue::<ArgValue>` +3; a 57 B body renamed
    from `drop_glue::<TrArgs>` to `drop_glue::<ArgList>`; names −1.
  * **The fixture, +3 B:** renamed duplicates (`.165` → `.170`, present in
    both builds), and destructors of `ArgValue`, `Text`, `Arc<str>` and
    `Arc<dyn ArgSource>` moving by ±5 to ±19 B.
* **The named builds are not the measured ones.** With their names stripped
  (`wasm-opt --strip-debug --strip-producers`), named `tr` goes 2,422,903 →
  2,422,743 (−160) where the measured goes 2,419,654 → 2,419,575 (−79);
  `tr-view` is +3 in both. That is why the measured files were compared
  function by function above.
* **The check can fail (positive control).** The fixture client was built
  with one `format!("{save} {items:?}")`, where `save` is a `Tr` and
  `items` a `TrArgs` (a working-tree edit in `target/a5-dev`, reverted and
  never committed).
  * The named build (`target/a5-dev/target/a5/control.sh`, then
    `twiggy.sh`'s fmt grep) has **79 fmt items (20,175 B)** against 32
    (3,272 B). **11 are ours**: `<Tr as Display>::fmt`; `Debug` for
    `TrArgs`, `ArgList`, `ArgValue`, `Text`, `Option<Text>` and
    `Arc<DateTimeValue>`; `mf2-runtime`'s `Date` and `Time`.
  * Built as `b12-generated` builds its A (`control-stripped.sh
    plain|control`): **365,001 → 405,860 B raw; after `wasm-opt -Oz`
    328,579 → 364,332; `gzip -9` of that 87,371 → 103,975 (+16,604 B)**.

### Lints and tests

`bash target/a5/clippy-variant.sh` (in `target/a5-dev`, 2026-09-28; log:
`target/a5/clippy-variant.log`) ran two kinds of step:
* every `cargo xtask ci` clippy step that compiles `leptos-mf2`:
  `--workspace --all-targets`, the nine `wasm32-unknown-unknown` feature
  sets, and `ssr,mark-fallback-lang --all-targets`;
* two steps CI lacks: the Leptos-free core,
  `-p leptos-mf2 --no-default-features --lib`, on the host and on wasm32.

**All exit 0** with `-D warnings`, and none reports a lint in our crates.
The only warning is the future-incompatibility note for the dependency
`proc-macro-error2` 2.0.1. `cargo test -p leptos-mf2 --features ssr --test
render`: 15 passed, the new test among them. The earlier session's
`target/a5/dev-clippy-workspace.log` holds only `exit 0`. Several of the
steps were already fresh in `target/a5-dev/target`, and cargo replays a
fresh unit's warnings.

* **`Debug` coverage**
  (`cargo clippy -p leptos-mf2 --features ssr -- -W missing_debug_implementations`,
  and the same with `--target wasm32-unknown-unknown --features hydrate`;
  outputs in `target/a5/missing-debug-{ssr,hydrate}.txt`). In `leptos-mf2`
  only the glue's tachys render states still lack `Debug`: `TrState`,
  `TrAttrState`, `TrRichState`. Outside this probe, 13 public types of
  `mf2-runtime` lack it (`Arg`, `Value`, `Part`, `ExpressionPart`,
  `MarkupPart`, `MarkupOptions`, `FnContext`, `Options`, `OptionValue`,
  `Number`, `Digits`, `Measure`, `NumberOut`), and 18 of `mf2-catalog`
  (`Entry` and the views).
* **Not run:** `cargo xtask ci` as a whole. `cargo xtask api --check` would
  fail on the branch (inferred, not run): `crates/leptos-mf2/api.txt`
  lists trait impls (`impl core::fmt::Debug for leptos_mf2::Tr`, line 262),
  and the commit adds `Display` and `Debug` impls without regenerating it.

### Verdict

* **Adopted**:
  * `Display` for the four descriptions;
  * the always-on inherent `to_string` / `to_plain_string`, with the
    `inherent_to_string_shadow_display` allow and B12 as its reason;
  * `Debug` on the call-site types, hand-written for `Custom` / `Source`.

  The fallback ("`Display` only with the std modes") is not needed.
* **B1: −83 B gz, outside the ±64 B band, downward. Judged as holding.** The
  band is there to catch a cost, and this move's cause is identified:
  * the delivered code is 79 B smaller at both scales, all of it in the
    destructors of the argument values;
  * no `Display`, `Debug` or `core::fmt` code, and no new data, reached the
    client;
  * the −83 is the fixed part's extrapolation (2 × −23 − 37) of two gzip
    readings of opposite sign.

  The whole app at the reference scale (−23 B gz) and B5 (+0.032 B a site)
  are inside their tolerances, and `b5 --view` moved +2 B fixed and 0 a
  site. The −79 B is not a saving to bank: it is the same destructors
  inlined differently, and the next change to the crate can move it back.
* **B12: clean.** `check.sh` is clean and byte-identical in both runs, but
  its harnesses do not compile `leptos-mf2`, so it cannot see this change.
  The twiggy check over the workload builds does see it; it finds nothing
  of ours, and its positive control shows it would. B1′ is +0 and B13
  13,573 B in both.
* **Done when**: met. Nothing needs the owner; one trade-off is left for
  A8 below.

### What it means for C2 (interpretation)

* **Take the probe's API, not its stand-in.** Without a Leptos mode the
  probe's `to_string()` returns `""` and `Display` writes nothing. If that
  shipped it would be a new silent failure, of the kind Part E removes. C2
  backs both with the native store's lookup (A4: formatting before
  `install()` panics and names it); a build with neither mode should not
  offer them.
* **Use A4's shape for the native forms.** The probe's `Display` is generic
  per description (`ambient::fmt<D>` → `text::fmt_display<D>`). On the web
  client that costs nothing, because nothing links it. A4 measured
  per-type `Display` impls at +3,784 B in a stripped CLI, and chose one
  non-generic forwarder taking a trait object. The inherent `to_string`
  stays (A4: faster, and one allocation against `ToString`'s two).
* **What adopting gives up.** In 1.x, and under the fallback,
  `format!("{}", tr!(…))` in a browser build does not compile. With
  `Display` always on it compiles and pays for `core::fmt`: the control's
  one `{}` and one `{:?}` cost +16.6 KB gz after `wasm-opt`. B12 thus
  moves from a type error to a rule applications must follow.
  `tr.to_string()` stays fmt-free, because method resolution picks the
  inherent method, and the book (F) should tell browser code to use it.
  The work order's rule decides on bytes alone; A8 may want to put this
  trade-off to the owner.
* **`Debug` in 2.0.** A description's `Debug` shows `MsgId(0)`, the number:
  the description carries no name (B6). A8 should state whether 2.0's
  "`Debug` everywhere" covers the runtime's and the catalog's public types
  (31 lack it). `check.sh` does see `mf2-runtime`, so adding them there is
  B12-checked directly.
* **Reading a ±64 B size gate** (B1 and every later size gate). The fixed
  part is an extrapolation, 2·δ₁ − δ₂, so it doubles gzip's noise: the same
  −79 B raw read −83 B with flate2, −187 with `gzip -9` and +213 with
  brotli. Before reaching for a fallback, read a move outside ±64 B
  alongside the raw bytes at both scales (a fixed change is equal at both)
  and a function-level diff.
* **The adopting change** regenerates `crates/leptos-mf2/api.txt`, and any
  other listing that shows these impls, with `cargo xtask api`.

## A6 — a single-crate web application: what was built

**The probe.** `probes/p10-single-crate/hello/`: Getting started's `hello`
in its lazy-route form (delivery-modes.md), as **one crate** — `build.rs`,
`mf2.toml` and `locales/` in the application, no translation crate, no
workspace members. The 1.x crates by path (the tree at `2fb7f54`); Leptos
0.9.0-beta2; cargo-leptos 0.3.9. `build.rs` runs `mf2-build` (`Emit::Both`)
and swaps the generated wrappers for A3's shape (3c), standing in for what
2.0's codegen would emit. `src/lib.rs` includes the module and keeps 1.x's
hand-shaped `setup()`. `tr!` is called from `src/pages.rs` (declared before
the include; `use crate::prelude::*`), `src/app.rs` (after it; `use
crate::tr`), the crate root (no import) and the server binary `src/main.rs`
(another crate; `use hello::tr`). Only `fn-number` is on (the corpus uses
`:integer`; hello's `fn-datetime` / `datetime-icu` were left off to spare the
machine). Everything ran under load (load average 12–18, other forks
building): times are indicative only.

| Check | Result | Observed | Command |
|---|---|---|---|
| both builds | **PASS** | `cargo check --features ssr` (lib + server binary); `cargo leptos build`: the server (`--features=ssr`) and the wasm (`--lib --target=wasm32-unknown-unknown --features=hydrate`), first build 14 min 21 s cold at `CARGO_BUILD_JOBS=1` | `results/build-first.log` |
| a translation edit leaves the wasm byte-identical | **PASS** | wasm `4758729623dccb0e` at every step: nothing changed, mtime only, a translation-only edit (fr `visit-again`), a source-text edit (en `visit-again`), each reverted; `MANIFEST_HASH` `0x445b_1e6d_8cf8_c8e0` throughout. The server binary changes with each text edit (its catalogs are embedded) and returns to `59af98f49826615d` on each revert. Each edit recompiles the one crate in both builds (8–21 s a step) — in the two-crate layout it is the i18n crate *and* the application | `./scenario.sh` → `results/scenario.txt` |
| `--split` | **PASS** | `cargo leptos build --split` emits `split___visits_view_….wasm` for the lazy route. Served (`cargo leptos serve --split`) and opened in Chromium: `/?lang=fr` renders in French with `Content-Language: fr` and `Vary: cookie, accept-language`; clicking the link to `/visits` navigates client-side (no document request) and fetches the chunk; the button, a switch to English and "Apply" fetch only `/i18n/en.….mf2b` and give "You have been here 2 times." (count kept, title switched). Console: only the probe's missing favicon | `results/browser/README.txt` |
| `cargo leptos watch`: is `watch-additional-files` still needed? | **still needed** | Without it: no restart within 240 s of a locale edit, while a control edit to `src/app.rs` restarted it 27 s later. With `watch-additional-files = ["locales"]`: restarted 18 s after the edit, the edit served, then the revert served | `./watch.sh without`, `./watch.sh with` → `results/watch-*.txt` |
| rust-analyzer | **PASS** | `rust-analyzer diagnostics .` (with `default = ["ssr"]` for the run; 156 s): the seeded misspelt id before the include (`unknown message id vistis; did you mean visits?`) and the seeded wrong argument after it (`message greeting has no variable $nmae …`) reported at the call sites; no `unresolved-macro-call` / `unresolved-proc-macro`. One unrelated error: an `E0507` rust-analyzer reports in 1.x `leptos-mf2`'s `text.rs:191` (`*slot = buf` in a `LocalKey::with` closure), which rustc compiles | `results/ra-diagnostics.txt` |
| A3's in-crate `tr!` from several modules | **PASS** | before the include through the prelude, after it by `use crate::tr`, at the root with no import, and from the server binary by `use hello::tr`; in `ssr`, `hydrate` and `--split` builds | the builds above |

**Verdict.** **One crate for the web works**: both builds, the wasm
byte-identical across translation edits, `--split`, hydration and a live
switch, rust-analyzer, and `tr!` from every module. The web starter (D5) can
offer it. **Done when:** met.

### What the one-crate layout still needs (observed)

- **`watch-additional-files = ["locales"]`** — cargo-leptos watches the
  sources, not `locales/`, in one crate as in two. A one-crate starter
  writes the line itself: the manifest is the application's own, which 1.x's
  `mf2 init` could not assume.
- **The function features, twice.** 1.x's build reads the *including*
  crate's `CARGO_FEATURE_*`, so the application declares `fn-number =
  ["mf2/fn-number"]` and turns it on from both `ssr` and `hydrate` (and
  `mf2/host-std` + `mf2/ssr`, `mf2/host-web` + `mf2/hydrate`) — A2's `links`
  metadata is what removes this.
- **`mf2 check` sees none of those features.** In the directory,
  `mf2 check` resolves the crate with cargo's defaults, where neither `ssr`
  nor `hydrate` is on, so it warns `neutral-numbers` for each locale (a
  false warning: both builds have `fn-number`); `mf2 check --features
  fn-number` reports nothing. In the two-crate layout the application names
  the features on the i18n crate's dependency, which the plain resolve does
  see.
- **`setup()` is still hand-written** in `src/lib.rs` (D3's generated setup
  removes it).

### What it means for the design (A8, C6, D3, D5)

- D5: `mf2 init --ssr` / `--islands` / `--csr` can offer the one-crate
  layout, writing `watch-additional-files = ["locales"]`; the two-crate
  layout stays for a workspace where several crates share one corpus.
- C6: `mf2 check` / `compile --site` must see the features the *builds*
  use. If 2.0 puts the function features on the `mf2` dependency itself
  (unconditionally, not behind `ssr` / `hydrate`), `mf2`'s node in a plain
  resolve carries them and the gap closes; otherwise `mf2 check` needs
  cargo-leptos's `bin-features` / `lib-features`.
- A3's shape (3c) needs nothing more on the web: the prelude and `use
  crate::tr` work in `ssr`, `hydrate` and a split chunk.

## A7 — names and coherence: what was built

* **Where.** The probe branch `p10-a7-names` (worktree
  `.claude/worktrees/agent-aaa2ebae2e252a3e5`, off `2fb7f54`; not merged,
  and kept for B1). On `main`: `probes/p10-names/`, copied from the branch,
  with the branch's crate changes as `probes/p10-names/helper-crates.patch`. Two sessions: the first built and checked the branch on 0.9 and
  was stopped by the session limit; the second finished the measurements,
  the 0.8 line and this record.
  * `b258474` — the six components in per-line helper crates behind a table
    (**v1**: `install()` installs the table, the client switch inside it);
  * `7dbcfd8` — the client switch out of the table, into a `LocaleSwitcher`
    wrapper (**v2**);
  * `9c945ed` — each component installs the table, not `install()` (**v3**,
    the branch's form);
  * `f54cf6d` — `probes/p10-names/coherence.sh`;
  * `6014948` — the scripts behind this record (`probes/p10-names/`
    `demos-0-8.py`, `measure-demo.mjs`, `twiggy-norm-diff.py`, `README.md`).

  Scratch, in that worktree's git-ignored `target/`: `a7-notes.md` (first
  session), `a7-*.log`, `a7-demo-size/` (the demos' A/B: every shipped
  `pkg`/`dist`, `named/` with the twiggy tables), `a7-demo-0-8/` (the 0.8
  copies and their e2e logs).
* **The helper crates.** `crates/mf2-leptos-ui-0-9` owns `src/ui.rs`: the six
  components, written with `view!`, `#[component]` and `#[prop]` as in 1.x.
  `crates/mf2-leptos-ui-0-8/src/ui.rs` is a symlink to it. The 0.8 crate
  binds `extern crate leptos_0_8 as leptos;` — legal there, because that crate
  has no module named `leptos`. Each crate depends on its own line, on
  `mf2-catalog` (for `Dir`) and, for `hydrate` and `csr`, on
  `js-sys` / `wasm-bindgen` / `web-sys`. Its `ssr` / `hydrate` / `csr`
  features are exclusive (`compile_error!`). `forbid(unsafe_code)`; clippy's
  `unwrap_used`, `expect_used`, `indexing_slicing` and `panic` are denied.
  `publish = false` on the branch. `cargo package -p mf2-leptos-ui-0-8 --list
  --allow-dirty` lists `src/ui.rs`, so the link is packaged as a file.
* **No root rename.** Both lines are dependencies under names of their own
  (`leptos_0_9`, `tachys_0_3`, `reactive_graph_0_3`, `leptos_axum_0_9`
  beside the existing `*_0_8`), in `[workspace.dependencies]`. `leptos-mf2`
  reaches the active line through a private module:

  ```rust
  #[cfg(feature = "leptos")]
  mod line {
      #[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
      pub(crate) use ::{leptos_0_8 as leptos, mf2_leptos_ui_0_8 as ui,
          reactive_graph_0_2 as reactive_graph, tachys_0_2 as tachys};
      #[cfg(feature = "leptos-0-9")]
      pub(crate) use ::{leptos_0_9 as leptos, mf2_leptos_ui_0_9 as ui,
          reactive_graph_0_3 as reactive_graph, tachys_0_3 as tachys};
  }
  ```

  Each module imports what it names (`use crate::line::{leptos, tachys};`),
  so the bodies are unchanged. `mf2-axum` does the same
  (`line::{leptos, leptos_axum}`). At `2fb7f54` the crate roots renamed three
  crates (`leptos-mf2`) and two (`mf2-axum`); on the branch the only
  `extern crate … as leptos` is in `mf2-leptos-ui-0-8/src/lib.rs`. The
  integration tests bind the line themselves (`#[cfg(feature =
  "leptos-0-9")] extern crate leptos_0_9 as leptos;`).
* **The function table** (v3). The helper defines `pub struct Table`;
  `leptos-mf2` fills it as a `static` struct literal (its `ssr` fields are
  `cfg`'d, which a constructor's arguments could not be):

  | Field | Type | Filled with (`leptos-mf2`) | Modes |
  |---|---|---|---|
  | `locales` | `fn() -> &'static [(&'static str, Dir)]` | `state::locales` | all |
  | `source_locale` | `fn() -> &'static str` | `state::source_locale` | all |
  | `is_current` | `fn(&str) -> bool` | the active catalog's locale is `tag` (no catalog: the source locale) | all |
  | `locale_query` | `&'static str` | `links::LOCALE_QUERY` | all |
  | `islands_gate` | `&'static str` | `links::ISLANDS_GATE` | all |
  | `preload` | `fn() -> Option<(String, Option<String>)>` | the page catalog's URL and the reader's zone | `ssr` |
  | `catalog_links` | `fn() -> Vec<(&'static str, String)>` | every locale's catalog URL | `ssr` |
  | `catalog_link_rel` | `&'static str` | `links::CATALOG_LINK_REL` | `ssr` |

  * **Outside the table:** the client's switch, a `OnceLock<fn(String)>` in
    the helper (`install_switch`, `hydrate` / `csr` only).
  * **Who installs it:** each of the six, re-exported by `leptos-mf2` as a
    plain `fn X(props: XProps) -> impl IntoView` (not `#[component]`),
    calls `ui::install(&TABLE)` — a `OnceLock<&'static Table>`; later calls
    are a check — and returns `ui::X(props)`. `LocaleSwitcher` also installs
    the switch. The wrappers take the helper's own `*Props` (re-exported),
    so `view!` builds them exactly as it built 1.x's components.
    `install()` installs nothing, as in 1.x.
  * **Reading it:** the helper reads the table through `table() ->
    Option<&'static Table>`. With nothing installed, every component renders
    as with no languages, and nothing panics.
* **`mf2::leptos`**, a stand-in for 2.0's module: `#[cfg(feature =
  "leptos")] pub mod leptos { pub use leptos_mf2::{AlternateLinks,
  CatalogLinks, CatalogPreload, IslandsGate, LocaleOption, LocaleSwitcher,
  Setup, html_lang, install}; }`. `examples/demo-ssr` and `demo-islands`
  import the components from it; `demo-csr` still uses `leptos_mf2::…`.
* **The Ratatui probe** (`leptos-mf2`'s `ratatui` feature,
  `src/ratatui_probe.rs`). For `Tr`, `TrArgs`, `TrRich` and `TrDyn`:
  `From<_>` for `Span<'static>`, `Line<'static>` and `Text<'static>`;
  `Widget`; `Styled<Item = Line<'static>>`; `Display`. The bodies are
  placeholders: what is under test is that the impls coexist with the Leptos
  glue. The `probe-cow`, `probe-fromiter`, `probe-cell` and
  `probe-listitem` features add one forbidden impl each.
* **`probes/p10-names/`** (`cargo check` only; its `README.md` lists every
  file):
  * `naming` — a stand-in `mf2` with `pub mod leptos` and `pub mod axum`
    beside those crates;
  * `naming-n2` — the 0.9 line under its real name;
  * `naming-app` — an application beside both;
  * `coherence` — what a Ratatui application writes, with Leptos on too;
  * the scripts `controls.sh`, `coherence.sh`, `demos-0-8.py`,
    `measure-demo.mjs` and `twiggy-norm-diff.py`.

### Why three forms

Measured with `b5 --view`, below. **v1** put the client switch in the table,
and `install()` — which every application calls — installed it. The
`tr-view` client's fixed part grew **+4,407 B gz**: the switch reaches the
catalog fetch, the registry's walk, the cookie and the address. **v2** moved
the switch to the `LocaleSwitcher` wrapper, and the rest of the table stayed
in `install()`: **+249 B gz**. **v3** installs nothing from `install()`, so
an application that renders none of the six links neither the table nor
what it points at.

### The checks, per Leptos line

Tools: rustc 1.98.1, cargo-leptos 0.3.9, trunk 0.21.13, wasm-bindgen 0.2.128
(CLI; the library 0.2.129 in the demos' locks), wasm-opt 120, twiggy 0.8.0,
Node 23.11, Playwright 1.63.0 (Chromium and Firefox). The worktree's lock:
leptos 0.9.0-beta2 / 0.8.21, tachys 0.3.0-beta3 / 0.2.19, reactive_graph
0.3.0-beta3 / 0.2.15, leptos_axum 0.9.0-beta2 / 0.8.10, ratatui 0.30.2
(the probe's lock), ratatui-core 0.1.2, ratatui-widgets 0.3.2, axum 0.8.9.

**Which commit each ran on.** No tracked file in the worktree changed after
06:46:56, and v3 was committed at 06:47:38 (`git reflog`; file mtimes).
Every run of the first session that finished after 06:47 therefore ran on
v3's content: the 0.9 e2e checks (07:02–07:16), `l6-web` (07:13),
`l7-web` (07:29), `leptos-0-8` (06:50), clippy (06:47) and the tests
(06:48). The base measurements (`size` 05:51, `b5 --view` 05:55:13) finished
before the first edits were applied (05:55:20); until then the helper
crates were untracked files outside the build. The second session re-ran
clippy, the tests, `leptos-0-8`, the naming and the coherence checks at
`f54cf6d`; each was unchanged, and every unit was fresh.

| Check | Leptos 0.9 | Leptos 0.8 | Observed | Command (log, under the worktree's `target/`) |
|---|---|---|---|---|
| clippy `-D warnings`, CI's `leptos-mf2` set (`ssr,mark-fallback-lang` all targets; wasm32: `hydrate`, `hydrate,static-locale`, `csr`, `csr,static-locale,mark-fallback-lang`, `hydrate,fn-datetime,static-locale,mark-fallback-lang`, `csr,fn-datetime`), `mf2-axum` and `mf2` all targets | **PASS** | **PASS** (the xtask's five clippy steps) | every step `Finished`, no warning | 0.9: `bash target/a7-draft/lint.sh` (`a7-lint-head.log`); 0.8: `cargo xtask leptos-0-8` |
| clippy `-D warnings` on each helper crate itself, `ssr`, and `hydrate` / `csr` on wasm32 | **PASS** (`mf2-leptos-ui-0-9`) | **PASS** (`mf2-leptos-ui-0-8`) | six runs, `Finished`, no warning | `cargo clippy -p mf2-leptos-ui-0-{9,8} --features <mode> [--target wasm32-unknown-unknown] -- -D warnings` (`a7-lint-helpers-head.log`) |
| tests: `leptos-mf2` `render` 14 (the switcher's markup: a `GET` form, `select name="lang"` inside its label, the page's option `selected`), `time_zone` 8 (the preload link and its zone), `fallback_lang` 8, `churn` 1; `mf2-axum` 13 + 4 (`time_zone` renders `CatalogPreload`) | **PASS** | **PASS**, the same counts | `test result: ok` for each | 0.9: `bash target/a7-draft/test09.sh` (`a7-test09-head.log`); 0.8: the xtask |
| `cargo xtask leptos-0-8` — the tree's own 0.8 check, the nightly job `leptos-0-8` | — | **PASS** | both lines at once refused: "leptos-mf2: \`leptos-0-8\` is on, and so is the default \`leptos-0-9\`. For Leptos 0.8, every dependency on leptos-mf2 and mf2-axum needs \`default-features = false\` beside \`features = ["leptos-0-8"]\`."; the clippy and test steps above; conformance `layers` 4 and `l6` 3 | `CARGO_BUILD_JOBS=2 cargo xtask leptos-0-8` at `f54cf6d` (`a7-leptos-0-8-head.log`; 7.5 s, every unit fresh from `a7-leptos-0-8-v3.log`) |
| e2e `demo` | **210/210** (first session) | **210/210** | Chromium 105, Firefox 105 each: the preload link and the catalog from it, hydration changes no text, a live switch reaches text, attributes, `<title>` and `<html lang dir>`, the path-prefix switcher with and without the wasm, no message text in the client | `node run.mjs demo --base-url http://127.0.0.1:3702 --browser chromium,firefox`: 0.9 `a7-e2e-demo-09.log`; 0.8 `a7-demo-0-8/e2e-demo-08.log` |
| e2e `lazy` (`--split`) | **74/74** | **74/74** | the chunk fetched on navigation only; a direct `/lazy` load hydrates through `hydrate_lazy` and switches live | `node run.mjs lazy …`: `a7-e2e-lazy-09.log`; `a7-demo-0-8/e2e-lazy-08.log` |
| e2e `islands` | **58/58** | **58/58** | the gate the first island (`mf2_islands_gate`, the table's name), the switcher a `GET` form and not an island, no island hydrates before the catalog; the control without the gate traps | `node run.mjs islands --base-url http://127.0.0.1:3704 …`: `a7-e2e-islands-09.log`; `a7-demo-0-8/e2e-islands-08.log` |
| e2e `csr` | **98/98** | **98/98** | the 0.8 run served the 0.8 copy's `dist` (it fetched `demo-csr-7cebfc3c1dabd481_bg.wasm`, not the 0.9 build's `…44f34a8b…`) | `node run.mjs csr …`: `a7-e2e-csr-09.log`; `a7-demo-0-8/e2e-csr-08.log` |
| e2e `zone` | **40/40** (second session) | **40/40** | in `America/New_York` and `Asia/Kolkata`: a reload is served in the reader's zone and the preload link states it (the table's `preload`) | `node run.mjs zone --base-url http://127.0.0.1:3702 …`: `a7-e2e-zone-09.log`; `a7-demo-0-8/e2e-zone-08.log` |
| e2e `a11y` (28 pages of the three demos) | **720/720** | **720/720** | axe clean, the switcher's keyboard behaviour, and every negative control failing as designed | `MF2_ISLANDS_URL=http://127.0.0.1:3704 node run.mjs a11y --base-url http://127.0.0.1:3702 …`: `a7-e2e-a11y-09.log`; `a7-demo-0-8/e2e-a11y-08.log` |
| `cargo xtask l6-web` | **20/20** | not run: no 0.8 mode (L6 on 0.8 runs natively inside `leptos-0-8`) | "every engine hydrated the page and switched locale" | `a7-l6-web.log` |
| `cargo xtask l7-web` | **34/34** | not run: no 0.8 mode | L7 444/444, L7c 444/444, L7d 325/444 (+119 documented degradations), L7cd 325/444 (+119); "the ledger's L7 columns hold in chromium, firefox" | `a7-l7-web.log` |
| no Leptos procedural macro outside the six | **PASS** | **PASS** (one source) | 6 `#[component]`, 7 `view!`, 3 `#[prop]`, all in `mf2-leptos-ui-0-9/src/ui.rs`; none in `leptos-mf2`, `mf2-axum` or `mf2`. At `2fb7f54` the same 6 / 7 / 3 were in `leptos-mf2/src/components.rs` | `rg` over the four crates' `src/`, comment lines excluded; `git grep … 2fb7f54` |
| naming (`probes/p10-names`) | **PASS** | **PASS** | `cargo check --workspace` (`naming` with `leptos-0-9,axum`, `naming-n2`, `naming-app`, `coherence`) and `-p naming --no-default-features --features leptos-0-8,axum`: `Finished`; N1–N5 below | `bash probes/p10-names/controls.sh` (`a7-naming-head.log`, `a7-naming-errors-head.log`) |
| coherence, `leptos` and `ratatui` both on | **PASS** | not run (the impls do not depend on the line) | `leptos-mf2 --no-default-features --features ratatui`; `--features ssr,ratatui`; `--features hydrate,ratatui --target wasm32-unknown-unknown`; `coherence` for `ssr` and for `hydrate`: `Finished`. The four controls fail with E0119 (below) | `bash probes/p10-names/coherence.sh` (`a7-coherence-head.log`, `a7-coherence-errors-head.log`) |

**The 0.8 line, exactly.**
* **The tree's own check.** The tree builds the demos on 0.9 only. Its 0.8
  check is `cargo xtask leptos-0-8` (`.forgejo/workflows/nightly.yml`, job
  `leptos-0-8`; phase 8 A0). The one 0.8 application build in the tree is
  `cargo xtask fluent-ab`: its mf2 side renders `CatalogPreload` and
  `CatalogLinks`. It is a snapshot benchmark that builds the 1,860-site
  reference application several times, and was not run.
* **The first session's failure is not A7's.** Its `demo-ssr` 0.8 copy
  (`target/a7-draft/demo08.py`) fails identically at `2fb7f54`:
  `CARGO_BUILD_JOBS=1 cargo leptos build --split` in the copy
  (`a7-demo-0-8/demo-ssr-08-base-orig.log`) gives "package \`demo-ssr\`
  depends on \`leptos\` with feature \`lazy\` but \`leptos\` does not have
  that feature". `cargo info leptos@0.8.21` lists no `lazy` feature. The
  demo turns it on for 0.9's lazy-route hydration.
* **The browser checks on 0.8.** `python3 probes/p10-names/demos-0-8.py head`
  copies the three demos with:
  * the Leptos crates at 0.8, and `leptos-mf2` / `mf2-axum` with
    `default-features = false, features = ["leptos-0-8", …]`;
  * `demo-ssr` without `leptos/lazy`;
  * `demo-csr`'s trunk hook pointed at the worktree's manifest;
  * a copy of the harness beside them.

  Then `CARGO_BUILD_JOBS=1 cargo leptos build [--split]` or `trunk build`,
  each server on its own port, and the checks from the copied harness.
  `cargo tree` shows leptos 0.8.21, tachys 0.2.19, reactive_graph 0.2.15,
  leptos_axum 0.8.10, leptos_router 0.8.16, leptos_meta 0.8.7 and
  `mf2-leptos-ui-0-8`, with no 0.9 crate, in each server and client build.
  All six checks passed, as in the table.

### Sizes

**B1, the size gate** (`tr` against `idlit` and `dummy`, at 1,860 and 3,720
sites). The workloads were generated once at base, and every form was
measured in the same tree with the same per-app locks:
* base: `CARGO_BUILD_JOBS=2 cargo xtask size --out target/size-base`;
* each form after it: the same, with `--keep`.

| Form | `tr` opt gz, 1,860 / 3,720 | B1 fixed | B5 a site | whole app, 1,860 sites | Log |
|---|---:|---:|---:|---:|---|
| base `2fb7f54` | 693,671 / 1,203,351 | **26,402 B gz** | 8.3 | 41,861 | `a7-size-base.final.log` |
| v1 | identical | 26,402 | 8.3 | 41,861 | `a7-size-v1.log` |
| v2 | identical | 26,402 | 8.3 | 41,861 | `a7-size-v2.log` |
| v3 | identical | **26,402** | 8.3 | 41,861 | `a7-size-v3.log` (second session: `leptos-mf2` and both `tr` apps rebuilt at 13:02–13:05; the apps' lock `a1f1bcb3…` unchanged) |

Every stage is identical in all four runs: bindgen gz, opt raw and opt gz.
The `tr` template has no Leptos layer. In its app, `cargo tree --locked
--offline -e features -i leptos-mf2 --target wasm32-unknown-unknown
--no-default-features --features hydrate` shows `leptos-mf2` with no
feature on, and no `mf2-leptos-ui-*` crate in the graph. So v3 did not
strictly need the run; it was run to have the figure.

**`b5 --view`** (`tr-view`, where a description renders itself, against
`idlit-view`). Base: `CARGO_BUILD_JOBS=2 cargo xtask b5 --view --out
target/b5v`; each form after it: the same, with `--keep` (the logs say
"reusing"). `idlit-view` (528,251 / 892,759) and `dummy` were identical in
all four runs.

| Form | `tr-view` opt gz, 1,860 / 3,720 | opt raw Δ, both scales | fixed | Δ fixed | marginal a site | Δ marginal | Log |
|---|---:|---:|---:|---:|---:|---:|---|
| base | 572,533 / 956,400 | — | 24,923 | — | 10.408 | — | `a7-b5v-base.final.log` |
| v1 | 577,182 / 961,291 | +11,102 / +11,094 | 29,330 | **+4,407** | 10.538 | +0.130 | `a7-b5v-v1.log` |
| v2 | 572,817 / 956,719 | +898 / +890 | 25,172 | **+249** | 10.427 | +0.019 | `a7-b5v-v2.log` |
| v3 | 572,543 / 956,470 | +93 / +93 | 24,873 | **−50** | 10.440 | +0.032 | `a7-b5v-v3.log` |

Fixed and marginal are the xtask's difference of differences. The marginals
are recomputed from the logged sizes; the xtask prints them to one decimal
(10.4 / 10.5 / 10.4 / 10.4). v3's whole-app change is +10 B gz at 1,860
sites and +70 B gz at 3,720. **Against the gate (±64 B gz fixed,
±0.2 B a site), v3 meets it in the size workloads.** Neither workload
renders any of the six components.

**The demos: the table in applications that use the components.** Measured
in the one worktree, with each demo's own `Cargo.lock` kept. The steps:
1. At `f54cf6d`: `CARGO_BUILD_JOBS=1 cargo leptos build --release
   [--split] --frontend-only --cargo-offline` in `examples/demo-ssr`
   (`--split`, the README's size command) and `examples/demo-islands`, and
   `CARGO_BUILD_JOBS=1 trunk build --release` in `examples/demo-csr`.
2. `git switch --detach 2fb7f54`, and the same.
3. `git switch p10-a7-names`, the v3 locks restored, and the same again.

The base builds changed only the two helper crates' lock entries
(`a7-demo-size/locks/*.base-vs-v3.diff`). Every registry package kept its
version, and the v3 locks were restored byte for byte (sha256 `f2a99bf9…`,
`c1a6f667…`, `7b92fa28…`). The second v3 build reproduced the first:
demo-islands and demo-csr byte for byte. In demo-ssr the `.wasm` files and
`demo_ssr.js` were identical; only the split loader `__wasm_split.js`
differed, in the order in which it lists two names. Measured with `node
probes/p10-names/measure-demo.mjs <pkg|dist>`: gzip −9 and brotli q11 via
Node's zlib, as phase 7 measured demo-ssr (`a7-demo-size/*.md`).

| Demo (mode) | Components on the client | wasm, base → v3 (raw / gz / br) | Δ wasm | Δ every shipped file (gz / br) |
|---|---|---|---:|---:|
| demo-ssr (`hydrate`, `--split`) | 3 `LocaleSwitcher`, 6 `LocaleOption` (the shell's `CatalogPreload` and `CatalogLinks` render on the server only) | 758,046 / 317,580 / 252,342 → 759,440 / 318,039 / 252,795 | +1,394 / **+459** / +453 | **+454** / +464 (the lazy chunk 0 raw, −4 gz; `demo_ssr.js` 0 raw) |
| demo-csr (`csr`, trunk) | 1 `LocaleSwitcher`, 3 `LocaleOption` | 207,692 / 91,293 / 77,722 → 208,020 / 91,423 / 77,772 | +328 / **+130** / +50 | **+127** / +47 |
| demo-islands (islands) | none (the switcher is not an island) | 197,382 / 85,357 / 72,289 → 197,371 / 85,349 / 72,328 | −11 / −8 / +39 | −9 / +41 |

**Where demo-ssr's bytes are.** The same client was built by cargo with its
symbol names kept, at base and at v3:
`CARGO_PROFILE_WASM_RELEASE_STRIP=none cargo build --lib --target
wasm32-unknown-unknown --profile wasm-release --no-default-features
--features hydrate --target-dir target/a7-named/demo-ssr --offline`.
Then `twiggy top -n 1000000 -f json` on each, joined with `python3
probes/p10-names/twiggy-norm-diff.py`, which drops crate hashes and closure
numbers so that renames cancel (`a7-demo-size/named/*.norm-diff.txt`). The
net, before wasm-bindgen and wasm-opt:
* **code +1,339 B**, `.rodata` +344 B, types and imports +8 B (the name
  section, +3,797 B, does not ship);
* items new in v3:
  * `OnceLock<&Table>`: `initialize` 105 B and `Once::call` 176 B;
  * `OnceLock<fn(String)>`: the same, 105 B and 176 B;
  * `is_current`: 171 B, linked through the table (`html_lang` stays, since
    the page uses it);
  * `switch`: 110 B (its 1,137 B async body moved from `switch_on_submit`);
  * `install` 75 B, `install_switch` 75 B, `install_table` 16 B;
  * the wrappers' `FnOnce::call_once` shims: 64 B and 60 B;
  * `__component_locale_switcher` +44 B, `LocaleSwitcher` +32 B,
    `LocaleOption` +20 B;
* everything else nets to about zero: inlining moved, and identical-code
  folding kept a different name.

Part of `.rodata` is a second absolute source path: the shipped wasm
carries `…/crates/mf2-leptos-ui-0-9/src/ui.rs` beside
`…/crates/leptos-mf2/src/components.rs` (`strings`). demo-csr, the same
way: code +353 B, `.rodata` +40 B. `html_lang` −181 B (no longer linked),
`is_current` +145, `switch` +96, `install_table` +77.

### Naming: N1–N5 (rustc 1.98.1)

| # | What | Observed |
|---|---|---|
| N1 | 1.x's root rename, `extern crate leptos_0_8 as leptos;`, beside `pub mod leptos` | **E0260** "the name \`leptos\` is defined multiple times … previous definition of the module \`leptos\` here … \`leptos\` reimported here" (`naming/src/lib.rs:76`) — the reason 0.8 cannot keep the root rename under `mf2::leptos` |
| N2 | the 0.9 line under its real name `leptos`, `pub mod leptos`, and a bare `use leptos::prelude::RwSignal;` at the crate root | **E0432** "unresolved import \`leptos::prelude\` … could not find \`prelude\` in \`leptos\`". The local module wins; there is no ambiguity error. In the same crate, `#[::leptos::component]` and `::leptos::view!` at the root compile: only 0.8's `extern crate` binding clashed |
| N3 | `use axum::Router;` at the crate root beside `pub mod axum` | **E0603** "struct import \`Router\` is private": it resolved to `crate::axum`, whose private `use axum::Router` it names |
| N4 | `axum::Router` as a type and as an expression path at the root | **E0603** twice, the same |
| N5 | an application: `use naming::leptos;` (the module itself), then `use leptos::prelude::RwSignal as _;` beside its own `leptos` dependency | **E0432** "unresolved import \`leptos::prelude::RwSignal\` … no \`RwSignal\` in \`leptos::prelude\`"; "cannot find macro \`view\` in this scope" ×2; **E0405** "cannot find trait \`IntoView\`" ×2; and the application's own `use leptos::prelude::*` is reported unused. Importing the module shadows the application's `leptos` crate in every bare path of that module |

Positive, in the same probe: inside `pub mod axum`, `use axum::Router`
names the crate (a module is not in its own scope); `::axum::Router` at the
root names the crate; an application naming `naming::leptos::…` /
`naming::axum::…` by path, beside its own `leptos` and `axum`, compiles.
The comments in `naming-n2` and `naming-app` still expect E0659 for N2, N3
and N5; what rustc 1.98.1 reports is the table above.

**The rules, for 19:**
* inside `mf2`, reach the Leptos lines through internal aliases, never a
  crate-root `extern crate … as leptos`;
* inside `mf2`, name the `axum` crate as `::axum::…` wherever the `axum`
  module is in scope at the crate root;
* the book tells applications to import items from `mf2::leptos` or to write
  full paths, never `use mf2::leptos;` in a module that uses the `leptos`
  crate.

### Coherence: the rules and their errors

The positive set compiles with `leptos` and `ratatui` both on (`ssr`, and
`hydrate` on wasm32) and with `ratatui` alone. `coherence/src/lib.rs`:
* `Paragraph::new(t)` and `Block::bordered().title(t)`;
* `Row::new([t, t])`, `Cell::from(t)`, `ListItem::new(t)`,
  `List::new([t, t])` and `Tabs::new([t, t])`;
* `t.bold()` and `t.fg(Color::Yellow).italic()` through `Styled`;
* `[t, t].into_iter().collect::<Line>()`, `Span::from(t)`, `Text::from(r)`
  and `t.render(area, buf)`;
* `format!("{t} {a}")`, and `t.to_string()` choosing the inherent method;
* `view! { <p title=t>{t}</p> }` in the same scope.

The four controls (`cargo check -p coherence --features probe-*`):
* **Never `From<Tr> for Cow<str>`.** `probe-cow` gives **E0119**
  "conflicting implementations of trait \`From<tr::Tr>\` for type
  \`ratatui_core::text::Span<'_>\`". It is reported at our `From<Tr> for
  Span<'static>`: "conflicting implementation in crate \`ratatui_core\`: -
  impl<'a, T> From<T> for ratatui_core::text::Span<'a> where T: Into<Cow<'a,
  str>>;".
* **No `FromIterator<Tr> for Line`.** `probe-fromiter` gives **E0119**
  "conflicting implementations of trait \`FromIterator<tr::Tr>\` for type
  \`ratatui_core::text::Line<'_>\` … impl<'a, T> FromIterator<T> for
  ratatui_core::text::Line<'a> where T: Into<ratatui_core::text::Span<'a>>;".
  `Line` already collects descriptions through that blanket.
* **`Cell` and `ListItem` come through their blankets over `Into<Text>`.**
  * `probe-cell` gives **E0119** "conflicting implementations of trait
    \`From<tr::Tr>\` for type \`ratatui_widgets::table::Cell<'_>\` … impl<'a,
    T> From<T> for ratatui_widgets::table::Cell<'a> where T:
    Into<ratatui_core::text::Text<'a>>;".
  * `probe-listitem` gives **E0119** "… for type \`ListItem<'_>\` …
    impl<'a, T> From<T> for ListItem<'a> where T:
    Into<ratatui_core::text::Text<'a>>;".

### Verdict

**Done when** ("the module and crate names confirmed; the coherence rules
for 19"):
* **Names: confirmed.**
  * `mf2::leptos` works with no root rename, on both lines.
  * `mf2-leptos-ui-0-9` and `-0-8` build and render on their lines.
  * `pub mod axum` sits beside `axum`, with one rule at the crate root.
  * No Leptos procedural macro is outside the six (item 2).
* **The coherence rules: confirmed**, each with its error (item 4).
* **Hydration: not broken on either line.** The six browser checks of the
  demos are green on 0.9 and on 0.8 (on 0.8 through copies, since the tree
  builds the demos on 0.9 only). `l6-web` and `l7-web` are green on 0.9;
  they have no 0.8 mode.
* **The table's cost (item 1).**
  * **In the gate's own measurements, v3 meets ±64 B gz:** B1 is unchanged
    at 26,402 B gz, and `b5 --view` moves −50 B gz fixed, +0.03 B a site.
  * **In applications that render the switcher on the client, v3 costs more
    than ±64 B gz:** +130 B gz in demo-csr and +459 B gz in demo-ssr (the
    wasm; +127 and +454 over every shipped file). An islands application
    whose switcher stays on the server pays nothing (−8).
* **The fallback.** Question 13's is "back to the owner with the other two
  options". Under the coordinator's rule, it applies **if applications that
  render the switcher count as a configuration that matters**. That call is
  the coordinator's; the owner was not asked.

### What B1 should reuse from the branch, and what to change (interpretation, brief)

* **Reuse:**
  * the helpers' shape: one `ui.rs`, a link in the 0.8 crate, and
    `extern crate leptos_0_8 as leptos;` only there;
  * the renamed line dependencies, and the private `line` alias module in the
    Leptos and Axum layers;
  * wrappers that take the helper's `*Props`, so `view!` is unchanged;
  * nothing installed by `install()`: v1 and v2 show what that costs every
    application;
  * `table()` as an `Option`, so a missing table cannot panic;
  * the tests' line binding;
  * `probes/p10-names/demos-0-8.py` for B1's "e2e on both Leptos lines": the
    tree has no other way to run the browser checks on 0.8.
* **Change — the install mechanism, if the demos' figures count.** About
  1,230 B of demo-ssr's +1,339 B of code are that mechanism:
  * two `std::sync::OnceLock`s (with `Once`);
  * the install calls and the wrappers' shims;
  * `is_current`, linked because a table entry points at it;
  * the switch behind a function pointer.

  One untested option would remove all of it: static dispatch. The
  helper declares a trait whose associated functions are the table's
  entries, `leptos-mf2` implements it for a zero-sized type, and the
  components are generic over it. There would be no `OnceLock` and no
  function pointer, and nothing would be linked that a page does not call.
  Its cost is not measured.
* **Measure applications that use the components.** B1's size gate should
  cover them, since `size` and `b5 --view` render none of the six:
  demo-ssr, demo-csr and demo-islands, with `measure-demo.mjs`.
* **Leave out of B1:**
  * the `ratatui` / `probe-*` features, `ratatui_probe.rs` and the
    `ratatui-widgets` workspace dependency (C5 builds the real impls);
  * `publish = false` on the helpers;
  * the demos' import change (the work order keeps the examples on the shims
    until C8 and D6).
* **B1 decides:**
  * whether the `*Props` types, which the wrappers' signatures expose, belong
    in the per-mode `api.txt`;
  * what a checkout without symlink support (`core.symlinks=false`) gets for
    the 0.8 helper's `ui.rs`. `cargo package` follows the link.

## A9 — what `Display` / `Debug` cost the browser's wasm: what was built

* **Where.** A5's worktree (`.claude/worktrees/agent-a7b6f4df78ce56a04`), on
  the probe branch `p10-a5-display` at **`70e0b27`** (2026-09-28,
  14:09–16:05): one tree, and one lock per application. Nothing was
  committed there; every change was a working-tree edit, taken out after its
  build, and the worktree is clean. On `main`: `probes/p10-display-cost/`,
  the scripts, the library variants as patches (`lib-*.patch`) and a README.
  `all.sh STEP` reruns each step in the order the figures came from. The
  outputs stay in the worktree's git-ignored `target/a9/`: `results.tsv`
  (every figure), `log.txt` (every build), `out/` (each case's shipped files
  and names-kept build), `check/` and `dev-check/` (the type-checks and the
  debug-profile builds).
* **The cases** (`case.py`): one statement after a fixed line of the
  client's entry point, so that every build of the client reaches it; `x` is
  a black-boxed description:
  * `base`: nothing;
  * `display-T`: `black_box(format!("{}", x))`;
  * `debug-T`: `black_box(format!("{:?}", x))`;
  * `both-T`: `black_box(format!("{} {:?}", x, x))`;
  * `tostring-T`: `black_box(x.to_string())`, the inherent, fmt-free method.
    It is the control: the description built and its text path linked, with
    no `core::fmt` of ours;
  * `control`: A5's positive control, `format!("{save} {items:?}")` (a
    `Tr`, a `TrArgs`);
  * T is `tr`, `trargs`, `trrich` or `trdyn`.
* **The clients** (`run.sh`), each built as it ships:
  * **fixture**: `mf2-i18n-client`, as `cargo xtask b12-generated` builds
    its A (`hydrate,fn-number`, `wasm-release`), then `wasm-opt -Oz`. Its
    `leptos-mf2` has no Leptos mode, so the variant's `Display` is the
    stand-in that writes nothing: there `{}` measures `format!`'s own
    machinery, not the text path;
  * **`tr`** at 1,860 sites: the same Leptos-free core, in a Leptos
    application, built as `cargo xtask size` builds it (`wasm-bindgen`,
    `wasm-opt -Oz`) in its own target directory;
  * **`tr-view`** at 1,860 sites: the Leptos layer (`hydrate`), where
    `Display` reads the page's catalog and pads; built as `b5 --view`
    builds it;
  * **the demos**, as A7 measured them: `cargo leptos build --release
    --split --frontend-only` (demo-ssr), `--frontend-only` (demo-islands),
    `trunk build --release` (demo-csr). Every shipped `.wasm` and `.js` is
    counted.
* **What is measured** (`measure.py`): raw bytes, `gzip -9 -n` and
  `brotli -q 11` (the CLIs, as in A5's compressor table), and a module's
  code and data sections. `gzip -9 -n` reads a few bytes away from the
  gate's flate2. The demos' Node figures (`measure-demo.mjs`) are kept
  beside each build. **A figure is shipped bytes.**
* **The library variants** (`libvar.sh apply NAME`, from `lib-NAME.patch`):
  `v1x` (the crate at `2fb7f54`), `s1-writestr`, `s2-debug`, `r1`, `r1d`,
  `r2`, `r2d`, `r3`, each described below.
* **The A/B is valid** (checked):
  * each client's `base` was built first and last: byte-identical every
    time (the fixture's three times). Six fixture cases, built twice, agree
    to the byte;
  * the bases are A5's: `tr-view`'s `opt.wasm` is byte-identical to A5's
    variant measurement; `tr` is 2,419,575 B, as A5's; the fixture is
    365,001 B raw, 328,579 after `wasm-opt`, as A5's plain build. `v1x`
    reproduces A5's A/B in `tr-view` (−3 B: 1,890,880);
  * A5's control reproduces: +16,620 B gz, against A5's +16,604. A5's
    `format!` sat in the fixture's existing block, this one in a block of
    its own; the optimised raw size is 364,404, against A5's 364,332;
  * **one lock per application.** `tr` kept `a1f1bcb3…` (A7's size runs'
    lock) and `tr-view` `09effa29…`. The demos got A7's locks (`f2a99bf9…`,
    `7b92fa28…`, `c1a6f667…`), which cargo trimmed of the two helper
    crates' entries (24 lines each; every registry package kept its
    version), the same change A7's base builds made;
  * `results.tsv` has one line twice (demo-csr's `base` at 14:40:52: a
    measurement repeated, not a build, after a script was edited while it
    ran). One row was deleted: R3's `display-tr` in demo-ssr, a case that
    does not compile, which `run.sh` measured from an empty directory
    before it learned to stop on a failed build. No other build failed
    (every run and build log was searched for `error`).
* **The names-kept builds** (for twiggy and the check): the same clients
  with `strip = false`, then `wasm-opt --strip-dwarf -Oz --debuginfo`.
  **Without `--strip-dwarf`, the standard library's DWARF stays in, and
  binaryen then emits +557 B of code** in the fixture (`eq-test.sh`,
  `sections.py`: 67,552 against 66,995). That is why A5's named builds read
  −160 B where the measured ones read −79. With `--strip-dwarf`, the code
  section is the shipped one's size to the byte, in another function order
  (gzip 2 B apart).

### The split

`python3 probes/p10-display-cost/tables.py split CLIENT`: Δ gz over `base`,
raw in brackets.

**The fixture** (the Leptos-free core; `Display` is the stand-in):

| form | `Tr` | `TrArgs` | `TrRich` | `TrDyn` |
|---|---:|---:|---:|---:|
| `{}` | +321 (+591) | +341 (+660) | +586 (+1,038) | +550 (+1,033) |
| `{:?}` | +1,647 (+3,263) | **+16,458 (+35,515)** | +16,765 (+36,116) | +16,670 (+35,959) |
| `{}` and `{:?}` | +1,777 (+3,550) | +16,609 (+35,801) | +16,932 (+36,421) | +16,804 (+36,241) |
| `.to_string()` | +49 (+104) | +59 (+161) | +310 (+534) | +232 (+518) |
| **`{}` over `.to_string()`** | **+272 (+487)** | **+282 (+499)** | **+276 (+504)** | **+318 (+515)** |

**`tr-view`** (the Leptos layer; `Display` is the text path, padded):

| form | `Tr` | `TrArgs` | `TrRich` | `TrDyn` |
|---|---:|---:|---:|---:|
| `{}` | +215 (+523) | +238 (+549) | +1,095 (+2,926) | +531 (+1,139) |
| `{:?}` | +87 (+331) | **+11,784 (+27,114)** | +12,715 (+29,503) | +12,067 (+27,811) |
| `{}` and `{:?}` | +264 (+797) | +11,974 (+27,580) | +12,776 (+29,981) | +12,300 (+28,302) |
| `.to_string()` | +62 (+51) | +90 (+74) | +931 (+2,461) | +455 (+1,045) |
| **`{}` over `.to_string()`** | **+153 (+472)** | **+148 (+475)** | **+164 (+465)** | **+76 (+94)** |

`TrRich` and `TrDyn` cost more in every row because building one links what
the page did not have (the rich description, the names); `.to_string()`
pays that too, so the last row is what `Display` itself adds.

### In applications

`tables.py apps`: Δ gz of every shipped file against the client's `base`,
library `a5`:

| client | `base` raw / gz | `{}` `Tr` | `{}` over `.to_string()`, `Tr` / `TrArgs` | `{:?}` `Tr` | `{:?}` `TrArgs` | A5's control |
|---|---:|---:|---:|---:|---:|---:|
| fixture | 328,579 / 87,371 | +321 | +272 / +282 | +1,647 | +16,458 | +16,620 |
| `tr` | 2,419,575 / 688,943 | +103 | +60 / — | +223 | +12,966 | +13,085 |
| `tr-view` | 1,890,883 / 569,373 | +215 | +153 / +148 | +87 | +11,784 | +12,005 |
| demo-csr | 248,281 / 98,942 | +241 | +244 / +35 | +361 | +14,797 | +15,015 |
| demo-ssr | 808,799 / 334,826 | +235 | +217 / +367 | +66 | +11,903 | +12,052 |
| demo-islands | 213,233 / 90,398 | +938 | +373 / +594 | +637 | +12,308 | +12,896 |

(`tr` ran `base`, `control`, `display-tr`, `tostring-tr`, `debug-tr` and
`debug-trargs` only.)

* **`{}` costs a few hundred bytes gz, once per description type, in
  every client.** Over `.to_string()` of the same description it is
  +35 to +594 B gz. In demo-ssr it is exactly one function,
  `<Tr as Display>::fmt` (444 B raw), plus the call site (`attribute.py`
  over the names-kept builds). `core::fmt::write`, `Formatter::pad` and
  `alloc::fmt::format` are already in every client: the standard library's
  panic machinery puts them there, and Leptos and wasm-bindgen use them.
  demo-islands pays more over its `base` (+938) because its hydrate entry
  point otherwise links no text path; `.to_string()` alone costs +565 there.
* **`{:?}` on a description with arguments costs 11.8–16.5 KB gz in every
  client, the applications included**: `tr-view` +11,784, demo-ssr
  +11,903, demo-islands +12,308, `tr` +12,966, demo-csr +14,797, the
  fixture +16,458. No application links float formatting on its own.
* **`{:?}` on a bare `Tr`** (1.x's own `Debug`) costs +66 to +637 B gz in
  the applications, which already link the builders and integer
  formatting, and +1,647 in the bare fixture.
* **A5's +16.6 KB was the `{:?}`.** The control is within 0.2 KB of
  `debug-trargs` in every client.
* **A5's variant against 1.x, in the demos** (same tree and lock, `v1x`):
  demo-ssr −4 B raw, demo-islands −1, demo-csr +1. Against A7's `2fb7f54`
  figures, from another tree, demo-ssr's main module reads +117 B raw: the
  tree, not the variant.

### What the bytes are

`python3 probes/p10-display-cost/attribute.py BASE CASE` over the
names-kept builds: `twiggy top -f json` of both, joined on names with crate
hashes, binaryen's `.N` suffixes and closure numbers dropped, each item put
in the first group whose pattern matches it (the patterns are in the
script). The fixture, raw bytes, names excluded:

| group | `{:?}` `TrArgs` | `{:?}` `Tr` | `{}` `Tr` over `.to_string()` |
|---|---:|---:|---:|
| **float formatting** (`<f64 as Debug>` 6,051, Dragon 3,374 + `mul_pow10` 593, Grisu 1,671, `bignum` 640 + …, `__multi3`) | **+13,605** | | |
| **data** (tables and text; segments renumber, so taken as one) | **+7,371** | +514 | +222 |
| integers (`pad_integral`, `<&u64/&u32/&u16/i64/u8 as Debug>`, hex) | +4,306 | +1,070 | |
| ours (`Text` 778, `&ArgValue` 476, `&ArgList` 433, `Arc<DateTimeValue>` 279, `Time` 255, `Date`, `MsgId`, `TrArgs`, `Option<Text>`) | +2,845 | +595 | +6 |
| string escaping (`char::escape_debug_ext` 2,042; the Unicode tables are data) | +2,186 | | |
| `Debug` builders (`PadAdapter::write_str` 561, `DebugStruct::field` 334, `DebugTuple::field` 258, …) | +1,658 | +659 | |
| **panic paths** (`str::slice_error_fail` 1,083, `slice_index_fail` 283, `panic_bounds_check` 82, `Range<usize>`'s `Debug`) | **+1,470** | | |
| `Formatter` core (`pad_formatted_parts` 600, `write_formatted_parts` 398, …) | +1,040 | +42 | +42 |
| the call site (`main`) | +264 | +272 | +168 |
| other | +809 | +122 | +66 |
| **total** | **+35,608** | **+3,274** | **+498** |

* **About 2.3 KB of the new data is text** (the printable runs of the data
  section): core's panic and assertion messages from the float code
  (`assertion failed: d.mant + d.plus < (1 << 61)`, `assertion failed:
  buf[0] > b'0'`, …), the absolute `/rustc/<hash>/library/core/src/num/imp/
  flt2dec/…` paths they report, `str` slicing's panic messages, our
  variants' and fields' names, and `alloc::fmt`'s "a formatting trait
  implementation returned an error when the underlying stream did not".
  The rest is binary: Grisu's cached powers, Dragon's, `escape_debug`'s
  Unicode tables, the two-digit table. **So `{:?}` on a description with
  arguments brings core's panic paths into the client too**; B12 forbids
  them in our own code, and the float formatter asserts.
* **`{}` over `.to_string()`** in the fixture is the call site, `format!`'s
  pieces, `format_inner`'s panic text and 42 B of `Formatter`:
  `core::fmt::write` and `Formatter::pad` were there already (A5 counted 32
  fmt items in its base).
* **In an application (demo-ssr):** `{:?}` on a `TrArgs` is +27,003 B raw,
  and float formatting is 18,163 of it (`<f64 as Debug>` 8,032, Dragon
  5,302 + `mul_pow10` 1,117, Grisu 1,611, `bignum` 1,259). Then come data
  3,838, ours 1,546, integers 1,189, `Formatter` 1,138, the builders 842 and
  `<str as Debug>` 761; panic paths only +17, since the application has
  them already. `{}` over `.to_string()` is `<Tr as Display>::fmt` (444 B)
  and the call site, and nothing else.

### Ways to shrink it

`tables.py variants --libs s1-writestr,s2-debug …`: each variant against
`a5`, the same client and case, Δ raw / Δ gz:

| client | case | `a5` raw / gz | S1 | S2 |
|---|---|---:|---:|---:|
| fixture | `base` | 328,579 / 87,371 | — | +0 / +0 |
| fixture | `debug-tr` | 331,842 / 89,018 | — | −2,484 / −1,182 |
| fixture | `debug-trargs` | 364,094 / 103,829 | — | **−33,133 / −15,226** |
| fixture | `control` | 364,404 / 103,991 | — | −33,131 / −15,210 |
| `tr-view` | `base` | 1,890,883 / 569,373 | +0 / +0 | −3 / −12 |
| `tr-view` | `display-tr` / `-trargs` | 1,891,406 / 569,588 | +0 / +0; +0 / +1 | — |
| `tr-view` | `debug-tr` | 1,891,214 / 569,460 | — | −71 / +102 |
| `tr-view` | `debug-trargs` | 1,917,997 / 581,157 | — | **−25,493 / −10,900** |
| demo-csr | `base` | 248,281 / 98,942 | +0 / +0 | −2 / +0 |
| demo-csr | `display-tr` / `-trargs` | 249,169 / 99,183 | +44 / +9; +44 / +12 | — |
| demo-csr | `debug-tr` | 249,165 / 99,303 | — | −309 / −132 |
| demo-csr | `debug-trargs` | 279,917 / 113,739 | — | **−29,294 / −13,733** |
| demo-ssr | `base` | 808,799 / 334,826 | +0 / −1 | −7 / +0 |
| demo-ssr | `display-tr` / `-trargs` | 809,314 / 335,061 | +0 / +0; +0 / +1 | — |
| demo-ssr | `debug-tr` | 809,039 / 334,892 | — | +1 / +86 |
| demo-ssr | `debug-trargs` | 835,754 / 346,729 | — | **−25,374 / −10,942** |
| demo-ssr | `silent-debug` (below) | 836,700 / 347,183 | — | −25,541 / −11,081 |

* **S1, `Display` through `write_str` in place of `Formatter::pad`
  (`lib-s1-writestr.patch`, one line): nothing to gain.** 0 B in `tr-view`
  and demo-ssr, +44 B raw in demo-csr. `pad` is already in every
  application, so `write_str` would only drop `{:<12}`'s padding.
* **S2, every `Debug` through `write_str` (`lib-s2-debug.patch`: a new
  `debug.rs` of digit, float, quote and id writers, and the impls rewritten;
  no builder): −92 % to −93 %.** `{:?}` on a `TrArgs` falls to +884 B gz
  (`tr-view`), +961 (demo-ssr), +1,064 (demo-csr) and +1,232 (the fixture),
  from 11.8–16.5 KB. In the fixture it is 2,393 B raw: our writer 1,576
  (the argument, text and date writers inlined into it), the call site,
  354 B of names and 42 B of `Formatter`. There is no float, integer,
  escaping, builder or panic code. On a bare `Tr`: −1,182 B gz in the
  fixture, within ±132 B in the applications. Every `base` is within 7 B
  raw and 12 B gz of `a5`'s. A5's render test passes on it (15 tests; its
  `Debug` shapes unchanged).
  * **What S2 gives up** (a throwaway test printing both, in `a5-dev`):
    a float shows six fraction digits at most (`1e-7` prints `0.0`); a quote
    or a newline in a text is not escaped; a date prints
    `DateTimeValue(2026-09-28T14:05:09.007)` rather than the nested fields;
    `{:#?}` prints what `{:?}` does. Integers, `2.5`, `0.1`, `123456.789`,
    `Unset` and `Custom(..)` print as before.
* **S3, `Display` through the text the inherent `to_string()` builds
  (`lib-s3-display-via-string.patch`, +4 −15: `ambient::fmt` pads
  `string(…)`, and `text::fmt_display` is gone).** Built after the owner
  asked for "our own Display … that does this for us" (question 14).
  **One copy of the text path per description type**, shared by
  `.to_string()`, `{}` and the traps; A5's `Display` compiled a second one
  (`fmt_display`, 444 B in demo-ssr). `all.sh s3`, Δ raw / Δ gz:

  | client | `{}` `Tr` over `.to_string()`, A5 → S3 | `{}` `TrArgs` over `.to_string()`, A5 → S3 |
  |---|---:|---:|
  | `tr-view` | +472 / +153 → **+82 / +26** | +475 / +148 → **+85 / +22** |
  | demo-ssr | +471 / +217 → **+81 / +70** | — |
  | demo-csr | +834 / +244 → +398 / +140 | +329 / +35 → +434 / +80 |
  | demo-islands | +1,206 / +373 → +1,212 / +374 | — |

  * **Every `base` is A5's** (0 B; demo-ssr −1 B gz).
  * **In `tr-view` and demo-ssr, what is left is the call site.** demo-csr
    and demo-islands call `format!` nowhere else, so there the first `{}`
    also links `core::fmt`'s `pad` and `String`'s `fmt::Write`: in
    demo-islands (names kept), `Formatter` +380 B and data +219, while the
    text path only moves, from `hydrate` into `<Tr as Display>::fmt`
    (+1,076 / −613). Any `format!` would bring the same.
  * **The traps** (demo-ssr, each alone, over `base`): read guard +1,546 /
    +694, `Arc` +385 / +116, `RefCell` +327 / +75, `&&` +333 / +77. That is
    about +290 raw and +60–100 gz over R3's fmt-free path, from about +685
    and +230–255 under A5.
  * A5's render test passes (15; `{}` equals `to_string()`, `{:<16}` pads).
  * **The price:** one more `String` per `{}`, since the text is built,
    then copied into the formatter.

### Ways to remove it

Five variants of A5's crate (`make-removal.py`, `lib-r3.patch`). R1d, R2d
and R3 were also type-checked for `ssr` and with no Leptos mode (`cargo
check -p leptos-mf2`; R1d and R2d contain R1's and R2's changes): no error,
only an unused `use core::fmt` where `Display` is compiled out.
* **R1**, A5's fallback: `Display` only with the std mode (`ssr`; 2.0 adds
  `native`);
* **R2**: no `Display` on `wasm32-unknown-unknown`, whatever the features;
* **R1d, R2d**: the same, and every `Debug` A5 added (`Tr` keeps the
  `Debug` it derived in 1.x);
* **R3**, found here: on `wasm32-unknown-unknown`, the `Display` impls
  exist but carry a bound no type meets, `where &'a Tr: FmtInBrowser`.
  `FmtInBrowser` is a hidden trait carrying `#[diagnostic::on_unimplemented]`
  (stable since 1.78; the MSRV is 1.88), and nothing outside the crate can
  implement it (the orphan rule). A scratch crate (`target/a9/r3-test`)
  showed the mechanism first.

**What they save where nothing is formatted: nothing.** `tables.py variants
--libs v1x,r1,r1d,r2,r2d,r3 base`:

| client | `a5` raw / gz | `v1x` | R1 | R1d | R2 | R2d | R3 |
|---|---:|---:|---:|---:|---:|---:|---:|
| fixture | 328,579 / 87,371 | +22 / −22 | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| `tr-view` | 1,890,883 / 569,373 | −3 / −7 | 0 / 0 | −3 / −9 | 0 / 0 | −3 / −9 | 0 / 0 |
| demo-csr | 248,281 / 98,942 | +1 / −4 | 0 / 0 | +1 / −4 | 0 / 0 | +1 / −4 | 0 / 0 |

A `Display` impl nobody calls was never linked. The few bytes A5's variant
moved against 1.x (A5: −79 in `tr`) come from its non-generic `Debug`
impls, which reshape the destructors: R1d and R2d read what `v1x` reads.

**What they bring back: the compile error**, for `{}` on a `Tr` in
demo-ssr's client (`A9_CHECK=1 A9_LIB=… run.sh demo-ssr display-tr`;
`target/a9/check/*/demo-ssr/*/check.log`):
* **1.x, R1 and R2** (identical text):
  ```
  error[E0277]: `leptos_mf2::Tr` doesn't implement `std::fmt::Display`
      |         std::hint::black_box(format!("{}", x));
      |                                       --   ^ `leptos_mf2::Tr` cannot be formatted with the default formatter
      |                                       required by this formatting parameter
      = help: the trait `std::fmt::Display` is not implemented for `leptos_mf2::Tr`
      = note: in format strings you may be able to use `{:?}` (or {:#?} for pretty-print) instead
  ```
  **The note steers to `{:?}`.** With A5's `Debug` kept, that compiles
  and costs 12–16 KB gz for any description with arguments: 30 to 70 times
  what the refused `{}` would have cost.
* **R3:**
  ```
  error[E0277]: a message description is not formatted with `{}` in a browser build
      |         std::hint::black_box(format!("{}", x));
      |                                       --   ^ `{}` links `core::fmt` into the wasm; write `.to_string()`, the description's fmt-free method
      = help: the trait `leptos_mf2::FmtInBrowser` is not implemented for `&leptos_mf2::Tr`
      = note: `to_string()` gives the same text; `Display` stays for servers, native applications and tests
  help: the trait `std::fmt::Display` is conditionally implemented for `leptos_mf2::Tr`
     --> crates/leptos-mf2/src/display.rs:85:9   (the impl, in `string_conversions!`)
  ```
  No `{:?}` hint, since the impl exists. rustc adds the impl's location and
  a macro-origin note: noisier than 1.x's text, but the first line names
  the fix.
* **`{:?}`** on a `TrArgs`: refused under R1d, R2d and 1.x with
  "`TrArgs` doesn't implement `Debug`" (1.x's text, no hint); it compiles
  under R1, R2 and R3. On a `Tr` it compiles everywhere (1.x's derive).

**What removing `Debug` would break:** an application's `#[derive(Debug)]`
over a struct holding a description stops compiling in browser builds, even
when nothing prints it. The scratch crate shows "`&TrArgs` doesn't
implement `Debug`", rustc's text, in the derive. That was 1.x's state for
`TrArgs`, `TrRich` and `TrDyn`. R3's trick applied to `Debug` breaks the
derive the same way, and its message is lost there. **So `Debug` is the one
to shrink, not to remove.**

### The silent paths

Where a description reaches `Display` or `Debug` with no `format!` in the
application. Searched with `rg` for `ToString`, `Display` and `Debug`
bounds, and for `Display` impls over generic contents, in the Leptos crates
of both lines, vendored into `target/a9/vendor*` with `cargo vendor
--offline`: from demo-ssr's lock, and from a manifest naming
`leptos_router` 0.8.16, `leptos_meta` 0.8.7 and `leptos_axum` 0.8.10. Then
in `crates/`. Each path was compiled into demo-ssr's client
(`A9_CHECK=1 run.sh demo-ssr silent-new silent-traps silent-debug`, `cargo
check --target wasm32-unknown-unknown --features hydrate`). It was checked
on 0.9, and on 0.8 through A7's copy of demo-ssr (`demos-0-8.py`, as
`demo-ssr-08`), at A5's variant and at 1.x (`v1x`); at R3, on 0.9.

| # | Path | Where (0.9.0-beta / 0.8) | A5, 0.9 / 0.8 | 1.x, 0.9 / 0.8 | R3, 0.9 |
|---|---|---|---|---|---|
| 1 | `<Redirect path=tr!(…)/>`: a translated redirect | `leptos_router` `components.rs:579` / `:572`: `P: Display`, then `path.to_string()` | compiles / compiles | refused / refused | refused, R3's message |
| 2 | `<ProtectedRoute redirect_path=\|\| tr!(…)/>` (and `ProtectedParentRoute`) | `components.rs:418`, `:500`, both lines: `Fn() -> P, P: Display` | compiles / compiles | refused / refused | refused, R3's message |
| 3 | `ServerFnError::new(tr!(…))` | `server_fn` `error.rs:207` / `:201`: `msg: impl ToString` | compiles / compiles | refused / refused | refused, R3's message |
| 4 | the application's own generic code: `fn label(x: impl ToString)` | — | compiles / compiles | refused / refused | refused, R3's message |
| 5 | `Either<Tr, TrArgs>::to_string()` | `either_of` 0.1.9 `lib.rs:106` | compiles / compiles | refused / refused | refused (rustc's E0599) |
| 6 | `StaticParamsMap::insert(tr!(…), …)`: implausible, a parameter's name | `static_routes.rs:144`, both lines | compiles / compiles | refused / refused | refused, R3's message |
| 7 | **a signal's read guard**: `signal.read().to_string()` | `reactive_graph` 0.3.0-beta3 / 0.2.15 `guards.rs:83`: `ReadGuard<T: Display>: Display`; also `SignalReadGuard`, `Derefable`, `Mapped*` | **compiles, through `Display`** | compiles, fmt-free | compiles, **fmt-free** |
| 8 | **a smart pointer**: `Arc<Tr>` (and `Rc`, `Box`) | std's `Display` for each | same as 7 | same | same |
| 9 | **a `RefCell` borrow**: `cell.borrow().to_string()` (and `MutexGuard`) | std: `Ref<T: Display>: Display` | same as 7 | same | same |
| 10 | **a reference to a reference**: `(&&t).to_string()`, what `.iter().find(\|t\| t.to_string() == …)` hands a closure | std: `&T: Display` | same as 7 | same | same |
| 11 | `Result::unwrap()` / `expect()` on a `Result` whose error is a description | std: the panic message's `{:?}` | compiles | refused for `TrArgs` (no `Debug`) | compiles |
| 12 | `assert_eq!` / `assert_ne!` on descriptions | std: the failure message's `{:?}` | compiles | compiles for `Tr` (1.x's derive) | compiles |
| 13 | the application's `#[derive(Debug)]` type holding a description, reached by 11, 12 or `{:?}` | — | compiles | refused for `TrRich` | compiles |

* **Paths 1–6 are new:** 1.x refused them (9 errors, the same on both
  lines). Each formats a description through `Display`, so each links what
  a `{}` links (inferred; not measured one by one). **Paths 7–10 are traps:** the
  same code compiles in 1.x, where it took the inherent, fmt-free
  `to_string()`, because a wrapper whose `Display` forwards to the
  description gets the blanket `ToString`. Method resolution then finds it
  one auto-deref step before the description's inherent method. **With A5's
  `Display` the same source silently changes method.** R3 undoes that: the
  wrapper's `Display` bound fails, so resolution falls through to the
  inherent method, as in 1.x.
* **What a trap costs** (demo-ssr, each alone on a `Tr`, `A9_NAMED=1 run.sh
  demo-ssr silent-trap-*` at `a5` and at R3):

  | trap | `a5` over `base` | R3 over `base` | the trap (`a5` − R3) |
  |---|---:|---:|---:|
  | read guard | +1,933 / +845 | +1,249 / +590 | **+684 / +255** |
  | `Arc` | +778 / +287 | +90 / +55 | **+688 / +232** |
  | `RefCell` borrow | +720 / +248 | +38 / +16 | **+682 / +232** |
  | `&&` | +726 / +249 | +44 / +18 | **+682 / +231** |
  | (`.to_string()`) | +44 / +18 | +44 / +18 | 0 / 0 |

  Raw / gz. Each trap is what a `{}` costs (+515 / +235 in the same demo).
  Under R3 each ships what `.to_string()` ships, within 46 B raw, and the
  read guard adds its signal (+1,205 raw).
* **Paths 11–13 are `Debug`'s:** a panic message formats its payload with
  `{:?}`, so an `unwrap()` on a `Result` whose error is a description links
  its `Debug`. demo-ssr with all three (`silent-debug`) is **+27,901 raw /
  +12,357 gz** over `base` at `a5`, and **+2,360 / +1,276 under S2**.

**Ruled out:**
* **tachys** 0.2.19 and 0.3.0-beta3: no `Display` or `ToString` bound
  anywhere, so rendering never formats a value through `Display`; **leptos_meta**:
  none either.
* `<A href=…>`: `ToHref` is implemented for `&str`, `String`, `Cow<str>`,
  `Oco<str>`, `Rc<str>` and `F: Fn() -> String` (`link.rs:14–49`, both
  lines), with no `Display`.
* `<Title text=…>` and the other `TextProp` and `Oco` positions: ours
  (`convert.rs`), through the fmt-free `text::to_string`.
* The router's `query_signal` family (`T: FromStr + ToString`,
  `hooks.rs:28–102`), leptos_server's serializers (`T: ToString + FromStr`)
  and `ParamToString for Option<T: ToString>` (a `#[derive(Params)]` field
  also needs `IntoParam`, i.e. `FromStr`): a description has no `FromStr`.
* leptos_server's `SharedValue<T: Display>: Display` derefs to its value,
  but building one needs a serde codec for it, which a description lacks.
* wasm-bindgen: `JsError: From<E: core::error::Error>` (a description is
  not an `Error`); `JsOption<T: JsGeneric + Display>` (not a `JsGeneric`).
* **Our crates:** no API bounded on `Display` or `ToString` takes a
  description. The one hit, `boot.rs:359`, is `ToString::to_string` on a
  `&str`: the standard library's specialisation, `String::from`, with no
  `core::fmt`.

### A check

**CI can catch it, but not from a release build alone.** The shipped wasm
has no names, so a check reads the same client built with them, then greps
`twiggy top` (`fmt-check.sh`). It fails on any `Display` or `Debug` impl
of a type of ours, including generic ones over one (`<&Tr as Display>`,
`<Arc<DateTimeValue> as Debug>`). It also fails on the blanket `ToString`
over one, and on the helpers only `Display` and `Debug` reach
(`display::ambient::fmt`, `text::fmt_display`, S2's `debug::`).
* **A release build with names kept** (`strip = false`; what A5's
  `twiggy.sh` read):
  * passes on all three demos as they are (`base`) and on `tostring-tr`;
  * fails on every direct `{}` and `{:?}`, naming each item: demo-ssr's
    `display-tr` (1 item, `<Tr as Display>::fmt`), `debug-tr` (2),
    `debug-trargs` (11: `TrArgs`, `ArgList`, `ArgValue`, `Text`,
    `Option<Text>`, `Arc<DateTimeValue>`, `MsgId`, `Date`, `Time`), the
    control (12); the fixture's likewise; S2's `debug-trargs` (5);
  * **misses every trap on its own** (4 of 4 pass). `fmt::Arguments`
    reaches `Display::fmt` through a function pointer, so a `{}` keeps the
    symbol. The blanket `to_string()` calls it directly, and LLVM inlines
    the whole chain into the caller: no symbol is left that names our type.
    With all four traps in one build, only `<TrArgs as Display>::fmt` (the
    `Arc` over a `TrArgs`) survived.
* **A debug-profile build** (`cargo build --lib --target
  wasm32-unknown-unknown --features hydrate`, no LTO, no inlining;
  `all.sh devcheck`): **9 of 9 right.** `base` and `tostring-tr` pass;
  `display-tr`, `debug-tr`, `debug-trargs` and each of the four traps fail
  (3 to 22 items; a trap shows `<Tr as Display>::fmt`, `fmt_display::<Tr>`
  and the guard's `ToString`). No false positive from Leptos's debug-only
  code. **Under R3, the four traps in one debug-profile build pass**: no
  `Display` of ours is linked, so R3's traps are fmt-free, seen directly.
* **Its cost:** from a cold target directory, 116 s (demo-ssr, debug) and
  125 / 87 / 88 s (demo-ssr, demo-csr, demo-islands, release with names), at
  `CARGO_BUILD_JOBS=2` here; a rebuild after an edit, 3 s (debug) or
  10–11 s (release); `fmt-check.sh` over the 136 MB debug module, 0.35 s.
  The only demo client CI builds today is demo-islands, twice, in the
  nightly `b5` job (`cargo xtask islands-zero`); the browser jobs build
  the conformance applications, not the demos. The `b12` job already
  installs twiggy.

### Verdict

* **Done when:**
  * every figure is recorded here with its command: yes;
  * the silent paths are listed: yes, 13, each compiled on both Leptos
    lines, at A5 and at 1.x;
  * a recommendation for A8: below. It changed what A5 adopted for
    `Display`, so it went to the owner, who proposed a third way (S3),
    measured here, and chose it: **question 14**.
* **The figure A5 left open:**
  * **`{}` is cheap:** +0.15–0.6 KB gz, once per description type, in
    every client;
  * **`{:?}` on a description with arguments is not:** 11.8–16.5 KB gz in
    every client, most of it core's float formatter, with its panic paths.
    It is reachable with no `{:?}` in the source (`unwrap()`, `assert_eq!`,
    a derived `Debug`: +12.4 KB gz in demo-ssr).
* **Shrink:** S1 gains nothing; **S2 cuts `Debug`'s cost by 92–93 %**, to
  about 1 KB gz, and drops the panic paths, at a documented loss of
  fidelity. **S3 cuts what `{}` adds to about 25–70 B gz**, and a trap to
  60–100, by sharing `.to_string()`'s text path.
* **Remove:** R1, R2 and their `d` forms save 0 B where nothing formats.
  They bring back rustc's error, which suggests `{:?}`. **R3 refuses `{}`
  in a browser build with a message naming `.to_string()`, and returns the
  wrapper traps to the fmt-free method**, also at 0 B. Removing `Debug`
  breaks `#[derive(Debug)]` over descriptions in browser builds.
* **Check:** a debug-profile client build per demo, then `fmt-check.sh`,
  catches every path found; a release build with names misses the traps.

### What it means for A8 (interpretation, brief)

* **`Debug`: keep it on every type (A5), written as S2 writes it.** It is
  where the bytes are: 12–16 KB gz behind any `{:?}`, `unwrap()` or
  `assert_eq!` on a description with arguments, against about 1 KB with S2.
  Keeping it keeps `#[derive(Debug)]` over an application's types working.
  S2 changes how A5's `Debug` is written, not what A5 adopted; its fidelity
  limits are A8's to state. The runtime's `Date` and `Time` keep their
  derives, which S2 no longer reaches.
* **`Display`: S3, as the owner decided (question 14).** Before it, A9
  recommended R3: `{}` cost 0.15–0.6 KB gz, and code written against 1.x
  (`signal.read().to_string()`, an `Arc`, a `RefCell` borrow, `&&`) took
  `core::fmt` silently, about 0.25 KB gz each. S3 leaves both allowed and
  cuts them to 25–70 and 60–100 B gz. No compile error in browser builds,
  so `<Redirect path=tr!(…)>`, `ServerFnError::new(tr!(…))` and generic
  `impl ToString` code just work. R3 stays documented (`lib-r3.patch`), with
  what it would cost: `.to_string()` before any `Display`-bounded API in
  browser code.
* **The book's rule (F):** `.to_string()` is the leanest; `{}` costs a few
  dozen bytes; `{:?}` about 1 KB with S2, and `unwrap()` / `assert_eq!` on a
  description reach it. **The check** (a debug-profile client build, then
  `fmt-check.sh`) stays available for the demos. With `{}` allowed, a gate
  on it would flag legitimate uses; whether the demos keep one, and for
  what, is A8's call.
* **For every size investigation that keeps names:** `wasm-opt
  --strip-dwarf` before `-Oz`, or the names-kept build is not the shipped
  one (+557 B of code in the fixture).

## Part B — one crate, with every old path kept by shims (B1 after A1 and A7; B2–B4 after B1; B5 with or after B4)

| Task | Deliverable | Done when |
|---|---|---|
| **B1** The types and the Leptos layer into `mf2` | **Moves:** <br>• the four core files → `crates/mf2/src/`; <br>• the Leptos layer → `crates/mf2/src/leptos/`, reaching each line through internal aliases; <br>• the six components → `crates/mf2-leptos-ui-0-9` and `-0-8` (one source, compiled once per line), behind `mf2`'s function table. <br>**Features:** `mf2` gains the Leptos dependencies and `leptos` / `leptos-0-8`, `ssr`/`hydrate`/`csr` (implying the hosts), `static-locale`, `mark-fallback-lang`; the `compile_error!`s name `mf2`'s features. <br>**`leptos-mf2` becomes a shim:** it depends on `mf2`, forwards features, and re-exports everything. <br>**Tests** move to `crates/mf2/tests/`, which ends the dev-dependency cycle. `xtask` `ci` / `msrv` steps switch to `-p mf2`. <br>**In the same commit:** docs.rs metadata, `api.txt`, `package.txt`, 04 §2.1 and 05 §9 rewritten, master plan §4; moved item docs lose their `plans/` citations | `cargo xtask ci` and `docs`; `size` within the gate; `codegen-matrix`, `scenarios`, `leptos-0-8`, `l6-web`, `l7-web`, the e2e checks, `churn`, `msrv`; the ledger unchanged |
| **B2** `mf2::native` | The `native` feature (`sys-locale`, `jiff[tz-system]`, `mf2-catalog`'s `static-bytes` / `content-hash`, std). `mf2-native` becomes a shim; its tests move | as B1, with no web change |
| **B3** `mf2::ratatui` | The `ratatui` feature (implies `native`; `ratatui-core` 0.1). `mf2-ratatui` becomes a shim; its tests move | as B2 |
| **B4** Internal users name `mf2` | Conformance (`conformance/`, `l6-web`, `l7-web`), `bench/churn`, `bench/fluent-ab/mf2`, the `workload-gen` templates the size gate measures (`tr`, `tr-view`, `fluent-converted`), and the xtask crate lists. The examples and the book stay on the shims until C8 and D6 | `conformance-report --check`, `l6-web`, `l7-web`, `size`; the ledger unchanged |
| **B5** The API listed per mode | `cargo xtask api` writes `crates/mf2/api/{core,ssr,hydrate,csr,native,ratatui}.txt` (`axum` joins with D1) from a table in the manifest. `release.rs` runs cargo-semver-checks per mode, with baseline feature sets for 1.0.0's `leptos-mf2/…` spellings | a hydrate-only public item added without its listing fails `api --check` (the negative control) |

## B1 — the types and the Leptos layer into `mf2`: what was built

* **Where.** Commit `1023d57` on `main`. The measurements ran in a worktree of
  their own, `.claude/worktrees/p10-b1-measure` (detached at `7a7994d`;
  `c8a087d`, which came between on `main`, changed only this file): the base
  first, then the same tree with the change applied as a patch (its code
  is the commit's; only comments differ), so every A/B is in one tree with one
  lock per application. The scripts are in `probes/p10-b1/` (its `README.md`
  lists them); their outputs stay in that worktree's git-ignored
  `target/p10-b1/`. Tools: rustc 1.98.1, cargo-leptos 0.3.9, trunk 0.21.13,
  wasm-bindgen 0.2.128, wasm-opt 120, twiggy 0.8.0, Node 23.11 with Playwright's
  Chromium and Firefox. The demos' locks: leptos 0.9.0-beta, tachys 0.3.0-beta2,
  reactive_graph 0.3.0-beta2, wasm-bindgen 0.2.128; their 0.8 copies leptos
  0.8.21, tachys 0.2.19, reactive_graph 0.2.15.
* **What moved where.**
  * `crates/leptos-mf2/src/{tr,arg,dynamic,markup}.rs` → `crates/mf2/src/`.
    The markup closures' impls (`NestingHandler`, `Flat`, `FlatHandler`) went
    with the layer, to `crates/mf2/src/leptos/markup.rs`.
  * The Leptos layer (`boot`, `catalog`, `convert`, `glue`, `glue/view`,
    `lang`, `links`, `registry`, `rich`, `signal`, `state`, `text`, `zone`)
    → `crates/mf2/src/leptos/`, the module `mf2::leptos`. Each file imports
    the line it names from `crate::line`, a private alias module
    (`leptos_0_9`, `tachys_0_3`, `reactive_graph_0_3`, `mf2_leptos_ui_0_9`,
    or the 0.8 set); the glue's two `to_html_with_buf` forms switch on
    `all(leptos-0-8, not(leptos))`. `LoadError` → `crates/mf2/src/error.rs`,
    beside `CompileError`.
  * The string conversions (`to_string`, `to_plain_string`,
    `to_display_string`, `From<_> for String`) → `crates/mf2/src/display.rs`,
    with `Display` (S3, question 14); the `Debug` writers →
    `crates/mf2/src/debug.rs` (S2).
  * The six components → `crates/mf2-leptos-ui-0-9/src/ui.rs`, still
    `view!` and `#[component]`; `crates/mf2-leptos-ui-0-8/src/ui.rs` is a link
    to it, and that crate binds `extern crate leptos_0_8 as leptos`.
    `crates/mf2/src/leptos/components.rs` keeps `html_lang` and wraps each
    component (below).
  * `crates/leptos-mf2/tests/{render,time_zone,churn,fallback_lang}.rs` →
    `crates/mf2/tests/`, each binding the line it is compiled for
    (`extern crate leptos_0_9 as leptos`, or `leptos_0_8`). The
    dev-dependency cycle is gone: `mf2`'s tests use `mf2`.
* **Features of `mf2`.** `leptos` (Leptos 0.9, the default line: `leptos_0_9`,
  `tachys_0_3`, `reactive_graph_0_3`, the 0.9 helper) and `leptos-0-8`; the
  modes `ssr` (implies `host-std`), `hydrate` and `csr` (imply `host-web`),
  each also turning on the layer's own dependencies and forwarding itself to
  the active line and its helper; `static-locale`, `mark-fallback-lang`.
  `fn-datetime` also compiles the reader's time zone, as 1.x's
  `leptos-mf2/fn-datetime` did. The layer, `display.rs` and `line` compile only
  with a mode *and* a line, so each misuse shows one `compile_error!`, naming
  `mf2`'s features: two modes; both lines ("mf2: both Leptos lines are on,
  `leptos` (Leptos 0.9) and `leptos-0-8`: turn on one. Through leptos-mf2 or
  mf2-axum, whose default is Leptos 0.9, Leptos 0.8 needs `default-features =
  false` beside `features = ["leptos-0-8"]` …"); and a mode with no line
  ("mf2: `ssr` needs a Leptos line: turn on `leptos` (Leptos 0.9) or
  `leptos-0-8` beside it."). docs.rs shows `compile, fn-number, datetime-icu,
  host-std, leptos, ssr, static-locale, mark-fallback-lang`.
* **What the shims keep working.**
  * `leptos-mf2` depends on `mf2` alone and re-exports everything 1.x named
    under its path: the core types and the hidden constructors from `mf2`,
    and `mf2::leptos::*` (every layer item, the modules `components`,
    `glue`, `links`, `__private`, and `islands_gate!`) with a mode. Its
    features forward: `leptos-0-9` (still the default) → `mf2/leptos`,
    `leptos-0-8` → `mf2/leptos-0-8`, the modes, `static-locale`,
    `mark-fallback-lang`, `fn-datetime`; `leptos` is kept, empty.
  * `mf2::leptos_mf2` (hidden) is 1.x's facade path, every item under it:
    the demos' i18n crates, `bench/churn`, the book and `mf2 init`'s template
    name `mf2::leptos_mf2::Setup`. `mf2::{Flat, FlatHandler, NestingHandler,
    SignalArg, signal_arg}` stay at the root, where 1.x had them.
  * Unchanged, over the shims, and green: `mf2-axum`, the conformance crates,
    `bench/churn`, `bench/fluent-ab/mf2`, the workload templates, the three
    demos, the book's projects (B4, C8 and D6 move them).
* **`islands_gate!`** is `mf2::leptos::islands_gate!`: a hidden
  `#[macro_export]` macro re-exported there. Its body is a second hidden
  macro, `__islands_gate_export!`, because the `#[rustfmt::skip]` that body
  needs (rustfmt re-indents its `$crate` attribute on every run) makes rustc
  treat the macro as macro-expanded, and such a macro cannot be re-exported by
  path from its own crate.

### The dispatch: static, not the table

The six are generic over the helper's `Layer` trait — `LOCALE_QUERY`,
`ISLANDS_GATE`, `CATALOG_LINK_REL`, `locales()`, `source_locale()`,
`html_lang()`, `preload()`, `catalog_links()`, `switch(tag)`, the calls 1.x's
components made — through a `#[prop(optional)] _layer: PhantomData<L>` prop,
Leptos's own idiom for a generic component. `mf2` implements it for a hidden
`mf2::leptos::components::Mf2`, and each `mf2::leptos` component is a plain
function taking the helper's props with `Mf2` chosen (`pub type
LocaleSwitcherProps = ui::LocaleSwitcherProps<Mf2>`), so `view!` builds it as
it builds any component. Nothing is installed; no function pointer.

Measured against A7's v3 table (`probes/p10-b1/table/`, the same wrappers and
table over the merged layout), each demo's shipped files with
`probes/p10-names/measure-demo.mjs` (gzip −9 and brotli q11 through Node's
zlib), the base's locks restored for every build:

| Demo | base: wasm raw / gz / br | static dispatch (kept): Δ wasm | table: Δ wasm | Δ every shipped file, gz: static / table |
|---|---:|---:|---:|---:|
| demo-ssr (`--split`) | 753,740 / 315,728 / 250,974 | −123 / **−136** / +154 | +1,384 / +339 / +179 | −124 / +340 |
| demo-csr | 207,562 / 91,212 / 77,732 | +78 / **+11** / −152 | +330 / +183 / −69 | +10 / +183 |
| demo-islands | 197,676 / 85,588 / 72,363 | −3 / **+1** / +47 | −6 / +7 / +6 | +1 / +7 |

Commands: `bash probes/p10-b1/measure.sh base` at `7a7994d`, `… b1` with the
change applied; `bash probes/p10-b1/measure-demos.sh table` with
`probes/p10-b1/table/` copied over it. The table reproduces A7's reading in
this tree (A7: +459 / +130 / −8).

**demo-ssr's −136 B gz is outside ±64, downward.** Read as 19 §14 asks
(`bash probes/p10-b1/named.sh probes/p10-b1/b1-static.patch`, the patch
being B1's change as the measurement tree holds it: the client with names
kept, cargo's output
before wasm-bindgen and wasm-opt, joined on normalized names): net −409 B of
code and −72 B of data, every changed item the same function under a name
carrying `<Mf2>`, except that the switch's submit handler is 51 B smaller
(its spawn now in `<Mf2 as Layer>::switch`, whose async body is 1.x's 1,137
B), a method of the switcher's props builder (229 B) is inlined,
`selected_href` (148 B) no longer is, and the wrappers add two
`FnOnce::call_once` shims (124 B). After
wasm-opt the module is 123 B smaller; brotli reads +154. The shipped module
carries one more source path, `…/crates/mf2-leptos-ui-0-9/src/ui.rs`, beside
`…/crates/mf2/src/leptos/components.rs`. A decrease, not a cost: **judged as
holding**, as A5's −83 B was; the owner agreed (question 19). The lazy chunk is the same 23,688 B raw (+13 gz).
**Static dispatch is kept:** the cheaper in every demo, and the only one within
the gate. Question 13's fallback is not needed.

### The gates (19 §14, rows 1 and 2)

| Figure | Base `7a7994d` | B1 | Δ | Gate | Command |
|---|---:|---:|---:|---|---|
| **B1**, fixed | 26,332 B gz | 26,329 | **−3** | ±64 | `CARGO_BUILD_JOBS=3 cargo xtask size --out target/p10-b1/size [--keep]` |
| **B5**, per site | 8.340 B gz | 8.335 | **−0.004** | ±0.2 | same |
| whole app, 1,860 sites | 41,844 B gz | 41,833 | −11 | ambition 105,120 | same |
| `tr` opt raw, 1,860 / 3,720 | 2,419,646 / 4,455,987 | +1 / +1 | | a fixed change | same |
| `idlit`, `dummy` | | | identical | | same |
| `b5 --view`: fixed / per site | 24,811 / 10.459 | 24,817 / 10.458 | +6 / −0.001 | | `cargo xtask b5 --view --out target/p10-b1/b5v [--keep]` |
| `tr-view` opt raw, both scales | 1,890,848 / 3,375,222 | identical | 0 | | same |
| **B7** | | | catalog-bench's report identical but its timestamp; demo-csr's 3 catalogs (and `.br`, `.gz`, `index.json`) byte-identical | byte-identical | `cargo xtask catalog-size`; `cmp` over the demos' `dist/i18n/` |
| **B12** | | clean | | clean | `bash bench/b12/check.sh` (main tree) |
| B1′ / B13 | | +0 B / 13,573 B avoided | | +0 | `cargo xtask b12-generated` |
| demos | see above | −136 / +11 / +1 B gz | | ±64 | `probes/p10-b1/measure.sh` |
| e2e on 0.9 | | demo 210/210, lazy 74/74, islands 58/58, csr 98/98, zone 40/40, a11y 720/720 | | green | `bash probes/p10-b1/e2e.sh . leptos-0-9` |
| e2e on 0.8 | | the same six, the same counts | | green | `python3 probes/p10-names/demos-0-8.py b1`, then `bash probes/p10-b1/e2e.sh target/a7-demo-0-8/b1 leptos-0-8`; `cargo tree` shows leptos 0.8.21, tachys 0.2.19, reactive_graph 0.2.15 and `mf2-leptos-ui-0-8` only, in each server and client build |

B1's −3 is `tr` one byte larger raw at both scales, read by gzip as −11 and
−19; B5 and `b5 --view` move by thousandths.

### The checks (main tree, `1023d57`'s content)

| Check | Result | Command |
|---|---|---|
| `cargo xtask ci` | **pass** (twice; the second after the last edits) | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
| `cargo xtask docs` | **pass**: 120 blocks, 9 applications, `hello-0-8` included | `cargo xtask docs` |
| `cargo xtask docs-rs` | **pass**: 19 crates, no warnings (an outer doc comment on `pub mod leptos` made rustdoc resolve the module's links at the root: removed) | `cargo xtask docs-rs` |
| `codegen-matrix` | **pass**: 18 combinations (8 server, 10 client; five new with the layer on, both lines, `csr` and `static-locale` among them); B6 clean | `cargo xtask codegen-matrix` |
| `scenarios` | **pass**: S1–S4 wasm identical, S5 and S6 rebuilt | `cargo xtask scenarios` |
| `leptos-0-8` | **pass**: both refusals (on `mf2`, and through the shim); on 0.8, the five clippy steps, `render` 15, `time_zone` 8, `churn` 1, `fallback_lang` 8, `mf2-axum` 13 + 4, conformance `layers` 3 and `l6` 4 | `cargo xtask leptos-0-8` |
| its negative control | **fails as designed** (E0425 `RenderFlags`, E0050): its pattern for the glue's cfgs had not matched since `a0f5938` | `cargo xtask leptos-0-8 --negative-control` |
| `l6-web` | **20/20** | `cargo xtask l6-web` |
| `l7-web` | **34/34**; L7 444/444, L7c 444/444, L7d 325/444 and L7cd 325/444 (+119 documented degradations each); the ledger's L7 columns hold | `cargo xtask l7-web` |
| `churn` | **84/84** | `cargo xtask churn` |
| `msrv` | **pass**: the 20 build on Rust 1.88, the five steps | `cargo xtask msrv` |
| the ledger | **unchanged**: `conformance/ledger.toml` untouched, `REPORT.md` regenerated identical; `coverage.toml` names `crates/mf2/tests/render.rs`, and `COVERAGE.md` with it | `cargo xtask conformance-report` |
| clippy on each helper alone | **pass**: `ssr` natively, `hydrate` and `csr` on wasm32, both lines; `mf2` and the shim with no mode | `cargo clippy -p mf2-leptos-ui-0-{9,8} --features <mode> …` |

### `Display` and `Debug` (19 §14, row 3)

Against the B1 build of each client, in the same tree
(`bash probes/p10-b1/display-debug.sh`, `fixture-trview.sh`,
`islands-control.sh`; A9's cases):

| Figure | Measured | Gate |
|---|---:|---|
| a client that formats nothing: our `Display` / `Debug` linked | none (a debug-profile demo-ssr client, `fmt-check.sh`); the controls fail: `{}` 1 item, `{:?}` on a `TrArgs` 11 | none |
| `{}` on a `Tr` over `.to_string()`, demo-ssr | +81 B raw, **0 B gz** (both +36 over base) | ≤ 70 B gz |
| `{:?}` on a `TrArgs` over base: demo-ssr / tr-view / demo-csr / fixture | **+860 / +866 / +1,141 / +1,220 B gz** (the demos through Node's gzip −9, the other two `gzip -9 -n`, as A9) | ≤ 1.3 KB gz |
| the same, demo-islands | **+1,353 B gz** | ≤ 1.3 KB gz |

demo-islands formats nothing otherwise, so its first `format!` brings
`format!`'s own machinery: `format!("{:?}", a String)` costs +424 B gz there
(`format!("{}", …)` +410). Over that control, the `Debug` of a `TrArgs` is
+929 B gz. Two leaner forms of the writers were tried there and were larger
(`write_str` for every character: +38 B gz; digits through a buffer: +56).
Their code was not kept; `probes/p10-b1/debug-writestr.patch` and
`debug-buffer.patch` reconstruct it, and `bash
probes/p10-b1/debug-variants.sh` (in the measurement tree) builds S2 and
both, one after the other: 200,681 / 86,942 B raw / gz for S2 (B1's own
build of it, byte for byte), then 200,792 / 86,980 (+38) and 200,813 /
86,998 (+56). The buffer's build is byte-identical to the one B1 left in
that tree (B1's review fixes).
A9 measured S2 in the fixture, `tr-view`, demo-csr and demo-ssr, not in
demo-islands. **The owner's reading (question 18):** the cap counts our
code's own cost, over a base that already formats text, so the row holds.

### What A7 left to B1

* **The `*Props` types in the listings:** listed. `mf2`'s `api.txt` has
  `mf2::leptos::LocaleSwitcherProps` and the rest as type aliases of the
  helper's props at `Mf2`; each helper's `api.txt` has its generic props and
  the `Layer` trait. B5's per-mode listings take them as they are.
* **A checkout without symbolic links:** the link stays, as every crate's
  `LICENSE` is already one. `cargo package` follows it; `cargo xtask package`
  now checks where it points (its link table gains
  `mf2-leptos-ui-0-8/src/ui.rs` → `crates/mf2-leptos-ui-0-9/src/ui.rs`); a
  checkout with `core.symlinks=false` does not compile the 0.8 helper, which
  its `lib.rs` says.

### Found along the way (routed)

* B5: `cargo xtask api` lists neither `mf2::leptos::islands_gate!` (the
  re-export of a hidden macro) nor what the shim re-exports by glob (its
  listing is `pub use leptos_mf2::<<mf2::leptos::*>>`).
* B5 / G: `release.rs`'s semver check leaves `leptos` out of `mf2`'s feature
  set too, since 1.0.0's `leptos` needs a line from `leptos-mf2`.
* C2: `missing_debug_implementations` warns in `mf2` since B1; the runtime's
  and the catalog's 31 types remain C2's.
* Whoever next edits `CLAUDE.md`: its list of client-path crates names
  `leptos-mf2`; that code is now `mf2`'s and the helpers'.
* Not scheduled: nightly cargo warns that the workspace dependency
  `mf2-ratatui` is unused (as before B1); B3 changes it.

## B2 — `mf2::native`: what was built

* **Where.** Commit `90c8b2a` on `main`. Its three renames
  (`crates/mf2-native/src/{native,locale}.rs`, `tests/native.rs`) and the
  removal of `crates/mf2-native/src/error.rs` went in one commit early: B2
  had staged them (`git mv`, `git rm`), and the coordinating session's
  `46303b9`, a plans commit, committed the index with them. That commit
  alone does not build (1.x's `mf2-native/src/lib.rs` names the modules it
  lost); `90c8b2a` completes the move. The measurements ran in a worktree of
  their own, `.claude/worktrees/p10-b2-measure`, detached at `c41225c` (the
  commit before both), with the main tree's lock files copied in: the base
  first, then the same tree with B2 applied (`git diff --binary c41225c --
  crates xtask CHANGELOG.md plans/00-master-plan.md plans/05-tooling.md
  plans/19-native-and-terminal.md plans/README.md`, and the two new files
  copied; its code is the commit's). So every A/B is in one tree, with one
  lock per application. The scripts are in `probes/p10-b2/` (its
  `README.md` lists them). Their outputs are in that worktree's
  `target/p10-b2/`, and the checks' logs are in the main tree's
  `target/p10-b2/`. Tools as B1's.
* **What moved where.**
  * `crates/mf2-native/src/native.rs` → `crates/mf2/src/native.rs`, the
    module `mf2::native`; its documentation is 1.x's crate documentation.
    `locale.rs` (the matcher and the enum) → `crates/mf2/src/native/locale.rs`.
    `NativeError` → `crates/mf2/src/error.rs`, beside `CompileError` and
    `LoadError`. `tests/native.rs` → `crates/mf2/tests/native.rs`
    (`required-features = ["native", "compile"]`). The code is 1.x's, on
    `mf2`'s own paths (`crate::Corpus`, `mf2_catalog`, `mf2_runtime`,
    `mf2_host_std`), with `alloc` imports for the `no_std` crate.
  * **Names.** `NativeI18n` and `NativeError` keep 1.x's names. 19 §4's
    `Catalogs` is `NativeI18n` reshaped (`format(locale, &message)`, no
    active locale), and its `Error` is what C2's `install_from_directory`
    returns, so C2 names both as it builds that API. `LocaleSource` was
    renamed `LocaleOrigin` here, since 19 §16 then counted it a clash
    with `mf2::axum::LocaleSource`; the owner answered that items in
    different modules keep their names (question 23), and B1's review
    fixes undid the rename: **`mf2::native::LocaleSource`**, 1.x's name,
    with its three variants, and `locale_source()`.
  * **`Debug`** (19 §6): `NativeI18n` has one, written by hand: the active
    locale, where it came from, the catalogs (each `Catalog`'s own: locale,
    messages, bytes) and the formatting context. A derived one would print
    the corpus's embedded bytes. `NativeError` and `LocaleSource` derive
    it, as in 1.x.
* **The `native` feature.** It implies `host-std`, and turns on
  `sys-locale`, jiff's `std` and `tz-system`, and `mf2-catalog`'s
  `static-bytes` and `content-hash`, as 1.x's crate did; `extern crate std`
  is compiled under it too. **Beside `hydrate` or `csr` it is refused**
  (19 §3): "mf2: `native` is on beside `hydrate` or `csr`: `native` is for
  an application that runs natively (a command-line tool, a terminal UI, a
  server), never for a browser build. cargo unifies features across a
  workspace, so a browser client and a native application belong in
  workspaces of their own." `bash probes/p10-b2/refusals.sh`: on `mf2` for
  `wasm32-unknown-unknown` with `hydrate` and with `csr`, through the shim
  (`-p mf2-native --features mf2/leptos,mf2/hydrate`, wasm32), and natively
  with `hydrate`, that sentence is the only error the user reads
  (`sys-locale` and `jiff` compile for wasm32 first, without error).
  `native` beside `ssr` compiles. docs.rs and `api.txt` show `mf2` with
  `native` beside `ssr`.
* **What the shim keeps working.** `mf2-native` depends on `mf2` with
  `native` and re-exports, under 1.x's names and paths, `NativeI18n`,
  `NativeError`, `LocaleSource`, and
  `BidiStrategy`, `Corpus`, `Dir`, `Message`, `TimeZone`. It has no
  features, as 1.x's had none, so its paths do not depend on how an
  application names features: the gap the review of B1 found in
  `leptos-mf2` does not arise here. Its test,
  `crates/mf2-native/tests/names.rs`, holds each name to `mf2`'s item.
  Unchanged over it, and green: `mf2-ratatui` (7 tests; its `api.txt` now
  names `mf2::native::NativeI18n`), the book's native project with and
  without `tui` (`cargo xtask docs`), and `examples/tui` (`tui-gate`).
* **xtask.** `ci` gains two steps for `native` alone:
  `clippy -p mf2 --features native,compile --all-targets`, and
  `test -p mf2 --features native,compile --lib --test native`. The reason:
  `--workspace` always unifies `ssr` into `mf2`, so nothing else compiles
  `native` without the Leptos layer. `msrv`'s server features name
  `mf2/native`. `release` leaves `native` out of `mf2`'s semver run: 1.0.0's
  `mf2` has no such feature (1.x's `mf2-native` was never published).
* **In the same commit:** 05 §9; the master plan's §4 and §4.1; 19 §4 and
  §16; `plans/README.md`; the changelog; `mf2`'s and the shim's READMEs;
  the comments in `mf2-build` and `mf2-catalog` that named `mf2-native`.
  Also `api.txt` (`mf2`, `mf2-native`, `mf2-ratatui`) and `package.txt`
  (`mf2`, `mf2-native`).

### The gates (19 §14, row 1): the web byte-identical

`bash probes/p10-b2/measure.sh base` at `c41225c`, then `… b2` with B2
applied. It runs `cargo xtask size --out target/p10-b2/size [--keep]`,
`cargo xtask b5 --view --out target/p10-b2/b5v [--keep]`, `cargo xtask
catalog-size` and the three demos' clients as they ship, and hashes each
built file (`target/p10-b2/logs/{base,b2}/*.sha256`). B2's runs hashed
demo-csr's `dist/` but not the other two demos' `pkg/`, which `measure.sh`
hashes since B1's review fixes; `bash probes/p10-b2/demo-hashes.sh` in the
measurement tree compares the kept outputs of both runs: 30 files a run
(demo-csr 16, demo-ssr 9, demo-islands 5), all byte-identical but
demo-ssr's `__wasm_split` loader (below):

| Figure | Base `c41225c` | B2 | Gate |
|---|---:|---:|---|
| **B1**, fixed | 26,344 B gz | 26,344 | ±64 |
| **B5**, per site | 8.334 B gz | 8.334 | ±0.2 |
| whole app, 1,860 sites | 41,845 B gz | 41,845 | ambition 105,120 |
| the size workloads' 12 wasm files (`tr`, `idlit`, `dummy`, both scales, before and after `wasm-opt`) | | byte-identical | |
| `b5 --view`: fixed / per site, and its 12 wasm files | 24,755 / 10.480 | identical; byte-identical | |
| **B7**: catalog-bench's report; demo-csr's `dist/` (16 files: its 3 catalogs, their `.br` / `.gz`, `index.json`, the wasm and JS) | | identical but the report's `unix_time`; byte-identical | byte-identical |
| the demos' shipped wasm, raw / gz: demo-ssr, its lazy chunk, demo-islands, demo-csr | 753,780 / 315,718; 23,688 / 11,516; 197,670 / 85,590; 207,638 / 91,224 | byte-identical | ±64 B gz |
| **B12** | | clean | clean (`bash bench/b12/check.sh`, main tree) |
| B1′ / B13 | | +0 B / 13,573 B avoided | +0 (`cargo xtask b12-generated`) |
| locks | | the demos' and the workloads' identical; the workspace's has the same package versions as B1's tree (only `mf2`'s and `mf2-native`'s dependency lists moved) | one lock per application |

One file differs: demo-ssr's `__wasm_split` loader, 2,187 B raw either
way. Its imports come in another order. Three rebuilds of the B2 tree with
nothing changed gave three hashes of it (`ed794eaf907b`, `53b12bf3095a`,
`1055d334c703`) over one wasm (`ff1f2e4059af`): `touch src/lib.rs; cargo
leptos build --release --split --frontend-only`, thrice, in
`examples/demo-ssr`. B1's base and B1 runs show two of the same three
hashes. Not B2's.

**Native, reported** (C2's gate, not B2's). `cargo xtask tui-gate
--save-baseline target/p10-b2/tui-base` at base, then `cargo xtask tui-gate
--baseline target/p10-b2/tui-base` with B2: the four binaries alternating,
31 runs, timings under a load of 2.81.

| Binary | Stripped (B), base → B2 | Allocations per frame (en / de / es / fr), both | Median µs per frame, base / B2 |
|---|---:|---|---:|
| `tui-mf2` | 1,965,496 → 1,965,672 (+176: `.text` −128, unwind tables +296) | 1816 / 1815 / 1816 / 1817 | 378.5 / 383.3 (ranges 360–474, 358–509) |
| `tui-upstream`, which uses no MF2 | 1,390,784 → 1,400,640 (+9,856: `.gcc_except_table` +9,780, `.text` the same) | 1517 / 1519 / 1518 / 1526 | 336.3 / 337.1 |

The sections are `size -A` over the kept and the new binaries. Rebuilt
(`touch` on two sources, then the gate's own `cargo build --release --bins`
with `CARGO_PROFILE_RELEASE_STRIP=symbols`), the B2 binaries are
byte-identical. Inference, brief: the linker keeps exception tables of
functions it discards, so a binary's size moves with what its rlibs hold.
Both sizes are this worktree's. Its longer path makes them larger than the
main tree's, where 19 §14's C2 figure (1,965,320 B, at 1.x) was taken.

### The checks (main tree, `90c8b2a`'s content)

| Check | Result | Command |
|---|---|---|
| `cargo xtask ci` | **pass**, with the two new `native` steps (the matcher's 5 tests, the moved 7) | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
| `cargo xtask docs` | **pass**: 120 blocks, 9 applications; the native project with and without `tui`, over the shims | `bash probes/p10-b2/checks.sh docs` |
| `cargo xtask docs-rs` | **pass**: 19 crates, no warnings | `… checks.sh docs-rs` |
| `codegen-matrix` | **pass**: 18 combinations; B6 clean | `… codegen-matrix` |
| `scenarios` | **pass**: S1–S4 wasm identical, S5 and S6 rebuilt | `… scenarios` |
| `leptos-0-8` | **pass**: both refusals; on 0.8, the five clippy steps, `render` 15, `time_zone` 8, `churn` 1, `fallback_lang` 8, `mf2-axum` 13 + 4, conformance `layers` 3 and `l6` 4 | `… leptos-0-8` |
| `l6-web` | **20/20** | `… l6-web` |
| `l7-web` | **34/34**; L7 444/444, L7c 444/444, L7d 325/444 and L7cd 325/444 (+119 documented degradations each); the ledger's L7 columns hold | `… l7-web` |
| `churn` | **84/84** | `… churn` |
| `msrv` | **pass**: the 20 on Rust 1.88, five steps | `… msrv` |
| e2e on 0.9 | demo 210/210, lazy 74/74, islands 58/58, csr 98/98, zone 40/40, a11y 720/720 | `… e2e-0-9` (`bash probes/p10-b1/e2e.sh . b2-leptos-0-9`) |
| e2e on 0.8 | the same six, the same counts; `cargo tree` shows leptos 0.8.21, tachys 0.2.19, reactive_graph 0.2.15 and `mf2-leptos-ui-0-8` only, in each server and client build | `… e2e-0-8` (`python3 probes/p10-names/demos-0-8.py b2`, then `bash probes/p10-b1/e2e.sh target/a7-demo-0-8/b2 b2-leptos-0-8`) |
| the ledger | **unchanged**: `conformance-report`'s check in `ci`; `conformance/ledger.toml` untouched | `cargo xtask ci` |
| the shim and `mf2-ratatui` | **pass**: `names` 2 tests; `mf2-ratatui` 7 | `cargo test -p mf2-native -p mf2-ratatui` |
| the refusals | as above | `bash probes/p10-b2/refusals.sh` |

### Found along the way (routed)

* C2: 19 §4's `Catalogs` and `Error` are the names of the API C2 builds on
  `NativeI18n` and `NativeError`; the shim's 1.x names stay.
* C2 / C8: native sizes carry the build's source paths and move with the
  rlibs linked beside the code: `tui-upstream`, the baseline, grew +9,856 B
  with no change of its own. Compare within one tree and one lock.
* B1's review fixes (item 4) / D1: of 19 §3's refusals, only "both lines"
  is checked by an xtask (`leptos-0-8`). The others were checked by hand
  (B1's; `native`'s by `probes/p10-b2/refusals.sh`). B3's `ratatui`
  inherits `native`'s, and D1 adds `axum`'s.
* Every task: a commit made while another task has changes staged carries
  them (`46303b9` carried B2's renames). Stage and commit by path
  (`git commit -- <paths>`).

## B1's review fixes: what was done

Items 1–6 came from a read-only review of B1's commits and the owner's answer to question 23;
items 7–9 from a read-only review of B2's. Made in the main tree; one commit per item or pair
(below). Commands ran there unless a measurement tree is named. Logs: the main tree's git-ignored
`target/p10-b1-fixes/`.

1. **B1's figures without a command** (`418b189`). `probes/p10-b1/b1-static.patch` is the change
   B1's measurement tree holds, and `named.sh`'s PATCH: it reverse-applies there (`git apply -R
   --check`), every path that tree changes is in it, and its code is `1023d57`'s but for
   comments. The two leaner `Debug` writers were not kept. `debug-writestr.patch` and
   `debug-buffer.patch` reconstruct them, the second from B1's build of it, which that tree still
   held (its `mf2` object, disassembled: an index loop into a 20-byte buffer, then `from_utf8`
   and one `write_str`). `bash probes/p10-b1/debug-variants.sh` in that tree (demo-islands, A9's
   `debug-trargs`):

   | Build | wasm raw / gz / br | Δ gz over S2 | sha256 (12) |
   |---|---:|---:|---|
   | S2, kept | 200,681 / 86,942 / 73,552 | | `65a813f6dec6`, as B1's own build |
   | `write_str` for every character | 200,792 / 86,980 / 73,516 | +38 | `a02b2316fccd` |
   | the digits through a buffer | 200,813 / 86,998 / 73,591 | +56 | `09ef9aef3834`, as B1's own build |

   B1's record cites both commands.
2. **"Wraps", not "re-exports"** (`09aed9a`): master plan §4.1 and D20, 04 §12.1, 19 §3, the root
   `Cargo.toml`, the helpers' descriptions, crate docs, READMEs and `ui.rs`, and the changelog.
3. **`bench/churn/README.md`** names `crates/mf2/tests/churn.rs` (`09aed9a`).
4. **Two modes with a line on: confirmed, and fixed** (`f5e72fd`).
   - *Observed.* `cargo check -p mf2 --features leptos,ssr,hydrate` exited 101, and its one error
     read "mf2-leptos-ui-0-9: turn on exactly one of `ssr`, `hydrate` and `csr`." The helper
     compiles before `mf2`, which never ran. The same came from `mf2-leptos-ui-0-8` with
     `leptos-0-8,ssr,hydrate`, and from the 0.9 helper with `leptos,hydrate,csr`. With no line
     (`ssr,hydrate`), `mf2`'s own three sentences show.
   - *Fixed.* The helpers' two refusals are `mf2`'s sentences, word for word.
     `cargo xtask refusals`, a step of `ci`, checks 19 §3's nine cases: two modes on each line;
     the two client modes; both lines; each mode with no line; `native` beside `hydrate`, and
     beside `csr`, for `wasm32-unknown-unknown`. Each must fail with `mf2`'s sentence as its only
     error. With the 0.9 helper's old sentence put back, it fails at its first case (the negative
     control, run once by hand).
5. **The shim's layer paths: confirmed, and fixed** (`5eb881e`).
   - *Observed.* With `crates/leptos-mf2/tests/layer.rs` written first,
     `cargo test -p leptos-mf2 --features leptos,mf2/ssr --test layer` gave 11 errors (E0425,
     E0433): `Setup`, `LoadError`, `RequestI18n`, `LocaleSwitcherProps`, `install`, `html_lang`,
     `catalog`, `setup`, `links`, `components`. `leptos_mf2::islands_gate!` resolved to the shim's
     own empty macro, so in a `hydrate` build it would have exported no gate.
   - *Why.* 1.x's `mf2/ssr` turned on `leptos-mf2/ssr`; since B1, `mf2`'s mode does not reach
     the shim.
   - *Fixed.* `mf2::leptos_mf2` (hidden; 1.x's facade path) is present in every build. It
     carries `mf2::leptos` whenever `mf2` compiles it, else `islands_gate!` alone, expanding to
     nothing. The shim re-exports `mf2::leptos` under its own mode, as before, so its docs and
     `api.txt` do not change. With no mode of its own, it re-exports `mf2::leptos_mf2`. The test
     passes; `ci` runs it in that arrangement, and `--workspace` runs it too
     (`required-features = ["mf2/ssr"]`).
6. **`mf2::native::LocaleSource` again** (`c7dd54d`): the code, the shim (a plain re-export), the
   tests, `mf2`'s `api.txt` (`cargo xtask api`: those 16 lines only), the changelog, 05, 19 §4,
   `mf2-native`'s README and docs, and B2's record.
7. **B2's demo hashes from a committed script** (`1552c1d`). `probes/p10-b2/measure.sh` hashes
   demo-ssr's and demo-islands' `pkg/` too. B2's runs predate that, so `bash
   probes/p10-b2/demo-hashes.sh` compares their kept outputs, in B2's measurement tree: 30 files
   a run (demo-csr 16, demo-ssr 9, demo-islands 5), all byte-identical but demo-ssr's
   `__wasm_split` loader (`591006faae48` → `27af43e4de73`). B2's record cites it.
8. **`mf2`'s implicit feature `jiff`: confirmed, and gone** (`f51b30e`).
   - *Observed.* `cargo metadata --no-deps --format-version 1 | jq -c '.packages[] |
     select(.name=="mf2") | .features.jiff'` printed `["dep:jiff"]`. That is a public feature no
     manifest declares: `native` named `jiff/std` and `jiff/tz-system`, but never `dep:jiff`.
   - *Fixed.* `native` names `dep:jiff`, and the same command prints `null`.
   - *Checked.* No other workspace package has a feature its manifest does not declare: each
     package's `features` in that metadata, against its manifest's `[features]`.
9. **`native` beside `csr` across a workspace: confirmed; the owner's to decide** (`f1ce0d0`).
   - *Observed.* `bash probes/p10-b2/unify.sh` builds a workspace under `target/p10-b2/unify/`: a
     web crate naming `csr` on `leptos-mf2` and `mf2` (as `examples/demo-csr` does), and a
     command-line tool on `mf2-native`. `cargo check -p web` exited 0, and so did `-p cli`.
     `cargo check --workspace` exited 101; its one error was 19 §3's refusal, "mf2: `native` is
     on beside `hydrate` or `csr`: …".
   - *Read, not built.* At `7a7994d` (1.x), `mf2` has no `native` feature and no
     `compile_error!`, and `mf2-native` names `mf2` with `host-std` only.
   - The rule stays until the owner chooses ("Owner questions found in the work", above).
     *Since:* the owner chose to refuse only for the browser (question 24), and the browser-only
     refusal built it (its record, below).

**Checks** (main tree, with every fix in):

| Check | Result | Command |
|---|---|---|
| `cargo xtask ci` | **pass**, with its two new steps: the nine refusals, and the shim's test | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
| `leptos-0-8` | **pass**: both refusals; the five clippy steps, `render` 15, `time_zone` 8, `churn` 1, `fallback_lang` 8, `mf2-axum` 13 + 4, conformance `layers` 3 and `l6` 4 | `cargo xtask leptos-0-8` |
| `l7-web` | **34/34**; L7 444/444, L7c 444/444, L7d and L7cd 325/444 (+119 documented degradations each); the ledger's L7 columns hold | `cargo xtask l7-web` |
| `docs` | **pass**: 120 blocks on 10 pages, 9 applications | `cargo xtask docs` |
| `docs-rs` | **pass**: 19 crates, no warnings | `cargo xtask docs-rs` |

**Sizes** (19 §14, row 1; the fixes change no client code): `bash probes/p10-b2/measure.sh base`
in the main tree, with the 21 files the fixes change under `crates/` and the root `Cargo.toml` at
`8e1c6bd`'s content (`git show 8e1c6bd:<path>` into each; the new test set aside), then `… fixes`
with the fixes back. One tree; the workloads' locks kept (`--keep`), the demos' restored, the
workspace's unchanged. The main tree's figures, not comparable with B1's or B2's worktrees:

| Figure | Base | Fixes | Gate |
|---|---:|---:|---|
| **B1**, fixed | 26,720 B gz | 26,720 | ±64 |
| **B5**, per site | 8.167 B gz | 8.167 | ±0.2 |
| whole app, 1,860 sites | 41,911 B gz | 41,911 | ambition 105,120 |
| the size workloads' 12 wasm files | | byte-identical | |
| `b5 --view`: fixed / per site, and its 12 wasm files | 24,634 / 10.5 | identical; byte-identical | |
| **B7**: catalog-bench's report; demo-csr's `dist/` (16 files) | | identical but the report's `unix_time`; byte-identical | byte-identical |
| the demos' shipped files, 30 a run (`bash probes/p10-b2/demo-hashes.sh base fixes`) | | byte-identical but demo-ssr's `__wasm_split` loader | ±64 B gz |
| **B12** | | clean | clean (`bash bench/b12/check.sh`) |

The loader went `27af43e4de73` → `ed8d63b87a98`. Two rebuilds with nothing changed (`touch
src/lib.rs; cargo leptos build --release --split --frontend-only` in `examples/demo-ssr`) gave
`27af43e4de73` and `53b12bf3095a`, over one wasm (`82338f4b3f4c`, the same in both runs). As B2
found, the loader's hash varies between identical builds; it is not the fixes'.

## The browser-only refusal (question 24): what was built

Made in the main tree, on `main`: `1dd5058`. Commands ran there, one build at a time. Logs: the
main tree's git-ignored `target/p10-q24/` (`before/` with `bdbbd4f`'s code, `after/` with the
change) and `target/p10-b2/logs/q24-base/` and `…/q24/` (the size runs).

1. **The rule** (`crates/mf2/src/lib.rs`). The `compile_error!` for `native` beside `hydrate` or
   `csr` now also needs `target_arch = "wasm32"`: every `wasm32` target, as question 24 words it,
   so `wasm32-wasip1` too, where neither browser mode has a use. Its old advice (a browser client
   and a native application in workspaces of their own) no longer holds, so the sentence now says
   what a browser build needs: "mf2: `native` is on beside `hydrate` or `csr` in a build for the
   browser (`wasm32`): `native` is for an application that runs natively (a command-line tool, a
   terminal UI, a server), never for a browser build. cargo unifies features across the packages
   it builds together: build the browser client on its own (`-p`), and keep `native` off in every
   crate it depends on." On the host the two compile together, lint-clean: `cargo clippy -p mf2
   --features native,leptos,hydrate -- -D warnings` exits 0, and so does the same with `csr`.
2. **`cargo xtask refusals`, both sides.** The nine refused cases as before, the two `native` ones
   for `wasm32-unknown-unknown` with the new sentence. Then three host cases, each a `cargo check`
   that must pass: `native` beside `hydrate`; beside `csr`; and 1.x's workspace, a client on
   `leptos-mf2` beside a tool on `mf2-native` (`-p leptos-mf2 -p mf2-native --features
   leptos-mf2/csr`). *Negative control* (run once by hand): with the old `cfg` put back, it
   passes the nine and fails at the first host case, whose one error is the refusal (exit 1).
3. **Before and after:**

   | Command | Before (`bdbbd4f`'s code) | After |
   |---|---|---|
   | `bash probes/p10-b2/unify.sh`: `-p web`, `-p cli`, `--workspace` | 0, 0, **101**: 19 §3's refusal, the one error | 0, 0, **0** |
   | its workspace, `cargo check --workspace --all-targets` (rust-analyzer's default check) | | 0 |
   | `bash probes/p10-b2/refusals.sh`: `mf2` for `wasm32` with `hydrate`, and with `csr`; the shim with `mf2/hydrate` | 101 each, the old sentence alone | 101 each, the new sentence alone |
   | … `native,leptos,hydrate`, natively | **101**, the refusal | **0** |
   | … `native,leptos,ssr` | 0 | 0 |
4. **Text.** Master plan §4 (`mf2`'s row) and §4.1; 05 §9; the changelog, whose promise stands,
   a mixed workspace included; `mf2`'s crate docs, its manifest's comment on `native`, and
   `mf2::native`'s docs; both shims' docs and READMEs, which make the same promise and now name
   the workspace case; the xtask's help and `ci`'s comment; `probes/p10-b2/`'s comments and
   README. 19 §3 was amended with the answer.

**Checks** (main tree, with the change):

| Check | Result | Command |
|---|---|---|
| `cargo xtask ci` | **pass**, `refusals` with its 9 + 3 | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
| `docs-rs` | **pass** | `CARGO_BUILD_JOBS=3 cargo xtask docs-rs` |

Not run: `leptos-0-8`, `l7-web` and the full `docs`. The change compiles no client differently
(below), leaves the 0.8 refusals and the book alone, and `ci` runs `docs --no-build`.

**Sizes** (19 §14, row 1): `bash probes/p10-b2/measure.sh q24-base` with `bdbbd4f`'s code, then
`… q24` with the change. One tree; the size workloads' locks kept (`--keep`), the demos' restored
from `locks/base` both times, the workspace's unchanged.

| Figure | Before | After | Gate |
|---|---:|---:|---|
| **B1**, fixed | 26,720 B gz | 26,720 | ±64 |
| **B5**, per site | 8.167 B gz | 8.167 | ±0.2 |
| whole app, 1,860 sites | 41,911 B gz | 41,911 | ambition 105,120 |
| the size workloads' 12 wasm files | | byte-identical | |
| `b5 --view`: fixed / per site, and its 12 wasm files | 24,634 / 10.5 | identical; byte-identical | |
| **B7**: catalog-bench's report; demo-csr's `dist/` (16 files) | | identical but the report's `unix_time`; byte-identical | byte-identical |
| the demos' shipped files, 30 (`bash probes/p10-b2/demo-hashes.sh q24-base q24`) | | byte-identical but demo-ssr's `__wasm_split` loader | ±64 B gz |
| **B12** | | clean | clean (`bash bench/b12/check.sh`) |

The loader went `ed794eaf907b` → `27af43e4de73`. The before run's demo builds took 2–3 s each
(nothing to recompile), and its files match B1's review fixes' last run (`fixes`) in all but that
loader too (`ed8d63b87a98` → `ed794eaf907b`): as B2 found, its hash varies between identical
builds.

**Found along the way** (routed, above): C2's ambient forms must compile, and its one lookup hold,
with a client mode and `native` both on, which the host now allows; B3's and D1's cases go on both
sides of `cargo xtask refusals`.

## B3 — `mf2::ratatui`: what was built

* **Where.** Commit `afd3811` on `main`, made in the main tree; commands ran there, one build at a
  time. The before figures were taken at `140a18a`, before any change, in the same tree (below).
  Logs: the main tree's git-ignored `target/p10-b3/` (`ci-2.log`, the refusals, `checks/`, the
  `tui-gate` runs), and `target/p10-b2/logs/b3-base/` and `…/b3/` (the size runs).
* **What moved where.**
  * `crates/mf2-ratatui/src/lib.rs` → `crates/mf2/src/ratatui.rs`, the module `mf2::ratatui`; its
    documentation is 1.x's crate documentation on `mf2`'s paths, and says where the feature is
    refused. The code is 1.x's: `MarkupStyles`, `line`, `text` and the private sink, over
    `crate::native::NativeI18n` and `mf2`'s own re-exports, with `alloc` imports for the `no_std`
    crate and `core::mem::take` for `std::mem::take`. `tests/styled.rs` →
    `crates/mf2/tests/ratatui.rs` (`required-features = ["ratatui", "compile"]`), its 7 tests
    unchanged but for their imports.
  * **Names.** 1.x's three keep their names; 19 §8's conversions, `Styled`, `Widget` and the theme
    are C5's, and the three go with C8's page (19 §8 now says where they live until then).
    `MarkupStyles` derives `Debug`, as in 1.x (19 §6).
* **The `ratatui` feature.** `ratatui = ["native", "dep:ratatui-core"]`: it implies `native` and adds
  `ratatui-core` 0.1.2 alone, the latest release on crates.io (its sparse index, 2026-09-29; the
  workspace already named it), with no default features (it has none). **Beside `hydrate` or
  `csr`, when compiling for `wasm32`** (question 24), it is refused in a sentence of its own:
  "mf2: `ratatui` is on beside `hydrate` or `csr` in a build for the browser (`wasm32`): `ratatui`
  is for a terminal UI, which runs natively, and implies `native`; neither belongs in a browser
  build. cargo unifies features across the packages it builds together: build the browser client
  on its own (`-p`), and keep `ratatui` and `native` off in every crate it depends on." `native`'s
  `cfg` gains `not(feature = "ratatui")`, so the one error names the feature the application
  turned on; `native`'s sentence alone would tell a `ratatui` user to turn off a feature their
  manifests never name (19 §3: a refusal "says what to write"). A small choice within 19 §3, not a
  question. On the host the combination compiles. docs.rs and `api.txt` show `ratatui` beside
  `ssr` and `native`.
* **What the shim keeps working.** `mf2-ratatui` depends on `mf2` with `ratatui` and re-exports
  `MarkupStyles`, `line` and `text` under 1.x's names and paths; it has no features, as 1.x's had
  none. Its test, `crates/mf2-ratatui/tests/names.rs`, holds each name to `mf2`'s item: the type
  by its `TypeId`, each function by the `TypeId` of its function item, which is one type only when
  two paths name one function. *Negative control* (once, by hand): with the shim's `line` and
  `text` swapped, it fails. Unchanged over the shim, and green: `examples/tui` (`tui-gate`) and the
  book's native project with `tui` (`cargo xtask docs`). Through the shim, for `wasm32` with
  `mf2/leptos,mf2/hydrate`, the `ratatui` sentence is the one error (`cargo check -p mf2-ratatui
  --features mf2/leptos,mf2/hydrate --target wasm32-unknown-unknown`, exit 101).
* **xtask.**
  * `ci` gains two steps for `ratatui` alone, as B2's for `native`: `clippy -p mf2 --features
    ratatui,compile --all-targets` and `test -p mf2 --features ratatui,compile --test ratatui`
    (`--workspace` always unifies `ssr` into `mf2`).
  * `refusals` takes `ratatui`'s cases on both sides: refused beside `hydrate` and beside `csr`
    for `wasm32-unknown-unknown` (11 refusals), and compiling on the host beside each, and in 1.x's
    workspace with a terminal UI, `-p leptos-mf2 -p mf2-ratatui --features leptos-mf2/csr` (6 host
    cases). *Negative control* (once, by hand): with `native`'s old `cfg` put back, it fails at the
    first `ratatui` case, whose errors are both sentences.
  * `msrv`'s server features name `mf2/ratatui`; `release` leaves `ratatui` out of `mf2`'s semver
    run, since 1.0.0's `mf2` has no such feature and `mf2-ratatui` was never published.
* **The root manifest.** `[workspace.dependencies]` loses the two shims' entries: no member names
  either through the workspace any more (the `mf2-ratatui` shim named `mf2-native`; nothing named
  `mf2-ratatui`), and the pinned nightly's cargo warned "unused workspace dependency" for each, 19
  times a run of `cargo xtask api` and of `docs-rs` (for `mf2-ratatui` since before B3: the
  browser-only refusal's `docs-rs` log has it). After: none (`cargo xtask docs-rs`, 0 such lines).
* **In the same commit:** 05 §9 and §9.1; the master plan's §4 (the tree, the `mf2-ratatui` and
  `mf2` rows) and §4.1; 19 §8; `plans/README.md`; the changelog; `mf2`'s crate docs, README and
  manifest; the shim's docs, README and manifest; `leptos-mf2`'s docs and README (1.x's workspace
  with a terminal UI compiles too); the root manifest's comment on `ratatui-core`; `api.txt`
  (`mf2`: the module's 14 lines and the feature list; `mf2-ratatui`: three re-exports) and
  `package.txt` (`mf2`, `mf2-ratatui`).

### The gates (19 §14, row 1): the web byte-identical

`bash probes/p10-b2/measure.sh b3-base` at `140a18a` before any change, then `… b3` with the
change, in the main tree: the size workloads' locks kept (`--keep`), the demos' restored from
`locks/base` both times. The workspace's lock moved only in dependency lists (`mf2` gains
`ratatui-core`; `mf2-ratatui` loses `mf2-native` and `ratatui-core`), no package version.

| Figure | Before | B3 | Gate |
|---|---:|---:|---|
| **B1**, fixed | 26,720 B gz | 26,720 | ±64 |
| **B5**, per site | 8.167 B gz | 8.167 | ±0.2 |
| whole app, 1,860 sites | 41,911 B gz | 41,911 | ambition 105,120 |
| the size workloads' 12 wasm files, and their 6 locks | | byte-identical | |
| `b5 --view`: fixed / per site, and its 12 wasm files | 24,634 / 10.5 | identical; byte-identical | |
| **B7**: catalog-bench's report; demo-csr's `dist/` (16 files) | | identical but the report's `unix_time`; byte-identical | byte-identical |
| the demos' shipped files, 30 (`bash probes/p10-b2/demo-hashes.sh b3-base b3`): demo-ssr's wasm and lazy chunk, demo-islands', demo-csr's | 753,393 / 315,627; 23,688 / 11,519; 197,543 / 85,566; 207,538 / 91,184 raw / gz | byte-identical but demo-ssr's `__wasm_split` loader | ±64 B gz |
| **B12** | | clean | clean (`bash bench/b12/check.sh`) |
| B1′ / B13 | | +0 B / 13,573 B avoided | +0 (`cargo xtask b12-generated`) |

The loader went `591006faae48` → `ed8d63b87a98`, over one wasm (`82338f4b3f4c`) both times. Two
rebuilds after the run with nothing changed (`touch src/lib.rs; cargo leptos build --release
--split --frontend-only` in `examples/demo-ssr`, twice) gave `27af43e4de73` and `ed794eaf907b`,
over the same wasm: as B2 found, the loader's hash varies between identical builds.

**Native, reported** (C5's gate, not B3's). `cargo xtask tui-gate --save-baseline
target/p10-b3/tui-base` at `140a18a`, then `cargo xtask tui-gate --baseline target/p10-b3/tui-base`
with the change: the four binaries alternating, 31 runs, timings under a load of 1.73. The
example's lock moved in dependency lists only, as the workspace's.

| Binary | Stripped (B), base → B3 | Allocations per frame (en / de / es / fr), both | Median µs per frame, base / B3 |
|---|---:|---|---:|
| `tui-mf2` | 1,965,672 → 1,965,432 (−240: `.text` −176, unwind tables −120, `.gcc_except_table` +96) | 1816 / 1815 / 1816 / 1817 | 284.5 / 282.9 (ranges 252–330, 254–313) |
| `tui-upstream`, which uses no MF2 | 1,400,544 → 1,400,640 (+96: `.gcc_except_table`) | 1517 / 1519 / 1518 / 1526 | 250.0 / 237.0 |

The bytes per frame are identical too. `tui-upstream` moves with what the rlibs linked beside it
hold, as B2 found.

### The checks (main tree, `afd3811`'s content)

| Check | Result | Command |
|---|---|---|
| `cargo xtask ci` | **pass**, with the two new `ratatui` steps (the moved 7 tests) and `refusals` at 11 + 6 | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
| `cargo xtask docs` | **pass**: 120 blocks on 10 pages, 9 applications; the native project without and with `tui`, over the shims | `CARGO_BUILD_JOBS=3 cargo xtask docs` |
| `docs-rs` | **pass**: 19 crates, no warnings | `CARGO_BUILD_JOBS=3 cargo xtask docs-rs` |
| `codegen-matrix` | **pass**: 18 combinations (8 server, 10 client); B6 clean | `… cargo xtask codegen-matrix` |
| `scenarios` | **pass**: S1–S4 wasm identical, S5 and S6 rebuilt | `… cargo xtask scenarios` |
| `msrv` | **pass**: the 20 on Rust 1.88, five steps; step 1 now with `mf2/ratatui` | `… cargo xtask msrv` |
| `churn` | **84/84** | `… cargo xtask churn` |
| the ledger | **unchanged**: `conformance-report`'s check in `ci` (612 tests, 612 entries); `conformance/ledger.toml` untouched | `cargo xtask ci` |
| the shim | **pass**: `names`, 2 tests; the negative control fails it | `cargo test -p mf2-ratatui` |
| the refusals | 11 refused, each with one sentence; 6 host combinations compile; the negative control fails at the first `ratatui` case | `cargo xtask refusals` |

Not run, as B3 cannot reach them: `leptos-0-8`, `l6-web`, `l7-web` and the six browser checks on
either Leptos line. Each builds `mf2` with a Leptos mode and never with `ratatui` (no package they
build depends on `mf2-ratatui`), so what they compile changed only in `cfg`s that are off there
and in comments; the demos' shipped wasm is byte-identical (above).

### Found along the way (routed)

* C8 (with G1): the `mf2-ratatui` shim re-exports `mf2::ratatui`'s `MarkupStyles`, `line` and
  `text`. When C8 takes them out of `mf2` (19 §8), the shim either keeps its own copy of the three
  until G1 deletes it, or goes then; `examples/tui` and the book's native page, which name the
  shim, move to 2.0 in C8 anyway.

## B4 — internal users name `mf2`: what was built

* **Where.** Commit `6f3a87b` on `main`, made in the main tree; commands ran there, one build at a
  time. The before figures were taken at `7cf9790`, before any change, in the same tree. Logs: the
  main tree's git-ignored `target/p10-b4/` (`before/` and `after/`: each check's log, the hashes of
  what it built, the lock files; `wasm/`: the clients compared below),
  `target/p10-b2/logs/b4-base/`, `…/b4-head/` and `…/b4-regen/` (the size runs), and
  `target/p10-b1/e2e/b4-{before,after}-leptos-0-{9,8}/` (the browser checks).
* **What moved.** Each user the row names writes the Leptos line on its `mf2` dependency
  (`features = ["leptos"]`, or `["leptos-0-8"]` for the two on 0.8) and its mode where it names
  Leptos's (`mf2/ssr`, `mf2/hydrate`, `mf2/csr`), and takes the layer from `mf2::leptos` and the
  call-site core from `mf2`, where it named `leptos-mf2` (and, for a `Setup`, 1.x's hidden
  `mf2::leptos_mf2`):
  * conformance: `conformance/` (layer L6 in `cargo test`: its features `leptos-0-9` and
    `leptos-0-8` name `mf2/leptos` and `mf2/leptos-0-8`, and its `mf2` gains `ssr`), `l6-web`,
    and `l7-web` with its four sets (`set-build/src/set.rs`);
  * `bench/churn`, the harness and its i18n crate;
  * `bench/fluent-ab/mf2` on 0.8 (the manifest, the entry point, the timing hooks), and the shell
    edit `cargo xtask fluent-ab` makes (`use mf2::leptos::{CatalogLinks, CatalogPreload,
    html_lang}`). Its server keeps `mf2-axum`, which D1 folds in;
  * the templates `tr-view` and `fluent-converted`: the dependency line, the modes and
    `support.rs`. `tr` named `mf2` alone already (it has no Leptos layer), so it is unchanged.
* **The xtask lists.** Besides `fluent_ab.rs`'s edit, the help and doc text that named
  `leptos-mf2` for `mf2`'s layer: `churn` (moved here), and `leptos-0-8` and its error variant
  (stale since B1). What still names a shim in `xtask/` stays, each for a reason: the published
  crates' lists (`packages.rs`, `msrv.rs`, `release.rs`; `api`, `docs-rs` and `package` through
  them) until G1 deletes the shims; the shims' own checks while they exist (`ci`'s `layer` test of
  `leptos-mf2`, `leptos-0-8`'s refusal through it, `refusals`' two 1.x workspaces); the book's
  crates (`docs.rs`'s `OUR_CRATES`) until C8 and D6. The row's exceptions stay on the shims too:
  the three demos, `examples/tui`, the book, and what `mf2 init` writes.
* **Locks.** Each moved only by losing `leptos-mf2`: its entry where nothing else names it, and the
  lines that listed it (`diff` against the copies in `target/p10-b4/before/`). The workspace's
  (`mf2-conformance`'s list), `l6-web`'s, `l7-web`'s, `bench/churn`'s, fluent-ab's `app-mf2`
  (where `mf2-axum` still names it), and each `tr-view` application's (`python3
  probes/p10-b4/lockback.py target/p10-b2/b5v/wl-view-1860 app-tr-view
  target/p10-b2/logs/b4-regen/wl-view-1860.before.sha256`, and the same at 3,720: the kept lock is
  the new one with those lines put back). No version moved.
* **Text.** 01 §3's layer table (L6 and L7 test `mf2`'s Leptos layer); the READMEs of
  `bench/churn`, `bench/fluent-ab` (its `mf2/` names `mf2` ahead of the migration guide: routed
  below) and `bench/workload-gen` (the template field `leptos`); `tools/e2e`'s README and two
  checks' comments; the nightly workflow's comments and one step's name; the comments in
  `leptos-mf2`'s `tests/layer.rs` and in `ci` that named the sets as the shim's example.
  `probes/p10-b4/` (below).

### The gates (the row, and 19 §14's first row)

**Sizes.** B4 changes templates, not library code. `--keep` reuses a kept workload as it stands,
so `probes/p10-b2/measure.sh` alone measures the applications generated before the change: an
after run that way (label `b4`) rebuilt them byte for byte, with nothing of B4 in them.
`probes/p10-b4/regen.sh` generates the kept workloads again in place, each application keeping its
`Cargo.lock` and build directory:
1. `bash probes/p10-b2/measure.sh b4-base` at `7cf9790`;
2. with `7cf9790`'s `tr-view` put back (`git show 7cf9790:FILE` over its two files), `bash
   probes/p10-b4/regen.sh b4-head`: every file of the four workloads as it was, so `b4-base`
   measured `7cf9790`'s templates;
3. with B4's, `bash probes/p10-b4/regen.sh b4-regen`: `app-tr-view`'s `Cargo.toml` and
   `src/support.rs` changed at both scales, nothing else; then `bash probes/p10-b2/measure.sh
   b4-regen`. The demos' locks restored from `locks/base` each time.

| Figure | Before `7cf9790` | B4 | Gate |
|---|---:|---:|---|
| **B1**, fixed | 26,720 B gz | 26,720 | ±64 |
| **B5**, per site | 8.167 B gz | 8.167 | ±0.2 |
| whole app, 1,860 sites | 41,911 B gz | 41,911 | ambition 105,120 |
| the size workloads' 12 wasm files (`tr`, `idlit`, `dummy`, both scales, before and after `wasm-opt`) and their 6 locks | | byte-identical | |
| `b5 --view`: fixed / per site | 24,634 / 10.524 | 24,636 / 10.520 | |
| `tr-view` after `wasm-opt`, raw / gz: 1,860 sites; 3,720 | 1,890,752 / 572,434; 3,375,126 / 956,530 | 1,890,752 / 572,429; 3,375,126 / 956,518 | |
| `idlit-view` and `dummy`'s 8 wasm files | | byte-identical | |
| **B7**: catalog-bench's report; demo-csr's `dist/` (16 files) | | identical but the report's `unix_time`; byte-identical | byte-identical |
| the demos' shipped files, 30 (`bash probes/p10-b2/demo-hashes.sh b4-base b4-regen`): demo-ssr's wasm and lazy chunk, demo-islands', demo-csr's | 753,393 / 315,627; 23,688 / 11,519; 197,543 / 85,566; 207,538 / 91,184 raw / gz | byte-identical but demo-ssr's `__wasm_split` loader | ±64 B gz |
| **B12** | | clean | clean (`bash bench/b12/check.sh`) |
| B1′ / B13 | | +0 B / 13,573 B avoided | +0 (`cargo xtask b12-generated`) |

The loader went `ed8d63b87a98` → `591006faae48` (`27af43e4de73` in the `b4` run), over one wasm:
as B2 found, its hash varies between identical builds.

**`tr-view`'s move, read** (`python3 probes/p10-b4/wasmcmp.py BEFORE AFTER`, both builds kept in
`target/p10-b4/wasm/`): the same length at both scales; every section byte-identical, the
code included, but the data section, which differs in 3,218 bytes at 1,860 sites and 6,411 at
3,720, its length the same. *Inference, brief:* tachys 0.3's `AnyView` records each view's
`TypeId`, a hash that takes in the crate defining the type; the view types are the application's,
and cargo derives its crate's identity from its dependency list, which lost `leptos-mf2`. gzip
reads the new values as −5 and −12 B.

**fluent-ab's mf2 client** (not a gate; `cargo xtask fluent-ab`'s application): 2,104,072 B raw
before and after; gz 614,291 → 614,293, brotli 422,143 → 422,186; its JS byte-identical; the
served `--split` main module 872,005 B raw both times. Rebuilt in the after tree with `7cf9790`'s
manifest, entry point and shell line (the same lock, `leptos-mf2` listed again), it gave the
before run's three hashes. Against the after build (`wasmcmp.py`; both kept in
`target/p10-b4/wasm/`): the three lazy routes' `wasm_split` import and export names carry another
32-digit hash, one function differs in three data addresses, and the data section in 4,517
bytes; the rest byte-identical. The same reading as `tr-view`'s.

**The ledger and the browser layers.**

| Check | Before and after | Command |
|---|---|---|
| the ledger | green both times: 612 tests, 612 entries, `current_phase = P9`; 164 statements, 0 gaps; `ledger.toml`, `REPORT.md` and `COVERAGE.md` unchanged | `cargo xtask conformance-report` (its default is the check) |
| `l6-web` | 20/20 both times; the page (which names each catalog by its content hash) and `page.json` byte-identical | `cargo xtask l6-web` |
| `l7-web` | 34/34 both times; L7 444/444, L7c 444/444, L7d and L7cd 325/444 (+119 documented degradations each); the ledger's L7 columns hold; the pages, catalogs and case lists byte-identical | `cargo xtask l7-web` |

The clients of `l6-web`, `l7-web` and `bench/churn` are not byte-identical; none is a size
workload, and they were not read further.

### The checks (main tree, `6f3a87b`'s content)

| Check | Result | Command |
|---|---|---|
| `cargo xtask ci` | **pass**: `refusals` 11 refused and 6 host cases, `docs --no-build` (120 blocks), the ledger, `api --check` and `package --check` (the 20 listings and file lists unchanged) | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
| `leptos-0-8` | **pass** before and after: both refusals; on 0.8, the five clippy steps, `render` 15, `time_zone` 8, `churn` 1, `fallback_lang` 8, `mf2-axum` 13 + 4, conformance `l6` 3 and `layers` 4 (the earlier records swap the last two) | `cargo xtask leptos-0-8` |
| `churn` | **84/84** before and after | `cargo xtask churn` |
| `fluent-migrate` | **pass** before and after: the report is the guide's hand-finishing; 64 files `fluent-converted`'s, byte for byte; the application finished from `fluent-converted`'s manifest and `support.rs` builds for the client and the server | `cargo xtask fluent-migrate` |
| `fluent-ab` | **pass** before and after: both applications built; the same text in Chromium and Firefox (every route in `en` and `pl`, 53,622 characters); sizes above. One run a side, so no timing is reported | `cargo xtask fluent-ab --browser chromium,firefox --runs 1` |
| e2e on 0.9 | before and after: demo 210/210, lazy 74/74, islands 58/58, csr 98/98, zone 40/40, a11y 720/720 | `bash probes/p10-b1/e2e.sh . b4-{before,after}-leptos-0-9` |
| e2e on 0.8 | the same six, the same counts, before and after; `cargo tree` shows leptos 0.8.21, tachys 0.2.19, reactive_graph 0.2.15 and `mf2-leptos-ui-0-8` only, in each server and client build of the copies | `python3 probes/p10-names/demos-0-8.py b4`, then `bash probes/p10-b1/e2e.sh target/a7-demo-0-8/b4 b4-{before,after}-leptos-0-8` |
| `examples/tui` | fresh (nothing to compile); its three binaries byte-identical (`tui-mf2` 1,965,432 B) | `CARGO_PROFILE_RELEASE_STRIP=symbols cargo build --release --bins --manifest-path examples/tui/Cargo.toml --target-dir target/tui-gate/target` |

Not run: the full `docs`, `docs-rs`, `codegen-matrix`, `scenarios` and `msrv`. B4 changes no
published crate's code, manifest or documentation (one test of `leptos-mf2` changes in a comment
only), no book page and no example; `ci` runs `docs --no-build`, `api --check` and `package
--check`.

### Found along the way (routed)

* D6 (with D1 and D5): `docs/migrating-from-leptos-fluent.md`, which D6's row does not name,
  finishes the migration on the shims (`leptos-mf2`, `leptos_mf2::install`, `use
  leptos_mf2::{…}`), and so do `mf2 convert --from leptos-fluent`'s `leptos-fluent-initializer`
  finding and 05 §6.2's table. Since B4 the same hand-finishing as `cargo xtask fluent-ab` and
  `fluent-migrate` apply it (`bench/fluent-ab/mf2`, `fluent-converted`) names `mf2` and
  `mf2::leptos`. The page, the finding and 05 §6.2 move together (D6; D1 for
  `mf2_axum::install`); `mf2 init`'s printed next steps name `leptos_mf2::install` too (D5, with
  its template's `mf2::leptos_mf2::Setup`).
* D1: `ci.rs`'s comments say `--workspace` turns `ssr` on in `mf2` through `mf2-axum`'s
  dependency on the `leptos-mf2` shim. Since B4 the conformance crate turns it on itself, so that
  holds whatever D1 makes of `mf2-axum`; the comments name it.
* Every size A/B that changes a template: `--keep` reuses the generated applications as they
  stand; `probes/p10-b4/regen.sh` generates them again in place, each keeping its lock.

## B5 — the API listed per mode: what was built

* **Where.** Commit `97b0cd4` on `main`, made in the main tree; commands ran there, one build at a
  time. Logs: the main tree's git-ignored `target/p10-b5/logs/`. No crate's source changed
  (`git diff --stat ccbdc9f 97b0cd4 -- 'crates/*/src'` is empty): the change is the xtask, `mf2`'s
  manifest metadata, the listings and `package.txt`, so no size was measured, and no book page or
  crate documentation changed.
* **The table**, `[package.metadata.api]` in `crates/mf2/Cargo.toml`:
  * `modes`: `core` is `compile`, `fn-number`, `datetime-icu`, `host-std` (the options docs.rs
    shows); `ssr`, `hydrate` and `csr` add `leptos`, the mode, `static-locale` and
    `mark-fallback-lang`; `native` and `ratatui` add their own.
  * `baseline."1.0.0"`: the four modes 1.0.0 had, as it needed them. `core` is the same; each
    Leptos mode names its line as `leptos-mf2/leptos-0-9` (1.0.0's own `leptos` was the layer,
    which a mode implied). It names no `native` and no `ratatui`.
  * `cargo xtask api` and `release` read it through `cargo metadata` and refuse what they would
    not follow (unit tests): another key, no mode, a mode that cannot name a file, a dependency's
    feature in a mode, a baseline naming a mode `modes` does not. The docs.rs table stays what
    docs.rs shows.
* **`cargo xtask api`** writes `crates/mf2/api/{core,csr,hydrate,native,ratatui,ssr}.txt` (312,
  738, 739, 370, 384, 741 lines) in place of `api.txt` (812 lines: `ssr` with `native` and
  `ratatui`); the other crates are listed as before. Each header names the mode. A file it would
  not write (an `api.txt` beside `api/`, a mode the table drops) is removed on writing and named
  by `--check` (shown once with both). Every listing is built twice on the pinned nightly, the
  second time documenting hidden items, and rustdoc's lints are capped (the first per-mode run
  printed 79 warnings, links to other modes' items). Two things the JSON leaves out are put back
  before public-api reads it:
  * **A public re-export of a hidden item:** rustdoc drops it with the item. Adding
    `#[doc(inline)]` to `islands_gate`'s re-export, tried in place and undone, gave byte-identical
    JSON. The listing puts back each such re-export the second build finds (the re-export not
    hidden, in a module not hidden, of a hidden item of the crate): `pub macro
    mf2::leptos::islands_gate!` in `ssr`, `hydrate` and `csr`. Only a macro is put back; another
    kind is refused with a sentence. No other crate has one: of the 19 other listings, only the
    shim's changed, by its glob.
  * **The shim's glob,** `pub use mf2::leptos::*`, listed before as `pub use
    leptos_mf2::<<mf2::leptos::*>>`: now one `pub use leptos_mf2::<name>` per name it brings (39).
    They come from `mf2`'s JSON built with the features the shim turns on in it (`cargo tree -p
    leptos-mf2 -e normal -i mf2 --depth 0 --format {f}`, with the listing's features:
    `default,fn-datetime,host-std,leptos,mark-fallback-lang,ssr,static-locale`), its hidden
    re-exports put back. A name of the shim's own shadows the glob's. The shim's listing now names
    all 45 names 1.x's listing (`2fb7f54`) had, and the six `*Props` aliases.
  * **What the listings hold** (as sets of lines, headers aside): `ssr` ∪ `native` ∪ `ratatui` is
    the old `api.txt` and `islands_gate!`. Over `core`, `ssr` adds 350 lines, `native` 57 and
    `ratatui` 71, and none drops one. Against `ssr`, `hydrate` has 14 lines more and 16 fewer, and
    `csr` 13 and 16: the client's boot and switch and `host_web`, against `RequestI18n` and the
    request's items, as `mf2::leptos`'s own table of the modes says.
  * `api --check` took 14 s at HEAD and takes 31 s (warm), for 25 listings where it had 20.
* **`cargo xtask release`** compares `mf2` mode by mode, and `SEMVER_WITHOUT` is gone.
  * A mode its baseline spelled as the tree does is cargo-semver-checks' own run, with
    `--current-features` and `--baseline-features`.
  * A mode 1.0.0 spelled with a dependency's feature cannot be: the tool writes a crate that
    depends on the one checked and passes it the features, and cargo refuses a dependency's feature
    there ("not allowed to contain slashes"). Both sides are then built as the tool builds its
    own: a crate that depends on `mf2` (and on `leptos-mf2` for the line), `cargo doc --no-deps`
    with the tool's flags, resolved afresh, in `target/semver-checks/mf2-modes/`. The tool then
    compares the two JSON files.
  * A mode the baseline did not have is skipped with a line.
  * `--baseline-rev` spells each mode as the tree does.

### The gate (the row)

| Step | Result | Command |
|---|---|---|
| At HEAD (`ccbdc9f`), `#[cfg(feature = "hydrate")] pub fn b5_negative_control() {}` added to `mf2::leptos` | exit 0, "20 listings unchanged": the item unseen | `cargo xtask api --check` |
| With B5, the same item and no listing | **exit 1**: `crates/mf2/api/hydrate.txt: + pub fn mf2::leptos::b5_negative_control()`, the one difference | `cargo xtask api --check` |
| The listing written | `api/hydrate.txt` +1 line; then exit 0, "25 listings unchanged" | `cargo xtask api`, then `--check` |
| The item removed | the listings as before (`diff -r`); exit 0 | `cargo xtask api`, then `--check` |

### Against 1.0.0 (`CARGO_BUILD_JOBS=3 cargo xtask release --allow-dirty`)

Run twice, the second time on the commit's code (after a last tidy of `release.rs`), with the same
results; warm, the second took 1 min 18 s. The names step passed ("1.1.0; after 1.0.0: every name
free or ours"); the semver step:

| Crate, mode | Result |
|---|---|
| `mf2`: `ssr`, `hydrate`, `csr` (built here) | no semver update required (196 checks) |
| `mf2`: `native`, `ratatui` | skipped: 1.0.0 has no such mode |
| `mf2`: `core` | **refused**: `feature_no_longer_enables_feature`, as `ssr`, `hydrate` and `csr` no longer turn on `leptos` (19 §3: a mode needs a line). Reported by `core`'s comparison, which the tool builds from the two manifests; the JSON comparisons are given none |
| `leptos-mf2` | **refused**: 6 lints (`declarative_macro_missing` for `islands_gate`, `enum_missing`, `function_missing`, `module_missing` for `components`, `struct_missing`, `trait_missing`). Every item is `mf2`'s now, re-exported, and the tool does not follow a re-export into another crate |
| `mf2-build` | **refused**: `enum_no_repr_variant_discriminant_changed`, as E1's `Lint::DroppedMarkup` at index 3 moved the later variants' values (and a `partial_ord_enum_variants_reordered` warning) |
| the 11 others | no semver update required; `mf2-macros` skipped (a proc-macro crate); the four never published not compared |

So the run stops there (exit 1). Each refusal is a change a minor may not make, and the tree is at
1.1.0; each was made before B5, and 2.0.0's major allows it (G1, G2). HEAD's release makes the
same three comparisons, with the same commands (for `mf2`, `core`'s features). *Control:* each of the three JSON
comparisons reversed, 1.1.0 as the baseline (`cargo-semver-checks semver-checks --package mf2
--baseline-rustdoc …/<mode>-current/rustdoc.json --current-rustdoc …/<mode>-baseline/rustdoc.json
--release-type minor`), is refused (exit 100) and names the mode's own items: `hydrate`'s
`hydrate_body`, `hydrate_lazy`, `hydrate_islands`, `wait_for_catalog`, `set_locale`; `ssr`'s
`RequestI18n`, `provide_locale`, `default_catalog`.

### The checks (main tree, `97b0cd4`'s content)

| Check | Result | Command |
|---|---|---|
| `cargo xtask ci` | **pass** (3 min 12 s, warm): `refusals` 11 refused and 6 host cases, `docs --no-build` (120 blocks), the ledger (612 tests, 612 entries), `api --check` 25 listings, `package --check` (`mf2`'s list: `api.txt` → the six) | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
| the xtask's tests | 18 pass: `api` 4 (the table, its refusals, finding a hidden item's re-export, the difference), `release` 12 (the spellings, the crate written for a spelled mode), `docs_rs` 2 | `cargo test -p xtask -- api:: release:: docs_rs::` |

Not run: `docs`, `docs-rs`, the sizes and the browser checks, as no crate source, book page or
crate documentation changed. `target/semver-checks/` (the release's builds) is removed.

### Found along the way (routed)

* G2: against 1.0.0, the semver step refuses `leptos-mf2`, `mf2`'s `core` and `mf2-build` at 1.1.0
  (above), each for a change made before B5 that 2.0.0's major allows. The shim's names are held
  by its test (`tests/layer.rs`) and its listing, not by the tool.
* G1: `docs/versioning.md` says each crate commits `api.txt`, and every listing's first line says
  "1.x promises" (`api.rs`'s `HEADER`); since B5 `mf2`'s listings are per mode, in `api/`.
* D1: `axum` joins `[package.metadata.api.modes]` (`api/axum.txt`), and not `baseline."1.0.0"`,
  since 1.0.0's `mf2` had no `axum`. A glob re-export of `mf2::axum` in the `mf2-axum` shim is
  listed name by name, as `leptos-mf2`'s is.
* D6: docs.rs shows `islands_gate!` neither in `mf2::leptos` (the module's link to it renders as
  plain text) nor on the `leptos-mf2` shim's page, as 1.x's did (`target/docs-rs`, B3's build of
  them). Through its glob, that page shows `mf2::leptos`'s hidden items instead (`installed`,
  `live_nodes`, `CatalogEntry`, the view states). D6 moves the book's and the islands demo's
  `leptos_mf2::islands_gate!()` to `mf2::leptos`; G1 deletes the shim.

## Part C — native and Ratatui (API work after A8's review; C1 and C2 after B1–B3; C3 before C4; C5 after C2 and C4; C6 after A2, A3 and C4; C7 after C6; C8 after C5 and C7; C9 after C8)

Each task builds what [19](19-native-and-terminal.md) designs, once the owner has reviewed it.
Where 19 refines a row below, 19 wins:
- C1 → §7: the `Display` step, and `IntoArg`'s list;
- C2 → §5, §6: `install()` returns nothing; `with_locale` and `Locale::format` work before it;
- C3 → §9;
- C4 → §10: `Locale::name()`, `best_match()`, the extractor, and the `clap` value parser in
  place of `ValueEnum`;
- C5 → §8: the theme's defaults;
- C6 → §11, §12;
- C7 → 05 §6.4;
- C8 → §1, §2.

| Task | Deliverable | Done when |
|---|---|---|
| **C1** Arguments | A trait `IntoArg` with `#[diagnostic::on_unimplemented]` naming the accepted types, **implemented per type** (a blanket over `Into` would bypass the message): <br>• integers up to 128 bits exact (past `i64`, an exact decimal written without `core::fmt`), and `usize` without saturation; <br>• `bool`, `Cow<'static, str>` (borrowed stays static); <br>• `Path` / `OsStr` / `SystemTime` under std; jiff's `Timestamp`, `Zoned` and civil types under `native`; <br>• signals over `T: IntoArg`; `ArgValue`; `&T` for `Copy` types. <br>The macro emits `IntoArg::into_arg(e)` spanned at the argument; `From` stays for `ArgValue::from`. The `&str` copy is measured, and inlined only if it pays and B5 holds | per-conversion tests through `compile_str`; a trybuild case whose `.stderr` shows the message pointing at the argument; `b5 --view` unchanged |
| **C2** The ambient store | `mf2::native` provides: <br>• `install(&'static Corpus)` (idempotent; a different corpus is an error) and `install_from_directory` (a partial set of files is accepted; only the source locale's is required); <br>• `set_locale`, `locale()`, `locale_source()`, and `with_locale` (restored by a guard); <br>• the time-zone and bidi settings, and `Catalogs`. <br>**The one ambient lookup** (A4's design). **`Display`**, the always-on `to_string` / `to_plain_string` / `to_cow` (borrowed for a simple message), and `Debug`, as A5 decided. **The system zone** by name, else one that follows the system's DST rules — never a frozen offset | parallel `with_locale` tests; `set_locale` seen from another thread on the next format; B10 through the ambient path; a `TZ=EST5EDT,M3.2.0,M11.1.0` test across DST; `tui-gate` against A1 |
| **C3** One matcher | **Data:** CLDR's `languageMatching` added to `cargo xtask cldr-sync`'s set (done: the data half), and `likelySubtags` (already vendored) used. UTS #35 Part 1's language-matching section fetched into the cache, never committed (question 16; done: the text half, `cargo xtask uts35-sync`). **Scope:** one matcher for native `set_locale`, `Locale::from_str`, web negotiation, and the client-only boot. **Rules:** POSIX names; exact; the script implied by likely subtags; region fallback; another script only where CLDR's data accepts it, with no project rule on top (question 15); the threshold, `oneway`, demotion and match-variable groupings as the specification's text states them (as read in "C3 (text half)", for A8 to state: a match below 50, the top of the text's range; demotion 5 per later entry; a paradigm wins a tie; a macroregion is in a variable when all its contents are). The client's table holds only the corpus's languages and is measured against B1 | a test table, each case with its reason: <br>• `zh-Hant-TW → zh-TW`, and `zh-HK → zh-TW` when there is no `zh-HK`; <br>• `zh-Hant ↛ zh-Hans` and back, `zh-TW ↛ zh-CN` (question 15): the reader's next listed language, else the source; `zh-TW, zh` → Simplified through its plain `zh`; <br>• `sr-Latn ↔ sr-Cyrl`; `pa-Arab ↛ pa-Guru`; <br>• `es-MX → es`, `es → es-MX`; <br>• `fr_CA.UTF-8`, `C`, `POSIX`; <br>• every case the current matchers pass, except those questions 11 and 15 change (e.g. `zh-Hant-TW` → an application's `zh`, today reached by truncation); <br>• UTS #35 Part 1's worked examples that the rules reproduce, each with its section (listed in "C3 (text half)") |
| **C4** The generated module | **`enum Locale`** (the build refuses variant-name collisions): `ALL`, `SOURCE`, `tag()`, `dir()`; `FromStr` through C3 (client-path code, since the wasm reaches it), whose error lists the supported locales; `Display`; `format(&impl Message)`; `clap::ValueEnum` under an optional `clap` feature. **Also:** `install()`, `set_locale(Locale)`, `with_locale`, `current_locale()`, and `markup::*` (a name hash per markup name the corpus uses); a prelude; doc comments that fit the mode (no wasm wording in a native module); compile-time choices through A2's cfg macros; one embedded byte table shared by `CATALOGS` and `CORPUS` when `ssr` and `native` are both on | unit tests in `crates/mf2-build/src/codegen.rs`; `codegen-matrix` with the native combinations; L5 unchanged; `scenarios` |
| **C5** Ratatui | **Conversions:** `From<Tr / TrArgs / TrRich / TrDyn>` for `Span`, `Line` and `Text`. Constant text is borrowed with no allocation; pattern text parts are borrowed as A4 decided; only placeholders allocate. **Traits:** `Styled` with `Item = Line<'static>`, so `.bold()` keeps a message's own markup; `Widget` for the descriptions. **Theme:** an app-wide `Theme` (markup-name hash → `Style`) with `set_theme` and a scoped `with_theme`, and defaults for `b` / `strong`, `i` / `em`, `u`, `s` / `del`, `code` / `kbd`. **Line breaks:** `Text` splits at a line break; `Line` and `Span` join with a space; a `Span` flattens markup — all documented. The old `line` / `text` / `MarkupStyles` go with C8's page | tests on a ratatui-core `Buffer` (text and styles); allocation tests; `Stylize` compiles; A7's coherence set with `leptos` and `ratatui` on |
| **C6** The build script | **`mf2_build::run()`** reads `mf2`'s features through `links` (else `CARGO_FEATURE_*`), picks what to emit, and prints `cargo::warning=` / `cargo::error=`; it exits non-zero on errors, and prints its rerun lines. **Compression** only for a web server, and no maximum-quality brotli in debug builds (review #18). **Checks:** a clear error when `datetime-icu` is on without `mf2-build`'s `icu-blob`; the single-crate layout with A3's `tr!`; `mf2 check` and `compile --site` read `mf2`'s node in the cargo resolve. **`mf2`** gains `links = "mf2-v2"` and a `build.rs` | A2's scenarios re-run on the real crates; `package --check`; `scenarios`; the edit-loop time with and without the opt-level tip, measured |
| **C7** `mf2 init` as a starter (native) | `mf2 init --cli` / `--tui` either creates a complete, runnable application, or adds translations to the current crate (`build.rs`, `locales/`, `cargo add mf2 -F native[,ratatui]`, `cargo add --build mf2-build`). It prints the `[profile.dev.build-override] opt-level = 2` tip, or writes it into a new application. `mf2 --help`'s summary names every command | the book runs `init` in `run=` blocks, so `cargo xtask docs` compiles what it makes; `mf2-cli` tests |
| **C8** The samples, the native book and the gates | `examples/tui` on 2.0. `tui-gate` becomes a gate: allocations in CI (deterministic); time nightly, alternating with the kept 1.x binary; sizes. `docs/native-apps.md` rewritten as three compiled projects (the one-file CLI, the TUI whose `main` reaches it, the two-crate workspace), added to `xtask/src/docs.rs`'s projects | `cargo xtask docs` (full); `tui-gate`; the native UX rows all fall |
| **C9** The trippy port as acceptance (the port stays untracked) | `vendor/trippy` ported to 2.0: <br>• the `thread_local` / `t!` wrapper deleted, `tr!` called directly; <br>• the messages made real MF2 (a `.match` plural instead of the `plural_flows` word; key hints as markup instead of the slicing hack; no `format!` word order); <br>• the language from `--tui-locale` through `Locale: FromStr`; <br>• upstream's 22 locale tests restored. <br>Recorded in 19 §"Prior art: trippy": call sites and keys, lines added and removed against upstream, stripped size and build time against upstream, each upstream bug class with the compile error that now catches it, and what did not fit | `cargo build` and `cargo test -p trippy-tui` in the checkout; a smoke run in a pseudo-terminal, as far as the machine allows; the record written |

## C1 — arguments: what was built

* **Where.** Commit `232f0ac` on `main`, made in the main tree; commands ran there, one build at a
  time. The before figures were taken at `ca373c9`, before any change: `bash
  probes/p10-b2/measure.sh c1-base` and `cargo xtask tui-gate --save-baseline
  target/p10-c1/tui-base`. Logs: the main tree's git-ignored `target/p10-c1/` and
  `target/p10-b2/logs/c1-base/`, `…/c1/` (the earlier forms' runs beside them: `c1-option`,
  `c1-probe-of`, `c1-closure-dyn`, `c1-pre-wide`), and `target/p10-b2/checks/`. Scripts:
  `probes/p10-c1/` (its `README.md`).
* **What was built** (19 §7, with its "As built", which C1 added):
  * **`mf2::IntoArg`** (`crates/mf2/src/into_arg.rs`), with `#[diagnostic::on_unimplemented]`
    naming what an argument may be, implemented per type: `i8`…`i128`, `u8`…`u128`, `isize`,
    `usize` and their `NonZero` forms (past `i64`, the exact decimal written digit by digit, no
    `core::fmt`; `usize` without saturation); `f32`, `f64`, `char` and 1.x's text types as 1.x's
    `From` made them; `bool` (the static string `true` or `false`); `Cow<'static, str>` (borrowed
    stays static); `&Path`, `PathBuf`, `&OsStr`, `OsString` (lossy) and `SystemTime` (an
    instant) wherever `mf2` links `std`; with `native`, jiff's `Timestamp`, `Zoned`,
    `civil::Date` and `civil::DateTime`; `DateTimeValue`, the runtime's `DateTime`, `ArgValue`
    itself and `Arc<C: CustomValue>`; `&T` for `T: IntoArg + Copy`. `From<&PathBuf>`,
    `From<&OsString>` and, with `native`, `From<&Zoned>` join 1.x's `From<&String>` for the
    references that cannot also be an `IntoArg` (E0119 beside the `&T` rule).
  * **Signals** (`mf2::leptos`): `IntoArg` for the eight signal types over any `T: IntoArg`,
    through a private twin of `SignalArg`; `SignalArg`, `signal_arg` and the `From` impls keep 1.x's
    `Into<ArgValue>`.
  * **The dispatch**, `mf2::__arg` (hidden, and listed with `tr!`'s other hidden items in
    `docs/versioning.md`): four steps, `IntoArg`, then 1.x's `From<T> for ArgValue`, then
    `Display`'s text (made when the description is built), then `IntoArg`'s message. `tr!`
    (`crates/mf2-macros/src/expand.rs`) emits `convert(e, |p| (&&&p).__mf2_kind())` for every
    argument but a string literal, with the kind traits imported in a block around the
    description; its tokens are `mixed_site`, located at the argument, and the method a refused
    type fails at carries the argument's own span.
  * `extern crate std` also under `host-std`, which links it through `mf2-host-std` anyway.
  * **In the same commit:** 19 §7 ("As built") and §16; 04 §2 and §2.1; 05 §9; the changelog;
    `docs/call-sites.md`'s table of argument types; `docs/versioning.md`'s hidden items; `mf2`'s
    six API listings (98–114 lines each) and `package.txt`; the tests (below); `probes/p10-c1/`.
* **Choices within the design** (none an owner question; each in 19 §7's "As built"): the second
  step, 1.x's `From`, which keeps an application's own `impl From<X> for ArgValue` and generic
  code bounded `where ArgValue: From<T>` compiling (19 §14's gate "every 1.x argument type still
  accepted"; three steps would have refused them, or turned them into their `Display` text);
  references to non-`Copy` types through `From`; a `Cow` shorter than `'static` refused by the
  borrow checker, as 1.x refused every `Cow`; `civil::Time` its text, since a date/time value
  always has a date; instants floored to the millisecond, `Unset` past a `Date`'s years; the span,
  one token on stable.
* **The dispatch, as measured.** 19's three steps need the value by value at step 1 and a fourth
  receiver type for step 2, which needs it too. Three forms were built and measured with `bash
  probes/p10-b2/measure.sh`, `bash probes/p10-c1/named.sh` (names kept, before wasm-bindgen and
  wasm-opt) and `bash probes/p10-c1/ab.sh` (the shipped modules at both scales, kept):

  | Form | `tr-view` opt raw, 1,860 / 3,720 sites | What changed (observed) |
  |---|---:|---|
  | the value in an `Option` behind a `Deref`, so that step 2 can move it out of `&mut` | +7,501 / +15,644 (`b5 --view` 11.52 B gz a site, +1.0) | 40 of 60 components +60…+338 B: a check of the `Option<String>`'s niche at each `String` argument (a load of the capacity and a compare, in `__component_c_039`). Inference: LLVM cannot prove a returned `String`'s capacity is not the niche |
  | the kind picked on a probe of `&value`, the value bound by a `match` | +196 / +220 (gzip +530 / −91, brotli −125 / +216; `tr` +86 / +92) | 14 components ±5 B before wasm-opt: stack-slot offsets, the same instructions; after it, 4 functions |
  | **kept:** the kind picked in a closure given the probe, the value passed straight into `convert` | **±0 / ±0** | names kept: no function's size moves. Shipped: every section the same length; 173 and 309 bodies differ by a few bytes (call targets, table indices, and pairs of bodies that traded places), and the element section |

  demo-islands moved +79 B raw under the first two forms, for two other reasons, both removed
  (observed with `named.sh demo-islands`): the signal read shared through a generic helper that
  was not inlined (`read::<RwSignal<i32>>`, 153 B), now written out in each `ArgSource` as 1.x's
  was; and `String::push`, 49 B with HEAD and 100 B with a non-generic `display(&dyn Display)`.
  Inference: that function's `to_string` gave `mf2`'s own copy of `push` a caller with any `char`.
  `display` is generic now, so nothing of it is compiled where no argument takes the step.
* **The web** (`bash probes/p10-b2/measure.sh c1-base` before, `… c1` after, in one tree, the size
  workloads' and the demos' locks kept; figures as the commands print them):

  | Figure | Before `ca373c9` | C1 | Gate |
  |---|---:|---:|---|
  | **B1**, fixed | 26,720 B gz | 26,728 | ±64 |
  | **B5**, per site | 8.167 B gz | 8.160 | ±0.2 |
  | whole app, 1,860 sites | 41,911 B gz | 41,905 | ambition 105,120 |
  | `tr` opt raw, 1,860 / 3,720 | 2,419,615 / 4,455,948 | the same; 37 and 88 bodies differ as `tr-view`'s do | |
  | **`b5 --view`**: fixed / per site | 24,636 / 10.520 | 24,682 / 10.502 | unchanged (19 §14) |
  | `tr-view` opt raw, 1,860 / 3,720 | 1,890,752 / 3,375,126 | the same (above) | |
  | `idlit`, `idlit-view`, `dummy` | | byte-identical | |
  | **B7**: catalog-bench's report; demo-csr's catalogs | | identical but `unix_time`; byte-identical | byte-identical |
  | demo-ssr's wasm, raw / gz / br | 753,393 / 315,627 / 250,981 | 753,393 / 315,623 / 250,993; 12 bodies and the element section differ in 1–2 bytes | ±64 B gz |
  | its lazy chunk | 23,688 / 11,519 / 9,960 | 23,688 / 11,519 / 9,970; 2 bodies, 1–2 bytes | |
  | demo-islands' wasm | 197,543 / 85,566 / 72,415 | 197,543 / 85,567 / 72,429; 2 data bytes | ±64 B gz |
  | demo-csr's wasm and JS | 207,538 / 91,184 / 77,523 | byte-identical (trunk names both files with another hash, so `index.html` differs) | |
  | **B12** | | clean | clean (`bash bench/b12/check.sh`) |
  | B1′ / B13 | | +0 B / 13,573 B avoided | +0 (`cargo xtask b12-generated`) |

  Inference, brief: the renumbering comes from the closures' instances; demo-islands' two data
  bytes are a type's identity, `SignalIntoArg` in place of `SignalArg`, as B4 read tachys'
  `TypeId`s.
* **The `&str` copy (19 §7):** an inline short string, a hidden `Text::Inline` of up to 22 bytes
  (10 on `wasm32`) that `IntoArg for &str` and `char` fill (`probes/p10-c1/inline-str.patch`, `bash
  probes/p10-c1/inline.sh`, against C1): `tui-mf2` 10 fewer allocations a frame (1,806 of 1,816),
  288 fewer bytes, 1,168 B smaller stripped, and 367.5 against 366.3 µs a frame (load 5.28); on
  the web every client +500 B raw fixed (`tr` +534, `tr-view` +502 at both scales; B1 26,983 B gz,
  B5 8.176, `b5 --view` 24,866 / 10.537). **Not kept:** it saves 0.55 % of a frame's allocations
  and no time the machine can measure, costs every web client, and a new variant of `Text`, whose
  variants 1.x code may match exhaustively, would break the promise that a 1.x application
  compiles unchanged.
* **Native** (`cargo xtask tui-gate --baseline target/p10-c1/tui-base --save-baseline
  target/p10-c1/tui-c1`; four binaries alternating, 31 runs, load 3.14):

  | Binary | Stripped (B), before → C1 | Allocations per frame (en / de / es / fr), both | Median µs, before / C1 |
  |---|---:|---|---:|
  | `tui-mf2` | 1,965,432 → 1,967,192 (+1,760: `.text` +1,184, `.gcc_except_table` +364, `.eh_frame` +168) | 1816 / 1815 / 1816 / 1817 | 295.6 / 309.6 (ranges 261–372, 264–338) |
  | `tui-upstream`, which uses no MF2 | 1,400,640 → 1,400,864 (+224) | 1517 / 1519 / 1518 / 1526 | 269.4 / 262.1 |

  Bytes per frame identical too; the medians move both ways within the ranges, as `tui-upstream`'s
  do. The +1,760 B, observed (`bash probes/p10-c1/tui-named.sh`, symbols kept): `exact`, the
  decimal writer, 458 B; `draw` +680, its `usize` arguments now a call; `<usize as
  IntoArg>::into_arg` and `<u64 …>` 42 B each. The exact path is 19's (`usize` without
  saturation), and on a 64-bit target a `usize` may take it. Of three placements measured, the
  conversions out of line cost least: inline, +1,618 B of symbols (`draw` +1,064); a cold shared
  call, +1,741; out of line, sharing `exact`, +1,239 (kept). On `wasm32` every `usize` fits `i64`,
  and nothing of it is linked.
* **The gates** (C1's row and 19 §14's):

  | Gate | Result | Command |
  |---|---|---|
  | `b5 --view` unchanged | the shipped modules the same lengths at both scales, the code HEAD's (above); the printed fixed / per-site figures move +46 B gz and −0.018 B | `bash probes/p10-b2/measure.sh c1`; `bash probes/p10-c1/ab.sh` |
  | every 1.x argument type still accepted | 1.x's types by `IntoArg` (their `From`, inlined); an application's own `From` and generic code bounded on it by step 2 (`tests/arguments.rs`); the L5 suite's `ArgValue`s, the fixture, the book's projects, `examples/tui`, the demos and every workload compile unchanged | `cargo xtask ci`, `docs`, `tui-gate`, the size runs |
  | the message at the argument | `tools/i18n-fixture/tests/ui/not_an_argument.stderr`: "`Opaque` is not a message argument" under `Opaque`, and under `Some` of `Some(3)`, with the list of what an argument may be; no "originates in the macro" note | `cargo test -p mf2-i18n-fixture --test ui` |
  | each `IntoArg` type exact, through `compile_str` | `tests/arguments.rs` 10 tests (integers at each type's limits, past `i64` formatted and selected exactly; `bool` selected by `.match`; text kept, shared or copied; paths, lossy; `SystemTime` floored, and `Unset` past the years; dates and application values; each dispatch step, generic code included; a `Display` text made once); `tests/native.rs` (jiff); `tests/render.rs` (signals of `u64` and `bool`, read when formatted); the fixture's `a_call_site_converts_each_argument_by_its_type`, through the real macro | `cargo xtask ci` |

* **The checks** (main tree, `232f0ac`'s content):

  | Check | Result | Command |
  |---|---|---|
  | `cargo xtask ci` | **pass**: `refusals` 11 refused and 6 host cases; `docs --no-build` (120 blocks); the ledger (612 tests, 612 entries, `current_phase = P9`; 164 statements, 0 gaps; `REPORT.md` and `COVERAGE.md` unchanged); `api --check` (25 listings, `mf2`'s six written by `cargo xtask api`: 98–114 lines each, `IntoArg` and its impls); `package --check` (`mf2`'s list gains `src/__arg.rs`, `src/into_arg.rs`, `tests/arguments.rs`) | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
  | `docs` | **pass**: every sample compiled | `bash probes/p10-b2/checks.sh docs` |
  | `docs-rs` | **pass**: 19 crates, no warnings | `… docs-rs` |
  | `codegen-matrix` | **pass**: 18 combinations (8 server, 10 client); B6 clean | `… codegen-matrix` |
  | `scenarios` | **pass**: S1–S4 wasm identical, S5 and S6 rebuilt | `… scenarios` |
  | `msrv` | **pass**: the 20 on Rust 1.88, five steps | `… msrv` |
  | `leptos-0-8` | **pass**: both refusals; on 0.8, the five clippy steps, `render` 16 (the signal test is C1's), `time_zone` 8, `churn` 1, `fallback_lang` 8, `mf2-axum` 13 + 4, conformance `l6` 3 and `layers` 4 | `… leptos-0-8` |
  | `churn` | **84/84**; every row's live bytes and allocations as the last run's (the `signal` row 614.58 B, 19.03 allocations a row) | `… churn` |
  | `l6-web` | **20/20** | `… l6-web` |
  | `l7-web` | **34/34**; L7 444/444, L7c 444/444, L7d and L7cd 325/444 (+119 documented degradations each); the ledger's L7 columns hold | `… l7-web` |
  | B12, B1′ / B13 | above | `… b12`, `… b12-generated` |

  Not run: the six browser checks on either Leptos line (not on this task's list): the demos'
  shipped modules differ from HEAD's only in function numbering and two data bytes, and demo-csr's
  not at all; `l6-web` and `l7-web` render the layer's arguments in browsers.
* **Found along the way (routed).**
  * C2: `tui-mf2` 1,967,192 B stripped (+1,760, above), against C2's gate "≤ 1,965,320 B" (19 §14;
    HEAD was 1,965,432).
  * F: `IntoArg` for an application's own types; a `Cow` shorter than `'static`; `civil::Time` as
    text; `SystemTime` and jiff's instants in place of `DateTimeValue::instant`'s `Option` (the UX
    review's finding 9, whose `instant` keeps 1.x's signature). `docs/call-sites.md`'s table of
    argument types is C1's; the teaching is F's.
  * G2: `ArgValue::from(usize)` saturates (1.x's, kept), `IntoArg` does not.
  * Not scheduled: the refusal's note naming the hidden `KindNeither::__mf2_kind`; an underline of
    the whole argument needs `Span::join` (nightly).

## C2 — the ambient store: what was built

* **Where.** Commit `c5b602e` on `main`, made in the main tree; commands ran there, one build at a
  time. The before figures were taken at `ac11c9d`, before any change: the six browser checks on
  both Leptos lines (C1 had skipped them), `bash probes/p10-b2/measure.sh c2-base`, `cargo xtask
  tui-gate --baseline target/p10-baseline --save-baseline target/p10-c2/tui-head`, and B10's 1.x
  rows with the crates as HEAD has them (`bash probes/p10-c2/at-head.sh cargo run --release -p
  runtime-bench -- b10 …`). Logs: the main tree's git-ignored `target/p10-c2/`,
  `target/p10-b2/logs/c2-base/` and `…/c2c/` (the two earlier after-runs, `c2` and `c2b`, beside
  them), and `target/p10-b1/e2e/c2-*`. Scripts: `probes/p10-c2/` (its `README.md`).
* **The browser checks at HEAD** (`bash probes/p10-b1/e2e.sh . c2-head-leptos-0-9`; on 0.8
  `python3 probes/p10-names/demos-0-8.py c2-head`, B2's copies' build directories moved in, then
  `bash probes/p10-b1/e2e.sh target/a7-demo-0-8/c2-head c2-head-leptos-0-8`): green on both lines,
  demo 210/210, lazy 74/74, islands 58/58, csr 98/98, zone 40/40, a11y 720/720. Nothing of C1's.
* **What was built** (19 §5 and §6, with §5's "As built", which C2 added):
  * **The store** (`crates/mf2/src/native/store.rs`), A4's shape: a `Catalogs` in a `OnceLock`,
    each catalog living as long as the process; the app-wide language and where it came from in one
    atomic; a thread's language in one thread-local cell, restored by a guard; the settings (bidi,
    time zone) behind an `RwLock` with a generation counter, copied per thread. `install(&CORPUS)`
    returns nothing and panics naming the catalog, or the second corpus; `install_from_directory`
    returns `Result<(), mf2::native::Error>` and takes a partial set (the source language's file
    required); `set_locale`, `locale()`, `locale_source()`, `with_locale(&CORPUS, locale, body)`
    (before `install` too), `set_bidi` / `bidi`, `set_time_zone` / `time_zone`.
  * **`Catalogs`** (`native/catalogs.rs`), the explicit form: `embedded`, `from_directory`
    (partial), `format(locale, &message)`, `format_with_errors`, `formatter`, `locales`, its own
    bidi and zone. **`Error`** is 1.x's `NativeError`, renamed, with `AnotherCorpus`. **1.x's
    `NativeI18n`** (`native/handle.rs`) is built on `Catalogs` with 1.x's API and rules (every file
    required), kept visible beside them, as B3 kept `line` / `text`. The `mf2-native` shim
    re-exports both under 1.x's names; its listing is unchanged.
  * **The ambient forms** (`display.rs`): with `native`, `to_string()`, `to_plain_string()`,
    `to_cow()`, `From<_> for String` and `Display` (S3: `f.pad(&to_cow())`) on the four
    descriptions, each through one function taking a trait object of the two methods the text needs.
    **The one lookup:** beside a Leptos mode the request's context (`ssr`) or the page's catalog
    (`hydrate`, `csr`) first, then the store, then the web's rule; a native-only build panics before
    `install`, naming it. A build with a Leptos mode and no `native` compiles the web's text path as
    before; `to_cow()` joins its forms (owned there).
  * **The system zone** (`native/zone.rs`): jiff's, by its IANA name; else the POSIX rule it follows
    — `TZ`'s, or the one its TZif file ends with — as `mf2_runtime::TimeZone::rules`, which
    `mf2-host-std`'s `zone_offset` evaluates with jiff after its database; else UTC. `NativeI18n`
    gets the same zone.
  * **`Debug`** on the runtime's 13 public types that lacked it and the catalog's 18 (hand-written
    where a field has none, or to show counters rather than a byte table), and
    `missing_debug_implementations` warns in both crates.
  * **Tests:** `crates/mf2/tests/ambient.rs` (16 pinned threads × 2,000 rounds while another thread
    changes the app-wide language, bidi and zone; `set_locale` and the settings seen by another
    thread's next format; `with_locale` nested and restored when its body unwinds; `to_cow`
    borrowed from the embedded bytes; `Display` padded; another corpus refused; `Catalogs`, a
    partial set), `uninstalled.rs`, `from_directory.rs`, `system_zone.rs` (a child process with
    `TZ=EST5EDT,M3.2.0,M11.1.0`), `lookup.rs` (beside `ssr` and beside `csr`), and a corpus written
    as `mf2-build` writes one (`tests/support/corpus.rs`). `ci` runs them with `native` alone,
    beside `csr`, and (`--workspace`) beside `ssr`.
  * **In the same commit:** 19 §5 ("As built") and §16; 03 §6 (`TimeZone::rules`); 05 §9; D17's
    status; the changelog; `docs/native-apps.md`'s sentence on the zone; `mf2`'s crate docs and the
    shim's README; `mf2`'s five mode listings and `mf2-runtime`'s; `mf2`'s `package.txt`; B10's
    harness (`bench/runtime-bench`, rows below); `probes/p10-c2/`.
* **Choices within the design** (none an owner question; each in 19 §5's "As built"):
  `with_locale` takes the corpus, which is what lets it load before `install`; `set_locale` needs
  `install`; the rule zone rides in the named zone's buffer, marked in its last byte (below: its
  own variant cost the web); `NativeI18n` visible (hidden, the shim's listing lost it: rustdoc drops
  a re-export of another crate's hidden item); `to_display_string()` stays with the Leptos modes;
  `Catalogs::format` in a language it lacks is the source's; the parts seam is C5's.
* **Native** (`cargo xtask tui-gate --baseline target/p10-baseline --save-baseline
  target/p10-c2/tui-c2`, then `… --baseline target/p10-c2/tui-head`, each again with `--runs 61`;
  four binaries alternating; `examples/tui` is still on 1.x's API through the shims, so this is
  1.x's `NativeI18n::format` and `mf2::ratatui::line` over C2's `Catalogs`):

  | Binary | Stripped (B): 1.x (A1) / HEAD → C2 | Allocations per frame (en / de / es / fr), all | Median µs per frame, C2 / baseline, **under load** |
  |---|---:|---|---:|
  | `tui-mf2` | 1,965,320 / 1,967,192 → **1,804,776** | 1816 / 1815 / 1816 / 1817 | against 1.x: 276.4 / 275.1 (31 runs, load 1.51), 296.0 / 290.7 (61, 1.00); against HEAD: 280.7 / 276.0 (31, 1.43), 272.4 / 268.5 (61, 1.30) |
  | `tui-upstream`, which uses no MF2 | 1,390,784 / 1,400,864 → 1,401,856 | 1517 / 1519 / 1518 / 1526 | the same runs: 243.1 / 243.3, 252.8 / 253.3; 254.8 / 245.9, 245.1 / 252.6 |

  The time: C2's median is 0.5–1.8 % above the baseline's in the four runs of the final code (two
  runs before the runtime's last two changes, which the TUI's path does not reach: 0.0 % against
  1.x, −2.8 % against HEAD), while `tui-upstream`,
  whose code is the same in all its builds, moved −3.0 % to +3.6 % between two of them in the
  same runs; its ranges overlap throughout. The one change in the TUI's path, `NativeI18n::format`
  through `Catalogs`' trait-object text path, measured no faster monomorphized as 1.x's was (294.7
  against 293.7 µs, 61 runs, `target/p10-c2/tui-generic-vs-dyn.log`), and 15 KB larger. This
  machine does not resolve a difference from 1.x's time.

  The −160,544 B, observed (`bash probes/p10-c2/tui-named.sh build head|c2`, symbols kept, then
  `diff head c2`; `size -A`): `.text` −77,344, `.rodata` −48,680, `.eh_frame` −13,472, `.rela.dyn`
  −10,632, `.data.rel.ro` −8,696; by symbol, jiff's −67,978 B (its error types' `Display` and
  `Debug`, its temporal and POSIX printers), core's float formatting −5,222, and
  `jiff::Timestamp::now` gone; `mf2`'s own +2,079 (`native::zone::read` 1,770 where 1.x's
  `system_time_zone` was 608, `Catalogs::system_choice` 539). *Inference:* 1.x froze the
  zone at `to_offset(Timestamp::now())`, and `now()`'s panic message formats a jiff error, which
  linked jiff's error formatting; the rule-following zone never asks for the offset now. Every
  native application gets it, the book's CLI too.
* **B10 through the ambient path** (`cargo run --release -p runtime-bench -- b10 --gate --md
  target/p10-c2/b10-final.md --json …`, C2's rows beside B10's own; `native-*` is 1.x's
  `NativeI18n::format`, whose counts at HEAD's code are the same, `target/p10-c2/b10-head.md`):

  | Row (`en`; pl, en-XA, ar-XB alike) | Allocations / op | Median ns, **under load** (1.3–1.4) |
  |---|---:|---:|
  | 1.x `NativeI18n::format`, simple / 1-argument | 1.000 / 2.271 (pl 2.235, en-XA 2.724, ar-XB 2.271) | 42.4 / 205.1 |
  | ambient `to_cow`, simple (borrowed) | **0.000** | 33.2 |
  | ambient `to_string`, simple | 1.000 | 46.0 |
  | ambient `{}` into a reused `String`, simple | **0.000** | 47.5 |
  | ambient `to_string`, 1-argument | **1.000** | 201.4 |
  | ambient `{}` into a reused `String`, 1-argument | **1.000** | 223.2 |

  The gate (≤ 1.x, and B10's 0 / ≤ 1) holds in all four locales, and the harness now checks it
  (`--gate`). A 1-argument `to_string` through the store took 0.98 × 1.x's handle's time here and
  1.07 × in an earlier run (`b10-c2.md`): no difference the load lets through. `--gate` exits 1 on
  B10's older rule "P0.8 not lost" for two of the runtime's own `write` rows, as at HEAD
  (`b10-head.md`) and as A1 found: time under load, not C2's.
* **The lookup's first step** (A4 left it to C2; `bash probes/p10-c2/lookup.sh 21 51`: the store
  installed, nothing in a request, built with `native` alone and with `ssr` unified beside it, the
  two binaries alternating; median of 21 runs, **under load** 2.3–2.6; an earlier run, at load
  1.8–2.1, gave +5.9, +7.1, +13.3 and +4.8):

  | Row | `native` ns | `ssr` + `native` ns | Step 1 |
  |---|---:|---:|---:|
  | simple `to_cow` | 24.53 | 31.21 | +6.7 |
  | simple `to_string` | 38.53 | 44.77 | +6.2 |
  | 1-argument `to_string` | 186.54 | 200.26 | +13.7 |
  | 1-argument `{}` | 201.57 | 207.43 | +5.9 |

  About 6–7 ns a format (the reactive owner looked up, none found): 0.7 µs of A4's 120 µs frame.
* **The web** (`bash probes/p10-b2/measure.sh c2-base` before, `… c2c` after, one tree, the
  workloads' and the demos' locks kept, identical; `bash probes/p10-b2/demo-hashes.sh c2-base c2c`):

  | Figure | Before `ac11c9d` | C2 | Gate |
  |---|---:|---:|---|
  | **B1**, fixed | 26,728 B gz | 26,733 | ±64 |
  | **B5**, per site | 8.2 B gz | 8.2 | ±0.2 |
  | whole app, 1,860 sites | 41,905 B gz | 41,908 | ambition 105,120 |
  | `tr` opt raw, 1,860 / 3,720 | 2,419,615 / 4,455,948 | 2,419,614 / 4,455,947 | |
  | `b5 --view`: fixed / per site | 24,682 / 10.5 | 24,678 / 10.5 | |
  | `tr-view` opt raw, 1,860 / 3,720 | 1,890,752 / 3,375,126 | 1,890,751 / 3,375,125 | |
  | `idlit`, `idlit-view`, `dummy` | | byte-identical | |
  | **B7**: catalog-bench's report; demo-csr's catalogs | | identical but `unix_time`; byte-identical | byte-identical |
  | demo-ssr's wasm, raw / gz / br | 753,393 / 315,623 / 250,993 | 753,393 / 315,613 / 250,983 | ±64 B gz |
  | its lazy chunk | 23,688 / 11,519 / 9,970 | 23,688 / 11,520 / 9,971 | |
  | demo-islands' wasm | 197,543 / 85,567 / 72,429 | 197,542 / 85,568 / 72,379 | ±64 B gz |
  | demo-csr's wasm | 207,538 / 91,184 / 77,523 | 207,537 / 91,186 / 77,638 | |
  | **B12** | | clean | clean (`bash bench/b12/check.sh`) |
  | B1′ / B13 | | +0 B / 13,573 B avoided | +0 (`cargo xtask b12-generated`) |

  What moved, observed on `tr` at 1,860 sites built both ways in the kept workload (`bash
  probes/p10-c2/at-head.sh cargo xtask size --out target/p10-b2/size --keep`, then the same with
  C2's sources; `python3 probes/p10-b4/wasmcmp.py`): every section the same length but the data
  section (−1 B after `wasm-opt`); two function bodies exchanged places, 7 others differ in 1–6
  bytes (their call targets), and the element section's and the data's table indices with them.
  The names-kept
  builds (`bash probes/p10-c2/named.sh tr`, `… demo-ssr`) show no function whose size moved.
  *Inference:* the runtime's new items change the order LLVM emits two functions in. Two earlier
  forms moved code, both undone (`c2`, `c2b`): `Options`' `Debug` through its `iter()` took that
  iterator out of line in every client (+172 B raw), and `impl Debug for Digits` placed beside
  `Digits` made LLVM inline `OperandsBuilder::finish` (−5 B); the rule zone as a variant of its
  own cost demo-ssr +154 B of code (a derived `PartialEq` over two buffers). The runtime's `Debug`
  impls are now written apart from the client's helpers and placed after them.
* **The gates** (C2's row and 19 §14's):

  | Gate | Result | Command |
  |---|---|---|
  | time per frame ≤ 1.x | not resolved from 1.x's: C2 +0.5–1.8 % in four alternating runs where the unchanged `tui-upstream` moved −3.0 to +3.6 %; the one change in its path measured no faster undone (above) | `cargo xtask tui-gate --baseline target/p10-baseline`, `… target/p10-c2/tui-head` |
  | stripped `tui-mf2` ≤ 1,965,320 B | **met**: 1,804,776 B (−160,544 against 1.x, −162,416 against HEAD); allocations per frame identical | same |
  | B10 allocations ≤ 1.x | 0 / 1 / 1 where 1.x takes 1 / 2.24–2.72 (above), all four locales | `runtime-bench -- b10 --gate` |
  | parallel `with_locale`; `set_locale` seen from another thread | pass | `tests/ambient.rs` |
  | a `TZ=EST5EDT,M3.2.0,M11.1.0` test across DST | pass: 09:30 in January and July, 01:30 and 03:30 either side of 2026-03-08's change; a negative control (the offset frozen at −4 h) fails it at January's 10:30 | `tests/system_zone.rs` |
  | the web | within every budget; the moves above | `measure.sh`, `demo-hashes.sh` |

* **The checks** (main tree, `c5b602e`'s content):

  | Check | Result | Command |
  |---|---|---|
  | `cargo xtask ci` | **pass**, with C2's steps (`native` alone: `ambient`, `uninstalled`, `from_directory`, `system_zone`; beside `csr`: clippy, `lookup`, `ambient`); `refusals` 11 refused and 6 host cases; `docs --no-build` (120 blocks); the ledger (612 tests, 612 entries, `current_phase = P9`; 164 statements, 0 gaps; `REPORT.md` and `COVERAGE.md` unchanged); `api --check` (25 listings; `mf2`'s five mode listings and `mf2-runtime`'s rewritten by `cargo xtask api`, the shims' unchanged); `package --check` (`mf2`'s list gains the four `native/` files and the five tests with their support file) | `CARGO_BUILD_JOBS=3 cargo xtask ci` |
  | `docs` | **pass**: every sample compiled, the native project with and without `tui` | `bash probes/p10-c2/checks.sh docs` |
  | `docs-rs` | **pass**: 19 crates, no warnings | `… docs-rs` |
  | `codegen-matrix` | **pass**: 18 combinations (8 server, 10 client); B6 clean | `… codegen-matrix` |
  | `scenarios` | **pass**: S1–S2 identical, S3–S6 rebuilt what each changes, as before | `… scenarios` |
  | `msrv` | **pass**: the 20 on Rust 1.88, five steps | `… msrv` |
  | `leptos-0-8` | **pass**: both refusals; on 0.8, the five clippy steps, `render` 16, `time_zone` 8, `churn` 1, `fallback_lang` 8, `mf2-axum` 13 + 4, conformance `layers` 3 and `l6` 4 | `… leptos-0-8` |
  | `churn` | **84/84**; each row's live bytes and allocations as the last run's (the `signal` row 614.58 B, 19.03 allocations a row) | `… churn` |
  | `l6-web` | **20/20** | `… l6-web` |
  | `l7-web` | **34/34**; L7 444/444, L7c 444/444, L7d and L7cd 325/444 (+119 documented degradations each); the ledger's L7 columns hold | `… l7-web` |
  | e2e on 0.9 | demo 210/210, lazy 74/74, islands 58/58, csr 98/98, zone 40/40, a11y 720/720 | `… e2e-0-9` (`bash probes/p10-b1/e2e.sh . c2-leptos-0-9`) |
  | e2e on 0.8 | the same six, the same counts | `… e2e-0-8` (the copies as at HEAD, then `bash probes/p10-b1/e2e.sh target/a7-demo-0-8/c2 c2-leptos-0-8`) |
  | B12, B1′ / B13 | above | `… b12`, `… b12-generated` |
  | the shims | **pass**: `mf2-native`'s names (its `NativeError` is `mf2::native::Error`), `mf2-ratatui` 7 | `cargo test -p mf2-native -p mf2-ratatui` (in `ci`) |

* **Found along the way (routed).**
  * Every change to a client-path crate: adding items, even ones no client reaches (a `Debug`
    impl), can move LLVM's inlining or function order in every client; read a names-kept build
    (`probes/p10-c2/named.sh`, B1's `norm-diff.py`) before calling a web move noise.
  * C3: the native matcher copies the candidate tag into a `String` (`replace('_', "-")`) on every
    `set_locale` and `with_locale`: one allocation a call, outside any format.
  * C5 / C8: `examples/tui` still measures 1.x's API over C2's `Catalogs`; the store's frame (A4's
    0.86–0.88 × 1.x) is measured when C8 moves it, against C5's gate.
  * C9: the trippy port's stripped size against upstream starts from a native build without jiff's
    error formatting (the −160 KB above).
  * F: `with_locale(&CORPUS, …)` before C4's typed form; `install` panics on a second corpus, and
    `Catalogs` serves another; before `install`, a native-only build panics, a web one shows no text.

## C3 (data half) — CLDR's language-matching data: what was built

Done ahead of C3's code and API (which wait for the owner's review of A8),
so that A8 can state the matching rules decided in question 11 from the
data rather than from expectation.

* **Vendored** (commit `acdbc01` on `main`), through
  `cargo xtask cldr-sync` at the pinned commit (`48.2.1` =
  `26a79cb4…`), byte-for-byte:
  * `cldr-core/supplemental/languageMatching.json` — **55,261 B**: 378
    `languageMatch` rules (`written-new`), 4 match variables, 6 paradigm
    locales;
  * `cldr-core/supplemental/territoryContainment.json` — **11,959 B**. Needed
    because the `$americas` match variable is the macro-region `019`, whose
    members (`MX`, `AR`, …) only the containment data gives.
  * The vendored set grows from 40 files (3,633,315 B) to **42 files
    (3,700,535 B)**; `PIN`'s `layout` and `vendored` lists name both (the
    text is written by `cldr-sync`, `xtask/src/cldr_sync.rs`). A second
    `cargo xtask cldr-sync` changes nothing: the sha-256 of every file under
    `third_party/cldr-json` is the same before and after
    (`14652cb3…` over the sorted list).
* **`parentLocales.json` stays cache-only.** CLDR's matcher decides by
  likely subtags and distances, not by the inheritance chain; `es-MX`'s
  preference for `es-419` over `es` comes from the `$americas` rules
  (below), which need the containment file instead.
* Nothing else asserts the vendored set (no test counts it; `locale-data`
  reads only the files it read before). `05-tooling.md` §7's list of
  vendored supplemental files should gain the two when this record is
  integrated.
* **Found by `cargo xtask ci`, fixed in the same commit:** once
  `cldr-sync` has filled its cache (`target/xtask-cache/cldr-json`, which
  did not exist on this machine before this run), `cargo xtask package`
  failed: `mf2-locale-data: LICENSE-UNICODE is a copy of
  target/xtask-cache/cldr-json/LICENSE`. The audit's map from content to
  forbidden path kept one path per digest, and the cache's upstream
  `LICENSE` — byte for byte `third_party/cldr-json/LICENSE`, the symlink's
  expected target — overwrote it. `xtask/src/package.rs` now keeps every
  path per digest and accepts the licence link when its target is among
  them; any other copy is still refused. (By the code, the audit would have
  failed the same way on any machine with the CLDR cache since Phase 9 A4
  wrote it; it passed here because the cache was absent.)

### What the file holds (observed)

| Level | Rules | Default |
|---|---|---|
| language (`desired` / `supported` one subtag) | 310, e.g. `nb`↔`no` 1, `hr`↔`bs` 4, `da`↔`nb` 8, `ca`→`es` 20 oneway, `yue`→`zh` 10 oneway, `pa`→`en` 30 oneway | `*`↔`*` **80** (#310) |
| script (`lang-Script`) | 49 (#311–#359) | `*-*`↔`*-*` **50** (#360) |
| region (`lang-Script-Region`, match variables) | 16 (#361–#376) | `*-*-*`↔`*-*-*` **4** (#377) |

Match variables: `$americas` = `019`; `$cnsar` = `HK+MO`; `$enUS` =
`AS+CA+GU+MH+MP+PH+PR+UM+US+VI`; `$maghreb` = `MA+DZ+TN+LY+MR+EH`. Paradigm
locales: `en`, `en-GB`, `es`, `es-419`, `pt-BR`, `pt-PT`.

**The script-level rules, all of them** — which languages have a cross-script
entry at all:
* **same language, both ways:** only `sr-Latn` ↔ `sr-Cyrl` **5** (#335);
* **same language, one way, towards the usual script** (a reader who asked
  for a transliteration or a sub-script accepts the usual one): `ar`, `bn`,
  `gu`, `hi`, `kn`, `ml`, `mr`, `ta`, `te` `-Latn` → native script 20;
  `zh-Latn` → `zh-Hans` 20; `zh-Hani` → `zh-Hans` and → `zh-Hant` 20;
  `ja-{Latn,Hani,Hira,Kana,Hrkt}` → `ja-Jpan` 5, `ja-{Hira,Kana}` → `ja-Hrkt`
  5; `ko-{Hani,Hang,Jamo}` → `ko-Kore` 5, `ko-Jamo` → `ko-Hang` 5;
* **another language, one way** (they make a language-level fallback
  usable across scripts): `am-Ethi`, `bn-Beng`, `ka-Geor`, `km-Khmr`,
  `kn-Knda`, `lo-Laoo`, `ml-Mlym`, `my-Mymr`, `ne-Deva`, `or-Orya`,
  `pa-Guru`, `ps-Arab`, `sd-Arab`, `si-Sinh`, `ta-Taml`, `te-Telu`,
  `ti-Ethi`, `ur-Arab`, `yi-Hebr` → `en-Latn` 10; `az-Latn`, `hy-Armn`,
  `tk-Latn`, `uz-Latn` → `ru-Cyrl` 10; `bo-Tibt`, `za-Latn` → `zh-Hans` 10.
* **There is no `zh-Hant` ↔ `zh-Hans` rule and no `pa-Arab` ↔ `pa-Guru`
  rule**; both fall to the default 50. Nor is there one for any other
  language written in two scripts (`az`, `bs`, `ff`, `ks`, `mn`, `sd`,
  `shi`, `uz`, `vai`, `yue`).

### The evidence for question 11

Computed by a throwaway reader over the three vendored files
(`target/p10-c3/evidence.py`, not in the tree): each tag maximized by
likely subtags, then per level the first rule that matches (a `oneway` rule
only in its stated direction), the three distances summed. **The threshold
is not in the data**: the JSON has no documentation keys, and the rule text
is UTS #35 Part 1 §4.4 (Language Matching). The verdicts were first
computed with **ICU's LocaleMatcher default — a match only when the total
is below the default script distance, 50** — as recalled. **C3's text half
read the section (below) and re-ran them:** the threshold is the
implementation's, below the default script distance, and a match below 50
is the top of that range; **no verdict changed**. Eight figures rose by 4
(the data half's reader stopped adding once a total reached 50; the section
sums the three levels) and are corrected in the table.

| Reader asks → app has | Maximized | Deciding rules | Distance | Verdict |
|---|---|---|---|---|
| `zh-Hant` → `zh-Hans` | zh-Hant-TW → zh-Hans-CN | script: #360 `*-*` 50; region #377 4 | 54 | **refuse** |
| `zh-Hans` → `zh-Hant` | zh-Hans-CN → zh-Hant-TW | script: #360 `*-*` 50; region #377 4 | 54 | **refuse** |
| `zh` → `zh-TW`, `zh-TW` → `zh` | zh-Hans-CN ↔ zh-Hant-TW | script: #360 50; region #377 4 | 54 | **refuse** |
| `zh-Hant-TW` → `zh-TW` | both zh-Hant-TW (likelySubtags: `zh-Hant` → zh-Hant-TW, `zh-TW` → zh-Hant-TW) | — | 0 | match (exact) |
| `zh-HK` → `zh-TW` (and back) | zh-Hant-HK ↔ zh-Hant-TW | region: #376 `zh-Hant-*` 5 (HK is `$cnsar`, TW is not) | 5 | match |
| `zh-HK` → `zh-MO` | zh-Hant-HK → zh-Hant-MO | region: #374 `zh-Hant-$cnsar` 4 | 4 | match |
| `yue` → `zh-TW` / → `zh` | yue-Hant-HK → zh-Hant-TW / zh-Hans-CN | language #309 `yue`→`zh` 10 oneway; then region 4 / script 50 and region 4 | 14 / 64 | match / refuse |
| `sr-Latn` → `sr-Cyrl` (and back) | sr-Latn-RS ↔ sr-Cyrl-RS | script: #335 5 (both ways) | 5 | match |
| `sr-ME` → `sr` | sr-Latn-ME → sr-Cyrl-RS | script #335 5; region #377 4 | 9 | match |
| `pa-Arab` → `pa-Guru` (and back) | pa-Arab-PK ↔ pa-Guru-IN | script: #360 50; region #377 4 | 54 | **refuse** |
| `pa` → `en` / `pa-PK` → `en` | pa-Guru-IN / pa-Arab-PK → en-Latn-US | language #84 30 oneway; script #324 `pa-Guru`→`en-Latn` 10 / #360 50; region 4 | 44 / 84 | match / refuse |
| `es-MX` → `es`, `es` → `es-MX` | es-Latn-MX ↔ es-Latn-ES | region: #370 `es-*-*` 5 | 5 | match |
| `es-MX` → `es-419`; `es-AR` → `es-MX` | es-Latn-MX → es-Latn-419 … | region: #368 `es-*-$americas` 4 | 4 | match (closer than `es`) |
| `es-419` → `es`; `es-ES` → `es-419` | … | region: #370 5 | 5 | match |
| `en-GB` → `en-US` (and back) | en-Latn-GB ↔ en-Latn-US | region: #367 `en-*-*` 5 | 5 | match |
| `en-AU` → `en-GB` / → `en-US` | … | region: #365 `en-*-$!enUS`→`en-*-GB` 3 / #367 5 | 3 / 5 | match (prefers en-GB) |
| `pt-PT` → `pt-BR` (and back); `pt` → `pt-PT` | pt-Latn-PT ↔ pt-Latn-BR | region: #373 `pt-*-*` 5 | 5 | match |
| `fr-CA` → `fr` (and back) | fr-Latn-CA ↔ fr-Latn-FR | region: #377 `*-*-*` 4 | 4 | match |
| region implies script: `zh-TW`→`zh-CN`, `pa-PK`→`pa-IN`, `uz-AF`→`uz`, `az-IR`→`az`, `mn-CN`→`mn`, `sd-IN`→`sd` | the scripts differ after maximizing | script: #360 50; region #377 4 | 54 | refuse |
| `ca` → `es` / `es` → `ca` | … | #24 `ca`→`es` 20 oneway / #310 `*` 80 | 20 / 80 | match / refuse |

* **`419` and `$americas`.** In the containment data `019` contains `021`,
  `013`, `029`, `005`; `419` is a *grouping* (`013`, `029`, `005`), not in
  `019`'s tree. The `es-MX` → `es-419` = 4 row counts `419` as in
  `$americas` because all its members are; counting only the tree gives 5,
  a tie with `es` (then settled by the paradigm locales, both of which are
  paradigms, or by order). **Fixed by C3's text half:** the section's own
  es-419 examples need `419` inside `$americas`, so a macroregion is in a
  variable when all its contents are — this reading.
* **Demotion** (not in the data; the section leaves it to the
  implementation): each later entry in the reader's list is demoted by 5.
  The section suggests a step a little above the default region distance
  (4); 5 is also `en-US` ↔ `en-GB`, which this bullet first mislabelled as
  the default. A region difference of 5 on the reader's first choice ties
  an exact match on the second, and the earlier wins; one of 4 (`de-AT` →
  `de`) beats it.

### Verdict against question 11's expectations

* **Serbian Latin ↔ Cyrillic: confirmed** (5, both ways).
* **Punjabi's two scripts: confirmed refused** (no rule; 50, and 54 with
  the region step).
* **Spanish regions: confirmed** as the owner described: `es-MX` served when
  the app has it; otherwise `es` (5), preferring `es-419` (4) when the app
  has that; and `es` → `es-MX` (5) when that is all the app has.
* **Traditional ↔ Simplified: contradicted.** CLDR 48 has no rule between
  `zh-Hant` and `zh-Hans`; the pair takes the default script distance, 50
  (54 with the region step), which every threshold the section allows
  refuses — in both directions, and for region-implied scripts too
  (`zh-TW` ↔ `zh-CN`). Following CLDR's data, a
  Traditional reader of an application that has only Simplified gets the
  source language, and so does a Simplified reader of an application that
  has only Traditional. (Today's native matcher serves `zh` to `zh-Hant-TW`
  by truncation; the web's likewise, then any `zh`.) The owner expected
  Traditional → Simplified to be accepted. **Decided (question 15): follow
  the data**, with no project rule on top.

**Needs (for the coordinator) — answered 2026-09-28:** question 15 follows
the data; question 16 fetches the section into the cache (C3's text half,
under "Next"). As first written: (1) the owner's decision on Traditional ↔
Simplified — CLDR's refusal, or a documented project rule on top of the data;
(2) the matching algorithm's normative text (UTS #35 Part 1 §4.4: the
threshold, `oneway`, paradigm locales, demotion, groupings in match
variables) is not in the tree and cannot be fetched under the network rule;
C3 either works from this record and ICU's documented behaviour, or the
owner allows vendoring that section.

### For C3's rules and the client's table (interpretation)

* The rules D21 lists map onto the data: "the script implied by likely
  subtags" is maximization; "region fallback" is the region level (4–5);
  "another script only where CLDR's data accepts it" is the script level
  below the threshold (sr 5; the transliteration and sub-script one-ways;
  the native-script → English/Russian/Chinese one-ways).
* The client's table needs, for a corpus: the rules whose supported side can
  be one of its languages (or `*`), and likely subtags for its languages and
  for the languages whose readers those rules accept. Counted over the
  vendored files: the demos' `ar en fr` — 129 of 378 rules and 109 of 7,788
  likely entries; `examples/tui`'s `de en es fr` — 108 and 91; the 11-locale
  probe panel — 169 and 160. Most of it is the one-way "reader of X accepts
  English/French/Spanish" rules (a reader of Acholi accepts English at 30):
  keeping only the rules between the corpus's own languages and the
  wildcards leaves 11 / 10 / 23 rules and 4 / 5 / 12 likely entries. Which
  cut the client makes is C3's to measure against B1; the server and native
  builds can carry the whole table.

**Commands:** `cargo xtask cldr-sync` (twice; the second changes nothing);
`python3 target/p10-c3/evidence.py` (the table above; `evidence.py <desired>
<supported>` for one pair); `cargo xtask ci` green on the commit (one
earlier run failed `mf2-catalog`'s timing test `linear.rs` under load 10–13
— a different row on each re-run — and passed on the next).

## C3 (text half) — UTS #35 Part 1's matching rules: what was read

Done so that A8 states the matcher's rules from the specification rather
than from recollection (question 16). What follows paraphrases the text
and cites its sections. Nothing is quoted, and the text is not in the tree.

### What was fetched (observed)

* **`cargo xtask uts35-sync`** (commit `f54f39f`) fetched
  `docs/ldml/tr35.md` from `https://github.com/unicode-org/cldr` at tag
  **`release-48-2`** = `11299982335beb974c1c63c45265184e759c0f41`
  (committed 2026-03-16): **345,718 B, SHA-256
  `01967399eb0cc4aa714d25fa25b8f717bc5d8ce75a704836c77565126c7a7f76`**.
  It is written to `target/xtask-cache/uts35/docs/ldml/tr35.md` with a
  `COMMIT` stamp; the clone is `target/xtask-cache/cldr` (shallow,
  blobless, sparse; 1.8 MB). `third_party/uts35/PIN` records the upstream,
  tag, commit, date, file and digest.
* **The recalled path is right:** the CLDR repository keeps Part 1's source
  as `docs/ldml/tr35.md`. At this tag its header gives version 48.2, dated
  2026-03-03, revision 78 of the published report (`tr35-78`).
* **The release.** `uts35-sync --list` shows 674 tags. For 48:
  `release-48`, `release-48-1` and `release-48-2`, with their milestones,
  alphas, betas and candidates, and no `release-48-2-1`. 49 has milestones,
  alphas and betas only. The vendored JSON says only `_cldrVersion` 48, and
  cldr-json's commit message is `48.2.1`. Taken (inference): cldr-json's
  48.2.1 packages CLDR 48.2, hence `release-48-2`. The PIN's `data =
  48.2.1` records the pairing, and both `uts35-sync` and the xtask test
  `the_text_goes_with_the_vendored_data` refuse a PIN whose `data` is not
  `third_party/cldr-json/PIN`'s `tag`.
* **The section numbers.** The markdown's headings carry none. The text's
  own table relating inheritance, default content, likely subtags and
  matching (in §4.2) calls the two sections 4.3 Likely Subtags and 4.4
  Language Matching. The variables are §4.4.1 by position (4.4's one
  sub-heading).
* **Its terms.** The closing notice puts the text under Unicode's Terms of
  Use: private or in-house copies only. Publishing it, or building it into
  a product or publication, needs Unicode's written permission. That is the
  reason for cache-only, as for the MF2 specification.
* **Found and fixed:** `Repo::remote_tag_commit` (`xtask/src/git.rs`, shared
  with `cldr-sync`) returned `fc1fd058…` for `release-48-2`: the annotated
  tag's own object, not its commit. `git ls-remote` lists a tag's peeled
  line only when a pattern names it. The helper now asks for both.
  `cldr-sync`'s lightweight tag resolves as before; a re-run left
  `third_party/cldr-json` unchanged (43 files).
* **Shown** (2026-09-28):
  * a second `uts35-sync` changes nothing: the PIN, cldr-json's PIN and
    both cache files hash the same before and after;
  * with the PIN's digest altered (`01967399…` → `11967399…`), the sync
    exits 1, naming the file and both digests, and the cache's files keep
    their modification times;
  * with `data` altered (48.1.0), it exits 1, naming both releases;
  * `git ls-files` has no `tr35`;
  * no tracked text file shares a run of 10 words with the text except
    `third_party/message-format-wg/LICENSE`, upstream's own file carrying
    Unicode's same notice. In the new PIN, code and plan text, the only
    runs of 6 words are the upstream URL and one example's list of locale
    codes, used as a test input: data, not prose;
  * `cargo xtask ci` green.

### What the section says (read; paraphrased)

§4.4 unless marked.

* **The procedure.** Take each of the reader's locales in order and, for
  each, each of the application's. The weighted distance is a demotion
  for the reader's entry plus the matching distance. A pair replaces the
  best so far only when strictly smaller, so ties go to the earlier pair.
  The best pair is the answer only when its weighted distance is below
  the threshold; otherwise the answer is the application's default
  locale. The pseudocode has two slips. Its comparison names the best
  desired locale where it plainly means the best weighted distance. Its
  fallback puts the default in the desired slot; read: the default
  supported locale is the result.
* **The matching distance.** Maximize both tags with likely subtags (§4.3),
  except a desired `und`, which is left as it is; otherwise it would become
  English and outrank the reader's real languages. Then take language,
  script and region in turn. Identical subtags add nothing. Otherwise the
  rules are searched in file order, and the first that matches adds its
  distance; `*` matches anything.
* **The threshold is not a number.** It is the implementation's,
  typically above the default region distance and below the default
  script distance (4 and 50 in CLDR 48), and the best pair must be
  strictly below it.
* **`oneway`.** Without it a rule matches both ways. With it, only one: the
  reader's locale against the rule's desired side and the application's
  against its supported side.
* **Demotion** is also the implementation's: a positive value that grows
  with the entry's distance from the head of the reader's list. The text
  suggests a step a little above the default region distance, so that a
  regional variant of the first language beats an exact second one. Its
  example is a reader of `de-AT` then `fr`, and an application with `de`,
  `fr` and `ja`: the reader should get `de` (4 away), which the text
  secures with a step above 4. Under its own strict comparison, a step of
  exactly 4 would tie and keep the earlier pair, so it would do too. The
  demotion is part of the weighted distance, so it counts against the
  threshold.
* **Paradigm locales** (§4.4.1) are preferred within their cluster: of two
  candidates at the same distance, the paradigm wins. The text gives no
  number. They also make a macroregion locale the one its cluster matches
  most closely:
  * for a reader of `es-419`, `es-MX` beats `es`;
  * for a reader of `es-MX`, `es-419` beats `es`;
  * and `es-419` beats its own sub-locales, such as `es-CR`.

  The text's illustrative paradigms include Russian and leave Portuguese
  out; CLDR 48's list is en, en-GB, es, es-419, pt-BR, pt-PT, and the data
  governs.
* **Match variables** (§4.4.1). A value combines regions with `+` (union)
  and `-` (difference), strictly left to right, and `$!X` is every region
  not in `$X`. A macroregion in a value stands for its contents,
  recursively (the text writes macrolanguages, but its examples are
  regions). The text's sample `$enUS` lacks CA and PH, which CLDR 48 has;
  again the data governs.
* **The `419` / `$americas` reading.** The text does not say outright
  whether a locale whose region is itself a macroregion (`es-419`) lies
  inside a variable. Read literally, its expansion leaves only leaf regions
  in the set. But it gives the reason for 019: to put `en-US` in one
  cluster with `es-419` and everything under it. And its three `es-419`
  examples (above) need `419` inside `$americas`. Counting a macroregion
  as inside when all its contents are (the data half's reading)
  reproduces all three; the literal reading reproduces none (the re-run,
  below). **Read: a macroregion is in a variable when all its contents
  are.** So `es-MX` → `es-419` is 4 (#368) and `es-MX` → `es` is 5 (#370),
  as the data half had it.
* **§4.2**, in its table relating inheritance, default content, likely
  subtags and matching: parent locales and default content are not to be
  used for matching or for likely subtags. That confirms keeping
  `parentLocales.json` in the cache only.
* **§4.3, likely subtags.** A tag with language, script and region is
  taken as it is. Otherwise look up language-script-region,
  language-script, language-region, then language; the first hit fills
  only the empty fields (and `und`). No hit is an error the implementation
  signals as it chooses; returning the input with `Zzzz` / `ZZ` is one of
  the options listed. An implementation may leave `und` out of maximizing.
* **Not binding on C3:** a worked example in percentages (the older model
  that multiplied them; the algorithm and the data add distances); an
  optional mode that scales language distances up so that script
  differences dominate; geographic closeness between regions as an
  optional refinement.

### The evidence re-run with the rules as read (observed)

`target/p10-c3/evidence.py` (untracked; the data half's reader kept as
`evidence_v1.py`, with its output `evidence_v1.out`) now follows the
text:
* §4.3's lookup order;
* every level summed, and identical subtags adding nothing;
* a macroregion counted in a variable when all its contents are, with the
  literal reading beside it;
* the best-match loop: a demotion of 5 per later entry, weighted distances
  compared strictly, a paradigm winning a tie for the same reader entry,
  and a threshold of 50.

Output: `target/p10-c3/evidence_v2.out`.

* **No verdict changed:** 0 of the data half's 39 pairs.
* **Eight figures rose by 4**, the region step now added, and all are
  still refusals: `zh-Hant` ↔ `zh-Hans`, `zh` ↔ `zh-TW` and `pa-Arab` ↔
  `pa-Guru` 50 → 54; `yue` → `zh` 60 → 64; `pa-PK` → `en` 80 → 84. The
  region-implied row is now 54 too. The data half's table is corrected in
  place.
* **The literal reading of `419`** would move five pairs: `es-MX` →
  `es-419`, `es-AR` → `es-419` and `es-419` → `es-MX` from 4 to 5, and
  `es-419` → `es` and `es-ES` → `es-419` from 5 to 4 (#369, with `419` then
  outside the Americas). Every verdict would still be a match, but the
  preferences the text states would reverse.
* **The text's and the plan's cases, run as lists** (the reader's list
  against the application's locales; threshold 50):

| Reader | Application | Result | Weighted | The text or plan says | With the literal `419` |
|---|---|---|---|---|---|
| en-US, de, fr, gsw, it | ja-JP, de, zh-TW | de | 5 | de (§4.4, opening) | same |
| zh | ja-JP, de, zh-TW | the default | 54 | **zh-TW** (§4.4, opening) | same |
| de-AT, fr | de, fr, ja | de | 4 | de (§4.4, demotion) | same |
| und, it | en, it | it | 5 | it (§4.4, `und`) | same |
| en-SA | en-GU, en, en-IN, en-GB | en-GB | 3 | en-GB (§4.4.1, paradigm) | same |
| es-419 | es-MX, es | es-MX | 4 | es-MX (§4.4.1) | es |
| es-MX | es-419, es | es-419 | 4 | es-419 (§4.4.1) | es-419, only by order (a tie at 5) |
| es-MX | es-CR, es-419 | es-419 | 4 | es-419 (§4.4.1) | es-CR |
| en, fr | fr-CA, ru | fr-CA | 9 | fr-CA (§4.2's table) | same |
| zh-Hant-TW | zh-TW, zh | zh-TW | 0 | zh-TW (C3) | same |
| zh-HK | zh-TW, zh | zh-TW | 5 | zh-TW (C3) | same |
| zh-Hant | zh, en | the default | 54 | the default (C3; question 15) | same |
| zh-TW, zh | zh, en | zh | 5 | zh, through the plain `zh` (C3) | same |
| pa-Arab, en | pa-Guru, en | en | 5 | en (question 11) | same |
| sr-Latn | sr, en | sr | 5 | sr (question 11) | same |
| es-MX | es, en | es | 5 | es (question 11) | same |
| es | es-MX, en | es-MX | 5 | es-MX (question 11) | same |
| ten languages, zh-Hant 10th | zh-Hant, en | zh-Hant | 45 | — | same |
| eleven languages, zh-Hant 11th | zh-Hant, en | the default | 50 | — | same |

* **One of the text's examples is not reproduced:** the section's opening
  illustration. It has a reader of plain Chinese, offered `ja-JP`, `de` and
  `zh-TW`, best served `zh-TW`. `zh-TW` is the closest of the three, but at
  54 (script 50 + region 4) it is above any threshold the text allows, so
  the steps return the default. Read (interpretation): the illustration
  ranks the candidates and leaves the threshold out. The steps govern, and
  they agree with question 15.
* **`en-SA`:** CLDR 48 settles it by rule #365 (`en-*-$!enUS` → `en-*-GB`
  3, against 4 for `en-IN`), so the paradigm tie-break is not what picks
  `en-GB` there.
* **No script-level rule pairs a script with itself.** So the data half's
  handling of that level (it searched the rules when only the languages
  differed) could not have changed a result.

### Question 11's and question 15's expectations, as read

* Serbian Latin ↔ Cyrillic served (5). Punjabi's two scripts refused (54).
  Spanish regions as the owner described (`es-MX` → `es` 5, → `es-419` 4;
  `es` → `es-MX` 5). Traditional ↔ Simplified refused (54) both ways, and
  `zh-TW` ↔ `zh-CN` too: question 15's decision, which the text's
  algorithm gives for every threshold in its range.
* **The case to be sent back to the owner, a threshold above the default
  script distance, does not arise:** the text puts the threshold below it.

### For A8 and C3 (interpretation, brief)

* **The threshold to state: a match when the weighted distance is below
  50.** That is the top of the text's range; for whole-number distances,
  any threshold between 49 and 50 behaves identically.
  * The owner's answers need a threshold above 5: Serbian's scripts and
    Spanish regions.
  * Following the data with nothing on top (question 15) keeps every
    acceptance the data scores below the script distance: `ca` → `es` 20,
    `yue` → `zh-TW` 14, `pa` → `en` 44. A lower threshold would quietly
    undo some of them.
  * It is also the reading the data half's verdicts used.
* **Demotion: 5 per later entry,** the step the text suggests. Counted
  against the threshold, it caps how deep in a reader's list a match can
  come from: an exact match 10th in the list is served (45), 11th not
  (50). C3 either states this or bounds the demotion, with a test either
  way.
* **Paradigms** are a tie-break among the application's locales for the
  same reader entry. A tie across entries goes to the earlier entry, which
  is what the demotion is for.
* **Open for C3, a test each:**
  * `$!X` for a macroregion that straddles a variable (`en-001` against
    `$enUS`). With `$!X` as every region not in `$X`, `en-001` → `en-GB` is
    3, by #365.
  * A desired `und` is not maximized.
  * A tag that likely subtags cannot fill keeps its empty fields (`Zzzz` /
    `ZZ`).
* **C3's test table** can take the text's own examples above, all but the
  opening illustration, each with its section.
* **The client's table** does not change with this reading. The rules,
  variables and paradigms are the data's, and the containment is needed
  only for the macroregion codes that the corpus and its readers use.

**Commands:**
* `cargo xtask uts35-sync --list`;
* `cargo xtask uts35-sync --tag release-48-2` (the pin);
* `cargo xtask uts35-sync`, twice: nothing changed (hashes in
  `target/p10-c3/uts35-{before,after}.sha`);
* the two refusals: the digest, then `data`, altered, with the PIN restored
  after each (`sha256sum`: `4bcc7470…` before and after);
* `cargo xtask cldr-sync`, after the `remote_tag_commit` fix;
* `python3 target/p10-c3/evidence.py`, the tables above
  (`evidence.py <desired> <supported>` for one pair);
* `python3 target/p10-c3/shared_runs.py <N> <file>…`, the quoting check;
* `cargo xtask ci`, green on `f54f39f`'s tree (log
  `target/p10-c3/ci-text-half.log`).

## Part D — the web (D1 after B4; D2–D4 after D1; D5 after D3 and C7; D6 last)

The design is [04](04-leptos-integration.md) §12, with its shared parts in
[19](19-native-and-terminal.md). The owner approved it with A8's review (question 17):
- D2 and D3 → 04 §12.5: `Negotiator` as the tower layer;
- D4 → 04 §12.2–§12.4: the switcher's options from `language.<tag>`;
- D5 → 05 §6.4;
- D6 → 19 §1.4 and §2, and the demos' nightly `fmt-check` (19 §6).

| Task | Deliverable | Done when |
|---|---|---|
| **D1** `mf2::axum` | `mf2-axum` becomes a shim. Negotiation and catalog serving compile without a Leptos mode (a plain Axum application: a `Locale` extractor, per-request formatting through `Locale::format`); the request glue compiles only with `ssr`. `api/axum.txt` | as B1; a plain-Axum test serving a formatted response per `Accept-Language` |
| **D2** Defaults | `Negotiator::default()` becomes `?lang=`, then the cookie, then `Accept-Language` (the order Getting started writes by hand). The switcher takes its parameter name from the installed query source | unit tests; e2e `demo.mjs` with the wasm blocked |
| **D3** Server wiring and the generated setup | **Probe first:** a tower layer `mf2::axum::negotiate(Negotiator)` that negotiates, puts the result in the request's extensions, and writes `Content-Language`, `Vary` and the cookie; the render finds it through `Parts` in the Leptos context. If it holds, the `_with_context` wiring (and the silent failure when one entry point misses it) goes. **Then:** a generated `setup()` / `install()` on each side; no `[features]` block in the translation crate (A2) | the e2e checks (`demo`, `lazy`, `csr`, `islands`, `a11y`) without the context; `scenarios` byte-identical |
| **D4** Typed languages and the switch on the web | • `LocaleOption tag=Locale::Fr`; <br>• `set_locale` / `preload_locale` callable on both sides (a spawned call on the client, nothing on the server), so application code needs no `#[cfg]` pair; <br>• a reactive `current_locale()`; <br>• an options component driven by `language.<tag>` messages; <br>• markup closures that need no type annotation (`\|c\| view! { … }`, review #10) | compiled samples; e2e; `churn` for the new conversions |
| **D5** `mf2 init` as a starter (web) | `mf2 init --ssr` / `--islands` / `--csr`: a complete, runnable application, or translations added to an existing one (A6 decides whether one crate is offered) | `cargo xtask docs` builds what each makes |
| **D6** The web book and examples | Getting started (and its 0.8 variant), switching, call sites and delivery modes on 2.0; `examples/demo-{ssr,islands,csr}` and `tools/e2e` updated | `cargo xtask docs`, every e2e check, `churn`, `islands-zero`, `size`; the web UX rows all fall |

## Part E — the silent failures (required for 2.0; any order, E4 after D1)

| Task | Deliverable | Done when |
|---|---|---|
| **E1** `dropped-markup` | A lint: a translation that leaves out a markup element of the source message (as `dropped-variable` does for variables). Run by `check`, the build and `import` | a seeded-drift case; the review's `terms` example refused |
| **E2** `@do-not-translate` is not missing | Such a message counts neither as missing nor in coverage (`check`, `stats`, JSON and XLIFF exports) | the review's case: "3 of 4 missing" becomes "2 of 3" |
| **E3** `import` checks what it writes | `mf2 import` (JSON and XLIFF) runs the checks on the result and fails on errors, writing nothing. JSON import either adds ids the language lacks or names XLIFF in its message | the review's `$nom` case refused; a negative control per format |
| **E4** Server and client warnings | Logged once on the server: a page rendered without the request's language; formatting with no catalogs installed. A browser console warning in debug builds only | the warnings shown once; B1 and B12 unchanged in release (measured) |

## E1 — `dropped-markup`: what was built

Commit `a72d057` on `main`, "Phase 10 E1: `dropped-markup` — a
translation may not lose the source's markup" (written on the branch
`p10-e-silent-failures` off `8f6569e`, and cherry-picked unchanged).

**The lint.** `Lint::DroppedMarkup` (`"dropped-markup"`): a translation
that leaves out markup its source message has. Checked in
`check::against_source` beside `dropped-placeholder`, the same way: markup
**names** compared over the **whole message** (the analysis' NFC names), so
one variant may leave the markup out while another keeps it — Polish `one`
("a message") drops the bold with the count. Reported once per message at
its start: `the source message has {#link}, which this translation leaves
out, so this language loses what it marks (a link, a style)`.

**Its level — a decision, stated here.** The work order says "as
`dropped-variable` does for variables" (the lint is `dropped-placeholder`,
a warning with floor `allow`) and "the review's `terms` example refused".
A warning cannot refuse, so `dropped-markup` is an **error by default with
floor `allow`**, the catalogue's pattern for "an error, which a corpus that
means it may turn down" (`dynamic-select`, `do-not-translate`, …). Why it
differs from `dropped-placeholder`: a plural variant routinely drops
`{$count}`; nothing routinely drops a link. A corpus that drops emphasis on
purpose (italics in a script without them) sets `dropped-markup = "warn"`.
plans/05 §3's rule "a translation MAY use a subset of the source's …
markup" was changed in the same commit.

**Run by** `mf2 check`, the build (the same `check::corpus` pass
`Build::run` and `Build::check` share, so a build script now fails on it),
and `mf2 import` from E3 (commit `894c0c4`).

| Test | What it proves | Result |
|---|---|---|
| `tests/drift.rs` `every_lint_fires_on_its_own_drift_and_nothing_else_does` — the new drift `{#kbd}Esc{/kbd}` → `Esc` in `pl` | the lint fires on its drift and no other error does | PASS |
| same table, `undeclared-markup`'s drift | changed from `{#kbd}…` → `{#b}…` (which also dropped `kbd`) to `{#kbd}{#b}Esc{/b}{/kbd}`, so each drift stays one lint's | PASS |
| `a_lint_set_to_allow_says_nothing` | floor `allow` works | PASS |
| `markup_that_one_variant_keeps_is_not_dropped` (new) | Polish `one` without `{#b}`, `few`/`many`/`*` with it: clean; `{#b}` out of every variant: one error naming `{#b}` | PASS |
| `the_reference_workload_is_clean` | no workload translation drops markup | PASS |
| `tests/commands.rs` `check_refuses_a_translation_that_drops_markup` (new) | the review's case: `terms = Accept our {#link}terms{/link}.` / `Acceptez nos conditions.` → `mf2 check` exits 1, `… (in terms, locale fr) [dropped-markup]`; with the link kept it passes | PASS |
| `tests/xliff.rs` `import_refuses_a_translation_that_drops_markup` (E3's commit) | the same case through XLIFF import: refused, nothing written; with the `<pc>` kept, it lands | PASS |

**Corpora in the tree** (`mf2 check --features
fn-number,fn-datetime,datetime-icu` on each): `tools/i18n-fixture`'s
Polish `help` had dropped the source's `{#b}` (`Nacisnij {#kbd}Esc{/kbd},
aby zamknac` against `Press {#kbd}Esc{/kbd} to {#b}close{/b}`); nothing
relied on it (rg), so it was restored — without that the fixture's build
script fails. The demos, `bench/churn`, the fixture's two variants and the
book's projects (`cargo xtask docs --no-build`) drop nothing. L5's corpora
are identical twins.

**Commands run** (on the branch, `CARGO_BUILD_JOBS=2`): `cargo test -p
mf2-build` (all pass), `cargo test -p mf2-cli` (all pass),
`cargo test -p mf2-i18n-fixture --lib`, `cargo clippy -p mf2-build -p
mf2-cli -p mf2-i18n-fixture --all-targets -- -D warnings`, `cargo fmt
--all --check`, `cargo xtask docs --no-build`. `crates/mf2-build/api.txt`
gained `pub mf2_build::Lint::DroppedMarkup` by hand, in the listing's
sorted place; on `main`, `cargo xtask api --check` confirms it ("18
listings unchanged"). What ran on `main` with the three applied is in E3's
record.

**Verdict:** done — the seeded drift and the review's `terms` example are
refused by `check`, the build and (with E3) `import`.

**For the design (interpretation).** None for A8. The book's lint
reference (F2) should give `dropped-markup` its section, with the
`dropped-placeholder` contrast.

## E2 — `@do-not-translate` is not missing: what was built

Commit `3f1eed9` on `main`, "Phase 10 E2: `@do-not-translate` messages
are neither missing nor covered" (written on the branch
`p10-e-silent-failures`, and cherry-picked unchanged).

**The rule.** A message the source marks `@do-not-translate` needs no
translation: it counts neither as missing (where a language lacks it) nor
as translated (where a language copies it). Coverage is over the messages
that need translating.

**Where the count comes from.** `check::coverage_of(corpus, locale) ->
Coverage { tag, translatable, missing }` (the source's ids less its
do-not-translate ones; those the locale lacks, in manifest order). The
`missing-translation` lint uses it, and the build's `Outcome` carries one
per locale in a new `#[doc(hidden)]` field, `coverage`, which `mf2 stats`
reads — one computation, so `check` and `stats` cannot disagree. Hidden,
like `Outcome::manifest` and `catalogs`: not in `api.txt`, not promised.

**Where it shows.**
- `mf2 check` / the build: `2 of 3 messages are missing here and fall back
  to en: apply, farewell` where it said `3 of 4 …` (text, and the same
  diagnostic in `--format json`).
- `mf2 stats`: the coverage and missing columns, and `--format json`'s
  per-locale `messages` / `missing`, count only messages that need
  translating; the header says `4 messages (1 marked @do-not-translate)`,
  and the JSON gains a top-level `do_not_translate`. The catalog's own
  `missing` / `fallbacks` (what it carries) are unchanged.
- **XLIFF export: no change needed.** A do-not-translate unit is already
  `translate="no"` with no `<target>` — XLIFF 2.1's own examples do exactly
  that (the spec §5.9.8.1, vendored), and tools leave such units out of
  their counts. A test now asserts the unit has no target.
- **JSON export: no change.** It writes a language's own messages, with
  no count: a do-not-translate message the language lacks is not in it,
  and a copy it has is exported as it stands (on import, E3's checks hold
  the copy to its source through the `do-not-translate` lint). *This is an
  interpretation of the work order's "JSON and XLIFF exports": if it meant
  that `mf2 export` of the source language should leave such messages out
  of the file a translation tool counts, that is a small follow-up — not
  done, to keep the export a faithful copy of a language.*

**Also: the mark on a section or a file now covers its entries** in every
check (the resource loader adds the property to each entry under a
`@do-not-translate` section head or resource). XLIFF export already treated
it so (§6.3: `translate="no"` on the file, group or unit); `check` read
only an entry's own properties, so a `[language]` section marked once
would still have counted as missing. Consequence: the `do-not-translate`
lint and `mf2 pseudo`'s copy-as-is now also apply to such entries.

| Test | What it proves | Result |
|---|---|---|
| `tests/drift.rs` `do_not_translate_messages_are_neither_missing_nor_covered` (new) | the review's case: en has 4 messages, 1 `@do-not-translate`; fr has 1 → "2 of 3 missing: apply, farewell", `coverage[fr] = (3 translatable, 1 translated)`; the source's coverage has nothing missing; fr copying the do-not-translate message → still 1 of 3 translated; a `@do-not-translate` `[language]` section → fr lacking it reports nothing | PASS |
| `tests/commands.rs` `do_not_translate_messages_are_not_missing` (new) | through the binary: `check` text "2 of 3 …"; `stats` header "4 messages (1 marked @do-not-translate)", rows fr `33.3%` / `2` and en `100.0%` / `0`; `stats --format json`: `messages` 4, `do_not_translate` 1, fr `messages` 1, `missing` 2 | PASS |
| `tests/xliff.rs` `the_export_has_the_mapping_of_the_plan` (extended) | the do-not-translate unit `brand` is `translate="no"` and has no `<target>` | PASS |
| existing `stats_reports_coverage_sizes_and_the_pins`, `check_names_the_first_missing_translations`, the drift table, the workload round trips | nothing else moved | PASS |

**Commands run** (on the branch, `CARGO_BUILD_JOBS=2`): `cargo test -p
mf2-build`, `cargo test -p mf2-cli` (all pass), `cargo clippy -p mf2-build
-p mf2-cli --all-targets -- -D warnings`, `cargo fmt --all --check`,
`cargo xtask docs --no-build`; every corpus in the tree re-checked with
the new binary (same results as before E2). What ran on `main` with the
three applied is in E3's record.

**Verdict:** done — the review's case reads "2 of 3" (test), in `check`
and `stats`, text and JSON.

**Found along the way (interpretation, brief).** A do-not-translate
message a language lacks is served from the source's catalog, so under
`mark-fallback-lang` it is wrapped `<span lang="en">` — wrong for a
language's own name (`Français` read with English rules). The book's
pattern (each language file copies the names) avoids that; the switcher's
own `lang` handling may too. Not changed; worth a look when D4 builds the
options component driven by `language.<tag>` messages.

## E3 — `import` checks what it writes: what was built

Commit `894c0c4` on `main`, "Phase 10 E3: `mf2 import` checks what it
would write, and writes nothing on an error it brings" (written on the
branch `p10-e-silent-failures`, and cherry-picked unchanged).

**Plan → check → write.** Both formats first make the files in memory
(`exchange::Rewrite`; `xliff::import` now returns its rewrites and
refusals — `plan_target` — instead of writing). Then `checked()` copies
`locales/` into a scratch directory, puts the rewritten files in it, and
runs `Build::check` on the corpus as it stands and on the copy, with
`mf2 check`'s configuration and features (`--features`, new on `import`,
else cargo's, the same function `check` uses) and `Emit::Module`, so
nothing is compressed. The findings the copy has and the corpus does not
(a finding keyed by level, locale, id, lint, error kind and message — not
by line, which a rewrite moves; the copy's paths mapped back) are printed
as `check` prints them. **Any error among them: nothing is written**,
`mf2 import: N error(s) that fr does not have now; nothing was written`,
exit 1. Otherwise the files are written, the new warnings printed, and the
summary is as before.

**"Brings", not "has" — a decision.** An error already in the files does
not stop an import that adds none: another language's error, or one raised
only because cargo could not name the features on a translator's machine
(`check` then checks with none, and `:currency` is an error there — before
and after alike). The review's case is an error the import brings.

**JSON and new ids — a decision.** The work order allows either adding
them or naming XLIFF. JSON import keeps plans/05 §6's rule (a new message
needs a file and a section, which flat JSON does not carry; XLIFF does,
and adds them there) — but a left-out id is now a **refusal**: `mf2
import: 1 message(s) fr does not have yet were left out: farewell; JSON
import changes the messages a language has, and XLIFF adds the others
where the source has them (`mf2 export fr --format xliff`)`, exit 1 after
writing the rest (as XLIFF's unit refusals do). An id the source lacks is
named apart. XLIFF's unit-by-unit refusals (§6.3) are unchanged; what they
leave is checked like the rest. A flat-JSON *locale* is still replaced by
the document (unchanged).

| Test | What it proves | Result |
|---|---|---|
| `tests/commands.rs` `import_refuses_what_check_would_refuse` (new) | the review's `$nom` case (JSON): exit 1, `$nom is not an input of the source message … (in greeting, locale fr) [undeclared-variable]`, "nothing was written", file byte-identical; **negative control:** a clean `Salut, {$name} !` lands; an existing `$nom` in another message does not stop a clean import | PASS |
| `tests/commands.rs` `import_names_xliff_for_messages_a_language_lacks` (new) | a new id is left out and XLIFF named, an id the source lacks named apart, exit 1, the rest written | PASS |
| `tests/xliff.rs` `import_refuses_a_translation_that_drops_markup` (new) | the review's `terms` case (XLIFF): the target without the link's `<pc>` is refused (`[dropped-markup]`, nothing written); **negative control:** with the `<pc>` kept it lands, `0 message(s) changed, 1 added` | PASS |
| `xliff.rs` unit test `without_the_data_check_an_edited_code_lands` (adapted) | the negative control for `xliff-code-edited` still holds, on the returned rewrites | PASS |
| every existing import test (round trips of the reference workload, each `xliff-*` code, the JSON round trip, the book's `import --dry-run`) | unchanged behavior where nothing is wrong | PASS |

**Cost, measured:** `mf2 import pl pl.json --dry-run` on the 1,600-message
reference workload (4 locales), one message changed, debug build: 1.18 s
wall, load average ≈ 11 (other forks building). For scale, `mf2 check
--features fn-number` on the same corpus took 20.1 s under the same load —
it compresses every catalog with brotli 11 in a debug build (the review's
#18; C6's business), which the import's checks skip.

**Commands run** (on the branch, `CARGO_BUILD_JOBS=2`): `cargo test -p
mf2-cli` (all 106 pass), `cargo test -p mf2-build`, `cargo clippy -p
mf2-build -p mf2-cli --all-targets -- -D warnings`, `cargo fmt --all
--check`, `MF2_CLI_API_WRITE=1 cargo test -p mf2-cli --bin mf2 api_txt`
(`crates/mf2-cli/api.txt` gains `--features <LIST>` under `mf2 import`),
`cargo xtask docs --no-build` (the book's `import` output is unchanged).

**On `main`, the three together** (picked onto `c0c1d43` without a
conflict, 2026-09-28; `CARGO_BUILD_JOBS=3`): `cargo xtask ci` green, first
run — no test failed, `mf2-catalog`'s timing test `linear.rs` included;
`api --check` "18 listings unchanged"; `package --check` "lists
unchanged". The full `cargo xtask docs` green: every sample compiled, the
samples' translation crates rebuilt against the new `mf2-build`. The corpora
`main` gained since `8f6569e` pass the new checks: `mf2 check --features
fn-number,fn-datetime,datetime-icu` on `examples/tui/i18n` and
`probes/p10-{ambient,single-crate,tr-in-crate}` reports nothing
(`probes/p10-links` holds only English, with nothing to compare).

**Verdict:** done — the review's `$nom` case refused; a negative control
per format; JSON names XLIFF.

**For the design (interpretation).** F3 (the translator workflow) can now
say: import checks, XLIFF adds, JSON edits.

## Part F — the 2.0 book (F1 with or after C8; the rest after D6)

| Task | Deliverable | Done when |
|---|---|---|
| **F1** MF2 for developers | A chapter on MF2 itself: <br>• the `.mf2` file (`@locale`, `---`, `[section]`); <br>• variables, functions and options; plurals and ordinals; selection on several values (gender); markup; <br>• `@do-not-translate` and translator comments; <br>• side by side with Fluent, ICU MessageFormat 1 and i18next; <br>• why messages are written by id, not extracted from code | compiled blocks; linked from Getting started |
| **F2** Reference pages | `mf2.toml` (every key); every lint, with an example and its fix; every command (extending `command-line.md`); every feature of `mf2` | a test that every lint and config key has a section (review, and [04](04-leptos-integration.md) §11's lesson) |
| **F3** The translator workflow | Export, translate, import (JSON, XLIFF 2); pseudo-locales; `stats`; checks in CI | compiled / `run=` blocks |
| **F4** Testing and troubleshooting | **Testing:** `with_locale`, `TestBackend` snapshots in several languages, pseudo-locales for layout. **Troubleshooting:** a stale manifest, empty text, pages stuck in the default language, the in-crate `tr!` rule | compiled blocks |
| **F5** First pages and upgrading | A one-crate landing page; Getting started before the crate map; `[output.html.playground] runnable = false`; mechanism moved into callouts; **"Upgrading from 1.x"** (crates, features, paths, the API changes) | `mdbook build`; `cargo xtask docs` |

## Part G — removal, release, exit (G1 after Parts B–F; G2 after G1; G3 after G2; G4 last)

| Task | Deliverable | Done when |
|---|---|---|
| **G1** 18 crates become 16 | **Code and lists:** <br>• the four shims deleted; <br>• `xtask/src/packages.rs`, `msrv`, `docs`, `api`, `package` and `release` updated for the 14 crates plus the two Leptos UI helpers; <br>• `version = "2.0.0"`, with `=2.0.0` pins. <br>**Docs:** <br>• a `## 2.0.0` changelog entry absorbing 1.1.0's items; <br>• `docs/versioning.md`: what 2.x promises (per mode, the generated items, the Leptos lines), and where the releases stand; <br>• `CLAUDE.md`'s client-path list names `mf2` | `cargo xtask ci` and `docs` |
| **G2** The release checks | `cargo xtask release` as a dry run at 2.0.0 for the 16 crates: <br>• every name ours; <br>• semver-checks against 1.0.0 treating it as a major; <br>• `docs-rs`, `msrv`, `msrv --below`, `package --check --test`. <br>**Owner questions, asked here:** the 2.0.0 stubs for `leptos-mf2` / `mf2-axum`; reserving the two unpublished names. The publish stays the owner's | every existing negative control still refuses; the dry run green |
| **G3** Cold start | A fresh agent with only the book and `mf2 init` (the crates through `[patch.crates-io]` at `cargo xtask package`'s output) builds the CLI, the TUI and the Leptos application. Each stumble is fixed, and the run repeated | a clean run, recorded |
| **G4** Exit | `plans/phase-10-results.md`; `P10` in `conformance/src/matrix.rs`, `current_phase = "P10"`; the probes deleted; the master plan's "Later" reviewed | written; the harness green at `P10` |

## The order that keeps `cargo xtask ci` green

- **A0–A9:** plans, docs, excluded workspaces and xtask code, each with its tests.
- **B1 is one atomic commit.** A type cannot live in two crates, and the shim keeps every old path
  working, in examples and the book too.
- **Internal users move one at a time behind the shims** (B2–B4).
- **New APIs land before old ones go.** An old API is removed only in the commit that rewrites the
  page using it (C8, D6).
- **A commit that touches the book also runs the full `cargo xtask docs`,** since `ci` runs it with
  `--no-build`. A commit that touches the client runs `size`, `b12`, `codegen-matrix`,
  `scenarios`, the e2e checks, `l6-web` / `l7-web`, `churn` and `leptos-0-8`.
- **The shims go last** (G1), when nothing names them.

## Risks

| Risk | Mitigation |
|---|---|
| Feature unification turns `ssr`, `native`, `ratatui` and `axum` on together in a workspace | Every combination except the exclusive modes must compile, checked by a feature matrix. The generated module handles `ssr` + `native` |
| `links` is fragile under rust-analyzer or cargo-leptos | A2 first; fallback in the gate table |
| The function table costs bytes, or helper crates break hydration | A7 first; e2e on both lines; back to the owner with question 13's other options |
| A future `ratatui-core` 0.2 | An opt-in line feature, as with Leptos |
| The `links` name makes two `mf2` majors unable to share a graph | The major is in the name (`mf2-v2`); in practice two majors of this library in one application cannot work anyway |
| `mf2` is client-path code, so its `deny` lints cover the native modules too | Scoped `allow`s with reasons: the native panic from "Decided without asking", and std-only formatting in `native` / `axum` |
| New crate names hit crates.io's rate limit | Two new names only (the Leptos UI helpers) |
| The CLDR matching data is larger than expected on the client | Only the corpus's languages go in; measured; fallback in the gate table |

## Reuse

| Module | What is reused |
|---|---|
| `crates/leptos-mf2/src/{tr,arg,dynamic,markup}.rs` | moved as they are |
| `text.rs` | `with_active_text`, `format_with`, `with_scratch`: the ambient path |
| `state.rs` | `Setup`, `install`, `context_for`, `lookup_locale`: the matcher's web half |
| `catalog.rs` | the server store, and `current()` |
| `crates/mf2-native/src/locale.rs` | `match_locale`, `MULTI_SCRIPT`: the matcher's native half, replaced by C3 |
| `native.rs` | `load`, `system_time_zone` |
| `crates/mf2-ratatui/src/lib.rs` | the parts sink (markup stack, `Style::patch`, line breaks) |
| `crates/mf2-runtime/src/format.rs` | `Formatter::simple`, and the `Sink::push_catalog_text` seam pattern |
| `crates/mf2-catalog/src/reader.rs` | `Catalog::from_static`, `content_hash` |
| `crates/mf2/src/{corpus,message}.rs` | as they are |
| `crates/mf2-build/src/{codegen,features,build,report}.rs` | `Features::from_vars`, `Emit`, `to_cargo_warnings` |
| `crates/mf2-macros/src/expand.rs` | `emit` |
| `crates/mf2-cli/src/{init,cargo}.rs` | as they are |
| `crates/mf2-axum/src/*` | as they are |
| xtask `docs`, `size`, `b5`, `api`, `docs_rs`, `msrv`, `packages`, `package`, `release`, `ci`, `codegen_matrix`, `scenarios`, `leptos_0_8`, `cldr_sync` | as they are |
| `tools/i18n-fixture/tests/ui/` | the trybuild cases |
| `bench/runtime-bench` | B10 |

## Standing

* **No agent publishes, pushes, tags or rewrites history** (CLAUDE.md). The
  2.0.0 publish is the owner's, by `cargo xtask release --publish`.
* **`vendor/` and `comparison.md` are never committed;** stage files by name.
  The trippy port is edited only in C9, and stays in `vendor/`.
* **Owner questions are asked when they come up,** in plain English,
  and their answers are recorded here before the next task starts.
* **Leptos 0.9's release** is taken as a patch within the `leptos` feature's line, as the 1.x
  policy did.

## Exit (master plan §9, P10)

- [ ] every UX row falls for the four samples, against A1's 1.x counts (C8, D6)
- [ ] one crate: applications name `mf2` (+ `mf2-build`), with 16 published crates (B, D1, G1)
- [ ] the native ambient language, `Locale`, the Ratatui conversions and theme, and a one-line build (C1–C7)
- [ ] the web defaults, generated setup, typed languages and starters (D2–D5)
- [ ] one CLDR-based matcher everywhere (C3)
- [ ] the silent failures fixed (E1–E4)
- [ ] the book: native and web on 2.0, the MF2 guide, reference, translator workflow, testing, troubleshooting, upgrading (C8, D6, F1–F5)
- [ ] the gates held: web budgets within tolerance; `tui-gate` allocations, time and size (method §3)
- [ ] the trippy port finished and recorded (C9)
- [ ] the cold start clean (G3)
- [ ] `cargo xtask release` green as a dry run at 2.0.0; the publish is the owner's (G2)
- [ ] `cargo xtask ci` green; the harness green at `current_phase = "P10"`; `plans/phase-10-results.md` (G4)
