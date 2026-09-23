# Phase 5b results — macros (layer L5)

What Phase 5b built and what it measured, against
[13-phase-5b-work-order](13-phase-5b-work-order.md). Every figure here comes
with the command that produced it; where a figure moved a budget or a ledger
status, the commit that moved it says why (master plan §11).

## Status at exit

| Exit criterion | Verdict |
|---|---|
| L5 100 % all features, every L5d cell recorded, none `xfail` | **met** — L5 485/485, L5d 416/485 with 69 documented degradations, the same 69 tests as L4d |
| the `trybuild` compile-fail set green | **met** — seven cases, every `.stderr` reviewed |
| B5 met on the 2,000-site build | *(A6 — filled in below)* |
| rust-analyzer expands the macro; macro overhead within P0.9's threshold | *(A7)* |
| owner question 1 answered and recorded in 04 §2 | **met** — [04](04-leptos-integration.md) §2.1 |
| `b12-generated` green as a gate | **met** — B1′ = +0 B, B13 = +13,573 B, in CI (§A10) |
| results and the Phase 6 work order written | *(this file; A9)* |

## A1 — the call-site types

[04](04-leptos-integration.md) §2.1 is the API as built; the code is
`crates/mf2/src/{tr,arg,dynamic}.rs`, under `mf2-runtime`'s client-path
discipline (`no_std`, `forbid(unsafe_code)`, no `core::fmt`, no panicking
operation), because a call site is what 2,000 of them are made of.

| Type | Size | What it is |
|---|---:|---|
| `Tr` | 4 B, `Copy`, `const` | a `MsgId` and nothing else — a description can sit in a `const` table |
| `TrArgs` | one type at every arity | `MsgId` + up to four values inline, a boxed slice beyond |
| `TrRich` | `TrArgs` + handlers | one handler per markup name, keyed by the hash of that name |
| `TrDyn` | names at run time | the suite's deliberate mismatches, and tools with data-driven arguments |

**The one thing the owner's answer left open — the extension point — is a
trait, not the `Custom` variant.** `Custom` cannot carry a signal: every
`CustomValue` method hands back a borrow of `&self`, and a signal's value does
not exist until it is read, inside the observer that is formatting. Reading it
early defeats the point, and caching it behind a lock cannot hand out `&str`.
So `ArgSource::arg_value(&self) -> ArgValue`, called once per format, in
whatever reactive context is formatting — and `Custom` is left to do its own
job, which is how a call site passes a measure, a date or a value a custom
function downcasts.

