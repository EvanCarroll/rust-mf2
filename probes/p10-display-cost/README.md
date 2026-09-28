# p10-display-cost — Phase 10 A9: what `Display` / `Debug` cost the browser's wasm

A probe (`plans/18-phase-10-work-order.md`, A9), deleted at Phase 10's exit;
its result is A9's task record in that file.

It ran in A5's worktree, on the probe branch `p10-a5-display` (`70e0b27`, or
`probes/p10-display/display.patch` applied at `2fb7f54`): one tree, and one
lock per application. Run every script from the root of such a tree; the
outputs go to its `target/a9/` (`results.tsv`, `log.txt`, `out/`, `check/`,
`dev-check/`). `all.sh STEP` runs the steps in the order the record's
figures came from.

| Script | What |
|---|---|
| `all.sh STEP` | the runs: `fixture`, `tr-view` (the split), `demos [DEMO…]`, `tr`, `named`, `shrink`, `remove`, `remove-demos`, `errors`, `silent`, `traps`, `devcheck` |
| `run.sh CLIENT CASE…` | builds a client once per case, as it ships, and measures it; `A9_LIB` labels the library variant, `A9_NAMED=1` also builds it with names kept, `A9_SHIP=0` skips the shipped build, `A9_CHECK=1` only type-checks a demo |
| `case.py apply\|restore\|show CLIENT CASE` | the one-statement edit a case makes to a client's entry point (a working-tree edit, never committed), including the silent paths (`silent-new`, `silent-traps`, `silent-trap-*`, `silent-debug`) |
| `libvar.sh apply NAME \| restore` | a library variant of `crates/leptos-mf2`, from `lib-NAME.patch` |
| `lib-*.patch` | the variants: `v1x` (the crate at `2fb7f54`), `s1-writestr`, `s2-debug`, `r1`, `r1d`, `r2`, `r2d`, `r3` |
| `make-removal.py` | how `lib-r1*.patch` and `lib-r2*.patch` were written |
| `measure.py` | raw, `gzip -9 -n`, `brotli -q 11`, and a `.wasm`'s code and data sections, into `results.tsv` |
| `report.py [CLIENT…]`, `tables.py split\|apps\|variants` | the record's tables, from `results.tsv` |
| `attribute.py BASE CASE [--list N]` | what a case adds, by kind of code: twiggy over two names-kept builds |
| `fmt-check.sh WASM…` | the check CI could run: a names-kept client that links any `Display` / `Debug` impl of ours (or the blanket `ToString` over one) fails. Reliable on a debug-profile build; a release build misses what LLVM inlines (`all.sh devcheck`) |
| `sections.py`, `eq-test.sh` | why a names-kept build is not the measured one (the standard library's DWARF; hence `--strip-dwarf`) |
| `demos-0-8.py` | A7's script, copied into `target/a9/` of the tree: demo-ssr on Leptos 0.8 (`demo-ssr-08`), for the silent paths' type-checks |
| `lib.sh` | shared definitions |

The clients: `fixture` (`mf2-i18n-client`, as `cargo xtask b12-generated`
builds its A); `tr` and `tr-view` at 1,860 sites (the size workloads of
`cargo xtask size` and `b5 --view`, in their target directories); the three
demos (as A7 measured them, with A7's locks). The Leptos sources for the
silent paths were vendored into `target/a9/vendor*` with
`cargo vendor --offline`.

Never edit these scripts in place while one runs: bash reads a script as
it goes. Write a new file and rename it over the old one (a running bash
keeps the old one).
