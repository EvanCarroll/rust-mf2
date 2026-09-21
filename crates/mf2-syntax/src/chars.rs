//! Character classes of `spec/message.abnf`.

/// `ws = SP / HTAB / CR / LF / %x3000`.
#[cfg(test)]
pub(crate) fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{3000}')
}

/// `bidi = %x061C / %x200E / %x200F / %x2066-2069`.
#[cfg(test)]
pub(crate) fn is_bidi(c: char) -> bool {
    matches!(
        c,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{2066}'..='\u{2069}'
    )
}

/// `name-start`.
pub(crate) fn is_name_start(c: char) -> bool {
    let u = u32::from(c);
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
pub(crate) fn is_name_char(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '-' || c == '.'
}

/// `name-start` for an ASCII byte.
pub(crate) fn is_ascii_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'+' || b == b'_'
}

/// `name-char` for an ASCII byte.
pub(crate) fn is_ascii_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'_' | b'-' | b'.')
}

/// `ws` or `bidi` starting at byte `i` of `b` (UTF-8): `(length, is_ws)`.
pub(crate) fn trivia_at(b: &[u8], i: usize) -> Option<(usize, bool)> {
    match *b.get(i)? {
        b' ' | b'\t' | b'\n' | b'\r' => Some((1, true)),
        // U+061C
        0xD8 if b.get(i + 1) == Some(&0x9C) => Some((2, false)),
        // U+200E, U+200F, U+2066..U+2069
        0xE2 => match (*b.get(i + 1)?, *b.get(i + 2)?) {
            (0x80, 0x8E | 0x8F) | (0x81, 0xA6..=0xA9) => Some((3, false)),
            _ => None,
        },
        // U+3000
        0xE3 if b.get(i + 1) == Some(&0x80) && b.get(i + 2) == Some(&0x80) => Some((3, true)),
        _ => None,
    }
}

/// The index of the first byte at or after `i` that is `{`, `}`, `\\` or
/// U+0000 (`b.len()` if none): where a run of pattern text ends.
pub(crate) fn find_text_end(b: &[u8], i: usize) -> usize {
    find_any(b, i, *b"{}\\")
}

/// The index of the first byte at or after `i` that is `|`, `\\` or U+0000
/// (`b.len()` if none): where a run of quoted-literal text ends.
pub(crate) fn find_literal_end(b: &[u8], i: usize) -> usize {
    find_any(b, i, [b'|', b'\\', 0])
}

