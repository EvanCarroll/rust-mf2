# p10-b1 — Phase 10 B1: the scripts behind B1's figures

Not a probe of an idea but the measurements of a change: B1, the call-site
types and the Leptos layer into `mf2` (`plans/18-phase-10-work-order.md`,
B1's record). Deleted at Phase 10's exit with the other `probes/p10-*`;
the figures stay in the record.

They ran in a worktree of their own (`.claude/worktrees/p10-b1-measure`,
detached at `7a7994d`): the base first, then the same tree with B1's change
applied (`git diff` of the main tree as a patch), so that every A/B is in one
tree, each generated application keeps its `Cargo.lock` (`--keep`), and the
demos' locks from the base run are restored byte for byte. Outputs go to the
tree's git-ignored `target/p10-b1/`. Run each from the root of such a tree.

| Script | What |
|---|---|
| `measure.sh base\|b1` | `cargo xtask size`, `cargo xtask b5 --view`, `cargo xtask catalog-size`, and the three demos' clients built as they ship (`cargo leptos build --release [--split] --frontend-only`, `trunk build --release`), measured with `probes/p10-names/measure-demo.mjs` |
| `measure-demos.sh LABEL` | the demos only, with the base's locks restored: for a variant of the components' dispatch applied to the tree first (`table/`, A7's function table in the merged layout, over `crates/mf2-leptos-ui-0-9/src/ui.rs` and `crates/mf2/src/leptos/components.rs`) |
| `named.sh PATCH` | demo-ssr's client with its names kept, at base and with PATCH applied, then `norm-diff.py` (A7's `twiggy-norm-diff.py` with B1's moves mapped back onto 1.x's paths): the function-level reading of a move. B1's reading: `named.sh probes/p10-b1/b1-static.patch` |
| `b1-static.patch` | B1's change as the measurement tree holds it (static dispatch; the code is `1023d57`'s, only comments differ): the tree is `7a7994d` with it applied, and `named.sh` reverses it for its base build |
| `display-debug.sh` | 19 §14's `Display` (S3) / `Debug` (S2) row, with A9's cases (`probes/p10-display-cost/case.py`): shipped builds of `{}`, `.to_string()` and `{:?}` in the demos, and debug-profile builds of demo-ssr for `fmt-check.sh` |
| `fmt-check.sh WASM…` | A9's check for the merged crate: a client that links a `Display` / `Debug` impl of ours, or the `Debug` writers, fails |
| `islands-control.sh` | what `format!` itself costs demo-islands, whose client calls it nowhere else: the control for `{:?}` there |
| `debug-variants.sh` | the two leaner forms of the `Debug` writers B1 tried beside S2, in demo-islands with A9's `debug-trargs` case: `debug-writestr.patch` (`write_str` for every character) and `debug-buffer.patch` (the digits through a buffer), each applied to `crates/mf2/src/debug.rs` and reverted after its build. Their code was not kept when B1 measured them; the patches reconstruct it (the buffer one byte-identical to B1's build of it) |
| `fixture-trview.sh` | `{:?}` on a `TrArgs` in the fixture client and in `tr-view` at 1,860 sites |
| `e2e.sh ROOT LABEL` | the six browser checks (demo, lazy, islands, csr, zone, a11y) in Chromium and Firefox against release builds of the demos under ROOT: the tree (Leptos 0.9), or copies made by `probes/p10-names/demos-0-8.py` (Leptos 0.8) |
