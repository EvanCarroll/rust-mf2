# 05 — Tooling: syntax, build pipeline, macros' inputs, CLI

Part of the [master plan](00-master-plan.md). RFC 2119 keywords apply.

Everything here runs at build time or on a developer's machine. None of it is
linked into the client wasm.

## 1. `mf2-model` and `mf2-syntax`

`mf2-model` (`no_std` + `alloc`) is the spec's interchange data model as Rust
types — `Message = PatternMessage | SelectMessage`, declarations, variants,
catch-all keys, patterns, expressions, literals, variable refs, function refs,
options, markup, attributes — plus `MsgId`, `Dir`, and the error enums shared by
every crate, and the small **`Frontend` trait** (`parse(&str) -> (Option<Message>,
Diagnostics)`) that both `mf2-syntax` and an `ox_mf2_parser` adapter implement —
it lives here so the D1 gate and its fallback need nothing from `mf2-build`.
With the `serde` feature the model (de)serializes to JSON that validates against
the vendored `spec/data-model/message.json`.

`mf2-syntax` (`no_std` + `alloc`) provides:

* **Parser → lossless CST** with spans and error recovery (diagnostics keep
  going after the first error; editors and `mf2 check` need that).
* **CST → data model lowering**: unescaping and literal cooking. Values are kept
  **as written** — nothing is normalized here.
* **Validation**: the six Data Model Errors, comparing keys and names **under
  NFC** (normalization belongs to comparison and to catalog encoding, never to
  the model, or L3's lossless round trip could not hold).
* **Serializer**: data model → canonical MF2 source (used by the round-trip
  property, the converter, and `mf2 fmt`).
* **Variable analysis**: the external variables, local variables, markup names
  and function names a message uses — the input to the manifest (§3).

### Decision D1 — own parser, behind a measured gate against `ox_mf2_parser`

Facts (planning audit 2026-09-20; suite result and allocations re-confirmed
in-tree by P0.12):

| | `ox_mf2_parser` 0.14.0-alpha.12 | Own `mf2-syntax` |
|---|---|---|
| Suite result | 460/462 on syntax + data-model errors at our pin; the 2 misses are benign over-reporting | gated by conformance L1/L2 — 100 % required |
| Output | flat, CST-linked `SemanticModel` of records and spans — **not** the spec data model; lowering to `PatternMessage`/`SelectMessage` is ours to write either way | the data model directly |
| Stability | alpha line only (stable 0.1.6 lacks validation); must be pinned to an exact alpha; single author, repo three months old, "WIP" | ours |
| Platform | not `no_std`; `memchr` + `unicode-normalization` | `no_std`; `unicode-normalization` (no default features), quick check first |
| License | MIT (crate); no license detected at repo root | — |
| Size of the job | lowering + serializer + variable analysis ≈ 1.5 k lines on top of it | the grammar is a 121-line ABNF with **no recursive productions**; parser + CST ≈ 2 k lines more than the ox route |

**Decided by the owner: write `mf2-syntax` — on the condition that it is at
least as good as what exists.** There is no point building something worse than
`ox_mf2_parser`. "As good" is therefore a measured gate, not an intention:

*Baseline* — `ox_mf2_parser` 0.14.0-alpha.12, native release (`opt-level=3`, fat
LTO), single thread, one message per call, counting allocator; measured in-tree
by P0.12 on the committed corpora with `bench/parser-gate` (median of 31,
2026-09-20, i7-1165G7; details, per-pass limits and IQRs in
`bench/parser-gate/BASELINE.md`, results in
[phase-0-results](phase-0-results.md) §P0.12). *Fresh* = a new `SourceStore`
per message; *reused* = state shared across one pass (CST: ox's session API
with one `ParseWorkspace`; model: a shared `SourceStore`, since ox's model API
needs an owned parse result):

| Input | Stage | fresh ns/msg | allocs/msg | alloc B/msg | reused ns/msg | allocs/msg | alloc B/msg |
|---|---|---:|---:|---:|---:|---:|---:|
| 462 suite messages (14.6 KB) | parse → CST | 1,011 | 13.2 | 2,869 | 508 | 1.1 | 190 |
| | + semantic model + validation | 1,912 | 26.3 | 3,490 | 2,025 | 25.3 | 3,082 |
| `workload-1600.json` (43.2 KB) | parse → CST | 361 | 8.6 | 1,381 | 232 | 1.0 | 167 |
| | + semantic model + validation | 659 | 11.7 | 1,516 | 733 | 10.7 | 1,108 |
| its 1,256 placeholder-free messages (27.4 KB) | parse → CST | 258 | 8.0 | 1,171 | 173 | 1.0 | 158 |
| | + semantic model + validation | 447 | 9.0 | 1,235 | 488 | 8.0 | 827 |

Allocations are exact and match the planning audit byte for byte on the suite
rows; times were taken at a load average ≈ 2.8 and are ≈ 30 % below the
audit's (a different machine — the audit's own probe, re-run beside the
harness, agrees with it). The gate compares ratios within one run, so the
absolute times only orient. Note how much of ox's fresh-state cost is
per-call setup: with reused state its CST rows allocate once per message (the
source copy) — the reused rows are the demanding bar.

