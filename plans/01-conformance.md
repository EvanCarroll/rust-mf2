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
| License | **Split since #1112 (2026-05-26, inside the pin).** The specification text (UTS #35 Part 9) is under the Unicode Terms of Use: copies for personal or internal business use only, **no public distribution** without Unicode's written permission. All other content — the test suite — is Unicode License v3. Vendoring `test/` is fine; vendoring `spec/` in a public repository is not, as the license now reads. **Decided (owner, 2026-09-25): `spec/` leaves the tree** and `spec-sync` fetches it on demand into a git-ignored cache; `test/` and `LICENSE` stay vendored. The old commits still hold the text; rewriting them is the owner's call, needed only if that history is made public ([17](17-phase-9-work-order.md) A0). *(Was open: out of the tree, seek permission, or keep the repository private.)* |

The spec is effectively frozen (three small commits in eight months), so the pin
is cheap to keep current.

**Vendoring.** `third_party/message-format-wg/` holds a verbatim copy of upstream
`test/` and `LICENSE`, plus a `PIN` file (sha, date, URL, and the SHA-256 of
every `spec/` file). It is vendored, not a submodule, so the suite works
offline. The directory is read-only: nothing under it is ever hand-edited.

The **spec text** (~4.6k lines of Markdown, a 121-line ABNF, the data-model
JSON Schema) is not vendored (License, above; Phase 9 A0). `cargo xtask
spec-sync` fetches it at the pin into `target/xtask-cache/message-format-wg-spec/`
(git-ignored), refusing any file whose digest differs from the `PIN`'s, and
stamps the cache with the commit. Everything that reads it — the ABNF-driven
generators, the L2 schema check, the coverage matrix — goes through
`conformance/src/spec.rs`, which fails naming the command when the cache is
missing or from another pin; nothing skips. Every CI job runs
`cargo xtask spec-sync --check` first. Nothing committed quotes the text: the
coverage matrix's `says` fields are paraphrases.

