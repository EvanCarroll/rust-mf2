# Phase 5a results

Part of the [master plan](00-master-plan.md) (§9, P5a); the record behind the
exit checklist of the [Phase 5a work order](12-phase-5a-work-order.md). Every
figure names the command that produced it. Measured 2026-09-22 on the Phase 0
machine (Intel Core i7-1165G7, 4 cores / 8 threads, Linux 6.12), toolchain
stable 1.98.1, at a load average of 1.1–1.4. **Sizes do not depend on load;
timings do**, so timings are ranges and the two arrangements of owner question
1 are compared only by alternating them.

## Summary

| Exit item | Verdict | Evidence |
|---|---|---|
| the reference workload builds reproducibly, under plain cargo and cargo-leptos | **met for cargo**; cargo-leptos **not run** (no Leptos application before P6) | §A4, §A9 |
| `mf2 check` catches seeded drift of every lint class and is silent on the clean workload; gated functions are build errors with file and line | **met** | §A7 |
| editing one message rebuilds only what it must; a translation-only edit leaves the client wasm byte-identical | **met** | §A9 |
| the `mf2-build` ≡ `compile_str` differential green on the whole suite in both configurations | **met** — 485/485 | §A4 |
| B7, B8, B1′, B13 measured on `mf2-build`'s output and met, or restated; build cost reported | **met**: B7 and B8 on the catalogs the build writes, B1′ = +0 B and B13 = 13.6 KB avoided on the wasm the generated module produces | §A10, §A11 |
| owner questions 1 and 2 answered and recorded | **answered**, both with measurements and a recommendation | §"Owner question 1", §"Owner question 2" |
| fuzz targets clean for ≥ 1 h each on the final code | see §A12 | §A12 |
| `plans/phase-5a-results.md` written | this document | — |

What Phase 5a leaves for later, with reasons:

* **cargo-leptos** is not run: there is no Leptos application in the tree
  until P6, and installing the tool to build a synthetic one would measure
  the tool, not the pipeline. What cargo-leptos changes — two builds, two
  `OUT_DIR`s, two baked paths — P0.9 measured, and §A9's scenarios cover the
  same edits under plain cargo for both a server and a client target.
* **B1′ and B13 are measured, but not gated.** §A10 has both as byte deltas
  on the client wasm the generated module produces; they were taken by
  editing the fixture's corpus by hand and putting it back. A `b12-generated`
  harness pair would make a regression fail CI instead of waiting to be
  re-measured.

## A1 — `mf2-resource`

The W3C Message Resource container (D2) as a crate: a line-oriented parser
with error recovery, the data model generic over the message type, a
serializer, a `LineIndex`, and the JSON shape behind the `serde` feature. The
draft states no license, so nothing is vendored: the crate implements the
working grammar of [05](05-tooling.md) §2, written from the draft at
`third_party/w3c-message-resource/PIN`.

| Check | Figure | Command |
|---|---|---|
| Hand-written tests, one per production and for each example in 05 §2 | 40 | `cargo test -p mf2-resource --test grammar` |
| Generated resources through `parse(serialize(r)) == r` and fmt idempotence, in two styles | 4,000 | `--test generated` |
| The reference workload's files parsed | 72 files, 6,400 entries, **0 diagnostics** | §A2 |
| Its ids and values against the flat JSON the generator writes beside them | equal, all four locales | §A2 |

