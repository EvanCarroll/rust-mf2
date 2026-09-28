# Phase 10 A6 probe — a single-crate web application

A throwaway probe (plans/18-phase-10-work-order.md, task A6; deleted at the
phase's exit, its result recorded in the work order).

`hello/` is Getting started's application in its lazy-route form
(delivery-modes.md), with **no translation crate**: `build.rs`, `mf2.toml`
and `locales/` sit in the application, whose `src/lib.rs` includes the
generated module. The 1.x crates by path. `build.rs` runs `mf2-build` and
swaps the generated `tr!` / `msg_id!` wrappers for A3's shape (3c: exported
under a hidden name, re-exported by `use`, plus a `prelude`), standing in
for what 2.0's codegen would emit.

`tr!` is called from a module declared before the include
(`src/pages.rs`, through `crate::prelude`), one declared after it
(`src/app.rs`, `use crate::tr`), the crate root (no import) and the server
binary (`src/main.rs`, `use hello::tr` — another crate).

`scenario.sh` runs the edit scenarios under `cargo leptos build` and hashes
the wasm; `watch.sh` runs `cargo leptos watch` and edits a translation.
Outputs and observations are in `results/`.