*Gate (Phase 1 exit)* — `bench/parser-gate/` (a workspace member; `ox_mf2_parser`
is a dependency of that crate only). Rules of the comparison, so that two
engineers would measure the same thing:

* **Corpora are committed files**, not generated at bench time:
  `bench/corpora/suite.json` (the 462 `src`) and `bench/corpora/workload-1600.json`
  (flat `id → source`, emitted once by `workload-gen` with the default seed), plus
  the placeholder-free subset of the latter (no `{` and not starting with `.`:
  1,256 messages). The planning audit used an inline generator with a slightly
  different shape (≈ 35 B/message, 80.1 % simple); the table above is P0.12's
  re-measurement on the committed corpora and is the baseline.
* **Both parsers run in the same process, same invocation**, interleaved, so the
  result is a ratio and does not depend on the machine. CI runs it; the gate is
  `ours ≤ 1.05 × ox` on the median of ≥ 30 runs for time (5 % is the noise
  allowance), and `ours ≤ ox` exactly for allocation count and bytes.
* **Like for like**: row "CST" = our `parse_cst` vs. their `parse_source`; row
  "+ model" = our `parse_model` (straight to the data model, plus validation) vs.
  their `parse_source` + `build_semantic_model` + `validate_semantics`.
* **Fresh state per message** for both (ox: one `SourceStore` per message). A
  second set of rows reuses state across one pass over a corpus on both sides
  (ox: `parse_source_session` with one shared `ParseWorkspace` for the CST
  rows, a shared `SourceStore` for the model rows; ours: the arena) and must
  also pass.
* "Lean" means allocation count and allocated bytes. Dependency count and
  `no_std` are recorded but are not gate criteria.

`mf2-syntax` MUST, **on every row**:

1. be at least as fast (ns/msg) and allocate no more (count and bytes);
2. be strictly more correct: 462/462 exact on the suite (ox: 460/462);
3. and keep what ox has that matters: lossless CST, error recovery, stable
   diagnostic codes, `forbid(unsafe_code)`.

*Target* — ≥ 3× on the workload rows and **zero allocations for a
placeholder-free message**. That is realistic because the designs differ: ox
builds full token/trivia/node tables for every input; `mf2-syntax` has

* a **simple-message fast path** — one scan for `{`, `}`, `\` and a leading `.`;
  if none, the message is a single borrowed text run (79 % of real messages);
* a byte-oriented, non-recursive, single-pass parser (the grammar has no
  recursive productions) writing into a flat, reusable arena;
* cooked values as `Cow<'src, str>` — borrowed unless an escape is present;
* NFC by quick-check first, normalizing only when needed;
* two entry points over one parser: `parse_model` (what the build uses; no
  trivia kept) and `parse_cst` (tooling: formatter, diagnostics, editors).

