//! Comparison under Unicode Normalization Form C (NFC), quick check first.
//!
//! Names and keys are compared "as if" normalized (spec, "Names and
//! Identifiers", "Key"); nothing here changes the model. The quick check
//! (every code point below U+0300 is already NFC, then the Unicode quick-check
//! property) answers almost every comparison without normalizing, and never
//! allocates; only a string that may not be NFC is normalized, lazily, as an
//! iterator.

use alloc::borrow::Cow;
use alloc::string::String;

use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};

/// Whether `s` is certainly in NFC (without normalizing).
pub(crate) fn is_nfc(s: &str) -> bool {
    // Bytes below 0xCC encode only code points below U+0300 (lead bytes
    // C2..CB and continuation bytes 80..BF), which are always NFC.
    s.bytes().all(|b| b < 0xCC) || is_nfc_quick(s.chars()) == IsNormalized::Yes
}

/// `a` and `b` are canonically equivalent.
pub(crate) fn nfc_eq(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if is_nfc(a) && is_nfc(b) {
        return false;
    }
    a.nfc().eq(b.nfc())
}

/// `s` in NFC, borrowed when it already is.
pub(crate) fn nfc(s: &str) -> Cow<'_, str> {
    if is_nfc(s) {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(s.nfc().collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;

    use super::{is_nfc, nfc, nfc_eq};

    #[test]
    fn equivalence() {
        // Ḍ̇ (U+1E0C U+0307) ≡ D U+0323 U+0307 ≡ Ḍ̇ (U+1E0A U+0323).
        assert!(nfc_eq("\u{1E0C}\u{307}", "D\u{323}\u{307}"));
        assert!(nfc_eq("\u{1E0A}\u{323}", "\u{1E0C}\u{307}"));
        assert!(nfc_eq("D\u{307}\u{323}", "\u{1E0C}\u{307}"));
        // Ǻ (U+01FA) ≡ A U+030A U+0301.
        assert!(nfc_eq("A\u{30A}\u{301}", "\u{1FA}"));
        assert!(!nfc_eq("a", "b"));
        assert!(!nfc_eq("\u{e9}", "e"));
        assert!(nfc_eq("", ""));
    }

    #[test]
    fn quick_check_and_normalization() {
        assert!(is_nfc("plain ascii"));
        assert!(is_nfc("\u{e9}t\u{e9}"));
        assert!(!is_nfc("e\u{301}"));
        assert!(matches!(nfc("abc"), Cow::Borrowed("abc")));
        assert_eq!(nfc("e\u{301}"), "\u{e9}");
        assert_eq!(nfc("D\u{323}\u{307}"), "\u{1E0C}\u{307}");
    }
}
