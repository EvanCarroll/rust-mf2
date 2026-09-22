//! Small text helpers written by hand for the client path (05 §8): no
//! `core::fmt`, no `str::find`/`split*`, no panicking slice.

use crate::sink::Sink;

/// Writes `n` in ASCII decimal (`-` for negatives).
pub(crate) fn write_i64(n: i64, out: &mut dyn Sink) {
    if n < 0 {
        out.push_str("-");
    }
    write_u64(n.unsigned_abs(), out);
}

/// Writes `n` in ASCII decimal.
pub(crate) fn write_u64(n: u64, out: &mut dyn Sink) {
    let mut buf = [0u8; 20];
    out.push_str(u64_str(n, &mut buf));
}

/// `n` in ASCII decimal, in `buf`.
pub(crate) fn u64_str(mut n: u64, buf: &mut [u8; 20]) -> &str {
    let mut at = buf.len();
    loop {
        at -= 1;
        // `n % 10 < 10`, so the cast and the addition cannot overflow.
        #[allow(clippy::cast_possible_truncation)]
        let d = (n % 10) as u8;
        if let Some(b) = buf.get_mut(at) {
            *b = b'0' + d;
        }
        n /= 10;
        if n == 0 || at == 0 {
            break;
        }
    }
    let digits = buf.get(at..).unwrap_or(&[]);
    core::str::from_utf8(digits).unwrap_or("")
}

/// The NFC quick check of `plans/03-runtime.md` §7: every code point is
/// below U+0300 (so the string is already NFC). In UTF-8 that is every byte
/// below 0xCC, since U+0300 is `CC 80`.
#[inline]
pub(crate) fn nfc_quick(s: &str) -> bool {
    s.bytes().all(|b| b < 0xCC)
}

/// Writes `s` with `\` and `|` escaped (a literal's fallback source,
/// formatting.md "Fallback Resolution"). Both are ASCII, so every cut is on
/// a char boundary.
pub(crate) fn write_escaped_literal(s: &str, out: &mut dyn Sink) {
    let mut start = 0;
    for (i, b) in s.bytes().enumerate() {
        if b == b'\\' || b == b'|' {
            out.push_str(s.get(start..i).unwrap_or(""));
            out.push_str(if b == b'\\' { "\\\\" } else { "\\|" });
            start = i + 1;
        }
    }
    out.push_str(s.get(start..).unwrap_or(""));
}

#[cfg(test)]
mod tests {
    extern crate std;
    use alloc::string::String;

    use super::{nfc_quick, write_escaped_literal, write_i64};

    #[test]
    fn integers() {
        for n in [0i64, 7, -7, 10, 1_000_000, i64::MAX, i64::MIN] {
            let mut s = String::new();
            write_i64(n, &mut s);
            assert_eq!(s, std::format!("{n}"));
        }
    }

    #[test]
    fn escaping() {
        let mut s = String::new();
        write_escaped_literal(r"C:\a|b", &mut s);
        assert_eq!(s, r"C:\\a\|b");
    }

    #[test]
    fn quick_check() {
        assert!(nfc_quick("hello é"));
        assert!(!nfc_quick("e\u{301}"));
        assert!(!nfc_quick("\u{1e0a}"));
    }
}
