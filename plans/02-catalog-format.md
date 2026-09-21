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
(§4.1), which `Catalog::new` walks for structure; §4 lists the entries Phase 4
adds.

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
