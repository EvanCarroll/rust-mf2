# 17 — Phase 9 work order: release

Part of the [master plan](00-master-plan.md) (§9, P9). RFC 2119 keywords
apply. Written at the close of Phase 8 (A8) from
[phase-8-results](phase-8-results.md), the Phase 8 work order's task
records ([16](16-phase-8-work-order.md)), and the four owner answers below.

**Native-app addition (2026-09-27):** `mf2-native` and `mf2-ratatui` extend
the original 16 published crates to 18. Release, API, package, docs.rs and
MSRV tooling tracks 18; verify the two names with the rest at release time.
The release carrying them is **1.1.0**, not 1.0.1 (owner, 2026-09-27): they
add public API to `mf2` (`Corpus`, `CatalogFile`, `Message`), `mf2-build`
(`Emit::Native`, `Emit::NativeFiles`) and `mf2-catalog`
(`Catalog::from_static`), and additions are a minor release.

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

5. **How much of the lower-level crates 1.0 promises** — **answered
   (owner, 2026-09-25): only what applications use.** Promised and
   documented: what applications and custom-function authors use (the
   build step, the Leptos and Axum setup, `tr!` and its argument types,
   the runtime's function interface, the stand-alone parser and data
   model). The internals our own crates share — `mf2-build`'s pipeline
   modules that `mf2-cli` and the conformance crate reach into, the
   compiled catalog's layout types, the raw locale-data tables — stay
   `pub` but `#[doc(hidden)]`, and `docs/versioning.md`'s exemption names
   them. A1's exact inter-crate requirements are what make that safe. The
   work is **A2**. The question as it was put (found by A2's listing:
   `mf2-build` 1,495 lines of `cargo public-api` output, `mf2-catalog`
   1,379, `mf2-locale-data` 1,213, most of it plumbing): promise only what
   applications use, promise everything public today, or make the
   internals private by restructuring first.

6. **The book's fixes, and the 1.1.0 publish** — **answered (owner,
   2026-09-27): folded into 1.1.0, before it is published.** A review and
   an empirical verification of the user guide (Part B) found claims that
   are false and code that does not do what the book says; some fixes add
   public API. They land before the publish, so no user sees the broken
   book or the missing pieces. The question as it was put: fold into
   1.1.0, ship 1.1.0 as it is and follow with 1.2.0, or correct the book
   now and schedule the code separately.

7. **A message whose id is known only at run time** — **answered (owner,
   2026-09-27): a public constructor.** The book told tool authors to use
   `msg_id!` and `TrDyn`, but `TrDyn`'s only constructor, `tr_dyn`, is
   `#[doc(hidden)]`. `TrDyn::new` becomes promised API; `tr!` stays the
   only form a page uses. **B5.** The question as it was put: add a public
   constructor, or drop the advice.

8. **The switcher on a site whose languages live in the URL** —
   **answered (owner, 2026-09-27): the switcher learns URLs.** With
   `PathPrefix` first, `<LocaleSwitcher>`'s `?lang=` cannot change the
   language (observed: `/en/page?lang=fr` stays English, and so does a
   `fr` cookie). The switcher gains an optional per-language URL function;
   without the wasm, the server redirects. **B4.** The question as it was
   put: teach the switcher URLs, or document a list of links instead.

9. **`class=` and `style=` with a message** — **answered (owner,
   2026-09-27): documented as a limit, no code.** `class=tr!(…)` does not
   compile (`TrArgs: IntoClass`); the book says so, with `attr:class=` as
   the workaround only if a compiled sample shows it works. **B10.** The
   question as it was put: make them work, or document the limit.

10. **A catalog from another deploy during a switch** — **answered
    (owner, 2026-09-27): `set_locale` reloads into the new language.** At
    boot a mismatch already reloads; at a switch it was refused silently
    (observed: the page stayed French, one console line). `set_locale`
    itself writes the choice and reloads, so every control gets it. **B2.**
    The question as it was put: `set_locale` reloads, or only the built-in
    switcher does.

11. **The server's language cookie** — **answered (owner, 2026-09-27):
    written only on an explicit choice.** `CookieLocale` wrote a one-year
    cookie on every response, even when the language came from
    `Accept-Language` or the default, turning a guess into a remembered
    choice. It is now written only when the language came from `?lang=` or
    the path; the client writes it on every switch. **B3.** The question as
    it was put: only on an explicit choice, or keep and document.

12. **`mf2 check`'s feature set** — **answered (owner, 2026-09-27): ask
    cargo, fall back to none.** Without `--features` it checked with none,
    so it warned where the build does not and failed where the build
    succeeds. It resolves the translation crate's features through cargo,
    as `compile --site` does, and without cargo checks with none and says
    so. **B8.** The question as it was put: ask cargo, or keep and
    document `--features`.

13. **`mf2 fmt` and blank lines** — **answered (owner, 2026-09-27): fmt
    keeps a blank line after `---` and around multi-line messages.** The
    book's own files were not in fmt's form (13 of 15 would change).
    Changes the decision in 05 §5 (recorded there). **B9.** The question
    as it was put: fmt keeps some blank lines, or the book adopts fmt's
    form.

14. **What the book gains** — **answered (owner, 2026-09-27): all four.**
    A command-line chapter; the server's language options; native details
    (catalog files outside the executable, the matching rules, a Ratatui
    draw loop); and the small completions. **B11.**

## Part A — tasks (A0 first; A1–A3 in any order; A4 after A1; A5 after A2; A6 and A7 after A4; Part B after A7; A8 last)

