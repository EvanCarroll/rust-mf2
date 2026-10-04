//! The number LOCALE entries built
//! from the shipped table: the byte-exact vectors of §4.5, CLDR's locale
//! resolution, every locale and style encoding to entries the client views
//! read back, a catalog round trip, and budget B8 (plural +
//! number symbols ≤ 0.5 KB gz per locale on the panel).
//!
//! Run the B8 table with `cargo test -p mf2-locale-data --test numbers b8 --
//! --nocapture`.

use std::io::Write as _;

use mf2_catalog::format::locale_key;
use mf2_catalog::number::{Patterns, SignShown, Style, Symbols};
use mf2_catalog::writer::{Options, catalog};
use mf2_catalog::{Catalog, Dir, Manifest};
use mf2_locale_data::{
    CurrencyNeeds, LocaleNeeds, NumberNeeds, locale_entries, number_data, number_locale_entries,
    number_locales,
};

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex"))
        .collect()
}

/// §4.5: `number.symbols`, and `number.patterns` with the percent style.
const VECTORS: &[(&str, &str, &str)] = &[
    (
        "en",
        "33 01 01 2E 01 2C 01 2D 01 2B 01 25",
        "01 04 33 00 01 02",
    ),
    (
        "fr",
        "33 01 01 2C 03 E2 80 AF 01 2D 01 2B 01 25",
        "01 06 33 00 03 C2 A0 02",
    ),
    (
        "ar",
        "33 01 01 2E 01 2C 04 E2 80 8E 2D 04 E2 80 8E 2B 07 E2 80 8E 25 E2 80 8E",
        "01 04 33 00 01 02",
    ),
    (
        "ar-u-nu-arab",
        "33 01 02 D9 AB 02 D9 AC 03 D8 9C 2D 03 D8 9C 2B 04 D9 AA D8 9C \
         D9 A0 D9 A1 D9 A2 D9 A3 D9 A4 D9 A5 D9 A6 D9 A7 D9 A8 D9 A9",
        "01 04 33 00 01 02",
    ),
    (
        "hi",
        "23 01 01 2E 01 2C 01 2D 01 2B 01 25",
        "01 04 23 00 01 02",
    ),
    (
        "pl",
        "33 02 01 2C 02 C2 A0 01 2D 01 2B 01 25",
        "01 04 33 00 01 02",
    ),
];

/// §4.5: every style, for the negative subpatterns and the omitted
/// `…Alpha` records.
const ALL_STYLES: &[(&str, &str)] = &[
    (
        "en",
        "01 04 33 00 01 02  02 04 33 01 03 00  03 06 33 03 03 C2 A0 00  04 03 33 00 00  \
         05 09 33 01 03 00 02 28 03 01 29  06 0D 33 03 03 C2 A0 00 04 28 03 C2 A0 01 29  \
         07 07 33 00 00 01 28 01 29",
    ),
    (
        "pl",
        "01 04 33 00 01 02  02 06 33 00 03 C2 A0 03  04 03 33 00 00  \
         05 0D 33 00 03 C2 A0 03 01 28 04 C2 A0 03 29  07 07 33 00 00 01 28 01 29",
    ),
    (
        "ar",
        "01 04 33 00 01 02  \
         02 12 33 03 E2 80 8F 03 C2 A0 03 04 E2 80 8F 01 03 C2 A0 03  \
         04 0C 33 03 E2 80 8F 00 04 E2 80 8F 01 00  \
         05 0D 33 02 D8 9C 01 03 03 28 D8 9C 02 03 29  \
         06 11 33 02 D8 9C 03 C2 A0 03 03 28 D8 9C 04 C2 A0 03 29  \
         07 0B 33 02 D8 9C 00 03 28 D8 9C 01 29",
    ),
];

