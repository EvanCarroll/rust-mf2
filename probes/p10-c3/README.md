# p10-c3 — Phase 10 C3: the scripts behind C3's figures

Not a probe of an idea but the measurements and checks of a change: C3, the
one locale matcher (`plans/18-phase-10-work-order.md`, C3's record;
`plans/19-native-and-terminal.md` §9). Deleted at Phase 10's exit with the
other `probes/p10-*`; the figures stay in the record. Outputs go to the
main tree's git-ignored `target/p10-c3/`.

| Script | What |
|---|---|
| `oracle.sh` | the matcher against C3's text half's independent reader (`target/p10-c3/evidence.py`, untracked: the rules as that half read them from UTS #35 Part 1): 447 tags, 23,906 pairs' distances and 3,000 random reader lists against random applications (`oracle.py gen`), answered by both and compared (`oracle.py compare`). The matcher's side is `oracle_test.rs`, copied into `crates/mf2/tests/` for the run and removed after it |
| `named.sh [TEMPLATE]` | a client with its symbol names kept, at HEAD's sources and at C3's (`at-head.sh`), and a function-level diff of the two (B1's `probes/p10-b1/norm-diff.py`): `tr-view` and `tr` from the kept size workloads, `demo-islands` and `demo-ssr` (hydrate), `demo-csr` (its binary, `csr`) |
| `at-head.sh CMD…` | runs a command with `crates/` and demo-csr's translation crate as HEAD has them, and puts C3's back afterwards (C2's `probes/p10-c2/at-head.sh`, plus the one file outside `crates/` C3 changes that a build reads) |
| `checks.sh [NAME…]` | the rest of the done-when list, one after the other in the main tree (C2's `probes/p10-c2/checks.sh` with C3's labels): `docs`, `docs-rs`, `codegen-matrix`, `scenarios`, `leptos-0-8`, `l6-web`, `l7-web`, `churn`, `msrv`, B12, `b12-generated`, and the six browser checks on both Leptos lines |

Also used, unchanged: `probes/p10-b2/measure.sh LABEL` and
`demo-hashes.sh BASE OTHER` (the web's figures, one tree, the locks kept),
and `cargo xtask tui-gate --save-baseline DIR` at HEAD, then `--baseline DIR`
with C3 (native, alternating the binaries).
