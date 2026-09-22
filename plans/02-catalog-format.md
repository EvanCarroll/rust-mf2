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

## 2. Layout — format version 1

Frozen at the exit of Phase 2 as **version 1.0**. This section is the complete
byte-level grammar; the writer (`mf2-catalog`, feature `writer`), the reader and
the decoder implement exactly this, and the vectors of §2.10 are unit tests of
all three.

```
┌──────────────────────────────────────────────────────────────┐
│ Header (32 B + 10 B per section, §2.4)                       │
│   magic "MF2B" · format_version u16 · flags u16              │
│   manifest_hash u64 · message_count u32 · locale str32       │
│   cldr_version u32 · chunk u8 · dir u8 (ltr|rtl)             │
│   section_count u16 · section table [kind u16, off u32, len u32]… │
│   (ordered, non-overlapping)                                 │
├──────────────────────────────────────────────────────────────┤
│ INDEX      message_count × u32 (2-bit kind ‖ 30-bit offset), │
│            stored as 4 BYTE PLANES: all byte 0s, then all    │
│            byte 1s, … — still one O(1) read of 4 bytes       │
│ MESSAGES   encoded data-model nodes for non-simple messages, │
│            in MsgId order (offsets strictly increasing)      │
│ COLD       attributes, original spellings, catch-all values  │
│            (never read by the client evaluator; F1)          │
│ NAMES      deduplicated per-message variable names (fallback │
│            `{$name}`, dynamic named-arg API)                 │
│ FALLBACK   table of fallback locales used + sorted sparse    │
│            list (MsgId → index into that table)              │
│ LOCALE     varint n · (varint key · varint len · payload)*;  │
│            locale data sliced to what the corpus uses (§4)   │
│ FUNCS      the manifest's function set: sorted `ns:name`     │
│            str32s, indexed by MESSAGES' function index       │
│ IDS        (optional) front-coded message ids for            │
│            lookup-by-name; server / CLI / dev builds only    │
│ STRINGS    deduplicated pool of NUL-terminated UTF-8         │
│            strings; StrRef = offset. Always the LAST section │
└──────────────────────────────────────────────────────────────┘
```

**Conventions.**

* Integers (`u8`, `u16`, `u32`, `u64`) are little-endian and read from
  unaligned slices (F5).
* `varint` is unsigned LEB128, **minimal** (a varint of more than one byte does
  not end in `0x00`), at most 5 bytes, value ≤ 2³² − 1. A reader rejects any
  other form. (The plural entries of LOCALE use their own u64 LEB128, §4.1.)
* A **StrRef** is the offset of a string's first byte from the start of
  STRINGS. The string runs to the next `0x00`, exclusive, and is valid UTF-8 —
  checked when read (F4). In MESSAGES and COLD a StrRef is a `varint`; in the
  header, NAMES, FUNCS and FALLBACK it is a `u32`, written **`str32`**, so those
  tables index in O(1).
* Counts before lists are `varint`s unless stated otherwise.

Unknown section kinds MUST be skipped (forward-compatible additions do not need
a version bump); required kinds missing ⇒ reject.

**Cost bounds.** `Catalog::new` is linear in the file. Every later operation —
a lookup, a string, a view step, a decode — is linear in the bytes it reads and
never reads past the file. Records and strings carry no length, so a corrupt
or hostile catalog can make a message's walk run on through the records after
it, and every reference read the same long string: walking every message is
then O(messages × size) (P2 A7 built such a catalog). It can never make an
access panic or read out of bounds. Catalogs are content-hashed build
artifacts served by the application's own server, so this is robustness, not a
security boundary; a length per record would bound records (1–2 B per
message) but not strings, short of giving up F4's per-access UTF-8 check.

**Why NUL-terminated strings** (P0.7): a length prefix of ≥ 128 emits bytes
≥ 0x80 that are not UTF-8, and every length scheme measured — a prefix encoded
as one UTF-8 scalar, offset + length in the structure, an end-offset table —
costs 1.5–3.2 KB gz more per locale, because it interleaves high-entropy bytes
with text or structure. MF2 syntax cannot contain U+0000 (the ABNF excludes
`%x00`), so every message with a syntax form is representable; the writer
rejects a NUL in a data model built in code — the one documented limit of F1.
Writer policies (pool order, deduplication, stripping) are in §2.9.

### 2.1 The INDEX and the four message kinds

| Kind | Meaning | Lookup cost |
|---|---|---|
| `0 simple` | a single text run, no placeholders, no declarations | offset points **straight into STRINGS**; one bounds-checked read and a slice. The evaluator is not entered. |
| `1 pattern` | one pattern with placeholders/markup, maybe declarations | offset into MESSAGES |
| `2 select` | `.match` message | offset into MESSAGES |
| `3 absent` | id exists in the manifest but this chunk does not carry it | triggers chunk load or the missing-message policy |

Real-world UI corpora are overwhelmingly `simple` (the reference workload in
[06-size-and-perf](06-size-and-perf.md): 79 % of messages have no placeholder at
all, 0.9 % select). The format and the API make that case nearly free.

```text
INDEX    := byte plane 0 · byte plane 1 · byte plane 2 · byte plane 3
            ; plane p = byte p of entry 0, byte p of entry 1, …; length exactly 4 × message_count
entry    := u32  kind << 30 | offset
            kind 0 simple:  offset is a StrRef (< STRINGS length) — the message text
            kind 1 pattern, 2 select: offset into MESSAGES (< MESSAGES length); over the
                            pattern and select entries, in MsgId order, strictly increasing
            kind 3 absent:  offset 0 (ignored by readers)
```

A message is **simple** when it is a pattern message without declarations
whose pattern is empty or one text part.

### 2.2 MESSAGES encoding

A compact tagged byte stream with LEB128 varints, mirroring the spec data model
one-to-one. MESSAGES holds the **formatting-relevant** model: names, keys and
function identifiers in NFC, variables as slots. Everything else a lossless
round trip needs — attributes, catch-all key values, the original spelling of a
name or key whose written form is not its NFC form — is in COLD (§2.5), which
production catalogs strip.

```text
Message     := varint names · varint decls · [varint cold] · Decl{decls >> 1} · Body
               names = 0: no NAMES entry; else the entry's offset in NAMES + 1 (§2.6)
               decls = decl_count << 1 | c
               c = 1: `cold` follows, the offset of this message's record in COLD
Body        := Pattern                                               ; INDEX kind 1
             | varint nsel · VarRef{nsel} · varint nvar · Variant{nvar}  ; INDEX kind 2
Variant     := varint nkeys · Key{nkeys} · varint plen · Pattern     ; plen = byte length of Pattern
Key         := varint k        ; 0: the catch-all key `*`
                               ; k ≥ 1: a literal key, its NFC value at StrRef k − 1
Pattern     := varint nparts · Part{nparts}
Part        := u8 tag · …      ; tag bits 0–2 select the part
    0 TEXT        tag = 0x00                            · StrRef text
    1 EXPRESSION  tag = 0x01 | op << 3 | f << 5          · Expr
    2 OPEN        tag = 0x02 | o << 7                   · StrRef name · [Options if o = 1]
    3 STANDALONE  tag = 0x03 | o << 7                   · StrRef name · [Options if o = 1]
    4 CLOSE       tag = 0x04 | o << 7                   · StrRef name · [Options if o = 1]
                  (markup names in NFC)
Decl        := u8 tag · Expr   ; tag = 0x01 | op << 3 | f << 5 | l << 7
                               ; l = 0 `.input` (then op = 2: its variable), l = 1 `.local`
Expr        := [Operand] · [FunctionRef]
               op = 0 no operand (then f = 1: a function-only expression), 1 literal, 2 variable
               f = 1: a FunctionRef follows
Operand     := StrRef value (op = 1) | VarRef (op = 2)
FunctionRef := varint fn · Options          ; fn: index into FUNCS (§2.6)
Options     := varint n · (StrRef name · Value){n}                  ; option names in NFC
Value       := varint v        ; v even: a literal, its value at StrRef v >> 1
                               ; v odd:  a variable, VarRef v >> 1
VarRef      := varint r        ; r = index << 1 | l
                               ; l = 0 external: index = slot (the manifest's slot order)
                               ; l = 1 local: index = position among the message's `.local`s
```

