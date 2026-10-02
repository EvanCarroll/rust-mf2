# Phase 1 results

Part of the [master plan](00-master-plan.md) (§9, P1); the record behind the
exit checklist of the [Phase 1 work order](08-phase-1-work-order.md). Every
figure names the command that produced it. Measured 2026-09-21 on the Phase 0
machine (Intel Core i7-1165G7, 4 cores / 8 threads, Linux 6.12), toolchain
stable 1.98.1; the fuzz target on nightly 1.98.0 (2026-06-11) with cargo-fuzz
0.11.0.

## Summary

| Exit item | Verdict | Evidence |
|---|---|---|
| L1 and L2 100 % over all 462 `src` | **met** | L1 462/462, L2 326/326 (the other 136 are syntax-error tests: `n/a`); ledger promoted, harness green |
| lossless CST and model round trip on generated input | **met** | 1,000,000 ABNF-generated messages: well-formed, lossless, `parse(serialize(m)) == m`, deterministic validation |
| fuzz run clean (≥ 1 h) | **met** | 6,973,130 executions in 65 min on the final code, no crash or timeout (§Fuzzing) |
| **D1 gate holds on every row** | **met** | every row faster (3.2–15.9×), fewer allocations and bytes, 462/462 vs ox 460/462 — so D1 is settled: own `mf2-syntax`, no fallback |
| target ≥ 3× on the workload rows, 0 allocations for placeholder-free | **met** | workload rows 4.3–7.4×, placeholder-free rows 4.5–15.9×; placeholder-free model rows allocate nothing |
| `mf2-model`'s public types unchanged | **met, with additions** | every frozen name, field and signature as specified; the additions (methods and trait impls, which the work order allows) are listed in 08 §"Status at exit" |
| Phase 2 work order | **written** | [09-phase-2-work-order](09-phase-2-work-order.md) |

## What was built

