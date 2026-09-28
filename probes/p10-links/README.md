# Phase 10 A2 probe — `links` metadata

Can `mf2` carry its own features to the build script of a crate that
depends on it, so that no translation crate declares or forwards features
(`plans/18-phase-10-work-order.md` A2; D19)? The result is recorded in
the work order's A2 section. Deleted at Phase 10's exit (G4).

| Directory | What it is |
|---|---|
| `mf2/` | the stand-in `mf2` (`p10-links-mf2`): `links = "mf2-v2"`; `build.rs` prints `cargo::metadata=features=…`; the cfg-forwarding macros `__if_ssr!` / `__if_not_ssr!` / `__if_hydrate!` / `__use_host!` |
| `mf2-build/` | the stand-in `mf2-build`: `run()` reads `DEP_MF2_V2_FEATURES` (else the crate's own `CARGO_FEATURE_*`) and writes a generated module whose `report()` compares the build script's view with the compiled one |
| `fixture/` | a translation crate `mf2`'s own test depends on (the dev-dependency cycle) |
| `single/` | a one-crate application |
| `two-crate/` | an application, a translation crate with no features, and a second dependent using the macros |
| `host-target/` | `mf2` as a normal and a build dependency with different features; and as a build dependency only |
| `leptos-app/` | cargo-leptos's two builds (no Leptos crate: the probe is about the features) |
| `ra/`, `ra-control/` | rust-analyzer, and a plain crate as the control for its CLI |
| `dup/` | two packages with `links = "mf2-v2"`; two versions; the next major (`mf2-v3`); `links` without a build script |

Every scenario is a workspace of its own. Set `P10_RUN_LOG=<file>` to have
each run of the stand-in build script append a line (package, target,
source, features, `OUT_DIR`); `cargo build -v` shows which units run.

```sh
cd single     && cargo build -v --features ssr && ./target/debug/p10-links-single
cd two-crate  && cargo build -v -p p10-links-app --features ssr && ./target/debug/p10-links-app
cd host-target && cargo build -v -p p10-links-both --target wasm32-unknown-unknown
cd leptos-app && cargo leptos build && cargo leptos build   # the second does nothing
cd ra         && rm -rf target && rust-analyzer analysis-stats .
cd two-crate  && cargo metadata --format-version 1 --features p10-links-app/ssr
cd dup/app-copy && cargo build                              # the duplicate-`links` error
cargo package -p p10-links-mf2 --allow-dirty && cargo publish --dry-run -p p10-links-mf2 --allow-dirty
cargo test -p p10-links-mf2 --test generated --features ssr
```

`dup/mf2-nobuild/` has an invalid manifest on purpose.
