# rust-mf2

Unicode MessageFormat 2 (MF2) for Leptos: the full spec, lazily loaded per-locale
binary catalogs, minimal wasm. A Rust monorepo (one Cargo workspace). License: MIT;
`mf2` and `mf2-locale-data`, which ship CLDR data, are MIT AND Unicode-3.0.

## Start here

1. Read the documents in `plan/` (not its `archive/`). `plan/01-size-and-features.md` is the
   current proposal: native binary size, the feature structure, and three optimizations.
2. **Never read, search, list or reference `plan/archive/`.** It holds the plans of phases
   0–10 (the former `plans/` directory) and is kept as history only. Paths such as
   `plans/03-runtime.md` in code comments point into it; do not follow them. If something
   seems to be decided only there, ask the owner.
3. **Phases 1–10 are done.**
   - Release: 1.0.0 and **2.0.0 are on crates.io** (2.0.0 published 2026-09-30, tag `v2.0.0`);
     1.1.0 was never published.
   - The check suite is `tools/checks/` (its README).
   - `vendor/` (the owner's trippy port and reference checkouts) and `comparison.md` are untracked
     and never committed: stage files by name.

## Boundary — this repository is self-contained

This section is the canonical statement of the rule; other documents point here.

* Do **not** read, search, list or reference anything outside the repository
  root: no parent directories, no sibling directories, no other projects on this
  machine. Everything needed is in the tree (`plan/`, `third_party/`, the
  committed corpora in `bench/corpora/`) or is produced by a Phase 0 task
  (CLDR inputs: A4; the reference workload: A7). The Phase 0 probes and the
  planning session's scratch code (`probes/`, `probes/audit/`) were deleted in
  Phase 2 (C5) and stay in the first commit's history.
* Network access is limited to:
  1. `rustup` — toolchains and targets;
  2. crates.io, docs.rs and official crate repositories — dependencies and their
     documentation;
  3. the upstreams named in `third_party/*/PIN`, fetched **only** through the
     `cargo xtask *-sync` commands;
  4. `code.forgejo.org` — to look up maintained action forks for CI;
  5. the npm registry and Playwright browser downloads — only for the browser
     test harness under `tools/e2e/`.

  Nothing else. In particular, do not fetch other MF2 or i18n projects to copy
  from.
* If a requirement seems to be missing, ask the owner. Do not go looking for an
  application to infer it from.

## Non-negotiables

* **Full MF2, never a subset.** The vendored WG test suite
  (`third_party/message-format-wg/test/`) runs at every layer; every test has a
  ledger entry; a test that cannot pass yet is an `xfail` naming the phase that
  fixes it — never a silent skip.
* **No locale data in the client wasm** — no message text, ids, argument names,
  plural rules or symbols. CI greps for canaries.
* **Budgets are requirements.** A size or speed
  claim comes with a measurement and the command that produced it.
* **Client-path crates** (`mf2-catalog` reader, `mf2-runtime`, `mf2-fn-*`,
  `mf2-host-web`, `mf2`'s call-site types and `mf2::leptos`, `mf2-leptos-ui-0-8` /
  `-0-9`) are `no_std` where stated, `forbid(unsafe_code)`,
  and fmt-free / panic-free: no `format!`, `Debug`/`Display` use, `unwrap`, or
  panicking indexing on the client path.
* **Don't build worse than what exists.** Replacing an existing crate with our
  own needs a measured baseline, a gate, and a fallback.
* If the plan is wrong, change the plan in the same change that departs from it.

## Conventions

* Rust 2024 edition; no `mod.rs`; dependencies at their latest versions, declared
  once in `[workspace.dependencies]`; `Cargo.lock` is not committed.
* Errors: `thiserror`, defined in each crate's `src/error.rs`, conversions via
  `#[from]`.
* CLI arguments: `clap`. Anything that serves HTTP: `axum`.
* Front-end code in examples: flexbox layout, WCAG 2.2 AA, consider schema.org,
  SVGs as external files (never inline).
* Containers: podman / buildah, never docker. CI: `.forgejo/workflows/`, Forgejo
  action forks (`https://code.forgejo.org/...`), never Woodpecker. The one
  exception is the user guide (owner, 2026-09-27): the repository is published
  on GitHub, and `.github/workflows/book.yml` builds it with mdBook and deploys
  it to GitHub Pages. `mdbook build` writes to `target/mdbook`; the rendered
  book is never committed.
* Search with `rg`.
* `third_party/` is read-only and changes only through `cargo xtask *-sync`.
* **Agents may stage and commit** as the work needs (owner, 2026-09-21): on
  the current branch, one coherent change per commit, with a message that says
  why — and, when it moves a budget or a ledger status, why that moved. Never commit with `cargo xtask ci` red. Do **not** push, force,
  rewrite published history or change git configuration unless the owner says
  so.

## Layout

`plan/` · `third_party/` (pinned spec, test suite, CLDR subset, resource-format
pin) · `crates/` · `conformance/` · `bench/` · `examples/` · `fuzz/` · `xtask/` ·
`tools/` · `.forgejo/workflows/`. (`probes/`, the throwaway Phase 0
experiments, was deleted in Phase 2; see the first commit.)
