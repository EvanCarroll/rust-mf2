# p10-c1 — Phase 10 C1: the scripts behind C1's figures

Not a probe of an idea but the measurements of a change: C1, `tr!`'s
arguments through `mf2::IntoArg` (`plans/18-phase-10-work-order.md`, C1's
record; `plans/19-native-and-terminal.md` §7). Deleted at Phase 10's exit
with the other `probes/p10-*`; the figures stay in the record.

C1 ran in the main tree. `bash probes/p10-b2/measure.sh c1-base` at HEAD
before any change, then `… c1` with it (both keep the size workloads' and the
demos' locks), and `cargo xtask tui-gate --save-baseline` / `--baseline` for
the native side. These scripts read what that left in `target/p10-b2/`. Each
one that builds HEAD's sources keeps C1's aside and puts them back with fresh
modification times, whatever happens.

| Script | What |
|---|---|
| `named.sh [TEMPLATE]` | a size workload's client (`tr-view` by default, or `tr`) at 1,860 sites with its symbol names kept, at HEAD's sources and at C1's, compared function by function (`probes/p10-b1/norm-diff.py`). What found the first dispatch's `Option` check at every `String` argument, and what the kept dispatch leaves |
| `ab.sh` | the shipped clients of `tr` and `tr-view` at both scales, built at HEAD's sources and at C1's in one tree and kept: raw, gzip -9 and brotli q11 of each, and `probes/p10-b4/wasmcmp.py` section by section. `target/p10-c1/ab/report.md` |
| `inline.sh` | 19 §7's inline short string: `inline-str.patch` applied, the `arguments` test, `cargo xtask size --keep`, `cargo xtask b5 --view --keep` and `cargo xtask tui-gate --baseline target/p10-c1/tui-c1`, then the patch reversed |
| `inline-str.patch` | the variant measured: a hidden `Text::Inline` holding up to 22 bytes on 64-bit targets and 10 on `wasm32` (what fits in the room the two pointer variants take), which `IntoArg for &str` and `for char` fill. Not kept (the record says why) |
