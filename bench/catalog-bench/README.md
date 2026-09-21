# catalog-bench — catalog size (B7) and reader cost

Phase 2's measurements of the `.mf2b` catalog
([`plans/09-phase-2-work-order.md`](../../plans/09-phase-2-work-order.md)):

* **`size`** — task A8, budget **B7** ([`plans/06-size-and-perf.md`](../../plans/06-size-and-perf.md)
  §3). Every locale of the reference workload as a production (COLD and IDS
  stripped) and an unstripped catalog: raw, gzip, brotli; structure vs pool;
  section by section. It also checks the B7 thresholds, compares with P0.7,
  compares the gzip implementations and estimates varint NAMES.
  Committed run: [SIZE-P2.md](SIZE-P2.md) ([size-p2.json](size-p2.json)).
* **`bench`** — the native half of task A9 (B9/B10 natively). It times
  `Catalog::new`, `get`, `text` and view walks on the four production
  catalogs, counts allocations, checks 0 copies at load and compares with
  P0.8. It uses the parser gate's method (`bench/parser-gate`).
  Committed run: [READER-P2.md](READER-P2.md) ([reader-p2.json](reader-p2.json)).

## Running it

```sh
CARGO_BUILD_JOBS=3 cargo xtask catalog-size                                 # = cargo run --release -p catalog-bench -- size
CARGO_BUILD_JOBS=3 cargo run --release -p catalog-bench -- bench --gate    # the reader bench and its gate
```

The committed reports are regenerated with exactly these commands:

```sh
CARGO_BUILD_JOBS=3 cargo xtask catalog-size \
    --md bench/catalog-bench/SIZE-P2.md --json bench/catalog-bench/size-p2.json
CARGO_BUILD_JOBS=3 cargo run --release -p catalog-bench -- bench --gate \
    --md bench/catalog-bench/READER-P2.md --json bench/catalog-bench/reader-p2.json
```

Always use `--release`, which is the workspace `release` profile (opt-level 3,
fat LTO). `bench --gate` refuses to run on a debug build. `size` gives the same
bytes in any profile, but brotli 11 is slow in debug. A `size` run takes about
4 s after the build. A `bench` run takes about 15 s.

| Option | Mode | Default | Meaning |
|---|---|---|---|
| `--gz gnu\|flate2\|zlib-rs` | size | `gnu` | the gzip implementation of the reported gz figures (B7 itself is brotli). `gnu` falls back to flate2 when GNU `gzip` is not on `PATH` |
| `--emit DIR` | size | — | also write `<tag>.mf2b` (stripped), `<tag>.full.mf2b` and `manifest.mf2m` into `DIR` |
| `--runs N` | bench | 31 | samples per row (`--gate` needs ≥ 31) |
| `--min-sample-ms MS` | bench | 10 | minimum timed work per sample |
| `--warmup-ms MS` | bench | 50 | warm-up per row |
| `--batch N` | bench | 32 | `Catalog::new` / `clone` calls per timed batch |
| `--gate` | bench | off | apply the gate; exit 1 if a rule fails |
| `--md FILE`, `--json FILE` | both | `target/catalog-bench/<mode>.{md,json}` | the reports (the Markdown also goes to stdout) |

Exit status:

* 0 — success.
* 1 — `size`: a B7 threshold or a round trip failed. `bench --gate`: a gate
  rule failed. `cargo xtask catalog-size` passes this through as a failure.
* 2 — an error: a stale corpus, a wrong manifest hash, a catalog that does not
  load, or bad settings.

## Inputs

Nothing is generated on disk. The four locales `en`, `pl`, `en-XA` and `ar-XB`
come from the `workload-gen` library in process, with default knobs and seed 1
(what `cargo xtask gen-workload locales` writes). The run first checks that
the generated `en` is byte-identical to the committed
`bench/corpora/workload-1600.json`, and stops otherwise. Every message is then
parsed with `mf2_syntax::parse_model` and must have no diagnostic.

