# 09 — Phase 2 work order: the binary catalog (layer L3)

Part of the [master plan](00-master-plan.md) (§9, P2). RFC 2119 keywords apply.
Written at the close of Phase 1 from [phase-1-results](phase-1-results.md).
The format itself is specified in [02-catalog-format](02-catalog-format.md);
this document orders the work, fixes the API the later phases build on, and
sets the exit criteria.

Phase 2 builds `mf2-catalog`: the `.mf2b` writer (including `writer::single`,
the one-message compile every later layer uses), the client reader, the
model-rebuilding decoder and the manifest file — and proves the format
**lossless over the full data model** (layer L3) on all 301 L3-applicable suite
messages, on generated input and under fuzzing, **deterministic**, and within
budget **B7** on the committed reference corpus. The byte format is frozen at
exit as version 1.

## State at the start

| In the tree | Where |
|---|---|
| The data model, frozen (plus the additions listed in 08 §"Status at exit") | `crates/mf2-model` |
| Parser, validation, serializer, `analyze` (slot order = ascending bytewise NFC; markup and function sets) | `crates/mf2-syntax` |
| L1/L2 harnesses; ledger at `current_phase = "P1"`, every L3 cell `xfail until = "P2"` (301); `--promote`; the spec path helper; the ABNF generator | `conformance/` |
| The fuzz workspace (target `parse`), seeded by `cargo xtask fuzz-seed` | `fuzz/` |
| The D1 gate in CI; the nightly long runs | `.forgejo/workflows/` |
| Throwaway starting points: P0.7's encoder, all layout variants and model-rebuilding decoder (`probes/p0-07-catalog-encoding/enc`); P0.3's `no_std` reader and evaluator skeleton (`probes/p0-03-runtime-floor/rt`); P0.4's plural rule encoder (`probes/p0-04-plural/rules`); P0.8's load/lookup benches | `probes/` (in the first commit; C5 deletes them — port from them, never depend on them) |

Figures Phase 2 re-measures and must not lose (P0.7/P0.8, phase-0-results):
`en` 50,867 B raw / 20,536 B gz (stripped, recommended layout); structure
6.9–7.3 KB gz of it; `Catalog::new` 5.7 µs native and 0.08 ms at 4× throttle
with 0 allocations and 0 copies; simple lookup 20.7 ns, 1-argument pattern
93.5 ns (P0.3 runtime); the reference `manifest_hash` of the default workload,
**`43e0dc12eeb05ef1`** (P0.7; function set `["integer"]`, which the four
generated locales share, so it is reproducible from
`bench/corpora/workload-1600.json` alone).

Carried — **owner decisions** (none blocks A1–A10):

1. The remote host (it decides the CI `runs-on` label). *Commits are decided:
   agents may stage and commit (`CLAUDE.md`); the first commit holds the
   probes.* C5 (delete `probes/`) is therefore unblocked: do it once A2 and A4
   have ported what they need from P0.3 and P0.7 — the code stays recoverable
   from history.
2. Spec license (#1112): unchanged; consumers go through
   `mf2_conformance::spec::spec_path`.
3. B5/B7/B9 restatements from Phase 0 — confirm or overrule.
4. The D1 gate job: every push, or scheduled (08 §"Status at exit").
5. **New**: the compressor behind B7 in CI (see A8).

Seams kept for catalog text as JS strings
([stretch_goals_after_v1/prob_builtin_strings](stretch_goals_after_v1/prob_builtin_strings.md)
§7) — Phase 2's two: **`StrRef` is opaque** in the public API (nothing outside
the reader assumes "offset into a NUL-terminated pool"), and **loading is one
function** (`Catalog::new` takes the fetched buffer; nothing else constructs a
catalog).

## What Phase 1 changes here

* **Slots come from `mf2_syntax::analyze`.** `mf2-catalog` does not depend on
  the parser (02 §5): the writer takes the slot list (NFC names in slot order)
  as input and resolves each variable reference by NFC equality. So
  `writer::single` takes the slots too; `mf2::compile_str` (P3) and the L3/L4
  harnesses do `parse_model` → `analyze` → `writer::single`. **02 §5 is
  updated accordingly in this change.**
