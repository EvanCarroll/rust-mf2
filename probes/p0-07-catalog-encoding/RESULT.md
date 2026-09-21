# P0.7 — catalog encoding: RESULT

**Question** (plans/07, plans/06 §5, plans/02 §6): encode the reference
workload per plans/02 §2. How big is a catalog per locale, raw, gzip -9 and
brotli? Does it meet **B7**: ≤ 25 KB gz per locale, and raw ≤ 1.25 × text
bytes + 8 B/message? Which format details win: single or split string pools,
fixed or varint-delta INDEX, stripped or unstripped? A further question came
from the coordinator: the plan's STRINGS pool was "varint length + UTF-8", but
F4 needs the pool to pass one `str::from_utf8`. How should that conflict be
resolved, and at what size cost?

Inputs: `bench/corpora/workload-1600.json` (the source locale `en`) and the
generated locales. `cargo xtask gen-workload locales --out
probes/p0-07-catalog-encoding/corpus` produces `corpus/json/{en,pl,en-XA,ar-XB}.json`;
its `en.json` is byte-identical to the committed corpus. Plural rules come from
P0.4's encoder (`plural-rules`, read-only path dependency) over CLDR 48.2.1.
Toolchain: rustc 1.98.1 stable, GNU gzip (`gzip -9 -n`), and the `brotli` crate
9.0.0 (quality 11, window 22). **KB = 1,024 B**; exact bytes are always given.

## Verdicts (recommended layout, production = stripped)

| locale | MF2 source bytes | raw limit 1.25 × src + 8 × 1600 | **raw** | **gzip -9** | brotli 11 | B7 raw | B7 gz ≤ 25 KB (25,600 B) |
|---|---|---|---|---|---|---|---|
| en | 43,250 | 66,862 | **50,867** | **20,536** | 17,929 | **met** | **met** |
| pl | 58,518 | 85,947 | **67,302** | **26,647** | 24,068 | **met** | **not met** (+1,047 B; met under brotli) |
| en-XA (pseudo) | 95,078 | 131,647 | **100,528** | **23,620** | 21,477 | **met** | **met** |
| ar-XB (pseudo) | 54,491 | 80,913 | **60,790** | **20,518** | 18,491 | **met** | **met** |

For comparison, the **plan as written** (length-prefixed strings, fixed u32
INDEX, first-use pool; `prefix-fixed-single-stripped`) gives 23,083 / 29,609 /
28,433 / 24,833 B gz, i.e. it fails gz for pl and en-XA. The recommended layout
saves **2.5–4.8 KB gz per locale** (11–17 %) over it, with no change to O(1)
lookup and no per-message work at load. Unstripped catalogs (with IDS) are
29–36 KB gz, so **production catalogs must be stripped**.

**pl cannot meet 25 KB gz under any layout measured** (600 variants; the best,
a non-O(1) varint-delta INDEX with relative refs, is 25,905 B). Its pool alone is
18,567 B gz: 55 KB of synthetic Polish, 1.35 × the `en` bytes, with 2-byte
letters. B7 should scale with text volume (see "Recommendations").

**All 600 catalogs (150 layouts × 4 locales) were decoded back to the data
model and compared with the parse: all lossless** (F1 on the workload). The
recommended layout was also checked for mini-corpus constructs the workload
lacks (literals, options, locals, function-only expressions, markup options)
in `probes/p0-03-runtime-floor/check`.

## The F4 conflict: how string lengths are found

A LEB128 length ≥ 128 emits bytes ≥ 0x80 that are not valid UTF-8, so the pool
could not be one `str`. Four ways out were measured, with all other axes as
recommended:

| String layout | en raw / gz | pl gz | en-XA gz | ar-XB gz | Δ gz vs NUL |
|---|---|---|---|---|---|
| **NUL-terminated** (StrRef = offset; string ends at U+0000) | 50,867 / **20,536** | **26,647** | **23,620** | **20,518** | — |
| length prefix encoded **as one UTF-8 scalar** (1–3 B; lengths ≥ 0xD800 shifted past surrogates) — keeps "prefix" semantics | 50,867 / 22,037 | 29,002 | 26,025 | 23,255 | +1.5 … +2.7 KB |
| StrRef = `varint offset, varint len` in the structure; bare-text pool; simple lengths in a u16 side table | 53,360 / 22,738 | 29,123 | 26,810 | 23,342 | +2.2 … +3.2 KB |
| StrRef = string index + u32 end-offset table | 55,487 / 22,807 | 29,450 | 26,363 | 23,266 | +2.3 … +2.8 KB |

**Choice: NUL-terminated strings.** Every length scheme interleaves
high-entropy bytes with the text or the structure. A constant terminator
compresses to almost nothing. It is also safe:

* MF2 syntax cannot contain U+0000. The ABNF excludes `%x00` from `text-char`,
  `quoted-char` and `name-char`, and there is no escape for it. So every message
  that has a syntax form is representable. The one suite test with `\u0000` is
  in `syntax-errors.json` and never reaches a catalog. The writer rejects a NUL
  in any string, which only a data model built in code can contain. That is a
  documented F1 limitation.