Seven things the working grammar left open are now decided and written into
05 §2: a whitespace-only line is empty; a continuation loses **all** its
indentation; trailing whitespace is part of the message; a comment may not
carry raw control characters either; an element carries one comment; a
property with an empty value is one with none; and `\` escapes any character
in an id. Each is a reading of the draft, to be re-checked against its ABNF
when the license allows vendoring it.

A value is *cooked* — indentation stripped, line breaks joined, escapes read
— so a diagnostic inside a message needs a way back to the file. `ValueMap`
is that way: one run for the common case (a single-line value with no
escapes, which stays borrowed), a list of runs otherwise.

## A2 — the loaders

`mf2-build` never sees a container: it asks a `Loader` for records — id,
source, spans, comment, properties, file. Two ship.

The flat JSON reader is written here rather than taken from `serde_json`,
because it keeps the offset of every key and every value and a `ValueMap`
through the JSON escapes: a message error in a JSON corpus then names a line
and a column exactly as one in a `.mf2` file does. It accepts an object of
strings and nothing else.

| Check | Figure |
|---|---|
| The two loaders on the reference workload | 1,600 messages per locale, the same ids and sources, all four locales |
| `@locale` of every file | equal to its directory, 72 files |
| Translator context the container carries | 1,140 comments and 336 `@param` properties in `en`, each `@param` naming a variable its message uses |
| A locale exported to flat JSON and read back | equal; and byte-identical to the JSON `workload-gen` writes |

`cargo test -p mf2-build --test loaders`.

## A3 — `mf2.toml` and the feature set

Every key of 05 §3.1, with `deny_unknown_fields` and a line and column from
the TOML span, so a typo names itself. `[lints]` is typed: a lint the rest of
the build relies on states a floor of `error` and refuses to be turned down,
while `unknown-function` goes all the way to `allow`, since custom functions
are legal. Fallback chains default to the tag's own parents and always end at
the source locale.

The feature set is deliberately not in that file: it is the i18n crate's own
cargo features, read from `CARGO_FEATURE_*` in `build.rs` and `--features` in
the CLI, so a server and a wasm build cannot disagree about which functions
exist.

`cargo test -p mf2-build --test config` (9 tests).

## A4 — the manifest and the catalogs

The pipeline end to end. On the reference workload:

| Item | Figure |
|---|---|
| Messages, locales | 1,600 × 4 |
| `manifest_hash` | **`0x43e0dc12eeb05ef1`** |
| Catalogs that decode to the messages that went in | 6,400 / 6,400 |
| Two builds in different directories | byte-identical manifests, catalogs and generated modules |
| A second build in the same directory | 0 files written |

The manifest hash is the figure [02](02-catalog-format.md) §3 records for this
corpus from P0.7. Nothing in Phase 5a was written to match it: the ids, the
slot order, the markup names and the function set were derived from the
corpus again, through a different path, and came out the same.

**The differential** — `conformance/tests/build_differential.rs`. For all 485
suite messages, the catalog `mf2-build` writes for a one-message corpus
formats identically — string, errors *and* parts — to the one `compile_str`
writes, through L4's runner:

| Configuration | Compared | Refused |
|---|---|---|
| every feature on (L4's registry) | **324** messages | **161**, every one of which `compile_str` also refuses |
| the default configuration (L4d) | **256** messages | **68**, each a gated function the build rejects |

324 + 161 = 485, the whole suite. The 161 are the syntax and data-model
tests: a message the spec refuses, which the build must refuse too — and does,
with the same verdict `compile_str` reaches.

Three lints are turned down for the differential and only there:
`unknown-function`, `dynamic-select` and `bad-option-value`. The suite has
messages that use all three on purpose, to exercise the *runtime's* errors;
mf2-two refuses them in a real corpus by policy (05 §5), which is not what
this differential is about. The slicer does not rely on any of them: a
`select` taken from a variable carries both plural rule sets rather than
guessing.

A corpus with errors comes back as an `Outcome` carrying its report, and no
catalog is built: the writer would otherwise refuse a message the lint had
just explained, and the reader would see the writer's words instead of the
lint's.

## A5 — the generated module

What an i18n crate includes at its root: `MANIFEST_HASH`, `SOURCE_LOCALE`,
the locale table with each tag's direction, the closed-world `registry()`,
the host the corpus needs, `pub use ::mf2 as __mf2;` and the `tr!` wrapper
with the manifest's absolute path and hash baked in (D8).

Three things it does not say on the client: the catalogs, their names and
their content hashes are behind `#[cfg(feature = "ssr")]`; the registry names
only the handlers the corpus uses, and takes the unannotated number and date
hooks only when some placeholder has no function and can receive one (#90);
the host module names one browser host per configuration, and a corpus that
formats no dates never names a date host at all (B1′).

`tools/i18n-fixture` is an i18n crate written as an application writes one, so
an ordinary workspace build compiles what the generator produced.

| Check | Figure | Command |
|---|---|---|
| Feature combinations compiled | **13** — 6 server, 7 client on `wasm32-unknown-unknown`, covering `fn-number`, `fn-datetime`, both date backends and `intl` | `cargo xtask codegen-matrix` |
| B6 in the client artifact | **clean**: none of 5 patterns (two canary strings, three catalog file names) is in the 15,798 B rlib | same |

The B6 check has both controls: the canary text *is* in each catalog the same
build wrote, and the rlib *does* carry other strings (17 hits for the crate's
own name), so the grep can fail.

## A6 — slicing on a corpus

A locale's number needs are exactly the union of its messages'. Two facts
`compile_str` cannot know are the build's: which plural rule sets the
selectors ask for, and whether `fn-number` is on at all.

| Corpus | Carries |
|---|---|
| plain text | nothing |
| cardinal selectors only | `plural.cardinal`, no ordinal rules |
| ordinal selectors only | `plural.ordinal`, no cardinal rules |
| `select` from a variable | both, which is what `dynamic-select` warns costs |
| numbers without `fn-number` | no number entry: the client would never read it |

**B8** (plural + number symbols ≤ 0.5 KB gz per locale), measured raw —
these entries are smaller than a gzip header, so the raw size is both the
honest figure and the conservative one:

| Corpus | Per locale |
|---|---|
| reference workload | en 17 B · en-XA 17 B · ar-XB 41 B · pl 45 B |
| every numeric function, 11-locale panel | ja 12 · de 17 · es 27 · hi 30 · en 32 · he 32 · fr 33 · ar 41 · ru 44 · pl 45 · cy 47 B |

All under 512. The panel corpus's whole locale data — currency and unit
tables included — is 385–1,023 B per locale.
`cargo test -p mf2-build --test slicing -- --nocapture`.

## A7 — `mf2 check`

Twenty lints, each named and levelled in `mf2.toml`. The seeded-drift corpus
is a table of one mutation per lint over a clean base, which reads as the
lints' documentation; each drift is applied on its own, and the run must
report that lint and no *other* error. The same table proves that a lint set
to `allow` says nothing, and a test asserts the table covers every lint there
is, so a new lint cannot be added without a drift for it.

The reference workload is the other half: 6,400 messages, **no errors**, and
warnings of exactly one class — the pseudo-locales carry the source's plural
variants, which Arabic's six categories do not match.

**The ledger cross-check** holds 01's L4d cells to the build:

| L4d cell | Requirement | Cells |
|---|---|---|
| `degraded`, `unknown-function` | a build rejection naming `gated-function`, with file and line | **68** |
| `degraded`, `neutral-numbers` | a `check` warning of that name, and the build still succeeds | **1** |

`cargo test -p mf2-build --test drift`,
`cargo test -p mf2-conformance --test build_differential`.

## A8 — `mf2-cli`

Every command of 05 §6 but `convert --from fluent`, which is P8: `init`,
`check`, `compile`, `fmt`, `stats`, `dump`, `pseudo`, `export`, `import`,
`watch` — each with an integration test that runs the binary as a person runs
it (`cargo test -p mf2-cli`, 10 tests).

`mf2 fmt` writes what `bench/workload-gen` writes: **`--check` reports 0 of
72 files would change** on a generated corpus. Reaching that settled three
points of the canonical form, all of them the generator's: a value longer
than 100 bytes wraps at column 76, filling greedily so a line breaks *before*
the word that would pass the width; a value with line breaks starts under its
`=` with every line at one indent and is never wrapped, because its line
structure is the message's; and a blank line sets off a comment, not a bare
property.

`mf2 stats` on the reference workload:

```
corpus … — 1600 messages, source locale en, manifest 0x43e0dc12eeb05ef1
CLDR 48.2.1 · MF2 spec 5c4ddb27 · catalog format v1

locale    coverage  missing       raw        gz        br  catalog
ar-XB       100.0%        0     61444     21314     18498  ar-XB.caa6ddf136b2a900.mf2b
en          100.0%        0     51516     20648     17992  en.17a8b19effe7bfa1.mf2b
en-XA       100.0%        0    101170     24580     21502  en-XA.64928debeaf51b58.mf2b
pl          100.0%        0     67945     27084     24109  pl.9b87882fe40080eb.mf2b

locale data, entry by entry (raw bytes in the catalog):
  ar-XB        41 B  plural.cardinal 17 B, number.symbols 24 B
  en           17 B  plural.cardinal 5 B, number.symbols 12 B
  en-XA        17 B  plural.cardinal 5 B, number.symbols 12 B
  pl           45 B  plural.cardinal 32 B, number.symbols 13 B
```

`mf2 watch` polls modification times rather than subscribing to the operating
system's file events: a corpus is a few hundred files, and nothing then has
to know about inotify, kqueue or the editors that write through a temporary
file. `mf2 import` writes a translation back into the container it came from,
keeping every section, comment and property; an id the locale does not have is
reported, not invented, because which section it belongs in is the
translator's decision.

## A9 — reproducible and incremental

`cargo xtask scenarios`, over `tools/i18n-fixture`, building the client
artifact for `wasm32-unknown-unknown` each time:

| Scenario | Outputs rewritten | Manifest hash | Client wasm |
|---|---|---|---|
| S1 nothing changed | none | same | **identical** |
| S2 a locale file touched, not changed | none | same | **identical** |
| S3 one message's text changed in a translation | that locale's catalog, and the module | same | **identical** |
| S4 one message's text changed in the source | that locale's catalog, and the module | same | **identical** |
| S5 a message added | every catalog, the manifest, the module | moved | rebuilt |
| S6 a variable added to a source message | every catalog, the manifest, the module | moved | rebuilt |

S3 is the one the design exists for. The generated module's *text* does change
on a translation edit — it lists the catalog's new content-hashed name — but
those lines are behind `#[cfg(feature = "ssr")]`, so the client compilation
never sees them. The fixture's client binary reads `MANIFEST_HASH`, as a real
one does, which is what makes "the wasm did not change" a claim rather than an
artefact of dead-code elimination.

Reproducibility: two builds of the reference workload in different directories
give byte-identical manifests, catalogs, `.br`, `.gz` and generated modules
(`cargo test -p mf2-build --test build`), and a second build in the same
directory writes nothing.

CI gains a `build-pipeline` job running `codegen-matrix` and `scenarios`.

## A10 — sizes

**B7** on the catalogs the build writes for the reference workload — the
manifest from the source locale, fallbacks flattened, COLD and IDS stripped,
and the LOCALE entries this corpus needs — brotli 11:

| locale | source | raw | raw max | br | br max | Phase 2 br |
|---|---:|---:|---:|---:|---:|---:|
| en | 43,250 | 51,516 | 66,862 | **17,992** | 20,610 | 18,072 |
| pl | 58,518 | 67,945 | 85,947 | **24,109** | 27,557 | 24,137 |
| en-XA | 95,078 | 101,170 | 131,647 | **21,502** | 44,192 | 21,537 |
| ar-XB | 54,491 | 61,444 | 80,913 | **18,498** | 25,725 | 18,423 |

Met everywhere; `en` is also under its absolute 23,296 B. The source figures
are Phase 2's to the byte, so this is like for like — and built *without*
`fn-number` the catalogs come out at exactly Phase 2's sizes, which says the
difference is the number data and nothing else. That data costs **−80 to +75
B brotli**: for three of the four locales the catalog got *smaller* when
12–24 B of symbols were added, brotli finding more to share.
`cargo test -p mf2-build --test sizes -- --nocapture`.

**B8**: §A6.

**B13 and B1′ on the generated module**, measured on the client binary
`tools/i18n-fixture` builds for `wasm32-unknown-unknown` under the
`wasm-release` profile (opt-level z, fat LTO, one codegen unit, panic abort,
stripped — 06 §3's size method), the corpus edited and put back:

| Build | Corpus | Features | `.wasm` |
|---|---|---|---:|
| A | `:integer` and markup | `hydrate,fn-number` | 364,235 |
| B | A plus `:currency`, `:unit`, `:percent` | `hydrate,fn-number` | 377,834 |
| E | nothing unannotated, no numeric or date function | `hydrate` | 349,171 |
| F | the same | `hydrate,fn-number,fn-datetime` | **349,171** |

**B13**: B − A = **+13,599 B** — using `:currency`, `:unit` and `:percent`
costs 13.6 KB, so a corpus that does not use them pays nothing for them.
**B1′**: F − E = **+0 B**, byte-identical, with two whole function crates
linked and neither reachable from the generated registry. A is reproducible:
the third build of it came out at 364,235 B again.

These are one-off measurements with the corpus edited by hand, not a gate.
Turning them into one — a `b12-generated` harness pair beside the others, so
that a regression fails CI rather than waiting to be re-measured — is what
this phase leaves for the next.

## A11 — build cost

`bench/build-cost` measures one `build.rs` pass the way an application pays
it: read, parse, validate, lint, flatten, slice, write.

| Pass, reference workload (1,600 × 4) | release | debug |
|---|---:|---:|
| cold (empty `OUT_DIR`), 14 files written | **369 ms** | 2,151 ms |
| warm (nothing changed), 0 files written | **358 ms** | 2,141 ms |
| after one message changed, 14 files written | **355 ms** | 2,160 ms |
| peak resident memory | 21.9 MB | 26.5 MB |

Split apart (owner question 1), the two halves cost very different amounts,
because brotli 11 at a 22-bit window is nearly all of it — measured in one
sitting on a quiet machine, `cargo run --release -p build-cost -- <corpus>
--emit <both|module>`:

| Pass, `--emit` | `both` | `module` |
|---|---:|---:|
| cold (empty `OUT_DIR`) | 367.4 ms, 14 files | **22.2 ms**, 2 files |
| warm (nothing changed) | 363.2 ms | **20.8 ms** |
| after one message changed | 359.5 ms | 22.0 ms |
| peak resident memory | 22.2 MB | **13.2 MB** |

So the i18n crate's half of the split is **17× cheaper**, not the same price
twice: it parses and validates the corpus but writes no catalog and
compresses nothing. (Until the review at the end of this phase it *did*
compress, and discarded the result — see §"Review of `mf2-build`".)

Cargo compiles a build script at `opt-level = 0` by default, so the debug
column is what an application actually pays; `[profile.dev.build-override]
opt-level = 2` in the application's manifest buys back 1.8 s per build.

`datetime-icu` costs nothing at build *time*: on a corpus with 160 `:datetime`
messages the pass is **315 ms** with the `icu.blob` and 350 ms without, and the
blob adds ≈500 B to each locale's catalog (Phase 4 measured 0.61–0.91 KB gz
for every shape). What it costs is the *compile*: the feature pulls ICU4X's
baked data crates in, and `build-cost` itself grew from 11.2 to 16.7 MB.

`cargo run --release -p build-cost -- <corpus> --features …`.

## Owner question 1 — server-only catalog embedding

**Recommendation: adopt the split, as an option the application chooses.**

`Build::emit(Emit::Module | Emit::Catalogs)` is implemented. The i18n crate
emits the module and the manifest; a crate only the server binary depends on
emits the catalogs and the table that embeds them. The scenarios run both
ways (`cargo xtask scenarios`, `--split`):

| Edit | One crate | Catalogs apart |
|---|---|---|
| a translation's text | module + that catalog rewritten | **nothing rewritten** |
| the source's text | module + that catalog rewritten | **nothing rewritten** |
| a new id | module, manifest, every catalog | module, manifest |
| a new input | module, manifest, every catalog | module, manifest |

With the catalogs apart, a text edit in any locale rewrites **nothing** in the
i18n crate's `OUT_DIR`. Its generated module is byte-identical, cargo's
fingerprint for it is unchanged, and neither it nor anything that depends on
it is recompiled — in either of cargo-leptos' builds. That is the 8–23 s per
debug build P0.9 measured for a 2,000-site application, avoided for every
translation change.

What it costs: the corpus is parsed twice per build, once by each crate, and
an application has one more crate in its tree. That second parse is cheap —
the i18n crate's half is **20.8 ms** warm against 363.2 ms for a combined
build, because it writes no catalog and compresses nothing (§A11) — so the
split *lowers* the i18n crate's build-script time by an order of magnitude
and adds a server-side crate that pays the full 363 ms once. The wall-clock
*saving* on the downstream recompile is not visible on `tools/i18n-fixture`
— one small crate has nothing above it to recompile — so the evidence for
that half is the mechanism, not a stopwatch; the figure is P0.9's, and it
scales with the application, not with the corpus.

Hence "an option the application chooses" — though the cost side of that
choice is now small enough that the reason to stay with one crate is the
simpler tree, not the build time. `mf2 init` should scaffold the
single-crate layout and say, in the file it writes, what the second crate
buys.

## Owner question 2 — one catalog or two for `intl` clients

**Recommendation: one catalog.**

The measurement is what a client-only variant — the same messages without the
number, currency, unit and plural entries — would save per locale:

| Corpus | Catalog br | Entries (raw) | br saved by dropping them |
|---|---:|---:|---:|
| reference workload, en | 17,992 | 17 | **−80** |
| reference workload, pl | 24,109 | 45 | **−28** |
| reference workload, en-XA | 21,502 | 17 | **−35** |
| reference workload, ar-XB | 18,498 | 41 | **+75** |
| every numeric function, en | 601 | 498 | +242 |
| every numeric function, ar | 638 | 525 | +285 |
| every numeric function, pl | 695 | 770 | +318 |
| every numeric function, ru | 724 | 1,023 | +356 |
| every numeric function, ja | 590 | 385 | +240 |

On a corpus that looks like an application, a second variant saves nothing —
three of the four locales are *smaller* with the data in them, because brotli
has more to share. On a corpus of nothing but numeric functions it saves
0.24–0.36 KB per locale, on a catalog of well under a kilobyte.

Against that: two content hashes per locale, two preload links, a server that
must know which build the client is, and a second set of files to serve and
cache. The `intl` client ignores the entries it does not read, and the server
needs them anyway — the Rust path is the server's (D4).

Revisit if a corpus's locale data grows into the kilobytes. The build already
says when: `mf2 stats` prints the per-entry breakdown, and the LOCALE section
is keyed and optional, so a client variant can be added later without
changing the format.

## A12 — fuzzing and generated input

| Target | Input | Checks |
|---|---|---|
| `resource` | bytes → `from_utf8_lossy` | no panic; every span in bounds and on char boundaries; every cooked offset maps back into the file; a file read without a diagnostic serializes in both styles, parses to the same resource and writes itself again byte for byte |
| `pipeline` | bytes → one locale's resource file; the target makes a two-locale corpus from it, the translation holding every other entry | no panic; a refused corpus reports an error and writes no catalog; an accepted one's catalogs load, decode message by message to the flattened models, carry the right fallback flag, are byte-identical on a second build, and name no catalog outside the module's `ssr` block |

Seeds: `cargo xtask fuzz-seed` writes `fuzz/corpus/resource/` — the reference
workload and the suite's messages, each as one resource file (76.6 KB and
20.1 KB).

