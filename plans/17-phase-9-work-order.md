# 17 — Phase 9 work order: release

Part of the [master plan](00-master-plan.md) (§9, P9). RFC 2119 keywords
apply. Written at the close of Phase 8 (A8) from
[phase-8-results](phase-8-results.md), the Phase 8 work order's task
records ([16](16-phase-8-work-order.md)), and the four owner answers below.

Phases 1–8 built the library and the way into it. Phase 9 makes it
something another project can depend on: the names checked, every public
item reviewed for a **1.0.0** promise, a written version policy tied to the
Leptos lines, packages that build from crates.io alone, documentation that
builds on docs.rs, a changelog, and one command that checks all of it and
that the owner runs to publish. It also takes the specification text out
of the tree, which a public repository or package cannot carry.

## State at the start (Phase 8's exit)

| In the tree | Where |
|---|---|
| 16 library crates, `publish = false` and `version = "0.0.0"` from `[workspace.package]` | `crates/*` |
| Leptos 0.9 (still `0.9.0-beta` on crates.io, 2026-09-25) the default line, 0.8 an opt-in; "`leptos-mf2`'s major version follows its default Leptos line" | [04](04-leptos-integration.md) §10 |
| The MF2 specification text vendored at `third_party/message-format-wg/spec/`, read only through `conformance/src/spec.rs`'s `SPEC_DIR` — a seam left for this move since Phase 1 | `conformance/src/spec.rs` |
| The suite, coverage matrix, size gates, browser checks, user documentation | `conformance/`, `xtask/`, `tools/e2e/`, `docs/` |
| No git remote; nothing pushed; no changelog, MSRV, docs.rs metadata or release command | — |
| Names: all 16 (`mf2`, `mf2-model`, `mf2-syntax`, `mf2-resource`, `mf2-catalog`, `mf2-runtime`, `mf2-fn-number`, `mf2-fn-datetime`, `mf2-host-web`, `mf2-host-std`, `mf2-locale-data`, `mf2-build`, `mf2-macros`, `mf2-cli`, `mf2-axum`, `leptos-mf2`) unregistered on crates.io on 2026-09-25 (its API, 404 each). Unrelated since D12: an `mf2_i18n*` family of nine crates, among them `mf2_i18n_leptos` | D12 |

## What Phase 8 left, and where it goes

* **`DateTimeValue::with_zone` relabels a wall time** instead of converting
  an instant (Phase 8 A7). A 1.0 cannot ship it as it is — **A2**.
* **`mf2 check` does not warn about an option a function does not have**
  (`dateStyle` on `:datetime` is ignored at run time, as MF2 says) — **A2**,
  a lint.
* **`mf2-cli`'s tests read `plans/05-tooling.md` from outside the crate**,
  to hold each report code against its table; they cannot travel in a
  package — **A4**.
* **Recorded, not tasked:** a date inside an island is not asserted in a
  browser (no example has one); WebKit and a screen reader were not run on
  the development machine. The changelog's known limitations say so (A6).

## Owner questions

1. **How the MF2 specification text is handled before anything goes
   public** — **answered (owner, 2026-09-25): downloaded on demand.**
   Since upstream #1112 the specification text may not be distributed
   publicly without Unicode's permission; the test suite may. The text
   leaves the tree: `cargo xtask spec-sync` fetches it into a git-ignored
   cache, and the tests stay vendored. Library users see no difference;
   contributors run one command. The old commits still hold it: a history
   rewrite is needed only if they are ever made public, and that is the
   owner's to order, never an agent's. Recorded in the master plan (D13,
   §7) and 01 §1; the work is **A0**. The question as it was put: download
   on demand, seek Unicode's permission, or keep the repository private and
   publish only the crates.

2. **When the crates are published, with Leptos 0.9 still a beta** —
   **answered (owner, 2026-09-25): at the end of this phase, on the
   beta.** Applications on a Leptos 0.9 pre-release, or on 0.8 through the
   opt-in, can use the library at once; when Leptos 0.9 is released, a
   patch release takes it. Recorded in the master plan (D11, §9 P9). The
   question as it was put: now on the beta, after Leptos 0.9 is released,
   or the framework-free crates now and the Leptos and Axum crates later.

3. **The first version and its promise** — **answered (owner,
   2026-09-25): 1.0.0.** A stable API from the first release: a breaking
   change needs 2.0. Every crate is versioned together. What 1.0 promises
   and what it does not is **A3**'s policy, and the review that earns the
   promise is **A2**. Recorded in the master plan (D11, §9 P9) and 04 §10.
   The question as it was put: 0.1.0 (the API may change between minor
   versions) or 1.0.0.

