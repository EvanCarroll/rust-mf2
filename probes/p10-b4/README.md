# p10-b4 — Phase 10 B4: the scripts behind B4's size figures

Not a probe of an idea but the measurement of a change: B4, the repository's
own users of the library naming `mf2` in place of the shims
(`plans/18-phase-10-work-order.md`, B4's record). Deleted at Phase 10's exit
with the other `probes/p10-*`; the figures stay in the record.

B4 changes two of the templates the size commands generate applications
from (`tr-view`, which `cargo xtask b5 --view` measures, and
`fluent-converted`), and no library code. B2's `probes/p10-b2/measure.sh`
keeps each generated application's `Cargo.lock` between runs (`--keep`),
but `--keep` also reuses the generated workload as it stands, so on its own
it measures the applications generated before the change. B4's runs, in the
main tree:

1. `bash probes/p10-b2/measure.sh b4-base` at `7cf9790`, before any change;
2. with `7cf9790`'s `tr-view` put back, `bash probes/p10-b4/regen.sh b4-head`:
   every file as it was, so step 1 measured `7cf9790`'s templates;
3. with B4's templates, `bash probes/p10-b4/regen.sh b4-regen`, then
   `bash probes/p10-b2/measure.sh b4-regen`.

| Script | What |
|---|---|
| `regen.sh LABEL` | the kept workloads (`target/p10-b2/size/wl-{1860,3720}`, `target/p10-b2/b5v/wl-view-{1860,3720}`) generated again in place from the tree's templates, with the knobs and the template order `cargo xtask size` and `cargo xtask b5 --view` use. The generator replaces only what it writes (each application's manifest and sources), so every application keeps its `Cargo.lock` and its build directory. Each workload's files but its build output are hashed before and after, and the files that changed are listed: `target/p10-b2/logs/LABEL/` |
| `lockback.py WORKLOAD APP HASHES` | whether APP's lock lost `leptos-mf2` and nothing else: the lock after the build, with `leptos-mf2`'s entry and the line listing it put back, must have the hash `regen.sh` recorded before (HASHES, its `<workload>.before.sha256`) |
| `wasmcmp.py BEFORE AFTER` | two builds of one wasm module compared section by section: sizes, and what differs (import and export names, function bodies, data bytes) |
