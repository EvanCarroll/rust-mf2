# 02 — The binary catalog (`.mf2b`)

Part of the [master plan](00-master-plan.md). RFC 2119 keywords apply.

One `.mf2b` file holds everything the runtime needs for **one locale** (and, once
chunking lands, one chunk of it): the messages, and the locale data those
messages need. It is what gets lazily fetched. The wasm contains no message
text, no variable names, no plural rules and no locale symbols.

## 1. Requirements

| # | Requirement | Verified by |
|---|---|---|
| F1 | **Lossless over the full MF2 data model** — declarations, selectors, variants, catch-all key values, literals (quoted-ness is *not* data-model, value is), options, attributes, markup of all three kinds, function namespaces. Decoding yields a model equal to the one compiled. | Conformance L3 |
| F2 | **No per-message work at load, and no copy.** The fetched buffer *is* the catalog: `Catalog::new` takes ownership, validates the structure, and keeps it; sharing is `Arc<Catalog>` (or `Rc` on the client) at the owner. Text is `&str` borrowed from the buffer. Loading allocates nothing (P0.8: 0 allocations, 0 copies). | bench + code review |
| F3 | **O(1) lookup** by `MsgId` — an array index, no hashing, no string compare. | bench |
| F4 | **Validate structure once; text on access.** `Catalog::new(bytes)` checks magic, version, `manifest_hash`, the section table, INDEX bounds and monotonicity, the LOCALE and FUNCS tables. Pool strings are validated with `str::from_utf8` **when read** (NUL-terminated, §2), which P0.8 measured at +0–45 ns per read against up to 1.1 ms per load for a whole-pool pass on non-ASCII text at 4× throttle. No `unsafe`. Accessors never panic; a bad offset or an invalid string yields the fallback representation for that message only. | fuzz target |
| F5 | **No `unsafe` — without exception — and no alignment assumptions.** All integers are little-endian and read with `from_le_bytes` from unaligned slices. | `forbid(unsafe_code)` |
| F6 | **Skew-proof.** Header carries `manifest_hash`; the wasm carries the same constant. A catalog compiled against a different id/slot table is rejected, never misread. | unit + e2e test |
| F7 | **Self-describing locale.** Header carries the BCP-47 tag, the direction, and for every message whether its text came from a fallback locale (and which). | L6 a11y test |
| F8 | **Deterministic.** Same inputs ⇒ byte-identical output (stable hashing, reproducible builds, cacheable URLs). | CI builds twice and compares |
| F9 | **Versioned.** `format_version` bumps on any incompatible change; readers reject unknown majors. The format is an internal build artifact — there is no cross-version compatibility promise, only detection. | unit test |
| F10 | **Compresses well.** String pool is contiguous and deduplicated; structure bytes are separated from text so brotli sees two homogeneous streams. | P0 probe + size gate |

Non-goals: random *write* access, in-place patching, streaming decode, a stable
public interchange format (that role belongs to MF2 source and the spec's JSON
data model, both of which `mf2-cli` can emit).

## 2. Layout

