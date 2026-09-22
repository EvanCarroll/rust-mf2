# 12 — Phase 5a work order: build pipeline and CLI

Part of the [master plan](00-master-plan.md) (§9, P5a). RFC 2119 keywords
apply. Written at the close of Phase 4 from [phase-4-results](phase-4-results.md)
and the Phase 0 build-orchestration probe (P0.9). The source format is
specified in [05-tooling](05-tooling.md) §2, the manifest and `mf2.toml` in
§3, orchestration in §4, lints in §5, the CLI in §6; the catalog writer and
the LOCALE slicing rule in [02-catalog-format](02-catalog-format.md) §2–§4;
this document orders the work and sets the exit criteria.

Phase 5a turns a directory of translated message resources into what an
application ships: the manifest, one catalog per locale (flattened, stripped,
precompressed, with its sliced locale data), and a generated Rust module —
from `build.rs` through `mf2-build`, and from the command line through
`mf2-cli`. It has no conformance layer of its own: L5 (the suite through
`tr!`) is Phase 5b's, and it rests on this phase's pipeline being exact. Its
exit is the master plan's: the reference workload builds reproducibly,
`check` catches seeded drift of every lint class, and editing one message
rebuilds only what it must.

## State at the start

| In the tree | Where |
|---|---|
| Parser, data model, validation, serializer (L1/L2 green, D1 settled) | `crates/mf2-syntax`, `crates/mf2-model` |
| Catalog format v1: `writer::catalog` (many messages against a `Manifest`), `writer::single`, stripping, the `Manifest` (ids, slots, markup, functions; `hash`, `write` / `read`), the reader and decoder (L3 green) | `crates/mf2-catalog` (features `writer`, `manifest`, `decode`) |
| The runtime, API frozen at P3 with Phase 4's additive §2.7; function crates `mf2-fn-number`, `mf2-fn-datetime`; hosts `mf2-host-std`, `mf2-host-web` (one static per host kind: `HOST`, and the date and `intl` hosts of Phase 4) | `crates/` |
| Locale data for every CLDR locale and the slicing API: `LocaleNeeds`, `NumberNeeds::add_message`, `CurrencyNeeds` / `UnitNeeds` with `Selection::{Listed, All}`, `locale_entries`; `icu.blob` and `DateNeeds` for `datetime-icu` (feature `icu-blob`) | `crates/mf2-locale-data` |
| `mf2::compile_str`: one message → one catalog with its LOCALE entries — the single-message model of everything this phase does for a corpus | `crates/mf2/src/compile.rs` |
| The reference workload generator (1,600 messages × 4 locales, flat JSON and `.mf2` files in the working grammar of 05 §2) and the committed corpora | `bench/workload-gen`, `bench/corpora/` |
| The W3C Message Resource draft **pinned, not vendored** (no license stated; 05 §2) | `third_party/w3c-message-resource/PIN` |
| Phase 0 code to port, **from history only** (`git show b15b6d6:probes/p0-09-build-orchestration/…`): a `build.rs` → manifest + catalogs + generated module, the baked-path `tr!` wrapper, relocation fallback, the edit-scenario scripts (`scenarios.sh`, `relocate-check.sh`, `watch-check.sh`, `ra-check.sh`), the resource reader it used | commit `b15b6d6` |

## What Phase 4 changes here

* **Slicing is corpus-wide and per locale.** `compile_str` computes one
  message's needs; `mf2-build` MUST compute, for each locale's *flattened*
  message set (after fallback, D5 — a translation may use functions and
  literal options its source does not), the union of `NumberNeeds` over its
  messages, the plural kinds its selectors use, and the configured currency
  and unit sets of `mf2.toml` `[locale_data]` (02 §4.4: a non-literal
  `currency=$c` makes the set `"all"`; `check` notes that a list would do).
  The entries are `mf2_locale_data::locale_entries`, plus, when
  `datetime-icu` is on, `icu.blob` from `mf2_locale_data::icu_blob` with a
  `DateNeeds` built the same way (`DateNeeds::add_message` over each locale's
  messages, or `DateNeeds::all()`); `mf2::compile_str` does all of this for
  one message and is the model to lift.
* **Gated functions are a build error, neutral numbers a warning** — and the
  default-features column of the ledger already says which tests those are:
  every L4d cell `degraded` as `unknown-function` MUST be a build rejection
  naming file and line, and `neutral-numbers` a `check` warning (01 §3; L5d
  in Phase 5b asserts exactly that). The facade's feature set is read from
  `CARGO_FEATURE_*` in `build.rs` and from `--features` in `mf2-cli` (05 §3.1).