Any tag bit not listed above set, a part kind ≥ 5, `op = 3`, `op = 0` without
`f`, or an `.input` whose `op ≠ 2` is malformed. So is a pattern with an empty
TEXT part or two TEXT parts in a row — the data model's patterns have neither;
the decoder rejects them (an evaluator may print them as they are).

A reference is **local** if a `.local` declaration whose name is NFC-equal to it
precedes the reference (a declaration's own expression does not see the name
it binds); its index is that of the last such declaration. Otherwise it is
**external**: its slot is the position of its NFC name in the manifest's slot
list for the message. An `.input` declares the name of its variable — which is
a local when a `.local` of that name precedes it (a Duplicate Declaration
error, still representable). A variable's name is recovered through NAMES: slot
names in NFC, local names as declared; a reference spelled differently has an
override in COLD.

Design points:

* **Variables are slots.** `VarRef` is `External(slot)` or `Local(idx)`. Slot
  numbers come from the manifest (see [05-tooling](05-tooling.md)) and are the
  same in every locale, so the call site passes arguments positionally and the
  wasm holds no argument names. The name is recoverable through NAMES for
  fallback output and for F1.
* **Functions are indices** into the catalog's FUNCS table (`str32` of the
  full `ns:name`). P0.3 resolved names against the closed-world registry at
  call time — a few short compares, no load-time allocation, and one fewer
  panic path; Phase 3 resolves once per catalog load instead only if it
  measures a real per-call cost.
* **Keys, names and identifiers are stored NFC-normalized** — what the spec
  compares (syntax.md, "Names and Identifiers"; formatting.md, NormalizeKey).
  The original spelling is kept in COLD when it differs, for F1.
* **Variants stay in source order**, each with its own key count (a Variant
  Key Mismatch is representable). Selection uses the spec's general algorithm
  (filter, then sort by `BetterThan`). Pre-sorting is a tempting optimization
  but is only sound when `BetterThan` is statically known, which custom
  functions break; with typical variant counts < 10 the general algorithm
  costs nothing measurable and keeps one code path. `plen` lets the evaluator
  skip a variant's pattern in O(1).
* **It is a data-model encoding, not a bytecode.** A lowered bytecode would be
  marginally faster and would make F1 unprovable. Acceleration lives in side
  information (kind bits, slots, function indices, interned strings), never in a
  lossy rewrite.

### 2.3 Production stripping

`mf2-build` MAY omit COLD and IDS for production client catalogs
(`strip = ["cold", "ids"]`), since neither affects formatting. Stripping
removes the two sections, sets header flag bits 0 and 1 and leaves out of the
pool the strings only COLD used; messages keep `c = 1` (with `cold` written as
0), so a decoder can tell exactly which messages lost attributes, catch-all
values or spellings. Decoding a stripped catalog gives the
formatting-relevant model: keys and function, option and markup names in NFC;
each variable reference spelled as the `.local` it resolves to declares it,
or else as its NFC slot name; no attributes, no catch-all values; `.local`
names, text and literals unchanged (`conformance/src/l3.rs`,
`formatting_model`). Conformance L3 always runs unstripped; a separate test
asserts stripped and unstripped catalogs decode to the same formatting-relevant
model across the whole suite, and — from Phase 3 on (L4) — format identically.

### 2.4 Header and section table

```text
offset size field
 0     4    magic           "MF2B" (4D 46 32 42)
 4     2    format_version  u16  major << 8 | minor; this document is 0x0100 (1.0).
                            A reader rejects a major it does not know (F9); a minor
                            bump only adds (sections, LOCALE keys, flags).
 6     2    flags           u16  bit 0: COLD stripped; bit 1: IDS stripped (§2.3);
                            other bits written 0, ignored by readers
 8     8    manifest_hash   u64  (§3); must equal the reader's expected value (F6)
16     4    message_count   u32  ≤ 2²⁴: the manifest's id count = INDEX entries
20     4    locale          str32  the BCP 47 tag
24     4    cldr_version    u32  major << 16 | minor << 8 | patch of the CLDR data in
                            LOCALE (48.2.1 → 0x00300201); 0 = none
28     1    chunk           u8   the MsgId chunk this catalog holds (0 until chunking)
29     1    dir             u8   0 ltr, 1 rtl
30     2    section_count   u16
32     10·n section table   n × (kind u16, offset u32, length u32); offsets from the
                            start of the file
```

Section kinds: `1` INDEX, `2` MESSAGES, `3` COLD, `4` NAMES, `5` FALLBACK,
`6` LOCALE, `7` FUNCS, `8` IDS, `15` STRINGS; all others unknown (skipped).
INDEX, MESSAGES, NAMES, LOCALE, FUNCS and STRINGS are **required** (all but
STRINGS may be empty); COLD, FALLBACK and IDS are optional. Rules, checked by
`Catalog::new`: the table lies inside the file; entries are in increasing
offset order, each section starting at or after the end of the previous one
(the first at or after the end of the table) and ending inside the file; a
known kind appears at most once; STRINGS is the last entry and ends exactly at
the end of the file. The writer emits sections in ascending kind order,
STRINGS last, without gaps.

### 2.5 COLD — what only a lossless decode needs

```text
COLD       := MsgCold*                        ; one record per message that needs one
MsgCold    := varint n · Override{n}          ; n ≥ 1
Override   := varint gap · u8 kind · payload
              site = previous site + 1 + gap  (the first: site = gap)
    kind 0 SPELLING    StrRef                 ; the name or key as written
    kind 1 ATTRIBUTES  varint m · (StrRef name · varint a){m}
                       ; a = 0: no value (the spec's `true`); else the value at StrRef a − 1
    kind 2 CATCH-ALL   StrRef                 ; the catch-all key's value (other formats)
```

**Sites** are numbered from 0 per message, in the order their bytes appear in
the message's MESSAGES record:

| Site | Where | Override kinds |
|---|---|---|
| expression | each `Decl`, and each EXPRESSION part, before its operand | ATTRIBUTES |
| markup | each OPEN/STANDALONE/CLOSE part, before its name | ATTRIBUTES |
| variable | each `VarRef` (`.input` variable, operand, option value, selector) | SPELLING |
| function | the identifier of each `FunctionRef` | SPELLING |
| option name | each option name (functions and markup) | SPELLING |
| markup name | each markup name | SPELLING |
| key | each `Key` | SPELLING (literal), CATCH-ALL (`*`) |

So an expression is: expression site, variable site (if its operand is a
variable), function site, then per option an option-name site and (if its value
is a variable) a variable site. Markup: markup site, markup-name site, then its
options. A select body: a variable site per selector, then per variant its key
sites and its pattern. A SPELLING override is written only where the spelling
differs from what MESSAGES and NAMES give: the NFC form for names, keys and
function identifiers, and for a local variable the name its declaration wrote.
Sites strictly increase within a record; an override at a site of the wrong
type, or left over after the walk, is malformed.

### 2.6 NAMES, FUNCS, FALLBACK

```text
NAMES      := Entry*
Entry      := varint n_ext · varint n_local · str32{n_ext + n_local}
              ; the message's slot names (the manifest's, NFC, in slot order),
              ; then its local names (as declared, in declaration order)
FUNCS      := str32*                          ; length a multiple of 4
              ; the manifest's function set, bytewise ascending NFC identifiers
FALLBACK   := varint n · str32{n} · u32*      ; n ≤ 256 fallback locale tags, then the
              ; sparse list: u32 = msg_index | locale_index << 24, msg_index strictly
              ; increasing and < message_count, locale_index < n; the rest of the
              ; section, a multiple of 4
```

A NAMES entry is fixed-width after its two counts, so a name is one O(1) read.
Only pattern and select messages have one (`names ≥ 1`), and only when they
have slots or locals; the writer deduplicates identical entries. FALLBACK is
present only when some message's text came from a fallback locale (F7): its
lookup is a binary search.

### 2.7 LOCALE container

```text
LOCALE     := varint n · (varint key · varint len · u8{len}){n}   ; keys strictly increasing
```

The key names the entry kind **and its version**; unknown keys are skipped by
length. Version 1 defines key 1 `plural.cardinal` and key 2 `plural.ordinal`
(§4.1), which `Catalog::new` walks for structure; Phase 4 adds key 3
`number.symbols`, key 4 `number.patterns`, key 16 `currency.data`, key 32
`unit.data` and key 48 `icu.blob` (§4.2, §4.3, §4.6, §4.7, §4.9), which it
does not walk (their views check what they read; ICU4X checks the blob);
§4 lists the keys and their ranges.

