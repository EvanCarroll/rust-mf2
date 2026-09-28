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

rust-mf2 adopts it rather than inventing a container: it is the only candidate
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

**What the working grammar leaves open, and how `mf2-resource` reads it**
(Phase 5a, A1). Each is a reading of the draft, not a departure from it; when
the draft can be vendored, each is re-checked against its ABNF:

* A line of nothing but spaces and tabs is an **empty line**, so it ends a
  value and detaches a comment. A value therefore cannot contain an empty
  line; `\n` writes one.
* A continuation line loses **all** of its leading whitespace, not a common
  prefix — which is why the grammar says to escape the first character to keep
  it (`\ `, `\t`).
* Trailing whitespace on a value line is **kept**: it is part of the message.
  (Leading whitespace after `=` is not, per the entry production.)
* Control characters and U+2028/2029 are rejected in a **comment** as well as
  in a value: a comment is text like any other, and nothing could write one
  back.
* An element carries one comment. Two comment blocks that properties separate
  (`# a` `@p` `# b` `entry`) both describe the entry, so they are read as one
  comment of two lines.
* A property with an empty value and one with no value are the same property:
  `@name` is what both write.
* In an id, `\` escapes any character (`\.` is a dot inside a part); a
  control character cannot appear in an id at all, escaped or not.

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
application forwards from its `ssr` and `hydrate` builds alike. `mf2-cli` takes
`--features`; otherwise `mf2 check` and `mf2 compile --site` ask cargo for the
i18n crate's features as its workspace resolves them (`cargo metadata`;
`check` with `--offline`), so they check and build what the build does.
Without an answer from cargo (no `Cargo.toml`, no cargo) `check` checks with
none and says so on stderr (Phase 9 B8, owner question 12); a plain `compile`
uses none.

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
any crate that depends on the i18n crate can call `tr!` — as
`my_app_i18n::tr!(…)`, and **inside the i18n crate itself only unqualified**,
because a `macro_export` macro that arrives through a macro expansion (here
`include_generated!` → `include!`) cannot be named by an absolute path in
its own crate (rustc [#52234]; `mf2 init`'s scaffold says so, and the
fixture's tests are written that way);

[#52234]: https://github.com/rust-lang/rust/issues/52234 the proc-macro reads
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
  Outputs stay deterministic. Accepted: the push of a recompiled catalog to
  open pages that would avoid it was deferred until after v1 (owner,
  2026-09-24; master plan §9 "Later").

  **`Build::emit(Emit::Module | Emit::Catalogs)` does not mitigate it**
  (P5a, owner question 1; measured in Phase 7 A6). The i18n crate emits the
  manifest and the module; a crate only the server binary depends on emits
  the catalogs and the table that embeds them. A text edit then rewrites
  nothing in the i18n crate's `OUT_DIR` — but its build script still has to
  rerun to see the edit, and cargo rebuilds a crate whose build script
  reran, and everything above it, whatever the script wrote. P5a's "cargo
  recompiles neither it nor anything above it" was reasoned, not timed, and
  was wrong: under `cargo leptos watch` on `examples/demo-ssr` both layouts
  recompiled the i18n crate and the app in both builds, ≈ 3 s an edit
  either way, and the wasm was byte-identical in both (the one-crate
  module's catalog names are behind `ssr`). **So one crate (`Emit::Both`)
  stays the default for an application with a server** (owner,
  2026-09-23, Phase 7 question 6); the split is for a client-only
  application, which has no server to embed catalogs in (`Emit::Module` +
  `mf2 compile --site`). Cutting the rebuild itself would need the i18n
  crate not to rerun on a translation edit — the functions a translation
  may use known without reading translations — a design change not yet
  planned. An mtime-only touch rewrites nothing either way: every output is
  written only when its bytes change.
* **`cargo leptos watch` does not watch `locales/`**: the application needs
  `watch-additional-files = ["<i18n crate>/locales"]` in its
  `[package.metadata.leptos]`. `mf2 init` cannot write it (the application's
  manifest is not its to edit) and prints it as a next step instead; the
  examples with a server set it (Phase 7 A6, asserted by an edit under
  `cargo leptos watch`).
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
arguments), and — for a call site that supplies markup handlers at all — that
every markup name of the message has one and none is unknown (handlers are
all or none, [04](04-leptos-integration.md) §2.1). It emits a positional
construction — see
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
reported once per locale, with the count and the first ten ids, Phase 9 B8); `neutral-numbers` — the corpus formats numbers
but `fn-number` is off, so digits render without locale symbols; `unpaired-markup`
— an open without a close or the reverse; a plural `.match` that does not mention every
category the *target* locale has; source text not in NFC; placeholder present in
source but dropped by a translation; unused ids (found by scanning the workspace
for `tr!` invocations); suspicious bidi (unpaired isolates in literal text);
`unknown-option` (Phase 9 A2) — an option a built-in function does not
define (`dateStyle` on `:datetime`), which MF2 ignores without an error;
options in a namespace are not checked. The lists are `mf2_build::OPTIONS`,
held against what the formatter reads by the conformance crate's
`options_lint` test.
`nonstandard-name` (Phase 7 A12, owner, 2026-09-24; `syntax.md` asks
linters to warn on names that break UAX #31 / UTS #39): a variable, option,
function, markup or attribute name the corpus uses that is not a UAX #31
identifier under MF2's profile (`-` and `.` added to Continue, `_` to Start;
each side of a namespace's `:` checked alone), that uses a character outside
UTS #39's Identifier_Status=Allowed, or that mixes scripts (UTS #39's
resolved script set, so Japanese kana with kanji is one script). Checked on
the NFC form; one warning per name per message. A warning, configurable like
the others; build-time only, with its Unicode data from `unicode-ident` and
`unicode-security` (measured in [phase-7-results](phase-7-results.md) §A12),
never in the client. *As built (Phase 7 A12): attribute names were added to
the list — they are names users choose as much as option names are.*

## 6. `mf2-cli` (clap)

| Command | Purpose |
|---|---|
| `mf2 init` | scaffold `locales/`, an i18n crate, `build.rs`. Since Phase 7 A13 the crate is ready for Leptos as written: `ssr`, `hydrate` and `csr` features forwarding `mf2`'s, and a `setup()` for `leptos_mf2::install` / `mf2_axum::install` — what every example had added by hand, and what the user documentation shows verbatim (checked by `cargo xtask docs`). `--no-messages` leaves out the starter `locales/<tag>/main.mf2` files, for a corpus that comes from elsewhere — `mf2 convert` writes into `locales/` and never overwrites (Phase 8 A4) |
| `mf2 check` | all lints, machine-readable output for CI (`--format json`); without `--features`, the i18n crate's features as cargo resolves them (§3), so a bare `check` reports what the build reports (Phase 9 B8) |
| `mf2 compile` | catalogs without cargo (for CSR/static hosting and debugging); `--site DIR` writes only what a static host serves — the catalogs and the `index.json` a client-only application reads to find them (Phase 7 A2, [04](04-leptos-integration.md) §8). With `--site` the functions are the i18n crate's features as `cargo metadata` resolves them (only `fn-number`, `fn-datetime` and `datetime-icu` change a catalog); a `--features` that names others fails, with both lists, and writes nothing — so the catalogs are built for the wasm's functions without the list being written twice (Phase 7 A6). DIR must be a cargo package |
| `mf2 fmt` | canonical formatting of `.mf2` resources |
| `mf2 stats` | per-locale coverage, catalog sizes raw/gz/br, locale-data breakdown, CLDR + spec pins |
| `mf2 dump <file.mf2b>` | decode a catalog back to MF2 source / data-model JSON |
| `mf2 pseudo` | generate pseudo-locales (`en-XA` expanded/accented, `ar-XB` RTL) |
| `mf2 export` / `import` | flat JSON, and XLIFF 2 (`export --format xliff`; `import` tells the two apart by content) against the standard vendored under `third_party/` (owner, 2026-09-24; built in Phase 8, [16](16-phase-8-work-order.md) A6; the mapping is §6.3) |
| `mf2 convert --from fluent` | one-shot Fluent (`.ftl`) → `.mf2`: selectors → `.match`, `NUMBER`/`DATETIME` → `:number`/`:datetime`, terms and message references inlined, attributes → `id.attr`; reports anything it cannot map. `--from leptos-fluent` also rewrites the project's call sites to `tr!` where the rewrite is mechanical, and reports the rest (Phase 8, [16](16-phase-8-work-order.md) A1, A4; the mapping is §6.1, the call-site rules §6.2) |
| `mf2 watch` | recompile on change. *The push of the new catalog to open pages through an `mf2-axum` dev mode is deferred until after v1 (owner, 2026-09-24)* |

Every command but `convert` ships in P5a. Three details the implementation
settled:

* **`fmt`'s canonical form is the one `bench/workload-gen` writes**, so
  `mf2 fmt --check` reports no change on a generated corpus: a value longer
  than 100 bytes wraps at column 76, filling greedily so a line breaks
  *before* the word that would pass the width; a value with line breaks starts
  under its `=` with every line at one indent and is never wrapped, since its
  line structure is the message's; and a blank line sets a comment off, not a
  bare property.
* **`watch` polls modification times** (300 ms by default) rather than
  subscribing to the operating system's file events: a corpus is a few hundred
  files, and nothing then has to know about inotify, kqueue or the editors
  that write through a temporary file.
* **`import` writes a translation back into the container it came from**,
  keeping every section, comment and property; an id the locale does not have
  is reported, not invented, because which section it belongs in is the
  translator's decision. (An XLIFF document says which: §6.3.)

### 6.1 `mf2 convert --from fluent` — the mapping

*Designed before code (Phase 8 A1, 2026-09-24), from `fluent-syntax` 0.12.0's
AST and `fluent-bundle` 0.16.0's resolver, both read at those versions.*
`fluent-bundle` is what a `leptos-fluent` application formats with, so
**"faithful" means: for the same arguments, the converted message selects
the variant `fluent-bundle` selects and writes the same text around it.**
How each side then renders a number, a date or an isolate is not the
mapping's business; A3 measures those differences, and the known ones are
listed at the end of this section.

**Command.** `mf2 convert --from fluent FTL_DIR` reads `FTL_DIR/<locale>/**.ftl`
(`fluent-templates`' layout, and the one `leptos-fluent`'s documentation shows: `locales/<lang>/main.ftl` — confirmed by A4 from its docs.rs page, 0.3.1) and writes
`<dir>/locales/<tag>/<name>.mf2` — the layout `mf2 init` makes, `<dir>` being
the global `--dir`. A file in a subdirectory is flattened, its path parts
joined by `.` (`menus/file.ftl` → `menus.file.mf2`), since the loader reads a
locale's directory flat. A file's name never enters its ids, on either side.
`<tag>` is the directory name with `_` read as `-`; `@locale <tag>` is written
in the frontmatter. Output is `mf2 fmt`'s canonical form (§6), so
`mf2 fmt --check` reports nothing on it. An existing `.mf2` with other text
is not overwritten: the command stops before writing anything and names it.
One that already holds what the command would write is left alone and not
counted as written (the summary says how many were unchanged), so a second
run writes nothing and exits as the first did (Phase 9 B8).
`--format json` gives the report as `mf2 check` gives its own
(`{"diagnostics": [{level, locale, file, line, column, id, code, message}]}`),
plus `inlined` (id → the terms and messages copied into it), `features` and
`entries` (per locale); the text form ends with one `note:` line per copied
term or message and one naming the features.
The exit status is non-zero when the report has an error; everything else
is still written, so the rest of a corpus can be checked while the errors are
fixed by hand.

**Positions.** `fluent-syntax`'s AST has no spans, but parsing a `&str` makes
every identifier and text element a slice of the source, so a construct's byte
offset is its slice's start minus the source's (safe pointer-to-address
arithmetic, no `unsafe`); line and column follow. A construct with no slice of
its own (a `Placeable`) is reported at the first slice inside it. A parse error
has its own range (`ParserError::pos`).

**Mapping.** Each row is an AST kind of `fluent-syntax` 0.12.0; the construct
corpus under `crates/mf2-cli/tests/` has one case for every row and one test
per code.

| Fluent construct | MF2 | Code when not mapped |
|---|---|---|
| `Entry::Message` with a value | entry `id = <value>` | — |
| its `Attribute`s | one entry each, `id.attr` (Fluent ids have no `.`, so an attribute id can never collide with a message id) | — |
| a message with attributes only | the `id.attr` entries alone | — |
| `Entry::Term` | no entry: terms are private in Fluent (`fluent-bundle` cannot format one by id), so each is **inlined** where referenced (below); a term's comment goes with it | — |
| a comment's tab or other control character (the resource syntax cannot hold one in a comment) | a tab becomes a space, any other is dropped — losing it is better than losing the file (as built) | — |
| `Entry::Comment` attached to a message (`#`) | that entry's comment | — |
| `Entry::Comment` standing alone (`#`, blank line after) | a detached comment at the same place (the resource model's `Detached`) | — |
| `Entry::GroupComment` (`##`) | a detached comment, its text kept. **Not a `[section]`**: a section prefixes every following id, which would rename the messages under it and break every call site; a Fluent group has no effect on ids | — |
| `Entry::ResourceComment` (`###`) | the resource's comment, above `---`; several are joined with a line of `#` between them | — |
| `Entry::Junk` | nothing | **`fluent-junk`** (error; at the parser's own range). *As built,* the code also covers text no MF2 message or resource can hold (U+0000), which `fluent-syntax` accepts |
| `PatternElement::TextElement` | text, escaped by the serializer (`{`, `}`, `\`; a leading `.` or whitespace by the resource's rules) | — |
| `Placeable` holding a `Placeable` | the inner one | — |
| `StringLiteral` in a placeable | its unescaped text, merged into the surrounding text (`fluent-bundle` does not isolate a string literal, and `{ "{" }` is Fluent's only way to write a brace) | — |
| `NumberLiteral` in a placeable | a literal placeholder of the text `fluent-bundle` writes for it, computed at conversion (`{ 007 }` → `{|7|}`, `{ 1.50 }` → `{|1.50|}`) | — |
| `VariableReference` in a message | `{$name}` (Fluent names are valid MF2 names) | — |
| `VariableReference` in a term body, bound by the reference's arguments | the bound literal, as the rows above | — |
| … unbound (`fluent-bundle` writes `{$name}` as text, without an error) | the text `{$name}`, escaped | **`fluent-unbound-term-variable`** (warning: converted faithfully, but it is almost certainly a bug in the original) |
| `MessageReference` (`{ other }`, `{ other.attr }`) | the referenced value or attribute, **inlined**, recursively; its variables are the referring message's, as in `fluent-bundle` | — |
| … to a missing message, attribute or value | nothing | **`fluent-missing-reference`** (error) |
| … a cycle | nothing | **`fluent-cyclic-reference`** (error) |
| `TermReference` (`{ -brand }`, `{ -brand(case: "gen") }`) | the term's value (or, if `fluent-bundle` would render it, an attribute), inlined, with the named arguments bound; a positional argument is ignored, as `fluent-bundle` ignores it | missing term: **`fluent-missing-reference`**; positional argument: **`fluent-term-positional`** (warning) |
| `FunctionReference` `NUMBER(…)` in a placeable | `{$x :number …}`, or `:percent` / `:currency` by `style`, options by the table below | operand not a variable or number literal (`fluent-bundle` returns an error for it): **`fluent-number-operand`** (error) |
| `FunctionReference` `DATETIME(…)` | `:date` / `:time` / `:datetime` by the table below | an option with no MF2 counterpart: **`fluent-datetime-option`** (error) |
| any other `FunctionReference` | nothing — a custom Fluent function has no MF2 counterpart the converter can know | **`fluent-unknown-function`** (error) |
| `Expression::Select` | hoisted into the message's `.match` (below) | see below |
| a message whose `.match` would exceed 256 variants | nothing | **`fluent-variant-limit`** (error) |
| a file whose flattened name collides with another's | the first, in path order | **`fluent-file-collision`** (error) |
| an id defined twice in one locale (in one file or two) | the first, in path order — which `fluent-bundle`'s `add_resource` keeps depends on the application's load order, so it is not guessed | **`fluent-duplicate-id`** (error) |
| a locale directory whose name is not a well-formed BCP 47 tag | nothing | **`fluent-locale`** (error) |

**Selection.** MF2 has one `.match` per message; Fluent puts a select
expression anywhere in a pattern, and inside another's variant. Every select
expression is **hoisted**: each becomes one selector, and the message's
variants are the product of their keys, each variant's pattern being the
Fluent pattern with every select replaced by the chosen variant's pattern. A
select nested inside a variant contributes keys only under that variant; under
the others its column is `*`. Two selects on the same selector and the same
keys are one column (they always choose alike). Keys keep Fluent's order, `*`
last. The product is capped at 256 variants — beyond that, hand conversion is
the better answer (`fluent-variant-limit`).

*The selector.*

* A selector that is constant at conversion — a string or number literal, a
  term attribute (`{ -brand.gender -> … }`, Fluent's grammatical-gender idiom),
  a term variable bound to a literal — is **evaluated at conversion**: the
  variant `fluent-bundle` would pick is kept and the select disappears.
* `NUMBER($x, …)` → `.input {$x :number …}`, keeping **only**
  `select=ordinal` (from `type: "ordinal"`) and `minimumFractionDigits`:
  those are the options that reach `fluent-bundle`'s plural operands (the
  rule set, and the padding of `v` and `f`). A rounding option
  (`maximumFractionDigits`, the significant digits) does not reach them, so
  carrying it would make MF2 select on a rounded value Fluent never sees
  (1.2 under `maximumFractionDigits: 0` is `other` in Fluent, `one` in MF2).
  A placeholder `NUMBER` keeps every option (the table below). *(As built,
  Phase 8 A1.)*
* A bare `$x` has no type in Fluent — `fluent-bundle` matches a plural category
  if the argument is a number, a key's string if it is a string. The converter
  **infers it from the keys**: `:number` if any key is a number or a plural
  category other than `other`; `:string` if every key is an identifier that
  is not one of those (`other` is Fluent's usual default for a gender or a
  kind too); both at once is **`fluent-mixed-keys`** (error), since the
  argument decides at run time and the converter cannot know it. The
  documented consequence: a string argument whose keys look like plural
  categories is converted as a number (A3 checks the rule on the corpus).
* `DATETIME(…)` as a selector: **`fluent-date-selector`** (error; MF2 dates do
  not select). Any other function: `fluent-unknown-function`.
* One variable selected under two different annotations (a cardinal and an
  ordinal `NUMBER` of `$n`) gets `.input` for the first and
  `.local $n-2 = {$n :number select=ordinal}` for the next, the suffix the
  smallest integer that collides with no name in the message.
* A select left with only its default (`{ $count -> *[other] … }`) stays a
  one-variant `.match` rather than being folded into its pattern: folding
  would drop `$count` from the message's arguments, and every call site that
  passes it would stop compiling. *(As built.)*

*The keys.* An identifier key is an unquoted MF2 key; a number key is written
as Fluent wrote it when that is an MF2 `number-literal` (`1.50` keeps its
digits), else as the value `fluent-bundle` reads (`007` → `7`); a number
literal placeholder likewise. Fluent's default `*[k]` becomes `*`, plus an explicit `k`
variant with the same pattern **only when `k` is a number** — MF2 prefers an
exact number to a category, so dropping it could change the result (`*[0]`
before `[one]` in French); an identifier or category `k` adds nothing `*` does
not already cover.

*Source order against best match.* `fluent-bundle` takes the **first**
variant in source order whose key matches; MF2 takes the **best** (an exact
number before a category). They differ only where a Fluent variant can never be
reached, and the converter drops exactly those, each with
**`fluent-unreachable-variant`** (warning — the output is faithful, the
original had dead text):

* a key repeated in one select (the later one);
* a number key after a category key that the number's plural category equals
  — cardinal or ordinal as the selector is, in the resource's locale, with
  `mf2-locale-data`'s rules (`[one] … [1] …` in English);
* a number key under a `NUMBER` with any option set away from its default:
  `fluent-bundle` compares a key with the argument **including their number
  options** (`FluentNumber`'s `PartialEq`), so `[1]` never matches
  `NUMBER($n, minimumFractionDigits: 1)`; and conversely `[1.0]` matches only
  a `NUMBER` whose only option is `minimumFractionDigits: 1`.

**`NUMBER` options** (`FluentNumberOptions::merge`; an option of the wrong type
or an unknown name is ignored by `fluent-bundle`, so it is dropped with
**`fluent-number-option`**, a warning):

| `NUMBER` option | MF2 |
|---|---|
| `style: "decimal"` / absent | `:number` |
| `style: "percent"` | `:percent` |
| `style: "currency"` + `currency: "EUR"` | `:currency currency=EUR`; without `currency`: **`fluent-currency-missing`** (error) |
| `currencyDisplay: "symbol" \| "code" \| "name"` | the same value |
| `useGrouping: "false"` | `useGrouping=never` (any other string means true: omitted) |
| `minimumIntegerDigits`, `minimumFractionDigits`, `maximumFractionDigits`, `minimumSignificantDigits`, `maximumSignificantDigits` | the same name and value; under `:currency`, equal minimum and maximum fraction digits become `fractionDigits`, unequal ones are **`fluent-number-option`** |
| `type: "ordinal"` | `select=ordinal` (formatting ignores it on both sides) |
| `type: "cardinal"` | omitted |

**`DATETIME` options** (Fluent's are `Intl.DateTimeFormat`'s; MF2's `:date`,
`:time` and `:datetime` take semantic fields and lengths, so this mapping is
the nearest one, not an identity). The function is `:date` when only date
options are present, `:time` when only time options are, `:datetime` when
both are; with no options, `Intl`'s default (numeric year, month and day) →
`:date length=short`.

| `DATETIME` option | MF2 |
|---|---|
| `dateStyle: full \| long \| medium \| short` | `length` (`dateLength`) `long` (with `fields=year-month-day-weekday` for `full`) / `long` / `medium` / `short` |
| `timeStyle: full \| long \| medium \| short` | `precision` (`timePrecision`) `second` + `timeZoneStyle=long` / `second` + `timeZoneStyle=short` / `second` / `minute` |
| `weekday`, `year`, `month`, `day` (no `dateStyle`) | `fields` (`dateFields`) from the set present: `weekday`, `day-weekday`, `month-day`, `month-day-weekday`, `year-month-day`, `year-month-day-weekday`; another set (`year` alone, `year` + `month`, …) is **`fluent-datetime-option`**. Length from `month` (`long` → `long`, `short` → `medium`, `narrow`, `numeric`, `2-digit` → `short`), else from `weekday` (`long` → `long`, else `medium`) |
| `hour`, `minute`, `second` (no `timeStyle`) | `precision` = the finest present; a set not starting at `hour` is **`fluent-datetime-option`** |
| `timeZoneName` | `long`, `longGeneric` → `timeZoneStyle=long`; the others → `short` |
| `timeZone` | `timeZone`, the same value |
| `hour12` / `hourCycle` | `hour12=true` / `false` (`h11`, `h12` → true; `h23`, `h24` → false) |
| `era`, `fractionalSecondDigits`, `dayPeriod`, anything else | **`fluent-datetime-option`** |

A `DATETIME` whose operand is neither a variable nor a string literal is
**`fluent-datetime-option`** too. `fluent-bundle`
0.16 has no `DATETIME` built in (only `NUMBER`), so A3 has no oracle for these
rows; they are checked by the construct corpus, and the report counts every
converted `DATETIME` with **`fluent-datetime-approximate`** (warning) so that
nobody mistakes the nearest form for an identity.

**Features.** A converted corpus that uses `:percent` or `:currency` needs
`fn-number`, and one that uses a date function needs `fn-datetime` (§5,
*gated function*). The report ends with the features the output needs, so the
`mf2 check` that follows is not a surprise.

**What conversion loses by design.** Inlining is a copy: after conversion a
term such as `-brand` lives in every message that used it, and a later edit to
it is an edit to each. The report counts inlined terms and references per
id so that this is visible, and the migration guide (A4) says so.

**Known differences by construction, for A3 to count** (not mapping errors —
the converted message selects the same variant; the text of a placeholder
differs):

* `fluent-bundle` writes a number with `f64`'s `Display` (no grouping, no
  locale symbols; `minimumFractionDigits` pads). `:number`, and an unannotated
  number with `fn-number` on, localize and group.
* `style: "percent"` does not multiply by 100 in `fluent-bundle`
  (`as_string` ignores `style`); `:percent` does. Currency likewise prints a
  bare number in `fluent-bundle`.
* Isolation: `fluent-bundle` wraps a placeable in FSI/PDI only when the
  pattern has more than one element and the placeable is not a string
  literal, message or term reference, and it isolates a referenced message's
  placeables by *that* message's element count; MF2's Default Bidi Strategy
  isolates every placeholder of the flattened message.
* A selector's `.input` annotates the variable for the whole message, so a
  bare `{$n}` in a variant formats through it (`:number`, with
  `minimumFractionDigits` if the `NUMBER` selector had it); `fluent-bundle`
  writes the argument as given.
* *Found by A3:* a selector's `:number` selects on the value it shows,
  rounded to `maximumFractionDigits` (3 by default), so 1.0004 selects
  `one` in English where `fluent-bundle` selects `other` on the exact value
  — here the variant differs, and the shown number and the wording agree.
* *Found by A3:* `fluent-bundle`'s plural rules (`intl_pluralrules`) are
  older CLDR than `mf2-locale-data`'s: French 1,000,000 is `many` here and
  `other` there, and it gives an ordinal of a non-integer a category CLDR
  does not (English 1.5 is `one` there). Only a select whose default is not
  `other` shows it.

All of these are accepted as what "formats identically" allows (owner,
2026-09-24; [16](16-phase-8-work-order.md) question 5, with the counts in
§A3). The last one, not a class of A3's (no sampled pair shows it):

* A term reference inside a term: `fluent-bundle` clears the term's
  arguments when the inner reference returns (its `local_args = None`), so a
  variable *after* it in the outer term reads the message's arguments. The
  converter keeps the outer term's, as the `VariableReference` row says —
  the other reading is a `fluent-bundle` defect, not Fluent's meaning.

**The codes**, stable like `mf2 check`'s lint names (a code's meaning never
changes; a retired one is not reused). *Errors* — the construct is not
converted, the exit status is non-zero: `fluent-junk`,
`fluent-missing-reference`, `fluent-cyclic-reference`,
`fluent-number-operand`, `fluent-currency-missing`,
`fluent-datetime-option`, `fluent-date-selector`, `fluent-unknown-function`,
`fluent-mixed-keys`, `fluent-variant-limit`, `fluent-file-collision`,
`fluent-duplicate-id`, `fluent-locale`. *Warnings* — converted faithfully,
but a person should look: `fluent-unbound-term-variable`,
`fluent-term-positional`, `fluent-number-option`,
`fluent-unreachable-variant`, `fluent-datetime-approximate`. A2's corpus
MUST convert with none of either (it does: 6,464 entries in four locales, no
finding — [16](16-phase-8-work-order.md) §A2).

### 6.2 `mf2 convert --from leptos-fluent` — the call sites

*Designed before code (Phase 8 A4, 2026-09-24), from `leptos-fluent`
0.3.1's documentation on docs.rs: the `macro_rules!` of `tr!` and
`move_tr!`, the crate page's layout and example, and `leptos_fluent!`'s
example.* What the macros accept:

* `tr!("id")`, `tr!("id", { "name" => value, … })` — a `String`, formatted
  when the expression runs; `move_tr!` with the same arguments — a
  `Signal<String>`, documented as `Signal::derive(move || tr!(…))`;
* each of those with the context first, `tr!(i18n, "id", …)`;
* `#[cfg(…)]` attributes before the id or before the argument map;
* an `if` form choosing between literal ids, `tr!(if c { "a" } else { "b" })`;
* an id that is any expression, not a literal.

**Command.** `mf2 convert --from leptos-fluent APP_DIR`, `APP_DIR` being the
application's crate (the directory of its `Cargo.toml`):

1. **The messages.** The `.ftl` directory is `--locales DIR`, else the
   `locales: "…"` string of the project's `leptos_fluent!` or
   `static_loader!` (relative to `APP_DIR`, as `leptos-fluent` reads it),
   else `APP_DIR/locales`. §6.1 converts it into `--dir`'s `locales/`.
2. **The Rust sources.** Every `.rs` file under `APP_DIR` — except `target/`,
   hidden directories and `--dir`'s own tree — that names `leptos_fluent`
   (a `use` or a path) is rewritten by the rules below. A file that does not
   name it is left alone: its `tr!` is not `leptos-fluent`'s, which also
   makes a second run change nothing. A file whose edits give back its own
   text — one that still names `leptos_fluent` after the first run, its
   calls in view positions already `tr!` — is not a rewrite: no diff, not
   counted, not written (Phase 9 B8).
3. **The manifests.** Every `Cargo.toml` line under `APP_DIR` naming
   `leptos-fluent` or `fluent-templates` is reported.

Without `--write` nothing is written: the command prints a unified diff of
every Rust file it would change and the list of `.mf2` files it would write,
then the report. `--write` writes both (a `.mf2` is never overwritten, as
§6.1). The report is §6.1's — text, or `--format json` with the same
fields — and the exit status is non-zero when it has an error: **a
migration is finished when the report is empty.** The i18n crate's name,
which the `use` rewrite writes, is `--i18n-crate NAME`, else the package
name in `--dir`'s `Cargo.toml` (`-` read as `_`); `mf2 init --no-messages`
makes that crate without the starter messages a conversion would collide
with.

**Edits.** A file is tokenized with `proc-macro2` (span locations on); every
`tr!` and `move_tr!` is found at any depth — inside `view!` and inside any
other macro — and its arguments are parsed with `syn`. The rewrite is a
byte-range edit of the call (and, for the closure rule, of the closure), so
comments, formatting and every other byte of the file stay as they were. An
argument's value is copied as its source text, with any call inside it
rewritten too.

**Positions.** A **view position** is one where Leptos renders the value:
inside a `view!` (at any nesting of `view!`), a brace block whose whole
content is the call — `{tr!(…)}`, a child, or an attribute or prop value
written with braces — or an attribute or prop value written without them
(`attr=tr!(…)`). Everything else is a **`String` position** — a function
body, a closure's body, a `match` arm, an `if` branch, another macro's
arguments — because a `String` is what `leptos-fluent`'s `tr!` gives there.
A call's arguments are **constant** when every value is a literal, a path
(`n`, `self::X`), or a reference to one: evaluating them again gives the same
value, so a closure around the call adds nothing.

| `leptos-fluent` | mf2 | Rule |
|---|---|---|
| `tr!(…)` in a `String` position | `tr!(…).to_string()` — the same `String`, formatted when the code runs (not appended when the call is already followed by `.to_string()`) | `tr-string` |
| `tr!(…)` in a view position | `tr!(…)`: the description renders itself — and now follows a language switch, which the `String` did not | `tr-view` |
| `move_tr!(…)` in a view position, constant arguments | `tr!(…)`: a description follows the language by itself | `move-tr-view` |
| `move_tr!(…)` anywhere else, or with arguments that are not constant | `Signal::derive(move \|\| tr!(…).to_string())` — `move_tr!`'s own documented expansion: the same `Signal<String>`, re-reading its arguments. (Passing a signal itself as the argument, `count = count`, is the idiomatic form, and the guide says how; the codemod cannot know that a `.get()` is a signal's) | `move-tr-signal` |
| `move \|\| tr!(…)` or `\|\| tr!(…)` in a view position, the closure's body exactly the call, constant arguments | `tr!(…)`. With arguments that are not constant the closure stays and its body is a `String` position (`tr-string`) | `closure` |
| `{ "name" => value, … }` | `name = value, …`; an empty map, nothing. A name that is not a Rust identifier (Fluent allows `-`) is written quoted, `"a-b" = value`, which `tr!` accepts (§3) | `arguments` |
| `tr!(i18n, …)`, `move_tr!(i18n, …)` | the context dropped (a variable left unused by it is the compiler's to report) | `context` |
| `"id.attr"` — a Fluent attribute, which `fluent-templates` looks up as `id.attr` | unchanged: §6.1 gave the attribute the id `id.attr` | `attribute` |
| `use leptos_fluent::tr;`, `use leptos_fluent::{move_tr, tr};` — a private `use` whose only items are `tr` and `move_tr` | `use <i18n crate>::tr;` | `import` |
| `leptos_fluent::tr!(…)`, `leptos_fluent::move_tr!(…)` | `<i18n crate>::tr!` by the rules above | `import` |

**Checked, still rewritten.** Each call is checked against the converted
messages of the source locale (`--dir`'s `mf2.toml`, else `en`), so the report
says what the compiler will say, before it does:
**`leptos-fluent-unknown-id`** — the id is not a message there (a Fluent
message with attributes and no value is one); **`leptos-fluent-arguments`**
— an argument the message does not have (Fluent ignores it; `tr!`
refuses it), or a variable no argument gives (Fluent writes `{$x}`; `tr!`
refuses to compile).

**Reported, not rewritten** (at the construct's line and column):

| Construct | Code | What to do (the guide) |
|---|---|---|
| `leptos_fluent! { … }`, `static_loader! { … }` — the initializer | **`leptos-fluent-initializer`** | the i18n crate's `setup()`, `leptos_mf2::install` and `mf2_axum::install`. With `cookie_name: "…"`, the finding also says to add `CookieLocale { name: "…", ..Default::default() }` to the `Negotiator` **as an extra source** (after the default `CookieLocale`, before `AcceptLanguage`), not a rename: the client always writes `mf2_locale` (Phase 9 B8) |
| `I18n`, or any `leptos_fluent::` path but the macros — the context, its `language` / `languages` (the language selector) and `tr` / `tr_with_args` (lookups by run-time id) | **`leptos-fluent-context`** | `<LocaleSwitcher>` or the locale API; `msg_id!` and `TrDyn` for a run-time id |
| a `use leptos_fluent::…` naming anything but `tr` and `move_tr`, a `pub use` or a rename of them, or any `use` when the i18n crate's name is unknown; a `move_tr!` in a file that does not name `leptos_fluent` (it came through a re-export) | **`leptos-fluent-import`** | import the i18n crate's `tr` |
| a call whose id is not a string literal | **`leptos-fluent-dynamic-id`** | a literal id, or `msg_id!` and `TrDyn` |
| the `if` forms | **`leptos-fluent-if-form`** | `if c { tr!("a") } else { tr!("b") }`, each branch the same type |
| `#[cfg(…)]` on the id or the arguments | **`leptos-fluent-cfg`** | a `#[cfg]` on a statement around the call |
| an argument key that is not a string literal | **`leptos-fluent-argument-name`** | the name, as the message spells it |
| a call whose arguments are none of the forms above (and, for the negative control, a call a disabled rule would rewrite) | **`leptos-fluent-call`** | by hand |
| a Rust file that does not tokenize | **`leptos-fluent-parse`** | fix the file; nothing in it was rewritten |
| a `Cargo.toml` line naming `leptos-fluent` or `fluent-templates` | **`leptos-fluent-dependency`** | a dependency on the i18n crate, and its features |
| … and the two checks above | `leptos-fluent-unknown-id`, `leptos-fluent-arguments` | the message, or the call |

Every code is an error: each is work left before the application builds
on mf2. The codes are stable, like §6.1's; one test per code and one per
rule (`crates/mf2-cli/tests/convert.rs`).

**What the rewrite changes in behaviour, by design.** A `tr!` in a view
position was a `String` fixed when the view was built; it becomes a
description, which re-renders on a language switch — the fix `leptos-fluent`
asks for with `move_tr!` or a closure. Isolation follows MF2's Default Bidi
Strategy and numbers are localized (§6.1's accepted differences). Nothing
else: every `String` stays a `String`, every `Signal<String>` a
`Signal<String>`, and an argument is evaluated exactly as often as before.

### 6.3 `mf2 export` / `import` in XLIFF 2 — the mapping

*Designed before code (Phase 8 A6, 2026-09-25), from XLIFF 2.1, the OASIS
Standard vendored in `third_party/xliff/` (D13). Only the core is used: no
module, so a document validates against `schemas/xliff_core_2.0.xsd` alone
and any XLIFF 2 tool can read it.* The flat JSON of §6 stays; XLIFF is the
format a translation tool can **protect** in: what is text is text, and
everything MF2 means by `{…}` is an inline code the translator moves or
drops but cannot edit.

**Commands.** `mf2 export --format xliff LOCALE [--out FILE]` writes one
document with `srcLang` the manifest's `source_locale` and `trgLang`
LOCALE; LOCALE equal to the source locale is refused (there is nothing to
translate). `--format json` stays the default. `mf2 import LOCALE FILE`
reads either format, told apart by content (an XML document whose root is
`xliff`); the document's `trgLang` must be LOCALE, else nothing is written.
`--dry-run` and the rule of §6 hold for what the locale has: the container
keeps every section, comment and property. *As built — a departure:* a
message the target locale does not have yet **is** written, where the
source has it — in the target's file of the source file's name, in the
section of the same id, after the nearest message before it in the
source that the target has (a file or section the target lacks is
added, at the end), with no comment or property of its own. §6's rule
reports such an id because flat JSON says nothing about where it
belongs; an XLIFF document carries the file and the section, so nothing
is guessed, and without it a translation of a new message — the common
case, and the one owner question 7's four Polish forms describe — could
never land. A flat JSON target gains the id at its end. A
`translate="no"` unit never writes anything.

**Structure.**

| MF2 / resource | XLIFF 2 |
|---|---|
| the document | `<xliff version="2.1" srcLang trgLang xml:space="preserve">` — a message's whitespace is significant everywhere |
| a source resource file (`locales/<src>/<name>.mf2`; a flat JSON locale is one file) | `<file id="f1"…` in path order, `original="<name>.mf2"`, `canResegment="no"` (a message is not split into sentences) |
| the resource's comment | the file's `<note category="comment">` |
| a section `[hotkeys]` | `<group id="s:hotkeys" name="hotkeys" type="mf2:section">`, its comment a note of the group |
| a message without `.match` | `<unit id=… name="<full id>">` with one `<segment>`: `<source>` the source locale's pattern, `<target>` the target locale's when it has the message (`state="translated"`), none when it does not |
| a message with `.match` | `<group id=… name="<full id>" type="mf2:select">`, one `<unit>` per variant, `id="<message id>:<n>"` in the group's order, `name` its keys as MF2 writes them (`one`, `*`, `=0 *`) — *which variants: the target locale's, below* |
| an entry's comment | `<note category="comment">` of its unit (or group) |
| `@param $count - …` | `<note category="param">$count - …</note>`, as written |
| any other property `@name value` | `<note category="property">@name value</note>` (context only) |
| `@do-not-translate` (on the resource, a section or an entry) | `translate="no"` on the file, group or unit it attaches to |

An XLIFF `id` is an `NMTOKEN`: a message id is used as it is when it is
an ASCII one (letters, digits, `.`, `_`, `-`; every id of the reference
workload), otherwise `x:` followed by its UTF-8 in lowercase hex — the
`:` keeps the two forms apart (*as built:* the design said `x` and hex,
which a real id such as `x6869` could equal; and ASCII only, since
XML's name characters beyond it differ between XML 1.0 editions and so
between validators); `name` always carries the id as written. Group and unit ids are separate scopes in XLIFF, and sections
are prefixed `s:`, so no two can meet.

**Inside a pattern.** Text is text; a character XML cannot carry is
`<cp hex="…"/>`. Every expression is an inline code whose original data —
the expression exactly as `mf2-syntax`'s serializer writes it — is a
`<data>` of the unit's `<originalData>`, and whose `disp` is that same text,
so a tool shows `{$count}` where it stands:

| MF2 | Inline code |
|---|---|
| a placeholder `{$count}`, `{$d :datetime}`, `{|literal|}` | `<ph id dataRef disp>` |
| markup `{#b}` … `{/b}` closed in the same pattern and properly nested | `<pc id dataRefStart dataRefEnd dispStart dispEnd type="fmt">` |
| standalone markup `{#br/}` | `<ph … type="fmt">` |
| an open without its close, or a close without its open (valid MF2) | `<sc isolated="yes">` / `<ec isolated="yes">` |

Codes keep XLIFF's defaults — copy, delete and reorder allowed — because a
translation may drop `{$count}` in `one`, repeat it, or move it. Import
accepts `<pc>` and an `<sc>`/`<ec>` pair interchangeably (a tool may
convert one into the other, XLIFF 4.7.7), and a code with `copyOf` in
place of its data (a copy of a code of the unit).

*As built* (Phase 8 A6 (c)): a unit has one `<data>` per distinct
expression text, `d1`, `d2`, … in order of first appearance, the
source's first; its codes are numbered in the source, and a code of the
target that is the same code (kind and data) as one of the source's
keeps that one's id, as XLIFF asks, while a code only the target has
takes the next number. A close markup pairs with the innermost open one
when their names match, which keeps every `<pc>` properly nested;
anything that does not pair so is isolated. A note, and a `disp`, are
never read back, so a character XML cannot carry is written there as
U+FFFD; everywhere else it is a `<cp>`. A carriage return is written
`&#xD;` (and in an attribute, a tab or line feed as a reference too), so
a parser's normalization gives back exactly the text.

**What is not in the document.** Declarations (`.input`, `.local`) and
selectors are not text and are not written: a target message keeps its
own, and a new one takes the source's. Import never reads MF2 syntax from
the document except through a code's data.

**Import, and what it refuses.** Import exports the current tree in
memory and compares: the document's `<file>`, group and unit ids, each
unit's `<originalData>` and each variant's keys must be what that export
writes. Then each unit's `<target>` is read back into a pattern (text, and
each code replaced by its data) and the target message is rebuilt from its
declarations, its keys and those patterns. A message whose data model is
unchanged keeps its bytes, so **an export imported back changes nothing**;
a changed one is written by the serializer in `mf2 fmt`'s form. A refused
unit leaves its message as it was and is reported, with the file's and the
unit's ids — and a refused unit of a `.match` message leaves the whole
message as it was. The command exits non-zero when anything was refused,
after writing what was not.

| Finding | Code |
|---|---|
| a `<data>` that differs from the export's — an edited protected code | **`xliff-code-edited`** |
| a code whose `dataRef` names no data of its unit, or a code with neither `dataRef` nor `copyOf` (a brand-new code, XLIFF 4.7.2.4.2: it has no MF2 meaning) | **`xliff-unknown-code`** |
| a file, group or unit id, or a variant's keys (a unit's `name`), the export does not have (the document is stale, or the id is not in the source locale) | **`xliff-unknown-unit`** |
| a `translate="no"` unit whose target differs from its source | **`xliff-do-not-translate`** |
| a `.match` message whose catch-all (`*`) variant has no target — it cannot be written | **`xliff-incomplete`** |
| not well-formed XML, not XLIFF 2, or no `trgLang` | **`xliff-malformed`** |

A unit with no `<target>`, or an empty one, is untranslated: it adds no
variant and changes no message. The `state` of a segment is the tool's and
is not read.

**Validation.** Every export in the tests is checked with `xmllint --noout
--schema third_party/xliff/schemas/xliff_core_2.0.xsd` (core only; `type`
values are `mf2:…`, the core's `userDefinedValue`). XML is read and written
with `quick-xml`, a dependency of `mf2-cli` only.

**Which variants a `.match` message offers** — *owner question 7 of
[16](16-phase-8-work-order.md).*
**Answered (owner, 2026-09-25): the target language's forms.** A
translator never needs to know MF2 to give their language every plural
form it has:

* A selector that selects by plural category — `:number` or `:integer`
  without `select=exact` (cardinal), or with `select=ordinal` (ordinal) —
  offers the **target locale's** CLDR categories for that kind, in CLDR's
  order, except `other`, which is the catch-all `*`; before them, the
  source's exact-number keys (`=0`, `0`), in the source's order. Any other
  selector (`:string`, `select=exact`) offers the source's keys.
* More than one selector: every combination, in order (Arabic with two
  counts: up to 36 units — accepted).
* When the target already has the message, its variants come first, as
  they stand and in its order; then every combination above it lacks,
  with no target.
* Each unit's `<source>` is the source variant MF2 would pick for those
  keys: the most specific source variant whose every key equals the unit's
  or is `*` (Polish `few` and `many` show English `*`, "{$count} files").
  When the target's selectors are not the source's (another number of
  them), every unit shows the source's all-`*` variant, and a
  `<note category="comment">` says the source selects differently.
* A unit left empty adds no variant; the catch-all must have a target
  (`xliff-incomplete`). A new variant goes before the target's catch-all,
  where a person would write it. So a Polish translation of an English one/other
  message comes back with four variants, an unchanged export comes back
  as it went, and a form the translator did not fill is simply absent —
  the reader gets the catch-all for it, as MF2 would.

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

**Two inputs** (Phase 4, A2). `third_party/cldr-json/` vendors the supplemental
files and the 11-locale panel's `numbers.json`, `currencies.json` and
`units.json` (unchanged); `cargo xtask cldr-sync` also materialises, in its
cache `target/xtask-cache/cldr-json` and **never vendored**, those three files
for **every** locale (766 at 48.2.1; 2,301 files, 143 MB) plus
`availableLocales.json`, `defaultContent.json` and
`supplemental/parentLocales.json`, each checked byte-for-byte against the
pinned commit's blob (`git hash-object --no-filters`; the PIN's `cache`
field). One sync serves A2's numbers and A4's currencies and units. `cargo
xtask locale-data` reads the cache (it must be at the PIN's commit) for the
all-locale number table and the vendored files for everything else; the drift
test (`tests/table.rs`, feature `extract`) holds the committed tables to
`third_party/` offline where the vendored files reach — the digits, and every
panel locale resolving to its vendored record, for numbers, currencies and
units — and regenerates the whole number, currency and unit tables from the
cache in `#[ignore]`d tests.

**The `-full` locale files are resolved.** At 48.2.1 every locale's
`numbers.json` spells out everything it inherits (`en-AU` carries all of
`en`'s fields); no default-content locale (`en-US`, `ar-001`, …) has its own
files, so truncation reaches the data that stands for it; root is `und` and
has only `latn`. `locale-data` checks all of it: a child lacking a field its
parent has, a default-content locale with files, `availableLocales` differing
from the files — each fails the extraction.

**Number table** (`data/numbers.txt`, 93.2 KB, 1,812 lines; format in
`src/number/table.rs`): the digits of the 77 numeric numbering systems other
than `latn` (from the vendored `numberingSystems.json`); the languages' likely
scripts and the 37 regions that imply another script with a locale behind it
(`zh-TW` → `Hant`, from the vendored `likelySubtags.json`); CLDR's 199
explicit parents; then one `locale` line per CLDR locale (default and native
numbering system, minimum grouping digits) and `system` lines with its
symbols (decimal, group, minus, plus, percent) and patterns (decimal,
percent, the six currency patterns, the `currencyDisplay=name` patterns)
per numbering system — each only where
it differs from its CLDR parent's, so `en-AU` is one line and root carries
the full set. Values escape invisible characters (`\u{202f}`, `\u{200e}`) so
review sees them. Writing the table re-resolves every locale through the
table's own parent chain and requires CLDR's record exactly. The CLDR
patterns are parsed (UTS #35 §3.2) into the `number.patterns` records; the
extractor also checks what the entry formats rest on — every locale's systems
are `latn`, its default and its native one; its traditional and finance
systems are algorithmic; digit sets have one UTF-8 width; currency spacing is
the same everywhere (02 §4.2–§4.3).

**Currency and unit tables** (Phase 4, A4; `data/currencies.txt` 3.1 MB,
36,814 lines, 0.65 MB gz; `data/units.txt` 4.2 MB, 60,021 lines, 0.63 MB gz;
formats in `src/currency.rs`, `src/unit.rs`), from every locale's
`currencies.json` / `units.json` in the cache and the vendored
`currencyData.json`: CLDR's fraction digits and rounding per code; per
locale and currency the symbol, narrow symbol, the currency's own pattern
and separators, display name and its plural forms; the unit-identifier map
(232 identifiers, CLDR's keys without their category, checked unique); per
locale and width the `per` compound pattern, and per unit the display name,
per-unit pattern and plural patterns (case and gender forms left out: MF2 has
no case option). The lines are TAB-separated so that ordinary spaces stay
readable, and a locale stores a field only where it differs from what its
parents (those of `numbers.txt`) and CLDR's own fallback give — a plural form
equal to `other`, a `name-other` equal to the name, a narrow symbol equal to
the symbol cost nothing — a third and a quarter smaller than a parent-only
deduplication (4.8 → 3.1 MB and 5.8 → 4.2 MB, the TABs included). As for numbers,
writing re-resolves every locale through the text and requires CLDR's
record, every field of every key, exactly; the extractor also checks that
every field is one it knows (a new CLDR field fails the extraction), that
every pattern and template parses, and that the records are resolved. A
lookup indexes the table once by locale and parses only the blocks of the
locale's chain, cached per process. `cargo xtask locale-data` takes 35 s
(debug build) at a 259 MB peak: the records are interned and read one file
at a time.

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
parse + `writer::single`) for ad-hoc formatting on servers and in tests. Created
in Phase 3 (runtime + features), extended in P4 (function features), P5b and P6
(Leptos and Axum re-exports).

Beyond the re-exports it carries exactly one thing of its own, added in P5b:
the **call-site core** every `tr!` expansion goes through
([04](04-leptos-integration.md) §2.1) — `Tr`, `TrArgs`, `TrRich`, their
constructors `tr` / `tr_args1`…`tr_args4` / `tr_args_n` / `tr_rich`,
`ArgValue` with a `From` for every `Arg` variant and the two extension traits
`ArgSource` and `MarkupHandler`, and the lowering that borrows an
`&[ArgValue]` into the runtime's `&[Arg<'a>]`. It is Leptos-free, so a
server, a test and `mf2-cli` use it with no Leptos in the tree, and it is
**client-path code** (`no_std`, `forbid(unsafe_code)`, no `core::fmt`, no
panicking operation — the discipline of `mf2-runtime`), because it is what
2,000 call sites of a wasm build are made of (B5).

Also P5b: `__mf2` is the path the generated module re-exports the facade
under, `mf2::include_generated!()` includes what `mf2-build` wrote
(`$OUT_DIR/mf2_generated.rs`; `include_generated!(catalogs)` for the
catalog-only crate of `Emit::Catalogs`), and `mf2::__tr_impl` re-exports
`mf2-macros`' proc-macro so that the generated `tr!` wrapper reaches it
through `__mf2` alone.