* The pool is still one valid `str`, and ending strings with NUL keeps it valid.
* Reading costs one scan per string access. The P0.3 runtime uses a panic-free
  SWAR scan; simple lookup takes 21–23 ns native (P0.8).

## Recommended layout (what `probes/p0-03-runtime-floor/rt` reads)

Constants are in `format/src/lib.rs` (`mf2b-format`). All integers are
little-endian and read from unaligned slices. `varint` is minimal unsigned
LEB128.

```
Header (30 B + 10 B/section)
  magic "MF2B" · format_version u16 · flags u16 · manifest_hash u64 · chunk u8 · dir u8
  message_count u32 · locale_off u32 · locale_len u16 (the tag, in the pool)
  section_count u16 · [kind u16, off u32, len u32]…   (ordered, non-overlapping; unknown kinds skipped)
INDEX     message_count × u32 as 4 BYTE PLANES (all byte 0s, then all byte 1s, …);
          entry = kind (2 high bits: simple|pattern|select|absent) ‖ 30-bit offset
          simple → offset of its string in STRINGS; pattern/select → offset in MESSAGES
MESSAGES  per non-simple message, in MsgId order (offsets strictly increasing):
          varint names_ref (0 = none, else NAMES offset + 1) · varint decl_count · Decl* · Body
          Decl    := u8 head (bit 7 local; bits 3–4 operand; bit 5 function; bit 6 cold)
                     [input: varint slot] operand? FunctionRef? ColdRef?
          Pattern := varint n · Part*
          Part    := u8 head (bits 0–2: text|expr|open|standalone|close; expr bits as Decl;
                     bit 7 markup options) + StrRef / operand / FunctionRef / name + options / ColdRef
          Select  := varint nsel · VarRef* · varint nvar · (Key{nsel} · varint pattern_len · Pattern)*
          Key     := u8 0 = `*` | 1 StrRef | 2 StrRef(NFC) ColdRef(original spelling)
          VarRef  := varint (index << 1 | is_local)        slot for externals, order for locals
          Value   := varint 0 · StrRef (literal) | varint (VarRef << 1 | 1)
          FunctionRef := varint fn_idx · varint n · (StrRef name · Value)*
          StrRef  := varint offset into STRINGS (absolute)
COLD      attributes, original key spellings (absent when empty; stripped in production)
NAMES     deduplicated entries: varint n_ext · StrRef* (slot order) · varint n_local · StrRef*
FUNCS     varint n · StrRef* ("ns:name"), the manifest's function set, sorted
LOCALE    varint n · (varint key · varint len · payload)*   key 1 = plural.cardinal, 2 = plural.ordinal
          (P0.4 encoding v1); unknown keys skipped by length
IDS       (unstripped only) front-coded ids: (varint shared_prefix · varint suffix_len · suffix)*
STRINGS   last section, to EOF: deduplicated NUL-terminated UTF-8 strings; the writer groups
          identifiers (names, keys, function/option/markup names, locale tag) first, then text,
          and sorts each group bytewise
```

Section sizes for `en`: header 90, INDEX 6,400, MESSAGES 4,568, NAMES 701,
FUNCS 2, LOCALE 8, STRINGS 39,098 B. The structure (everything before STRINGS)
is 11.8–12.4 KB raw and **6.9–7.3 KB gz**. The pool is **12.8–18.6 KB gz**. The
plural data per locale is 5 B (en), 32 B (pl) and 17 B (ar, used for ar-XB by
subtag truncation).

## The open points of 02 §6, settled with numbers

Every row changes one axis of the recommended layout (Δ gz for en / pl / en-XA / ar-XB):

| Open point | Options measured | Result | Decision |
|---|---|---|---|
| Single vs split pools | one pool first-use · identifiers grouped first (= split pools) · one pool sorted · grouped + sorted | first-use +595/+571/+889/+1,538; grouped +464/+412/+704/+1,402; sorted +83/+160/+222/+26; **grouped + sorted best** | **One STRINGS section**. Grouping identifiers first and sorting each group is a writer policy only. A separate IDENTS section compressed identically (it only adds a 10 B section entry), and the reader does not care about order |
| Fixed 4-byte vs varint-delta INDEX | row-major u32 · **byte planes** · varint delta (not O(1)) · blocked delta (checkpoint / 16) | row-major +583/+1,147/+535/+1,371; delta +153/+641/+854/+907; blocked +731/+1,275/+1,251/+1,306 | **Fixed 4-byte entries, stored as byte planes.** Still one O(1) read (4 bytes). Delta schemes lose once the pool is sorted, and they would need load-time expansion (F2) or block decoding |
| Stripped vs unstripped | COLD + IDS (front-coded) | +8.6 / +9.4 / +9.8 / +9.3 KB gz | Strip for clients. COLD is empty for this workload; IDS is the whole cost |
| Cost of the load step | — | P0.8 | P0.8 recommends validating UTF-8 per string on access (a zero-copy load) |
| Dedup (implied by §2) | no dedup | +2.5 / +2.7 / +4.3 / +3.6 KB gz | Keep. Most of the gain is NAMES entries and names |
| Relative StrRefs (extra idea) | refs relative to a per-message base | +209 / +207 / +288 / +896 | Rejected: it only helps a first-use pool, and sorting beats it |