| Task | Deliverable | Done when |
|---|---|---|
| **A0** The specification text out of the tree | *Owner question 1; 01 §1.* `cargo xtask spec-sync` vendors `test/` and `LICENSE` as now and fetches `spec/` **at the pinned commit** into a git-ignored cache (`target/xtask-cache/`, as `w3c-message-resource` is), verified against digests the `PIN` records; `SPEC_DIR` becomes the cache (one line, as its comment promises); `spec/` is removed from the tree. Every consumer of `SPEC_DIR` that runs without the cache (the ABNF-driven generators, the coverage matrix, the data-model schema check) fails with a message naming `cargo xtask spec-sync`, never skips. CI's jobs run `spec-sync` first (a PIN-named upstream, through the xtask — inside the boundary). Nothing vendored or generated **quotes** the text: `coverage.toml`'s `says` fields and `COVERAGE.md` are our paraphrases, checked once and recorded. | `spec/` absent from the tree and from `git ls-files`; `cargo xtask ci` green from a clean clone after `spec-sync`; without the cache, the first consumer fails naming the command; negative control: a digest altered in the `PIN` is refused |
| **A1** Names and package metadata | *D12.* The 16 names re-checked on crates.io the day of the release (A7 checks again). Each published crate: `description`, `readme` (a short README per crate, pointing to `docs/`), `keywords`, `categories`, `rust-version` (A3), `repository` / `homepage` **only when the owner names a public remote** (none exists; left out, not invented). `license`: `MIT`, and for a crate that ships data derived from CLDR (`mf2-locale-data`'s `data/`) `MIT AND Unicode-3.0` with Unicode's licence text in the package. `[workspace.package] version = "1.0.0"`; `publish` on for the 16, off for everything else (conformance, bench, examples, tools, xtask, fuzz). Every dependency between our crates is an exact requirement (`=1.0.0`), since the generated module, the macro and the runtime share `#[doc(hidden)]` items that A3's policy exempts from semver. | `cargo metadata` shows exactly the 16 publishable, each with the fields above; a test in `xtask` holds that list, the version and the exact inter-crate requirements; negative control: a library crate with `publish` off, or a caret requirement between two of ours, fails it |
| **A2** The API review for 1.0 | *Owner question 3.* Every public item of the 16 crates, reviewed against what applications and the generated code need, before it is promised. For each crate: the public API listed (`cargo public-api`, from crates.io, on the nightly rustdoc JSON — rustup) and committed under `crates/<name>/api.txt`; items only the macro or the generated module use are `#[doc(hidden)]` and named in A3's exemption; enums and structs that may grow are `#[non_exhaustive]`; error types follow the conventions (`thiserror`, `src/error.rs`, `#[from]`); `#![warn(missing_docs)]`, then none missing. **Fixed here:** `DateTimeValue::with_zone` converts an instant (a value that is a wall time says so in its constructor's name), with a test that `instant(t).with_zone("Europe/Paris")` under `timeZone=input` shows Paris time; `mf2 check` warns on an option a function does not define (a new lint, with a seeded-drift case), and every existing corpus is re-checked. Each change the review makes is listed in the task record with why. A change to a user-visible behaviour, or a removal an application would notice, goes to the owner. | `api.txt` committed for every crate and diffed by `cargo xtask ci` (a change to it must be committed with it); no missing docs; the two fixes with their tests; negative control: an item made public without updating `api.txt` fails CI |
| **A3** The version policy, and the MSRV | *Owner question 3; 04 §10.* `docs/versioning.md`, written before A7 depends on it: 1.x's promise — the public API of the 16 crates, the `tr!` forms, the `mf2` CLI's commands and flags, the resource format as `mf2 fmt` writes it; **not** promised — `#[doc(hidden)]` items, the `.mf2b` catalog format and the manifest (master plan non-goal: a server and client from one build agree through `manifest_hash`), CLI report wording, and exact figures. Leptos lines: 0.9 the default and 0.8 an opt-in in 1.x; a Leptos 0.9 pre-release or its release taken in a patch; a new line (0.10) added as an opt-in feature in a minor; **changing the default line or dropping one is 2.0** (04 §10's rule). The W3C Message Resource draft: a change upstream is followed with `mf2 fmt` able to migrate, and a change that would reject a file 1.0 accepted is 2.0. **MSRV** measured, not assumed: the oldest stable Rust that builds the 16 crates for native and `wasm32-unknown-unknown` with their feature sets (edition 2024 needs 1.85; Leptos 0.9.0-beta states 1.88), written as `rust-version`, and a CI job on that toolchain. Raising it is a minor, said in the changelog. | the document written and linked from `docs/README.md` and every crate README; `rust-version` set; the MSRV job green, and the version below it measured failing; `cargo-semver-checks` (crates.io) wired into `cargo xtask release` against the published version from the second release on — *moved to A7 (A3's record): the command it wires into is A7's* |
| **A4** Packages that build from crates.io alone | `cargo package --workspace` (and `cargo publish --workspace --dry-run`, which verifies each package against the others in dependency order) green for the 16. Each package's file list audited and committed as `crates/<name>/package.txt`: no `third_party/` text, no `plans/`, no file outside the crate; sizes recorded (crates.io's 10 MB limit; `mf2-locale-data`'s `data/` is 7.3 MB raw). **`mf2-cli`'s tests that `include_str!` `plans/05-tooling.md`** move to a place that is not packaged (an `xtask` test, or `exclude`d from the package), keeping the check. A package's own `cargo test` from its unpacked `.crate` passes, or the test that cannot is excluded from the package and still run in the workspace. | the dry run green; `package.txt` committed for each and diffed by CI; negative control: a file from `third_party/message-format-wg/spec` added to a package's `include` fails the audit |
| **A5** docs.rs | `[package.metadata.docs.rs]` in each crate, with a feature set that compiles (the Leptos modes `ssr` / `hydrate` / `csr` and the lines `leptos-0-9` / `leptos-0-8` are exclusive: the docs show `ssr` on 0.9, and say which items the other modes add); `#[cfg_attr(docsrs, doc(cfg(…)))]` on feature-gated items. `cargo xtask docs-rs` builds every crate as docs.rs does (nightly, `--cfg docsrs`, its target list) with `rustdoc::broken_intra_doc_links` denied. Each crate's front page says what it is for and links the user guide. | `cargo xtask docs-rs` green in CI; every crate's front page non-empty; negative control: a broken intra-doc link fails it |
| **A6** The changelog and release notes | `CHANGELOG.md` (one file for the workspace, since every crate is versioned together): 1.0.0 — what is in it (full MF2 at `5c4ddb27`, the delivery modes, Fluent migration, XLIFF 2, the reader's time zone, budgets as measured), and **known limitations**: Leptos 0.9 is a beta; WebKit and a screen reader were not run; a date inside an island is not asserted in a browser; the W3C Message Resource format is a draft. The root `README.md` gains install lines for 1.0. | written; `cargo xtask release` refuses a version with no changelog entry |
| **A7** `cargo xtask release` | *Owner question 4.* One command, `cargo xtask release [--publish]`. Without `--publish` (and in CI, on every change): `cargo xtask ci` green, the names still free or ours, version and changelog agree, A1's metadata test, `cargo-semver-checks` (crates.io) against the published version from the second release on (from A3), A2's API listings unchanged or committed, A4's package audit, the docs.rs build, the MSRV build, `cargo publish --workspace --dry-run`, and the tree clean. With `--publish`, **run by the owner only**: the same checks, then `cargo publish --workspace` with cargo's own stored login — the command stores and reads no token of its own, and refuses in CI (a `CI` environment variable). It prints the tag to create; it never pushes. | the dry run green in CI (`.forgejo/workflows/ci.yml`); each check's refusal shown once (a negative control per check: a dirty tree, a missing changelog entry, a taken name simulated, an unaudited file; `cargo-semver-checks` shown skipping on 1.0.0, which has nothing published to compare with, and refusing a simulated break against a local baseline); `--publish` under `CI=true` refuses; the owner has what is needed to publish |
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

## A1 — names and package metadata: what was built

* **Names** re-checked on 2026-09-25 (crates.io's API, one request each):
  all 16 still 404. A7 checks again on the day.
* **`[workspace.package] version = "1.0.0"`.** `publish` stays `false`
  there, so every tool, bench, example and conformance crate keeps
  inheriting it; the 16 say `publish = true` in their own manifests.
* **Each of the 16:** `readme = "README.md"` — a new, short README per
  crate: what it is for, whether an application names it (only `mf2`,
  `mf2-build`, `mf2-cli`, `leptos-mf2` and `mf2-axum` are named; the
  others are reached through `mf2`, and say how), the docs.rs link, and
  the user guide's place (`docs/` in the repository; no URL, since there
  is no public remote). `keywords` (≤ 5) and `categories` (≤ 5, each
  checked against crates.io's list of slugs on the day). `documentation`
  = the crate's docs.rs page, which is where docs.rs will put it (not an
  invented remote); it also quiets `cargo package`'s "no documentation,
  homepage or repository" warning. `repository` / `homepage`: left out.
  The descriptions were already there and are unchanged.
* **Licences:** `MIT` for 15; `MIT AND Unicode-3.0` for `mf2-locale-data`,
  the only crate that ships CLDR-derived data (`data/`; the rest name CLDR
  in comments or read the catalog's entries at run time). Every crate has
  `LICENSE`, a symlink to the root's, and `mf2-locale-data` also
  `LICENSE-UNICODE`, a symlink to `third_party/cldr-json/LICENSE` — one
  source each, packaged as files (A4's audit: these two symlinks are the
  intended exceptions to "no file outside the crate").
* **Every dependency between two of ours is `=1.0.0`**, set once on the
  `[workspace.dependencies]` entry (the 16 now carry `path` and
  `version`; the unpublished ones, `path` only).
* **`mf2 init` and the docs name `"1"`, not `"0.1"`:** the version the
  generated translation crate and the guide's manifests ask for
  (`crates/mf2-cli/src/init.rs`; `getting-started`, `delivery-modes`,
  `accessibility`). `cargo xtask docs` replaces our versions with paths,
  so it is unaffected.
* **The test**, `xtask/src/packages.rs` (test-only until A7's `release`
  runs it): over `cargo metadata --no-deps`, exactly the 16 publishable;
  each at 1.0.0 with a description, a README that exists, the licence
  above and its files, 1–5 keywords valid for crates.io, 1–5
  categories; every normal and build dependency between two of ours
  `=1.0.0`. Five negative-control tests on edited metadata (a library
  crate unpublished; a tool published; a caret requirement; a path-only
  normal dependency; a missing README, a wrong licence, no keywords, an
  invalid keyword and a wrong version together), and
  two shown on the real tree (`publish = false` in `mf2-runtime`; its
  workspace requirement made `1.0.0`): each refused naming the crate.
* **Found for A4 — dev-dependency cycles.** A dev-dependency between two
  of ours may also be **path-only**, and the test allows it: cargo strips
  such a dev-dependency from the package, and it is the only form in which
  one that closes a cycle can be published. `cargo package` on the 16
  stopped at `mf2-fn-datetime`: its dev-dependency on `mf2` (which depends
  on it) cannot resolve, since `mf2` is packaged after it. The same holds
  for `mf2-fn-number` and `leptos-mf2`, each a dev-dependency on `mf2`.
  Making those path-only means their tests that use `mf2` do not run from
  the unpacked `.crate` — A4 decides each, as its criterion says.
* **Not here:** `rust-version` is A3's (measured, then set; the test gains
  the field then). No public remote exists, so no `repository`.

## A3 — the version policy and the MSRV: what was built

* **MSRV measured: Rust 1.88.** Over the lock's resolve, the highest
  `rust-version` any normal or build dependency of the 16 declares is
  1.88 (Leptos 0.9.0-beta and 0.8, `tachys`, `server_fn`, `config`,
  `either_of`, `encoding_rs`, …). On 1.88 the 16 check in five feature
  sets (below). On 1.87 cargo refuses up front, naming those packages;
  with `--ignore-rust-version`, our own crates fail too (`let` chains in
  `mf2-catalog`, `<[T]>::as_chunks`, both stable from 1.88) — so 1.88 is
  where both the dependencies and our code put it, and lowering it would
  need both changed.
* **`rust-version = "1.88"`** in `[workspace.package]`; the 16 inherit it
  (`rust-version.workspace = true`), nothing else does. A1's metadata test
  gains the field: every one of the 16 states it, equal to `mf2`'s
  (negative controls: one missing, one at 1.85). With `rust-version` set,
  resolver 3 prefers dependency versions that declare ≤ 1.88; the lock
  already met that, so nothing moved.
* **`cargo xtask msrv`** (`xtask/src/msrv.rs`): reads the MSRV from
  `cargo metadata`, installs that toolchain with rustup (with
  `wasm32-unknown-unknown`) and runs `cargo check` of the library targets
  into `target/msrv/<toolchain>`: (1) natively on 0.9, all 16 with every
  server feature at once (`ssr`, both function crates, `datetime-icu`,
  `compile`, `icu-blob`, `serde`, `extract`, `decode`, `fixed-decimal`,
  `static-locale`, `mark-fallback-lang`); (2) natively on 0.8,
  `leptos-mf2` and `mf2-axum`; (3) wasm32 `hydrate` with ICU4X dates;
  (4) wasm32 `csr` with `intl` and `Intl` dates; (5) wasm32 `hydrate` on
  0.8. Library targets only: tests and dev-dependencies may use a newer
  Rust, as an application never builds them. `--below` is the negative
  control: the release before must fail step 1.
* **Shown** (2026-09-25): `cargo xtask msrv` green on 1.88 (all five);
  `cargo xtask msrv --below` sees 1.87 refused and passes. CI: a new job
  `msrv` in `ci.yml` runs both.
* **`docs/versioning.md`**, linked from `docs/README.md` (and listed
  among `cargo xtask docs`'s sample-free pages) and from every crate
  README: released together at one version; what 1.x promises (the public
  API, the `tr!` forms, the CLI's commands and flags, the resource format
  as `mf2 fmt` writes it) and what it does not (`#[doc(hidden)]` items,
  `.mf2b` and the manifest — rebuild server and client together — report
  wording, figures); the Leptos lines as a table (a 0.9 pre-release or
  release a patch, a new line an opt-in in a minor, a changed default or
  dropped line 2.0); the Message Resource draft (followed with `mf2 fmt`
  able to migrate; a rejection of a 1.0 file waits for 2.0); MSRV 1.88,
  raised only in a minor, said in the changelog. It states only what holds
  now: the API listings (A2) and `cargo-semver-checks` (A7) are not named
  there until they exist.
* **Moved to A7:** `cargo-semver-checks` in `cargo xtask release` — A3's
  criterion wired it into a command A7 builds. Its row and criterion now
  carry it.
* **Recorded:** while the first 1.88 run was building, the whole `target/`
  directory was removed by something outside this task (the build failed
  on missing files; the spec cache went with it). The run was repeated
  and passed; `spec-sync` refills the cache.

## A2 — the API review for 1.0: what was built

* **The two fixes** (committed first): `with_zone` converts an instant,
  and `DateTimeValue::wall_time` names a zoned wall time (the date
  functions move a value with an offset to its zone's offset; test in
  `crates/mf2/tests/call_site.rs`, negative control shown); the
  `unknown-option` lint (`mf2_build::OPTIONS`, held against the formatter
  by `conformance/tests/options_lint.rs`; every corpus re-checked — none
  fires; the WG suite's two are its deliberate `foo` on `:offset`).
* **`cargo xtask api [--check]`** (`xtask/src/api.rs`) writes, or
  compares with, `crates/<name>/api.txt` for the 16. A library's listing
  is `cargo public-api -ss`'s — no blanket or auto-trait impls; derived
  impls stay, since removing a derive breaks a caller — made in-process
  with the `public-api` 0.52.2 and `rustdoc-json` 0.9.10 crates (the
  library `cargo-public-api` is built on; crates.io) from the rustdoc
  JSON of a pinned nightly, `nightly-2026-09-24` (format 61; installed
  through rustup when missing). Hidden items are not listed. Feature
  sets, matching `cargo xtask msrv`'s: `mf2` `compile,fn-number,
  datetime-icu,host-std,ssr,static-locale,mark-fallback-lang` plus
  `leptos-mf2/leptos-0-9` (the facade alone with `ssr` names no Leptos
  line; an application also depends on `leptos-mf2`, whose default names
  it); `leptos-mf2` `ssr,fn-datetime,static-locale,mark-fallback-lang`;
  `mf2-host-web` `intl,datetime-intl` on `wasm32-unknown-unknown`; each
  build-side crate with its features. `mf2-cli` is a binary: its listing
  is the command tree as clap declares it — every command, each argument,
  its value and the values it accepts — written and held by its own test
  (`listing::api_txt`, which `cargo test` runs too). `cargo xtask ci`
  runs `api --check` last; a difference names its lines.
* **Measured:** 8,374 lines before the review (`cargo public-api -s`, 15
  libraries) → 3,512 after (16 listings, derived impls included):
  `mf2-runtime` 840, `mf2-model` 614, `leptos-mf2` 565, `mf2-build` 374,
  `mf2-syntax` 353, … `mf2-resource` and `mf2-macros` 3 each (the crate
  alone). Missing docs: 137 → 5 once the internals were hidden (rustc
  does not ask for docs on hidden items), and those 5 written; every
  library crate now says `#![warn(missing_docs)]`, which CI's clippy
  denies.
* **Hidden (owner question 5)**, each named in `docs/versioning.md`'s
  exemption: what `tr!` and the generated module expand to (`tr`,
  `tr_args0`–`tr_args4`, `tr_args_n`, `tr_rich`, `tr_dyn`, `markup`,
  `ArgValue::str_static`, both proc-macros) and a `MsgId`'s bits
  (`new`, `from_raw`, `raw`, `chunk`, `index`, `INDEX_BITS`);
  `mf2-build`'s pipeline modules, `Layout`, the loader's types,
  `Outcome::catalogs` / `manifest` / `catalog()`, `Error::io`;
  `mf2-catalog` but for `Catalog` (`new`, `locale`, `dir`,
  `manifest_hash`, `message_count`, `lookup`, `cldr_version`, its bytes),
  `CldrVersion` and the error types; the runtime's catalog access
  (`FnContext::catalog`, `Formatter::simple_ref`, `StrRef`,
  `Sink::push_catalog_text`, `plural_category`) and the function crates'
  switches (`INTL_NUMBERS`, `Number::format_by_host`); the manifest
  (`Manifest`, `Compiled::manifest`); `mf2-locale-data` but for its
  errors, `CLDR_VERSION` and `direction`; `mf2_fn_datetime::icu::prime`
  (it names ICU4X's provider types) and `literal_options`;
  `ErrorKind::suite_name`; in `leptos-mf2` the glue and its view states,
  the sealed `Description` / `Stored`, the serving table
  (`CatalogEntry`, `catalog_entries`, `catalog_file`, `catalog_name`,
  `install_catalogs`), `links`, and the test hooks `live_nodes`,
  `installed`, `installed_twice`.
* **Decided here, within owner question 5:** `mf2-resource`'s API is
  hidden whole — the answer named neither it nor the resource model; it
  mirrors a draft that 1.x follows, no application names it, and our
  tools build its structs field by field, so promising it would make a
  draft change a 2.0; what 1.x promises is the format as `mf2 fmt`
  writes it (its README says so). **Kept promised**, though our crates
  are its main users: all of `mf2-syntax` (the stand-alone parser: CST,
  detail codes, serializer, analysis); `mf2-fn-datetime`'s `Backend`,
  `Plan`, `Neutral`, the `icu` variants and `DateTimeFunction`'s
  constructors (its docs offer the narrower ICU4X variants as an
  application's size choice); `Registry::with_numbers` / `with_dates`
  with `fn_number::NUMBERS` and `fn_datetime::DATES` (how a hand-built
  registry, as in the facade's example, gets unannotated values); the
  server calls of `leptos-mf2` outside the serving table
  (`provide_locale`, `RequestI18n`, `catalog`, …).
* **Found — a hidden module hides its re-exports too.** rustdoc strips
  every item inside a `#[doc(hidden)]` module, even when the crate root
  re-exports it, and `#[doc(inline)]` does not bring it back: the first
  listing showed `pub use mf2_build::Build` with nothing behind it, and
  docs.rs would have shown no `Build`, `Config` or `Report`, nor
  `mf2-locale-data`'s `Error`. So the modules that hold promised types
  (`mf2-build`'s `build`, `config`, `features`, `lint`, `report`;
  `mf2-locale-data`'s `error`) are private, and the few internals other
  crates read get hidden root re-exports (`mf2_build::{Layout,
  CONFIG_FILE, BUILTINS, OPTIONS, CATALOG_FEATURES, defines_option}`);
  `mf2-cli`'s and the options-lint test's paths changed with them.
* **`#[non_exhaustive]` added:** the data model's `Message`,
  `Declaration`, `Key`, `OptionValue` (as `Expression` and `PatternPart`
  already were: the spec's stability policy lets a later MF2 define new
  structures) and `ErrorClass`; `mf2_model::Diagnostic` (with
  `Diagnostic::new`); `mf2-syntax`'s `Error`, `NameRole`, `Analysis`,
  `Name`; the runtime's `Part`, `FallbackSource`, `BidiStrategy` and the
  option values (`CurrencyDisplay`, `UnitDisplay`, `SignDisplay`,
  `Grouping`, `RoundingMode`, `RoundingPriority`, `DateFields`,
  `DateLength`, `TimePrecision`, `ZoneOption`, `ZoneStyle`),
  `OptionValue`, `DateTimeRequest` (with `DateTimeRequest::new`);
  `mf2-build`'s `Config`, `CatalogConfig`, `LocaleDataConfig`, `DataSet`,
  `Missing`, `Strip`, `Emit`, `LocaleInfo`, `Report`, `Diagnostic`;
  `mf2-locale-data`'s errors; `mf2::Compiled`, `CompileError`;
  `mf2_axum::Negotiated`. `ErrorKind::ALL` became a slice (an array's
  length is part of its type). **Kept exhaustive:** the data model's
  structs (they mirror `message.json`, tools build them by literal, and
  a new structure arrives as a variant), `MarkupKind`, `Dir`, `Span`,
  `MeasureUnit` (exactly the two measure functions), `Category`,
  `Isolation`, `Sign`, `Level`, `TextUse`, `CldrVersion`, and
  `mf2_axum::CookieLocale` (the guide builds it with `..default()`).
  `Config` is built from `Default` and then assigned (three call sites,
  none in the guide).
* **What that costs inside our crates:** matches on those enums from
  another of our crates need a `_` arm, which the compiler no longer
  checks — a variant added later must be followed by hand. Each arm does
  the loud thing where it can: the serializer's `UnknownNode`, the
  catalog writer's unsupported message, `UnsupportedOperation` from the
  ICU4X date backend and the zone moves, and the `Intl` date host
  declines (`format_date_time` returns `false`); where it cannot, the
  option's default (the `Intl` number host, the neutral stub,
  `Grouping`). Also patched: the conformance harness, `l4-runner`, the
  fuzz targets, the B12 walk harnesses and the `intl` probe (`cargo
  check` of each; B12's harnesses one at a time, as `check.sh` builds
  them — their `--workspace` unifies features that cannot meet). The
  arms added to client-path code match variants that do not exist, so
  they compile to nothing; B12's job measures it.
* **Error conventions:** every public error type is `thiserror` in
  `src/error.rs` with `#[from]` conversions; `mf2-syntax`'s `Error`,
  `mf2-locale-data`'s two (documented variant by variant) and
  `mf2::CompileError` became `#[non_exhaustive]`, the others were.
* **Also fixed:** an ```` ```ignore ```` block in `mf2-axum` that was not
  Rust (a rustdoc warning); `mf2-resource`'s front page linked
  `https://example.invalid`; crate front pages that linked items now
  hidden say what 1.x promises instead (`mf2-build`, `mf2-catalog`,
  `mf2-locale-data`, `mf2-resource`, `mf2-macros`).
* **Shown** (2026-09-25): negative controls — `pub fn
  a2_negative_control()` added to `mf2-host-std` makes `cargo xtask api
  --check` fail naming `+ pub fn mf2_host_std::a2_negative_control()`; a
  flag `--a2-control` added to `mf2 fmt` fails `listing::api_txt` naming
  it. `cargo xtask ci` green with `api --check` as its last step, and
  `cargo xtask docs` green (every sample of the guide compiled for the
  server and for `wasm32-unknown-unknown`).
* **No behaviour an application sees changed,** and nothing it names was
  removed: the guide's samples name only promised items. The one
  run-time difference is unreachable today (an option value the runtime
  does not have).
* **Left for A5:** three links to feature-gated items do not resolve in
  the listings' feature sets (`leptos-mf2`'s `set_document_lang` and
  `hydrate_islands`, `mf2-fn-datetime`'s `Intl`) — docs.rs's feature set
  and `doc(cfg)` are A5's.

## A4 — packages that build from crates.io alone: what was built

* **The dev-dependency cycles** A1 found are broken the one way cargo
  allows: in `mf2-fn-number`, `mf2-fn-datetime` and `leptos-mf2` the
  dev-dependency on `mf2` (which depends on each) is path-only, written in
  the crate's manifest (a path, no version — the one departure from
  "declared once", commented there). Cargo strips it from the package.
* **What each package leaves out, and why** — each in its manifest's
  `exclude`, commented; every one still runs in the workspace:
  * the integration tests of those three crates, all of which use `mf2`;
  * `mf2-build`'s tests that build corpora with `workload-gen` (not
    published; a path-only dev-dependency) — all but `tests/config.rs`;
  * `mf2-catalog`'s `manifest`, `roundtrip`, `skew` (they read
    `bench/corpora/`), `mf2-locale-data`'s `table` (it regenerates `data/`
    from `third_party/cldr-json`);
  * `mf2-cli`'s integration tests (`workload-gen`, and `xliff.rs` reads
    `third_party/xliff` and `plans/`), and two unit-test files:
    `src/workspace_tests.rs` — new: the checks that held report codes and
    rules against `plans/05-tooling.md` (moved from `convert.rs`,
    `exchange/xliff.rs` and `leptos_fluent/tests.rs`) and `stats`'s spec
    commit against the `PIN` (which silently passed when the file was
    missing; now it is not compiled at all where it cannot run) — and
    `src/convert/leptos_fluent/tests.rs` (`workload-gen` and its
    templates). A new `build.rs` sets `cfg(mf2_workspace)` when
    `src/workspace_tests.rs` is present, and both modules compile only
    then. `tests/fluent/` stays in the package: `convert.rs`'s own tests
    read it;
  * `package.txt` itself, in all 16.
* **`mf2-fn-datetime`'s front-page example** formatted a date through
  `mf2`; in the package it could not compile. It now shows the handler and
  registry with `mf2-runtime` (a dependency), and the complete example —
  unchanged — moved to the `mf2` facade's front page, under `fn-datetime`.
* **`cargo xtask package [--check] [--test]`** (`xtask/src/package.rs`):
  `cargo package --no-verify` of the publishable crates (from `cargo
  metadata`; A1's test holds them to the 16), then each `.crate` read back
  and audited: every file from the crate's own directory, the two licence
  symlinks the only exceptions and each pointing where A1 put it; no file
  a copy (SHA-256) of anything under `third_party/`, `plans/` or
  `target/xtask-cache/` (the specification text), `LICENSE-UNICODE` the
  one expected copy; the `.crate` under crates.io's 10 MB. The file list
  is `crates/<name>/package.txt`, written, or with `--check` compared
  naming the lines. `cargo xtask ci` runs `package --check` last (3.6 s).
  `--test` unpacks every `.crate` into `target/package-test/`, joins them
  in a workspace of their own with `[patch.crates-io]` standing in for the
  registry, and runs `cargo test --no-fail-fast --workspace` with the
  server feature set (`cargo xtask msrv`'s first step, now the shared
  `msrv::SERVER_FEATURES`) — their own tests against a fresh resolve, as
  after publishing. It is A7's to run in `release`.
* **Sizes** (2026-09-25, `cargo xtask package`; printed, not committed —
  they move with every edit): the largest `.crate` is `mf2-locale-data`,
  1,377,249 B (7,838,847 B unpacked, of which `data/` 7.3 MB), 14 % of the
  limit; the next is `mf2-cli`, 88 KB; the rest 6–79 KB.
* **Shown** (2026-09-25): `package --check --test` green — the 16 audited,
  their lists unchanged, their own tests passing from the `.crate` files
  (the first run failed on `mf2-fn-datetime`'s front-page example, above);
  the moved `mf2-cli` tests run in the workspace (4 in
  `workspace_tests`). Negative controls: the spec's `syntax.md` copied
  into `mf2-model`, and a symlink to its `errors.md` added to
  `mf2-syntax`, refused — "a copy of target/xtask-cache/…/spec/syntax.md",
  "outside the crate"; an unlisted `notes.md` in `mf2-host-std` refused
  as `+ notes.md` in its `package.txt`. `cargo publish --workspace
  --dry-run` green: exactly the 16 packaged, each built against the
  others in dependency order, the unpublished members skipped. `cargo
  xtask ci` green with `package --check` as its last step.

## A5 — docs.rs: what was built

* **`[package.metadata.docs.rs]` in the 15 library crates** (`mf2-cli`
  has no library; docs.rs documents none, and `cargo xtask docs-rs`
  refuses a table there). Each lists its `targets` — `x86_64-unknown-
  linux-gnu`, and for `mf2-host-web` only `wasm32-unknown-unknown` as
  `default-target` — so docs.rs builds one target, not its default five.
  The features are A2's listing sets, so the table is now **the one
  source for both**: `cargo xtask api` reads it (its own feature list is
  gone) and the published documentation shows what `api.txt` promises.
  One set grew: `mf2-fn-datetime` is documented with both backends
  (`datetime-icu,datetime-intl`; they build together), which resolves
  A2's broken link to `Intl` and adds `Intl` — the `datetime-intl`
  backend the facade's feature table already offers — to its `api.txt`
  (+19 lines, the struct, its derives and its `Backend` impl).
  `leptos-mf2` and `mf2` are documented as `ssr` on Leptos 0.9 (`mf2`
  with `leptos-mf2/leptos-0-9`, since the facade alone names no line).
* **`cargo xtask docs-rs`** (`xtask/src/docs_rs.rs`): for each crate and
  target, `cargo rustdoc --lib` on the nightly `cargo xtask api` pins
  (`nightly-2026-09-24`; docs.rs uses its latest nightly — pinned here so
  a run repeats), with the table's features, `--cfg docsrs` given to
  rustdoc and `DOCS_RS=1` set, into `target/docs-rs`. Stricter than
  docs.rs, which only warns: `-D warnings` (every rustdoc lint that warns
  by default — `broken_intra_doc_links`, `private_intra_doc_links`,
  `invalid_markdown_table`, …) and `-D rustdoc::missing_crate_level_docs`
  (a front page must exist); then each front page must say where the
  user guide is. A table key it does not reproduce (`rustc-args`,
  `cargo-args`, …), a missing table, no `targets`, or a `default-target`
  outside them is refused, not ignored. It runs every crate and names
  all that fail. CI: a new job `docs-rs` in `ci.yml`.
* **Feature labels:** every library says
  `#![cfg_attr(docsrs, feature(doc_cfg))]`. On the pinned nightly that
  labels each feature-gated item by itself (the old `doc_auto_cfg` is
  part of `doc_cfg` now; probed first), so explicit
  `#[cfg_attr(docsrs, doc(cfg(…)))]` is only where the automatic label
  is wrong: the Leptos layer is gated on the internal `leptos` feature,
  and its items now say "`csr` or `hydrate` or `ssr`" (10 in
  `leptos-mf2`, 2 in `mf2`), the server's re-exports "`ssr`" (not
  "`leptos` and `ssr`"), the client's "`hydrate` or `csr`"; the
  stand-in `islands_gate!` without `hydrate` carries no label
  (`doc(auto_cfg = false)`) instead of "non-`hydrate`".
* **What the build found, fixed:** A2's two links in `leptos-mf2`
  (`set_document_lang`, `hydrate_islands` — client items absent from the
  `ssr` docs; now plain names with the mode); `mf2-build`'s link to the
  private `INDEX_FILE` (now `index.json`); `mf2-resource`'s two links to
  its hidden `parse` and `serialize`; `mf2-syntax`'s code table, whose
  row 3 held an unescaped `|` — rustdoc drops what follows it, so docs.rs
  would have shown that row cut short.
* **Front pages**, all 15, rewritten for a docs.rs reader: the
  references to `plans/` documents, section numbers, decision, budget and
  phase codes are gone (the content stays, in words); each ends with
  **"The user guide"** — what it covers and that it is the repository's
  `docs/` directory (no URL: there is no public remote, as A1's READMEs
  say) — and how an application reaches the crate (through `mf2`, and
  as which re-export and feature). `leptos-mf2`'s says which mode and
  line the docs show and lists, mode by mode, the items `hydrate` and
  `csr` add and the server items they lack; `mf2`'s points there. Also
  corrected: `leptos-mf2`'s claim that an application names only `mf2`
  (the guide has it name `leptos-mf2` too), and `mf2-model`'s "frozen by
  the Phase 1 work order", now what the data model's shape promises.
* **Shown** (2026-09-25): `cargo xtask docs-rs` green — 15 crates, no
  warnings; before the front pages were rewritten it refused all 15 as
  not naming the user guide. Negative control: `` [`NoSuchItem`] `` added
  to `mf2-host-std`'s front page is refused ("unresolved link to
  `NoSuchItem`", then the crate named). The table's refusals are unit
  tests (`docs_rs::tests`). `cargo xtask api` rewrote only
  `mf2-fn-datetime`'s listing (above).
* **Left:** item docs (not front pages) still cite `plans/` documents and
  section numbers in about 190 places, which docs.rs readers see but
  cannot follow; the criterion asked for the front pages. Nightly cargo
  warns of four unused `[workspace.dependencies]` entries (`leptos_meta`,
  `leptos_router`, `mf2-axum`, `mf2-cli`) — cargo's lint, not rustdoc's,
  and not a failure.

## A6 — the changelog and release notes: what was built

* **`CHANGELOG.md`** at the root, one file for the 16 (they are released
  at one version), newest first. The 1.0.0 entry: what is in it — MF2 at
  `5c4ddb27` with the suite at every layer and the coverage matrix, `tr!`,
  catalogs outside the wasm, the four delivery modes, Leptos 0.9 default
  and 0.8 opt-in, `mf2-axum`, the reader's time zone, CLDR numbers and the
  two date backends, the `mf2` command, the Fluent converters, XLIFF 2,
  accessibility, MSRV 1.88; **budgets as measured**, each with its
  command (B1, B5 and the whole app re-measured for this entry, below;
  the others their last recorded figure or CI's gate), and the
  `leptos-fluent` A/B's headline figures from its snapshot; **known
  limitations** — Leptos 0.9 a beta, the Message Resource format a draft,
  Chromium and Firefox only (no WebKit), no screen reader, a date in an
  island not asserted in a browser, and the dev push of an edited catalog
  not built (owner, 2026-09-24).
* **Entry headings carry no date:** `## 1.0.0`. The owner publishes when
  the owner chooses, and the tag `cargo xtask release` prints records the
  day; a dated heading would have to be edited in the publish step, which
  A7's clean-tree check forbids.
* **The check** (`xtask/src/changelog.rs`, test-only until A7's
  `release` runs it, as A1's `packages.rs` is): the workspace version has
  exactly one `## <version>` entry, it is the newest (first), and it is
  not empty. `the_version_has_its_entry` holds the tree; four negative
  controls — no entry, an empty one, one below a newer entry, one
  repeated — each refused with its line. Shown on the real tree
  (2026-09-25): the heading renamed `## 0.9.0` fails the test with
  "CHANGELOG.md: no `## 1.0.0` entry". (Raising the workspace version
  instead cannot be shown this way: the `=1.0.0` requirements between our
  crates stop cargo resolving first.)
* **The root README:** the status now says 1.0.0 is being prepared and
  is not published; an **Install** section — the browser target, `cargo
  install mf2-cli`, `mf2 init`, `cargo add` of the translation crate,
  `leptos-mf2@1` and `mf2-axum@1` (optional, for `ssr`), the 0.8 flags —
  with the note to use paths until 1.0.0 is on crates.io; the
  documentation list gains the migration and versions pages and the
  changelog. Its figures (B1 22,102 B, 12.6 B a site, 45,517 B) were
  Phase 7's; updated to this measurement.
* **Measured for the entry** (2026-09-25, this tree): `cargo xtask size`
  — B1 25,875 B gz, B5 8.4 B gz a site, 41,466 B gz at 1,860 sites (Phase
  8's exit gate read 25,835 / 8.4 / 41,506; B1 is fitted from two builds
  and moves a little between tree states); `cargo xtask catalog-size` —
  B7 `en` 18,072 B br (unchanged since Phase 2). B10 is Phase 3's
  recorded figure, dated as such in the entry. No budget moved.
* **Left for A7:** once the owner has published, three sentences stop
  being true — the README's status and its "until 1.0.0 is on crates.io"
  note, and `docs/getting-started.md`'s "Not published yet" box. `release
  --publish` should print them among its next steps.

## A7 — `cargo xtask release`: what was built

* **`cargo xtask release [--publish] [--allow-dirty] [--baseline-rev
  REV]`** (`xtask/src/release.rs`). In order: (1) **at once**, so one run
  names every cheap problem — the tree clean (`git status --porcelain`,
  untracked files included), the version's changelog entry (A6's check,
  no longer test-only), A1's metadata check (likewise), and the 16 names
  on crates.io; (2) `package --check` (seconds, so an unaudited file stops
  the run before the long steps); (3) `cargo-semver-checks`; (4) `cargo
  xtask ci` (A2's `api --check` among its steps); (5) `package --check
  --test` (A4: the packages' own tests from their `.crate` files); (6)
  `docs-rs`; (7) `msrv`; (8) `cargo publish --workspace --dry-run`; (9) the
  tree as it was at the start. `--publish` then runs `cargo publish
  --workspace` with cargo's own login, and prints the tag to create and
  A6's three sentences to update; it never tags or pushes. It refuses
  when `CI` is set, at all, before anything else; clap refuses it beside
  `--allow-dirty` or `--baseline-rev`. `--allow-dirty` (a dry run only)
  reports a dirty tree and carries on, to try a change before committing
  it; `--baseline-rev` compares the API with a git revision instead of
  crates.io.
* **The names:** crates.io's API, one request per name a second apart
  (its crawler policy), and one for the owners of a taken name; `curl`,
  as `xliff-sync` fetches; a user agent that names the command and no
  person. The rule needs no stored login: on the **first release** (no
  entry below the version's in `CHANGELOG.md`) every name must be free;
  from the second, `mf2` must carry the previous entry's version, and
  "ours" is `mf2`'s owners — every other name free (a crate new in the
  release) or owned by one of them, and no crate may already carry the
  version.
* **`cargo-semver-checks` 0.50.0**, pinned, installed by cargo into
  `target/tools` only when a crate is compared (so not on 1.0.0). Each
  library crate that has a published version is compared with it, with
  the features its `[package.metadata.docs.rs]` presents (the table is
  now the source for `api`, `docs-rs` and this) — `--default-features`
  plus the list, or `--only-explicit-features` when the table turns the
  defaults off; left to itself the tool guesses a feature set. Three
  departures, each found on the first run and each in the code:
  * `mf2` is compared **without its Leptos layer** (`ssr`,
    `static-locale`, `mark-fallback-lang`, `leptos-mf2/leptos-0-9`): the
    tool builds a placeholder crate that depends on the one checked, which
    cannot pass a dependency's feature ("not allowed to contain slashes"),
    and without one no Leptos line is named. The layer is `leptos-mf2`'s,
    re-exported, and compared there; the re-exports are held by `mf2`'s
    `api.txt`.
  * `mf2-host-web` is compared **on the host**, not wasm32: the tool runs
    rustc with `--cap-lints=allow`, which on Rust 1.98 silences the
    "unsupported crate type" warning cargo's wasm32 probe relies on
    ("output of --print=file-names missing"; reproduced with plain cargo
    and `RUSTFLAGS=--cap-lints=allow`). Its host and wasm32 API listings
    were identical (51 lines, 2026-09-25), and the command refuses the
    fallback for a crate whose `src/` gates code on the target (a unit
    test holds `mf2-host-web` to that, with `mf2-runtime` as the control).
  * `mf2-macros` is skipped with its reason: the tool reads a library's
    rustdoc JSON, which a proc-macro crate has none of; its promise is the
    macros' names (`api.txt`) and forms (the `tr!` tests).
* **CI:** a new job `release` in `ci.yml` runs `cargo xtask release`
  after `spec-sync --check` (git-ignored, so the tree stays clean). It
  repeats the `ci`, `msrv` and `docs-rs` jobs' work, so that one command
  says a commit can be released. It holds no token. No remote exists yet,
  so the job has not run on a runner.
* **`docs/versioning.md`** now names `cargo-semver-checks`: from the
  second release on, a release that breaks the one before is refused.
* **Shown** (2026-09-25), each refusal once: a **dirty tree** (the
  uncommitted files listed); a **missing changelog entry** (the heading
  renamed `## 0.9.0`: "CHANGELOG.md: no `## 1.0.0` entry"); an
  **unaudited file** (`notes.md` in `mf2-host-std`: "+ notes.md" in its
  `package.txt`); **`--publish` under `CI=true`** refused before any check;
  a **taken name**, simulated in unit tests (taken on the first release;
  owned by someone else later; `mf2` without the previous release; a
  version already out), since no name of the 16 is taken — the real run
  found all 16 free; **`cargo-semver-checks`** skipping on 1.0.0 (nothing
  published) and, with `--baseline-rev HEAD` and `StdHost`'s `Copy` derive
  removed, refusing `mf2-host-std` alone (`derive_trait_impl_removed`,
  "semver requires new major version") while the other 13 libraries
  passed.
* **The dry run green** (2026-09-25): first with `--allow-dirty` on the
  uncommitted change (490 s), then on the clean tree after the commit —
  every step, the names all free, `cargo-semver-checks` skipped, `cargo
  publish --workspace --dry-run` packaging and verifying the 16, and the
  tree unchanged (304 s; both with warm build caches, taken under the
  machine's usual load).
* **The owner's first publish stopped part way** (2026-09-26, 03:14 UTC):
  five crates went up at 1.0.0 (`mf2-model`, `mf2-resource`,
  `mf2-syntax`, `mf2-catalog`, `mf2-macros`), then crates.io refused the
  sixth, `mf2-runtime` — "429 Too Many Requests: You have published too
  many new crates in a short period of time. Please try again after …
  03:15:30 GMT". `cargo publish --workspace` stops there, and run again,
  this command would have refused the five as taken. The five are what
  the tree packages (each `.crate` byte for byte, and a repackaging
  reproduces them). Changed:
  * the names are checked **after** the package audit, which leaves each
    `.crate` in `target/package`. A crate already at this version is
    **released** when the published `.crate` (from `static.crates.io`,
    into `target/release-check`) ships the same files as this tree's —
    compared by one digest over each path and its bytes, leaving out the
    two files cargo generates from the tree's state: `.cargo_vcs_info.json`
    (the commit packaged from) and `Cargo.lock` (it records our sibling
    crates' package checksums, which move with that commit — found on the
    first try, when four of the five differed there and nowhere else — and
    third-party versions resolved that day; a library's lock file is not
    used by its dependents). The sources and the normalized `Cargo.toml`
    are compared. Otherwise it is
    refused: a published version cannot be replaced (negative control in
    the unit tests);
  * the dry run and the publish run `cargo publish --workspace` with
    `--exclude` for each released crate; with all 16 out, neither runs;
  * on a 429 the publish reads the time crates.io names (tested on the
    owner's message), waits until 5 s past it, asks crates.io again which
    crates are out, and publishes the rest — up to 40 times, then stops
    saying to run it again. Any other failure stops it; run again, it
    takes up where the publish left off;
  * `cargo-semver-checks`' baseline is the newest version published
    before this one (`--baseline-version`): a crate out at this version is
    not its own baseline;
  * the four `[workspace.dependencies]` entries nothing inherits
    (`leptos_meta`, `leptos_router`, `mf2-axum`, `mf2-cli`) are removed:
    the pinned nightly cargo warned about each once per crate in `api
    --check`, 64 warnings that hid the real error in the owner's output
    (A5 had left them as harmless).
  The wait loop itself cannot be run without publishing; its parts are
  unit-tested, and the dry run was shown resuming: the five recognised,
  the other 11 verified against them from crates.io.
* **The owner's command,** when the owner chooses to publish: `cargo
  login` once (cargo stores the token), then from a clean checkout of the
  commit to release, `cargo xtask spec-sync --check && cargo xtask release
  --publish`.

## Part B — the book, verified (added 2026-09-27; B1 first; B2–B9 in any order; B10 and B11 after the code they describe; B12 with each task; B13 last, then A8)

On 2026-09-27 the user guide (`docs/`) was reviewed, then every claim it
makes about behaviour was **run**, not read. The method:

* **Sample apps.** The book's own sample applications, as `cargo xtask docs`
  assembles them under `target/docs/projects`, were copied and built with
  `cargo leptos` (dev) and `trunk`.
* **What drove them.** Playwright (Chromium) scripts, curl, native runs,
  and negative-control variants: a server whose fallback lacks the
  context, and a second build whose catalogs stand in for "another deploy".
* **Repo tooling run as-is.** `cargo xtask docs`, `api --check`,
  `msrv` / `msrv --below`, `islands-zero` and `churn`.

About 130 claims were tested; most hold. The findings below are observed
behaviour. Each names what was run.

### Refuted (observed the opposite)

| Book | Observed | Task |
|---|---|---|
| switching.md: a catalog from another deploy during a switch makes the page reload | Hydrated `hello`; the switch-time fetch of `/i18n/en*` routed to another build's catalog. No navigation. The page stays `fr`; console `mf2: the locale could not be switched; the page is unchanged`. (At boot the same test reloads, as documented.) | B2 |
| delivery-modes.md: a lazy route behaves like every other part of the page | `lazy` sample built with `--split`. `/visits` loaded directly: the button counts 1→4. Reached via the `<A href>`: it renders and follows a switch, but its `on:click` does nothing, and a `{move \|\| count.get()}` stays at its first value. No console output. The same app with mf2 stripped and `leptos::mount::hydrate_lazy` works both ways. Not the `fn-datetime` boot branch (built without it, it still fails). | B1 |
| delivery-modes.md: without `--split` a lazy route "works the same" | Built without `--split`: `hello.js` imports `__wasm_split_placeholder__`, the browser cannot resolve it, and nothing hydrates on any page. A Leptos property (`#[lazy_route]` + `leptos/lazy`), already recorded in 16 (the fluent A/B). | B10 |
| delivery-modes.md: without `static-locale`, islands follow a live switch | Islands sample rebuilt without `static-locale`. The switcher (not an island) still submits `GET /?lang=fr`, and the island's counter resets. | B10 |
| migrating: the unchanged server honours the initializer's cookie | `Cookie: lang=fr` → `content-language: en`. Only `mf2_locale` is read. | B8, B10 |
| switching.md: the path-prefix negotiator, with the switcher | `/en/page?lang=fr` → en; `/en/page` + `mf2_locale=fr` → en. `CookieLocale::default()` is also `Secure` on plain HTTP. | B4 |
| native-apps.md: a catalog from another build "is an error, not wrong text" | `Emit::NativeFiles`. Rebuilding with only a text change keeps the manifest hash; the old file renamed to the new name loads and prints the old text. Only a message-set change is refused (`ManifestMismatch`). | B7 |
| call-sites.md: `msg_id!` + `TrDyn` for tools | No public constructor (`TrDyn::new` → E0599). The only one is hidden `tr_dyn`. | B5 |
| call-sites.md: a description can be any attribute's value (and `class` is a plain one) | `class=tr!(…)` → `TrArgs: IntoClass` not satisfied. `style=` is the same. | B10 |
| getting-started.md: `mf2 check` runs every check the build runs | Bare `check` on `hello`: 2 `neutral-numbers` warnings the build never gives. On date messages: `gated-function` errors where the build succeeds. | B8 |
| delivery-modes.md: islands-zero figures | Re-run: code 165,714 B / 865 functions both builds; shipped 85,644 → 85,648 B gz; data +8 B (not byte-identical). The book had 185,925 / 1,019 / 94,904 from before Leptos 0.9. | B10 |

### Partly right

| Claim | What was seen | Where it is fixed |
|---|---|---|
| The `static-locale` row of the switch table | On islands, the outcome is right, but the mechanism is the form's `GET` plus the server's `Set-Cookie`. No client code runs. | B10 |
| A description inside `<textarea>` "stays as it is" | Under `mark-fallback-lang` it gets `<span lang="en">`, shown as text. | B6 |
| "Each placeholder" is isolated | Only arguments that need it are. Strings get FSI/PDI; numbers get nothing. | B10 |
| Ratatui: "a name with no style is plain text" | It inherits the enclosing style. | B10 |
| migrating: `move \|\| tr!(…)` → `tr!(…)` | Only with constant arguments. Otherwise the closure is kept, plus `.to_string()`. | B10 |
| A second conversion run changes nothing | True, but a second `--write` exits 1 ("already exists"), and a dry run reports phantom files. | B8 |
| `mf2 check` warns about every missing translation | It gives one summary per locale and names no ids. | B8 |
| `<CatalogLinks/>` lists the other languages | It lists all of them. | B10 |
| leptos-0-8 plus the default line gives a compile error | The error is there, followed by 8 unrelated tachys errors. | B8 |
| The book's `.mf2` files are in `mf2 fmt`'s form | `fmt --check` fails on 13 of 15. | B9 |

### Tasks

| Task | Deliverable | Done when |
|---|---|---|
| **B1** Lazy route handlers after client navigation | **Test first.** `examples/demo-ssr`'s `LazyPage` gains an `RwSignal`, a `<button id="lazy-add">`, a `{move \|\| n.get()}` and a `tr!` with `count = n`. `tools/e2e/checks/lazy.mjs` clicks it after a direct load and after client navigation. **Then bisect:** a non-reactive `tr!` against the per-node `RenderEffect` (`registry.rs`), then the rich path, then the boot's `hydrate_from_async` against Leptos's `hydrate_lazy` (`boot.rs`). **Fix it here**, or, if the cause is Leptos's, work around it in `boot.rs` and draft an upstream report for the owner to file. | the new check fails before the fix and passes after; `lazy`, `demo` and `a11y` checks green |
| **B2** A switch reloads on deploy skew | *Owner question 10.* `reload_into` (`boot.rs`, now `static-locale`-only) is available to the live build. `set_locale` on `ManifestMismatch` writes the choice (cookie, or `localStorage` for `csr`), strips `?lang=`, and reloads. The changelog says so. | an e2e check routes the switch-time catalog to another build's (spec below) and sees a reload into the new language; before the fix it fails |
| **B3** Cookie only on an explicit choice | *Owner question 11.* `CookieLocale::store` returns early unless `negotiated.from` is `"query"` or `"path"`. Changelog entry (the expiry no longer slides). | unit tests per source (query/path write; cookie/accept-language/default do not); `demo` and `a11y` checks green |
| **B4** The switcher on path-prefix sites | *Owner question 8.* `LocaleSwitcher` gains an optional `href_of: fn(&str) -> String` (the `AlternateLinks` shape). Each `LocaleOption` carries `data-mf2-href`, and the submit handler navigates there. For the no-wasm case, mf2-axum redirects a `?lang=` that disagrees with the path to that language's URL; the API is decided in the task, additive, and goes in `api.txt`. | unit tests for the redirect; an e2e run on a path-prefix variant, with wasm and with wasm blocked, lands on `/fr/…`; `api --check` updated |
| **B5** `TrDyn::new` | *Owner question 7.* `TrDyn::new<N: Into<Text>, V: Into<ArgValue>>(id: MsgId, args: impl IntoIterator<Item = (N, V)>)`, wrapping `tr_dyn`, which stays hidden. `api.txt` updated. | a compiled ssr-only sample in `call-sites.md`; a test formats a known and an unknown name |
| **B6** No fallback span in text-only elements | Under `mark-fallback-lang`, no `<span lang>` when the parent is `textarea`, `title`, `option`, `script` or `style`. Server: `html_marked` checks the tag it just wrote (first child). Client: `Wrapper::fit` / mount. | a server HTML test; `csr` e2e fallback assertions green |
| **B7** Native catalogs checked by content | `content_hash` (SHA-256, 8 bytes; `mf2-build/src/catalog.rs`) moves to `mf2-catalog` behind a `content-hash` feature, shared by `mf2-build` and `mf2-native`. `from_directory` compares each file with the hash in its name, and a mismatch is a new `NativeError` variant. The web client is unchanged: its catalogs come from its own server or build. | test: a renamed catalog from a text-only rebuild is refused; `api --check`, `msrv` and `msrv --below` green |
| **B8** CLI | *Owner question 12.* **`check`:** resolves the features through `cargo::resolved_features` (with `--offline`); without cargo it prints a note and checks with none. **Missing translations:** the lint names the first missing ids. **`convert`:** unchanged text is not a rewrite, and an output identical to the existing file is not an error; `cookie_name:` is parsed, and the initializer's report says to add `CookieLocale { name, ..Default::default() }` **as an extra source**, since the client always writes `mf2_locale`. **leptos-0-8:** the five `leptos-0-8` cfgs in `glue/view.rs` become `all(leptos-0-8, not(leptos-0-9))`. | `mf2-cli` tests for each; a bare `mf2 -C i18n check` on the book's `hello` equals its build's report; a second `convert --write` on the book's `migrate` exits as the first did and writes nothing; the misconfiguration shows only the `compile_error!` |
| **B9** `mf2 fmt` blank lines | *Owner question 13.* `serialize_with` writes a blank line after `---`, and around an entry that starts on its own line. `bench/workload-gen` matches. 05 §5 says why. `cargo xtask docs` runs `mf2 fmt --check` on every page's `mf2` blocks. | `fmt --check` green on the corpora and the book; the docs gate refuses a seeded non-canonical block |
| **B10** Book corrections | Every row above not fixed by code, plus each code change's wording. The lists: **(a)** this work order's review notes (below) and **(b)** the tables above. | `cargo xtask docs` green; `mdbook build` clean |
| **B11** Book additions | *Owner question 14.* **`docs/command-line.md`:** every `mf2` command and its main flags; `export` / `import` (JSON, XLIFF 2); `convert --from fluent` and its report codes. Its commands are `run=` blocks, and its outputs `generated` blocks. **`switching.md`:** the server options (`Negotiator::default()`, `over`, `QueryParam("…")`, `CookieLocale` fields and `secure` on localhost, `default_locale`, `negotiated()`, options from `locales()`, custom sources and sinks). **`native-apps.md`:** `NativeFiles` output and shipping, the matching rules, `formatter()`, and a Ratatui draw loop in the compiled example. **Small completions:** `TimeZone::named`, Trunk and Rust 1.88 in the prerequisites, `cargo install mf2-cli`, and the "not published yet" note current. SUMMARY and README tables updated. | `cargo xtask docs` green with the new page |
| **B12** Guards | Each fix above carries its test (named in its row). In addition, the `migrate` page's report block becomes `generated`, so `cargo xtask docs` compares the report text, not only the exit status. | as each row |
| **B13** Re-verification | This Part's method run again on the corrected book, with the verdicts recorded in `phase-9-results.md`: sample apps built and driven, curl, native runs, `islands-zero`, `churn`, `msrv` and `msrv --below`, `api --check`. | every refuted row now verified; `cargo xtask ci` green |

**Review notes for B10** (from the review; each is a sentence to correct):

* **`README.md` (docs).** Title "Rust MF2". The table matches SUMMARY (the ecosystem and native pages). "For the targets it runs on", not "server and browser". Native apps are in scope.
* **`ecosystem.md`.**
  * The diagram: `mf2-build` produces the generated module and the `.mf2b` files, not the runtime.
  * `mf2` re-exports `leptos-mf2`'s call-site types.
  * `mf2-syntax` handles messages, not resources.
  * `mf2-cli` links crates.io.
* **`getting-started.md`.**
  * `strip` removes ids, attributes and comments. `[locale_data] "used"` leaves out unused currency and unit data.
  * `0.9.0-beta` matches later `0.9.0-*` pre-releases and the 0.9 releases.
  * Catalogs are compressed once and cached.
  * `1 234,5` is written with U+202F.
* **`call-sites.md`.**
  * `starts` needs its own section: it is `post.starts` today.
  * Text is copied once into a shared string; only an `Arc<str>` is shared as it is.
  * A markup message with no handlers renders its text.
  * `TimeZone::named` returns an `Option`.
* **`switching.md`.**
  * `Vary` gains `Cookie` because of `mf2_tz`.
  * A client-only application finds a catalog through `index.json`, with no `/i18n/<tag>` fallback.
* **`delivery-modes.md`.**
  * `mount_to_body` loads the index before it chooses.
  * The gate costs no request, but a few bytes.
* **`accessibility.md`.**
  * Focus is kept, but a markup fragment is rebuilt on a switch.
  * Repository paths become links.
* **`native-apps.md`.**
  * The title matches SUMMARY.
  * The isolation characters are U+2066–U+2069.
* **`migrating-from-leptos-fluent.md`.**
  * A real app also gets `leptos-fluent-dependency`.
  * leptos-fluent 0.3 needs Leptos 0.8: point to the opt-in.
  * The `.ftl` directory falls back to `locales/`.
* **`versioning.md`.**
  * An application names `mf2-native` / `mf2-ratatui`, and installs rather than depends on `mf2-cli`.
  * Semver checks skip `mf2-macros`.
  * "1.0 is published" is stated as it stands.

**Probe specs** (the verification's scripts lived in a scratch directory; this is what they did):

* **Switch skew (B2).** Run the app, plus a second build whose manifest differs (one added message). Hydrate the first app (wait until a counter click changes its text). Route `**/i18n/<target>*` to the second server's catalog bytes (`GET <other>/i18n/<target>`, following the redirect). Set `window.__marker`, press the switcher, then count main-frame navigations and check the marker. At boot, route the preload's catalog the same way: expect one navigation and `mf2: this page's catalog is from another deploy; reloading.`
* **Deploy check at start-up (already green).** `mf2_axum::install` with one catalog swapped for another build's returns `Err(ManifestMismatch)`.
* **Missing context (already green).** The fallback registered with `file_and_error_handler(shell)`: a 404 for `Accept-Language: fr` renders `lang="en"` with no `Content-Language`, and no log.
* **Wasm unchanged by a translation edit (already green).** `cmp` of `pkg/hello.wasm` before and after editing `fr/main.mf2` and rebuilding, and under `cargo leptos watch` (about 40 s to serve the new text).
* **Lazy click (B1).** See B1: click after a direct load, and after `<A href>` navigation. A control copy with mf2 removed from the client works.
* **Environment.** `trunk build` needs a `wasm-bindgen` CLI matching the lockfile. Servers run as the built binary with `LEPTOS_SITE_ADDR`, `LEPTOS_SITE_ROOT`, `LEPTOS_SITE_PKG_DIR`, `LEPTOS_OUTPUT_NAME` and `LEPTOS_ENV`, set one per variable (zsh does not word-split a variable holding several).

## B1 — lazy route handlers after client navigation: what was built

* **Test first.** `examples/demo-ssr`'s `LazyPage` gained a counter: an
  `RwSignal`, `<button id="lazy-add">`, `<output id="lazy-count">{move ||
  n.get()}</output>` and `tr!("lazy.presses", count = presses)` (two new
  messages in each locale, `lazy.press` and `lazy.presses`).
  `tools/e2e/checks/lazy.mjs` presses it twice after the link navigation
  and twice after a direct load; each time the handler must count and the
  message must follow. Before the fix (2026-09-27, debug `--split` build,
  Chromium): 35/37 — both `navigated-*` assertions failed (count 0, text
  unchanged after two presses), both `direct-*` passed, console silent.
* **Bisected** on the lazy view alone (a scratch probe: navigate, press
  twice, read the count), each a `cargo leptos build --split`:
  no `tr!` at all — works after navigation; plus a text `tr!` — works;
  plus the markup message — works; plus `tr!` with a **signal argument** —
  fails. The boot (`hydrate_from_async` against `hydrate_lazy`) was not
  involved, as the review's run without `fn-datetime` had suggested.
* **The cause is ours.** A description's first format — in `build`,
  `hydrate`, and `registry::replace` (a rebuild) — ran inside whatever
  observer was current, so a signal argument subscribed the enclosing
  render as well as the node's own argument effect. A lazy route reached
  by a link is built inside the router's view, which re-runs on what it
  tracked. **Observed** with a temporary log line in `LazyPage::view`
  (Chromium, debug `--split`): before the fix, after navigation the view
  ran once, then once more per press (1 → 2 → 3), each run a new signal
  at 0, rebuilt in place — the button and heading were the same DOM
  elements after a press (tagged before it); loaded directly it ran once
  and did not re-run. After the fix it runs once either way and the count
  reaches 2. Why a hydrated route does not re-run was not traced. No
  upstream report.
* **Fixed** in `leptos-mf2`: the view glue's formats (`glue/view.rs`'s
  `with_text`, and a new `with_marked_text` for `mark-fallback-lang`) and
  the registry's writes outside the effect (`replace`, and the zone
  correction in `insert_hydrated`) run under `reactive_graph`'s
  `untrack`. The argument effect still subscribes: it sets itself as the
  observer when it runs. The `to_string()` path keeps tracking, since a
  closure that builds a `String` must re-run.
* **Shown** (2026-09-27, Chromium and Firefox, debug `--split` build):
  `lazy` 74/74 (the four new assertions pass in both), `demo` 170/170,
  `a11y` 720/720 (with `demo-islands` rebuilt on 3704 and `demo-csr`'s
  `trunk build`), `csr` 88/88, `islands` 58/58. The changelog's 1.1.0
  entry says what was fixed.
* **The chunk** (release `--split`, `gzip -9`, 2026-09-27): the old
  `LazyPage` is 12,277 B (6,188 gz) with and without the fix — identical;
  the demo README's 11,255 B had drifted before this change. With the
  counter it is 23,688 B (11,405 gz), now in the README. `cargo xtask ci`
  green with the fix, its size gates included.
* **Read, not run — markup messages.** A markup message (`TrRich`) has no
  argument effect of its own: its fragment is formatted in the enclosing
  render, left tracked here, since inside a `move || …` closure that is
  the only way it follows a signal. So a markup message with a signal
  argument follows it only when something around it re-runs, and in a lazy
  route reached by a link it would re-run the route as above. The book
  promises a following signal for a message, not for markup; B10 (or a
  task of its own) should decide which it says, after running it.

## B2 — a switch reloads on deploy skew: what was built

* **Test first.** `tools/e2e/checks/demo.mjs` gained `switchSkew`: load
  `/?lang=en`, hydrate, mark the window, then switch to French while
  `**/i18n/fr*` is answered **once** with this build's French catalog, its
  `manifest_hash` (header bytes 8–15) altered by one byte. That stands in
  for the probe spec's second build: the hash is the first thing the reader
  checks and all that tells two builds' catalogs apart, and it costs no
  second build. After that one answer the route passes through, as a new
  deploy serves its own wasm and catalogs together. Six assertions: the
  skewed catalog was served, the page reloads into `fr` (marker gone),
  exactly one main-frame navigation, `mf2_locale=fr`, no `?lang=` in the
  address, one `mf2:` line naming another deploy. Before the fix
  (2026-09-27, debug `--split` build, Chromium): 86/91 — the five after the
  first failed as the review saw it: no navigation, the page `en`, the
  cookie `en`, `?lang=en` kept, console `mf2: the locale could not be
  switched; the page is unchanged`.
* **Fixed** in `leptos-mf2`'s `boot.rs`: the live `set_locale` matches
  `switch_live`'s result, and on `ManifestMismatch` logs `mf2: the catalog
  is from another deploy; reloading into the new locale.` and returns
  `reload_into(tag)`, which is no longer `static-locale`-only (both
  variants: the cookie, `?lang=` removed and a navigation; `csr`:
  `localStorage` and a reload). `Ok` is returned with the navigation under
  way, so `LocaleSwitcher` logs nothing more. No public API change (`api
  --check` unaffected). Clippy clean for wasm with `hydrate`, `csr`,
  `hydrate,static-locale`, `csr,static-locale` and `hydrate,fn-datetime`.
* **Also `csr`.** `tools/e2e/checks/csr.mjs` gained the same case against
  `examples/demo-csr` (`**/i18n/fr.*`), asserting `localStorage` instead of
  the cookie.
* **Shown** (2026-09-27, Chromium and Firefox): `demo` 182/182, `csr`
  98/98, `lazy` 74/74. **Seen, left:** Chromium keeps a bare `?` in the
  address after `?lang=` is removed (`location.set_search("")`; Firefox
  drops it) — the same function the `static-locale` switch has always
  used; harmless to the server.
* **Words.** The changelog's 1.1.0 entry; `docs/switching.md`'s "What a
  switch does" (step 2 and the sentence after the list); the e2e README's
  `csr` row.

## B3 — the cookie only on an explicit choice: what was built

* **`CookieLocale::store`** (`crates/mf2-axum/src/negotiate.rs`) returns
  `None` unless `negotiated.from` is `"query"` or `"path"`. So a guess
  (`Accept-Language`, the default) is not remembered, and a cookie that was
  read is not written back — its `Max-Age` no longer slides. The type's and
  `Negotiator::default()`'s docs say so: the default lists no explicit
  source, so its sink writes nothing, and the client writes the cookie on a
  switch.
* **Tests:** `the_cookie_is_written_for_an_explicit_choice` (query, path)
  and `the_cookie_is_not_written_for_a_guess_or_a_cookie_already_there`
  (cookie, accept-language, default). Negative control: with the guard
  disabled, the second fails.
* **e2e:** `demo.mjs`'s `sink-writes-the-cookie` (which asserted the old
  behaviour on an `Accept-Language` request) became
  `sink-leaves-a-guess-unwritten`, and two were added:
  `sink-writes-an-explicit-choice` (`?lang=ar`) and
  `sink-leaves-a-read-cookie-alone`.
* **Shown** (2026-09-27, debug `--split` build, Chromium and Firefox):
  `demo` 186/186, `lazy` 74/74. The changelog's 1.1.0 entry says what
  changed. No public API change.

## B5 — `TrDyn::new`: what was built

* **`TrDyn::new<N: Into<Text>, V: Into<ArgValue>>(id, impl IntoIterator<Item
  = (N, V)>)`** in `crates/leptos-mf2/src/dynamic.rs`, over the hidden
  `tr_dyn`, which stays hidden. `api.txt` gains the one line (`mf2`
  re-exports the type, and its listing does not list inherent methods of a
  re-export, so only `leptos-mf2`'s moved).
* **Test:** `crates/mf2/tests/call_site.rs`'s
  `a_run_time_message_is_built_by_its_public_constructor` — a known name
  formats; an unknown one (`nom`, with an integer value) is ignored and the
  variable shows its fallback.
* **The book:** `docs/call-sites.md`'s "When the id is only known at run
  time" names `TrDyn::new` and gains a sample under
  `#[cfg(feature = "ssr")]` (the `calls` project is checked for `ssr` and
  `hydrate`, so the sample compiles for the server only, as the page says
  it should be used): a server function picking `msg_id!("file.saved")` or
  `msg_id!("signature.text")` from data and formatting it with
  `to_plain_string()`.

## B6 — no fallback span in text-only elements: what was built

* **Server:** `html_marked` (`glue/view.rs`) writes a borrowed message
  unmarked when it is the first child of `textarea`, `title`, `option`,
  `script` or `style` — the tag read back from what was just written
  (`lang::open_tag`: the buffer ends with `>`, the name after the last
  `<`). Only the first child is checked, as the row says; a borrowed
  message after another text in such an element would still be wrapped
  (not seen in any sample).
* **Client:** `Wrapper::fit` adds no span around a text mounted in such an
  element, and `TrState::mount` (through `Wrapper::mount_into`) removes the
  span a text built unmounted was fitted with when its parent is one. A
  switch then keeps it unmarked.
* **Tests:** `tests/fallback_lang.rs`'s `a_text_only_element_gets_no_span`
  (a `<textarea>`, an `<option>` and a `<title>` unmarked; a `<p>` beside
  them still marked) and `lang`'s unit test of `open_tag`. Negative
  control: with the check disabled, the first fails. Clippy clean for
  wasm with `hydrate` and `csr` under `mark-fallback-lang`.
* **Not asserted in a browser:** no example puts a borrowed message in a
  text-only element, so the client half is read, not run; the existing
  fallback assertions (`demo`, `csr`) cover the unchanged paths.

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
- [x] the 16 names verified and every package's metadata complete (A1; `rust-version` comes with A3's measurement)
- [x] every public item reviewed, listed and documented for 1.0.0; `with_zone` fixed; the unknown-option lint (A2)
- [x] the version policy written, the MSRV measured and held in CI (A3)
- [x] the packages audited and verified from crates.io's point of view (A4)
- [x] the documentation builds as docs.rs builds it (A5)
- [x] the changelog with 1.0.0 and its known limitations (A6)
- [x] `cargo xtask release` green as a dry run in CI; the publish is the owner's (A7) — green locally on the clean tree; the `release` job is in `ci.yml`, waiting for a runner like every job (no remote yet)
- [ ] Part B: the book's refuted claims fixed in code or text, each with its guard (B1–B12)
- [ ] Part B: the book re-verified by running it (B13)
- [ ] `cargo xtask ci` green; the conformance harness green at `current_phase = "P9"`
- [ ] `plans/phase-9-results.md` written (A8)
