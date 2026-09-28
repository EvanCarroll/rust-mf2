# p10-display — Phase 10 A5: `Display` / `Debug` against B12

A probe (`plans/18-phase-10-work-order.md`, A5), deleted at Phase 10's exit;
its result is A5's task record in that file.

It was run on the probe branch `p10-a5-display` (one commit, `70e0b27`, on
`2fb7f54`), which is not merged. The variant is kept here as
`display.patch` (`git apply probes/p10-display/display.patch` at `2fb7f54`);
the scripts that measured it expect to run from the root of a tree with the
patch applied, and write under `target/a5/`:

| Script | What |
|---|---|
| `measure.sh <label>` | `cargo xtask size`, `b5 --view`, `b12-generated`, `bench/b12/check.sh`, keeping every wasm |
| `named.sh <label>` | symbol-kept builds of the measured applications, for twiggy |
| `twiggy.sh`, `ours.sh` | twiggy over them: `Display` / `Debug` / `core::fmt`, and our own items |
| `clippy-variant.sh` | CI's clippy steps on the variant |
| `control.sh`, `control-stripped.sh` | the positive control: one `{}` / `{:?}` in a client, which the checks must find |
| `analysis/*.py` | the function-body and size comparisons behind the record's −83 B explanation |
