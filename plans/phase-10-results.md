# Phase 10 results — 2.0, the user experience

What Phase 10 built and what it measured, against
[18-phase-10-work-order](18-phase-10-work-order.md). Each task's own record
(what was built, how it was checked, what it found) is in the work order:
the Done list under "Where the work stands", and, for Parts A–C, the
records under each task's heading. This file gathers them at the exit: what
2.0 is, what each part delivered, the owner's decisions, the figures as
last measured, the change of method, and what is left.

## Status at exit

**Every task of the work order is done** (2026-09-30), and
`current_phase = "P10"` is in the exit commit with the harness green
(`cargo xtask conformance-report`: 612 tests, 612 ledger entries, 164
normative statements, 0 gaps). `cargo xtask release` is green as a dry run
at 2.0.0 (G2). **What stands between the tree and crates.io is the owner's
publish**, below under "What is left".

## What 2.0 is

One crate, **`mf2`**, with features, and `mf2-build` for the build script
(D16–D24; owner questions 1, 2, 4, 6). 1.x's `leptos-mf2`, `mf2-axum`,
`mf2-native` and `mf2-ratatui` are gone (G1); the two published ones get a
2.0.0 pointer release (question 34).

| Feature | What it turns on |
|---|---|
| (none) | the call-site types, `tr!` / `msg_id!`, `IntoArg`, the one CLDR-based locale matcher |
| `native` | the ambient store: `install()`, `set_locale`, `with_locale`, `Display` / `to_string()` on a message; the system time zone |
| `ratatui` | `Span` / `Line` / `Text` from a message, one app-wide `Theme` (implies `native`) |
| `leptos` + `ssr` / `hydrate` / `csr` (or `leptos-0-8`) | `mf2::leptos`; the six components in `mf2-leptos-ui-0-9` / `-0-8` |
| `axum` | `mf2::axum`: the negotiator as a tower layer, catalog routes, the redirect |

`native`, `ratatui` and `axum` beside a browser mode are refused only when
compiling for `wasm32` (question 24). A generated module gives each
application `Locale` (with `FromStr`, `name()`, `ALL`), `install()`,
`setup()`, `markup::*` and a prelude with `tr!`. `mf2 init --cli`, `--tui`,
`--ssr`, `--islands`, `--csr` start each kind of application.

## What each part delivered

- **A — plan, baselines, probes.** 1.x's figures and `examples/tui` with
  `cargo xtask tui-gate` (A1); `links` metadata, the in-crate `tr!`, one
  crate for the web (A2, A3, A6); the ambient store's cost (A4); `Display`
  and `Debug` against the client wasm (A5, A9: a lean `Display` and `Debug`
  through `write_str`); names and coherence across both Leptos lines (A7);
  the 2.0 design, approved ([19](19-native-and-terminal.md); A8).
- **B — one crate, with shims.** The call-site types and `mf2::leptos` into
  `mf2`, the components by static dispatch (B1); `mf2::native` (B2);
  `mf2::ratatui` (B3); the repository's own users on `mf2` (B4); the API
  listed per mode (B5). Every web artifact byte-identical or within ±64 B gz.
- **C — native and Ratatui.** `IntoArg` (C1); the ambient store (C2); one
  matcher over CLDR 48's `languageMatching` (C3); the generated module and
  `setup()` (C4); Ratatui conversions and the theme (C5); the build
  (`links = "mf2-v2"`, `mf2_build::run()`; C6); `mf2 init --cli` / `--tui`
  (C7); `examples/tui` on 2.0 (C8); the trippy port as the acceptance test,
  untracked in `vendor/` (C9).
- **D — the web.** `mf2::axum` (D1); the default negotiator (query, cookie,
  `Accept-Language`) (D2); the negotiator as a tower layer, demos on plain
  `leptos_routes` (D3); `LocaleTag`, a self-listing `<LocaleSwitcher/>`
  (D4); `mf2 init --ssr|--islands|--csr` (D5); book, README, demos and
  benches on `mf2` alone (D6).
- **E — the silent failures.** `dropped-markup` an error by default (E1);
  `@do-not-translate` neither missing nor covered (E2); `mf2 import` writes
  nothing on an error (E3); server warnings once per process, and a debug
  browser warning with no catalog (E4).
- **F — the 2.0 book.** MF2 for developers (F1); configuration, lints and
  features reference pages (F2); translating (F3); testing and
  troubleshooting (F4); the landing page and the upgrade guide (F5). Every
  `run=` block is compiled or run by `cargo xtask docs`.
- **G — removal, release, exit.** The shims deleted, 16 crates at 2.0.0
  (G1); the release dry run green (G2); the cold start (G3, below); this
  file, `P10`, the probes deleted (G4).

## The figures, as last measured

