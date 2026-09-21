# fuzz — cargo-fuzz targets

plans/01-conformance.md §5 ("Fuzzing"): no panic, no out-of-bounds, linear
time. A standalone workspace (the root workspace excludes it), because
cargo-fuzz needs a **nightly** toolchain: this is the one place in the
repository that uses `+nightly`; everything else stays on the stable toolchain
pinned by `rust-toolchain.toml`.

| Target | Checks, on every input (bytes → `from_utf8_lossy`) |
|---|---|
| `parse` | `mf2-syntax`: no panic; the CST is lossless and every span is in bounds and on char boundaries; `parse_model` gives a model exactly when `parse_cst` finds no syntax error, with the same syntax diagnostics; a model serializes and re-parses to itself; validation and analysis run; the whole check stays within 50 ms + 50 µs per input byte |

The catalog decoder target is added in Phase 2.

## Running

```sh
cargo xtask fuzz-seed          # seed corpus: the 462 suite messages + the 1,600 workload messages
cd fuzz
cargo +nightly fuzz run parse -- -dict=mf2.dict -max_len=16384 -timeout=10 \
    -rss_limit_mb=2048 -max_total_time=3900
```

`fuzz/corpus/` and `fuzz/artifacts/` are git-ignored. A crash leaves its input
in `fuzz/artifacts/parse/`; reproduce it with
`cargo +nightly fuzz run parse fuzz/artifacts/parse/<file>`.

cargo-fuzz builds with AddressSanitizer, which reserves terabytes of virtual
memory: do **not** run it under `ulimit -v`; bound memory with libFuzzer's
`-rss_limit_mb` instead.

The Phase 1 exit run (≥ 1 h, clean) and its figures are recorded in
[plans/phase-1-results.md](../plans/phase-1-results.md).
