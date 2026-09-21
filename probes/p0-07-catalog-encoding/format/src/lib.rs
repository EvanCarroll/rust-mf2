//! P0.7 — byte-format constants of the probe `.mf2b` encoding (plans/02 §2).
//!
//! The writer (`p07-enc`) and the client reader (`probes/p0-03-runtime-floor/rt`)
//! both use these, so they cannot drift. The full layout is specified in
//! `probes/p0-07-catalog-encoding/RESULT.md` ("Recommended layout").
//!
//! All integers are little-endian and read from unaligned slices (F5).
//! `varint` is unsigned LEB128, minimal, at most 5 bytes for `u32` values.
#![no_std]
#![forbid(unsafe_code)]

/// `"MF2B"`.
pub const MAGIC: [u8; 4] = *b"MF2B";
/// Probe format version (the product starts at 1 when Phase 2 freezes it).
pub const VERSION: u16 = 0x0007;

/// Fixed header, byte offsets.
pub mod header {
    pub const MAGIC: usize = 0; // [u8; 4]
    pub const VERSION: usize = 4; // u16
    pub const FLAGS: usize = 6; // u16
    pub const MANIFEST_HASH: usize = 8; // u64
    pub const CHUNK: usize = 16; // u8
    pub const DIR: usize = 17; // u8: 0 ltr, 1 rtl
    pub const MESSAGE_COUNT: usize = 18; // u32
    pub const LOCALE_OFF: usize = 22; // u32, offset of the locale tag in the pool region
    pub const LOCALE_LEN: usize = 26; // u16, its byte length
    pub const SECTION_COUNT: usize = 28; // u16
    /// Section table starts here: `section_count × (kind u16, off u32, len u32)`.
    pub const SECTIONS: usize = 30;
    pub const SECTION_ENTRY: usize = 10;
}

/// Header `flags`.
pub mod flags {
    /// Bits 0–1: how string lengths are found (the P0.7 comparison; the reader
    /// supports only [`STR_NUL`]).
    pub const STR_MASK: u16 = 0b11;
    /// Each pool string is preceded by its byte length encoded **as one UTF-8
    /// scalar value** (1–3 bytes; lengths ≥ 0xD800 are shifted by 0x800 past
    /// the surrogates), so the whole pool stays valid UTF-8. StrRef = offset of
    /// the prefix.
    pub const STR_PREFIX_CHAR: u16 = 0;
    /// StrRef = `varint offset, varint len`; the pool is bare text. Simple
    /// messages take their length from the parallel `SLEN` section (u16 each).
    pub const STR_REF_LEN: u16 = 1;
    /// StrRef = `varint string index`; `STROFF` holds one u32 end offset per
    /// string; the pool is bare text.
    pub const STR_OFFSETS: u16 = 2;
    /// StrRef = offset; each string ends at the next U+0000 (the writer rejects
    /// strings that contain U+0000).
    pub const STR_NUL: u16 = 3;
    /// Bits 2–3: INDEX layout (size comparison; the reader supports only planes).
    pub const INDEX_MASK: u16 = 0b1100;
    pub const INDEX_FIXED: u16 = 0;
    pub const INDEX_VARINT_DELTA: u16 = 0b0100;
    pub const INDEX_BLOCKED: u16 = 0b1000;
    /// Fixed u32 entries stored as four byte planes (all low bytes, then all
    /// second bytes, …): still one O(1) read per entry, compresses far better.
    pub const INDEX_PLANES: u16 = 0b1100;
    /// Informational: the writer grouped identifiers (names, keys, function
    /// and option names) before text in the one STRINGS pool.
    pub const SPLIT_POOLS: u16 = 0x10;
    /// Informational: each pool group is sorted bytewise (writer policy).
    pub const SORTED_POOL: u16 = 0x80;
    /// COLD and IDS were stripped (plans/02 §2.3).
    pub const STRIPPED: u16 = 0x20;
    /// StrRefs inside a MESSAGES entry are `zigzag(value − base)`, `base` being a
    /// varint right after the entry's names_ref. NAMES / FUNCS / COLD refs stay absolute.
    pub const REL_REFS: u16 = 0x40;
}