#[test]
fn vectors() {
    for (tag, symbols, percent) in VECTORS {
        let d = number_data(tag).expect("data");
        assert_eq!(
            d.symbols_entry().expect("symbols"),
            hex(symbols),
            "{tag} symbols"
        );
        assert_eq!(
            d.patterns_entry(&[Style::Percent]).expect("patterns"),
            hex(percent),
            "{tag} percent"
        );
        let mut needs = NumberNeeds::default();
        needs.percent = true;
        assert_eq!(
            number_locale_entries(tag, &needs).expect("entries"),
            [
                (locale_key::NUMBER_SYMBOLS, hex(symbols)),
                (locale_key::NUMBER_PATTERNS, hex(percent)),
            ],
            "{tag}"
        );
    }
    for (tag, all) in ALL_STYLES {
        let d = number_data(tag).expect("data");
        assert_eq!(
            d.patterns_entry(&Style::ALL).expect("all"),
            hex(all),
            "{tag}"
        );
    }
}

#[test]
fn vectors_read_back() {
    let fr = hex(VECTORS[1].1);
    let s = Symbols::parse(&fr).expect("fr");
    assert_eq!(
        (s.decimal(), s.group(), s.minus(), s.plus(), s.percent()),
        (",", "\u{202f}", "-", "+", "%")
    );
    assert_eq!((s.grouping().primary, s.grouping().secondary), (3, 3));
    assert_eq!(s.minimum_grouping_digits(), 1);
    assert!(s.digits().is_ascii());

    let arab = hex(VECTORS[3].1);
    let s = Symbols::parse(&arab).expect("ar-u-nu-arab");
    assert_eq!(s.decimal(), "٫");
    assert_eq!(s.minus(), "\u{61c}-");
    assert_eq!(s.digits().as_str(), "٠١٢٣٤٥٦٧٨٩");
    assert_eq!(s.digits().digit(9), "٩");

    let hi = hex(VECTORS[4].1);
    let hi = Symbols::parse(&hi).expect("hi");
    assert_eq!((hi.grouping().primary, hi.grouping().secondary), (3, 2));
    let pl = hex(VECTORS[5].1);
    let pl = Symbols::parse(&pl).expect("pl");
    assert_eq!(pl.minimum_grouping_digits(), 2);

    let en = hex(ALL_STYLES[0].1);
    let p = Patterns::new(&en);
    assert!(p.is_valid());
    let acc = p.get(Style::Accounting).expect("accounting");
    let neg = acc.signed(SignShown::Minus);
    assert!(!neg.sign_first);
    assert_eq!(neg.prefix.encoded(), "(\u{3}");
    assert_eq!(neg.suffix.encoded(), ")");

    let ar = hex(ALL_STYLES[2].1);
    let p = Patterns::new(&ar);
    let cur = p.get(Style::Currency).expect("currency");
    let neg = cur.signed(SignShown::Minus);
    assert_eq!(neg.prefix.encoded(), "\u{200f}\u{1}");
    let plus = cur.signed(SignShown::Plus);
    assert!(!plus.sign_first, "the plus sign takes the minus's position");
    assert_eq!(plus.prefix.encoded(), "\u{200f}\u{1}");
    // `ar`'s alphaNextToNumber currency equals its standard one: omitted.
    assert!(p.get(Style::CurrencyAlpha).is_none());
    assert_eq!(p.resolve(Style::CurrencyAlpha), Some(cur));
}