RESULTS-PENDING

## Review of `mf2-build`

An adversarial read of the pipeline at the end of the phase, checked claim by
claim against the code. Eight candidates; six were real and are fixed here,
two were the code being right and a comment being wrong.

| # | What | Verdict |
|---|---|---|
| 1 | `Emit::Module` runs brotli 11 for every locale and discards the result | **real** — the split's whole point, undone |
| 2 | …and returns before `remove_stale`, so a crate that switches to it leaves every old catalog in `OUT_DIR` | **real** |
| 3 | `slice.unannotated` looks only at a placeholder's inline function, so `{$n}` under `.input {$n :integer}` links the #90 hooks it can never reach | **real**, and a B1′ leak |
| 4 | `locales/` takes every subdirectory as a tag, unvalidated, and the tag is interpolated into generated Rust unescaped | **real** — `direction()` accepts any string, so `locales/_templates/` became a locale and a quote in a name broke the module |
| 5 | brotli and gzip errors are discarded (`let _ =`, `unwrap_or_default`), so a truncated `.br` ships beside a `.mf2b` whose hash says it is intact | **real** |
| 6 | the coverage message names a fallback chain even under `missing = "empty"` | **real** |
| 7 | "a translation-only edit never changes the manifest hash" | **comment wrong**: a translation that introduces a *function* the source does not use must move it, or the closed-world registry leaves the page with an Unknown Function |
| 8 | "`check` refused any function the configuration does not name" | **comment wrong**: `unknown-function` is an error *by default* and can be turned down, and then the runtime reports Unknown Function — the spec's own fallback |

The fixes: compression is skipped and pruning still runs under `Emit::Module`;
`declared_function` resolves a bare `{$n}` through the declarations before the
hooks are linked; `Layout::locales` validates tags and `codegen` escapes them;
`brotli`/`gzip` return `Result` and a new `Error::Compress`; the coverage
message follows `[catalog] missing`. Tests for the first four are in
`crates/mf2-build/tests/{build,slicing}.rs`
(`emitting_only_the_module_does_not_compress`,
`emitting_only_the_module_prunes_the_catalogs_it_stopped_writing`,
`a_stray_directory_is_not_a_locale`,
`a_declaration_annotates_the_placeholders_that_use_it`).

What #1 was costing is in §A11.

## What changed in the plans

* [05](05-tooling.md) §2 records the seven readings of the working grammar
  that A1 settled.
* [12](12-phase-5a-work-order.md) gains its exit checklist, ticked.