```
┌──────────────────────────────────────────────────────────────┐
│ Header (30 B + 10 B per section)                             │
│   magic "MF2B" · format_version u16 · flags u16              │
│   manifest_hash u64 · chunk u8 · dir u8 (ltr|rtl)            │
│   message_count u32 · locale (off u32, len u16, in STRINGS)  │
│   section_count u16 · section table [kind u16, off u32, len u32]… │
│   (ordered, non-overlapping)                                 │
├──────────────────────────────────────────────────────────────┤
│ INDEX      message_count × u32 (2-bit kind ‖ 30-bit offset), │
│            stored as 4 BYTE PLANES: all byte 0s, then all    │
│            byte 1s, … — still one O(1) read of 4 bytes       │
│ MESSAGES   encoded data-model nodes for non-simple messages, │
│            in MsgId order (offsets strictly increasing)      │
│ COLD       attributes, original key spellings (never read by │
│            the client evaluator; present for F1)             │
│ NAMES      deduplicated per-message variable names (fallback │
│            `{$name}`, dynamic named-arg API)                 │
│ FUNCS      the manifest's function set: sorted `ns:name`     │
│            StrRefs, indexed by MESSAGES' fn_idx              │
│ FALLBACK   table of fallback locales used + sorted sparse    │
│            list (MsgId → index into that table)              │
│ LOCALE     varint n · (varint key · varint len · payload)*;  │
│            locale data sliced to what the corpus uses (§4)   │
│ IDS        (optional) front-coded message ids for            │
│            lookup-by-name; server / CLI / dev builds only    │
│ STRINGS    deduplicated pool of NUL-terminated UTF-8         │
│            strings; StrRef = offset. Always the LAST section │
└──────────────────────────────────────────────────────────────┘
```

Unknown section kinds MUST be skipped (forward-compatible additions do not need
a version bump); required kinds missing ⇒ reject. All integers little-endian,
read from unaligned slices; varints are minimal unsigned LEB128.

**Why NUL-terminated strings** (P0.7): a length prefix of ≥ 128 emits bytes
≥ 0x80 that are not UTF-8, and every length scheme measured — a prefix encoded
as one UTF-8 scalar, offset + length in the structure, an end-offset table —
costs 1.5–3.2 KB gz more per locale, because it interleaves high-entropy bytes
with text or structure. MF2 syntax cannot contain U+0000 (the ABNF excludes
`%x00`), so every message with a syntax form is representable; the writer
rejects a NUL in a data model built in code — the one documented limit of F1.
**Writer policy** (F8, F10): deduplicate; put identifiers (names, keys,
function/option/markup names, the locale tag) first, then text, each group
sorted bytewise — worth 0.4–1.5 KB gz over first-use order, while a separate
identifier section compresses identically. Production client catalogs omit COLD
and IDS (IDS alone is 8.6–9.8 KB gz on the reference workload). The byte-level
grammar of MESSAGES as prototyped is recorded in
[phase-0-results](phase-0-results.md) §P0.7; Phase 2 freezes it.

### 2.1 The INDEX and the three message kinds

| Kind | Meaning | Lookup cost |
|---|---|---|
| `0 simple` | a single text run, no placeholders, no declarations | offset points **straight into STRINGS**; one bounds-checked read and a slice. The evaluator is not entered. |
| `1 pattern` | one pattern with placeholders/markup, maybe declarations | offset into MESSAGES |
| `2 select` | `.match` message | offset into MESSAGES |
| `3 absent` | id exists in the manifest but this chunk does not carry it | triggers chunk load or the missing-message policy |

Real-world UI corpora are overwhelmingly `simple` (the reference workload in
[06-size-and-perf](06-size-and-perf.md): 79 % of messages have no placeholder at
all, 0.9 % select). The format and the API make that case nearly free.

### 2.2 MESSAGES encoding

A compact tagged byte stream with LEB128 varints, mirroring the spec data model
one-to-one:

* `Message  := decl_count Declaration* Body`
* `Declaration := Input(VarRef, Expr) | Local(LocalIdx, Expr)`
* `Body := Pattern | Select(selector_count VarRef* variant_count Variant*)`
* `Variant := Key* Pattern` · `Key := Literal(StrRef) | CatchAll(Option<StrRef>)`
* `Pattern := part_count Part*` · `Part := Text(StrRef) | Expr | Markup`
* `Expr := Operand? FunctionRef? ColdRef` · `Operand := Literal(StrRef) | VarRef`
* `FunctionRef := FnIdx option_count (StrRef name, Literal|VarRef value)*`
* `Markup := kind(open|standalone|close) StrRef name options ColdRef`

Design points:

* **Variables are slots.** `VarRef` is `External(slot)` or `Local(idx)`. Slot
  numbers come from the manifest (see [05-tooling](05-tooling.md)) and are the
  same in every locale, so the call site passes arguments positionally and the
  wasm holds no argument names. The name is recoverable through NAMES for
  fallback output and for F1.
* **Functions are indices** into the catalog's FUNCS table (`StrRef` of the
  full `ns:name`). P0.3 resolved names against the closed-world registry at
  call time — a few short compares, no load-time allocation, and one fewer
  panic path; Phase 3 resolves once per catalog load instead only if it
  measures a real per-call cost.
* **Keys are stored NFC-normalized** (the original spelling is kept in COLD when
  it differs, for F1).
* **Variants stay in source order.** Selection uses the spec's general algorithm
  (filter, then sort by `BetterThan`). Pre-sorting is a tempting optimization but
  is only sound when `BetterThan` is statically known, which custom functions
  break; with typical variant counts < 10 the general algorithm costs nothing
  measurable and keeps one code path.
* **It is a data-model encoding, not a bytecode.** A lowered bytecode would be
  marginally faster and would make F1 unprovable. Acceleration lives in side
  information (kind bits, slots, function indices, interned strings), never in a
  lossy rewrite.

### 2.3 Production stripping

`mf2-build` MAY omit COLD and IDS for production client catalogs
(`strip = ["cold", "ids"]`), since neither affects formatting. Conformance L3
always runs unstripped; a separate test asserts stripped and unstripped catalogs
format identically across the whole suite.

## 3. Identity, hashing, delivery

* `MsgId` is `u32`: **low 24 bits index, high 8 bits chunk** (16.7 M messages,
  256 chunks). Until chunking is implemented every message is in chunk 0, but
  the bits are reserved **now** so adding per-route catalogs later changes no
  call site and no format version.
* `manifest_hash` = FNV-1a 64 over a canonical byte serialization of: the
  ordered list of (message id, ordered variable-slot names, markup names) **and
  the set of function names the corpus uses**. Those are exactly the things the
  wasm was compiled against (id table, slots, handlers, closed-world registry).
  A translation edit changes the hash only if it introduces a function no
  message used before — in which case the wasm really does have to change. The
  lists that merely slice locale data (currencies, units, numbering systems) are
  **not** in the hash.
* File name: `<locale>[.<chunk>].<content-hash>.mf2b`, served
  `Cache-Control: public, max-age=31536000, immutable`, with `.br` and `.gz`
  siblings produced at build time. A translation fix changes one locale's URL
  and nothing else; a wasm rebuild does not invalidate any catalog whose
  manifest hash is unchanged.
* The server tells the client which URL to fetch (boot data + `preload`); the
  client never guesses a hash. See [04-leptos-integration](04-leptos-integration.md).

## 4. LOCALE section — locale data, sliced

Locale data is lazy for the same reason text is: it scales with locale count.
The section is a small keyed table; each entry exists only if the corpus needs
it (closed-world slicing, decided by `mf2-build` from the functions and literal
option values actually used).

