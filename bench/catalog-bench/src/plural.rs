//! The `plural.cardinal` LOCALE entries (key 1, encoding v1,
//! `plans/02-catalog-format.md` §4.1) the four workload locales carry, from
//! `mf2-locale-data` (Phase 3, A6) — until then P0.4's bytes were hard-coded
//! here; the test below holds the two to be identical.

use mf2_catalog::CldrVersion;
use mf2_locale_data::{PluralKind, plural_rules};

/// The CLDR version of the entries (`third_party/cldr-json/PIN`).
pub const CLDR: CldrVersion = mf2_locale_data::CLDR_VERSION;

/// The CLDR locale whose cardinal rules a workload tag uses, and its entry.
/// Pseudo-locales take their base language's rules by subtag truncation, as
/// P0.7 did: `en-XA` → `en`, `ar-XB` → `ar`. `None` if the tag has no rules
/// of its own (it would fall back to root).
pub fn cardinal(tag: &str) -> Option<(&'static str, Vec<u8>)> {
    let rules = plural_rules(PluralKind::Cardinal, tag).ok()?;
    if rules.locale == "und" {
        return None;
    }
    Some((rules.locale, mf2_locale_data::plural::encode(rules.rules)))
}

#[cfg(test)]
mod tests {
    use super::cardinal;

    /// P0.4's bytes, which P0.7 and Phase 2 measured with (CLDR 48.2.1).
    const EN: &[u8] = &[0x21, 0x01, 0x05, 0x82, 0x01];
    const PL: &[u8] = &[
        0x21, 0x01, 0x05, 0x82, 0x01, 0x61, 0x02, 0x01, 0x09, 0x0b, 0x02, 0xd1, 0x33, 0x02, 0x83,
        0x02, 0x01, 0x41, 0x05, 0x89, 0x03, 0x01, 0x02, 0x01, 0x89, 0x17, 0x04, 0x02, 0x01, 0x91,
        0x33, 0x02,
    ];
    const AR: &[u8] = &[
        0x01, 0x80, 0x01, 0x21, 0x80, 0x05, 0x41, 0x80, 0x09, 0x61, 0x90, 0x0f, 0x07, 0x81, 0x90,
        0x2f, 0x58,
    ];

    #[test]
    fn entries_are_p04_bytes() {
        assert_eq!(cardinal("en"), Some(("en", EN.to_vec())));
        assert_eq!(cardinal("en-XA"), Some(("en", EN.to_vec())));
        assert_eq!(cardinal("pl"), Some(("pl", PL.to_vec())));
        assert_eq!(cardinal("ar-XB"), Some(("ar", AR.to_vec())));
        assert!(cardinal("xx").is_none());
    }
}