*Fallback* — the parser sits behind the `Frontend` trait (in `mf2-model`). If
the gate is not met at Phase 1 exit, `ox_mf2_parser` is adopted behind that
trait and only the lowering (needed either way) is kept. The benchmark stays in
CI so a later regression is visible.

*Result (Phase 1 exit, [phase-1-results](phase-1-results.md) §A9)* — **the gate
holds on every row; no fallback.** Median of 31, load average 1.2,
`bench/parser-gate/GATE-P1.md`:

| Input | Stage | fresh: ox → ours ns/msg | allocs/msg | reused: ox → ours ns/msg | allocs/msg |
|---|---|---:|---:|---:|---:|
| 462 suite messages | parse → CST | 820 → 158 (5.2×) | 13.2 → 1.3 | 415 → 129 (3.2×) | 1.1 → 0.0 |
| | + model + validation | 1,537 → 305 (5.0×) | 26.3 → 2.9 | 1,681 → 284 (5.9×) | 25.3 → 1.9 |
| `workload-1600.json` | parse → CST | 288 → 67 (4.3×) | 8.6 → 1.0 | 199 → 45 (4.4×) | 1.0 → 0.0 |
| | + model + validation | 527 → 86 (6.1×) | 11.7 → 0.5 | 610 → 82 (7.4×) | 10.7 → 0.3 |
| its 1,256 placeholder-free | parse → CST | 199 → 44 (4.5×) | 8.0 → 1.0 | 149 → 26 (5.7×) | 1.0 → 0.0 |
| | + model + validation | 340 → 27 (12.6×) | 9.0 → **0** | 397 → 25 (15.9×) | 8.0 → **0** |

Bytes allocated are below ox on every row; correctness 462/462 against 460/462.
Both targets are met: ≥ 3× on the workload rows (4.3–7.4×), and **zero
allocations for a placeholder-free message** on the model rows. The fuzzing and
generated-input differentials also found that ox hangs on `{:` + a
noncharacter and accepts markup as a `.local` value; it remains a dev-only
oracle ([phase-1-results](phase-1-results.md) §A8).

In both outcomes `ox_mf2_parser` is a **dev-dependency differential oracle** in
`mf2-conformance` (assert *expected ⊆ reported*), where alpha churn cannot break
a release. (Parser speed matters for principle and for editor tooling, not build
time: either parser handles a 1,600-message locale in about a millisecond.)

`mf2_parser` (GPL-3.0-or-later, 2024 draft, fails 21/27 bidi tests, hangs on
one suite input) MUST NOT be used, even as an oracle.

## 2. Source files (decision D2): the W3C *Message Resource* draft

**Unicode does not define a file format for MF2.** The MF2 spec defines one
message and says only that its syntax is designed to embed in many containers,
"includ[ing] a future *MessageResource* specification" (`spec/syntax.md`). That
future specification is being written by the MF2 spec editor: it began as a
Unicode MessageFormat WG side effort, was presented at the 2024 Unicode Tech
Workshop, and since TPAC 2025 is **incubated by the W3C Internationalization
WG** (`w3c/i18n-discuss`, `explainers/message-resources.md`, with an 87-line
ABNF, a TypeScript data model and a JSON Schema). It is a prerequisite for the
DOM Localization proposal. Status: an explainer under incubation — **not a
standard, and it can change.**

mf2-two adopts it rather than inventing a container: it is the only candidate
with standards backing, it is designed around MF2's multi-line messages, and it
is nearly what we would have designed anyway.

```ini
# The resource-level locale is the only required property.
@locale en-US
---

chat-send = Send

@param $count - How many people are in the room; a whole number.
users-online =
  .input {$count :integer}
  .match $count
  one {{{$count} user online}}
  *   {{{$count} users online}}

[hotkeys]
# → id "hotkeys.release"
release = Release {#kbd}?{/kbd} to close

@do-not-translate
[brand]
name = Example
```

What the draft gives us: `id = value`; continuation lines indented; MF2's own
escapes pass through untouched (no double escaping); `#` comments and
`@property` metadata with **specified attachment rules** (to the next entry,
section or the resource); `[section]` headers producing dotted ids
(`hotkeys.release`), which are our namespaces; `@locale` frontmatter; a data
model that can also represent `.po`-style resources.

