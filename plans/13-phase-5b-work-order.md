# 13 — Phase 5b work order: macros (layer L5)

Part of the [master plan](00-master-plan.md) (§9, P5b). RFC 2119 keywords
apply. Written at the close of Phase 4 from [phase-4-results](phase-4-results.md)
and the Phase 0 probes P0.1 (call sites) and P0.9 (build orchestration); it
needs Phase 5a's pipeline ([12](12-phase-5a-work-order.md)) and MAY be revised
at Phase 5a's close. The call-site API is specified in
[04-leptos-integration](04-leptos-integration.md) §2, the macro's contract in
[05-tooling](05-tooling.md) §4, layer L5 in [01-conformance](01-conformance.md)
§3; this document orders the work and sets the exit criteria.

Phase 5b adds `tr!`: the proc-macro that checks a call site against the
manifest at compile time and lowers it to a positional description of the
message (`MsgId` + arguments, no names, no strings), and the facade glue that
makes the generated i18n module usable from any crate. It turns **L5** green —
the whole suite through `tr!`, in both feature configurations — and holds the
per-call-site budget B5 on a 2,000-site build.

## State at the start (expected at Phase 5a's exit)

| In the tree | Where |
|---|---|
| Everything of Phases 1–4: parser, catalog, runtime and function crates, L1–L4 green (L4d recorded) | `crates/`, `conformance/` |
| `mf2-resource`, the loaders, `mf2-build` (manifest, flattened stripped catalogs with sliced LOCALE entries, the generated module with the `tr!` wrapper baked with the manifest path and hash), `mf2-cli`, `mf2 check` | Phase 5a |
| The `mf2-build` ≡ `compile_str` differential over the suite, both configurations | Phase 5a A4 |
| P0.1's call-site library and measurement method (24.5 B gz per site; 35.7 against the `dummy` bound), P0.9's macro, cache and relocation fallback, its timing and rust-analyzer scripts | `git show b15b6d6:probes/p0-01-call-site/…`, `…/p0-09-build-orchestration/…` |
| `bench/workload-gen`'s app generator (`literal` and `closure` templates, `sites.json`) | `bench/workload-gen` |

## What Phase 4 (and 5a) change here

* **The default configuration is a build outcome at L5.** L4d records, test
  by test, how the default-features runtime degrades (`unknown-function`,
  `unsupported-operation`, `neutral-numbers`). At L5d the same tests MUST
  show the *build's* verdict: a gated function rejects the corpus (Phase 5a's
  error, file and line), neutral numbers warn and then format exactly as L4d
  recorded. A `degraded` L5d cell names the same kind as its L4d cell, and
  the ledger checker SHOULD require that.
* **Arguments carry Phase 4's value kinds.** `Arg::DateTime` (instant or
  floating, optional zone) and measures from application values
  (`CustomValue::as_measure`) must be expressible at a call site; the suite's
  date tests pass date strings, but the macro's conversion (`ArgValue::from`)
  MUST cover every `Arg` variant.
* **Registry and host come from the generated module.** `tr!` never names a
  function or a host; whatever the corpus uses is already in `registry()`
  (Phase 5a A5), so L5 exercises the closed world end to end — an L5 test
  that passes only because the harness registered a handler by hand is a
  harness bug.