### 2.8 IDS — lookup by name

```text
IDS        := u32 restart{⌈message_count / 16⌉} · Id{message_count}
Id         := varint shared · varint len · u8{len}
              ; the id is the first `shared` bytes of the previous id followed by the
              ; `len` bytes; every 16th id (0, 16, 32, …) has shared = 0 and
              ; restart[k] = its offset from the first Id
```

Ids are the manifest's, in MsgId order (bytewise ascending), so
`Catalog::lookup` binary-searches the restart points and scans at most 16 ids.
The writer writes the longest possible `shared`; a reader MUST NOT rely on it
(any `shared` up to the previous id's length decodes the same ids — the fuzz
target found a reader that assumed it).

### 2.9 STRINGS and writer policies

`STRINGS` is the pool: NUL-terminated UTF-8 strings, back to back; its last
byte is `0x00` (it is never empty: it holds the locale tag).

The writer (F8, F10):

* **Deterministic**: output depends only on the manifest, the messages and the
  options — never on hash-map order, time or paths. Writing the same input
  twice, or in two processes, gives identical bytes.
* **Pool**: every distinct string once (exact bytes); **identifiers first, then
  text, each group sorted bytewise** — worth 0.4–1.5 KB gz over first-use order,
  while a separate identifier section compresses identically (P0.7).
  Identifiers: locale tags (header, FALLBACK), FUNCS entries, NAMES names,
  option names, literal option values, markup names, keys, COLD spellings,
  attribute names. Text: text parts, simple messages, literal operands,
  attribute values, catch-all values. A string used both ways is an identifier.
* NAMES entries deduplicated and ordered most-referenced first (ties in
  first-use order), so most messages' `names` value is one byte; COLD records
  in message order; one COLD record
  only for a message that needs one.
* **Rejects**: U+0000 in any string; a variable neither local in scope nor in
  the message's slot list; a function outside the manifest's set; an `.input`
  whose name differs from its variable's; direction `auto`; sizes beyond the
  format (`message_count > 2²⁴`, MESSAGES or STRINGS ≥ 2³⁰ bytes, more than 256
  fallback locales). Everything else in the data model is accepted —
  including data-model errors (duplicate options, key-count mismatches, …):
  the build refuses to ship them, the format does not.
* **Stripping** (§2.3) omits COLD and IDS. Production client catalogs are
  stripped: IDS alone is 8.6–9.8 KB gz on the reference workload (P0.7).

### 2.10 Test vectors

Three one-message catalogs as `writer::single` writes them (id `""`, locale
`en`), in hex. Each is a unit test of the writer (these exact bytes) and of the
reader and decoder (these values).

**V1 — simple**: `Hello`, stripped. Manifest `{ids [""], slots [[]], markup
[[]], functions []}`, canonical serialization `01 00 00 00 00`,
`manifest_hash` `d80d6caea7dc7eec`. Pool `en␀Hello␀` (identifiers `en`; text
`Hello` at 3). 106 bytes.

```text
00  4D 46 32 42  00 01  03 00  EC 7E DC A7 AE 6C 0D D8   magic, v1.0, flags 3 (stripped), hash
10  01 00 00 00  00 00 00 00  00 00 00 00  00 00  06 00   count 1, locale @0, cldr 0, chunk 0, ltr, 6 sections
20  01 00 5C 00 00 00 04 00 00 00                        INDEX     @92  len 4
2A  02 00 60 00 00 00 00 00 00 00                        MESSAGES  @96  len 0
34  04 00 60 00 00 00 00 00 00 00                        NAMES     @96  len 0
3E  06 00 60 00 00 00 01 00 00 00                        LOCALE    @96  len 1
48  07 00 61 00 00 00 00 00 00 00                        FUNCS     @97  len 0
52  0F 00 61 00 00 00 09 00 00 00                        STRINGS   @97  len 9
5C  03 00 00 00                                          INDEX: simple, StrRef 3 (one entry, four planes)
60  00                                                   LOCALE: 0 entries
61  65 6E 00 48 65 6C 6C 6F 00                           STRINGS: "en" "Hello"
```

**V2 — pattern**, unstripped: `Hi {$user :string @x}!`. Manifest `{ids [""],
slots [["user"]], markup [[]], functions ["string"]}`, serialization
`01 00 01 04 75 73 65 72 00 01 06 73 74 72 69 6E 67`, hash `ac37ed9e17d00628`.
Pool: identifiers `en`@0 `string`@3 `user`@10 `x`@15, text `!`@17 `Hi `@19.
174 bytes.

```text
00  4D 46 32 42  00 01  00 00  28 06 D0 17 9E ED 37 AC   magic, v1.0, flags 0, hash
10  01 00 00 00  00 00 00 00  00 00 00 00  00 00  08 00   count 1, locale @0, cldr 0, chunk 0, ltr, 8 sections
20  01 00 70 00 00 00 04 00 00 00                        INDEX     @112 len 4
2A  02 00 74 00 00 00 0C 00 00 00                        MESSAGES  @116 len 12
34  03 00 80 00 00 00 06 00 00 00                        COLD      @128 len 6
3E  04 00 86 00 00 00 06 00 00 00                        NAMES     @134 len 6
48  06 00 8C 00 00 00 01 00 00 00                        LOCALE    @140 len 1
52  07 00 8D 00 00 00 04 00 00 00                        FUNCS     @141 len 4
5C  08 00 91 00 00 00 06 00 00 00                        IDS       @145 len 6
66  0F 00 97 00 00 00 17 00 00 00                        STRINGS   @151 len 23
70  00 00 00 40                                          INDEX: pattern, MESSAGES offset 0
74  01                                                   names 1: NAMES entry @0
75  01 00                                                decls 1: 0 declarations, c 1; cold @0
77  03                                                   pattern: 3 parts
78  00 13                                                  TEXT "Hi " (StrRef 19)
7A  31 00 00 00                                            EXPRESSION op variable, f: $ slot 0, fn 0, 0 options
7E  00 11                                                  TEXT "!" (StrRef 17)
80  01 00 01 01 0F 00                                     COLD: 1 override, site 0 (the expression), ATTRIBUTES: @x (StrRef 15), no value
86  01 00 0A 00 00 00                                     NAMES: 1 slot, 0 locals, "user" (str32 10)
8C  00                                                   LOCALE: 0 entries
8D  03 00 00 00                                          FUNCS: "string" (str32 3)
91  00 00 00 00  00 00                                   IDS: restart[0] = 0; id: shared 0, len 0 ("")
97  65 6E 00 73 74 72 69 6E 67 00 75 73 65 72 00 78 00 21 00 48 69 20 00
```

**V3 — select**, stripped, with `en`'s `plural.cardinal` (`21 01 05 82 01`, §4.1)
and CLDR 48.2.1: `.input {$n :integer} .match $n 1 {{one}} * {{{$n} items}}`.
Manifest `{ids [""], slots [["n"]], markup [[]], functions ["integer"]}`,
serialization `01 00 01 01 6E 00 01 07 69 6E 74 65 67 65 72`, hash
`204fb910f49f3de2`. Pool: identifiers `1`@0 `en`@2 `integer`@5 `n`@13, text
` items`@15 `one`@22. 163 bytes.