**Working grammar** (written from the draft at the pin; until the draft's own
ABNF can be vendored this is what Phase 0's workload generator and, later,
`mf2-resource` implement — where it and the pinned draft disagree, the draft
wins and this text is corrected):

* UTF-8; lines end in LF or CRLF. A line is a frontmatter separator, a section
  head, an entry, a property, a comment, or empty. Every non-empty line starts
  in column 0 except continuation lines.
* **Entry**: `id`, optional spaces, `=`, optional spaces, value. The value
  continues on following lines that start with at least one space or tab; that
  indentation is stripped, and each line break becomes one LF. To begin a value
  line with significant whitespace, escape its first character (`\ `).
* **Id**: one or more parts joined by `.`; a part is letters, digits, `_`, `-`
  and non-ASCII name characters; any other symbol is `\`-escaped. An id may not
  look like `---`.
* **Section head**: `[` id `]`. Entries that follow have ids prefixed with the
  section's id and a dot. Sections do not nest; each states its full path.
* **Frontmatter**: a line `---`. Comments and properties before it belong to the
  resource. `@locale <tag>` there is the one required property.
* **Property**: `@name` optionally followed by a value (continuable like an
  entry). It attaches to the next frontmatter, section head or entry; other
  properties may sit in between, comments and empty lines may not.
* **Comment**: `#` to end of line. Adjacent comment lines form one comment, which
  attaches to the next frontmatter, section head or entry if no empty line
  intervenes (properties may intervene).
* **Escapes in values**: MF2's own (`\\`, `\{`, `\|`, `\}`) pass through
  untouched to the message parser; additionally `\n`, `\r`, `\t`, `\xHH`,
  `\uHHHH`, `\UHHHHHH`, escaped space/tab, and an escaped line break (which,
  with the following indentation, is removed — a way to wrap long lines). A raw
  CR never appears in a value; control characters and U+2028/2029 are not allowed
  raw.

Until `mf2-resource` exists (P5a), anything that needs messages — probes, the
parser gate, the catalog-size budget — reads the **flat JSON** (`id → source`)
that `workload-gen` emits alongside the `.mf2` files.

How it lands here:

* New crate **`mf2-resource`** (`no_std` + `alloc`): parser and serializer for
  the resource syntax and its data model, generic over the message type exactly
  as the draft's `Resource<Message>` is. Tested by ABNF-driven generation and
  round-trip properties, and its JSON output validated against the draft's JSON
  Schema (the draft has no test suite of its own; ours is written in a form that
  can be offered upstream).
* The draft's files are **vendored and pinned** under
  `third_party/w3c-message-resource/` (explainer, ABNF, `.d.ts`, JSON Schema,
  license notice, `PIN`), synced by `cargo xtask resource-sync`. When the draft
  changes, `mf2 fmt` rewrites source files to the new syntax. **Before
  vendoring verbatim, confirm the license**: `w3c/i18n-discuss` carries no
  license file (checked 2026-09-20). If it cannot be confirmed as compatible
  with MIT redistribution, vendor nothing: keep only the `PIN` (URL + commit) and
  our own grammar written from the draft.
* `mf2-build` stays container-agnostic behind the `Loader` trait
  (`(id, source, spans, comments, properties, file)`), so a draft change — or a
  team that insists on JSON — never reaches the rest of the pipeline. Flat JSON
  (`id → source`) import/export ships from day one because every TMS speaks it.
* Metadata is used, not just preserved: `@param` descriptions and comments are
  exported as translator context; `@do-not-translate` is honoured by `check`
  (a differing translation is an error) and by pseudo-localization.
* The draft names no file extension or media type. Working choice: `.mf2`,
  laid out as `locales/<bcp47>/<namespace>.mf2`; revisit if the draft names one.
* Message ids are therefore dotted paths whose parts may contain `-`
  (`chat-send`, `hotkeys.release`); `tr!("hotkeys.release")`.

## 3. The manifest — what the wasm and the catalogs agree on

