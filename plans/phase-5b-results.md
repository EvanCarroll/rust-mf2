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
| B5 met on the 2,000-site build | **met** — 12.6 B gz per call site against `idlit`, 34.0 against the `dummy` bound, budget ≤ 40 (§A6) |
| rust-analyzer expands the macro; macro overhead within P0.9's threshold | **met** — 0.122 s per 2,000 expansions, one manifest read, `cargo check` inside the noise; rust-analyzer expands every call site and reports the macro's own errors at the id (§A7) |
| owner question 1 answered and recorded in 04 §2 | **met** — [04](04-leptos-integration.md) §2.1 |
| `b12-generated` green as a gate | **met** — B1′ = +0 B, B13 = +13,573 B, in CI (§A10) |
| results and the Phase 6 work order written | **met** — this file and [14](14-phase-6-work-order.md) |

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

```
cargo xtask b5
```

| workload | template | bindgen gz | opt raw | opt gz |
|---|---|---:|---:|---:|
| 1,860 sites | **tr** | 703,857 | 2,711,962 | 730,765 |
| 1,860 sites | idlit | 661,117 | 2,711,750 | 685,422 |
| 1,860 sites | dummy | 627,889 | 2,443,553 | 643,781 |
| 3,720 sites | **tr** | 1,216,124 | 5,054,309 | 1,280,448 |
| 3,720 sites | idlit | 1,151,813 | 5,099,586 | 1,211,659 |
| 3,720 sites | dummy | 1,090,423 | 4,553,491 | 1,130,174 |

| baseline | marginal B gz/site | fixed B gz | budget |
|---|---:|---:|---|
| **`idlit`** | **12.6** | 21,897 | ≤ 40 — **met** |
| `dummy` | 34.0 | 23,694 | (the bound) |

