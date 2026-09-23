# fuzz — cargo-fuzz targets

plans/01-conformance.md §5 ("Fuzzing"): no panic, no out-of-bounds, linear
time. The time budget is **CPU time**, not wall clock (`common/budget.rs`):
the property is that the work per input byte is bounded, and measuring it on
a clock the machine shares makes a busy desktop fail a run the code passes.
A standalone workspace (the root workspace excludes it), because
cargo-fuzz needs a **nightly** toolchain: this is the one place in the
repository that uses `+nightly`; everything else stays on the stable toolchain
pinned by `rust-toolchain.toml`.

| Target | Input | Checks, on every input |
|---|---|---|
| `parse` | bytes → `from_utf8_lossy` | `mf2-syntax`: no panic; the CST is lossless and every span is in bounds and on char boundaries; `parse_model` gives a model exactly when `parse_cst` finds no syntax error, with the same syntax diagnostics; a model serializes and re-parses to itself; validation and analysis run; the whole check stays within 50 ms + 50 µs per input byte |
| `catalog` | starts with `MF2B`: a catalog, loaded with the hash its header carries. Otherwise: MF2 source up to the first NUL, then an options byte and 4-byte mutation instructions — the source is compiled with `writer::single`, and the instructions damage the catalog inside a chosen section (set, xor, add, insert, delete bytes; the section table kept consistent) | `mf2-catalog`: no panic; writer output loads and decodes to the parsed model (L3), deterministically; for every catalog that loads — a wrong hash is `ManifestMismatch`; header accessors, `sections`, FUNCS, LOCALE, NAMES, FALLBACK; per message `get`, a full view walk (every declaration, part, expression, option, markup, selector, variant, key; `text` on every string, every variable through NAMES, every function through FUNCS) and `decode_report`, which succeeds only after a clean walk and gives a model of the same shape; view iterators yield at most their count and end after `Malformed`; `lookup` finds every id of an ascending IDS; the decoded models are written again (must succeed, deterministically), load, and decode to themselves, stripped too; the whole check stays within 50 ms + 50 µs per input byte + 100 ns per byte of resolved text |
| `pipeline` | bytes → `from_utf8_lossy`, taken as one locale's resource file; the target makes a two-locale corpus from it, the translation holding every other entry | `mf2-build`: no panic; a corpus the build refused reported an error and wrote no catalog; one it accepted has catalogs that load under the manifest's hash, decode message by message to the flattened models, carry the right fallback flag, are byte-identical on a second build that writes nothing, and a generated module that names no catalog outside its `ssr` block |
| `resource` | bytes → `from_utf8_lossy` | `mf2-resource`: no panic; every span — diagnostics, entries, ids, values, comments, properties — is in bounds and on char boundaries, and every cooked offset of every value maps back into the file; a file read **without a diagnostic** serializes in both styles, parses to the same resource and writes itself again byte for byte (`mf2 fmt` is idempotent); the whole check stays within 50 ms + 50 µs per input byte |
| `format` | `[flags] [n] [n argument bytes] [payload]`: flags bit 0 = bidi `None`, bit 1 = the default configuration's registry; arguments `[tag] …` (`tag % 8`: string, `i64`, `f64`, decimal text, opaque value, date/time literal text, date/time instant in epoch ms, unset); payload a catalog (starts with `MF2B`, loaded with its header's hash) or MF2 source up to a NUL, a locale byte and a strip byte, written as a one-message catalog for that locale (valid or not) with its plural rules, number data and — when it formats a date or can receive one — the locale's `icu.blob` with every shape's data (built once per locale); source mode's compile is build side and off the clock (the `parse` and `catalog` targets hold it to linear time) | `mf2-runtime` with the L4 registry (all features: `:string`, the localized numeric functions, `:percent`, `:currency`, `:unit`, `:datetime` / `:date` / `:time` over ICU4X from the catalog's `icu.blob`, the unannotated hooks, `:test:*`; or the default configuration's) — in catalog mode with the date functions over the neutral backend, since ICU4X does not promise to survive a damaged blob (a smoke run found `DecimalFormatter` panicking on a damaged numbering-system name, in release builds too): ICU4X reads only the pristine blobs of source mode — and `mf2-host-std` (with `jiff`'s zone data): every message formatted positionally to a string twice (deterministic), to parts (they concatenate to the string, same errors) and with named arguments under its own slot names (same output, when the names are distinct and NFC); an id past the last message is a Missing Message; no panic; the run — a catalog's load and the formatting — within 50 ms + 50 µs per input byte + 100 ns per byte of output |

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
cargo +nightly fuzz run format -- -dict=mf2.dict -max_len=131072 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900
cargo +nightly fuzz run resource -- -dict=mf2.dict -max_len=16384 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900
cargo +nightly fuzz run pipeline -- -dict=mf2.dict -max_len=16384 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900
```

Seeds (`cargo xtask fuzz-seed`):

* `corpus/parse/`: the 462 suite messages and the 1,600 workload messages;
* `corpus/catalog/`: per suite message that parses (326), its one-message
  catalog unstripped and stripped, its source, and its source with an options
  byte and four mutation instructions; the whole workload as one catalog,
  unstripped with a plural entry and fallbacks (every section; 73,218 B) and
  stripped (51,502 B);
* `corpus/resource/` and `corpus/pipeline/`: the reference workload and the
  suite's messages, each as one resource file (`workload.mf2`, 76.6 KB;
  `suite.mf2`, 20.1 KB — the sources the container cannot write, the malformed
  ones with a lone `\`, are left out);
* `corpus/format/`: per L4 suite test (301), its catalog for its locale,
  unstripped and stripped, and its source (source mode), each behind its
  `params` as positional arguments; the workload catalogs with a few
  arguments; 400 generated L4 cases (`conformance/src/l4gen.rs`).

`-max_len=131072` for `catalog`: above the largest seed (the unstripped
workload catalog, 73 KB), with room for insertions; the per-byte budget at
that length, 6.6 s of CPU, stays below libFuzzer's `-timeout` — which is
wall clock, and remains the guard against a target that blocks instead of
spinning. The dictionary helps
source mode; catalog mode relies on libFuzzer's comparison tracing.

`fuzz/corpus/` and `fuzz/artifacts/` are git-ignored. A crash leaves its input
in `fuzz/artifacts/<target>/`; reproduce it from `fuzz/` with
`cargo +nightly fuzz run <target> artifacts/<target>/<file>`.

cargo-fuzz builds with AddressSanitizer, which reserves terabytes of virtual
memory: do **not** run it under `ulimit -v`; bound memory with libFuzzer's
`-rss_limit_mb` instead.

`format` charges output for the same reason: a message's output can be
quadratic in the catalog's size. Its string sink stops resolving catalog
text past 16 MiB (still counting what it is handed), and a message that
reached the cap is not formatted again.

The exit runs (≥ 1 h, clean) and their figures are recorded in
[plans/phase-1-results.md](../plans/phase-1-results.md) (`parse`), the
Phase 2 results (`catalog`) and the Phase 3 results (`format`).