#[test]
fn locale_resolution() {
    for (tag, locale, nu, symbols_system) in [
        ("en", "en", "latn", "latn"),
        ("en-US", "en", "latn", "latn"),
        ("en-Latn-US", "en", "latn", "latn"),
        ("en-AU", "en-AU", "latn", "latn"),
        ("en-Cyrl", "und", "latn", "latn"),
        ("EN_gb", "en-GB", "latn", "latn"),
        ("es-MX", "es-MX", "latn", "latn"),
        ("es-AR", "es-AR", "latn", "latn"),
        ("es-ZZ", "es", "latn", "latn"),
        ("pt-AO", "pt-AO", "latn", "latn"),
        ("zh-TW", "zh-Hant", "latn", "latn"),
        ("zh-HK", "zh-Hant-HK", "latn", "latn"),
        ("zh-CN", "zh", "latn", "latn"),
        ("pa-PK", "pa-Arab", "arabext", "arabext"),
        ("sr-ME", "sr-Latn-ME", "latn", "latn"),
        ("ar", "ar", "latn", "latn"),
        ("ar-EG", "ar-EG", "arab", "arab"),
        ("ar-EG-u-nu-latn", "ar-EG", "latn", "latn"),
        ("ar-u-nu-native", "ar", "arab", "arab"),
        ("ar-u-nu-arab", "ar", "arab", "arab"),
        ("en-u-nu-arab", "en", "arab", "latn"),
        ("hi-u-nu-deva", "hi", "deva", "deva"),
        ("th-u-nu-thai", "th", "thai", "thai"),
        ("en-u-nu-roman", "en", "latn", "latn"),
        ("ja-u-nu-traditio", "ja", "latn", "latn"),
        ("zh-u-nu-hanidec", "zh", "hanidec", "hanidec"),
        ("fa", "fa", "arabext", "arabext"),
        ("und", "und", "latn", "latn"),
        ("root", "und", "latn", "latn"),
        ("xx-YY", "und", "latn", "latn"),
        ("", "und", "latn", "latn"),
    ] {
        let d = number_data(tag).expect("data");
        assert_eq!(
            (d.locale, d.numbering_system, d.symbols_system),
            (locale, nu, symbols_system),
            "{tag}"
        );
    }
    let d = number_data("en-u-nu-arab").expect("data");
    assert_eq!((d.decimal, d.digits), (".", Some("٠١٢٣٤٥٦٧٨٩")));
    let d = number_data("zh-u-nu-hanidec").expect("data");
    assert_eq!(d.digits, Some("〇一二三四五六七八九"));
}

/// Every locale, every style, and every numbering system through `-u-nu-`:
/// the entries encode and the client views read them back.
#[test]
fn every_locale_encodes() {
    let locales = number_locales().expect("table");
    assert_eq!(locales.len(), 766);
    let mut needs = NumberNeeds::from_functions(["percent", "currency"]);
    assert!(needs.percent && needs.currency == Some(CurrencyNeeds::ALL));
    needs.symbols = true;
    // Every pattern style, no currency codes (their data: tests/measure.rs).
    let mut currency = CurrencyNeeds::ALL;
    currency.codes = mf2_locale_data::Selection::default();
    needs.currency = Some(currency);
    let mut systems = std::collections::BTreeSet::new();
    for locale in &locales {
        for tag in [(*locale).to_owned(), format!("{locale}-u-nu-native")] {
            let entries = number_locale_entries(&tag, &needs).expect("entries");
            let [(k1, symbols), (k2, patterns), (k3, _)] = entries.as_slice() else {
                panic!("{tag}: three entries");
            };
            assert_eq!(*k3, locale_key::CURRENCY_DATA);
            assert_eq!(
                (*k1, *k2),
                (locale_key::NUMBER_SYMBOLS, locale_key::NUMBER_PATTERNS)
            );
            let s = Symbols::parse(symbols).unwrap_or_else(|| panic!("{tag}: symbols"));
            assert!(!s.decimal().is_empty() && !s.minus().is_empty(), "{tag}");
            let p = Patterns::new(patterns);
            assert!(p.is_valid(), "{tag}: patterns");
            for style in Style::ALL {
                assert!(p.resolve(style).is_some(), "{tag}: {style:?}");
            }
            systems.insert(number_data(&tag).expect("data").numbering_system);
        }
    }
    assert!(systems.len() >= 20, "{systems:?}");
    // Every numeric system is reachable with `-u-nu-` and encodes its digits.
    for nu in systems {
        let d = number_data(&format!("en-u-nu-{nu}")).expect("data");
        assert_eq!(d.numbering_system, nu);
        let bytes = d.symbols_entry().expect("symbols");
        let s = Symbols::parse(&bytes).expect("parses");
        assert_eq!(s.digits().is_ascii(), nu == "latn", "{nu}");
    }
}

