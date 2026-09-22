//! The name character classes of `spec/message.abnf`.
//!
//! They live here because two frontends need them: `mf2-syntax` for names
//! inside a message, and `mf2-resource` for the parts of an entry's id
//! (`plans/05-tooling.md` §2: "letters, digits, `_`, `-` and non-ASCII name
//! characters").

/// `name-start`: an ASCII letter, `+`, `_`, or a non-ASCII character that is
/// not whitespace, a bidi control, a surrogate, a private-use character or a
/// noncharacter.
#[inline]
pub const fn is_name_start(c: char) -> bool {
    let u = c as u32;
    if u < 0x80 {
        return c.is_ascii_alphabetic() || c == '+' || c == '_';
    }
    matches!(u,
        0xA1..=0x61B
        | 0x61D..=0x167F
        | 0x1681..=0x1FFF
        | 0x200B..=0x200D
        | 0x2010..=0x2027
        | 0x2030..=0x205E
        | 0x2060..=0x2065
        | 0x206A..=0x2FFF
        | 0x3001..=0xD7FF
        | 0xE000..=0xFDCF
        | 0xFDF0..=0xFFFD)
        // %x10000-1FFFD … %x100000-10FFFD: every plane minus its last two
        // code points (the noncharacters U+nFFFE and U+nFFFF).
        || (u >= 0x1_0000 && (u & 0xFFFE) != 0xFFFE)
}

/// `name-char = name-start / DIGIT / "-" / "."`.
#[inline]
pub const fn is_name_char(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '-' || c == '.'
}