`mf2-build` derives one manifest from the **source locale**:

* **Ids → `MsgId`**: ids sorted, numbered densely (chunk bits reserved, see
  [02](02-catalog-format.md) §3).
* **Variable slots**: for each message, its external variables in ascending
  bytewise order of their **NFC-normalized** names → slot 0..n. Call sites pass
  arguments positionally; catalogs reference slots. Argument names never reach
  the wasm. (`tr!` argument names are Rust identifiers, which rustc already
  normalizes to NFC; MF2 names that are not valid Rust identifiers are passed
  with the raw-string form `tr!("id", "odd-name" = value)`.)
* **Markup names** per message (the handlers a call site must supply).
* **Functions used** across all locales (closed-world registry, B13).
* **`manifest_hash`** over the four items above — defined precisely in
  [02](02-catalog-format.md) §3.
* Separately, and **not** in the hash: the literal currencies / units /
  numbering systems used, which only slice locale data.

The manifest is written as `manifest.mf2m` by `mf2-build` and read by
`mf2-macros`; both use the `manifest` feature of `mf2-catalog`
([02](02-catalog-format.md) §5).

Rules for translations, enforced by `check`:

* A translation MAY use a subset of the source message's variables and markup.
* A translation MUST NOT use a variable or markup name the source lacks. If a
  language needs extra input (e.g. grammatical gender), the *source* message
  declares it with `.input`, even if its own pattern ignores it. The lint says so.
* Selectors and variants are free to differ per locale — that is the point of MF2.

### 3.1 Configuration — one home: `mf2.toml`

Beside the i18n crate's `Cargo.toml`; read by `mf2-build` (from `build.rs`) and
by `mf2-cli`, so both always agree:

```toml
source_locale = "en"
[fallback]                 # chains, flattened at build time (D5)
"es-MX" = ["es", "en"]
[catalog]
strip = ["cold", "ids"]    # production client catalogs
missing = "fallback"       # fallback | id | empty
[locale_data]
currencies = "used"        # "used" | "all" | ["USD", "EUR"]
units = "used"
[lints]
neutral-numbers = "warn"
unpaired-markup = "warn"
missing-translation = "warn"
[functions]                # custom functions for the generated registry
"app:emoji" = "my_app_i18n::functions::emoji"
```

The **client feature set** is not repeated here: `build.rs` reads it from the
i18n crate's own cargo features (`CARGO_FEATURE_FN_NUMBER`, …), which the
application forwards from its `ssr` and `hydrate` builds alike; `mf2-cli` takes
`--features` and otherwise reads the i18n crate's `default` features.

## 4. Build orchestration (decision D8; settled by probe P0.9)

```
my-app-i18n/            ← a crate in the application's workspace
  locales/<tag>/*.mf2
  build.rs              ← mf2_build::Build::new().source_locale("en").run()
  src/lib.rs            ← mf2::include_generated!();
```

`build.rs` (rerun-if-changed on `locales/`) parses, validates and lints all
locales; writes the manifest and the per-locale catalogs (+ `.br`, `.gz`,
content hashes) to `OUT_DIR` — each file only when its bytes change; and
generates a Rust module containing:

* `MANIFEST_HASH` and the locale table (tag, dir); the catalog file names and
  content hashes **only under `ssr`** — the client needs `MANIFEST_HASH` and the
  tags, and keeping hashes out of it is what makes the wasm byte-identical
  across translation edits (P0.9);
* `registry()` — the closed-world function registry;
* catalogs embedded for the **server** target only (`include_bytes!`, gated on
  the `ssr` feature): the server binary is self-contained and `mf2-axum` serves
  catalogs from memory; the client target embeds nothing;
* `pub use ::mf2 as __mf2;` — the path every `tr!` expansion goes through;
* an exported `tr!` wrapper (`macro_rules!`) that forwards to the `mf2-macros`
  proc-macro **with the absolute manifest path and the manifest hash baked in
  as literals**: `__tr_impl!("<path>" 0x<hash>u64 ; $crate ; …)`.