| Crate / tool | Lines | Content |
|---|---:|---|
| `crates/mf2-model` | 2,242 | the frozen types; `serde` (JSON per `message.json`, via a borrowed intermediate value so duplicates and the schema's relaxations are handled in one place) and `suite-names` features; `no_std`, no default dependencies, builds for `wasm32-unknown-unknown` |
| `crates/mf2-syntax` | 3,604 | parser → flat pre-order arena (lossless CST with trivia, or trimmed for the model); lowering; the six Data Model Errors under NFC; serializer; analysis; 26 syntax + 7 data-model diagnostic codes (`code.rs`); `no_std`, `forbid(unsafe_code)` |
| `conformance` | +852 | L1 and L2 harnesses, the ledger held to their results (`harness::verify`, `promote`), the spec path helper, the ABNF reader and generator; the ox oracle, analysis, generated-input and layer tests; `examples/differential.rs` |
| `bench/parser-gate` | +1 adapter | `src/mf2_syntax.rs` |
| `fuzz/` | new | target `parse` (nightly, standalone workspace) |
| `xtask` | 2 commands | `conformance-report --promote`, `fuzz-seed` |

Tests: 149 in `cargo test --workspace`, 2,415 lines of integration tests
(`crates/*/tests`, `conformance/tests`).

## A1 — `mf2-model`

The public types match the frozen listing name for name and signature for
signature. Choices the listing left open:

* `Pattern` stores one part inline; its `PartialEq`/`Hash`/`Debug` are
  implemented over `parts()`, so two internal representations of the same
  parts are equal (derived impls would have compared representations).
* `ErrorKind::suite_name` names the two kinds outside the suite schema
  `"unsupported-operation"` and `"message-function-error"`, so
  `suite_name`/`from_suite_name` are a bijection over all 15.
* JSON: serializing two options (or attributes) of the same exact name is an
  error; names that are merely NFC-equal are distinct JSON keys and serialize
  (validation reports them). Deserializing keeps duplicate members, ignores
  unknown fields, accepts missing empty collections, rejects an input
  declaration whose name differs from its variable, and normalizes patterns as
  `Pattern::push` does.

A test with a counting allocator shows a placeholder-free message model built
without any allocation (`crates/mf2-model/tests/alloc.rs`).

## A2–A4 — parser, lowering, validation

* **One parser, two arenas.** `parse_cst` keeps every token; `parse_model`
  keeps only the tokens lowering needs (text, escapes, literal text, names,
  `*`) and lowers from the same nodes. The position advances only through the
  function that emits a token, so the tokens tile the source by construction.
* **Non-recursive, linear.** No function calls itself; depth is bounded by the
  grammar. Lookahead is limited to a run of whitespace/bidi marks, which is
  then consumed. Text and quoted-literal runs are scanned eight bytes at a
  time.
* **Recovery.** An unusable character in a placeholder skips to its `}` (or
  stops before a `{`); junk between declarations skips to the next `.` or
  `{{`; a diagnostic at the position of the previous one is not repeated. On
  the 133 syntax-error tests nearly every message gets exactly one diagnostic
  per real error.
* **Lowering** borrows every string from the source unless an escape makes the
  cooked value differ (a lone escape still borrows: its character is in the
  source). Names exclude the bidi marks around them; an identifier with marks
  around its `:` is the one other owned case. Every vector is allocated once,
  at its final size.
* **Validation** reports each error with a location in model terms, mapped to
  a source span through the arena; the same checks run on models built in
  code (`validate`, spans `None`).

Two spec readings the suite settles and the code follows: a variant whose keys
are all `*` is a fallback **whatever its key count** (`data-model-errors.json`
#0, #1 expect only `variant-key-mismatch`), and two variants with identical key
lists are duplicates whatever their count. `ox_mf2_parser` restricts both
checks to variants of the right arity — that is its two suite misses and 2,197
of the differential cases below.

**Found by the linear-time test: quadratic validation.** 50,000 options in one
function took 47.6 s — the pairwise duplicate-name check, and the same pattern
in the duplicate-declaration, duplicate-variant and selector-chain checks, in
span resolution, in analysis and in `mf2-model`'s JSON duplicate check. Every
check now compares pairwise (no allocation) up to 16 items and sorts or uses an
ordered set beyond (O(n log n)); span resolution caches its node lists. A unit
test runs both forms on each side of the threshold and requires identical
results; `crates/mf2-syntax/tests/linear.rs` parses 29 adversarial inputs of
~200 KB each (including 50,000 duplicate options and a 8,332-link chain of
locals) in 2.3 s in a debug build.

## A5 — serializer

`serialize(&Message) -> Result<String, Error>` — a departure from the work
order's `-> String`, because some models built in code have no MF2 syntax: a
U+0000 in text or a literal, an invalid name, an input declaration whose name
differs from its variable, a select message without selectors, variants or
keys. Canonical form: simple message when possible (quoted pattern if the text
would start with `.` after whitespace/bidi), one declaration and one variant
per line, single spaces inside expressions, literals unquoted when they can be.
A catch-all key's `value` (other formats only) is dropped: MF2 writes `*`.

Round trip, L1-clean output and (for messages without data-model errors) JSON
valid against `spec/data-model/message.json` (validated with the `jsonschema`
crate) hold for every applicable suite test: they are part of the L2 harness.

## A6 — analysis

`analyze(&Message) -> Analysis { externals, locals, markup, functions }`, each
name with its NFC form and first spelling; externals in ascending bytewise
order of the NFC form (the manifest's slot order). Scope follows the
declarations (a `.local` counts from after its declaration).

Agreement with the Phase 0 manifest code: the P0.7 probe's own functions
(`external_vars`, `markup_names`, `function_names`, and `Manifest::build`'s NFC
+ sort + dedup) were run once over `bench/corpora/workload-1600.json`; the
result is committed as `conformance/fixtures/workload-1600.p07-manifest.json`
(344 messages with variables, markup or functions), and
`conformance/tests/analysis.rs` requires `analyze` to agree on all 1,600
messages. It does.

## A7 — conformance L1 + L2

```sh
cargo xtask conformance-report --promote   # once: 788 cells xfail → pass
cargo xtask conformance-report             # L1 462/462, L2 326/326: green
```

`matrix::HARNESSED` is now `[L1, L2]`. The report runs both layers over the
suite; a `pass` that fails and an `xfail` that passes are both violations
(harness rules 2 and 3). L1 checks, per test: syntax error iff expected; CST
lossless; every node and diagnostic span in bounds and on char boundaries;
`parse_model` agrees and gives a model exactly when there is no syntax error.
L2 checks: the set of data-model errors equals the expected subset; every
diagnostic has an in-bounds span; `validate` on the model alone agrees;
`serialize` round-trips and is L1-clean; JSON validates (clean messages). A
panic in a layer is recorded as a failure.

**Differential oracle** (`conformance/tests/oracle.rs`): on all 462 tests ox
reports every expected error; its only extra reports are the two recorded
over-reports; `mf2-syntax` reports exactly the expected set everywhere.

## A8 — generated input and fuzzing

**Generator.** `conformance/src/abnf.rs` reads the vendored `message.abnf`
through the spec path helper (RFC 5234 + RFC 7405 `%s`) and derives random
strings from it, biased towards ASCII, the BMP and range ends. Its output is
deliberately hostile: bidi marks and U+3000 wherever `o`/`s` allow them,
astral and noncharacter-adjacent code points in names and text.

```sh
cargo test -p mf2-conformance --test generated                                    # 3,000 cases
MF2_GEN_CASES=1000000 cargo test --release -p mf2-conformance --test generated    # 39 s
```

1,000,000 messages: every one parses without a syntax error, the CST is
lossless, the model round-trips through the serializer, and validation is
deterministic and agrees with `parse_model`. The nightly workflow repeats the
long run.

**Differential against ox on generated and mutated input**
(`conformance/examples/differential.rs`; 30,000 generated messages, each also
mutated twice by random edits with syntax characters):

```sh
cargo build --release -p mf2-conformance --example differential
(ulimit -v 4000000; ./target/release/examples/differential 30000)
```

| Outcome (90,000 messages) | Count |
|---|---:|
| same report (syntax error, or the same Data Model errors) | 81,919 |
| ox fails internally ("semantic model violates parser-owned invariants") on a message `mf2-syntax` parses | 5,878 |
| ox's arity rule (fallback / duplicate checks only among variants of the right arity) | 2,197 |
| ox never returns (killed after 2 s) | 4 |
| ox accepts markup as a `.local` value (`.local $x = {#b} {{}}`), which the ABNF does not allow; `mf2-syntax` reports code 24 | 2 |
| **any other syntax disagreement** | **0** |
| **any other data-model disagreement** | **0** |

**An ox denial of service.** `ox_mf2_parser` 0.14.0-alpha.12 never returns on
a function whose name starts with a noncharacter: `{:\u{10FFFF}` (4 bytes; also
U+FDD0, U+FFFE, U+FFFF, U+1FFFE, anywhere in a message) — and in the hangs
above it then tries to allocate 1.9–3.7 GB. `mf2-syntax` reports
EXPECTED_NAME and an unterminated placeholder. This is why the differential
runs ox in a child process with a timeout and a memory limit, and why ox must
never parse untrusted input in this project (it is a dev-only oracle anyway).

### Fuzzing

```sh
cargo xtask fuzz-seed          # 2,062 seed files: the suite + the workload
cd fuzz
cargo +nightly fuzz run parse -- -dict=mf2.dict -max_len=16384 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900
```

The target checks, per input: no panic; lossless CST; spans in bounds and on
char boundaries; model iff no syntax error, same syntax diagnostics; round
trip through the serializer; validation and analysis; and a time budget of
50 ms + 50 µs per input byte.

* Preliminary run (code before the validation fix): 4,028,091 executions in
  15 minutes, clean (stopped by hand to restart on the final code).
* **Exit run (final code), 2026-09-21 14:11–15:16: clean.** 6,973,130
  executions in 3,901 s (1,787/s, `nice -n 10` beside other work), 0 crashes,
  0 timeouts, 0 slow units, no artifacts; coverage 6,054 edges / 32,317
  features; corpus grown from 2,062 seeds to 4,899 inputs (8.0 MB), longest
  16,384 B (the `-max_len`); peak RSS 456 MB.

## A9 — the D1 gate

```sh
CARGO_BUILD_JOBS=3 cargo run --release -p parser-gate -- --gate \
    --json bench/parser-gate/gate-p1.json --md bench/parser-gate/GATE-P1.md
```

Median of 31 samples per cell, interleaved; load average 1.2 (a quiet
machine); report in `bench/parser-gate/GATE-P1.md`. **Gate: pass on every
row.**

| Input | Stage | State | ox ns/msg | ours ns/msg | speed-up | ox allocs/msg | ours | ox B/msg | ours |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| 462 suite messages | CST | fresh | 820 | 158 | 5.2× | 13.2 | 1.3 | 2,869 | 652 |
| | CST | reused | 415 | 129 | 3.2× | 1.1 | 0.0 | 190 | 19 |
| | + model | fresh | 1,537 | 305 | 5.0× | 26.3 | 2.9 | 3,490 | 575 |
| | + model | reused | 1,681 | 284 | 5.9× | 25.3 | 1.9 | 3,082 | 212 |
| 1,600-message workload | CST | fresh | 288 | 67 | 4.3× | 8.6 | 1.0 | 1,381 | 236 |
| | CST | reused | 199 | 45 | 4.4× | 1.0 | 0.0 | 167 | 5 |
| | + model | fresh | 527 | 86 | 6.1× | 11.7 | 0.5 | 1,516 | 185 |
| | + model | reused | 610 | 82 | 7.4× | 10.7 | 0.3 | 1,108 | 83 |
| 1,256 placeholder-free | CST | fresh | 199 | 44 | 4.5× | 8.0 | 1.0 | 1,171 | 64 |
| | CST | reused | 149 | 26 | 5.7× | 1.0 | 0.0 | 158 | 0 |
| | + model | fresh | 340 | 27 | 12.6× | 9.0 | **0.0** | 1,235 | **0** |
| | + model | reused | 397 | 25 | 15.9× | 8.0 | **0.0** | 827 | **0** |

Correctness: ox 460/462, `mf2-syntax` 462/462; neither reports an error on the
workload. What still allocates: a fresh CST owns its arena (one allocation;
the fast path reserves three entries for plain text); a model owns one vector
per collection with more than one element (a pattern with a placeholder, the
declarations, the variants…); nothing else.

Where the time went, in order of effect: the fast path for placeholder-free
messages (no arena, no lowering — the model rows' 12–16×); 8-bytes-at-a-time
scanning of text and literal runs; one reservation per message instead of
arena growth; and not scanning whitespace twice in lookahead loops.

**CI.** The gate is job `parser-gate` in `.forgejo/workflows/ci.yml` (a
release build with fat LTO plus ≈ 15 s of measurement), separate from `cargo
xtask ci` because of that cost. Whether it runs on every push or only on
schedule is the owner's call; it is ready for either.

## Carried forward

* **Owner decisions** unchanged from 08 (commits and remote; spec license —
  every consumer of `spec/` now goes through `mf2_conformance::spec::spec_path`,
  so moving it is one line; B5/B7/B9 confirmation; the later-phase items).
* `current_phase` is bumped to `P1` with the harness green; C5 (deleting
  `probes/`) still waits for a first commit — the A6 fixture has preserved what
  Phase 1 needed from P0.7.
* For Phase 2: the `mf2-model` additions (08 §"Status at exit"); NUL-free
  models only are representable in both the syntax and the catalog, so the
  writer's NUL check (02 §2) and the serializer's agree; `analyze` gives
  exactly the slot order and markup/function sets the manifest needs.