`cargo xtask spec-sync [--rev <sha>]` refreshes both and prints the added, removed
and changed tests; the ledger (§4) MUST be updated in the same commit. A new
`--rev` re-records the digests.

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
| **L2 data model** | `mf2-syntax` → `mf2-model` | Lower to the spec's interchange data model; run validation. The set of Data Model Errors equals the data-model subset of `expErrors`. | Model → JSON validates against the pinned `spec/data-model/message.json` (from the cache). `parse(serialize(m)) == m`. `serialize(m)` is itself L1-clean. |
| **L3 catalog** | `mf2-catalog` | Compile every L2-clean message into a binary catalog, decode it, and rebuild a data model. It MUST equal the L2 model — **the binary format is lossless over the full data model**, which is what proves it is not a subset. | Decoder never panics or reads out of bounds on truncated / bit-flipped input (fuzzed). Reader is `forbid(unsafe_code)`, without exception ([02](02-catalog-format.md) F5). |
| **L4 runtime** | `mf2-runtime` + function crates | Compile `src` to a one-message catalog, format it **from the catalog** with `params`, `locale`, `bidiIsolation`. Assert `exp`, `expParts`, `expErrors` (absent/empty ⇒ no errors allowed). | Runs on **native and wasm** with byte-identical results. The wasm run is `wasm32-wasip1` under wasmtime: suite catalogs are compiled natively beforehand and embedded, so the wasm executes only the client path (reader + runtime + functions) with a pure-Rust `Host`. (The browser target, `wasm32-unknown-unknown` with `mf2-host-web`, is exercised by L6 — and, for the `intl` build, by `cargo xtask l4-web` in three engines, below.) There is no "format from AST" code path anywhere — the shipped path is the only path, so it is the tested path. |
| **L5 macros** | `mf2-build` + `mf2-macros` | The suite becomes generated i18n crates — one per locale the suite uses (`en-US`, `und`, `fr`, `ar`), each single-locale, each built from the vendored suite by its own `build.rs` so no copy of it exists to drift (`conformance/l5/`). Message id = `suite.<file>.t<index>` (e.g. `suite.syntax.t078`; the file's path, dotted). Each test → a `#[test]` that calls `tr!` with `params` and asserts **the same `exp` / `expParts` / `expErrors` as L4**. For syntax- and data-model-error tests the assertion is that `mf2-build` rejects the message with the expected kind. | Proves compile-time variable extraction and slot lowering for every syntax form (`.input`, `.local` shadowing, variables used only in options / selectors / markup options). `trybuild` compile-fail cases: unknown id, missing arg, extra arg, unknown markup handler. Tests whose `params` deliberately mismatch the message (e.g. `unresolved-variable`) go through the dynamic named-args API and are marked `dyn` in the ledger. |
| **L6 Leptos** | `leptos-mf2` | (a) **SSR**: render each message through the view type to HTML; text content equals `exp`; markup is rendered with the **flat recorder handler** ([04](04-leptos-integration.md) §7), one empty marker element per markup part, whose order, kind, name and options MUST equal `expParts` (the suite contains unpaired markup, so nesting cannot be the assertion). (b) **Hydrate**: a generated page holding every runtime-valid suite message is server-rendered, hydrated in a headless browser, and MUST log zero hydration warnings with identical `innerText` before/after. Then the page switches to a **twin locale** (identical messages under another tag), which forces every node to be re-formatted *in the browser*; `innerText` MUST still equal `exp`; then back. Per-test `bidiIsolation` is applied through the provider's bidi-strategy setting. | SSR+hydrate in P6. |
| **L7 delivery modes** | `leptos-mf2` | L6(b)'s page and twin switch, delivered two other ways: (i) an **islands** page whose interactive parts are islands behind the islands gate (columns `L7` / `L7d`); (ii) a **client-only** page mounted by `mount_to_body` from a published index (columns `L7c` / `L7cd`). Each mode has columns of its own so that the report shows each one tested (owner, 2026-09-23). | P7. |

Layers map to phases — a phase cannot exit until its layer is green, and a later
layer never excuses an earlier one:

| Layer | Turns green at the exit of |
|---|---|
| L1, L2 | P1 |
| L3 | P2 |
| L4, every file except `functions/{percent,currency,date,time,datetime}.json` | P3 (core numeric semantics are core — [03](03-runtime.md) §5.1 — so `number`, `integer`, `offset` and the `:number` uses inside `bidi`/`fallback` belong here) — except `syntax.json` #90 |
| L4, the remaining function files, and `syntax.json` #90; all default-features columns for L4 | P4 |

`syntax.json` #90 (`{$one} et {$two}` in `fr`, 1.3 and 4.2) expects the
French decimal comma on unannotated floats — locale number symbols, which are
Phase 4's (`number.symbols`, `fn-number`); core output is neutral (`1.3 et
4.2`). The owner moved that one test to P4 (2026-09-21); the ledger checker
knows it by its key (`conformance/src/matrix.rs`, `L4_TESTS_AT_P4`).
| L5 (both configurations) | P5b |
| L6 (both configurations), SSR + hydrate | P6 |
| L7 (both configurations), islands and client-only | P7 |

**Two configurations.** Locale-aware number and date formatting on the client
are opt-in cargo features ([03-runtime](03-runtime.md) §5.1). L4–L7 therefore
run twice: with **all features on** — this is the full-MF2 claim and every test
MUST pass — and with **default features**, where the ledger carries second
status columns (`L4d`, `L5d`, `L6d`, `L7d`, `L7cd`) recording the documented degradations test
by test. At L4 the default configuration is simply a registry built without the
gated handlers, so a gated function shows up as `unknown-function` and a
locale-dependent numeric option as `unsupported-operation`; the L4d harness
(Phase 4, A7: `mf2_l4_runner::DEFAULT_REGISTRY`,
`mf2_conformance::l4::check_default`) formats each test in that
configuration and classifies the run — it passes, or it degrades as
`unknown-function` (the message names a gated function and the run reports
the error), `unsupported-operation` (the only other error difference), or
`neutral-numbers` (the expected errors, a different text, and the test passes
at L4) — and the cell MUST say exactly that: a `degraded` cell whose run
passes or degrades otherwise is red, and so is an `xfail` whose run degrades
as documented (`--promote` records it); at L5 the same tests
assert that **`mf2-build` rejects the corpus** (gated function) or warns (neutral
numbers). A degradation that is not written down is a failure.

**The `intl` build** (owner decision 4; [03](03-runtime.md) §5.3). The
`intl` client option is a third configuration, and only in a browser: on
`wasm32-unknown-unknown` the numeric functions take their display, their
rounding and the plural category from the engine's `Intl.NumberFormat` and
`Intl.PluralRules`, so its conformance is per engine. `cargo xtask l4-web`
runs it: every case `l4-wasi` runs — the suite in both configurations,
unstripped and stripped, and the locale-output goldens (§5) — compiled
natively into a bundle, formatted by `conformance/l4-web` (the L4 runner
built for `wasm32-unknown-unknown` with the `intl` features and
`mf2-host-web`'s `NUMBERS_HOST`) in Chromium, Firefox and WebKit through
`tools/e2e/checks/l4-intl.mjs`, and judged natively: each suite test against
its expectations exactly as L4 judges the native run (the stripped catalog
formats as the unstripped one), and each record against the native one —
the Rust path. Every difference is classified — `space` (only space
characters differ), `digits` (other digits: rounding, precision, numbering
system), `symbols` (the same digits with other signs, separators,
currency or unit text), `plural` (another variant: another plural
category), `errors`, `parts` (the same text in other sub-parts) — and MUST
be the ledger's (§4, `intl`): an unrecorded difference fails the run, and
so does a recorded one that no longer occurs. The L4 columns stay the Rust
path's; the engine runs are not in CI (they need the three browsers) but
are part of Phase 4's exit and of any change to the option.

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
L4d = { status = "degraded", kind = "unsupported-operation", detail = "useGrouping=always needs fn-number" }
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

**The `intl` build's differences** (§3) are recorded where they occur, not
as statuses: a `[[test]]` entry MAY carry `intl = [{ engines, kind, fails,
detail }, …]` — the engines it occurs in (`chromium`, `firefox`, `webkit`,
in that order), its kind (`space` | `digits` | `symbols` | `plural` |
`errors` | `parts`), whether the test's expectations then fail there
(`fails = true`; absent = they pass) and what differs and why; each engine
at most once per test, and only on a test that runs at L4. The goldens are
not suite tests, so their differences are top-level `[[intl]]` tables:
`cases = "golden/<family>/<locale>"`, `engines`, `kind`, `count` (how many
of the group's cases differ so, in each of those engines), `detail`.
`cargo xtask conformance-report` checks their form; `cargo xtask l4-web`
checks, in the engines, that they are exactly what occurs.

```toml
[[intl]]
cases   = "golden/units/cy"
engines = ["chromium"]
kind    = "symbols"
count   = 36
detail  = "Chromium 143 has no Welsh unit names and writes the English ones: …"
```

**Statuses**: `pass`; `xfail` (known failure; `until` names the phase that fixes
it); `degraded` (default-features columns only: `kind` is `unsupported-operation`,
`unknown-function`, `build-reject` or `neutral-numbers` — the last added in
Phase 4 for numbers written in the core's neutral digits, `syntax.json` #90); `skip` (the reason is a fact about the
*test*, e.g. surrogates); `n/a` (the layer cannot apply). An L5 entry MAY carry
`via = "dyn"` when the test's `params` deliberately mismatch the message and it
runs through the named-args API; `dyn` is a mode, not a status.

**What "pass" means for error tests, and where `n/a` is correct:**

| Test kind | L1 | L2 | L3 | L4 | L5 | L6 | L7 |
|---|---|---|---|---|---|---|---|
| expects `syntax-error` | error reported | `n/a` | `n/a` | one-message compile rejects with that kind | `mf2-build` rejects with that kind | `n/a` | `n/a` |
| expects a data-model error | parses clean | error(s) reported | `n/a` | compile rejects with that kind | `mf2-build` rejects with that kind | `n/a` | `n/a` |
| everything else | parses clean | validates clean | lossless | output/parts/errors match | same, through `tr!` | same, through Leptos | L6 passes, and every engine agrees with the server through the delivery mode and its twin switch |

L7's four columns (`L7`, `L7c`, `L7d`, `L7cd`) have no harness in `cargo
test`: only an engine can run them. `cargo xtask l7-web` is their harness
(Phase 7, A4) — it renders one page per locale the suite uses, per delivery
mode and configuration, drives each in every engine, judges every cell from
what the engines recorded and from L6/L6d's verdict on the test, and holds
the ledger to the result with the same rules 2 and 3 below (`--promote`
tightens it). A default-configuration page holds only what the default build
accepts, so a test L6d records as `build-reject` is `degraded` the same way
at L7d and L7cd.

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
7. a cell says `pass` or `degraded` for a layer that has no harness yet (a
   harness in the engines, like L7's, counts; `conformance-report` then checks
   the cell's form and `cargo xtask l7-web` its truth);
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

  *As built (Phase 7 A12).* A **normative statement** is a sentence of the
  pinned `spec/**/*.md` that uses a BCP 14 key word in capitals (the eleven
  of `intro.md`) —
  fenced code skipped, a quoted mention of a key word excepted — so the data
  model and the function documents are in, with the others: 164 at the pin.
  Its id is `<file>#<first 4 bytes of the SHA-256 of the whitespace-collapsed
  sentence, hex>` (`-1`, `-2`, … on a repeat within a file), stable across a
  `spec-sync` while the sentence is unchanged; the exact rules are
  `conformance/src/coverage.rs`' module doc. The matrix is `conformance/coverage.toml`: one entry per id,
  with `says` (a paraphrase — the spec text is not quoted) and either `tests`
  (suite keys `file@hash[#nth]`, or `path::fn` for a Rust test, or a path for
  a browser check) or `na = { kind, reason }`, where `kind = "permission"` is
  allowed only on a MAY. `conformance/src/coverage.rs` checks it — a missing,
  stale, duplicate, undecided, not-a-permission, empty or dangling entry is a
  gap — and renders `conformance/COVERAGE.md`; `cargo xtask
  conformance-report` fails on any gap and rewrites the file. The tests of
  `conformance/tests/coverage.rs` hold the matrix complete, `COVERAGE.md`
  current, the sentence splitter lossless (no key word outside an extracted
  statement) and each kind of gap detected.
* **Grammar-driven generation** — a generator derived from the vendored
  `message.abnf` emits random well-formed messages. Properties: L1 lossless CST,
  L2 round trip, L3 lossless, L4 never panics and is deterministic across native
  and wasm.
* **Fuzzing** — `cargo fuzz` targets for the parser and the catalog decoder: no
  panic, no OOB, linear time (MF2 has no recursive productions; the parser MUST
  NOT recurse on input). "Linear time" is asserted as a budget of **CPU time**
  per input byte, not wall clock (`fuzz/common/budget.rs`): the claim is about
  the work the code does, and a shared clock measures the machine too — on a
  loaded desktop that fails runs the code would pass. libFuzzer's `-timeout`
  stays wall clock, as the guard against blocking rather than spinning.
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
