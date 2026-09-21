//! The byte-format constants of `.mf2b` version 1 (`plans/02-catalog-format.md`
//! §2). The writer and the reader both use these, so they cannot drift.

/// `"MF2B"`.
pub const MAGIC: [u8; 4] = *b"MF2B";
/// `format_version`: major 1, minor 0 (`major << 8 | minor`).
pub const VERSION: u16 = 0x0100;
/// The one major version this reader understands (F9).
pub const VERSION_MAJOR: u16 = 1;

/// Length of the fixed header, before the section table.
pub const HEADER_LEN: usize = 32;
/// Length of one section-table entry: kind `u16`, offset `u32`, length `u32`.
pub const SECTION_ENTRY_LEN: usize = 10;

/// Largest `message_count` (the `MsgId` index has 24 bits).
pub const MAX_MESSAGES: u32 = 1 << 24;
/// Largest INDEX offset (30 bits): MESSAGES and STRINGS stay below 2³⁰ bytes.
pub const MAX_OFFSET: u32 = (1 << 30) - 1;
/// Largest number of fallback locales (the FALLBACK locale index has 8 bits).
pub const MAX_FALLBACK_LOCALES: usize = 256;
/// IDS: every this many ids, a restart point (an id with `shared = 0`).
pub const IDS_RESTART: usize = 16;

/// Header field offsets.
pub mod header {
    /// `[u8; 4]` `"MF2B"`.
    pub const MAGIC: usize = 0;
    /// `u16` `major << 8 | minor`.
    pub const VERSION: usize = 4;
    /// `u16`, see [`super::flags`].
    pub const FLAGS: usize = 6;
    /// `u64`.
    pub const MANIFEST_HASH: usize = 8;
    /// `u32` ≤ 2²⁴.
    pub const MESSAGE_COUNT: usize = 16;
    /// `str32`: the BCP 47 tag.
    pub const LOCALE: usize = 20;
    /// `u32` `major << 16 | minor << 8 | patch`; 0 = none.
    pub const CLDR_VERSION: usize = 24;
    /// `u8`.
    pub const CHUNK: usize = 28;
    /// `u8`: 0 ltr, 1 rtl.
    pub const DIR: usize = 29;
    /// `u16`.
    pub const SECTION_COUNT: usize = 30;
}

/// Header `flags` bits. Other bits are written 0 and ignored by readers.
pub mod flags {
    /// COLD was stripped (§2.3).
    pub const COLD_STRIPPED: u16 = 1;
    /// IDS was stripped (§2.3).
    pub const IDS_STRIPPED: u16 = 2;
}

/// Section kinds. Unknown kinds are skipped.
pub mod section {
    pub const INDEX: u16 = 1;
    pub const MESSAGES: u16 = 2;
    pub const COLD: u16 = 3;
    pub const NAMES: u16 = 4;
    pub const FALLBACK: u16 = 5;
    pub const LOCALE: u16 = 6;
    pub const FUNCS: u16 = 7;
    pub const IDS: u16 = 8;
    /// The string pool: always present, always the last section.
    pub const STRINGS: u16 = 15;

    /// The name of a known section kind.
    pub const fn name(kind: u16) -> Option<&'static str> {
        Some(match kind {
            INDEX => "INDEX",
            MESSAGES => "MESSAGES",
            COLD => "COLD",
            NAMES => "NAMES",
            FALLBACK => "FALLBACK",
            LOCALE => "LOCALE",
            FUNCS => "FUNCS",
            IDS => "IDS",
            STRINGS => "STRINGS",
            _ => return None,
        })
    }
}

/// INDEX entry: `kind << 30 | offset`.
pub mod kind {
    pub const SIMPLE: u32 = 0;
    pub const PATTERN: u32 = 1;
    pub const SELECT: u32 = 2;
    pub const ABSENT: u32 = 3;
    pub const SHIFT: u32 = 30;
    pub const OFFSET_MASK: u32 = (1 << 30) - 1;
}

/// Part and declaration tag bytes (MESSAGES, §2.2).
pub mod tag {
    /// Bits 0–2: the part kind.
    pub const KIND_MASK: u8 = 0b111;
    pub const TEXT: u8 = 0;
    pub const EXPRESSION: u8 = 1;
    pub const OPEN: u8 = 2;
    pub const STANDALONE: u8 = 3;
    pub const CLOSE: u8 = 4;
    /// Bits 3–4 of an expression or declaration tag: the operand.
    pub const OP_SHIFT: u8 = 3;
    pub const OP_MASK: u8 = 0b11 << 3;
    pub const OP_NONE: u8 = 0;
    pub const OP_LITERAL: u8 = 1;
    pub const OP_VARIABLE: u8 = 2;
    /// Bit 5 of an expression or declaration tag: a `FunctionRef` follows.
    pub const FUNCTION: u8 = 0x20;
    /// Bit 7 of a declaration tag: `.local` (else `.input`).
    pub const LOCAL: u8 = 0x80;
    /// Bit 7 of a markup tag: `Options` follow the name.
    pub const OPTIONS: u8 = 0x80;
    /// Every bit an expression tag may have.
    pub const EXPRESSION_BITS: u8 = KIND_MASK | OP_MASK | FUNCTION;
    /// Every bit a markup tag may have.
    pub const MARKUP_BITS: u8 = KIND_MASK | OPTIONS;
}

/// COLD override kinds (§2.5).
pub mod cold {
    pub const SPELLING: u8 = 0;
    pub const ATTRIBUTES: u8 = 1;
    pub const CATCH_ALL: u8 = 2;
}

/// LOCALE entry keys (§2.7, §4). The key names the entry kind and its version.
pub mod locale_key {
    /// `plural.cardinal`, encoding v1 (§4.1).
    pub const PLURAL_CARDINAL: u32 = 1;
    /// `plural.ordinal`, encoding v1 (§4.1).
    pub const PLURAL_ORDINAL: u32 = 2;
}
