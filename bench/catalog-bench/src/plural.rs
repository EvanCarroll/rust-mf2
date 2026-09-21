//! The `plural.cardinal` LOCALE entries (key 1, encoding v1,
//! `plans/02-catalog-format.md` §4.1) the four workload locales carry, as P0.7
//! put them in every catalog it measured.
//!
//! The bytes were produced once by P0.4's encoder over the vendored CLDR data
//! (`third_party/cldr-json`, CLDR 48.2.1): `p04 --cldr
//! third_party/cldr-json/cldr-core/supplemental dump <locale>` in
//! `probes/p0-04-plural` (a throwaway probe, deleted at the end of Phase 2).
//! They are hard-coded here until `mf2-locale-data` (Phase 3) produces them;
//! it replaces this module.

use mf2_catalog::CldrVersion;

/// The CLDR version of the entries below (`third_party/cldr-json/PIN`).
pub const CLDR: CldrVersion = CldrVersion {
    major: 48,
    minor: 2,
    patch: 1,
};

/// `en` cardinal — one: `i = 1 and v = 0` (5 B).
pub const EN: &[u8] = &[0x21, 0x01, 0x05, 0x82, 0x01];

/// `pl` cardinal — one, few, many (32 B).
pub const PL: &[u8] = &[
    0x21, 0x01, 0x05, 0x82, 0x01, 0x61, 0x02, 0x01, 0x09, 0x0b, 0x02, 0xd1, 0x33, 0x02, 0x83, 0x02,
    0x01, 0x41, 0x05, 0x89, 0x03, 0x01, 0x02, 0x01, 0x89, 0x17, 0x04, 0x02, 0x01, 0x91, 0x33, 0x02,
];

/// `ar` cardinal — zero, one, two, few, many (17 B).
pub const AR: &[u8] = &[
    0x01, 0x80, 0x01, 0x21, 0x80, 0x05, 0x41, 0x80, 0x09, 0x61, 0x90, 0x0f, 0x07, 0x81, 0x90, 0x2f,
    0x58,
];

/// The CLDR locale whose cardinal rules a workload tag uses, and its entry.
/// Pseudo-locales take their base language's rules by subtag truncation, as
/// P0.7 did: `en-XA` → `en`, `ar-XB` → `ar`.
pub fn cardinal(tag: &str) -> Option<(&'static str, &'static [u8])> {
    match tag.split('-').next() {
        Some("en") => Some(("en", EN)),
        Some("pl") => Some(("pl", PL)),
        Some("ar") => Some(("ar", AR)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{AR, EN, PL, cardinal};

    #[test]
    fn sizes_match_p04_and_p07() {
        // P0.4 / P0.7: 5, 32 and 17 B.
        assert_eq!((EN.len(), PL.len(), AR.len()), (5, 32, 17));
        assert_eq!(cardinal("en-XA").map(|c| c.0), Some("en"));
        assert_eq!(cardinal("ar-XB").map(|c| c.0), Some("ar"));
        assert_eq!(cardinal("pl").map(|c| c.1.len()), Some(32));
        assert!(cardinal("fr").is_none());
    }
}