Two smaller decisions, both in 04 §2.1: markup handlers are positional and
keyed by the FNV-1a 64 of the markup name, so **no markup name reaches the
wasm** (B6) and the macro — which has every markup name of the message —
rejects a message whose two names would collide; and handlers are **all or
none**, because a message's markup formats to parts with no handler at all
(which is what the suite's own markup tests assert at L5), while handling one
name and not its sibling is an oversight.

## A2 — `mf2-macros`

P0.9's design, on the real manifest. The generated `tr!` wrapper bakes in the
manifest's absolute path and hash; the macro reads it once per compiler
process, keyed by the path and verified against the baked hash on every hit,
and **reports a manifest that hashes to anything else rather than using it**.
The relocation fallback of P0.9 came with it: a target directory restored
elsewhere leaves a dead baked path, so the macro looks for the same
`build/<pkg>-<hash>/out/manifest.mf2m` under the profile directories rustc
received as `-L dependency=…` — and the hash check means a file found that way
can never be the wrong corpus. `Build::manifest_inline` bakes the bytes
instead, for builds whose target directory moves under them.

`MF2_MACRO_STATS=<file>` makes each rustc write what the macro cost it —
expansions, nanoseconds, manifest reads. That is how "a cache hit does not
re-read the manifest" is *measured* rather than asserted: an in-crate
assertion would depend on the order rustc expands macros in, which is rustc's
business (the first attempt read 0, because it expanded first).

**A footgun worth the plans** (05 §4): inside the i18n crate itself `tr!` must
be called unqualified. A `macro_export` macro that arrives through a macro
expansion — and `include_generated!` is one — cannot be named by an absolute
path in its own crate (rustc #52234). Every other crate writes
`my_app_i18n::tr!`.

`msg_id!` arrived with A3: `TrDyn` needs a `MsgId` and there was no checked
way to get one.

## A5 — the compile-fail set

`tools/i18n-fixture/tests/ui/`, seven cases, each `.stderr` read before it was
committed: unknown id (with its did-you-mean), missing argument, unknown
argument, duplicate argument, an unknown markup handler, handlers given for
one markup name and not its sibling, and a stale manifest. Writing them found
and fixed two bad diagnostics — a name that is neither a variable nor a markup
name now says so in both kinds, and a tie between two equally distant
candidates goes to the one of similar length (`kdb` is two edits from `kbd`
*and* from `b`).

The eighth case of the work order — a gated function with its feature off — is
not a `tr!` error but a build error, and it is tested where it happens:
`crates/mf2-build/tests/drift.rs` drives `Lint::GatedFunction` over the real
pipeline, and at L5d every one of those messages is a recorded
`build-reject` degradation with the file and line the build gave.

## A4 — layer L5

`conformance/l5/` — four i18n crates, one per locale the suite uses (en-US
458 tests, und 22, ar 4, fr 1), each built **by its own build script** from
the vendored suite. See the work order's "A4 as built" for why a build script
rather than a checked-in generator, why `mf2-conformance` depends on those
crates, and why L5d's kind for a gated function is `build-reject` rather than
L4d's `unknown-function`.

```
cargo xtask conformance-report
  → L5 485/485, L5d 416/485 (+69 documented degradations)
```

20 tests go through the dynamic path (`via = "dyn"`): their `params`
deliberately do not name the message's variables, which is the point of those
tests. The harness reports which ones, and `verify` now rejects a ledger that
says otherwise (`ViaMismatch`), so the mark cannot rot.

161 of en-US's tests have no call site at all, because the spec refuses the
message: their assertion is that **the build** refused it, with the right
kinds. For that a `mf2-build` diagnostic now carries the MF2 error kind it
reports, by the suite's name for it — the build's verdict is machine-checkable
instead of prose, in `--format json` too.

## A3 — dynamic named arguments

`mf2::TrDyn` over `Formatter::write_named`, with `msg_id!` for the id. It
lowers through the same path as the positional types — a source is read, a
date/time borrowed — and it is explicitly not the client path: names in the
wasm are what `tr!` exists to avoid.

## A6 — budget B5

`cargo xtask b5` — P0.1's method on the real crates. Two scales of the
reference workload, three applications each, built exactly as
[06](06-size-and-perf.md) §3 says (`wasm32-unknown-unknown`, profile
`wasm-release`, `wasm-bindgen`, `wasm-opt -Oz`, `gzip -9`), and the per-site
cost taken as the difference of the differences so that everything fixed
cancels:

```text
marginal = (Δ@3,720 − Δ@1,860) / (3,720 − 1,860),   Δ = tr − baseline
```

**What this measures, and what it does not.** Phase 5b has no `leptos-mf2`,
so a description cannot render itself yet: in every template — `tr` and both
baselines — each call site formats to a `String`. That is the 45 % of real
sites that need one anyway (06 §2), and the view positions cost the same on
both sides of the delta (tachys around a `String` leaf), so they cancel here
rather than being measured. P0.1 measured the whole mix at **24.5 B gz** with
a real `Tr` leaf and found the view positions' 46–102 B gz to be tachys', not
ours; Phase 6's A7 re-measures them with that leaf.

The baselines are the same shapes with the same positions: **`idlit`** builds
its `String` from a short per-site literal (so sites stay distinct, as they
are in a real application) and **`dummy`** uses one literal everywhere (which
lets the optimiser merge sites a real application keeps apart — P0.1 measured
it 11.4 B gz per site under `idlit`, which is why `idlit` is the baseline and
`dummy` only the bound).

## A10 — B1′ and B13 as a gate

`cargo xtask b12-generated`, in CI beside the other B12 checks. Phase 5a took
both figures by editing the fixture's corpus by hand and putting it back
([phase-5a-results](phase-5a-results.md) §A10); the corpus is now two cargo
features of the fixture, so a regression fails a command.

| Build | Corpus | Features | `.wasm` |
|---|---|---|---:|
| E | nothing a function crate could serve | `hydrate` | 349,161 |
| F | the same | `hydrate,fn-number,fn-datetime` | **349,161** |
| A | the fixture's own | `hydrate,fn-number` | 365,013 |
| B | A plus `:currency`, `:unit`, `:percent` | `hydrate,fn-number` | 378,586 |

**B1′ = F − E = +0 B**, byte-identical, with two whole function crates linked
and neither reachable from the generated registry — the same result Phase 5a
measured. **B13 = B − A = +13,573 B**: what a corpus that does not use the
measure functions does not pay. Phase 5a measured +13,599 on a slightly
different corpus A; 26 B apart, 0.2 %. The gate holds B1′ at exactly 0 and
B13 within 10 % of Phase 5a's figure, and prints both.
