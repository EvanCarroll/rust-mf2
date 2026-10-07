# parser-vs-ox — the D1 gate

The permanent benchmark behind decision D1: `mf2-syntax` must be at least as fast, allocate no more, and be strictly
more correct than `ox_mf2_parser`, on every row. Phase 0 (task P0.12) built the
harness and measured the baseline, ox alone ([BASELINE.md](BASELINE.md)).
Phase 1 added `mf2-syntax` as the second adapter; `--gate` is a real
pass/fail, run by CI (`.forgejo/workflows/ci.yml`, job `parser-vs-ox`). The
Phase 1 exit run is [GATE-P1.md](GATE-P1.md) (machine-readable:
[gate-p1.json](gate-p1.json)): **pass on every row**, 3.2–15.9× faster than ox,
fewer allocations everywhere and none for placeholder-free messages on the
model rows, 462/462 against ox's 460.

`ox_mf2_parser` is a dependency of this crate only, pinned exactly to
`=0.14.0-alpha.12` (the audited version) in the root `[workspace.dependencies]`.

## Running it

```sh
CARGO_BUILD_JOBS=3 cargo run --release -p parser-vs-ox              # every row; table to stdout
CARGO_BUILD_JOBS=3 cargo run --release -p parser-vs-ox -- --gate    # + apply the gate (exit 1 on failure)
cargo run --release -p parser-vs-ox -- --corpus workload --runs 5   # quick look at one corpus
```

Always use `--release`. It builds with the workspace `release` profile:
opt-level 3 plus fat LTO. A debug build prints a warning, and `--gate` refuses
to run on one. A full run of ox alone takes 10–15 s.

| Option | Default | Meaning |
|---|---|---|
| `--runs N` | 31 | timing samples per cell (cell = row × parser); `--gate` needs ≥ 30 |
| `--min-sample-ms MS` | 20 | minimum length of one sample; sets the passes per sample |
| `--warmup-ms MS` | 100 | warm-up per cell before anything is counted |
| `--corpus C` | all | `suite`, `workload`, `placeholder-free`; repeatable (`--gate` needs all three) |
| `--corpora-dir DIR` | `bench/corpora` | where `suite.json` and `workload-1600.json` are |
| `--suite-dir DIR` | `third_party/message-format-wg/test/tests` | the vendored WG suite (supplies the expected errors) |
| `--md FILE` | `target/parser-vs-ox/report.md` | Markdown report (it is also printed to stdout) |
| `--json FILE` | `target/parser-vs-ox/report.json` | machine-readable report: every sample statistic, allocation totals, build info, the gate's checks |
| `--gate` | off | apply the gate rules; exit 1 if any fails. With ox alone: "baseline only", exit 0 |

Exit status: 0 on success (or baseline only), 1 when the gate fails, 2 on an
error such as a missing or stale corpus or bad settings.

## Inputs

These are committed files only. Nothing is generated at bench time.

* `bench/corpora/suite.json`: the 462 `src` of the vendored WG suite. The run
  checks it test by test against `third_party/message-format-wg` (through
  `mf2-conformance`) and stops if they differ. Regenerate it with
  `cargo xtask gen-workload corpora`.
* `bench/corpora/workload-1600.json`: the reference workload (flat
  `id → source`, default seed), in id order.
* **placeholder-free**: the workload messages that contain no `{` and do not
  start with `.`: no variable, no markup, not complex. That is 1,256 of 1,600.
  The definition is in `bench/workload-gen/README.md`.

## The rows

The rows are 3 corpora × 2 stages × 2 state modes, which makes 12 rows. Every
parser is measured on every row.

| Stage | Means | ox calls |
|---|---|---|
| **parse → CST** | source → lossless CST + diagnostics | `parse_source` |
| **+ model + validation** | source → model + the Data Model validation | `parse_source` + `build_semantic_model` + `validate_semantics` (the model calls run only when there is no syntax diagnostic) |

