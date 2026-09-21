# mf2-two

Unicode MessageFormat 2 (MF2) for [Leptos](https://leptos.dev): the full
specification, lazily loaded per-locale binary catalogs, and a minimal wasm.

**Status: Phase 2 (the binary catalog) is next.** Phase 1 delivered the
syntax and the data model — [`crates/mf2-model`](crates/mf2-model) (the
interchange data model, `no_std`) and [`crates/mf2-syntax`](crates/mf2-syntax)
(a lossless-CST parser with recovery, lowering, validation, serializer and
analysis), passing the WG suite at layers L1 (462/462) and L2 (326/326) and
3–16× faster than the existing Rust parser ([results](plans/phase-1-results.md)).
There is no usable Leptos library yet.

* Plans and decisions: [`plans/`](plans/README.md) — start with
  [`plans/00-master-plan.md`](plans/00-master-plan.md).
* Current work order: [`plans/09-phase-2-work-order.md`](plans/09-phase-2-work-order.md).
* Conformance: [`conformance/REPORT.md`](conformance/REPORT.md).
* Vendored, pinned inputs (read-only): [`third_party/`](third_party/).

## Developing

```sh
cargo check --workspace        # the toolchain is pinned in rust-toolchain.toml
cargo xtask ci                 # everything CI runs, locally
cargo run --release -p parser-gate -- --gate    # the D1 parser gate (CI job `parser-gate`)
```

## License

MIT — see [`LICENSE`](LICENSE). Vendored material under `third_party/` keeps its
own license (Unicode License v3), stated in each directory.
