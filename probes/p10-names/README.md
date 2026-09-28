# p10-names — Phase 10 A7: names and coherence

A probe (`plans/18-phase-10-work-order.md`, A7), deleted at Phase 10's exit;
its result is A7's task record in that file.

It ran on the probe branch `p10-a7-names` (`b258474`, `7dbcfd8`, `9c945ed`,
`f54cf6d` on `2fb7f54`), which is not merged: the six components in
`crates/mf2-leptos-ui-0-9` / `-0-8` behind a function table, and
`leptos-mf2` / `mf2-axum` without the root rename — kept here as
`helper-crates.patch` (`git apply` at `2fb7f54`; the branch itself is kept
for B1). This directory is a
standalone workspace (the root workspace excludes `probes/`), checked with
`cargo check` only:

| Crate | What |
|---|---|
| `naming` | a stand-in `mf2` with `pub mod leptos` and `pub mod axum` beside those crates, the Leptos lines reached through internal aliases; its `n1-*`, `n3-*`, `n4-*` features are negative controls |
| `naming-n2` | the same with the 0.9 line under its real name `leptos` (`n2-root-use`) |
| `naming-app` | an application naming the stand-in's modules beside its own `leptos` and `axum` (`n5-bare-module-import`) |
| `coherence` | what an application writes with the call-site types, Leptos and Ratatui both on; the `probe-*` features switch on the branch's negative controls in `leptos-mf2` |

| Script | What |
|---|---|
| `controls.sh` | the 0.8 line of `naming`, then N1–N5, each a `cargo check` whose error is the observation |
| `coherence.sh` | the intended Ratatui impls beside the Leptos glue (no Leptos, `ssr`, `hydrate` on wasm32), then each coherence rule's negative control |
| `demos-0-8.py <tag> [--keep-lazy]` | copies of the three demos on Leptos 0.8, with the e2e harness beside them, under `target/a7-demo-0-8/<tag>/` |
| `measure-demo.mjs <dir> [label]` | a demo's shipped `.wasm` / `.js` files: raw, gzip −9 and brotli q11 (Node's zlib), sha256 |
| `twiggy-norm-diff.py base.json v3.json [rows]` | two `twiggy top -f json` tables joined on names with crate hashes, closure numbers and split hashes dropped, so renames cancel |

Run from the root of a tree on the branch. The demos' A/B (base `2fb7f54`
against the branch, in one worktree, each demo's `Cargo.lock` kept):
`cargo leptos build --release [--split] --frontend-only --cargo-offline` in
`examples/demo-ssr` (`--split`) and `examples/demo-islands`, `trunk build
--release` in `examples/demo-csr`, each output measured with
`measure-demo.mjs`, then `git switch --detach 2fb7f54`, the same, the locks
restored, `git switch p10-a7-names`, the same again (the second branch build
reproduces the first). The named builds for twiggy are the same clients built
by cargo with `CARGO_PROFILE_WASM_RELEASE_STRIP=none` (demo-ssr, `--profile
wasm-release --features hydrate`) or `CARGO_PROFILE_RELEASE_STRIP=none`
(demo-csr, `--release`) into a target directory of their own.