| What | 2.0 | 1.x (A1, `2fb7f54`) | Command |
|---|---|---|---|
| B1, the fixed cost | 26,711 B gz (limit 30,720; gate ±64 B) | 26,676 | `tools/checks/run.sh … --only sizes` |
| B5, per call site | 8.2 B gz (limit 40) | 8.2 | the same |
| the whole app (1,860 sites) | 41,903 B gz | 41,889 | the same |
| B7 catalogs en / pl / en-XA / ar-XB | 18,072 / 24,137 / 21,537 / 18,423 B br (limit 23,296 for `en`) | the same | the same |
| `tui-mf2`, stripped | 1,812,352 B | 1,965,320 (the gate) | `cargo xtask tui-gate` |
| allocations a frame (en/de/es/fr) | 1,329 (1,328 in `fr`) | 1,815–1,817 | the same |
| frame time (under load 3.3) | 355.7 µs | 363.9 | the same, alternated (C8) |
| conformance | 612 / 612 | 612 / 612 | `cargo xtask conformance-report` |
| `trip` (the trippy port), stripped | 8,917,240 B | upstream 8,392,456 (+6.3 %) | C9 |
| demo-csr with the full matcher | +2,870 B gz | — | C3 (question 28) |

**The UX targets** (19 §2), all met: setup lines 35 → 13 (CLI), 48 → 24
(`examples/tui`), 48 → 19 (two-crate), 96 → 22 (Leptos `hello`); crates
named `mf2` + `mf2-build` everywhere; the Leptos page needs 3 commands,
none of them this library's (C8, D6).

**The cold start** (G3): a fresh agent with only `docs/`, `mf2 init` and
the packaged crates built and ran a CLI, a Ratatui TUI and an SSR Leptos
application in two languages each, and every check the guide names. Four
runs: 1 found E4's warning at start-up; 2, `mf2 check` offline and no
`.gitignore`; 3, `unused-id` on the generated `language.<tag>` ids, and
`-C` reading paths in the wrong directory; 4 was clean but for one wording
slip. Each finding was fixed before the next run.

## The owner's decisions (questions 1–35)

- **Shape of 2.0:** skip 1.1.0, ship 2.0.0 (1); one crate with features
  (2); native and web together, native first (4); `mf2-axum` folded in (6);
  `mf2::leptos` with 0.8 kept (8); one helper crate per Leptos line (13);
  backward compatibility not a priority (30); `leptos-mf2` / `mf2-axum`
  pointer releases (34); `mf2-native` / `mf2-ratatui` not reserved (35).
- **Native and terminal:** app-wide language with a per-thread override
  (3); one app-wide theme (5); trippy as the acceptance test (7); the
  design approved as written (17).
- **Matching:** follow CLDR (9, 11); no rule on top for Chinese scripts
  (15); UTS #35's text fetched, cached only (16); the full matcher without
  a server (28); `mf2` is `MIT AND Unicode-3.0` (29).
- **Size and formatting:** a lean `Display` everywhere, `Debug` through
  `write_str` (14); the `{:?}` cap counts our own code (18); demo-ssr's
  −136 B gz holds (19).
- **Web setup:** refuse `native` beside a browser mode only for `wasm32`
  (24); the generated `setup()` (31).
- **Tooling and the book:** the silent failures required (10); the
  chapters and starter templates (12); `neutral-numbers` reworded (33).
- **Housekeeping and method:** `CLAUDE.md`'s client-path list (20); probe
  branches and measurement worktrees removed (21, 25); the history with
  `46303b9` kept (22); `LocaleSource` keeps its name in two modules (23);
  question 24's change first (26); lean mode (27); task agents at `high`
  (32).

## The method change: lean mode (question 27)

After question 26 the seven task agents re-read 625M tokens between them;
C1–C3 cost 130–145M each for small code changes. The cost was the method:
per-task baselines, byte-level investigations, the whole suite run step by
step in every task, and long records that later agents re-read. From C4 on,
one fresh agent per task read only its brief, its design section and the
code it changed, ran `cargo xtask ci`, committed and reported in ten lines,
with a Done entry of five lines at most. The coordinator ran the whole
suite as one quiet script after each task and compared its table with the
previous one; only a regression got an agent. The gates did not change.

The suite is now **`tools/checks/`** (`run.sh`, `compare.sh`, `README.md`
and their helpers, moved from `probes/p10-checks/` and the probes it
called at G4). Records in `plans/` that cite `probes/…` paths are history;
those paths are in git before the G4 commit.

## What is left

1. **The publish — the owner's.** In order (plans/18, G2's entry):
   `cargo xtask release --publish` (the 16 crates); then in `pointers/`,
   `cargo publish --no-verify -p leptos-mf2` and `-p mf2-axum`; then the
   tag `v2.0.0` as the release prints it.
2. **Routed to the master plan's "Later"** (§9), with the earlier list:
   the refusal's note naming the hidden `KindNeither::__mf2_kind`, and a
   whole-argument underline (needs `Span::join`, nightly); a shorthand
   `tr!("id", error)`; four allocations per `.match` message in the
   runtime; `mf2-catalog`'s timing test `linear.rs` under heavy load; the
   item docs still citing `plans/` (183 comment lines).