* **The `intl` option** changes nothing in the macro: it is a client runtime
  choice. L5 runs natively (as L4's native run), where the Rust path is used.

## Owner question (answered)

1. **Where the call-site types live** — **answered** (owner, 2026-09-23):
   a **Leptos-free core in the facade**. `mf2::Tr`, `mf2::TrArgs`,
   `mf2::TrRich` and `mf2::ArgValue`, formatting against a catalog the
   caller supplies, so a server, a test and `mf2-cli` need no Leptos.
   `leptos-mf2` (Phase 6) *adds* rendering, the catalog context and the
   reactive argument, and does not re-declare `ArgValue`: `Reactive` is not
   a variant of the core enum, and the signal arrives through the
   extension point. Recorded in [04](04-leptos-integration.md) §2.
   **A1 still decides one thing**: whether that extension point is the
   `Custom` variant or a trait — and records it in 04 §2.

## Part A — tasks (A1 first; A2–A5 in order; A6–A10 as their inputs exist)

| Task | Deliverable | Done when |
|---|---|---|
| **A1** Call-site types and the expansion contract | Owner question 1 settled; the types' API written into 04 §2 (and the facade's in 05 §9) before code: `Tr` (4 bytes, `Copy`, `const fn tr`), `TrArgs` (one concrete type, up to 4 arguments inline, a boxed spill beyond), `TrRich` (type-erased markup handlers), `ArgValue` and its `From` conversions for every `Arg` variant; `include_generated!` and the `__mf2` path. | 04 §2 and 05 §9 describe the built API; the expansion table of 04 §2 is final |
| **A2** `mf2-macros` | The proc-macro behind the generated wrapper (`__tr_impl!("<path>" 0x<hash>u64 ; $crate ; …)`, 05 §4): the manifest read once per compiler process, keyed by path and verified against the baked hash (a stale manifest is reported, not used); the relocation fallback through the `-L dependency=…` directories; the opt-in inline mode. Checks: the id exists (did-you-mean), the argument names equal the message's variables (unknown, missing, duplicate), every markup name has a handler, raw-string names for MF2 names that are not Rust identifiers (05 §3). Emits the positional construction spanned at the id literal; several errors wrapped in one block. | every check has a passing and a failing test; a cache hit does not re-read the manifest (measured) |
| **A3** Dynamic named arguments | The API the suite's `dyn` tests need (01 §3: `params` that deliberately mismatch the message) over `Formatter::write_named` / `parts_named` — no macro; ledger cells marked `dyn`. | the suite's mismatched-argument tests run through it at L5 |
| **A4** L5 harness | Generated i18n crates, one per locale the suite uses (`en-US`, `und`, `fr`, `ar`, each single-locale), built by `mf2-build`; id `suite.<file>.t<index>`, the file's *path* dotted so that `extra/functions/unit.json` cannot collide with `functions/unit.json`; every suite test driven through `tr!` with its `params`, asserting L4's `exp`, `expParts`, `expErrors`; syntax- and data-model-error tests asserting that `mf2-build` rejects the message with the expected kind. `conformance/extra/` tests included. **Built by each crate's own `build.rs`, not by an `xtask` command and not checked in** (see below). L5 and L5d ledger columns; `cargo xtask conformance-report` counts them. | L5 485/485 all features; every L5d cell `pass` or `degraded` with its kind, none `xfail` |
| **A5** `trybuild` compile-fail set | Unknown id (with its did-you-mean), missing argument, extra argument, duplicate argument, unknown markup handler, a stale manifest, a gated function with its feature off (the build error surfaces through `cargo build`). | the set passes on stable; its `.stderr` files reviewed |
| **A6** B5 | A 2,000-site synthetic application (`workload-gen`'s app generator over the reference workload) built for `wasm32-unknown-unknown`, the per-site marginal measured by P0.1's method against its `dummy` bound. | B5 ≤ 40 B gz per site (P0.1: 24.5), or restated with the owner |
| **A7** Macro cost and tooling | Macro time and `cargo check` overhead for 2,000 expansions against P0.9 (0.12–0.45 s macro time, +0.2–0.3 s `cargo check`); rust-analyzer expands `tr!` and reports its errors at the id (P0.9's `ra-check.sh`); the relocation and inline-mode scenarios re-run on the real crates. | within P0.9's figures, or the difference explained and accepted by the owner |
| **A8** Generated input | L5 on generated messages: `l4gen`'s generator writes a corpus of generated messages into one generated crate whose tests call `tr!` and compare with L4's runner on the same catalog. | a generated corpus of ≥ 10,000 messages clean |
| **A9** The Phase 6 work order | Written from Phase 5b's findings (call-site types, what the macro emits, B5's measured figure) into `plans/14-phase-6-work-order.md`. | written |
| **A10** `b12-generated` (inherited from Phase 5a) | A `bench/b12` harness pair driven by the **generated** module rather than a hand-written registry: the same corpus with and without a feature it does not use, and with and without a gated function. Phase 5a measured B1′ = +0 B and B13 = 13,599 B avoided by editing `tools/i18n-fixture`'s corpus by hand ([phase-5a-results](phase-5a-results.md) §A10); this turns those one-offs into a gate. | both deltas reproduce Phase 5a's figures and CI fails on a regression |

### A4 as built (2026-09-23)

Three things were decided in the doing, and the rows above say what was
decided; here is why.

* **A build script, not an `xtask` command with checked-in output.** The
  alternative was a generator whose output is committed and a CI check that
  regenerating changes nothing. A `build.rs` that reads the vendored suite
  removes the drift instead of policing it: `cargo xtask spec-sync` is
  carried through by the next build, and nothing in the tree can disagree
  with `third_party/`. The cost is that `cargo test -p mf2-conformance`
  builds four corpora; it is seconds, and `cargo test --workspace` already
  did the same work for the fixture.
* **`mf2-conformance` depends on the four L5 crates** (`conformance/l5/`),
  so `Harness` runs L5 in the same process as every other layer and
  `conformance-report --promote` and `verify` hold its cells exactly as they
  hold L4's. The library therefore reaches the build pipeline through their
  build scripts, which is a departure from "nothing in the conformance
  library depends on the build pipeline" — but L5 *is* the build pipeline,
  and the alternative (an out-of-process run, like `l4-wasi`) would have left
  `HARNESSED` unable to include L5 and the ledger unable to claim it.
* **L5d's kind for a gated function is `build-reject`, not L4d's
  `unknown-function`.** The work order said an L5d cell "names the same kind
  as its L4d cell"; that cannot be, and the ledger's own model already knew
  it — `DegradedKind::BuildReject` is documented "the build refuses the
  corpus (L5d)". At L4d a gated function is a run-time *Unknown Function*; at
  L5d the build stops first, with its file and line, which is the stronger
  statement and the one the plans promise (05 §5). Every other L5d kind is
  L4d's, classified by the same code (`l4::classify_default`).

## Exit (master plan §9, P5b)

- [x] L5 100 % in the all-features configuration (the suite and
      `conformance/extra/`), and every L5d cell recorded — `pass`, or
      `degraded` with the build's verdict — none `xfail`;
      `current_phase = "P5b"` in the exit commit with the harness green
      — **L5 485/485, L5d 416/485 + 69 documented degradations**
- [x] the `trybuild` compile-fail set green — **seven cases**
- [x] B5 met on the 2,000-site build (or restated with the owner) —
      **12.6 B gz per site against `idlit`, 34.0 against the `dummy` bound**,
      on the description and the `String` path; the view positions are
      Phase 6's, and [phase-5b-results](phase-5b-results.md) §A6 says why
- [x] rust-analyzer expands the macro; macro overhead within P0.9's threshold
      — **0.122 s per 2,000 expansions, one manifest read**, `cargo check`
      inside the noise; rust-analyzer reports the macro's own errors at the id
- [x] owner question 1 answered and recorded in 04 §2
- [x] `b12-generated` green as a gate, reproducing Phase 5a's B1′ = +0 B and
      B13 = 13,599 B (A10) — **+0 B and +13,573 B**
- [x] `plans/phase-5b-results.md` and the Phase 6 work order written

**A8 (generated input at L5) is not an exit item and is not done**; the state
it is in is [phase-5b-results](phase-5b-results.md) §A8.