4. **Who presses publish** — **answered (owner, 2026-09-25): the owner,
   by one command.** `cargo xtask release` checks everything and does a
   dry run, and CI runs that dry run on every change; the real publish is
   the owner's, with the owner's own crates.io login. No token is stored anywhere,
   and no agent publishes. Recorded in the master plan (§9 P9); the work
   is **A7**. The question as it was put: the owner by one command, or CI
   on a pushed tag with a stored token.

## Part A — tasks (A0 first; A1–A3 in any order; A4 after A1; A5 after A2; A6 and A7 after A4; A8 last)

| Task | Deliverable | Done when |
|---|---|---|
| **A0** The specification text out of the tree | *Owner question 1; 01 §1.* `cargo xtask spec-sync` vendors `test/` and `LICENSE` as now and fetches `spec/` **at the pinned commit** into a git-ignored cache (`target/xtask-cache/`, as `w3c-message-resource` is), verified against digests the `PIN` records; `SPEC_DIR` becomes the cache (one line, as its comment promises); `spec/` is removed from the tree. Every consumer of `SPEC_DIR` that runs without the cache (the ABNF-driven generators, the coverage matrix, the data-model schema check) fails with a message naming `cargo xtask spec-sync`, never skips. CI's jobs run `spec-sync` first (a PIN-named upstream, through the xtask — inside the boundary). Nothing vendored or generated **quotes** the text: `coverage.toml`'s `says` fields and `COVERAGE.md` are our paraphrases, checked once and recorded. | `spec/` absent from the tree and from `git ls-files`; `cargo xtask ci` green from a clean clone after `spec-sync`; without the cache, the first consumer fails naming the command; negative control: a digest altered in the `PIN` is refused |
| **A1** Names and package metadata | *D12.* The 16 names re-checked on crates.io the day of the release (A7 checks again). Each published crate: `description`, `readme` (a short README per crate, pointing to `docs/`), `keywords`, `categories`, `rust-version` (A3), `repository` / `homepage` **only when the owner names a public remote** (none exists; left out, not invented). `license`: `MIT`, and for a crate that ships data derived from CLDR (`mf2-locale-data`'s `data/`) `MIT AND Unicode-3.0` with Unicode's licence text in the package. `[workspace.package] version = "1.0.0"`; `publish` on for the 16, off for everything else (conformance, bench, examples, tools, xtask, fuzz). Every dependency between our crates is an exact requirement (`=1.0.0`), since the generated module, the macro and the runtime share `#[doc(hidden)]` items that A3's policy exempts from semver. | `cargo metadata` shows exactly the 16 publishable, each with the fields above; a test in `xtask` holds that list, the version and the exact inter-crate requirements; negative control: a library crate with `publish` off, or a caret requirement between two of ours, fails it |
| **A2** The API review for 1.0 | *Owner question 3.* Every public item of the 16 crates, reviewed against what applications and the generated code need, before it is promised. For each crate: the public API listed (`cargo public-api`, from crates.io, on the nightly rustdoc JSON — rustup) and committed under `crates/<name>/api.txt`; items only the macro or the generated module use are `#[doc(hidden)]` and named in A3's exemption; enums and structs that may grow are `#[non_exhaustive]`; error types follow the conventions (`thiserror`, `src/error.rs`, `#[from]`); `#![warn(missing_docs)]`, then none missing. **Fixed here:** `DateTimeValue::with_zone` converts an instant (a value that is a wall time says so in its constructor's name), with a test that `instant(t).with_zone("Europe/Paris")` under `timeZone=input` shows Paris time; `mf2 check` warns on an option a function does not define (a new lint, with a seeded-drift case), and every existing corpus is re-checked. Each change the review makes is listed in the task record with why. A change to a user-visible behaviour, or a removal an application would notice, goes to the owner. | `api.txt` committed for every crate and diffed by `cargo xtask ci` (a change to it must be committed with it); no missing docs; the two fixes with their tests; negative control: an item made public without updating `api.txt` fails CI |
| **A3** The version policy, and the MSRV | *Owner question 3; 04 §10.* `docs/versioning.md`, written before A7 depends on it: 1.x's promise — the public API of the 16 crates, the `tr!` forms, the `mf2` CLI's commands and flags, the resource format as `mf2 fmt` writes it; **not** promised — `#[doc(hidden)]` items, the `.mf2b` catalog format and the manifest (master plan non-goal: a server and client from one build agree through `manifest_hash`), CLI report wording, and exact figures. Leptos lines: 0.9 the default and 0.8 an opt-in in 1.x; a Leptos 0.9 pre-release or its release taken in a patch; a new line (0.10) added as an opt-in feature in a minor; **changing the default line or dropping one is 2.0** (04 §10's rule). The W3C Message Resource draft: a change upstream is followed with `mf2 fmt` able to migrate, and a change that would reject a file 1.0 accepted is 2.0. **MSRV** measured, not assumed: the oldest stable Rust that builds the 16 crates for native and `wasm32-unknown-unknown` with their feature sets (edition 2024 needs 1.85; Leptos 0.9.0-beta states 1.88), written as `rust-version`, and a CI job on that toolchain. Raising it is a minor, said in the changelog. | the document written and linked from `docs/README.md` and every crate README; `rust-version` set; the MSRV job green, and the version below it measured failing; `cargo-semver-checks` (crates.io) wired into `cargo xtask release` against the published version from the second release on |
| **A4** Packages that build from crates.io alone | `cargo package --workspace` (and `cargo publish --workspace --dry-run`, which verifies each package against the others in dependency order) green for the 16. Each package's file list audited and committed as `crates/<name>/package.txt`: no `third_party/` text, no `plans/`, no file outside the crate; sizes recorded (crates.io's 10 MB limit; `mf2-locale-data`'s `data/` is 7.3 MB raw). **`mf2-cli`'s tests that `include_str!` `plans/05-tooling.md`** move to a place that is not packaged (an `xtask` test, or `exclude`d from the package), keeping the check. A package's own `cargo test` from its unpacked `.crate` passes, or the test that cannot is excluded from the package and still run in the workspace. | the dry run green; `package.txt` committed for each and diffed by CI; negative control: a file from `third_party/message-format-wg/spec` added to a package's `include` fails the audit |
| **A5** docs.rs | `[package.metadata.docs.rs]` in each crate, with a feature set that compiles (the Leptos modes `ssr` / `hydrate` / `csr` and the lines `leptos-0-9` / `leptos-0-8` are exclusive: the docs show `ssr` on 0.9, and say which items the other modes add); `#[cfg_attr(docsrs, doc(cfg(…)))]` on feature-gated items. `cargo xtask docs-rs` builds every crate as docs.rs does (nightly, `--cfg docsrs`, its target list) with `rustdoc::broken_intra_doc_links` denied. Each crate's front page says what it is for and links the user guide. | `cargo xtask docs-rs` green in CI; every crate's front page non-empty; negative control: a broken intra-doc link fails it |
| **A6** The changelog and release notes | `CHANGELOG.md` (one file for the workspace, since every crate is versioned together): 1.0.0 — what is in it (full MF2 at `5c4ddb27`, the delivery modes, Fluent migration, XLIFF 2, the reader's time zone, budgets as measured), and **known limitations**: Leptos 0.9 is a beta; WebKit and a screen reader were not run; a date inside an island is not asserted in a browser; the W3C Message Resource format is a draft. The root `README.md` gains install lines for 1.0. | written; `cargo xtask release` refuses a version with no changelog entry |
| **A7** `cargo xtask release` | *Owner question 4.* One command, `cargo xtask release [--publish]`. Without `--publish` (and in CI, on every change): `cargo xtask ci` green, the names still free or ours, version and changelog agree, A1's metadata test, A2's API listings unchanged or committed, A4's package audit, the docs.rs build, the MSRV build, `cargo publish --workspace --dry-run`, and the tree clean. With `--publish`, **run by the owner only**: the same checks, then `cargo publish --workspace` with cargo's own stored login — the command stores and reads no token of its own, and refuses in CI (a `CI` environment variable). It prints the tag to create; it never pushes. | the dry run green in CI (`.forgejo/workflows/ci.yml`); each check's refusal shown once (a negative control per check: a dirty tree, a missing changelog entry, a taken name simulated, an unaudited file); `--publish` under `CI=true` refuses; the owner has what is needed to publish |
| **A8** The Phase 9 results and what follows v1 | `plans/phase-9-results.md`; the master plan's "Later" list reviewed into a post-1.0 order. | written |

## A0 — the specification text out of the tree: what was built

* **`cargo xtask spec-sync`** vendors `test/` and `LICENSE` as before and
  writes `spec/` to `target/xtask-cache/message-format-wg-spec/spec/`
  (git-ignored), then a `COMMIT` stamp beside it, last, so a partial
  write is never taken for a whole one. Each spec file must match the
  SHA-256 in the `PIN`'s new `digests` field — 14 files, recorded by the
  first sync after the move (the fetched text was byte-identical to the
  copy that left the tree); a new `--rev` re-records them. `--check` also
  fills the cache, and refuses a PIN with no digests. A `spec/` found
  under `third_party/message-format-wg/` counts as not upstream's
  (`--check` fails on it; a sync removes it). The digest helpers moved
  from `xliff_sync.rs` to `pin.rs`, shared by both.
* **One gate for every reader:** `conformance/src/spec.rs`'s `spec_dir`
  / `spec_path` / `read_spec` refuse a missing cache or one stamped with
  another commit (`Error::SpecMissing`, naming the command). Behind it:
  the L5 build script, the three generated-case tests and `l4gen`'s, the
  L2 schema check (`Harness::load`), the coverage matrix (its test, the
  `statements` example, `conformance-report`), `fuzz-seed`, `l4-wasi`,
  the `differential` example. `cargo xtask ci` checks the cache before
  its first step instead of failing minutes in, at the build script.
* **CI:** every job of `ci.yml` (7) and `nightly.yml` (6) runs
  `cargo xtask spec-sync --check` straight after the toolchain.
* **Nothing committed quotes the text.** Every tracked file outside
  `third_party/` was compared with the spec's Markdown for shared runs of
  10 words or more (a one-off script, not committed), then
  `coverage.toml` and `COVERAGE.md` for runs of 6 or more. One `says`
  field shared 8 words with `errors.md` and was reworded (COVERAGE.md
  regenerated, 164 statements, 0 gaps). Three prose quotations — in
  `mf2-fn-datetime/src/function.rs`, `mf2-model/src/json.rs` and
  `plans/phase-7-results.md` — were paraphrased. What remains matches
  by construction and is not prose: regular expressions and number
  lists (the datetime literal grammar, the rounding increments), test
  messages taken from the suite, the BCP 14 key-word list, and one
  11-word description of error 103 (`mf2-syntax/src/code.rs`, held
  against `plans/05-tooling.md`'s table).
* **Shown** (2026-09-25): `spec-sync --check` green; with one digest
  altered in the `PIN`, both `spec-sync` and `--check` refuse it naming
  the file and both digests, and write nothing; with the cache moved
  aside, the L5 build script, the coverage test and `conformance-report`
  each fail naming `cargo xtask spec-sync` (and `spec.rs`'s unit test
  holds the missing, unstamped and stale cases); `cargo xtask ci` green
  in the working tree; `spec/` absent from `git ls-files`. In a fresh
  clone: `cargo xtask ci` refuses up front naming the command, then after
  `spec-sync --check` green (`git status` clean after it).
  The clone's first `cargo xtask ci` failed elsewhere:
  `mf2-locale-data`'s `icu_blob` `vectors`. The clone has no lock file,
  so it resolved `icu_time_data` 2.3.1, a time-zone data patch released
  after the working tree's lock (2.3.0), and the zones vector moved
  (16,755 → 16,857 B). Nothing A0 did caused it; a `cargo update` in the
  working tree would have shown the same. Taken: the vector and 02 §4.9's
  zone sizes updated to 2.3.1 (+102 B raw per locale, 18.3–21.0 KB gz,
  B4's 25 KB met), in its own commit. After that, `cargo xtask ci` green
  in the clone and in the working tree. That this vector moves with
  every tz data release matters to A4 (a package's tests run against
  what crates.io resolves), and is left there.
* **Not done here:** the old commits still hold the text; rewriting them
  is the owner's (owner question 1).

## Standing

* **No agent publishes, pushes, tags or rewrites history** (CLAUDE.md).
  The publish is the owner's (owner question 4); removing the
  specification text from old commits is the owner's to order, if the history is
  ever made public (owner question 1).
* **Leptos 0.9's release** is taken as a patch when it appears (A3's
  policy). If it lands during this phase, the phase takes it and re-runs
  what A0 of Phase 8 re-ran.

## Exit (master plan §9, P9)

- [x] the specification text out of the tree, fetched on demand; the suite still vendored (A0)
- [ ] the 16 names verified and every package's metadata complete (A1)
- [ ] every public item reviewed, listed and documented for 1.0.0; `with_zone` fixed; the unknown-option lint (A2)
- [ ] the version policy written, the MSRV measured and held in CI (A3)
- [ ] the packages audited and verified from crates.io's point of view (A4)
- [ ] the documentation builds as docs.rs builds it (A5)
- [ ] the changelog with 1.0.0 and its known limitations (A6)
- [ ] `cargo xtask release` green as a dry run in CI; the publish is the owner's (A7)
- [ ] `cargo xtask ci` green; the conformance harness green at `current_phase = "P9"`
- [ ] `plans/phase-9-results.md` written (A8)