| State | Means | ox |
|---|---|---|
| **fresh** | all state is created and dropped per message | a new `SourceStore` per message |
| **reused** | state is created once per *pass* (one corpus, i.e. one locale's messages), shared by every message, and dropped at the end of the pass. Creating and growing it is part of the measurement | CST: one `SourceStore` (pre-sized) + one `ParseWorkspace`, `parse_source_session`. + model: one `SourceStore` only, because `build_semantic_model` needs an owned `ParseResult`, which the session API cannot give |

In the reused rows each parser uses the leanest reuse its API offers. Both
parsers get the same pre-sizing hint (message count and bytes of the pass).

Columns:

| Column | Meaning |
|---|---|
| ns/msg | median over the samples of (sample time ÷ (passes × messages)) |
| MB/s | source bytes per second at the median (MB = 10⁶ B) |
| allocs/msg, alloc B/msg | allocation calls and bytes requested in one pass, ÷ messages |
| best ns | the fastest sample (a guide to how much load inflated the median) |
| IQR | interquartile range ÷ median |

## Method

* **One process, interleaved.** Each cell gets a warm-up and then one counted
  pass. Each row's samples are calibrated on the baseline (fastest of three
  single passes → passes per sample). Then come `--runs` rounds. A round takes
  one sample of every (row, parser) cell back to back, and the order reverses
  every other round. Load that comes and goes during the run is therefore
  spread over every cell of the table. The comparison between parsers is a
  ratio of medians taken under the same conditions.
* **Counting allocator** (`src/alloc.rs`, the crate's only `unsafe`, allowed
  locally). It counts `alloc`, `alloc_zeroed` and `realloc` calls on the
  calling thread and the bytes they request. A `realloc` counts as one
  allocation of its new size, which is the audit's definition. Frees are not
  counted. Allocation figures are exact and do not depend on load: a row's
  counts are the same on every machine for the same code.
* **Checksum.** Each pass returns the number of diagnostics reported. It keeps
  the optimiser honest, and the unit tests use it to check that fresh and
  reused modes agree.
* **Build info.** The JSON records `rustc -V`, the opt-level, whether debug
  assertions were on, `available_parallelism` and the run time.

## Correctness

Every run also classifies each suite test, following
`probes/audit/ox-conformance`. The expected set is the test's `expErrors`
types, restricted to `syntax-error` and the six Data Model Errors. A parser is
*exact* on a test when it reports exactly that set: a syntax error if it has
any syntax diagnostic, and otherwise the Data Model errors from validation.
ox's two differently-named codes are mapped (`variant-key-arity-mismatch` →
`variant-key-mismatch`, `invalid-declaration-dependency` →
`duplicate-declaration`). The run also counts workload messages that report any
error; this should be 0, since the generator emits valid MF2.

The baseline is **ox 460/462**. Both misses over-report
`missing-fallback-variant` next to the expected `variant-key-mismatch`
(`data-model-errors.json` #0 and #1).

## The gate (`--gate`)

It applies these rules to every contender after the baseline:

1. on every row, median ns/msg ≤ **1.05 ×** ox's median;
2. on every row, allocation calls per pass ≤ ox's, and bytes per pass ≤ ox's,
   exactly;
3. **462/462** exact on the suite. ox is at 460, so this is "strictly more
   correct".

It needs a release build, all three corpora and `--runs` ≥ 30. The gate checks
appear in the report whether or not `--gate` is given. The flag only turns a
failure into exit status 1. The third D1 requirement (lossless CST, error
recovery, stable diagnostic codes, `forbid(unsafe_code)`) is a review item, not
something this harness can measure.

## The `mf2-syntax` adapter (Phase 1)

`src/mf2_syntax.rs` implements `adapter::Adapter` for `mf2-syntax`, and
`contenders()` lists it after ox (index 0 stays the baseline):

* `cst_fresh` / `model_fresh`: `parse_cst` / `parse_model` (which validates),
  each with a new arena per message — `parse_model`'s fast path needs none for
  a message without `{`, `}`, `\` or a leading `.`;
* `new_state`: an empty `mf2_syntax::Parser` (no pre-sizing: the arena grows to
  the largest message of the pass, and that growth is measured);
  `cst_reused` / `model_reused`: `Parser::parse_cst` / `Parser::parse_model`;
* `classify`: `{"syntax-error"}` if there is any syntax diagnostic, otherwise
  the suite names of the Data Model errors.

Its tests check that fresh and reused passes agree, a few classifications
(including `data-model-errors.json` #0, where ox over-reports), and that the
placeholder-free model rows allocate nothing.

To re-run the exit measurement:

```sh
CARGO_BUILD_JOBS=3 cargo run --release -p parser-vs-ox -- --gate \
    --json bench/parser-vs-ox/gate-p1.json --md bench/parser-vs-ox/GATE-P1.md
```

## Tests

`cargo test -p parser-vs-ox` takes well under a second and uses tiny inputs:
allocator accounting, corpus sizes and the placeholder-free definition,
quartiles, the gate's rules, both adapters (fresh = reused; classification;
the zero-allocation rows of `mf2-syntax`), one tiny measured row, and the
correctness baseline. The correctness baseline
runs ox over the 462 suite tests and the workload once and asserts 460/462,
superset-only misses, and a clean workload. The benchmark itself only runs as
the binary.