That last point solves discovery and invalidation without unstable features:
any crate that depends on the i18n crate can call `tr!`; the proc-macro reads
the manifest from the baked path (cached per compiler process keyed by path and
verified against the hash, so 2,000 expansions read it once — 0.12–0.45 s of
macro time, +0.2–0.3 s on `cargo check`, P0.9); a manifest whose hash differs
is reported as stale rather than used, which keeps a long-lived rust-analyzer
proc-macro server honest. When a locale file changes, `build.rs` reruns, the
generated module changes, and cargo rebuilds the i18n crate and its
dependents. Single-crate apps put `build.rs` and `locales/` in the app crate
itself.

cargo-leptos builds the app twice, in separate target directories (a third
with `--split`), so there are several OUT_DIRs and baked paths; the build is
deterministic (F8) and P0.9 found every manifest byte-identical at every step.
Measured behaviour and costs:

* **Every locale change recompiles the i18n crate and all its dependents** in
  both builds — even a translation-only edit or an mtime-only touch (cargo has
  no early cut-off); 8–23 s per debug `cargo leptos build` for a 2,000-site app.
  Outputs stay deterministic. Accepted until P7's dev hot reload; a mitigation
  to evaluate in P5a is moving server-only catalog embedding into a crate only
  the server binary depends on (owner question).
* **`cargo leptos watch` does not watch `locales/`**: `mf2 init` writes
  `watch-additional-files = ["<i18n crate>/locales"]` into
  `[package.metadata.leptos]`, and the docs say why.
* **Relocated target directories** (a CI cache restored elsewhere, a container
  mounting another path) leave the i18n crate fresh with a dead baked path; the
  proc-macro then looks for the same `build/<pkg>-<hash>/out/manifest.mf2m`
  under the profile directories rustc received as `-L dependency=…`, verified
  against the baked hash. **Inline mode** — the manifest bytes in the wrapper
  instead of a path — survives any relocation (remote execution) at +0.5 s per
  2,000 sites, growing with manifest size × sites: a documented opt-in.
* Plain cargo and cargo-leptos do not share build artifacts; CI should use one.

The proc-macro checks, at compile time: the id exists (with a did-you-mean), the
argument set equals the message's variables (unknown, missing and duplicate
arguments), every markup name has a handler, and no unknown argument is passed.
It emits a positional construction — see
[04-leptos-integration](04-leptos-integration.md) — spanned at the id literal;
several errors from one expansion are wrapped in a block (bare
`compile_error!`s in expression position misparse and hide the later ones); and
a cache hit never re-stringifies a large literal.

## 5. `mf2 check` — lints

Errors (fail the build): syntax and data-model errors in any locale; id present
in a translation but not the source; translation uses an undeclared variable or
markup name; `select` given a non-literal; malformed literal values for
well-known options; a function no registered crate provides (configurable, since
custom functions are legal); **a function whose client feature is off** —
`:percent`/`:currency`/`:unit` without `fn-number`, `:datetime`/`:date`/`:time`
without `fn-datetime` — reported with file and line, so a translation can never
silently add formatting code to the wasm (see [03-runtime](03-runtime.md) §5.1).

Warnings (configurable to errors): id missing in a translation (falls back —
reported with counts per locale); `neutral-numbers` — the corpus formats numbers
but `fn-number` is off, so digits render without locale symbols; `unpaired-markup`
— an open without a close or the reverse; a plural `.match` that does not mention every
category the *target* locale has; source text not in NFC; placeholder present in
source but dropped by a translation; unused ids (found by scanning the workspace
for `tr!` invocations); suspicious bidi (unpaired isolates in literal text).

## 6. `mf2-cli` (clap)