## Commands (from `probes/p0-07-catalog-encoding/`)

```sh
cargo xtask gen-workload locales --out probes/p0-07-catalog-encoding/corpus   # (repo root) inputs
scripts/measure.sh     # = build + unit tests + p07 stats + p07 measure + p07 emit
#   out/stats.md    corpus shape            out/sizes.tsv  every variant × locale (raw, gz, br)
#   out/tables.md   the tables above        out/catalogs/  recommended catalogs (<tag>.mf2b, <tag>.full.mf2b)
```

`p07 measure` writes, decodes and compares 600 catalogs. For each one it pipes
the bytes through `gzip -9 -n -c` and compresses them with brotli in process.
It takes about 3 minutes, single-threaded.

## Code (throwaway)

* `format/` (`mf2b-format`, `no_std`): the byte-format constants shared by
  writer and reader, so they cannot drift.
* `enc/` (`p07-enc`, lib + bin `p07`):
  * `parse.rs` is a lenient MF2 parser for the workload constructs. It handles
    text, escapes, `{$v}`, `{$v :fn opts}`, quoted and unquoted literals,
    function-only expressions, attributes, markup open/close/standalone with
    options, `.input` / `.local`, and `.match` with keys and `*`. It does no
    data-model validation.
  * `model.rs` holds the data model.
  * `manifest.rs` builds the manifest (plans/05 §3). Ids are sorted into
    MsgIds, and slots come from NFC-sorted names via `unicode-normalization`.
    It also records markup names and the function set, and computes
    `manifest_hash` (FNV-1a 64 per 02 §3; value `43e0dc12eeb05ef1`).
  * `write.rs` is the writer, with every layout variant as a `Layout` value.
  * `decode.rs` is a model-rebuilding decoder for every variant.
  * `lib.rs` is the corpus loader, which P0.3 and P0.8 reuse.

## Caveats

* The workload uses only `:integer`, and only in the 14 `.match` messages. It
  has no literals, no options, no attributes and no COLD content, and markup
  appears in 8 messages. Those encodings are specified and round-trip, but their
  share of the size is untested at scale. A corpus built with `--number` /
  `--datetime` would add options.
* `pl`, `en-XA` and `ar-XB` are synthetic. pl's syllable text is 35 % longer
  than `en` in bytes. en-XA's padding repeats (it compresses 4:1). The ar-XB
  RLO/PDF wrappers add 6 B per run.
* ar-XB takes CLDR `ar` plural rules by subtag truncation. A pseudo-locale of
  the source might instead want the source's rules. That choice belongs to the
  build tool.
* Numbers are from GNU gzip. Server-side `flate2`/zlib at level 9 would differ
  by a few bytes.

## Recommendations for Phase 2 (and inputs for C1/C2)

1. **Freeze the layout above** as format v1. It has NUL-terminated pool
   strings, a byte-plane INDEX, one STRINGS section written grouped and sorted,
   a FUNCS section, the LOCALE container as shown, and production catalogs
   stripped of COLD and IDS.
2. **Restate B7** so it scales with text. Keep "≤ 25 KB gz" for the reference
   `en` (it measures 20.5 KB), and add a per-locale rule such as
   **gz ≤ 0.5 × MF2 source bytes + 1 KB**. With the recommended layout that
   holds for all four locales: gz/source is 0.475 for en, 0.455 for pl, 0.248
   for en-XA and 0.377 for ar-XB. Alternatively, state B7 for brotli, which is
   how catalogs are served (`.br` sibling): ≤ 24.1 KB for every locale here.
3. **Writer policies to keep:** dedup, identifiers-first grouping, bytewise
   sort, and reject U+0000. Suffix sharing is also possible, since with NUL
   termination a string that is a suffix of another can point into it. It was
   not measured; the gain for UI text is probably small.
4. The mf2-build size gate should report **structure gz and pool gz
   separately**. The structure (≈ 7 KB gz, of which INDEX is 6.4 KB raw) is the
   part the format controls.

## Departures from plans/02 (plans not edited)

* §2 STRINGS "varint length + UTF-8" becomes **NUL-terminated UTF-8**. This is
  forced by F4, and it is also the smallest option measured.
* §2 INDEX `u32` entries are stored as **byte planes**. The semantics are the
  same and it stays O(1).
* §2 gains a **FUNCS section**: the function table §2.2 refers to has no
  section in the layout. The LOCALE container framing is as specified above.
* §6 "split pools" becomes one pool with a grouped + sorted writer order. §6
  "keep fixed unless the size gate complains" becomes fixed, as planes.

## Owner questions

1. **B7 for locales whose text is longer than the reference.** The synthetic
   `pl` has 35 % more bytes than `en` and misses 25 KB gz by 1.0 KB (it meets
   it under brotli). Should B7 scale with source bytes, as in recommendation 2,
   or be stated for brotli?