* **Manifest**: built from `en` the way `mf2-build` will build it. Ids are
  sorted. Slots and markup come from `mf2_syntax::analyze` (NFC). The
  function set is the union over all locales. Its hash must be P0.7's
  reference `43e0dc12eeb05ef1`, or the run stops.
* **Catalogs**: `mf2_catalog::writer::catalog` with `Options::new(tag, dir)`.
  `ar-XB` is RTL. Each catalog gets `cldr_version` 48.2.1 and one LOCALE
  entry, key 1 `plural.cardinal`, as P0.7 did. `en` and `en-XA` use `en`'s
  rules (5 B), `pl` its own (32 B) and `ar-XB` `ar`'s (17 B). These bytes
  came from P0.4's encoder over `third_party/cldr-json` and are hard-coded in
  `src/plural.rs` until `mf2-locale-data` (Phase 3) replaces them. The
  production catalog is `.stripped()`.
* **Round trip** (`size`): every catalog is loaded with `Catalog::new`
  against the hash. Every message is then decoded with `mf2_catalog::decode`
  and compared with the parsed model. The size report counts these round
  trips, so its figures are for lossless catalogs. `bench` loads the same
  catalogs but does not decode them.

## Size method

* **MF2 source bytes** follow plans/06 §2 and P0.7: the UTF-8 length of every
  message source of the locale, summed.
* **Compressors**:
  * GNU `gzip -9 -n -c`, run as a subprocess. This is what P0.7 measured.
  * `flate2`'s `GzEncoder` with its default backend (miniz_oxide) at level 9,
    `Compression::best()`.
  * `zlib-rs` (the pure-Rust zlib port), `best_compression()` (level 9,
    memLevel 8), gzip wrapper.
  * brotli quality 11, window 22, as P0.7.

  Every gzip figure includes gzip's 18 B framing, so the three are comparable.
  Sizes are deterministic. They change only with the inputs, the writer or the
  compressor versions, which the report prints (from `Cargo.lock`).
* **Structure vs pool**: structure is every byte before STRINGS (header,
  section table, INDEX, MESSAGES, NAMES, LOCALE, FUNCS, and IDS when
  unstripped). Pool is STRINGS. Each part is compressed on its own, as P0.7
  did, so structure gz + pool gz is more than the whole file's gz.
* **B7** (production catalogs), on **brotli 11** — the `.br` file the build
  writes and serves (owner decision, 2026-09-21; gzip is reported, not gated):
  * `en` ≤ 0.91 × 25 KB = 23,296 B.
  * Every locale: brotli ≤ 0.91 × (0.5 × source + 1,024 B).
  * Every locale: raw ≤ 1.25 × source + 8 B × messages (integer arithmetic,
    as P0.7).

  0.91 is the former gzip limits' scale: the worst brotli/gzip ratio
  measured on the four locales (en-XA, 0.909 in P0.7 and in Phase 2), rounded
  up, so no locale is held tighter than under gzip.
* **Against P0.7**: the P0.7 figures for the recommended layout are in
  `src/baseline.rs`. They come from P0.7's `out/tables.md`; the `en` row is
  also in plans/phase-0-results.md §P0.7. A brotli delta (the B7 metric)
  beyond 100 B or 1 % of P0.7's figure is flagged "beyond noise". The report then shows where the
  bytes went, section by section.
