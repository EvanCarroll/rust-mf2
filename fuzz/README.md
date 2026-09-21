# fuzz — cargo-fuzz targets

plans/01-conformance.md §5 ("Fuzzing"): no panic, no out-of-bounds, linear
time. A standalone workspace (the root workspace excludes it), because
cargo-fuzz needs a **nightly** toolchain: this is the one place in the
repository that uses `+nightly`; everything else stays on the stable toolchain
pinned by `rust-toolchain.toml`.

| Target | Input | Checks, on every input |
|---|---|---|
| `parse` | bytes → `from_utf8_lossy` | `mf2-syntax`: no panic; the CST is lossless and every span is in bounds and on char boundaries; `parse_model` gives a model exactly when `parse_cst` finds no syntax error, with the same syntax diagnostics; a model serializes and re-parses to itself; validation and analysis run; the whole check stays within 50 ms + 50 µs per input byte |
| `catalog` | starts with `MF2B`: a catalog, loaded with the hash its header carries. Otherwise: MF2 source up to the first NUL, then an options byte and 4-byte mutation instructions — the source is compiled with `writer::single`, and the instructions damage the catalog inside a chosen section (set, xor, add, insert, delete bytes; the section table kept consistent) | `mf2-catalog`: no panic; writer output loads and decodes to the parsed model (L3), deterministically; for every catalog that loads — a wrong hash is `ManifestMismatch`; header accessors, `sections`, FUNCS, LOCALE, NAMES, FALLBACK; per message `get`, a full view walk (every declaration, part, expression, option, markup, selector, variant, key; `text` on every string, every variable through NAMES, every function through FUNCS) and `decode_report`, which succeeds only after a clean walk and gives a model of the same shape; view iterators yield at most their count and end after `Malformed`; `lookup` finds every id of an ascending IDS; the decoded models are written again (must succeed, deterministically), load, and decode to themselves, stripped too; the whole check stays within 50 ms + 50 µs per input byte + 100 ns per byte of resolved text |

`catalog` charges resolved text because `Catalog::text` is linear in the
string by design (F4) and a string may be referenced many times, so the text
a catalog resolves to can be quadratic in its size. For the same reason a
message that resolves more than 16 MiB of text is not decoded, and the round
trip is skipped past 64 MiB.

## Running

```sh
cargo xtask fuzz-seed          # both corpora (below)
cd fuzz
cargo +nightly fuzz run parse -- -dict=mf2.dict -max_len=16384 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900
cargo +nightly fuzz run catalog -- -dict=mf2.dict -max_len=131072 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900
```

Seeds (`cargo xtask fuzz-seed`):

* `corpus/parse/`: the 462 suite messages and the 1,600 workload messages;
* `corpus/catalog/`: per suite message that parses (326), its one-message
  catalog unstripped and stripped, its source, and its source with an options
  byte and four mutation instructions; the whole workload as one catalog,
  unstripped with a plural entry and fallbacks (every section; 73,336 B) and
  stripped (51,620 B).

`-max_len=131072` for `catalog`: above the largest seed (the unstripped
workload catalog, 73 KB), with room for insertions; the per-byte budget at
that length, 6.6 s, stays below libFuzzer's `-timeout`. The dictionary helps
source mode; catalog mode relies on libFuzzer's comparison tracing.

`fuzz/corpus/` and `fuzz/artifacts/` are git-ignored. A crash leaves its input
in `fuzz/artifacts/<target>/`; reproduce it from `fuzz/` with
`cargo +nightly fuzz run <target> artifacts/<target>/<file>`.

cargo-fuzz builds with AddressSanitizer, which reserves terabytes of virtual
memory: do **not** run it under `ulimit -v`; bound memory with libFuzzer's
`-rss_limit_mb` instead.

The exit runs (≥ 1 h, clean) and their figures are recorded in
[plans/phase-1-results.md](../plans/phase-1-results.md) (`parse`) and the
Phase 2 results (`catalog`).
