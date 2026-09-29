# p10-checks — the whole check suite, quietly

    bash probes/p10-checks/run.sh LABEL [--against EARLIER] [--only CHECK,...] [--summarize]
    bash probes/p10-checks/compare.sh EARLIER LABEL

`run.sh` runs the existing checks one at a time, `ci` first, and carries on after a failure: ci; sizes (`probes/p10-b2/measure.sh`) and demos (`demo-hashes.sh`); docs, docs-rs, codegen-matrix, scenarios, msrv, leptos-0-8, churn, l6-web, l7-web; e2e-0-9 and e2e-0-8 (`probes/p10-b1/e2e.sh`); b12, b12-generated, conformance-report (the xtask has no `--check`: REPORT.md and COVERAGE.md must come out unchanged), api `--check`, refusals, tui-gate.

Logs: `target/p10-checks/LABEL/CHECK.log` (measure.sh and e2e.sh also keep theirs under `target/p10-b2/` and `target/p10-b1/e2e/`). stdout: one line per check, then the table, also written as `summary.tsv`. Exit 1 if any check failed. `--summarize` rebuilds the table from the logs.

The TUI gate keeps its binaries in `LABEL/tui-baseline`; `--against EARLIER` alternates them with EARLIER's (`base_us`: EARLIER's binary in the same session) and counts the demo files that changed.

Figures: `b1`, `app` (B gz); `b5`, `b5v` (B gz a site); `b7.LOCALE` (catalog, B brotli); `files`/`changed` (demo files); `tui` (stripped tui-mf2, B); `allocs` (per frame, en/de/es/fr); `us` (median µs a frame); `tests`, `asserts` (passed); `crates`, `rlib`, `L7d`/`L7cd`, `b1p`/`b13`.

`compare.sh` flags a check failing in LABEL; B1 beyond ±64 B gz; B5 beyond ±0.2 B a site; any other size that moved; demo files that changed (demo-ssr's `__wasm_split` loader aside: its hash varies between identical builds); `tui` above 1,965,320 B; `allocs` that changed. Frame time is shown, not judged (taken under load). Exit 1 if anything is flagged.
