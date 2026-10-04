# bench/churn — the conversions under churn

Phase 7 A5: P0.11's churning list, built
on `mf2`'s Leptos layer (`mf2::leptos`) itself. One row shape per variant — a
text, a text and an attribute, a signal-valued argument, a `TextProp` prop, a
`Signal<String>` prop, `to_string()` in a closure, an `Oco` prop — each
measured on a fresh page: 2,000 live rows, then 100,000 rows built, mounted,
unmounted and dropped, 50 per round under a round owner, with a counting
allocator reporting the live heap. `src/lib.rs` has the variants' table.

```sh
cargo xtask churn                      # build, then run in Chromium and Firefox
cargo xtask churn --no-build --browser chromium
```

`cargo xtask churn` builds the harness for `wasm32-unknown-unknown` in
release (`opt-level = "z"`, fat LTO), binds it with `wasm-bindgen`, publishes
the catalogs with `mf2 compile --site`, and runs
`tools/e2e/checks/churn.mjs`, which serves `target/churn/site/` and fails if
any shape grows the heap by more than 64 KiB over the 100,000 rows. The
figures go to `target/churn/report.json`.

A workspace of its own, like the examples: it is a `csr` application, and
the root workspace builds `mf2`'s Leptos layer with `ssr`. The native half of
the same guard — no DOM, the three conversions and a plain-`track()` control —
is `crates/mf2/tests/churn.rs`, in `cargo xtask ci`.