/// Section kinds. Unknown kinds are skipped; required kinds missing ⇒ reject.
pub mod section {
    pub const INDEX: u16 = 1;
    pub const MESSAGES: u16 = 2;
    pub const COLD: u16 = 3;
    pub const NAMES: u16 = 4;
    pub const FALLBACK: u16 = 5;
    pub const LOCALE: u16 = 6;
    /// Function table: `varint count, StrRef(ident)×count` (`ns:name`).
    pub const FUNCS: u16 = 7;
    pub const IDS: u16 = 8;
    /// `STR_REF_LEN` only: u16 length per message (0 unless simple).
    pub const SLEN: u16 = 9;
    /// `STR_OFFSETS` only: u32 end offset per pool string.
    pub const STROFF: u16 = 10;
    /// `INDEX_BLOCKED` only: u32 offset into INDEX of every 16th entry.
    pub const INDEX_BLOCKS: u16 = 11;
    /// Reserved (a separate identifier pool measured the same as grouping
    /// identifiers first inside STRINGS, so the layout keeps one pool).
    pub const IDENTS: u16 = 14;
    /// The string pool; always the last section, running to the end of the file.
    pub const STRINGS: u16 = 15;
}

/// INDEX entry kind (2 high bits of the u32 entry).
pub mod kind {
    pub const SIMPLE: u32 = 0;
    pub const PATTERN: u32 = 1;
    pub const SELECT: u32 = 2;
    pub const ABSENT: u32 = 3;
    pub const SHIFT: u32 = 30;
    pub const OFFSET_MASK: u32 = (1 << 30) - 1;
}

/// Part header byte (patterns).
pub mod part {
    /// Bits 0–2: part kind.
    pub const KIND_MASK: u8 = 0b111;
    pub const TEXT: u8 = 0;
    pub const EXPR: u8 = 1;
    pub const OPEN: u8 = 2;
    pub const STANDALONE: u8 = 3;
    pub const CLOSE: u8 = 4;
    /// Bits 3–4 (expressions and declarations): operand kind.
    pub const OPERAND_SHIFT: u8 = 3;
    pub const OPERAND_MASK: u8 = 0b11 << 3;
    pub const OPERAND_NONE: u8 = 0;
    pub const OPERAND_LITERAL: u8 = 1;
    pub const OPERAND_VARIABLE: u8 = 2;
    /// Bit 5: a FunctionRef follows (expressions).
    pub const HAS_FUNCTION: u8 = 0x20;
    /// Bit 6: a ColdRef (varint offset into COLD) follows last.
    pub const HAS_COLD: u8 = 0x40;
    /// Bit 7: options follow (markup; expressions carry options in the FunctionRef).
    pub const HAS_OPTIONS: u8 = 0x80;
}

/// Declaration tags (first byte of a declaration; the rest as a part header).
pub mod decl {
    /// Tag in bit 7 of the declaration header byte; bits 0–6 as [`super::part`]
    /// for the expression (operand bits, function, cold).
    pub const LOCAL: u8 = 0x80;
}

/// LOCALE container: `varint entry_count, (varint key, varint len, payload)*`.
/// Unknown keys are skipped by length.
pub mod locale_key {
    /// P0.4 encoding v1.
    pub const PLURAL_CARDINAL: u32 = 1;
    pub const PLURAL_ORDINAL: u32 = 2;
}

/// Surrogate gap for [`flags::STR_PREFIX_CHAR`]: lengths at or above this are
/// stored as `len + SURROGATE_SHIFT`.
pub const SURROGATE_START: u32 = 0xD800;
pub const SURROGATE_SHIFT: u32 = 0x800;
/// Largest length a prefix char can carry.
pub const MAX_PREFIX_LEN: u32 = 0x10_FFFF - SURROGATE_SHIFT;

/// Encodes `len` as the length-prefix scalar (writer side helper; `None` if too long).
#[must_use]
pub fn prefix_char(len: u32) -> Option<char> {
    let v = if len >= SURROGATE_START { len.checked_add(SURROGATE_SHIFT)? } else { len };
    char::from_u32(v)
}

/// Decodes a length-prefix scalar value.
#[must_use]
pub const fn prefix_len(c: u32) -> u32 {
    if c >= SURROGATE_START + SURROGATE_SHIFT { c - SURROGATE_SHIFT } else { c }
}
