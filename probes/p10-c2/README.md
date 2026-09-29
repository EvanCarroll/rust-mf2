# p10-c2 — Phase 10 C2: the scripts behind C2's figures

Not a probe of an idea but the measurements of a change: C2, the native
ambient store (`plans/18-phase-10-work-order.md`, C2's record;
`plans/19-native-and-terminal.md` §5, §6). Deleted at Phase 10's exit with
the other `probes/p10-*`; the figures stay in the record.

C2 ran in the main tree. `bash probes/p10-b2/measure.sh c2-base` at HEAD
before any change, then `… c2` with it (both keep the size workloads' and
the demos' locks), and `cargo xtask tui-gate --save-baseline` / `--baseline`
for the native side, alternating with A1's kept 1.x binaries and with
HEAD's. Outputs go to the main tree's git-ignored `target/p10-c2/`.

| Script | What |
|---|---|
| `tui-named.sh build LABEL` / `diff BASE OTHER` | `examples/tui`'s `tui-mf2` as the gate builds it, symbols kept, from the tree as it stands; the functions whose size moved between two labels. What found where the 160 KB went |
| `at-head.sh CMD…` | runs a command with `crates/` as HEAD has it and puts C2's sources back afterwards (with fresh modification times): the B10 harness's 1.x rows measured at HEAD's code |
| `named.sh [TEMPLATE [WORKLOAD]]` | a size workload's client (`tr` by default, `tr-view`, or a demo) with its names kept, built at HEAD's sources (through `at-head.sh`) and at C2's, compared function by function (B1's `norm-diff.py`): what found the web's moves and showed the last one to be function order alone |
| `checks.sh [NAME…]` | the rest of the done-when list, one after the other: `docs`, `docs-rs`, `codegen-matrix`, `scenarios`, `leptos-0-8`, `l6-web`, `l7-web`, `churn`, `msrv`, B12, `b12-generated`, and the six browser checks on both Leptos lines (the 0.8 copies taking over the build directories of the copies made at HEAD); logs in `target/p10-c2/checks/` |
| `lookup.sh [RUNS] [ROUNDS]` | the ambient lookup's first step, timed (A4 left it to C2): `lookup/` built with `native` alone and with `ssr` unified beside it, the two binaries alternating; medians per call of `to_cow`, `to_string` and `{}` |
| `lookup/` | the bench `lookup.sh` builds: the store installed with a two-message corpus, nothing in a request |

Also used, unchanged: `cargo run --release -p runtime-bench -- b10` (B10,
with C2's `native-*` and `ambient-*` rows), B1's `probes/p10-b1/e2e.sh` and
A7's `probes/p10-names/demos-0-8.py` for the six browser checks, and B2's
`probes/p10-b2/{measure,demo-hashes}.sh` for the web.
