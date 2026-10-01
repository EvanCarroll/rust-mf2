# rust-mf2

Unicode MessageFormat 2 (MF2) for Leptos: the full spec, lazily loaded per-locale
binary catalogs, minimal wasm. A Rust monorepo (one Cargo workspace). License: MIT;
`mf2` and `mf2-locale-data`, which ship CLDR data, are MIT AND Unicode-3.0.

## Start here

1. Read `plans/00-master-plan.md` — the single source of truth (goals, layout,
   feature flags, decisions D1–D24, phases, risks).
2. Read the companion document for the area you touch (`plans/README.md` maps
   crates to documents).
3. **Phases 1–10 are done.**
   - Release: 1.0.0 and **2.0.0 are on crates.io** (2.0.0 published 2026-09-30, tag `v2.0.0`);
     1.1.0 was never published.
   - Phase 10 — 2.0, the user experience — ran from `plans/18-phase-10-work-order.md`; its
     results are in `plans/phase-10-results.md`. The check suite is `tools/checks/` (its README).
   - Its decisions are the master plan's D16–D24: one crate, `mf2` with features; native and
     Ratatui first; the web's setup; one CLDR-based matcher; the silent failures; the book.
   - `vendor/` (the owner's trippy port and reference checkouts) and `comparison.md` are untracked
     and never committed: stage files by name.

   Built so far (`crates/mf2-model`, `crates/mf2-syntax`,
   `crates/mf2-catalog`, `crates/mf2-runtime` and its hosts,
   `mf2-locale-data`, `mf2-fn-number`, `mf2-fn-datetime` with both date
   backends, the `intl` client option, the `mf2` facade with the call-site
   core, `mf2-resource`, `mf2-build`, `mf2-cli`, the generated module,
   `tools/i18n-fixture`, `mf2-macros` and layer L5 in `conformance/l5/`;
   `leptos-mf2`, `mf2-axum`, layer L6 in `conformance/src/l6.rs` and
   `conformance/l6-web`, and `examples/demo-ssr`; islands, CSR and lazy
   routes, `examples/demo-islands` and `examples/demo-csr`, layer L7 in
   `conformance/l7-web`, the coverage matrix, and the user documentation in
   `docs/`; `mf2 convert --from fluent` / `--from leptos-fluent`, XLIFF 2
   export and import, the `leptos-fluent` A/B in `bench/fluent-ab/`, and
   dates in the reader's time zone; the specification text out of the
   tree, the API review and `api.txt` listings, the version policy and MSRV,
   packaging, docs.rs, the changelog, `cargo xtask release`, `mf2-native`
   and `mf2-ratatui`, and the user guide verified by running it;
   `plans/phase-1-results.md` … `plans/phase-9-results.md`).

## Boundary — this repository is self-contained

This section is the canonical statement of the rule; other documents point here.

* Do **not** read, search, list or reference anything outside the repository
  root: no parent directories, no sibling directories, no other projects on this
  machine. Everything needed is in the tree (`plans/`, `third_party/`, the
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
  from; the audit of prior art is already in `plans/`.
* If a requirement seems to be missing, ask the owner. Do not go looking for an
  application to infer it from.

## Non-negotiables

* **Full MF2, never a subset.** The vendored WG test suite
  (`third_party/message-format-wg/test/`) runs at every layer; every test has a
  ledger entry; a test that cannot pass yet is an `xfail` naming the phase that
  fixes it — never a silent skip (`plans/01-conformance.md`).
* **No locale data in the client wasm** — no message text, ids, argument names,
  plural rules or symbols. CI greps for canaries.
* **Budgets are requirements** (`plans/06-size-and-perf.md`). A size or speed
  claim comes with a measurement and the command that produced it.
* **Client-path crates** (`mf2-catalog` reader, `mf2-runtime`, `mf2-fn-*`,
  `mf2-host-web`, `mf2`'s call-site types and `mf2::leptos`, `mf2-leptos-ui-0-8` /
  `-0-9`) are `no_std` where stated, `forbid(unsafe_code)`,
  and fmt-free / panic-free: no `format!`, `Debug`/`Display` use, `unwrap`, or
  panicking indexing on the client path.
* **Don't build worse than what exists.** Replacing an existing crate with our
  own needs a measured baseline, a gate, and a fallback (see D1).
* If the plan is wrong, change the plan in the same change that departs from it.
  The master plan wins over companions.

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
  why — and, when it moves a budget or a ledger status, why that moved (master
  plan §11). Never commit with `cargo xtask ci` red. Do **not** push, force,
  rewrite published history or change git configuration unless the owner says
  so.

## Layout (target — see master plan §4)

`plans/` · `third_party/` (pinned spec, test suite, CLDR subset, resource-format
pin) · `crates/` · `conformance/` · `bench/` · `examples/` · `fuzz/` · `xtask/` ·
`tools/` · `.forgejo/workflows/`. (`probes/`, the throwaway Phase 0
experiments, was deleted in Phase 2; see the first commit.)