* **NUL.** A model with U+0000 in a string has no syntax form (the serializer
  returns `Error::Nul`) and no catalog form (NUL-terminated pool); the writer
  reports the same condition. Every model a parse produces is writable.
* **The writer accepts invalid models.** Data-model errors (duplicate options,
  mismatched key counts, …) are representable in the data model and must
  round-trip too — the build refuses to ship them, the format does not.
* **Canonical patterns.** The decoder must produce the model's canonical
  pattern form (no empty or adjacent text); `Pattern::push` / `From<Vec<_>>`
  already enforce it.
* **Catch-all values.** `CatchAllKey::value` is part of F1 (other formats set
  it); encode it.
* **Linear time everywhere.** Phase 1's linear-time test found quadratic
  validation. `Catalog::new` walks each section once, the decoder and views
  are linear in the bytes they touch, the writer's deduplication is hashed or
  sorted — and a linear-time test on adversarial catalogs (A7) holds it.
* **The harness pattern is ready**: add `Column::L3` to `HARNESSED`, an `l3`
  module beside `l1`/`l2`, then `cargo xtask conformance-report --promote`.
* **The generator** drives L3 on generated input (A6); **the fuzz workspace**
  takes a second target (A7).

## API of `mf2-catalog` (frozen at exit, with the format)

Phases 3 (runtime) and 5a (build) are written against these. Methods may be
added; changing one after exit needs this document (or its successor) changed
in the same commit. Crate rules: `#![no_std]` + `alloc`,
`#![forbid(unsafe_code)]`; the **reader** is client-path code (no `format!`,
`Debug`/`Display` use, `unwrap`, panicking indexing; `try_reserve*`; the 05 §8
rules); features `writer`, `decode`, `manifest` are build-side only and may use
`alloc` freely.

```rust
// ── reader (client) ─────────────────────────────────────────────────────────
pub struct Catalog { /* private: the owned buffer + validated section offsets */ }
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StrRef(/* private */);                    // opaque (seam)
#[derive(Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CatalogError { Magic, Version, ManifestMismatch, Truncated, SectionTable,
                        MissingSection, Index, Locale, Funcs, Fallback, Names }

impl Catalog {
    pub fn new(bytes: Vec<u8>, expect_manifest: u64) -> Result<Catalog, CatalogError>; // F2, F4, F6
    pub fn format_version(&self) -> u16;
    pub fn manifest_hash(&self) -> u64;
    pub fn locale(&self) -> &str;
    pub fn dir(&self) -> Dir;
    pub fn message_count(&self) -> u32;
    pub fn get(&self, id: MsgId) -> Entry<'_>;             // O(1); never fails after new()
    pub fn text(&self, r: StrRef) -> Option<&str>;          // UTF-8 checked on access (F4)
    pub fn fallback_locale(&self, id: MsgId) -> Option<&str>;   // F7
    pub fn function(&self, index: u32) -> Option<&str>;     // FUNCS: `ns:name`
    pub fn locale_entry(&self, key: u32) -> Option<&[u8]>;  // LOCALE: opaque payload
    pub fn names(&self, id: MsgId) -> Names<'_>;            // NAMES: slot and local names
    pub fn lookup(&self, id: &str) -> Option<MsgId>;        // IDS; None when stripped
}

pub enum Entry<'a> { Simple(StrRef), Pattern(MsgView<'a>), Select(MsgView<'a>), Absent }
```

`MsgView` is a cursor over MESSAGES that the evaluator walks without building a
tree (02 §5): declarations, the selectors and variants of a select, and a
pattern's parts, each as small `Copy` views — text as `StrRef`; operands as a
literal `StrRef` or a `VarRef` (`External(slot)` | `Local(index)`); functions
as a FUNCS index and an option iterator (name `StrRef`, literal-or-`VarRef`
value); markup kind, name and options; keys as `*` or an NFC `StrRef`. Views
never panic; a malformed record ends its iterator (the runtime then formats the
fallback for that message only). The exact view types are written in A2 and
frozen with the format; Phase 3 may add accessors.

