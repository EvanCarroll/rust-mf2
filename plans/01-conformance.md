# 01 — Conformance: the WG test suite through the whole stack

Part of the [master plan](00-master-plan.md). RFC 2119 keywords apply.

**Principle.** mf2-two implements *all* of Unicode MessageFormat 2 (MF2), never a
subset. The proof is the official test suite from
`unicode-org/message-format-wg`, and that proof MUST be re-established at every
layer of the stack — it is not enough that a parser somewhere passes it. A
feature is "supported" only when the suite passes at the *outermost* layer that
exists at that point in the project.

---

## 1. The pinned upstream

| | |
|---|---|
| Upstream | `https://github.com/unicode-org/message-format-wg` |
| Pin | `5c4ddb27e726fd7881c1787a632efba83ab0d850` (2026-08-31) |
| Relation to releases | `LDML48.2` (2026-01-08) + 7 commits: three change the spec or tests (well-formedness of timezone/calendar/unit values #1104, a `:test:function` erratum #1113, one new `:string` test #1114), one replaces `LICENSE` (#1112), three add meeting notes |
| Release history | `LDML47-Stable` 2025-02-26 · `LDML48` 2025-08-11 · `LDML48.2` 2026-01-08 |
| License | **Split since #1112 (2026-05-26, inside the pin).** The specification text (UTS #35 Part 9) is under the Unicode Terms of Use: copies for personal or internal business use only, **no public distribution** without Unicode's written permission. All other content — the test suite — is Unicode License v3. Vendoring `test/` is fine; vendoring `spec/` in a public repository is not, as the license now reads. **Open owner decision** before any public remote: keep `spec/` out of the tree (fetched by `spec-sync` into a git-ignored cache), seek permission, or keep it only while the repository is private. |

The spec is effectively frozen (three small commits in eight months), so the pin
is cheap to keep current.

**Vendoring.** `third_party/message-format-wg/` holds a verbatim copy of upstream
`spec/`, `test/` and `LICENSE`, plus a `PIN` file (sha, date, URL). It is
vendored, not a submodule, so every agent and every CI job works offline and the
spec text itself (~4.6k lines of Markdown, a 121-line ABNF, the data-model JSON
Schema) is greppable inside the repo. The directory is read-only: nothing under
it is ever hand-edited.

`cargo xtask spec-sync [--rev <sha>]` refreshes it and prints the added, removed
and changed tests; the ledger (§4) MUST be updated in the same commit.

## 2. What the suite contains (at the pin)

462 tests in 16 files. Schema: `test/schemas/v0/tests.schema.json`.

| File | Tests | Exercises |
|---|---:|---|
| `syntax.json` | 114 | every ABNF production; whitespace, escapes, literals, markup, attributes |
| `syntax-errors.json` | 133 | strings that MUST yield a Syntax Error |
| `data-model-errors.json` | 23 | the six Data Model Errors |
| `pattern-selection.json` | 22 | `.match` resolution, ordering, multi-selector, selector failure |
| `fallback.json` | 8 | fallback values for unresolved / failed expressions |
| `bidi.json` | 27 | bidi in syntax + the Default Bidi Strategy |
| `u-options.json` | 10 | `u:id` and `u:dir` (the only `u:` options defined at this pin), including *Bad Option* for `u:dir` on markup. (Unknown `u:` **functions** are tested in `syntax.json`.) |
| `functions/string.json` | 10 | `:string` format + select (NFC) |
| `functions/number.json` | 41 | `:number` options, selection, operand/option errors, option inheritance |
| `functions/integer.json` | 13 | `:integer` |
| `functions/offset.json` | 16 | `:offset` |
| `functions/percent.json` | 13 | `:percent` (tag `:percent`) |
| `functions/currency.json` | 12 | `:currency` (tag `:currency`) |
| `functions/datetime.json` | 7 | `:datetime` (Draft) |
| `functions/date.json` | 7 | `:date` (Draft) |
| `functions/time.json` | 6 | `:time` (Draft) |

Assertion kinds: 259 tests assert `exp` (string), 20 assert `expParts`
(text / string / number / markup / fallback / bidiIsolation parts), 253 assert a
non-empty `expErrors`. Per-test inputs: `locale`, `bidiIsolation`
(`default` | `none`), `params` (string, integer, float, bool, and typed
`datetime`), optional `tags` (`:currency`, `:percent`, `u:dir`, `u:id`).

Facts that shape the design:

* **Syntax and data-model error tests never assert output** (0 of 133 and 0 of
  22). An implementation is therefore conformant if it reports those errors at
  *catalog-compile time* and refuses to ship the message. The harness maps
  "compile error of kind X" onto `expErrors: [X]`.
* **Resolution and function errors do assert output** (fallback, selection,
  number…). Error *collection alongside a best-effort result* is a runtime
  feature, not an optional extra: `format` returns text **and** a list of errors.
* **`expParts` and markup are in the suite.** Format-to-parts and markup are part
  of "full MF2"; the Leptos layer builds on exactly this API (markup → elements).
* **`:test:function`, `:test:select`, `:test:format`** are specified in
  `test/README.md`. They MUST be implemented in the harness *through the public
  custom-function API* and nowhere else. That doubles as the proof that the
  public API is expressive enough: selector + formatter roles, resolved values
  that carry options into later expressions, selective failure, multi-part
  output.
* **Unpaired surrogates** cannot occur in JSON or in a Rust `&str`; the README's
  note on them is N/A by construction. The ledger records that once.
* The function tests mostly assert errors, not locale output, because formatted
  output is implementation-defined. Output correctness for numbers and dates
  needs our own golden tests (§5).

## 3. The layers

Each layer is a test target in the `mf2-conformance` crate (`conformance/`). Each
consumes the *same* vendored JSON; no layer has a private copy or a filtered
list. "All `src`" below means every test in all 16 files, not only the syntax
files — every message in the suite is a parser test, a data-model test and a
catalog test, whatever file it came from.

| Layer | Crate under test | For each test… | Extra invariants |
|---|---|---|---|
| **L1 syntax** | `mf2-syntax` | Parse `src`. A Syntax Error is reported **iff** `expErrors` contains `syntax-error`. | CST is lossless: `cst.to_string() == src` byte-for-byte. Never panics. Every diagnostic span is in bounds and on a char boundary. |
| **L2 data model** | `mf2-syntax` → `mf2-model` | Lower to the spec's interchange data model; run validation. The set of Data Model Errors equals the data-model subset of `expErrors`. | Model → JSON validates against the vendored `spec/data-model/message.json`. `parse(serialize(m)) == m`. `serialize(m)` is itself L1-clean. |
| **L3 catalog** | `mf2-catalog` | Compile every L2-clean message into a binary catalog, decode it, and rebuild a data model. It MUST equal the L2 model — **the binary format is lossless over the full data model**, which is what proves it is not a subset. | Decoder never panics or reads out of bounds on truncated / bit-flipped input (fuzzed). Reader is `forbid(unsafe_code)`, without exception ([02](02-catalog-format.md) F5). |
| **L4 runtime** | `mf2-runtime` + function crates | Compile `src` to a one-message catalog, format it **from the catalog** with `params`, `locale`, `bidiIsolation`. Assert `exp`, `expParts`, `expErrors` (absent/empty ⇒ no errors allowed). | Runs on **native and wasm** with byte-identical results. The wasm run is `wasm32-wasip1` under wasmtime: suite catalogs are compiled natively beforehand and embedded, so the wasm executes only the client path (reader + runtime + functions) with a pure-Rust `Host`. (The browser target, `wasm32-unknown-unknown` with `mf2-host-web`, is exercised by L6.) There is no "format from AST" code path anywhere — the shipped path is the only path, so it is the tested path. |
| **L5 macros** | `mf2-build` + `mf2-macros` | The suite becomes generated i18n crates — one per locale the suite uses (`en-US`, `und`, `fr`, `ar`), each single-locale. Message id = `suite.<file-stem>.t<index>` (e.g. `suite.syntax.t078`). Each test → a `#[test]` that calls `tr!` with `params` and asserts **the same `exp` / `expParts` / `expErrors` as L4**. For syntax- and data-model-error tests the assertion is that `mf2-build` rejects the message with the expected kind. | Proves compile-time variable extraction and slot lowering for every syntax form (`.input`, `.local` shadowing, variables used only in options / selectors / markup options). `trybuild` compile-fail cases: unknown id, missing arg, extra arg, unknown markup handler. Tests whose `params` deliberately mismatch the message (e.g. `unresolved-variable`) go through the dynamic named-args API and are marked `dyn` in the ledger. |
| **L6 Leptos** | `leptos-mf2` | (a) **SSR**: render each message through the view type to HTML; text content equals `exp`; markup is rendered with the **flat recorder handler** ([04](04-leptos-integration.md) §7), one empty marker element per markup part, whose order, kind, name and options MUST equal `expParts` (the suite contains unpaired markup, so nesting cannot be the assertion). (b) **Hydrate**: a generated page holding every runtime-valid suite message is server-rendered, hydrated in a headless browser, and MUST log zero hydration warnings with identical `innerText` before/after. Then the page switches to a **twin locale** (identical messages under another tag), which forces every node to be re-formatted *in the browser*; `innerText` MUST still equal `exp`; then back. Per-test `bidiIsolation` is applied through the provider's bidi-strategy setting. | SSR+hydrate in P6; CSR and islands in P7. |

Layers map to phases — a phase cannot exit until its layer is green, and a later
layer never excuses an earlier one:

| Layer | Turns green at the exit of |
|---|---|
| L1, L2 | P1 |
| L3 | P2 |
| L4, every file except `functions/{percent,currency,date,time,datetime}.json` | P3 (core numeric semantics are core — [03](03-runtime.md) §5.1 — so `number`, `integer`, `offset` and the `:number` uses inside `bidi`/`fallback` belong here) |
| L4, the remaining function files; all default-features columns for L4 | P4 |
| L5 (both configurations) | P5b |
| L6 (both configurations), SSR + hydrate | P6; other delivery modes P7 |

**Two configurations.** Locale-aware number and date formatting on the client
are opt-in cargo features ([03-runtime](03-runtime.md) §5.1). L4–L6 therefore
run twice: with **all features on** — this is the full-MF2 claim and every test
MUST pass — and with **default features**, where the ledger carries second
status columns (`L4d`, `L5d`, `L6d`) recording the documented degradations test
by test. At L4 the default configuration is simply a registry built without the
gated handlers, so a gated function shows up as `unknown-function` and a
locale-dependent numeric option as `unsupported-operation`; at L5 the same tests
assert that **`mf2-build` rejects the corpus** (gated function) or warns (neutral
numbers). A degradation that is not written down is a failure.

## 4. The ledger — no silent skips

`conformance/ledger.toml` starts with `current_phase = "P0"` and has one entry
per test:

```toml
current_phase = "P0"

[[test]]
file  = "functions/number.json"
index = 17                  # position in the file at the pin (a hint, not the key)
hash  = "9c1e44aa"          # sha256(src ‖ params ‖ locale ‖ bidiIsolation)[..8]
nth   = 0                   # occurrence ordinal among tests with this hash in this file
L1 = "pass"
L2 = "pass"
L3 = "pass"
L4 = { status = "xfail", reason = "roundingIncrement not implemented", until = "P3" }
L5 = { status = "xfail", until = "P5b" }
L6 = { status = "xfail", until = "P6" }
L4d = { status = "degraded", kind = "unsupported-operation", detail = "numberingSystem needs fn-number" }
L5d = { status = "xfail", until = "P5b" }
L6d = { status = "xfail", until = "P6" }
```

**Key** = (`file`, `hash`, `nth`). The hash alone is not unique — the suite
contains byte-identical tests (`syntax.json` #78≡#79, #95≡#98) — hence `nth`.
`index` (0-based) only helps humans and survives as a hint across re-syncs.

The hash, byte for byte (normative text and golden values in
`conformance/src/key.rs`): the first 4 bytes of SHA-256, as 8 lowercase hex
digits, over `field(src) ‖ field(params) ‖ field(locale) ‖ field(bidiIsolation)`,
taken **after** `defaultTestProperties` are applied, where `field(absent) =
0x00` and `field(b) = 0x01 ‖ len(b) as u64 big-endian ‖ b`. An absent
`bidiIsolation` stays absent (not replaced by the spec default). `params` is its
canonical JSON: no insignificant whitespace, object members sorted by UTF-8 key
bytes, minimal string escapes (other code points written as themselves),
integers as written, other numbers as the shortest round-trip double that
always carries `.` or `e` (so `1` and `1.0` differ). `nth` is the 0-based
occurrence ordinal among tests of the same file with the same hash, in file
order.

Ledger-wide notes that are facts about the suite, not about one test (at P0,
only the unpaired-surrogates N/A of §2), are `[[note]]` tables with `id`,
`status` and `reason`. A note with `status = "open"` is an obligation not yet
testable and MUST carry `until` (the phase that discharges it); only an open
note may. Since P2: `stripped-formats-identically`, `until = "P3"` —
stripped and unstripped catalogs format identically, checked once L4 exists
(L3 already checks they decode to the same formatting-relevant model).

**Statuses**: `pass`; `xfail` (known failure; `until` names the phase that fixes
it); `degraded` (default-features columns only: `kind` is `unsupported-operation`,
`unknown-function` or `build-reject`); `skip` (the reason is a fact about the
*test*, e.g. surrogates); `n/a` (the layer cannot apply). An L5 entry MAY carry
`via = "dyn"` when the test's `params` deliberately mismatch the message and it
runs through the named-args API; `dyn` is a mode, not a status.

**What "pass" means for error tests, and where `n/a` is correct:**

| Test kind | L1 | L2 | L3 | L4 | L5 | L6 |
|---|---|---|---|---|---|---|
| expects `syntax-error` | error reported | `n/a` | `n/a` | one-message compile rejects with that kind | `mf2-build` rejects with that kind | `n/a` |
| expects a data-model error | parses clean | error(s) reported | `n/a` | compile rejects with that kind | `mf2-build` rejects with that kind | `n/a` |
| everything else | parses clean | validates clean | lossless | output/parts/errors match | same, through `tr!` | same, through Leptos |

`cargo xtask conformance-report --init` generates exactly this matrix with every
applicable cell `xfail` and `until` taken from the layer → phase table in §3.

The harness fails when:

1. a test in the vendored suite has no ledger entry, or an entry has no test;
2. a `pass` fails;
3. an `xfail` or `skip` unexpectedly passes — the ledger must be tightened (a
   ratchet: the pass count only goes up);
4. any `xfail`, or `open` note, remains whose `until` is `current_phase` or
   earlier — the first commit of a phase's exit bumps `current_phase`, and must
   leave the harness green;
5. a test is `skip`ped by tag. **Tags are not a skip reason.** `:currency`,
   `:percent`, `u:dir`, `u:id` mark optional spec features; mf2-two implements
   all of them;
6. an `until` is later than its layer's deadline in §3 (a ledger cannot
   quietly postpone a layer);
7. a cell says `pass` or `degraded` for a layer that has no harness yet;
8. the `n/a` pattern of the table above is violated in either direction, a
   column is missing, `via` appears outside L5/L5d, or `degraded` outside the
   `d` columns.

`cargo xtask conformance-report` writes `conformance/REPORT.md` (a layer × file
matrix of pass / xfail / degraded / skip / n/a counts) and CI publishes it as
an artifact. `--init` refuses to overwrite an existing ledger without
`--force`; `--ledger` / `--report` take other paths (used by the mutation
tests).

## 5. Beyond the suite

462 tests are necessary, not sufficient.

* **Spec coverage matrix** — `conformance/COVERAGE.md` maps every normative
  statement (MUST / MUST NOT / SHOULD) in `syntax.md`, `formatting.md`,
  `errors.md`, `u-namespace.md` and `functions/*.md` to the test(s) that cover
  it. Gaps get tests under `conformance/extra/` **written in the WG schema**, so
  they run through the same six layers and can be offered upstream.
* **Grammar-driven generation** — a generator derived from the vendored
  `message.abnf` emits random well-formed messages. Properties: L1 lossless CST,
  L2 round trip, L3 lossless, L4 never panics and is deterministic across native
  and wasm.
* **Fuzzing** — `cargo fuzz` targets for the parser and the catalog decoder: no
  panic, no OOB, linear time (MF2 has no recursive productions; the parser MUST
  NOT recurse on input).
* **Locale-output goldens** — the suite leaves number/date output
  implementation-defined. `conformance/goldens/` pins our output for a locale
  panel chosen to stress the data: `en`, `es`, `de`, `fr`, `ar` (RTL, six plural
  categories, non-Latin digits), `he`, `ja`, `hi`, `ru`/`pl` (few/many), `cy`.
  Goldens are generated once from the reference backend, reviewed, committed, and
  compared on both targets.
* **Resource container** — the W3C Message Resource draft that holds our source
  files ([05-tooling](05-tooling.md) §2) has an ABNF and a JSON Schema but no test
  suite. `mf2-resource` gets ABNF-driven generation, parse → serialize → parse
  round trips, and schema validation of its JSON form; and every suite `src` is
  embedded as a resource entry and recovered byte-for-byte, proving the
  container never damages a message (escapes, leading whitespace, multi-line
  `.match` bodies).
* **Differential oracle (MAY, dev-only)** — `tools/oracle/` runs the reference JS
  implementation (`messageformat` v4, by the spec editor) over generated
  messages restricted to deterministic functions and diffs against L4. Never a
  build or runtime dependency.

## 6. Non-obvious spec obligations to keep visible

Easy to miss, all testable, all in scope:

* **NFC.** Variant keys are compared after NFC (`NormalizeKey`), duplicate-variant
  detection uses NFC, and `:string` selection compares the NFC form of the
  *runtime* operand. Keys are normalized at compile time. Runtime operands use a
  quick check (all code points < U+0300 ⇒ already NFC) and otherwise a **host
  normalizer**: `String.prototype.normalize` in the browser, a Rust normalizer on
  the server. No normalization tables in the wasm.
* **Default Bidi Strategy** MUST be the default; a no-op strategy MAY exist. It
  needs the message direction, so the catalog header carries the locale's `dir`.
* **Option inheritance**: a `:number` operand that is itself a resolved `:number`
  contributes its options, overridden by the expression's own.
* **`select` must be a literal** — a variable value is a Bad Option and the value
  loses selectability. Checked at compile time *and* enforced at runtime.
* **Exact numeric keys** match the serialized operand, ahead of plural category;
  plural category depends on the *formatted* number (`1.0` vs `1`), so plural
  operands are computed from the resolved formatting options.
* **Cardinal *and* ordinal** rules are required (`select=ordinal`).
* **Fallback representations** (`{$name}`, `{:fn}`, `{|lit|}`) need names at
  runtime — they live in the lazily loaded catalog, never in the wasm.
* **Function namespaces and custom functions** are first-class
  (`:ns:name`); unknown functions produce `unknown-function` plus a fallback, not
  a build break — unless the build-time lint is asked to make it one.
* **Attributes** (`@name`, `@name=literal`) have no runtime meaning but are part
  of the data model and MUST survive L2/L3 round trips.
* **Names compare under NFC too** (`syntax.md`, "Names and Identifiers";
  `syntax.json` #108–113): duplicate-declaration and duplicate-option checks,
  variable-slot names, the catalog's NAMES section and named-argument lookup all
  use the NFC form. The data model itself keeps every value **as written**;
  normalization happens where values are compared or encoded, never in lowering.
* **`u:dir` on markup is a *Bad Option*** and is ignored (`u-namespace.md`;
  `u-options.json`); `u:id` on markup is carried through to the part.
* **Markup need not be paired.** Fourteen runtime-valid suite tests have a lone
  open or close. Pairing is an application-level convention, linted, never
  required by the runtime.