**Read these two numbers together.** The `dummy` bound, 34.0, is directly
comparable to P0.1's **35.7** — `dummy` means the same thing in both, and the
two agree within 5 % on entirely different code, which is the corroboration
that the method carried over. The `idlit` figure is *lower* than P0.1's 24.5
because the baseline is not the same one: P0.1's `idlit` put a `&'static str`
in the view positions, so its delta also carried what tachys costs around a
`String` leaf, while this one produces a `String` in the same positions as
`tr` does, and that cost cancels. What is left — 12.6 B gz — is the
description, its lowering and the call into the formatter, and nothing else.

Both are within the budget with room, and the fixed part (21.9 KB gz: the
runtime, the reader, the call-site library and the generic instantiations)
belongs to B1, where 06 §3's whole-app ambition accounts for it. Phase 6's A7
re-measures the full mix with the real leaf, which is where P0.1's 24.5 sits.

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

## A7 — what the macro costs, and what an editor does with it

Measured on the 1,860-site application `cargo xtask b5` generates — the same
app, the same corpus, the same crates — against **`direct`**, a template that
writes out exactly what `tr!` expands to. The two apps differ in one thing:
the macro. (P0.9 used the same control; the scripts are one-off, the
templates are in the tree.)

| | Phase 5b | P0.9's threshold |
|---|---|---|
| The macro's own time, 1,860 expansions | **0.114 s** (0.122 s per 2,000) | 0.12–0.45 s per 2,000 |
| Manifest reads for those 1,860 expansions | **1** | 1 |
| `cargo check`, warm, every source touched, against `direct` | **+0.11 s** median, **−0.31 s** by minimum — inside this machine's run-to-run noise | +0.2–0.3 s |
| The same with the manifest inlined | +0.045 s over the path mode | +0.5 s per 2,000 |

**The first measurement found a defect.** The macro started at **0.839 s** per
1,860 expansions — 0.90 s per 2,000, twice P0.9's upper figure. Timing an
expansion that parsed but did nothing else (99 ms) placed the cost in the
cache, and it was the hit itself: `map.get(&key)` then `m.hash() == hash`,
where `Manifest::hash` **re-serializes the whole corpus** to compute it. Every
call site was re-hashing a 1,600-message manifest. The cache now stores the
hash it verified when it read the file, so a hit is a `u64` comparison, and
the same measurement gives 0.114 s — 7× faster, and barely above the
parse-only floor.

The inline mode is ten times cheaper than P0.9's figure for it because the
manifest literal is kept as the compiler handed it over and only turned into
bytes on a cache miss; P0.9's amendment assumed it would be stringified per
site.

**rust-analyzer.** `rust-analyzer diagnostics .` on the 200-site `tr`
application: **no** `unresolved-macro-call`, `unresolved-proc-macro` or
`macro-error` — the build script ran, the baked path resolved, the
proc-macro server expanded every call site. The negative control (a misspelt
id, and an argument the message does not have) is reported as
``unknown message id `this-id-does-not-exist` `` **at the call site**, which
is the whole point of doing the checking in the macro.

On *this* repository the same command found four `macro-error`s — and they
were ours, though not `tr!`'s: each L5 crate reached its shared source with
`include!("../../shared.rs")`, and rust-analyzer will not load a file outside
the crate's own directory. The generator now writes that file into `OUT_DIR`
beside the others, and the four are gone. What remains are 57
`unresolved-macro-call`s in `mf2-locale-data`, every one of them an ICU4X
baked-data macro (`icu_datetime_data::impl_datetime_names_*!`) — a
rust-analyzer limitation that predates this phase and has nothing to do with
`tr!`.

**Relocation.** The fallback of P0.9 — look for the same
`build/<pkg>-<hash>/out/manifest.mf2m` under the `-L dependency=…`
directories — could not be provoked through cargo 1.98: moving the target
directory, copying it and deleting the original, and hand-editing the baked
path to another machine's all made cargo **rerun the i18n crate's build
script**, which writes the path afresh. So on this cargo the stale-path case
does not arise from a relocated target directory; the fallback is for an
environment that *restores* build-script outputs without rerunning them
(remote execution, a build cache), which this machine cannot reproduce. It is
now unit-tested directly instead (`relocated_in`), both argument spellings,
with the hash check still the thing that makes a wrong file impossible.

## A8 — generated input at L5 (not done)

Not an exit item, and not finished. What is in place for it: `l4gen` now
hands back a generated message *without* compiling it
(`l4gen::message` → `GeneratedMessage`: the source, whether the spec accepts
it, the locale and bidi the seed chose, and the arguments), and the L5
generator's corpus-and-call-sites half is a public function
(`mf2_l5_gen::build`) that takes messages from anywhere, not only the suite.
What is left is the crate that feeds one into the other and the test that
compares each generated call site with L4's runner on the same message.

Two things that crate has to decide, both written down here so the next
session does not rediscover them:

* **One locale.** A corpus is one source locale, and `l4gen` picks a locale
  per seed out of eighteen. The corpus should be built in a single locale
  (`en`) and the L4 side compiled in the same one — the locale is what L4's
  own generated tests vary, and what L5 adds is the manifest, the slots and
  the macro.
* **The registries differ on purpose.** L4 formats with every handler;
  an L5 crate formats with the closed world its corpus needs. They agree for
  every function a message actually uses, so a disagreement is a finding
  about the slicing, not noise to paper over.

## What Phase 5b leaves for later

* **A8**, above.
* **B5's view half** (Phase 6 A7): the marginal with a real `Tr` leaf, and
  P0.1's open item — measure with `--cfg erase_components` and propose the
  tachys leaf hook that would let a description reuse `&str`'s state and
  async path.
* **The relocation fallback is untested end to end** (§A7): cargo 1.98 does
  not produce the stale baked path on this machine. If a build cache that
  restores build-script outputs ever appears in CI, that is where to try it.
* **`markup(h)` for a view closure** (Phase 6 A1): the core takes anything
  that implements `MarkupHandler`; the facade's re-export of the Leptos one
  is Phase 6's to settle, and the expansion does not change either way.