* **The generated registry and host.** Closed world (B13): `registry()` names
  only the handlers the corpus uses, `with_numbers(&NUMBERS)` only when
  `fn-number` is on and the corpus has a placeholder that can receive a
  number (#90), `with_dates` likewise for dates. `mf2-host-web`'s date and
  `intl` methods sit on their own host statics so that a feature on but unused
  links none of them (B1′); the generated module MUST pick the host the corpus
  needs, and B1′ is measured on the generated code, not only on the `bench/b12`
  harnesses.
* **`icu.blob` is build-side ICU4X.** With `datetime-icu`, `mf2-locale-data`'s
  `icu-blob` feature pulls ICU4X's baked data crates into the *build*
  (`build.rs`) — measured compile time and peak memory belong in this
  phase's results, and the feature MUST stay off unless the application
  enables `datetime-icu`.
* **The `intl` client option** (adopted, D4) formats numbers, plural
  categories and dates in the browser, so a catalog for an `intl` client
  needs no number, currency, unit or plural entries — but the server keeps
  the Rust path and does. Whether one catalog serves both (simplest; the
  client ignores the entries) or the build writes a client variant without
  them is **owner question 2** below; measure the bytes first.
* **Budgets moved.** B3 is ≤ 5.5 KB gz (owner, 2026-09-22). B7 and B8 are
  stated per locale; this phase measures them on the catalogs `mf2-build`
  writes for the reference workload, with the corpus's real LOCALE entries
  (Phase 2 measured B7 without locale data, Phase 4 B8 on single messages).

## Owner questions (answered; the measurements are in the results)

1. **Server-only catalog embedding** (05 §4): every locale change recompiles
   the i18n crate and all its dependents in both cargo-leptos builds (P0.9:
   8–23 s per debug build for 2,000 sites). Evaluate moving server-only
   embedding into a crate only the server binary depends on; report the
   measured rebuild times both ways and recommend.
   → **Answered: adopt the split, as an option the application chooses.**
   `Build::emit(Emit::Module | Emit::Catalogs)` is implemented, and the
   scenarios run both ways: with the catalogs apart, a text edit in any
   locale rewrites **nothing** in the i18n crate's `OUT_DIR`, so neither it
   nor anything above it is recompiled. It costs one more crate and a second
   parse of the corpus per build (+355 ms release, +2.1 s debug on the
   reference workload). [phase-5a-results](phase-5a-results.md), "Owner
   question 1".
2. **One catalog or two for `intl` clients** (above): the measured bytes of
   the number / currency / unit / plural entries per locale on the reference
   workload, against the complexity of a second catalog variant (two content
   hashes per locale, preload headers, the server's choice).
   → **Answered: one catalog.** On the reference workload a client-only
   variant saves −80 to +75 B brotli — three of the four locales are
   *smaller* with the data in them. On a corpus of nothing but numeric
   functions it saves 0.24–0.36 KB per locale. [phase-5a-results](phase-5a-results.md),
   "Owner question 2".
3. Carried: the W3C Message Resource draft's license (05 §2) — until it is
   confirmed, nothing is vendored and `mf2-resource` is written from the
   draft at the pin, citing it. **Still open**: `cargo xtask resource-sync`
   remains blocked, and 05 §2 now records the seven readings of the working
   grammar that A1 settled, each to be re-checked against the draft's ABNF
   when vendoring is allowed.

## Part A — tasks (A1–A3 first; A4–A8 in order; A9–A12 as their inputs exist)

| Task | Deliverable | Done when |
|---|---|---|
| **A1** `mf2-resource` | New crate (`no_std` + `alloc`): parser and serializer for the Message Resource syntax (05 §2's working grammar, which the draft at the pin wins over), its data model generic over the message type (the draft's `Resource<Message>`), spans for every entry, comment and property, the attachment rules; JSON output in the draft's data-model shape. Tests: ABNF-driven generation from our grammar, round trip `parse(serialize(r)) == r`, every example in 05 §2; a fuzz target (never panics, spans in bounds). | the grammar's productions each covered by a generated and a hand-written test; the fuzz target runs clean for 1 h |
| **A2** Loaders | The `Loader` trait (05 §2: `id, source, spans, comments, properties, file`), the `.mf2` loader over A1 and the flat JSON loader (`id → source`), both yielding the same records for the reference workload (`bench/workload-gen` writes both); `@param`, `@do-not-translate` and comments carried to lints and exports. | the two loaders agree on the reference workload; a resource and its JSON export round-trip |
| **A3** `mf2.toml` and features | Configuration (05 §3.1) with `thiserror` errors that name the key; the feature set from `CARGO_FEATURE_*` or `--features`; the locale set from `locales/<tag>/`. | every key of 05 §3.1 parsed and tested, unknown keys an error |
| **A4** `mf2-build`: manifest and catalogs | Parse and validate every locale (errors with file, line, column), the manifest from the source locale (05 §3: ids → `MsgId`, NFC slot order, markup, functions, `manifest_hash`), fallback chains flattened with the per-message origin flag (D5), one catalog per locale through `writer::catalog`, stripped per `[catalog] strip`, `missing` policy; the LOCALE entries per locale as above (A6's slicing); `.br` (brotli 11) and `.gz` beside each catalog, content hashes; every output written only when its bytes change. | the reference workload's four catalogs decode (L3's decoder) to the flattened source models; a differential: for every suite message, the catalog `mf2-build` writes for a one-message corpus formats identically to `compile_str`'s (L4's runner, both configurations) |
| **A5** Generated module | 05 §4: `MANIFEST_HASH`, the locale table (tag, dir), catalog names and hashes only under `ssr`, the closed-world `registry()` with the host the corpus needs (above), `include_bytes!` catalogs only under `ssr`, `pub use ::mf2 as __mf2;`, the `tr!` wrapper `macro_rules!` with the manifest path and hash baked in (its proc-macro target is Phase 5b's; until then the wrapper is compiled and tested against a stub). | the generated module compiles for `wasm32-unknown-unknown` and native in every feature combination of the facade; the client build contains no catalog name, hash or message text (B6 canaries grep) |
| **A6** Slicing on a corpus | `mf2-build` computes each locale's `LocaleNeeds` from its flattened messages and `[locale_data]`; `icu.blob` with `datetime-icu`; an `mf2 stats` breakdown per entry. B8 on the reference workload's catalogs (plural + number symbols ≤ 0.5 KB gz per locale) and on a panel corpus that uses every numeric function. | B8 met on both; the entries equal the union of the per-message entries `compile_str` would write, checked by a test |
| **A7** `mf2 check` | Every lint of 05 §5 with file/line/column, `--format json`, configurable levels from `[lints]`; the gated-function error and `neutral-numbers` warning cross-checked against the ledger's L4d `degraded` cells (each `unknown-function` cell's message rejected when its feature is off). A **seeded-drift corpus** (`conformance/drift/` or under `bench/`): one mutation per lint class over the reference workload. | every lint class caught on its seeded drift and silent on the clean workload |
| **A8** `mf2-cli` | `clap`: `init` (i18n crate, `build.rs`, `mf2.toml`, `watch-additional-files` for cargo-leptos), `check`, `compile` (catalogs without cargo), `fmt` (canonical resources via A1), `stats` (coverage, raw/gz/br sizes, locale-data breakdown, CLDR and spec pins), `dump` (catalog → MF2 source / data-model JSON), `pseudo` (`en-XA`, `ar-XB` as `workload-gen` defines them), `export` / `import` (flat JSON), `watch` (recompile on change; the push to open pages is P7). `convert --from fluent` is P8. | each command has an integration test on the reference workload |
| **A9** Reproducible and incremental | The master plan's exit: the reference workload built twice in different directories gives byte-identical manifests, catalogs and generated modules; P0.9's edit scenarios (translation-only edit, source edit, new id, mtime-only touch, relocated target dir) rewrite exactly the outputs that must change, and a translation-only edit leaves the client wasm byte-identical. Scripts ported from P0.9 into `tools/` or `xtask`. | a CI job runs the scenarios; the results table in `phase-5a-results` |
| **A10** Sizes | B7 (brotli 11, per locale) on the catalogs `mf2-build` writes for the reference workload *with* their LOCALE entries, against Phase 2's figures without them; B1′ and B13 on the generated module (a feature on and unused links nothing; an unused function links nothing). | B7, B8, B1′, B13 met or restated with the owner |
| **A11** Build cost | Cold and warm `build.rs` time and peak memory for the reference workload (`cargo build`, `cargo leptos build`), with and without `datetime-icu`; owner question 1 measured. | figures with their commands in `phase-5a-results` |
| **A12** Fuzzing and generated input | Targets for `mf2-resource` (A1) and for `mf2-build` on generated corpora (l4gen's generator, many messages per corpus): no panic, and every accepted corpus's catalogs decode to its flattened models. | both targets run clean for 1 h on the final code |

Order: A1 → A2 → A3 are independent of each other's details and may overlap;
A4 needs A2 and A3; A5–A7 need A4; A8 wraps A4–A7; A9–A12 grow alongside.

## Exit (master plan §9, P5a)

Checked at the close of the phase; the figures behind each are in
[phase-5a-results](phase-5a-results.md).

- [x] the reference workload builds reproducibly (byte-identical outputs from
      two directories), under plain cargo — **cargo-leptos is not exercised**:
      there is no Leptos application in the tree before P6, and P0.9 measured
      what the double build changes (results §A9)
- [x] `mf2 check` catches seeded drift of every lint class and is silent on
      the clean workload; gated functions are build errors with file and line
      (§A7)
- [x] editing one message rebuilds only what it must; a translation-only edit
      leaves the client wasm byte-identical (§A9)
- [x] the `mf2-build` ≡ `compile_str` differential green on the whole suite in
      both configurations — 485/485 (§A4)
- [x] B7 and B8 measured on `mf2-build`'s output and met; **B1′ and B13 held
      by construction and by test, not yet as a size delta on a wasm harness
      built from the generated module** (§A10); build cost reported (§A11)
- [x] owner questions 1 and 2 answered and recorded, both with measurements
      and a recommendation
- [x] fuzz targets clean for ≥ 1 h each on the final code (§A12)
- [x] `plans/phase-5a-results.md` written

**What Phase 5b inherits**: the `b12-generated` harness pair that would turn
B1′ and B13 from "the generated registry is the same registry Phase 4
measured" into a size delta; and cargo-leptos, whose double build P6 brings
into the tree.