```rust
// ── build side ──────────────────────────────────────────────────────────────
#[cfg(feature = "manifest")]
pub struct Manifest { pub ids: Vec<String>, pub slots: Vec<Vec<String>>,
                      pub markup: Vec<Vec<String>>, pub functions: Vec<String> }
// + fn hash(&self) -> u64 (FNV-1a 64 over 02 §3's canonical serialization),
//   fn write(&self) -> Vec<u8>, fn read(&[u8]) -> Result<Manifest, ManifestError>

#[cfg(feature = "writer")]
pub mod writer {
    pub struct Options { pub locale: String, pub dir: Dir, pub strip_cold: bool,
                         pub strip_ids: bool, /* locale entries, fallbacks */ }
    pub fn catalog(manifest: &Manifest, messages: &[Option<&Message<'_>>],
                   options: &Options) -> Result<Vec<u8>, WriteError>;
    /// One message, its own one-entry manifest (`slots` from `mf2_syntax::analyze`).
    pub fn single(message: &Message<'_>, slots: &[&str], options: &Options)
        -> Result<(Vec<u8>, Manifest), WriteError>;
}

#[cfg(feature = "decode")]
pub fn decode(catalog: &Catalog, id: MsgId) -> Result<Message<'_>, DecodeError>;
```

Errors: `CatalogError` is a plain `#[repr(u8)]` enum on the client path; the
build-side errors (`WriteError`, `DecodeError`, `ManifestError`) are
`thiserror` types in `src/error.rs`, behind their features — `Display` must
not be reachable from the reader (the B12 check in A9 proves it).

## Part A — tasks (A1 first; A2–A5 in order; A6–A10 as their inputs exist)