| Entry | Present when | Content |
|---|---|---|
| `plural.cardinal` | any `:number`/`:integer`/`:offset`/`:percent` selection with `select=plural` (the default) | CLDR rule for this locale as a tiny condition table: per category, OR of ANDs of `(operand n/i/v/w/f/t/c/e, modulus, negated, ranges)` |
| `plural.ordinal` | any `select=ordinal` | same encoding |
| `number.symbols` | any numeric formatter | decimal, group, minus, plus, percent, per-mille, exponent, infinity, NaN; grouping sizes; minimum grouping digits; default numbering system digits |
| `number.systems` | a literal `numberingSystem=` is used, or the locale default is non-Latin | digit sets for exactly those systems |
| `number.patterns` | `:percent`, `:currency`, `:unit` | the locale's patterns for the used styles |
| `currency.*` | `:currency` | symbols / narrow symbols / display names / fraction digits for the **configured currency set** (default: literal codes found in the corpus; `currencies = "all"` opts into the full table) |
| `unit.*` | `:unit` | patterns for the configured unit set, same policy |
| `icu.blob` | feature `datetime-icu` and the corpus formats dates | an ICU4X data blob for this locale, restricted to the markers the corpus needs. The ICU4X *code* is in the wasm (that is the feature's cost); the *data* is here, lazy, like everything else |

The CLDR version used is recorded in the header flags/metadata and printed by
`mf2 stats`. CLDR JSON is a build-time input only, pinned like the spec.

With `datetime-intl` the client needs no date data at all, so no `icu.blob` is
emitted.

**Container first, entries later.** Phase 2 freezes the LOCALE *container* (a
keyed table of versioned, opaque entries; unknown keys are skipped) and the two
`plural.*` entries, whose encoding Phase 0 probe P0.4 prototyped and verified
(§4.1). The number, currency, unit and date entries are defined in Phase 4 as
additive entry kinds, which by the skip rule need no format-version bump.

### 4.1 `plural.cardinal` / `plural.ordinal` entry, v1 (from P0.4)

Verified against all 15,041 CLDR 48.2.1 samples (224 cardinal, 108 ordinal
locales) and against ICU4X; results in [phase-0-results](phase-0-results.md)
§P0.4. The entry is the whole payload of one LOCALE entry; the container gives
its length and, through the key, its version. **An empty payload is valid** and
means every number is `other` (`ja`, `zh`, most ordinals). The entry is present
— possibly empty — whenever the manifest says the corpus selects on plural
(resp. ordinal); `Catalog::new` walks it once for structure (F4), and the
evaluator still answers `other` on malformed data.

```text
entry       := rule*
rule        := rule_hdr or_group{G}
rule_hdr    := u8   bits 7..5  C  category: 0 zero, 1 one, 2 two, 3 few, 4 many (5..7 reserved)
                    bits 4..0  G  number of OR groups, 1..31
or_group    := relation+           ; an AND of relations; its last relation has L = 1
relation    := rel_hdr [modulus] item+
rel_hdr     := u8   bit 7      L  last relation of its OR group
                    bit 6      X  negated (`!=`)
                    bits 5..3  M  modulus: 0 none; k = 1..6 → `% 10^k`; 7 → explicit modulus follows
                    bits 2..0  O  operand: 0 n, 1 i, 2 v, 3 w, 4 f, 5 t, 6 c (= e); 7 reserved
modulus     := leb128 (≥ 1)        ; only when M = 7
item        := leb128(lo << 2 | R << 1 | E) [leb128(hi − lo) if R = 1]
                    R = 1: range lo..hi, hi > lo;  R = 0: single value lo
                    E = 1: last item of the relation's list
leb128      := unsigned LEB128, minimal, ≤ 10 bytes, u64
```

* **Evaluation.** Rules in order; the first that holds gives the category,
  else `other`. A rule holds if any OR group holds; a group if all its
  relations hold. A relation takes the operand value `x`, applies `x %= m`
  when it has a modulus, and `hit` = (operand integral) ∧ ∃ item `lo ≤ x ≤ hi`;
  it holds iff `hit ≠ X`. Only `n` can be non-integral (when `t ≠ 0`), so
  `n = …` is then false and `n != …` true (UTS #35: ranges match integers).
  Truncation, `O = 7`, modulus 0, an over-long varint or `hi` overflow stop
  evaluation with `other`; nothing panics.
* **Canonical writer** (F8): rules in category order zero < one < two < few <
  many, `other` never written; groups, relations and items in source order;
  minimal LEB128; a range with `lo = hi` written as a single value; moduli
  10^1…10^6 use the short code. CLDR categories are disjoint, so the order only
  serves byte identity.
* **Operands contract** (Phase 3 formatter → evaluator):
  `{ i, f, t: u64, v, w, e: u32 }`, `n` derived. Values ≥ 10¹⁸ are stored as
  10¹⁸ + (value mod 10¹⁸) — exact for every modulus dividing 10¹⁸ and every
  literal below it, which covers CLDR 48 (largest modulus and literal 10⁶);
  `mf2-locale-data` asserts both at build time. MF2's `:number` has no compact
  notation, so `e = 0` from MF2 core; the evaluator supports it for CLDR.
* **Sizes** (CLDR 48): 40 distinct cardinal and 25 ordinal rule sets, 937 B
  together; per locale 0–84 B (median 3.5 B) for both entries. `en` cardinal is
  `21 01 05 82 01`. Evaluator: 429 B gz. Locales without ordinal rules (116 of
  224) resolve by subtag truncation, then root (empty entry).

## 5. Reader API sketch (`mf2-catalog`, `no_std`)

```rust
pub struct Catalog { bytes: Vec<u8>, /* validated section offsets */ }  // shared as Arc<Catalog> / Rc<Catalog>

impl Catalog {
    pub fn new(bytes: Vec<u8>, expect_manifest: u64) -> Result<Self, CatalogError>; // validates structure; no copy
    pub fn locale(&self) -> &str;
    pub fn dir(&self) -> Dir;
    pub fn get(&self, id: MsgId) -> Entry<'_>;        // never fails after new()
    pub fn text(&self, r: StrRef) -> Option<&str>;    // UTF-8 checked on access (F4)
    pub fn fallback_locale(&self, id: MsgId) -> Option<&str>; // Some(locale) if borrowed text
}

pub enum Entry<'a> { Simple(StrRef), Pattern(MsgView<'a>), Select(MsgView<'a>), Absent }
```

`StrRef` is opaque (a seam kept for catalog text as JS strings: nothing
outside the reader may assume "offset into a NUL-terminated pool"), which is
why even a simple entry yields a `StrRef` rather than a `&str`. The full API
Phase 2 builds, and freezes with the format, is in
[09](09-phase-2-work-order.md).

`MsgView` is a cursor over MESSAGES; the evaluator walks it without building an
intermediate tree. Non-client code is behind features the client never enables:

* `writer` — the encoder, including **`writer::single(&Message, slots, options)`**,
  which emits a one-message catalog (and its one-entry manifest) from a
  data-model value. This is what conformance L3/L4, ad-hoc server-side
  formatting and the CLI use, long before `mf2-build` exists. It takes a model
  **and its slot list** — the external variables in ascending bytewise order of
  their NFC names, which `mf2_syntax::analyze` computes — so `mf2-catalog`
  never depends on the parser (decided at the close of Phase 1; the API is in
  [09](09-phase-2-work-order.md)).
* `decode` — the model-rebuilding decoder used by L3.
* `manifest` — reader and writer for the **manifest file** (`manifest.mf2m`: the
  id table, per-message slot names and markup names, the function set, the
  hash), in the same varint encoding. `mf2-build` writes it; `mf2-macros` reads
  it. Slot order is ascending bytewise order of the NFC-normalized variable name.

## 6. Points settled by Phase 0

Measured by P0.7 and P0.8 ([phase-0-results](phase-0-results.md)); Phase 2
freezes them.

* **Pools**: one STRINGS section; the writer groups identifiers first and sorts
  each group (a separate identifier section compresses identically).
* **String lengths**: NUL termination (§2), the smallest option and the only
  one compatible with a valid UTF-8 pool.
* **Load step**: no split and no copy; structure validated at load, UTF-8 per
  string on access (F2, F4) — load ≤ 0.08 ms at 4× throttle for every locale.
* **INDEX**: fixed 4-byte entries stored as byte planes (0.5–1.4 KB gz smaller
  than row-major, smaller than varint-delta, and still O(1)).
* **FUNCS** section added; LOCALE container framing fixed (§2, §4.1).
