# p10-ambient — Phase 10 A4: the native ambient store's cost

A probe (`plans/18-phase-10-work-order.md`, A4), deleted at Phase 10's exit;
its result is A4's task record. A standalone workspace over the path crates.
It was built on the probe branch `p10-a4-ambient`; the runtime seam that
`--features seam` needs is not on `main` — it is kept here as `seam.patch`
(`git apply probes/p10-ambient/seam.patch` from the repository root).

| Member | What |
|---|---|
| `i18n/` | a 112-message trippy-shaped corpus (`en`, `fr`), 1.x's native build (`Emit::Native`) |
| `ambient/` | the design, (b), and (c): the store, the lookup, `Display`, `to_cow`, the zero-copy `Line` |
| `bench/` | every variant as a frame and as single-message cases; the thread tests |
| `cli-a/`, `cli-b/` | one CLI on 1.x's `NativeI18n` and on the store, for the stripped size |

```sh
cargo build --release -p ambient-bench [--features seam]   # seam: needs the runtime seam (below)
./target/release/ambient-bench check        # every variant renders every message alike
./target/release/ambient-bench allocs       # allocations, exact (run it twice)
./target/release/ambient-bench breakdown    # where a frame's allocations go
./target/release/ambient-bench time 101     # ns per case, variants interleaved
./target/release/ambient-bench time-mt 8 21 # frames on 8 threads at once
./target/release/ambient-bench pools        # R2's one-time cost
cargo test --release -p ambient-bench       # with_locale under threads; install() named
cargo build --profile stripped -p cli-a -p cli-b && ls -l target/stripped/cli-?
```