| Task | Deliverable | Done when |
|---|---|---|
| **A1** Byte grammar | Write the complete byte-level grammar of version 1 into 02 §2 **before code**: header (incl. flags and the CLDR-version field), section table, INDEX byte planes and kinds, MESSAGES (from phase-0-results §P0.7's prototype grammar, extended to the full model: attributes and original key spellings via COLD, catch-all values, markup options, literal/variable option values), COLD, NAMES, FUNCS, FALLBACK, LOCALE container (02 §4, §4.1), IDS front coding, STRINGS; writer policies (pool order, dedup, stripping). Hand-worked test vectors for a simple, a pattern and a select message. | 02 §2 has no "as prototyped" left; the vectors are unit tests of A2/A4 |
| **A2** Reader | `crates/mf2-catalog`: `Catalog::new` validating once and linearly (magic, version, hash, ordered non-overlapping sections, unknown kinds skipped, required kinds present, STRINGS last, INDEX monotonic, LOCALE/FUNCS/FALLBACK/NAMES walked); O(1) `get` over the byte planes; `text` with per-access UTF-8; `MsgView` and its views. Port from P0.3. | unit tests (incl. every `CatalogError`, truncation at every byte, bit flips); `cargo build -p mf2-catalog --target wasm32-unknown-unknown --no-default-features`; clippy denies `unwrap_used`, `expect_used`, `indexing_slicing`, `panic` in the reader |
| **A3** Manifest | Feature `manifest`: `Manifest`, `.mf2m` read/write (varints, 02 §5), `hash` per 02 §3. | round trip; **`hash` of the manifest built from `bench/corpora/workload-1600.json` (slots and markup via `analyze`, functions `["integer"]`) equals `43e0dc12eeb05ef1`** |
| **A4** Writer | Feature `writer`: `catalog` and `single`; identifiers-first sorted deduplicated pool, NUL-terminated; INDEX kinds; keys NFC with originals in COLD; FALLBACK and LOCALE from `Options`; stripping of COLD and IDS. | writing the same input twice, and in two processes, gives identical bytes (F8); NUL reported, never written |
| **A5** Decoder | Feature `decode`: `decode` rebuilds the full model (needs COLD; with COLD stripped it decodes the formatting-relevant model and reports what was dropped). | round trip on hand-written models of every node kind (reuse `mf2-model`'s JSON fixtures) |
| **A6** Conformance L3 | `conformance/src/l3.rs`: for every L3-applicable test, `parse_model` → `analyze` → `writer::single` → `Catalog::new` → `decode` → **equal to the L2 model**; the catalog also re-validates with the manifest hash and rejects a wrong one. `HARNESSED` gains L3; `--promote`. The same property on generated input (bounded in `cargo test`, 1,000,000 in the nightly run). A `[[note]]` records "stripped and unstripped catalogs format identically — checked from Phase 3 on (L4)". | L3 301/301 in `REPORT.md`; red on any regression |
| **A7** Decoder fuzzing and linear time | `fuzz/` target `catalog`: arbitrary bytes, and mutated writer output, through `Catalog::new`, every `get`, full view walks and `decode` — no panic, no out-of-bounds, time within a per-byte budget; seeds from `cargo xtask fuzz-seed` (extended to write catalogs). A linear-time test on adversarial catalogs (deep option lists, huge variant counts, long NAMES, many sections). | a ≥ 1 h fuzz run clean, command recorded; the linear-time test in `cargo test` |
| **A8** B7 on the committed corpus | A size report (xtask subcommand or test) over `bench/corpora/workload-1600.json` and the four generated locales (`cargo xtask gen-workload locales`): raw, gzip -9, brotli 11, structure vs pool, stripped and unstripped. Decide with the owner how CI compresses (GNU `gzip -9 -n`, as P0.7 measured, or a Rust implementation — record the difference). | `en` stripped ≤ 25 KB gz; every locale gz ≤ 0.5 × source + 1 KB and raw ≤ 1.25 × source + 8 B/message; no regression beyond noise against P0.7's recommended layout |
| **A9** Reader cost | Native bench (`bench/`, the parser-gate style: committed corpus, medians, allocation counts) of `Catalog::new`, `get` and `text` on the four locales; the **B12 check** for the reader — a `no_std` wasm harness whose `#[panic_handler]` calls an import that must be absent after LTO + `wasm-opt -Oz`, plus a `twiggy` grep for `core::fmt` — wired into CI; the reader's gz size recorded (part of B1). | 0 allocations and 0 copies at load; load and lookup within 1.5× of P0.8's native figures (or the difference explained); B12 clean |
| **A10** Skew and flags | F6: a catalog built against another manifest is rejected with `ManifestMismatch`; F7: `fallback_locale` round-trips; F9: an unknown major version is rejected; stripping: stripped and unstripped catalogs decode to the same formatting-relevant model on the whole suite. | unit and harness tests |

Order: A1 unblocks everything; A2 and A4 are the core and grow together (the
vectors of A1 test both); A3 before A4's `catalog`; A5 needs A2's views; A6 as
soon as A2, A4, A5 exist; A7–A10 alongside.

## Exit (master plan §9, P2)

- [ ] L3 100 % (301/301); `current_phase = "P2"` bumped in the exit commit and
      the harness green
- [ ] the L3 property holds on generated input (1,000,000 messages)
- [ ] deterministic output: identical bytes across runs and processes (F8)
- [ ] B7 met on the committed reference corpus (and the scaled rule for the
      generated locales)
- [ ] decoder fuzz run clean (≥ 1 h, command in the results); the linear-time
      test green
- [ ] reader: `no_std`, `forbid(unsafe_code)`, B12 clean; load allocates
      nothing
- [ ] format version 1 frozen: 02 §2 is the complete byte grammar, with test
      vectors; the API above unchanged (or this document changed in the same
      commit, with the reason)
- [ ] `stripped ≡ unstripped formatting` recorded as open until L4 exists (P3)
- [ ] the Phase 3 work order written from Phase 2's findings
