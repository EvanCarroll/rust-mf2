# probes/audit — artifacts from the planning audit (2026-09-20)

Scratch code that produced the figures marked *audit* in `plans/`. Kept so that
Phase 0 starts from something that compiles instead of from prose. **None of
this is product code**: it is unreviewed, mostly untested, and each directory is
a standalone crate (own `[workspace]`, not a member of the root workspace).
Phase 0 reproduces every number in-tree, then deletes or archives this directory.

| Directory | What it is | Backs | Status |
|---|---|---|---|
| `tr-prototype/` | One concrete, non-generic `Tr { id: u32 }` implementing `Render`, `RenderHtml`, `AddAnyAttr`, `AttributeValue`, `From<Tr>` for `TextProp` and `Signal<String>`, against leptos **=0.8.20**. Client state is a thread-local catalog + one `ArcTrigger`; SSR reads context. | `plans/04-leptos-integration.md` §1–5; probes P0.1, P0.2 | type-checks for `--features ssr` (native) and `--features hydrate --target wasm32-unknown-unknown`. **Never run in a browser.** Uses effect-per-node (strategy A); the plan prefers a registry (strategy B). The per-call-site size scaffolding (2,000 generated call sites behind `v_*` features) was dropped; the feature names remain in `Cargo.toml` as a reminder of the variants measured. |
| `render-effect-bench/` | Allocation/latency benchmark of `RenderEffect` subscribers on one trigger, incl. the dropped-subscriber leak. | `plans/04` §4; probe P0.11 | native only |
| `ox-parser-bench/` | Throughput + allocation baseline of `ox_mf2_parser` =0.14.0-alpha.12. Run: `cargo run --release -- ../../../third_party/message-format-wg/test/tests` | the D1 gate in `plans/05-tooling.md` §1; probe P0.12 | produced the baseline table |
| `ox-conformance/` | Runs `ox_mf2_parser` over the WG suite, classifying syntax + data-model errors (460/462 exact). Same argument as above. | `plans/05` §1; the differential oracle | — |
| `plural-size/` | wasm size of plural selection four ways: `base`, `hand` (hand-rolled UTS #35 evaluator, ≈ 1.2 KB), `blob` (`icu_plurals` + `icu_provider_blob`), `compiled` (`icu_plurals` compiled data). | `plans/03-runtime.md` §5; probe P0.4 | `hand` is a **size probe only — correctness untested** |
| `plural-payload/` | Postcard size of per-locale plural rule data (5–188 B). | `plans/02-catalog-format.md` §4 | — |

Build profile used for all wasm sizes: `opt-level="z"`, fat LTO, `codegen-units=1`,
`panic="abort"`, `strip`, then `wasm-opt -Oz` (the per-call-site figures in
`plans/06` were taken *before* `wasm-opt`).
