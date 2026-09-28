# plans/

Planning documents for **rust-mf2**. Start with the master plan; it is the single
source of truth and links to everything else.

| Order | Document | Read it when |
|---|---|---|
| 0 | [00-master-plan.md](00-master-plan.md) | always, first — goals, assessment, layout, feature flags, decisions, phases, risks |
| 1 | [01-conformance.md](01-conformance.md) | touching anything that parses, encodes, formats or renders a message |
| 2 | [02-catalog-format.md](02-catalog-format.md) | working on `mf2-catalog`, `mf2-build`, or the runtime's reader |
| 3 | [03-runtime.md](03-runtime.md) | working on `mf2-runtime`, `mf2-fn-*`, `mf2-host-*` |
| 4 | [04-leptos-integration.md](04-leptos-integration.md) | working on `leptos-mf2`, `mf2-macros`, `mf2-axum`, examples |
| 5 | [05-tooling.md](05-tooling.md) | working on `mf2-syntax`, `mf2-resource`, `mf2-build`, `mf2-cli`; the parser performance gate; repository conventions |
| 6 | [06-size-and-perf.md](06-size-and-perf.md) | budgets, the reference workload, Phase 0 probes, the size gate |
| 7 | [07-phase-0-work-order.md](07-phase-0-work-order.md) | Phase 0 tasks, dependencies, exit checklist (done) |
| — | [phase-0-results.md](phase-0-results.md) | the Phase 0 measurements behind every figure in the plans, the go/no-go recommendation |
| 8 | [08-phase-1-work-order.md](08-phase-1-work-order.md) | Phase 1 tasks (done), the frozen `mf2-model` types, the `Frontend` trait, status at exit |
| — | [phase-1-results.md](phase-1-results.md) | the Phase 1 measurements: L1/L2, the D1 gate, generated input, the ox differential, fuzzing |
| 9 | [09-phase-2-work-order.md](09-phase-2-work-order.md) | Phase 2 tasks (done), the frozen `mf2-catalog` API, status at exit |
| — | [phase-2-results.md](phase-2-results.md) | the Phase 2 measurements: format v1, L3, B7 sizes, reader cost and B12, fuzzing, the linear-time test |
| 10 | [10-phase-3-work-order.md](10-phase-3-work-order.md) | Phase 3 tasks (done), the frozen `mf2-runtime` API, status at exit |
| — | [phase-3-results.md](phase-3-results.md) | the Phase 3 measurements: L4, the numeric A/B (D15), plural samples, generated input, fuzzing, B1/B10/B12/B13 |
| 11 | [11-phase-4-work-order.md](11-phase-4-work-order.md) | Phase 4 tasks (done): function families, locale data, layer L4 (all files), the `intl` client option; status at exit |
| — | [phase-4-results.md](phase-4-results.md) | the Phase 4 measurements: the `intl` probe and the option as built, the number and date data, `:currency` / `:unit`, the date backends, L4d, goldens, generated input and fuzzing, numeric speed, B2–B4, B8, B12, B13, B1′ |
| 12 | [12-phase-5a-work-order.md](12-phase-5a-work-order.md) | Phase 5a tasks (done): `mf2-resource`, `mf2-build`, the generated module, `mf2 check`, `mf2-cli`; status at exit |
| — | [phase-5a-results.md](phase-5a-results.md) | the Phase 5a measurements: the pipeline on the reference workload, the `compile_str` differential, the seeded-drift corpus, the edit scenarios, B7 and B8 on the build's own catalogs, build cost, and both owner questions answered |
| 13 | [13-phase-5b-work-order.md](13-phase-5b-work-order.md) | Phase 5b tasks (done): `mf2-macros`, the call-site types, layer L5, and what A4 decided in the doing |
| — | [phase-5b-results.md](phase-5b-results.md) | the Phase 5b measurements: the call-site types as built, the macro and its cache, the compile-fail set, layer L5 and L5 on generated input, B5, the macro's cost, B1′ and B13 as gates |
| 14 | [14-phase-6-work-order.md](14-phase-6-work-order.md) | Phase 6 tasks (done): `leptos-mf2`, `mf2-axum`, layer L6 (SSR + hydrate) |
| — | [phase-6-results.md](phase-6-results.md) | the Phase 6 measurements: the crate and why the description types live in it, rendering and hydration, the registry and the switch, `mf2-axum`, layer L6 in Rust and in two engines, the example, the size gate, and B5 on the whole mix |
| 15 | [15-phase-7-work-order.md](15-phase-7-work-order.md) | Phase 7 tasks (done): islands, CSR, lazy routes, layer L7, the dev loop, the WCAG audit, spec coverage, documentation, `mark-fallback-lang` |
| — | [phase-7-results.md](phase-7-results.md) | the Phase 7 record: status at exit, the WCAG 2.2 AA audit, the coverage matrix, the user documentation, `mark-fallback-lang` (A1–A9 are recorded in the work order) |
| 16 | [16-phase-8-work-order.md](16-phase-8-work-order.md) | Phase 8 tasks (done): `mf2 convert --from fluent`, the call-site codemod and guide, XLIFF 2, the `leptos-fluent` A/B, the reader's time zone |
| — | [phase-8-results.md](phase-8-results.md) | the Phase 8 record: status at exit, the A/B and oracle figures, what the phase found and did not fix (tasks recorded in the work order) |
| 17 | [17-phase-9-work-order.md](17-phase-9-work-order.md) | Phase 9 tasks (done): the release — 1.0.0 and 1.1.0, the specification text out of the tree, the API review, the version policy, packaging, docs.rs, `cargo xtask release`; Part B, the book verified by running it |
| — | [phase-9-results.md](phase-9-results.md) | the Phase 9 record: status at exit, the release figures, the book's re-verification, what the phase found and did not fix, and the post-1.0 order of the "Later" list |
| — | [stretch_goals_after_v1/](stretch_goals_after_v1/) | ideas deferred until after v1, each with what was verified, the seams v1 keeps, and how to re-evaluate: [catalog text as JS strings](stretch_goals_after_v1/prob_builtin_strings.md), [narrower NAMES references](stretch_goals_after_v1/names_reference_width.md) |

## Rules for anyone (or any agent) working from these plans

1. **Stay inside the repository.** The boundary and network rule is stated once,
   in the root `CLAUDE.md` ("Boundary"); it applies to everything here. The spec and the test suite are in the tree; the CLDR
   inputs and the reference workload are produced by Phase 0 tasks A4 and A7. If a requirement seems
   missing, ask the owner.
2. **The master plan wins.** If a companion document disagrees with it, fix the
   companion — or change the master plan deliberately, in the same commit.
3. **Full MF2, always.** No layer may subset the spec. A test that cannot pass yet
   is an `xfail` in the ledger with the phase that fixes it, never a skip.
4. **Numbers, not adjectives.** Size and speed claims come with a measurement and
   the command that produced it.
5. Figures in these plans were measured in-tree by Phase 0 (2026-09-20/21);
   [phase-0-results.md](phase-0-results.md) gives each one's method and command.
   Where a planning-audit figure did not reproduce, the text says so.