* **NAMES estimate**: format v1 writes NAMES string references as fixed
  `str32` so that a name is one O(1) read. `src/names.rs` re-encodes the
  stripped catalog's NAMES with fixed 2-byte references (`str16`, still O(1))
  and with varint references (P0.7's). It also re-points every
  MESSAGES head at the new entry offsets and moves the INDEX offsets to
  match. Everything else is copied byte for byte, and the result is
  compressed like the real catalog. A unit test checks that `str32 → str32`
  is the identity and that `str32 → varint → str32` round-trips.

## Bench method

* **One process, interleaved**, as in the parser gate. Each row gets:
  1. a warm-up;
  2. one operation or sweep counted by the allocator;
  3. a calibration: the fastest of three single passes sets the passes per
     sample, so that one sample is ≥ `--min-sample-ms` of timed work;
  4. `--runs` rounds of one sample of every row, the order reversing every
     other round.

  Load that comes and goes is spread over every row. The report gives the
  median, the best sample and the IQR, plus the load average before and
  after.
* **Rows** (per locale):

  | Row | What it times |
  |---|---|
  | `new` | `Catalog::new` on the stripped catalog |
  | `new-unstripped` | `Catalog::new` on the unstripped catalog (IDS is walked too) |
  | `clone` | `Vec::clone` + drop of the buffer, informative. P0.8 measured the copy this way |
  | `simple` | `get` + `text` sweeping every simple id in `MsgId` order. This is P0.8's "simple lookup" and B10's reader share |
  | `utf8` | `str::from_utf8` alone on the same strings, to attribute `simple`'s cost |
  | `get-pattern`, `get-select` | `get` |
  | `walk-pattern`, `walk-select` | `get`, then the whole body, text resolved, keys and variants walked |

  `Catalog::new` is timed in batches of `--batch` calls. The buffers are
  prepared before the timed region and moved in. After the batch they are
  taken back with `into_bytes()`. Nothing is copied, allocated or freed while
  the clock runs, and the buffers are reused, so the allocator's page churn
  stays out of the figures.
* **Load checks**: for every catalog, allocations during one
  `Catalog::new(buffer, hash)` must be 0. After it, `as_bytes().as_ptr()`
  and the length must equal the moved-in `Vec`'s (0 copies).
* **Gate** (`--gate`):
  * those load checks;
  * 0 allocations in every lookup row;
  * `Catalog::new` median ≤ 1.5 × P0.8 (5.7 / 6.7 / 6.9 / 6.1 µs);
  * simple median ≤ 1.5 × P0.8 (20.7 ns `en`, 28.4 ns `ar-XB`; P0.8 measured
    no others);
  * simple ≤ 100 ns (B10) for the reference `en`. B10 is stated for the
    reference workload; the other locales' B10 rows are reported, not gated.

  Needs a release build and ≥ 31 runs.
* **Counting allocator**: `src/alloc.rs` is the parser gate's. It is the
  crate's only `unsafe`, allowed locally.

Timings depend on the machine and its load. Each report records the load
average. A figure taken while other work ran should be re-run in a quiet
window with the same command. Allocation counts and sizes do not depend on
load.

## Reading the reports

* **SIZE-P2.md**:
  * the B7 table (production catalogs) and every check;
  * stripped vs unstripped with structure / pool (raw / gz / br);
  * raw section sizes;
  * the P0.7 comparison, with the attribution by section and the format
    differences behind it;
  * the compressor table (Δ of flate2 and zlib-rs against GNU gzip);
  * the NAMES estimate.
* **READER-P2.md**:
  * the load checks;
  * one table of every row (median, best, IQR, the P0.8 figure and the ratio,
    allocations per operation, operations per pass);
  * the gate.
* The JSON files hold every figure, including all three gzip implementations
  for every part, the sample statistics, the settings and the build.

## Tests

`cargo test -p catalog-bench` takes about a second (debug build). It covers:

* **the deterministic half of the A9 gate on the real workload**: the eight
  catalogs of the four locales are built, and loading each one allocates
  nothing and keeps the moved-in buffer. So `cargo test --workspace` (CI)
  enforces "load allocates nothing" without a timing job;
* allocator accounting;
* the plural entry sizes;
* P0.7's rows adding up;
* the compressors: zlib-rs's gzip output read back by flate2, and every
  compressor shrinking its input;
* the NAMES re-encoder: identity and round trip;
* the order statistics;
* number formatting.

The timings and the compressed sizes run only as the binary.
