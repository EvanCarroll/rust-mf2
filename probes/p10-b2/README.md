# p10-b2 — Phase 10 B2: the scripts behind B2's figures

Not a probe of an idea but the measurements of a change: B2, `mf2-native`
into `mf2` as `mf2::native` (`plans/18-phase-10-work-order.md`, B2's
record). Deleted at Phase 10's exit with the other `probes/p10-*`; the
figures stay in the record.

B2 changes no web code, so the web's figures are checked for being
byte-identical, not only within the gates. The size A/B ran in a worktree of
its own (`.claude/worktrees/p10-b2-measure`, detached at `c41225c`, the commit
before B2), with the main tree's lock files copied in: the base first, then
the same tree with B2's change applied, so that every A/B is in one tree,
each generated application keeps its `Cargo.lock` (`--keep`), and the demos'
locks from the base run are restored byte for byte. The change applied was
`git diff --binary c41225c -- crates xtask CHANGELOG.md plans/00-master-plan.md
plans/05-tooling.md plans/19-native-and-terminal.md plans/README.md` of the
main tree (kept as `target/p10-b2/b2.patch` there), with its two new files,
`crates/mf2-native/tests/names.rs` and this directory, copied in. Outputs go
to the tree's git-ignored `target/p10-b2/`. Run `measure.sh` from the root of
such a tree.

| Script | What |
|---|---|
| `measure.sh base\|b2` | B1's `probes/p10-b1/measure.sh` — `cargo xtask size`, `cargo xtask b5 --view`, `cargo xtask catalog-size`, and the three demos' clients built as they ship (`cargo leptos build --release [--split] --frontend-only`, `trunk build --release`), measured with `probes/p10-names/measure-demo.mjs` — with every built wasm, the size workloads' locks and demo-csr's `dist/` (its catalogs among them) hashed, for a byte-for-byte comparison |
| `demo-hashes.sh [BASE [OTHER]]` | the three demos' shipped files as `measure.sh` kept them for two runs (demo-ssr's and demo-islands' `pkg/`, demo-csr's `dist/`), hashed and compared: what B2's "the demos byte-identical, but demo-ssr's `__wasm_split` loader" rests on. `measure.sh` hashes all three as it builds since B1's review fixes; B2's runs predate that, so this reads their kept outputs. Run in the measurement tree |
| `refusals.sh` | 19 §3's refusal of `native` beside `hydrate` or `csr`: what a user sees, on `mf2` and through the `mf2-native` shim, for `wasm32-unknown-unknown` and natively; and `native` beside `ssr`, which must compile. Since the browser-only refusal (`plans/18-phase-10-work-order.md` question 24) the natively built case compiles, and `cargo xtask refusals` checks both sides. Run in the main tree; logs in its `target/p10-b2/refusals/` |
| `unify.sh` | B1's review fixes, item 9: a workspace under `target/p10-b2/unify/` with a client-only web crate that names `csr` on `leptos-mf2` and `mf2` (as `examples/demo-csr` does) and a command-line tool on `mf2-native`. Each compiles alone (`cargo check -p`); `cargo check --workspace` unifies `native` with `csr`, and met 19 §3's refusal until the browser-only refusal (question 24), which made it compile. Run in the main tree |
| `checks.sh [NAME…]` | the rest of the done-when list, one after the other in the main tree: `cargo xtask docs`, `docs-rs`, `codegen-matrix`, `scenarios`, `leptos-0-8`, `l6-web`, `l7-web`, `churn`, `msrv`, `bash bench/b12/check.sh`, `cargo xtask b12-generated`, and the six browser checks on both Leptos lines (B1's `probes/p10-b1/e2e.sh`, labels `b2-leptos-0-9` and `b2-leptos-0-8`); each logged with its exit status in `target/p10-b2/checks/` |

Also used, unchanged: `cargo xtask tui-gate --save-baseline DIR` in the base
tree, then `cargo xtask tui-gate --baseline DIR` with B2 applied (the native
side: allocations per frame, time per frame alternating the two builds, the
stripped sizes); and B1's `probes/p10-b1/e2e.sh` for the six browser checks.