```text
00  4D 46 32 42  00 01  03 00  E2 3D 9F F4 10 B9 4F 20   magic, v1.0, flags 3 (stripped), hash
10  01 00 00 00  02 00 00 00  01 02 30 00  00 00  06 00   count 1, locale @2, cldr 48.2.1, chunk 0, ltr, 6 sections
20  01 00 5C 00 00 00 04 00 00 00                        INDEX     @92  len 4
2A  02 00 60 00 00 00 17 00 00 00                        MESSAGES  @96  len 23
34  04 00 77 00 00 00 06 00 00 00                        NAMES     @119 len 6
3E  06 00 7D 00 00 00 08 00 00 00                        LOCALE    @125 len 8
48  07 00 85 00 00 00 04 00 00 00                        FUNCS     @133 len 4
52  0F 00 89 00 00 00 1A 00 00 00                        STRINGS   @137 len 26
5C  00 00 00 80                                          INDEX: select, MESSAGES offset 0
60  01                                                   names 1: NAMES entry @0
61  02                                                   decls 2: 1 declaration, c 0
62  31 00 00 00                                            .input, op variable, f: $ slot 0, fn 0, 0 options
66  01 00                                                selectors: 1, $ slot 0
68  02                                                   variants: 2
69  01 01 03  01 00 16                                     keys 1: `1` (StrRef 0); plen 3: 1 part, TEXT "one" (22)
6F  01 00 05  02 11 00 00 0F                               keys 1: `*`; plen 5: 2 parts, EXPRESSION $ slot 0, TEXT " items" (15)
77  01 00 0D 00 00 00                                     NAMES: 1 slot, 0 locals, "n" (str32 13)
7D  01 01 05 21 01 05 82 01                               LOCALE: 1 entry, key 1 (plural.cardinal), len 5
85  05 00 00 00                                          FUNCS: "integer" (str32 5)
89  31 00 65 6E 00 69 6E 74 65 67 65 72 00 6E 00 20 69 74 65 6D 73 00 6F 6E 65 00
```

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
  **not** in the hash. The canonical serialization, byte for byte (`uint` is a
  minimal unsigned LEB128, `str` is `uint(byte length) · UTF-8 bytes`):

  ```text
  manifest := uint(n) · (str(id) · uint(s) · str(slot){s} · uint(m) · str(markup){m}){n}
              · uint(f) · str(function){f}
  ```

  Ids in MsgId order (bytewise ascending); per message its slot names (NFC,
  ascending bytewise — the slot order) and its markup names (NFC, ascending,
  unique); then the function identifiers (NFC, ascending, unique), all as
  `mf2_syntax::analyze` reports them. FNV-1a 64: offset basis
  `0xcbf29ce484222325`, prime `0x100000001b3`. The reference workload's hash is
  `43e0dc12eeb05ef1` (P0.7, re-derived by `mf2-catalog`'s manifest test).
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
option values actually used — the rule is §4.4).

| Key | Entry | Present when (§4.4) | Content |
|---|---|---|---|
| 1 | `plural.cardinal` | any `:number`/`:integer`/`:offset`/`:percent` selection with `select=plural` (the default) | CLDR rule for this locale as a tiny condition table: per category, OR of ANDs of `(operand n/i/v/w/f/t/c/e, modulus, negated, ranges)` (§4.1) |
| 2 | `plural.ordinal` | any `select=ordinal` | same encoding (§4.1) |
| 3 | `number.symbols` | `fn-number` on and a number is formatted | decimal, group, minus, plus and percent signs; the decimal pattern's grouping sizes; minimum grouping digits; the digits of the catalog's numbering system, absent for ASCII (§4.2) |
| 4 | `number.patterns` | `:percent`, `:currency` | the percent and currency patterns of the styles the corpus uses: grouping and affixes with the sign, percent and currency positions (§4.3) |
| 16 | `currency.data` | `:currency` | for the **configured currency set** (default: the literal codes of the corpus; `currencies = "all"` opts into every code): fraction digits and rounding increment, symbol, narrow symbol, display names by plural category, the currency's own pattern and separators, the edges of its symbols for currency spacing; the locale's name patterns (§4.6) |
| 32 | `unit.data` | `:unit` | for the configured unit set, same policy, and the widths used: the patterns by plural category, the per-unit pattern, optionally the display name; the locale's `per` compound pattern (§4.7) |
| 48 | `icu.blob` | feature `datetime-icu` and the corpus formats a date: a date function, or a placeholder that can receive a date/time argument (§4.4) | an ICU4X data blob for this locale holding exactly what the ICU4X backend requests for the shapes (parts, lengths, precisions, zone styles, hour cycles, calendars) the corpus can format. The ICU4X *code* is in the wasm (that is the feature's cost); the *data* is here, lazy, like everything else (§4.9) |

Keys are `mf2_catalog::format::locale_key`; all are below 128, one varint
byte. A new version of an entry takes a new key in its kind's range (3–15 for
`number.*`, 16–31 `currency.*`, 32–47 `unit.*`, 48–63 the date entries).

**Departures from the Phase 2 plan (Phase 4, A2).** (1) There is no
`number.systems` entry: the pinned spec (`spec/functions/number.md`) has **no
`numberingSystem` option**, so a catalog's numbering system comes from its
locale alone — CLDR's default, or a `-u-nu-` extension in the catalog's tag —
and a catalog has exactly one; its digits are folded into `number.symbols`
(absent = ASCII), which saves a key, a lookup and the entry framing. (2)
`number.symbols` carries no per-mille sign, exponent symbol, infinity or NaN:
no MF2 function at the pin produces them (`:percent` is ×100 and there is no
per-mille style; there is no `notation` option; numeric operands are finite).
(3) `:unit` needs no `number.patterns` record: its unit patterns (`{0} km`)
wrap a plain decimal and live in `unit.*`.

**Departures (Phase 4, A4).** (4) One entry per family, `currency.data` and
`unit.data`, rather than several `currency.*` / `unit.*` entries: a handler
needs a currency's symbol, digits and names together, and one keyed record per
code (or unit) with an index is one lookup; the key ranges stay reserved for
new versions. (5) `currency.data` also carries what CLDR attaches to a
currency and ICU applies: its own standard pattern (`en-DE` and the other
English-in-Europe locales put `€` first: `¤#,##0.00`; `tr` TRY) and decimal
and group separators (`pt-CV` CVE: `1$50`). (6) Currency spacing needs, per
symbol, whether its first and last characters are in CLDR's
`[[:^S:]&[:^Z:]]` — General_Category data the client does not have — so the
build computes two bits per symbol (§4.6). (7) `X-per-Y` units CLDR lacks are
composed from `X`, `Y` and the `per` pattern (UTS #35 Part 2, compound units;
ECMA-402 composes the same); `times`, powers and prefixes on arbitrary units
are not, since they need names inflected for plural, gender and case — CLDR's
common compounds (`square-kilometer`, `kilowatt-hour`, `meter-per-second`)
are units of their own.

The CLDR version used is recorded in the header flags/metadata and printed by
`mf2 stats`. CLDR JSON is a build-time input only, pinned like the spec.

