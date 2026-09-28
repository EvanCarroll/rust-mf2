# Phase 10 A3 probe — `tr!` inside its own crate

A throwaway probe (plans/18-phase-10-work-order.md, task A3; deleted at the
phase's exit, its result recorded in the work order). A standalone workspace,
path dependencies on the real `mf2` and `mf2-build`.

| Crate | What it is |
|---|---|
| `v1-today` | today's generated module (`Emit::Native`), included at the root; every spelling of `tr!` from modules before and after the include, one feature each; `allow-52234` is variant 2 |
| `v1-consumer` | another crate calling `v1_today::tr!`; `dep-allows-52234` for variant 2's report |
| `v3-local` | the generated module with the wrapper rewritten into one of five shapes (`shape-*`), and the call sites one feature each; `src/bin/app.rs` is shape 3c in a binary crate |
| `v3-consumer` | another crate naming the shapes' `tr` by path, `use`, prelude, `exports` |
| `v4-env-macro`, `v4-env` | two env-driven stand-ins for an `mf2::tr!` proc macro: `tr_env!` (the build script's `cargo::rustc-env=MF2_MANIFEST=…`) and `tr_outdir!` (`$OUT_DIR/manifest.mf2m`) |
| `v5-prelude`, `v5-glob` | a library prelude with its own `tr`, glob-imported beside a generated `tr` |

`./run.sh [filter]` compiles every case and writes the compiler's output to
`results/<case>.txt` (the repository path as `<repo>`). The scenarios run by
hand — invalidation, a relocated target directory, rust-analyzer, the
future-compatibility report of a git dependency — are in `results/` too, with
the commands at the top of each file.
