# The 3.0 feature structure and native binary size

Status: design, 2026-10-01; the owner's decisions of the same day are in §2.
Nothing here is built yet. The work is six phases, 11 to 16, one file each
(§7); this file is the design they read from. Every saving below is an
estimate made by subtracting measured component sizes, not a rebuilt binary;
each change lands with the binary measured again.

## 1. The case study: trippy

`vendor/trippy` (untracked, the owner's port) has two release builds:

* `target/release/trip` — branch `master`: 12 locales in `locales.toml`
  (32 KB), embedded with `include_str!`, parsed with the `toml` crate trippy
  already links for its configuration, arguments filled in with
  `String::replace("%{name}")`. No plurals, no locale-aware numbers, no
  language matching, no normalization.
* `target/release/mf2-trip` — branch `mf2` (one commit, `fa88ee8`): the same
  12 locales as `.mf2` sources, built by `mf2-build` into embedded catalogs,
  `mf2` with features `ratatui` and `fn-number`.

| Binary | Bytes on disk |
|---|---:|
| `trip` | 9,666,608 |
| `mf2-trip` | 10,222,768 |
| Growth | **+556,160 (543.1 KiB)** |

### 1.1 By section (`size -A`)

| Section | trip | mf2-trip | Δ |
|---|---:|---:|---:|
| `.rodata` | 841,128 | 1,195,220 | +354,092 |
| `.text` | 4,906,332 | 5,013,900 | +107,568 |
| `.data.rel.ro` | 1,159,856 | 1,184,080 | +24,224 |
| `.rela.dyn` | 879,576 | 900,408 | +20,832 |
| `.eh_frame` + `.eh_frame_hdr` | 411,940 | 427,788 | +15,848 |
| symbol tables (both unstripped) | | | ≈ +31,000 |

### 1.2 By component

Named code comes from `nm -S --size-sort -C`, grouped by crate. Anonymous
`.rodata` is attributed by disassembly (`objdump -d`): each referenced
address is charged to the crate whose functions reference it, up to the next
referenced address. The time-zone files were counted by parsing every TZif
blob in the binary; the zone-name table by counting `R_X86_64_RELATIVE`
relocations whose target is an IANA zone name (553 in `trip`, from
`chrono-tz`; 1,106 in `mf2-trip`).

| Component | Bytes | What it is |
|---|---:|---|
| Time zones | +315,421 | jiff's bundled database: 341 zone files (202,414 B) and the rest of jiff's `.rodata` (≈ 228 KB in all); `jiff_core` + `jiff` code (59,022 B); 553 zone-name relocations and table entries (≈ 22 KB); `mf2::native::zone` (5,878 B) |
| NFC normalization | +121,386 | `unicode-normalization`'s tables, compiled into `mf2_host_std` (≈ 116 KB), and `StdHost::nfc` |
| The MF2 interpreter | +93,401 | `mf2_runtime` 35 KB, `mf2` 27 KB (without the zone reader), `mf2_fn_number` 21 KB, `mf2_catalog` 9 KB, the host's remaining code |
| Float text (`ryu`) | +13,155 | `ryu`'s tables (10,688 B) and `StdHost::f64_to_text` |
| The 12 catalogs | +36,180 | `*.mf2b`, `include_bytes!` |
| The old `locales.toml` | −32,153 | no longer embedded |
| Everything else, net | +8,770 | unwind tables, symbols, smaller `core` / `toml` / `trippy_tui` code |
| **Total** | **+556,160** | |

trippy already ships `chrono-tz` for its own clock, so `mf2-trip` carries two
copies of the IANA database.

### 1.3 Where the interpreter's 91 KiB goes

| Part | Code | Needed without an interpreter? |
|---|---:|---|
| Evaluator (`mf2_runtime::eval`, `format`, `unannotated`, …) | ≈ 30 KiB | no |
| Catalog reader (`mf2_catalog::reader`, `view`) | ≈ 9 KiB | no, if messages were code |
| The functions: `:integer`, `:number`, `:percent`, plurals, digits | ≈ 26 KiB | yes, trippy uses them |
| Integration: Ratatui text, `tr!` arguments, the native store | ≈ 21 KiB | yes, in some form |
| Language matching (`mf2::matching`) | ≈ 6 KiB | yes |

Compiling messages to Rust would remove about 39 KiB of evaluator and reader
but add code per message and per locale (trippy: about 480 such messages,
guessed at 25–50 KiB). That is about break-even for trippy, and the web's lazily
loaded catalogs need the interpreter anyway. The interpreter stays.

### 1.4 What the three changes are worth

| Change | Saving for trippy | Design |
|---|---:|---|
| Link the time-zone database only when dates are formatted | −315,421 B (308.0 KiB) | §4.1 |
| Format floats with `core` instead of `ryu` | −13,155 B (12.8 KiB) | §4.2 |
| Decide at build time how much normalization a catalog needs | −121,386 B (118.5 KiB) | §4.3 |
| **mf2-trip over trip afterwards** | **+106,198 B (103.7 KiB)** | |
| *Measured for §4.3 at the Phase 13 exit* | *−122,168 B (119.3 KiB)* | §4.3 |
| *`mf2-trip` with all three, measured (2026-10-03)* | *8,244,072 B on disk, unchanged since Phase 13's exit* | §4 |

What remains is the interpreter (91 KiB), the catalogs net of the TOML they
replace (+4 KiB) and about 9 KiB of other changes.

The §4.3 row is measured, not estimated (Phase 13's exit, 2026-10-02): the
same trippy branch and toolchain either side, only this tree differing.
`trip` built against the tree before task 13.1 is 8,366,240 B; against the
phase's end, 8,244,072 B. The estimate was 121,386 B.

Those absolute bytes are not comparable with the 10,222,768 above. That
figure is branch `mf2`, whose own clock still used `chrono-tz`; the
measurement is branch `mf2-jiff` (owner, 2026-10-02), which migrated trippy
to jiff and so stopped carrying a second zone database. The two zone
databases §1.2 noted are therefore one, and the remaining gap is the owner's
migration, not this work.

The last row is measured too (2026-10-03, owner's request): branch `mf2-jiff`
at `c527775`, rustc 1.98.1, path dependencies on this tree at `f68832e`, the
release profile, no edit to trippy (it names `fn-number`, not the old
`intl`). `mf2-trip` is 8,244,072 B, the same to the byte as at Phase 13's
exit, so Phase 14 changed nothing trippy links; §4.1 and §4.2 had landed in
Phase 12, before that exit. Against the `trip` above (9,666,608 B, branch
`master`) it is −1,422,536 B, which is not a like-for-like gap: `master` still
carries `chrono-tz` and was built on an earlier date. A build before all three
changes was not made (owner, same day), so of the estimates' sum, 449,962 B,
only §4.3's share is measured.

## 2. Owner decisions (2026-10-01)

1. **The rework is 3.0, now.** Features may be renamed and removed and the
   `Host` trait may change. Compatibility with 2.x is not a goal.
2. **`mf2` keeps `default = []`.** The tools choose the feature list:
   `mf2 init` writes it, and `mf2 check` reports what the catalogs need and
   what is on but unused (§5).
3. **Time zones:** a native or terminal application that formats dates reads
   the system's time-zone database. A feature, `tzdb-bundled`, opts into the
   bundled database for the same answer everywhere. Servers (`ssr`, `axum`)
   keep the bundle. A container with no tzdata cannot resolve a named zone
   unless `tzdb-bundled` is on.
4. **`intl` must reach `Host::numbers`** (owner, 2026-10-02). The host wiring
   is meant to give an application both `Intl.NumberFormat` and
   `Intl.PluralRules` through `Host::numbers`. That a build with the feature
   gets neither (§8 F1) is a defect, not a design choice. Task 14.0 fixes it.
5. **One release, and the fix goes out in it** (owner, 2026-10-02). The
   `intl` defect is fixed in 3.x, never as a 2.x patch, and **no intermediary
   release is published**: 3.0.0 is the next thing on crates.io and it carries
   the fix, the size work, the renames and the guide together. The phases may
   therefore run in whatever order suits the work — the owner does not mind —
   because nothing is published between them. One consequence to keep in
   mind: the zone work (§4.1) is itself breaking, so publishing before it
   would force the savings into a 4.0.0 and make a developer who adopted
   3.0's names upgrade twice.

6. **No general NFC, and the helper says when it cannot answer** (owner,
   2026-10-02). `mf2` is not a normalization service. The only comparisons
   the specification asks for are a selector value against a variant key and
   an argument name against a declared name, and §4.3's map answers both
   exactly — the WG suite passes at every layer without `Host::nfc`. A
   general-NFC host behind a feature was considered and rejected: on a native
   target it would put the tables back to duplicate `unicode-normalization`,
   which an application that wants full NFC can depend on itself in two
   lines, and on the web it would be nearly free — a convenience that costs
   nothing on one target and 118 KiB on another does not belong in the host
   trait. What does need fixing is narrower: `FnContext::equivalent` answers
   `false` for a key the map does not cover, which can be wrong. That case is
   detectable where the check already walks, so task 14.5 makes it
   non-silent instead.

## 3. The feature structure

### 3.1 What was wrong in 2.0

1. **The native host broke the project's own rule.** `mf2-host-web` splits
   its statics so that a client links only the host methods it uses ("a host
   method is linked whenever its host is: it is in the `Host` vtable").
   `mf2-host-std` was one static with no features, so every `native`,
   `ratatui`, `axum` and `ssr` build linked jiff with its bundled database,
   the normalization tables and `ryu`. The size gates covered the client
   only, so nothing caught it.
2. **`native` meant "time zones" too.** It carried jiff and looked up the
   system's zone on every load, with or without a date in any message.
3. **A library fixed policy for every application:** `tzdb-bundle-always` was
   set inside `mf2-host-std`.
4. **`Host` mixed platform services with catalog facts.** Float text and zone
   offsets are platform services; equivalence with a catalog's keys is not.
5. **The build checked feature use in one direction only:** an error when a
   catalog uses a function whose feature is off, nothing when a costly
   feature is on and unused.
6. **Feature combinations were hand-listed in four places** (`xtask`'s `ci`,
   `codegen-matrix`, `msrv`, `refusals`), about 56 builds.

### 3.2 The rule

**A feature exists only when the linker cannot decide.** It must change the
dependency graph, change behaviour, or choose who supplies data. Everything
else is linked because the generated module names it, and the generated
module names only what the corpus uses. The function registry already works
this way (`crates/mf2-build/src/codegen.rs`, `registry`); 3.0 extends it to
the native host.

Two consequences:

* A generous feature list costs build time, not bytes, so the recommended
  sets stay simple.
* Where two choices conflict, the feature is the larger, additive side
  (`tzdb-bundled` over the system's database). Cargo unifies features across
  a workspace, so the feature that any crate turns on must be safe for all.

### 3.3 The features of `mf2` 3.0

Twenty, none on by default, as four questions.

| Question | Features |
|---|---|
| Where does it run? | `leptos` or `leptos-0-8`, with one of `ssr`, `hydrate`, `csr`; `axum`; `native`; `ratatui` (implies `native`); `clap`; with no framework, `host-std` or `host-web` |
| What can messages do? | `fn-number`; `fn-datetime` |
| Who supplies locale data? | `number-intl`; `datetime-icu`; `datetime-intl`; `tzdb-bundled` |
| Behaviour and tools | `static-locale`; `mark-fallback-lang`; `compile` |

The third question is the choice between the same answer everywhere (the
catalog's data, the bundled database) and a smaller build (the browser's
`Intl`, the system's database).

### 3.4 What changes from 2.0

| 2.0 | 3.0 | Why |
|---|---|---|
| `intl` | `number-intl` | pairs with `datetime-intl`; says what it replaces |
| `native` carries jiff and reads the system zone | `native` has no jiff; time zones come with `fn-datetime` | §4.1 |
| `IntoArg` for jiff's types, under `native` | under `host-std` with `fn-datetime` | they are only useful with dates; servers get them too |
| the bundled time-zone database, always | the system's; `tzdb-bundled` opts in; `ssr` and `axum` imply it | decision 3 |
| `Host::nfc`, required | removed | §4.3 |
| `mf2-fn-number/intl` | removed | it only forwarded to `mf2-runtime/intl` |
| `links = "mf2-v2"`, `DEP_MF2_V2_*` | `mf2-v3`, `DEP_MF2_V3_*` | the major version is in the name |

Kept on purpose:

* **`native` stays one feature.** Once jiff leaves it, what remains (embedded
  catalogs, catalog files, the system's languages, the store) should be linked
  only when used. The native canaries (§6.1) confirm or refute that.
* **`host-std` and `host-web` stay public.** They are the only way to use
  `mf2` without Leptos or Axum, and the benchmark workload does so.
* **`fn-datetime` with no backend** stays the neutral, ISO form: dates with no
  locale data.
* **Both date backends on at once** keeps its fixed precedence
  (`datetime-intl` in the browser, `datetime-icu` elsewhere); `mf2 check` says
  which one wins. The code had ICU win everywhere; the owner kept this
  design (2026-10-03), so a browser build with both on carries no ICU date
  data. Task 15.2a fixed the code.
* **`datetime-icu` still needs `mf2-build`'s `icu-blob`** in the build
  dependency. Cargo cannot tie the two; the build error says so.

### 3.5 The manifests

`mf2` (only the lines that change):

```toml
fn-datetime  = ["dep:mf2-fn-datetime", "mf2-host-std?/time-zones"]
number-intl  = ["mf2-runtime/intl", "mf2-host-web?/intl"]
tzdb-bundled = ["mf2-host-std?/tzdb-bundled"]
native       = ["host-std", "dep:sys-locale",
                "mf2-catalog/static-bytes", "mf2-catalog/content-hash"]
ssr          = [ …as in 2.0…, "tzdb-bundled"]
axum         = [ …as in 2.0…, "tzdb-bundled"]
```

`mf2-host-std` (it had no features):

```toml
# `ZONES_HOST` (Host::zone_offset through jiff) and the system's zone.
time-zones   = ["dep:jiff", "jiff/std", …the system-database features, §8 F2… ]
# Lookups use jiff's bundled database. Weak: nothing without `time-zones`.
tzdb-bundled = ["jiff?/tzdb-bundle-always"]
```

Every `?` is a weak dependency feature: it takes effect only when something
else has enabled the crate. That is how "the bundle, but only with dates" is
written without an "and" in Cargo. `mf2`'s `build.rs` turns
`CARGO_FEATURE_*` back into feature names and relies on no name containing
`_`; the new names keep to that.

The features of the sub-crates (`mf2-host-std`, `mf2-host-web`,
`mf2-runtime`, …) are not part of the compatibility promise; `mf2`'s are.

## 4. The three changes

Order: time zones, float text, normalization.

### 4.1 Time zones

2.0: `mf2-host-std` enables jiff with `tzdb-bundle-always` unconditionally
(`crates/mf2-host-std/Cargo.toml`), and `StdHost::zone_offset` asks
`TimeZoneDatabase::bundled()`, then tries the name as a POSIX rule. `mf2`'s
`native` adds `dep:jiff`, `jiff/std` and `jiff/tz-system`;
`crates/mf2/src/native/catalogs.rs` (`Catalogs::load`) hard-codes
`&mf2_host_std::HOST` and sets the system's zone on every load, and
`native/store.rs` (`refresh`) falls back to it. jiff is used in `mf2` in two
places only: `native/zone.rs` and the `IntoArg` impls in `into_arg.rs`
(`mod with_jiff`). `set_time_zone` takes `mf2-runtime`'s own `TimeZone`.

3.0:

* **Two statics on the native host.** `HOST` has float text only; its
  `zone_offset` is the trait's default, so a named zone is *Bad Option*, as
  with the browser's `HOST`. `ZONES_HOST` (feature `time-zones`) adds
  `zone_offset`.
* **Which database.** With `tzdb-bundled`, `ZONES_HOST` asks the bundled
  database. Without it, it asks the system's (jiff's default lookup, which on
  a platform with no system database uses jiff's own platform bundle). The
  POSIX-rule fallback stays in both.
* **The system's zone is a platform service.** The reader in
  `mf2::native::zone` (jiff's `try_system`, then `TZ`, then the footer of
  `/etc/localtime`, then UTC) moves into `mf2-host-std` behind `time-zones`,
  returning `mf2-runtime`'s `TimeZone`. Without `fn-datetime`, native code
  uses UTC and looks nothing up.
* **`mf2` has no jiff dependency of its own.** The `IntoArg` impls for jiff's
  types move to `cfg(all(feature = "host-std", feature = "fn-datetime"))`,
  over a re-export of jiff from `mf2-host-std`.
* **One host per application, named by the generated module.** It names the
  date host when `fn-datetime` is on **and** the corpus can reach a date: a
  message uses `:datetime`, `:date` or `:time`, or has a plain placeholder
  (which may be handed a date). This holds for the browser hosts
  (`__use_host!` in `crates/mf2/src/__generated.rs`, `host` in
  `crates/mf2-build/src/codegen.rs`) and the native one (`native_host`);
  `Catalogs` takes its host from the generated corpus instead of naming a
  static.
* The settings API (`set_time_zone`, `TimeZone`) stays; without dates it has
  no effect.

### 4.2 Float text

`StdHost::f64_to_text` uses `ryu`. `core`'s `{:?}` formatting of `f64` is
shortest round-trip, keeps `.0` on whole numbers and switches to an exponent
at the extremes. The two may switch to an exponent at different sizes, which
does not matter if `Number::from_f64` (`crates/mf2-runtime/src/number.rs`)
parses both to the same digits. The text must fit the 32-byte buffer.

3.0: `core` formatting into the fixed buffer; `ryu` leaves `mf2-host-std`
(it may stay a dev-dependency as the test oracle). No feature. `mf2-host-std`
is never in a browser client, so `fmt` is allowed there. A test compares the
parsed `Number` for both over the edge values and a large random sample
before `ryu` goes; the conformance suite decides. Fallback: keep `ryu`.

Done in 12.4. The two agree except where the shortest round-trip text is an
exact tie between two decimals (46 of 200,000 random floats): `core` rounds
the last digit up where `ryu` rounds it to even. Both are shortest and both
round-trip, so the test asserts "equal, or a last-digit tie".

The saving needs the application to format a float somewhere of its own, as
trippy and the `tui` example do (`{:.1}`): then `core`'s tables are already
linked and `ryu`'s 10.7 KiB go. A binary that formats no float itself links
them for `mf2` alone and grows instead — the `native` canary went from
623,016 B to 634,808 B.

### 4.3 Normalization: a fact about the catalog, not a platform service

MF2 compares `:string` selector values and argument names with variant keys
and names in NFC. Everything the build can see is already normalized at build
time: keys, literals and names by the catalog writer
(`crates/mf2-catalog/src/writer.rs`), `tr!` argument names by `mf2-macros`.
What reaches the runtime is the value passed in by the program (the `:string`
operand, `crates/mf2-runtime/src/functions/string.rs`) and argument names
passed through the dynamic API (`slot_map`,
`crates/mf2-runtime/src/format.rs`; its only production caller is `TrDyn`).
The vendored WG tests only exercise operands written in the message; the
runtime case comes from the spec's wording.

The runtime never needs the NFC form of an arbitrary string, only whether a
string is canonically equivalent to one of a fixed set of keys known when the
catalog is written. Two strings are canonically equivalent exactly when their
NFD forms are equal, and decomposition followed by canonical reordering only
rearranges the characters each character decomposes into. So a value can
match a key only if every character in it decomposes into characters of the
key's NFD form. The writer can therefore compute, from the full tables:

* the few code points whose full canonical decomposition uses only characters
  of the catalog's keys and names in NFD, with their decompositions (one entry
  per code point: each has exactly one canonical decomposition);
* the combining classes of those characters.

At runtime: the existing quick check (`nfc_quick`, every byte below 0xCC)
and a byte comparison; otherwise any character outside the map's reach means
no match, else decompose both sides with the small map, reorder by combining
class and compare. No composition tables are needed.

trippy selects on user key bindings (runtime values) against the keys `h`,
`s` and `q`. No code point decomposes to those, so its map is empty and the
check is "ASCII bytes equal, else no match". For keys covering much of a
script's accented letters the map approaches the full decomposition table; it
is never larger.

3.0:

* **The map is an optional catalog section**, written by both writers
  (`writer::catalog` and `writer::single`, so a `compile_str` catalog has it
  too: §8 F3). The keys stay in the catalog, never in the client wasm. An
  absent section means an empty map. The catalog format's version moves so
  that a 3.0 reader refuses a catalog written before the rule existed.
* **`mf2-runtime` performs the check itself**, on every host, from the
  section. It is client-path code: `no_std`, `forbid(unsafe_code)`, no
  `fmt`, no panic. Web and native then behave identically.
* **`Host::nfc` is removed**, from the trait and both hosts. The browser's
  `String.prototype.normalize` glue goes; `unicode-normalization` leaves
  `mf2-host-std`. There is no `nfc` feature: the full tables stay only in the
  build-side crates (`mf2-syntax`, the writer, `mf2-macros`, `mf2-build`).
* **A helper on the function context** gives a custom function the same
  comparison with a key. The map is exact for any string whose NFD characters
  it holds, and every key's and name's are there by construction, so that set
  is the helper's domain; a key outside it is reported, not answered
  (decision 6, task 14.5).

Provenance: the method combines standard parts of UAX #15 (canonical
equivalence as NFD equality, canonical ordering, the quick check) with
something like the canonical closure used in collation tailoring. It is not in
the MF2 specification, and no Unicode specification I know prescribes it.
Unicode Technical Note #5 ("Canonical Equivalence in Applications") is the
closest reference to read. None of these were checked from this repository.

The gate, before it replaces the normalizer ("don't build worse than what
exists"):

* an exhaustive test: for every code point and a corpus of key sets, the
  restricted check agrees with `unicode-normalization`;
* a differential fuzz target: random strings of decomposable characters and
  combining marks against full NFC;
* the WG suite at every layer, no ledger entry moving to `xfail`;
* the client budgets (B1, B5, B12) hold.

The fallback, if the gate fails: `Host::nfc` stays, as a provided method, and
the full tables go behind a feature `nfc` on `mf2-host-std`, enabled by
`compile`.

## 5. The tools choose the feature list

* **Lint `unused-feature`** (warn), in the lint table of
  `crates/mf2-build/src/lint.rs`, beside `gated-function` (error: a function
  whose feature is off) and `neutral-numbers`. It fires when a function or
  data feature is on for this build and no catalog can use it. For
  `fn-datetime` it says the cost: every plain placeholder then links the date
  code and time zones. It says "on for this build", because another crate in
  a workspace may have turned the feature on.
* **`mf2 check` prints the feature list the corpus needs**, as the line to
  write in `Cargo.toml`, beside the features that are on. It already reads the
  resolved features through `cargo metadata` (`crates/mf2-cli/src/cargo.rs`).
  It also says which date backend wins when both are on.
* **`mf2 init`** writes the list for each kind of application
  (`crates/mf2-cli/src/init.rs`: `Mode::features()` and the templates, which
  must agree).

## 6. Guard rails

1. **Native canaries** (`cargo xtask native-canaries`). Small native binaries
   are linked for a few feature sets, unstripped, and their symbols read. A
   row says which crates' symbols must be absent or present for a set: no
   `jiff` and no `ryu` without dates, no `unicode_normalization` without
   `compile`. This is the check that would have caught the 308 KiB. An absent
   symbol means no named function or static of that crate remains — the
   crate a symbol's path begins with, not whichever crate instantiated it.
   One row names a module path instead of a crate (12.5): jiff's bundled
   IANA database is `jiff::tz::db::bundled`, not a crate in the symbol
   table, and a dateless or system-zone build must not carry it.
   The workload is `tools/native-canary`, the smallest application that uses
   MF2, a workspace of its own; `nm` reads the symbols. Phase 11 lands the
   command with the positive control and the report only: the forbidding
   rows arrive with the phases that make them true.
2. **`tui-gate`'s limit follows the measurement.** `SIZE_LIMIT` in
   `xtask/src/tui_gate.rs` (1,965,320 B, also written in
   `tools/checks/compare.sh`) is lowered to the measured size of `tui-mf2` at
   the end of each phase that shrinks it. `examples/tui` is the in-tree
   measurement; trippy is the owner's, measured beside it.
3. **One table of feature sets in `xtask`**, read by `ci`, `codegen-matrix`,
   `msrv` and `refusals`, with a nightly check-only build of each feature
   alone where that is valid. A powerset is not practical: modes exclude each
   other and several sets exist for one target only.
4. **A measured cost per feature** (`cargo xtask feature-costs`): gzip bytes
   in the client and bytes in a native binary, written by the command to
   `docs/feature-costs.md` and included from `docs/features.md`, so the
   smaller build is chosen with numbers. `static-locale` and
   `mark-fallback-lang` are not measured; the table says so.
5. The client gates (B1, B5, B6 canaries, B12) are unchanged and hold.

## 7. The phases

| Phase | File | What |
|---|---|---|
| 11 | `plan/02-phase-11-open-3.0.md` | Version 3.0.0, the findings of §8, the feature-set table, the native canaries |
| 12 | `plan/03-phase-12-native-host.md` | Time zones and float text (§4.1, §4.2) |
| 13 | `plan/04-phase-13-normalization.md` | Normalization (§4.3) |
| 14 | `plan/05-phase-14-names-and-tools.md` | `number-intl`, the lint, `mf2 check`, `mf2 init` (§3.4, §5) |
| 15 | `plan/06-phase-15-costs-and-guide.md` | Costs per feature, the guide, the upgrade page |
| 16 | `plan/07-phase-16-release.md` | Release 3.0.0 |

They run in that order; each starts when the one before it is done.

**How a phase is run.** A coordinator keeps a small session and reads the
phase file only. For each task, in order, it starts one `mf2-task` agent with
a brief of three lines: the task's number, the phase file, and the design
section the task names. The agent builds, runs `cargo xtask ci`, commits by
path, and edits three places in the phase file: "In flight", "Next", and a
Done entry of at most five lines. When the tasks are done the coordinator
carries out the file's "Phase exit". A question for the owner is asked in
plain English, with the background, the options and what each means for
application developers; the answer is written into this file before the next
task starts.

## 8. Findings, and what is not verified

Task 11.2 answers F1–F3 here, in at most ten lines in all.

* **F1, number `intl`.** Broken in a browser: `__use_host!` names only
  `host_web::{HOST, ZONES_HOST, INTL_HOST}`, none of which overrides
  `Host::numbers` (default `None`), so every number and every plural
  selector is *Unsupported Operation*. A defect, confirmed by the owner
  (decision 4). Fix (14.0): `IntlNumbers` statics over each date host, named
  by new `intl` arms. Off `wasm32-unknown-unknown` the Rust path is chosen by
  target, so `intl` is inert there.
* **F2, jiff 0.2.37.** The names hold (`tzdb-zoneinfo`: `TZDIR` else
  `/usr/share/zoneinfo`; `tzdb-bundle-platform`: a bundle only where no system
  copy exists; `tz-system`). `TimeZone::get` goes through `tz::db()`
  (`from_env`); `tzdb-bundle-always` changes what it returns only where there
  is no system copy.
* **F3, the writers.** Both see every key and name: `catalog` holds each
  message's AST and the manifest's slots (locals via `locals_of`), and
  `single` passes `compile_str`'s NFC externals through. Keys and names are
  normalized inside `encode_all`'s one pass, so §4.3's map is collected there.
* **`native` links by use** — measured (11.4, `cargo xtask
  native-canaries`): a native application that formats text and a number
  links neither `sha2` nor `sys-locale`, and one that formats a date in a
  named zone links `jiff`; but jiff, `unicode-normalization` and `ryu` are
  linked with no date at all, which is what §4.1–§4.3 remove.
* **3.0.0's own 215 B.** Measured (11.5): B1 26723 → 26938 B gz, the whole
  app 41896 → 42017, from 133 B raw in the `tr` wasm's code section — no new
  code (same function count, same data length, 1072 bodies permuted netting
  +133: the 3.0.0 version and `links` name change `-C metadata`, so the same
  code is laid out differently). Reverting only those two reproduces v2's
  figures exactly, so 3.0.0 costs this and there is nothing to take out.
* **Float text.** That `core`'s formatting is already in most native binaries
  is not verified.
* **Every byte figure** in §1.4 is an estimate by subtraction.