With `datetime-intl` the client needs no date data at all, so no `icu.blob` is
emitted (the server formats from ICU4X's compiled data, 03 §5.2).

**Container first, entries later.** Phase 2 freezes the LOCALE *container* (a
keyed table of versioned, opaque entries; unknown keys are skipped) and the two
`plural.*` entries, whose encoding Phase 0 probe P0.4 prototyped and verified
(§4.1). The number, currency, unit and date entries are defined in Phase 4 as
additive entry kinds, which by the skip rule need no format-version bump. Unlike
the plural entries, `Catalog::new` does **not** walk the number entries: their
client views (`mf2_catalog::number`) are panic-free and read only what they are
asked for, answering `None` on malformed bytes, so a bad entry costs only the
numbers that read it (F4) and loading stays linear in nothing more than it was;
the same holds for `currency.data` and `unit.data` (`mf2_catalog::currency`,
`mf2_catalog::unit`). The writer refuses a malformed entry under a known key
(`WriteError::LocaleEntry`).

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

### 4.2 `number.symbols` entry, v1 (key 3; Phase 4, A2)

The symbols, grouping and digits of the catalog's **one** numbering system.

```text
entry        := u8 grouping · u8 min_grouping · str8 decimal · str8 group
                · str8 minus · str8 plus · str8 percent · digits
grouping     := u8   bits 0–3 primary size, bits 4–7 secondary size (0 = no grouping)
                     — of the locale's decimal pattern (`#,##0.###` → 3/3, `#,##,##0.###` → 3/2)
min_grouping := u8   CLDR minimumGroupingDigits, 1–15
str8         := u8 len · UTF-8{len}
digits       := ε                     ASCII 0–9 (`latn`)
              | u8{10 × w}            the ten digits in order, each w bytes of UTF-8, w ∈ 1..4;
                                      the rest of the entry
```

* **Which system.** The catalog tag's `-u-nu-<nu>` when `<nu>` is a numeric
  system (`native` → the locale's native one); otherwise the locale's CLDR
  default. `traditio` and `finance` name algorithmic systems in every CLDR
  48.2.1 locale, so they, and any algorithmic or unknown system, give the
  default. Every CLDR 48.2.1 locale has symbols for exactly `latn`, its
  default and its native system, and all 78 numeric systems have digits of
  one UTF-8 width (the extractor checks both). A locale without data for the
  requested system (`en-u-nu-arab`) takes `latn`'s symbols and grouping with
  that system's digits — CLDR's root has no other system (ICU's per-symbol
  fallback to `latn`).
* **Canonical writer** (`mf2_catalog::writer::number::symbols`): the ASCII
  digits are written as ε; sizes ≤ 15; `min_grouping` 1–15; each string
  ≤ 255 bytes; the ten digits of one width.
* **Reader** (`mf2_catalog::number::Symbols::parse`, `Symbols::of(catalog)`):
  `None` when truncated, a string is not UTF-8, or the digits are not 10 × w
  bytes (w ≤ 4) on character boundaries. Accessors return `&str` borrowed
  from the catalog; `Digits::digit(d)` is one slice.
* **Use** (`mf2-fn-number`, A3). The digits of the formatted number through
  `digits().digit(d)`; the decimal separator before the fraction; a group
  separator after integer digit *m* (0 = units) when
  `grouping().separator_after(m)` and grouping applies: `useGrouping=auto`
  groups when the integer part has at least `primary + min_grouping` digits,
  `min2` at least `primary + 2`, `always` at least `primary + 1`, `never`
  never (ECMA-402's reading, which P0.5 confirmed against node). Without a
  pattern the sign symbol precedes the number.
* **Sizes** (CLDR 48.2.1, every locale and its native system): 12–54 B,
  median 12 (`en` 12, `ar-u-nu-arab` 44, `ff-Adlm-u-nu-native` 54).

### 4.3 `number.patterns` entry, v1 (key 4; Phase 4, A2)

The percent and currency patterns of the styles the corpus uses.

```text
entry   := record*                          ; styles strictly ascending
record  := u8 style · u8 len · pattern{len}
style   := 1 percent               percentFormats/standard
         | 2 currency              currencyFormats/standard
         | 3 currency-alpha        …/standard-alphaNextToNumber   (absent: use 2)
         | 4 currency-no-symbol    …/standard-noCurrency          (currencyDisplay=never)
         | 5 accounting            …/accounting                   (currencySign=accounting)
         | 6 accounting-alpha      …/accounting-alphaNextToNumber (absent: use 5)
         | 7 accounting-no-symbol  …/accounting-noCurrency
pattern := u8 grouping · affix pos_prefix · affix pos_suffix
           · [affix neg_prefix · affix neg_suffix]              ; the negative subpattern, if CLDR has one
affix   := u8 len · u8{len}     ; UTF-8 in which 0x01 is the sign position (`-`), 0x02 the percent
                                ; sign (`%`), 0x03 the currency symbol (`¤`); no other byte < 0x20
```

* **From CLDR** (`mf2_locale_data::number::pattern`): `grouping` is the
  positive subpattern's integer grouping, which can differ from the decimal
  pattern's (165 of CLDR 48.2.1's 910 system records: `as` accounting groups
  3/3 where its decimals group 3/2, `bn` percent likewise); the affixes keep
  their text (bidi marks, U+00A0, U+202F) with `-`, `%` and `¤` as
  placeholders. The number part's digit counts are not kept: MF2 fixes
  `:percent`'s fraction digits and `:currency` takes the currency's. None of
  these CLDR 48.2.1 patterns use quoting, padding, `+`, `‰`, `¤¤`, or a
  scientific or significant-digit number part; the extractor refuses them,
  so a CLDR update that does is noticed.
* **Canonical writer** (`mf2_catalog::writer::number::patterns`): records in
  style order; an `…-alpha` record that equals its base, or that CLDR lacks
  (the `arab` systems), is omitted; adjacent text merged; each record and
  affix ≤ 255 bytes.
* **Reader** (`mf2_catalog::number::Patterns::new` / `of(catalog)`):
  `get(style)` scans the records and parses the one asked for; `resolve(style)`
  adds the `…-alpha` → base fallback; `None` when absent or malformed.
  `is_valid()` checks the whole entry (the writer and tests; a client need
  not).
* **Sign** (UTS #35 §3.2 as ICU applies it; `Pattern::signed`): no sign → the
  positive affixes; minus → the negative affixes if the pattern has them (the
  sign position renders the minus sign; accounting's parentheses have none),
  else the positive affixes with the sign first; plus → the negative affixes
  if they have a sign position (rendering the plus sign), else the positive
  affixes with the sign first.
* **Currency spacing.** CLDR 48.2.1's `currencySpacing` is the same in all
  910 system records — `currencyMatch` `[[:^S:]&[:^Z:]]`, `surroundingMatch`
  `[:digit:]`, `insertBetween` U+00A0, before and after — so no entry carries
  it: `:currency` (A4) applies it as a constant rule, and picks the
  `…-alpha` style when the symbol's letter would touch the digits. The
  extractor fails if a CLDR update changes it.
* **Sizes**: the percent record alone 6–14 B (median 6); all seven styles
  32–106 B (median 43).

### 4.4 Slicing rule — which entries a catalog carries

Decided by `mf2-build` from the manifest's functions and the corpus's literal
option values (`mf2_locale_data::{LocaleNeeds, NumberNeeds, locale_entries}`;
`NumberNeeds::from_functions` is the conservative rule from the function set
alone). An option given by a variable counts as every value it could take.

| Entry | Carried when |
|---|---|
| `plural.cardinal` | a selector on `:number`, `:integer`, `:offset` or `:percent` with `select=plural` (the default) |
| `plural.ordinal` | a selector with `select=ordinal` |
| `number.symbols` | `fn-number` is on and a number is formatted: a placeholder or declaration with `:number`, `:integer`, `:offset`, `:percent`, `:currency` or `:unit`, or a placeholder whose variable has no function (it can receive a number — `syntax.json` #90) |
| `number.patterns` style 1 | `:percent` |
| styles 2, 3 | `:currency` |
| style 4 | `:currency` with `currencyDisplay=never` (or a variable value) |
| styles 5, 6 | `:currency` with `currencySign=accounting` (or a variable value) |
| style 7 | both of the above |
| `currency.data` | `:currency`: the currencies of the configured set; narrow symbols when `currencyDisplay=narrowSymbol` can occur, display names and name patterns when `currencyDisplay=name` can |
| `unit.data` | `:unit`: the units of the configured set (an `X-per-Y` CLDR lacks brings `X` and `Y`), in the widths `unitDisplay` can take (`short` when an expression gives none); display names only on request |
| `plural.cardinal`, also | `:unit`, or `:currency` with names: the pattern (`{0} kilometers`, `{0} {1}` with `euros`) is chosen by the formatted number's plural category, so these carry the cardinal rules even without a selector (`locale_entries` adds them) |
| `icu.blob` | `datetime-icu` is on and a date is formatted (§4.9): per `:datetime`, `:date` or `:time` expression, its *shape* — the date part (fields × length), the time part (precision), the zone style — from its literal options (a non-override option set by a variable, or an invalid value, takes its default, as at run time: *Bad Option*); `:datetime`'s default shape for a placeholder or declaration whose variable has no function and is not declared with one (it can receive a date/time argument, which the unannotated handler formats — as `number.symbols` for #90); in each shape the locale's hour cycle and each `hour12` value the message may set (a variable: both), in the locale's default calendar, `gregory` and each literal `calendar=` value. Zone names, metazone periods and zone ids only for a shape with a zone style. A calendar only a variable or an argument names is not in the corpus: formatting it is an *Unsupported Operation* until it is listed (as a currency outside the set) |

**The configured sets** (`mf2.toml`, 05 §3.1):

```toml
[locale_data]
currencies = "used"   # default: the literal `currency=` codes of the corpus
                      # | "all": every code CLDR has (and every code of currencyData)
                      # | ["USD", "EUR"]: these, plus the literal ones
units = "used"        # the same for `unit=` identifiers
calendars = []        # `datetime-icu`: calendars besides each locale's default, `gregory`
                      # and the literal `calendar=` values (`DateNeeds::calendars`), for
                      # values only a variable or an argument carries
```

With `"used"`, a non-literal `currency=$c` or `unit=$u` in the corpus makes the
set `"all"` — correct output over size — and `mf2 check` notes that a list
would do. A code or unit that only arguments carry at run time (a currency
value from the application) is not in the corpus: list it. A code outside the
set still formats, with its code as symbol and name and CLDR's default
fraction digits (UTS #35's fallback); a unit outside it is unsupported.
`mf2_locale_data::NumberNeeds::add_message` implements the literal rule on the
data model; `Selection::{Listed, All}` is the set.

With `fn-number` off no number entry is written (numbers format neutrally,
`mf2 check` warns `neutral-numbers`). The numbering system is the catalog tag's
(§4.2); the tag resolves to a CLDR locale as CLDR does: a region whose likely
script differs from its language's gains that script when such a locale exists
(`zh-TW` → `zh-Hant-TW`, `pa-PK` → `pa-Arab-PK`); then the tag, its explicit
parent (`parentLocales.json`: `en-AU` → `en-001` → `en`) or truncation — a
`lang-Script` whose script is not the language's likely one has root as parent
(`_localeRules: nonlikelyScript`) — and root (`und`).

### 4.5 Test vectors

Byte-exact, as `mf2_locale_data::number_locale_entries` writes them from the
shipped table (CLDR 48.2.1); tests of the encoders (`mf2-catalog`
`tests/number.rs`, from hand-written input), of the extraction
(`mf2-locale-data` `tests/numbers.rs`) and of the views.

| Tag | `number.symbols` | `number.patterns`, percent only |
|---|---|---|
| `en` | `33 01 01 2E 01 2C 01 2D 01 2B 01 25` | `01 04 33 00 01 02` |
| `fr` | `33 01 01 2C 03 E2 80 AF 01 2D 01 2B 01 25` (group U+202F) | `01 06 33 00 03 C2 A0 02` (`#,##0 %`, U+00A0) |
| `ar` | `33 01 01 2E 01 2C 04 E2 80 8E 2D 04 E2 80 8E 2B 07 E2 80 8E 25 E2 80 8E` (`latn`: CLDR 48's default for `ar`) | `01 04 33 00 01 02` |
| `ar-u-nu-arab` | `33 01 02 D9 AB 02 D9 AC 03 D8 9C 2D 03 D8 9C 2B 04 D9 AA D8 9C` + digits `D9 A0 D9 A1 … D9 A9` (٠…٩) | `01 04 33 00 01 02` |
| `hi` | `23 01 01 2E 01 2C 01 2D 01 2B 01 25` (grouping 3/2) | `01 04 23 00 01 02` |
| `pl` | `33 02 01 2C 02 C2 A0 01 2D 01 2B 01 25` (minimum grouping 2) | `01 04 33 00 01 02` |

Every style, `en` (styles 1–7; the negative subpatterns are accounting's
parentheses):

```text
01 04 33 00 01 02                                   percent       #,##0%
02 04 33 01 03 00                                   currency      ¤#,##0.00
03 06 33 03 03 C2 A0 00                             currency-alpha ¤ #,##0.00
04 03 33 00 00                                      no-symbol     #,##0.00
05 09 33 01 03 00 02 28 03 01 29                    accounting    ¤#,##0.00;(¤#,##0.00)
06 0D 33 03 03 C2 A0 00 04 28 03 C2 A0 01 29        acct-alpha    ¤ #,##0.00;(¤ #,##0.00)
07 07 33 00 00 01 28 01 29                          acct-no-symbol #,##0.00;(#,##0.00)
```

`ar` currency (`‏#,##0.00 ¤;‏-#,##0.00 ¤`, a sign position in the negative
subpattern; its `-alpha` twin is omitted): `02 12 33 03 E2 80 8F 03 C2 A0 03 04
E2 80 8F 01 03 C2 A0 03`.

**B8** (plural + number symbols ≤ 0.5 KB gz per locale, 06 §3), the LOCALE
section of a catalog with both plural entries and `number.symbols` (+ the
percent record), standalone `gzip -9 -n`: 39–72 B gz (47–80 B with `:percent`)
on the panel `en es de fr ar he ja hi ru pl cy`, raw 19–54 B (27–62 B);
`cargo test -p mf2-locale-data --test numbers b8 -- --nocapture` prints the
table and asserts ≤ 512 B (flate2 at level 9, within 5 B of GNU gzip).

### 4.6 `currency.data` entry, v1 (key 16; Phase 4, A4)

The configured currencies, one record each, found by binary search over a
fixed-width index.

```text
entry    := u8 flags · u8 default_digits · [forms name_patterns (flags bit 1)]
            · u16 n · (u8{3} code · u32 offset){n} · records
flags    := bit 0 narrow symbols carried · bit 1 display names (and name patterns) carried
index    : codes upper-case ASCII, strictly ascending; offsets into `records`, the first 0,
           each record ending where the next begins
record   := u8 head · [varint rounding (head bit 4)] · u8 edges
            · [str8 symbol (bit 5)] · [str8 narrow (bit 6)] · [u8 len · pattern (edges bit 4)]
            · [str8 decimal (edges bit 5)] · [str8 group (edges bit 6)]
            · [str8 name (bit 7)] · [forms names (flags bit 1)]
head     := bits 0–3 fraction digits · bit 4 a rounding increment follows · bit 5 symbol stored
            (else the ISO code) · bit 6 narrow stored (else the symbol) · bit 7 name stored
            (else the ISO code)
edges    := bits 0, 1: the symbol's first, last scalar is in [[:^S:]&[:^Z:]]; bits 2, 3: the
            narrow symbol's; bits 4–6: the currency's own pattern, decimal, group follow
pattern  := a `number.patterns` record body (§4.3): the currency's own standard pattern
forms    := u8 k · (u8 category · str8){k}   ; categories 0 zero … 5 other, strictly ascending
template := str8, UTF-8 in which 0x01 is `{0}` and 0x02 is `{1}`; no other byte < 0x20
```

`name_patterns` are forms of templates: the locale's `currencyFormats`
`unitPattern-count-*` of the catalog's numbering system (else `latn`'s — `ckb`'s
`arab` has none), `{0}` the formatted number, `{1}` the display name. `names`
are forms of plain strings: `displayName-count-*`.

* **Fallbacks** (CLDR's, applied by the view): a name form falls back to
  `other`'s, then to the display name; a missing symbol and name are the
  code; a missing narrow symbol is the symbol. `Currency::name_for(category)`,
  `Currencies::name_pattern(category)` apply them.
* **Canonical writer** (`mf2_catalog::writer::currency::currencies`):
  records by code; a symbol equal to the code, a narrow symbol equal to the
  symbol, a name equal to the code not stored; a form equal to what its
  fallback gives dropped (`en` JPY: `one` = `other` = `Japanese yen`); the
  rounding increment only when non-zero (CLDR 48.2.1: never, outside cash);
  narrow symbols and names only when the flags carry them.
* **Reader** (`mf2_catalog::currency::Currencies::parse` / `of(catalog)`):
  O(1) header and index bounds; `get(*b"USD")` binary-searches and parses one
  record → `Currency`: `symbol()`, `narrow_symbol()`, `name()`,
  `name_for(cat)`, `fraction_digits()`, `rounding_increment()`,
  `pattern()` (a `number::Pattern`), `decimal()`, `group()`,
  `symbol_edges()` / `narrow_edges()`; `None` when absent or malformed.
  `is_valid()` checks everything (the writer, tests).
* **Use** (`:currency`, A4 handlers): digits from `fraction_digits()` for
  `fractionDigits=auto`; the pattern: the currency's own `pattern()` for the
  standard sign, else `number.patterns` `Currency`, or its `…Alpha` style
  when the symbol touches the number with a letter-like edge (`last` for a
  prefix symbol, `first` for a suffix one), else CLDR's constant currency
  spacing (§4.3: insert U+00A0) — the edges decide both; `decimal()` /
  `group()` replace the locale's separators; `currencyDisplay=code` writes the
  code, `name` the name pattern with `name_for(category)` (the category of the
  formatted number, `plural.cardinal`).
* **Sizes** (CLDR 48.2.1, panel): USD EUR JPY GBP with every display 113–561 B
  raw, 136–252 B gz; every code 9.6–37.9 KB raw, 3.9–9.1 KB gz (§4.8).

### 4.7 `unit.data` entry, v1 (key 32; Phase 4, A4)

The configured units in the carried widths, by identifier.

```text
entry    := u8 flags · template per{w} · u16 n · u32 offset{n} · records
flags    := bits 0–2 the widths carried (long, short, narrow; w of them) · bit 3 display names
per      : the `per` compound pattern of each carried width, in that order ({0} numerator,
           {1} denominator)
index    : offsets into `records`, the first 0, each record ending where the next begins;
           records strictly ascending by identifier, bytewise
record   := str8 id · block{w}
block    := u8 head · [str8 name (bit 1)] · [template per_unit (bit 2)] · forms patterns
          | u8 0x01                          ; the previous width's block, again
head     := bit 0 same as the previous width · bit 1 display name · bit 2 per-unit pattern
patterns : forms of templates ({0} the number): unitPattern-count-*; `other` present when
           any is; empty when CLDR has none for the unit in this width
```

* **Identifiers** are CLDR's unit keys without their category
  (`length-kilometer` → `kilometer`, `consumption-liter-per-100-kilometer` →
  `liter-per-100-kilometer`): 232 at CLDR 48.2.1, unique (the extractor
  checks it; `data/units.txt` lists the map).
* **Fallback**: a missing plural form is `other`'s (`Unit::pattern`). A
  pattern may have no `{0}`: CLDR writes some forms with the number in the
  word (`ar` dual `دورتان`).
* **Canonical writer** (`mf2_catalog::writer::unit::units`): widths in the
  order long, short, narrow; units by identifier; a form equal to `other`'s
  dropped; a block equal to the previous width's as `0x01`; display names only
  when flagged.
* **Reader** (`mf2_catalog::unit::Units::parse` / `of(catalog)`):
  `get("kilometer")` binary-searches the records by identifier → `Unit`:
  `pattern(width, category)`, `per_unit_pattern(width)`,
  `display_name(width)`, `has_patterns(width)`; `Units::per_pattern(width)`.
* **Use** (`:unit`): `unitDisplay` picks the width, the formatted number's
  plural category the pattern, `{0}` the localized number. `X-per-Y` that
  CLDR has as a unit is a unit; one it lacks (`kilometer-per-second`) is
  composed: when `Y` has a per-unit pattern, the formatted `X` goes into its
  `{0}`; else `per_pattern` with `{0}` the formatted `X` and `{1}` `Y`'s `one`
  pattern with its `{0}` and the space next to it removed (UTS #35 Part 2).
  `mf2_locale_data::composition(id)` gives `(X, Y)`.
* **Sizes** (panel, all three widths): five units 294–626 B raw, 200–310 B
  gz; every unit 11.5–24.7 KB raw, 5.0–7.4 KB gz (§4.8).

### 4.8 Test vectors and sizes (A4)

Byte-exact, as `number_locale_entries` writes them from the shipped tables;
tests of the encoders (`mf2-catalog` `tests/measure.rs`, the same bytes from
hand-written input), the extraction (`mf2-locale-data` `tests/measure.rs`) and
the views.

`en`, USD and JPY with names (flags `02`), no narrow symbols:

```text
02 02                                               flags: names; default digits 2
01 05 03 01 20 02                                   name patterns: other `{0} {1}` (one = other: dropped)
02 00 4A 50 59 00 00 00 00 55 53 44 21 00 00 00     2 codes: JPY @0, USD @33
A0 00 02 C2 A5 0C 4A 61 …  01 05 0C 4A 61 … 6E      JPY: digits 0, ¥, "Japanese Yen", other "Japanese yen"
A2 00 01 24 09 55 53 20 44 6F 6C 6C 61 72           USD: digits 2, $, "US Dollar",
   02 01 09 55 53 20 64 … 05 0A 55 53 20 64 … 73      one "US dollar", other "US dollars"
```

EUR, symbols only: `00 02 01 00 45 55 52 00 00 00 00 22 00 03 E2 82 AC` in
`en`, `fr`, `ar`, `hi`, `pl`; in `en-DE` its own pattern `¤#,##0.00`
follows (edges `10`): `… 22 10 03 E2 82 AC 04 33 01 03 00`.

`kilometer` and `hour`, short width only (flags `02`), `en`:

```text
02 03 01 2F 02                                      short; per `{0}/{1}`
02 00 00 00 00 00 11 00 00 00                       2 units: hour @0, kilometer @17
04 68 6F 75 72 04 03 01 2F 68 01 05 04 01 20 68 72  hour: per-unit `{0}/h`; other `{0} hr`
09 6B 69 6C … 72 04 04 01 2F 6B 6D 01 05 04 01 20 6B 6D
                                                    kilometer: `{0}/km`; other `{0} km`
```

(`fr`: `{0} h` and `{0} km` with U+202F; `ar`, `pl`: in `tests/measure.rs`.)

Sizes per panel locale, raw / `gzip -9` bytes of the entry (flate2 level 9);
P0.5's figures (its own layout) beside them. "Used": USD EUR JPY GBP with every
display; `kilometer kilogram celsius hour megabyte` in all three widths.
"All": every code with every display; every unit in all widths, no display
names.

| locale | currencies used | P0.5 | all | P0.5 all | units used | P0.5 | all | P0.5 all |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| en | 185 / 163 | 190 / 163 | 22,328 / 7,742 | 23,620 / 8,084 | 359 / 211 | 362 / 205 | 14,395 / 5,459 | 18,913 / 5,895 |
| es | 251 / 185 | 242 / 186 | 18,936 / 6,871 | 20,431 / 7,235 | 370 / 224 | 333 / 215 | 14,361 / 5,703 | 16,968 / 5,651 |
| de | 178 / 154 | 186 / 151 | 19,852 / 6,744 | 23,414 / 7,343 | 294 / 200 | 327 / 195 | 11,487 / 4,966 | 16,227 / 5,073 |
| fr | 251 / 186 | 238 / 181 | 21,630 / 7,482 | 22,322 / 7,672 | 402 / 245 | 404 / 238 | 14,752 / 5,730 | 19,122 / 5,912 |
| ar | 145 / 154 | 223 / 169 | 11,640 / 4,675 | 21,350 / 5,989 | 416 / 279 | 1,273 / 363 | 15,849 / 5,615 | 70,012 / 9,193 |
| he | 133 / 143 | 199 / 155 | 9,552 / 3,995 | 16,547 / 4,974 | 451 / 279 | 763 / 305 | 12,936 / 5,284 | 29,884 / 6,661 |
| ja | 113 / 136 | 145 / 145 | 12,412 / 5,072 | 22,373 / 6,514 | 310 / 235 | 234 / 201 | 11,703 / 4,988 | 11,705 / 4,363 |
| hi | 201 / 179 | 327 / 201 | 11,497 / 3,933 | 20,317 / 4,683 | 626 / 310 | 707 / 286 | 16,461 / 5,480 | 29,370 / 5,956 |
| ru | 561 / 252 | 585 / 260 | 37,875 / 9,057 | 44,281 / 9,639 | 550 / 287 | 921 / 321 | 24,670 / 7,373 | 46,414 / 9,002 |
| pl | 351 / 216 | 352 / 224 | 24,739 / 8,163 | 27,992 / 8,836 | 492 / 263 | 583 / 270 | 19,751 / 7,221 | 27,690 / 7,162 |
| cy | 191 / 161 | 348 / 204 | 14,245 / 5,847 | 39,517 / 9,164 | 441 / 235 | 697 / 255 | 16,490 / 6,756 | 32,798 / 8,427 |

(`cargo test -p mf2-locale-data --test measure sizes -- --nocapture`.) The
"used" sets stay within 0.3 KB gz a locale; "all" is 4–9 KB gz each, so the
default stays "used" (P0.5's recommendation). "All" is smaller than P0.5's
because forms equal to `other` and widths equal to the previous one are not
stored and placeholders take one byte (`ar` units: 15.8 against 70.0 KB raw);
it is larger where the per-unit patterns that composition needs dominate
(`ja` units).

### 4.9 `icu.blob` entry, v1 (key 48; Phase 4, A6)

The date data of `datetime-icu`: what `mf2-fn-datetime`'s ICU4X backend
(`icu::Icu`, 03 §5.2) requests to format the shapes the corpus can produce
(§4.4) in this catalog's locale — and nothing else.

```text
entry := blob   ; an ICU4X `BlobDataProvider` blob, format v3: the postcard encoding of
                ; `BlobSchema::V003` (first byte 03) — per data marker, a ZeroTrie from
                ; data identifier (locale · marker attributes) to a payload index, and the
                ; payloads' postcard bytes (icu_provider_blob 2.3, written by
                ; icu_provider_export's `BlobExporter`)
```

* **Why ICU4X's own format.** The backend hands the entry to
  `BlobDataProvider::try_new_from_blob` (over a copy: the provider owns its
  blob) and builds its formatter with `try_new_with_buffer_provider`. An own
  encoding would need a data provider per marker re-implementing ICU4X's
  (de)serialization for no size gain: the blob holds only the identifiers
  requested — no fallback chain, no `und` copies, one numbering system's
  digits. Its compatibility is ICU4X's: a v3 blob loads in ICU4X 2.x. The
  entry is rebuilt with the catalog, so a catalog always carries the blob its
  build's ICU4X wrote; the `icu_*_data` crates are the CLDR input (48, as
  ICU4X 2.3 bakes it), and a change of them shows in the vectors below and in
  the date goldens.
* **Built by recording** (`mf2_locale_data::icu_blob`, feature `icu-blob`,
  build side only): `mf2_fn_datetime::icu::prime` — the backend's own
  construction code — builds the formatter of every option set the corpus can
  produce (`DateNeeds`: each shape × the locale's hour cycle and each `hour12`
  value that may occur × the locale's default calendar, `gregory` and the
  extra calendars) through a provider that answers each request from ICU4X's
  baked data (the `icu_*_data` crates; nothing downloaded) and remembers it;
  the blob is exactly the requests seen, under the identifiers requested.
  Building requests everything formatting does but the IANA zone parser,
  which a shape with a zone style builds too. For the widest backend variant
  (`Icu<AnyCalendar, WithZones>`) the narrower ones are primed too, so one
  blob serves each of them (`IcuBlobSpec`). A calendar ICU4X does not build
  (`calendar=abcd`) records nothing; formatting it is an *Unsupported
  Operation*. Deterministic (a `BTreeMap` of requests; the exporter sorts).
* **Contents**, by ICU4X marker: the date patterns of each calendar carried
  (`DatetimePatternsDate<Calendar>V1`, attributes = field set and options,
  e.g. `ym0d`), the time patterns (`DatetimePatternsTimeV1`, e.g. `j`), the
  date–time glue (`DatetimePatternsGlueV1`, e.g. `mdt`, `mtz`), the names
  the patterns use (`DatetimeNamesMonth<Calendar>V1`,
  `DatetimeNamesYear<Calendar>V1`, `DatetimeNamesWeekdayV1`,
  `DatetimeNamesDayperiodV1`, by width), the decimal symbols and the digits
  of the locale's numbering system (`DecimalSymbolsV1`, `DecimalDigitsV1`),
  the Japanese eras for `japanese`; and only with a zone style: zone names
  (`TimezoneNamesEssentialsV1`, `TimezoneNamesSpecificLongV1` /
  `…ShortV1`), the metazone periods (`TimezonePeriodsV1`, 6.8 KB raw) and
  the IANA → BCP-47 zone ids (`TimezoneIdentifiersIanaCoreV1`, 9.5 KB raw)
  — locale-independent, and most of a zoned blob.
* **Reader / use** (`mf2-fn-datetime`, `datetime-icu`): per format, the
  entry becomes a provider and the formatter is built from it (no cache: the
  handlers are `no_std` statics without interior mutability). No entry, a
  malformed one, or a request it cannot answer (a calendar or shape the
  corpus did not name) is an *Unsupported Operation* and a fallback value —
  never a panic in our code, never compiled data in the client. The server
  formats from the same entry, so its text is the client's, byte for byte.
  Unlike the other entries, **a damaged `icu.blob` is not survivable by
  construction**: ICU4X parses it and does not promise to survive damage —
  the `format` fuzz target found `DecimalFormatter::try_new` panicking on a
  damaged numbering-system name (`DataMarkerAttributes::from_str_or_panic`,
  in release builds too), and `icu_provider_blob` checks the blob's
  invariants with `debug_assert!`. A catalog is the build's output,
  content-hashed and served immutable (§3), so a damaged blob means a
  damaged deployment; the fuzz target formats damaged catalogs' dates with
  the neutral backend and hands ICU4X only the blobs the build wrote.
* **Test vectors** (`mf2-locale-data` `tests/icu_blob.rs`, `vectors`; `en`,
  Gregorian-only variant; FNV-1a 64 of the whole entry):

  | Message | Variant | Size | FNV-1a 64 | First bytes |
  |---|---|---:|---|---|
  | `{$x}` (`:datetime`'s defaults) | no zones | 499 B (371 gz) | `c726446f5289ecf2` | `03 20 06 1C 28 2D 09 FF` |
  | `{$d :date length=long}` | no zones | 394 B (302 gz) | `ad77c5d1446f33b3` | `03 14 06 1C 28 2D 11 34` |
  | `{$d :time timeZoneStyle=short}` | zones | 16,755 B (12,171 gz) | `a4f9f2e0acca5b75` | `03 24 09 FF 7E EC 1D 18` |

* **Sizes** (B4: ≤ 3 KB gz per locale without zone names, ≤ 25 KB gz
  with; `cargo test -p mf2-locale-data --features icu-blob --test icu_blob
  sizes -- --nocapture`), raw / gzip -9 B, per locale: `:datetime`'s defaults
  only (Gregorian); every shape — Gregorian, any calendar (+ `th`'s Buddhist
  default), Gregorian with zone styles, every variant:

  | locale | defaults | every shape | any calendar | + zones | every variant |
  |---|---:|---:|---:|---:|---:|
  | en | 499 / 371 | 1,391 / 758 | 1,407 / 771 | 29,234 / 18,264 | 29,249 / 18,276 |
  | es | 472 / 335 | 1,473 / 783 | 1,489 / 801 | 35,459 / 20,559 | 35,474 / 20,571 |
  | de | 353 / 277 | 1,312 / 766 | 1,328 / 779 | 34,348 / 20,344 | 34,363 / 20,354 |
  | fr | 444 / 349 | 1,313 / 749 | 1,329 / 759 | 36,707 / 20,907 | 36,722 / 20,919 |
  | ar | 413 / 321 | 1,333 / 724 | 1,348 / 735 | 34,293 / 18,982 | 34,308 / 18,995 |
  | he | 505 / 374 | 1,589 / 818 | 1,605 / 835 | 33,245 / 18,840 | 33,260 / 18,851 |
  | ja | 318 / 284 | 1,016 / 613 | 1,031 / 627 | 31,680 / 18,648 | 31,695 / 18,661 |
  | hi | 647 / 451 | 1,793 / 877 | 1,809 / 890 | 41,363 / 19,985 | 41,378 / 19,996 |
  | ru | 516 / 370 | 1,587 / 844 | 1,603 / 861 | 35,302 / 19,569 | 35,317 / 19,583 |
  | pl | 418 / 326 | 1,351 / 788 | 1,367 / 802 | 34,352 / 20,398 | 34,367 / 20,409 |
  | cy | 437 / 337 | 1,441 / 799 | 1,458 / 810 | 32,473 / 19,665 | 32,488 / 19,677 |
  | th | 502 / 349 | 1,847 / 909 | 2,451 / 1,054 | 42,092 / 20,086 | 42,696 / 20,241 |

  Every shape without zones: 0.61–0.91 KB gz (P0.6's blobs, built another
  way: 2.2–2.6); with zone styles 18.3–20.9 KB gz (P0.6: 16.8–23.0). Every calendar (a variable `calendar=`) adds little: `en`
  `{$d :date calendar=$c}` is 2,180 B / 1,382 B gz.

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
[09](09-phase-2-work-order.md) and the crate's documentation.

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
  The file is `"MF2M" · u16 version (0x0100) · u64 manifest_hash · manifest`
  (the canonical serialization of §3, so the hash is FNV-1a 64 of the rest of
  the file); `Manifest::read` rejects a wrong magic, an unknown major, a hash
  that does not match, trailing bytes, and lists that are not strictly
  ascending (ids, each message's slots and markup, the functions).

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

Settled by Phase 2 ([phase-2-results](phase-2-results.md)), with version 1:

* **COLD** is a per-message record of overrides keyed by site (§2.5):
  MESSAGES stays the formatting-relevant model, and a stripped catalog still
  says which messages lost data.
* **O(1) names**: NAMES, FUNCS and FALLBACK hold `str32` references; IDS has
  restart points. Narrower NAMES references (2-byte, or P0.7's varints) would
  save 17–204 B brotli per locale; deferred until after v1 by the owner
  ([stretch_goals_after_v1/names_reference_width](stretch_goals_after_v1/names_reference_width.md)).
* **Head**: the COLD bit rides on the declaration count and NAMES entries are
  ordered most-referenced first — MESSAGES is 2 B smaller than P0.7's.
* **B7**, restated on brotli (the served encoding): `en` 18,072 B br
  stripped (20,592 B gz), every locale within the scaled rule; `Catalog::new` 2.3 µs native with 0 allocations and 0
  copies; the reader passes B12.