/// The first byte at or after `i` that is one of `needles` or 0, eight bytes
/// at a time (SWAR). For each needle `n`, `v ^ n·0x01…01` has a zero byte
/// where `v` has `n`; the classic zero-byte test flags it. That test can also
/// flag a byte *above* a true zero (a borrow), never below one, so the lowest
/// flagged byte of the combined mask is always the first match.
fn find_any(b: &[u8], mut i: usize, needles: [u8; 3]) -> usize {
    const LO: u64 = 0x0101_0101_0101_0101;
    const HI: u64 = 0x8080_8080_8080_8080;
    let zero = |v: u64| v.wrapping_sub(LO) & !v & HI;
    let [n0, n1, n2] = needles.map(|n| u64::from(n) * LO);
    while let Some(chunk) = b.get(i..i + 8) {
        let Ok(bytes) = <[u8; 8]>::try_from(chunk) else {
            break;
        };
        let v = u64::from_le_bytes(bytes);
        let m = zero(v) | zero(v ^ n0) | zero(v ^ n1) | zero(v ^ n2);
        if m != 0 {
            return i + (m.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while let Some(&c) = b.get(i) {
        if c == 0 || needles.contains(&c) {
            break;
        }
        i += 1;
    }
    i
}

/// Whether `s` matches `name` without its optional bidi marks:
/// `name-start *name-char`.
pub(crate) fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(is_name_start) && chars.all(is_name_char)
}

/// Whether `s` matches `identifier` without bidi marks: `[name ":"] name`.
pub(crate) fn is_identifier(s: &str) -> bool {
    match s.split_once(':') {
        Some((ns, name)) => is_name(ns) && is_name(name),
        None => is_name(s),
    }
}

/// Whether `s` matches `unquoted-literal = 1*name-char`.
pub(crate) fn is_unquoted_literal(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_name_char)
}

#[cfg(test)]
mod tests {
    use super::{
        is_ascii_name_char, is_ascii_name_start, is_bidi, is_identifier, is_name, is_name_char,
        is_name_start, is_unquoted_literal, is_ws,
    };

    #[test]
    fn ascii_classes_agree_with_char_classes() {
        for b in 0u8..0x80 {
            let c = char::from(b);
            assert_eq!(is_ascii_name_start(b), is_name_start(c), "{b:#x}");
            assert_eq!(is_ascii_name_char(b), is_name_char(c), "{b:#x}");
        }
    }

    #[test]
    fn name_start_ranges() {
        for c in [
            'a', 'Z', '+', '_', '\u{A1}', '\u{61B}', '\u{61D}', '\u{200B}', '\u{200D}',
        ] {
            assert!(is_name_start(c), "{c:?}");
        }
        for c in [
            '0',
            '-',
            '.',
            ':',
            ' ',
            '\u{A0}',
            '\u{61C}',
            '\u{1680}',
            '\u{200E}',
            '\u{2028}',
            '\u{202A}',
            '\u{205F}',
            '\u{2066}',
            '\u{3000}',
            '\u{FDD0}',
            '\u{FFFE}',
            '\u{FFFF}',
            '\u{1FFFE}',
            '\u{10FFFF}',
            '\u{7F}',
            '\u{9F}',
        ] {
            assert!(!is_name_start(c), "{c:?}");
        }
        assert!(is_name_start('\u{1F954}'));
        assert!(is_name_start('\u{10FFFD}'));
        assert!(is_name_char('7') && is_name_char('-') && is_name_char('.'));
    }

    #[test]
    fn whitespace_and_bidi() {
        for c in [' ', '\t', '\r', '\n', '\u{3000}'] {
            assert!(is_ws(c) && !is_bidi(c));
        }
        for c in ['\u{61C}', '\u{200E}', '\u{200F}', '\u{2066}', '\u{2069}'] {
            assert!(is_bidi(c) && !is_ws(c) && !is_name_char(c));
        }
        assert!(!is_ws('\u{B}') && !is_ws('\u{85}') && !is_bidi('\u{202A}'));
    }

    #[test]
    fn trivia_bytes_agree_with_char_classes() {
        let mut buf = [0u8; 4];
        for u in (0..0x3100u32).chain([0x61C, 0x200E, 0x200F, 0x2066, 0x2069, 0x3000]) {
            let Some(c) = char::from_u32(u) else { continue };
            let s = c.encode_utf8(&mut buf);
            let got = super::trivia_at(s.as_bytes(), 0);
            let want = (is_ws(c) || is_bidi(c)).then(|| (c.len_utf8(), is_ws(c)));
            assert_eq!(got, want, "{u:#x}");
        }
    }

    #[test]
    fn swar_scans_agree_with_bytewise_scans() {
        let naive = |b: &[u8], i: usize, set: &[u8]| {
            (i..b.len())
                .find(|&j| set.contains(&b[j]))
                .unwrap_or(b.len())
        };
        let samples: [&[u8]; 6] = [
            b"",
            b"plain text without specials, long enough for several words",
            b"a{b}c\\d|e\0f",
            "\u{ff}\u{7b}\u{fdd0}{x}|y|".as_bytes(),
            b"\x7b\x7b\x7b\x7b\x7b\x7b\x7b\x7b\x7b",
            b"0123456789abcdef0123456789abcdef}",
        ];
        for b in samples {
            for i in 0..=b.len() {
                assert_eq!(
                    super::find_text_end(b, i),
                    naive(b, i, b"{}\\\0"),
                    "{b:?} {i}"
                );
                assert_eq!(
                    super::find_literal_end(b, i),
                    naive(b, i, b"|\\\0"),
                    "{b:?} {i}"
                );
            }
        }
        // Every single-byte value, at every offset of a chunk.
        for byte in 0..=255u8 {
            for pos in 0..16 {
                let mut b = [b'a'; 16];
                b[pos] = byte;
                assert_eq!(super::find_text_end(&b, 0), naive(&b, 0, b"{}\\\0"));
                assert_eq!(super::find_literal_end(&b, 0), naive(&b, 0, b"|\\\0"));
            }
        }
    }

    #[test]
    fn composite_classes() {
        assert!(is_name("place-."));
        assert!(!is_name("1a") && !is_name("") && !is_name("a b"));
        assert!(is_identifier("ns:fn") && is_identifier("fn"));
        assert!(!is_identifier("ns:") && !is_identifier(":fn") && !is_identifier("a:b:c"));
        assert!(is_unquoted_literal("1.0") && is_unquoted_literal("-"));
        assert!(!is_unquoted_literal("") && !is_unquoted_literal("a|b"));
    }
}