| Command | Purpose |
|---|---|
| `mf2 init` | scaffold `locales/`, an i18n crate, `build.rs` |
| `mf2 check` | all lints, machine-readable output for CI (`--format json`) |
| `mf2 compile` | catalogs without cargo (for CSR/static hosting and debugging) |
| `mf2 fmt` | canonical formatting of `.mf2` resources |
| `mf2 stats` | per-locale coverage, catalog sizes raw/gz/br, locale-data breakdown, CLDR + spec pins |
| `mf2 dump <file.mf2b>` | decode a catalog back to MF2 source / data-model JSON |
| `mf2 pseudo` | generate pseudo-locales (`en-XA` expanded/accented, `ar-XB` RTL) |
| `mf2 export` / `import` | flat JSON now; XLIFF 2 later |
| `mf2 convert --from fluent` | one-shot Fluent (`.ftl`) → `.mf2`: selectors → `.match`, `NUMBER`/`DATETIME` → `:number`/`:datetime`, terms and message references inlined, attributes → `id.attr`; reports anything it cannot map |
| `mf2 watch` | recompile on change; with `mf2-axum`'s dev mode, pushes the new catalog to open pages |

## 7. Locale data extraction (`mf2-locale-data`, build-side)

Input: `cldr-json` pinned at **48.2.1** (`third_party/cldr-json/PIN`; at this tag
upstream ships `cldr-core` and `-full` packages only). An application's
`build.rs` has no `third_party/` and must not download anything, so the crate
**ships its data**: `cargo xtask cldr-sync` downloads the pinned release into a
cache, extracts what we consume for **every** CLDR locale into compact tables
under `crates/mf2-locale-data/data/` (plural and ordinal rules — only 40 distinct
cardinal rule sets exist — number symbols and patterns, currency and unit
display data), and commits those. They are build-side only, so their size does
not matter to the wasm. `third_party/cldr-json/` keeps the small supplemental
files verbatim for review. Output: the LOCALE section entries of
[02](02-catalog-format.md) §4, sliced to what the manifest says the corpus uses.
The CLDR version is recorded in every catalog.

The crate is built in two steps: **plural and ordinal rules in Phase 3** (the
runtime's plural evaluator cannot be tested without them), everything else in
Phase 4.

Plural rules are parsed from the UTS #35 rule strings by a small parser in this
crate (no maintained runtime crate does this: the `icu_plurals` reference parser
is feature-gated and explicitly unstable, `intl_pluralrules` is frozen at CLDR 37
with every locale compiled in). The CLDR `@integer`/`@decimal` samples are
turned into tests for the runtime evaluator automatically.

## 8. Repository conventions

Rust 2024 edition; no `mod.rs`; latest dependency versions, none pinned except
the oracle dev-dependency; `Cargo.lock` not committed; every crate's errors in
`src/error.rs` with `thiserror` and `#[from]` conversions; `clap` for the CLI;
`axum` for anything that serves; containers, when needed, with podman/buildah;
CI in `.forgejo/workflows/` using Forgejo-hosted actions only. Code search with
`rg`.

**Client-path coding rules** (each learned from a B12 failure in P0.3–P0.5):
no infallible `Vec`/`String` growth in client crates — use `try_reserve*`, or
let the caller's `Sink` own growth; no `str::find` / `str::split*` — use
hand-written scans; no `copy_from_slice` without a proven length; no
`u64::checked_pow` / `checked_mul` on wasm32 (they pull in `__multi3`; use
bounded loops); mark undefined wasm imports with `#[link(wasm_import_module =
"…")]`. The B12 gate is a `no_std` harness whose `#[panic_handler]` calls an
imported function — the import must be absent after LTO + `wasm-opt` — plus a
`twiggy` name grep for `core::fmt`. Size deltas are always taken against a base
that keeps an allocation alive.

**Boundary.** The repository is self-contained; the rule is stated once, in the
root `CLAUDE.md` ("Boundary").

## 9. The `mf2` facade

The one crate an application names. It re-exports the public API of
`mf2-runtime`, `leptos-mf2` (feature `leptos`), `mf2-axum` (feature `axum`) and
the build entry point (feature `build`), carries the user-facing feature flags of
the master plan §5 and forwards them, and offers `mf2::compile_str` (std only:
parse + `writer::single`) for ad-hoc formatting on servers and in tests. It
contains no logic of its own. Created in Phase 3 (runtime + features), extended
in P4 (function features), P5b (`__mf2` path, `include_generated!`) and P6
(Leptos and Axum re-exports).