fn empty_manifest() -> Manifest {
    Manifest {
        ids: Vec::new(),
        slots: Vec::new(),
        markup: Vec::new(),
        functions: Vec::new(),
    }
}

/// The LOCALE section of a message-less catalog carrying `entries`, as the
/// writer frames it; the catalog must load.
fn locale_section(locale: &str, entries: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    let manifest = empty_manifest();
    let mut options = Options::new(locale, Dir::Ltr).stripped();
    options.cldr_version = Some(mf2_locale_data::CLDR_VERSION);
    options.locale_entries = entries;
    let bytes = catalog(&manifest, &[], &options).expect("writes");
    let cat = Catalog::new(bytes, manifest.hash()).expect("loads");
    let (_, off, len) = cat
        .sections()
        .find(|(k, _, _)| *k == mf2_catalog::format::section::LOCALE)
        .expect("LOCALE");
    let (off, len) = (off as usize, len as usize);
    cat.as_bytes()[off..off + len].to_vec()
}

#[test]
fn catalog_round_trip() {
    let mut needs = LocaleNeeds::default();
    needs.cardinal = true;
    needs.numbers.percent = true;
    let entries = locale_entries("pl", &needs).expect("entries");
    assert_eq!(
        entries.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        [1, locale_key::NUMBER_SYMBOLS, locale_key::NUMBER_PATTERNS]
    );
    let manifest = empty_manifest();
    let mut options = Options::new("pl", Dir::Ltr).stripped();
    options.locale_entries = entries;
    let cat = Catalog::new(
        catalog(&manifest, &[], &options).expect("writes"),
        manifest.hash(),
    )
    .expect("loads");
    let s = Symbols::of(&cat).expect("symbols");
    assert_eq!((s.decimal(), s.group()), (",", "\u{a0}"));
    let p = Patterns::of(&cat).expect("patterns");
    assert!(p.get(Style::Percent).is_some());
    // A malformed payload under a known key is refused by the writer.
    let mut bad = Options::new("pl", Dir::Ltr);
    bad.locale_entries = vec![(locale_key::NUMBER_SYMBOLS, vec![0x33])];
    assert!(catalog(&manifest, &[], &bad).is_err());
}

fn gzip9(b: &[u8]) -> usize {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(b).expect("gzip");
    e.finish().expect("gzip").len()
}

/// B8: plural entries + `number.symbols` (+ the percent pattern when
/// `:percent` is used) per panel locale ≤ 0.5 KB gz (512 B), measured as the
/// standalone gzip -9 of the LOCALE section's bytes.
#[test]
fn b8() {
    const PANEL: [&str; 11] = [
        "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
    ];
    const LIMIT: usize = 512;
    println!("B8  locale  plural raw/gz  +symbols raw/gz  +percent raw/gz");
    let out = std::env::var_os("MF2_B8_OUT").map(std::path::PathBuf::from);
    for tag in PANEL {
        let mut needs = LocaleNeeds::default();
        needs.cardinal = true;
        needs.ordinal = true;
        let plural = locale_section(tag, locale_entries(tag, &needs).expect("plural"));
        needs.numbers.symbols = true;
        let symbols = locale_section(tag, locale_entries(tag, &needs).expect("symbols"));
        needs.numbers.percent = true;
        let percent = locale_section(tag, locale_entries(tag, &needs).expect("percent"));
        let (g0, g1, g2) = (gzip9(&plural), gzip9(&symbols), gzip9(&percent));
        println!(
            "B8  {tag:6}  {:4} / {g0:3}     {:4} / {g1:3}       {:4} / {g2:3}",
            plural.len(),
            symbols.len(),
            percent.len()
        );
        if let Some(dir) = &out {
            std::fs::create_dir_all(dir).expect("dir");
            std::fs::write(dir.join(format!("{tag}.percent.bin")), &percent).expect("write");
            std::fs::write(dir.join(format!("{tag}.symbols.bin")), &symbols).expect("write");
        }
        assert!(g1 <= LIMIT && g2 <= LIMIT, "{tag}: B8 {g2} B gz > {LIMIT}");
    }
}
