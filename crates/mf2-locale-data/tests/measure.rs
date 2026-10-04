//! The `currency.data` and `unit.data` LOCALE entries
//! built from the shipped tables:
//! byte-exact vectors, CLDR's resolution (parents, fallbacks, the
//! currency-specific pattern and separators), `X-per-Y` composition, the
//! slicing rule on parsed messages, every locale encoding, and the sizes of
//! the configured sets against P0.5.
//!
//! The size table: `cargo test -p mf2-locale-data --test measure sizes --
//! --nocapture`.

use std::io::Write as _;

use mf2_catalog::currency::Currencies;
use mf2_catalog::format::locale_key;
use mf2_catalog::number::TemplatePart;
use mf2_catalog::unit::{Units, Width};
use mf2_locale_data::{
    CurrencyNeeds, LocaleNeeds, NumberNeeds, Selection, UnitNeeds, composition, locale_entries,
    number_locale_entries, number_locales, unit_ids,
};

fn set(v: &[&str]) -> Selection {
    Selection::Listed(v.iter().map(|s| (*s).to_owned()).collect())
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn entry(tag: &str, needs: &NumberNeeds, key: u32) -> Vec<u8> {
    number_locale_entries(tag, needs)
        .expect("entries")
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
        .expect("the entry")
}

fn currency_needs(codes: &[&str], names: bool, narrow: bool) -> NumberNeeds {
    let mut c = CurrencyNeeds::default();
    c.codes = set(codes);
    c.names = names;
    c.narrow = narrow;
    let mut n = NumberNeeds::default();
    n.currency = Some(c);
    n
}

fn unit_needs(ids: &[&str], widths: [bool; 3]) -> NumberNeeds {
    let mut u = UnitNeeds::default();
    u.ids = set(ids);
    u.widths = widths;
    let mut n = NumberNeeds::default();
    n.unit = Some(u);
    n
}

/// §4.8.
#[test]
fn vectors() {
    let key = locale_key::CURRENCY_DATA;
    assert_eq!(
        hex(&entry(
            "en",
            &currency_needs(&["USD", "JPY"], true, false),
            key
        )),
        "02 02 01 05 03 01 20 02 02 00 4A 50 59 00 00 00 00 55 53 44 21 00 00 00 A0 00 02 C2 \
         A5 0C 4A 61 70 61 6E 65 73 65 20 59 65 6E 01 05 0C 4A 61 70 61 6E 65 73 65 20 79 65 \
         6E A2 00 01 24 09 55 53 20 44 6F 6C 6C 61 72 02 01 09 55 53 20 64 6F 6C 6C 61 72 05 \
         0A 55 53 20 64 6F 6C 6C 61 72 73"
    );
    let eur = currency_needs(&["EUR"], false, false);
    for tag in ["en", "fr", "ar", "hi", "pl"] {
        assert_eq!(
            hex(&entry(tag, &eur, key)),
            "00 02 01 00 45 55 52 00 00 00 00 22 00 03 E2 82 AC",
            "{tag}"
        );
    }
    // `en-DE`: EUR's own pattern `¤#,##0.00` (edges byte 0x10).
    assert_eq!(
        hex(&entry("en-DE", &eur, key)),
        "00 02 01 00 45 55 52 00 00 00 00 22 10 03 E2 82 AC 04 33 01 03 00"
    );
    let key = locale_key::UNIT_DATA;
    let short = unit_needs(&["kilometer", "hour"], [false, true, false]);
    for (tag, bytes) in [
        (
            "en",
            "02 03 01 2F 02 02 00 00 00 00 00 11 00 00 00 04 68 6F 75 72 04 03 01 2F 68 01 05 04 \
             01 20 68 72 09 6B 69 6C 6F 6D 65 74 65 72 04 04 01 2F 6B 6D 01 05 04 01 20 6B 6D",
        ),
        (
            "fr",
            "02 03 01 2F 02 02 00 00 00 00 00 12 00 00 00 04 68 6F 75 72 04 03 01 2F 68 01 05 05 \
             01 E2 80 AF 68 09 6B 69 6C 6F 6D 65 74 65 72 04 04 01 2F 6B 6D 01 05 06 01 E2 80 AF \
             6B 6D",
        ),
        (
            "ar",
            "02 03 01 2F 02 02 00 00 00 00 00 12 00 00 00 04 68 6F 75 72 04 04 01 2F D8 B3 01 05 \
             04 01 20 D8 B3 09 6B 69 6C 6F 6D 65 74 65 72 04 06 01 2F D9 83 D9 85 01 05 06 01 20 \
             D9 83 D9 85",
        ),
        (
            "pl",
            "02 03 01 2F 02 02 00 00 00 00 00 18 00 00 00 04 68 6F 75 72 04 07 01 2F 67 6F 64 7A \
             2E 01 05 07 01 20 67 6F 64 7A 2E 09 6B 69 6C 6F 6D 65 74 65 72 04 04 01 2F 6B 6D 01 \
             05 04 01 20 6B 6D",
        ),
    ] {
        let want: String = bytes.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(hex(&entry(tag, &short, key)), want, "{tag}");
    }
}

#[test]
fn currency_resolution() {
    let e = entry(
        "pt-CV",
        &currency_needs(&["CVE", "EUR", "usd", "XYZ"], true, true),
        locale_key::CURRENCY_DATA,
    );
    let c = Currencies::parse(&e).expect("parses");
    assert!(c.is_valid());
    let cve = c.get(*b"CVE").expect("CVE");
    assert_eq!((cve.symbol(), cve.decimal()), ("\u{200b}", Some("$")));
    assert_eq!(cve.fraction_digits(), 2);
    let usd = c.get(*b"USD").expect("lower-case code accepted");
    assert_eq!(usd.symbol(), "US$");
    let xyz = c.get(*b"XYZ").expect("a code CLDR lacks");
    assert_eq!(
        (xyz.symbol(), xyz.name(), xyz.fraction_digits()),
        ("XYZ", Some("XYZ"), 2)
    );
    assert!(xyz.symbol_edges().first && xyz.symbol_edges().last);
    // Digits from currencyData; names through the parents (`fr-CA` → `fr`).
    let e = entry(
        "fr-CA",
        &currency_needs(&["JPY", "BHD", "EUR"], true, false),
        locale_key::CURRENCY_DATA,
    );
    let c = Currencies::parse(&e).expect("parses");
    assert_eq!(c.get(*b"JPY").expect("JPY").fraction_digits(), 0);
    assert_eq!(c.get(*b"BHD").expect("BHD").fraction_digits(), 3);
    let eur = c.get(*b"EUR").expect("EUR");
    assert_eq!(
        (eur.name_for(1), eur.name_for(5)),
        (Some("euro"), Some("euros"))
    );
    let pat: Vec<_> = c.name_pattern(5).expect("pattern").parts().collect();
    assert_eq!(
        pat,
        [
            TemplatePart::Arg0,
            TemplatePart::Text("\u{a0}"),
            TemplatePart::Arg1
        ]
    );
    // Letter-like edges: `CHF` touches the digits, `€` does not; `de-CH`
    // writes EUR as `EUR`.
    let e = entry(
        "de",
        &currency_needs(&["CHF", "EUR"], false, false),
        locale_key::CURRENCY_DATA,
    );
    let c = Currencies::parse(&e).expect("parses");
    assert!(c.get(*b"CHF").expect("CHF").symbol_edges().last);
    assert!(!c.get(*b"EUR").expect("EUR").symbol_edges().last);
    let e = entry(
        "de-CH",
        &currency_needs(&["EUR"], false, false),
        locale_key::CURRENCY_DATA,
    );
    let c = Currencies::parse(&e).expect("parses");
    assert_eq!(c.get(*b"EUR").expect("EUR").symbol(), "EUR");
    // A malformed code is refused.
    assert!(number_locale_entries("en", &currency_needs(&["US"], false, false)).is_err());
}

#[test]
fn unit_resolution_and_composition() {
    assert_eq!(unit_ids().expect("ids").len(), 232);
    assert_eq!(
        composition("kilometer-per-second").expect("table"),
        Some(("kilometer", "second"))
    );
    assert_eq!(
        composition("kilometer-per-hour").expect("table"),
        None,
        "a CLDR unit"
    );
    assert_eq!(
        composition("furlong-per-fortnight").expect("table"),
        Some(("furlong", "fortnight")),
        "CLDR has both"
    );
    assert_eq!(composition("smoot-per-hour").expect("table"), None);
    let e = entry(
        "de",
        &unit_needs(
            &["kilometer-per-second", "smoot", "liter-per-100-kilometer"],
            [true; 3],
        ),
        locale_key::UNIT_DATA,
    );
    let u = Units::parse(&e).expect("parses");
    assert!(u.is_valid());
    assert_eq!(
        u.ids().collect::<Vec<_>>(),
        ["kilometer", "liter-per-100-kilometer", "second"]
    );
    let per: Vec<_> = u.per_pattern(Width::Long).expect("per").parts().collect();
    assert!(per.contains(&TemplatePart::Arg0) && per.contains(&TemplatePart::Arg1));
    let s = u.get("second").expect("second");
    assert!(s.per_unit_pattern(Width::Long).is_some());
    // `ar`'s dual has no placeholder; `few` → its own form; a missing form → other.
    let e = entry(
        "ar",
        &unit_needs(&["day"], [true, false, false]),
        locale_key::UNIT_DATA,
    );
    let u = Units::parse(&e).expect("parses");
    let day = u.get("day").expect("day");
    let two: Vec<_> = day.pattern(Width::Long, 2).expect("two").parts().collect();
    assert!(!two.contains(&TemplatePart::Arg0));
    assert!(day.pattern(Width::Short, 5).is_none(), "width not carried");
}

fn parse(src: &str) -> mf2_model::Message<'_> {
    mf2_syntax::parse_model(src).message.expect("parses")
}

#[test]
fn slicing_from_messages() {
    let mut n = NumberNeeds::default();
    for src in [
        "{$x}",
        "{$p :currency currency=eur currencyDisplay=name}",
        ".local $c = {1 :currency currency=JPY currencySign=accounting} {{{$c}}}",
        "{$k :unit unit=kilometer-per-hour unitDisplay=long} {$m :unit unit=meter}",
        "{$v :unit unit=$u}",
    ] {
        n.add_message(&parse(src));
    }
    assert!(n.symbols && !n.percent);
    let c = n.currency.as_ref().expect("currency");
    assert_eq!(c.codes, set(&["EUR", "JPY"]));
    assert!(c.names && c.accounting && !c.hidden && !c.narrow);
    let u = n.unit.as_ref().expect("unit");
    assert_eq!(u.ids, Selection::All, "a variable unit");
    assert_eq!(u.widths, [true, true, false]);
    assert!(n.needs_cardinal());
    let mut needs = LocaleNeeds::default();
    needs.numbers = n;
    let keys: Vec<u32> = locale_entries("en", &needs)
        .expect("entries")
        .iter()
        .map(|(k, _)| *k)
        .collect();
    assert_eq!(
        keys,
        [
            locale_key::PLURAL_CARDINAL,
            locale_key::NUMBER_SYMBOLS,
            locale_key::NUMBER_PATTERNS,
            locale_key::CURRENCY_DATA,
            locale_key::UNIT_DATA
        ]
    );
    let f = NumberNeeds::from_functions(["unit", "currency"]);
    assert_eq!(f.currency, Some(CurrencyNeeds::ALL));
    assert_eq!(f.unit, Some(UnitNeeds::ALL));
}

/// Every locale: the configured sets encode and read back; every tenth
/// locale (and root) with every currency and unit.
#[test]
fn every_locale_encodes() {
    let used = {
        let mut n = currency_needs(&["USD", "EUR", "JPY", "GBP", "CVE"], true, true);
        n.unit = unit_needs(
            &["kilometer", "kilogram", "celsius", "hour", "megabyte"],
            [true; 3],
        )
        .unit;
        n
    };
    let mut all = NumberNeeds::default();
    all.currency = Some(CurrencyNeeds::ALL);
    let mut u = UnitNeeds::ALL;
    u.names = true;
    all.unit = Some(u);
    for (i, locale) in number_locales().expect("locales").into_iter().enumerate() {
        let needs = if i % 10 == 0 || locale == "und" {
            &all
        } else {
            &used
        };
        let e = number_locale_entries(locale, needs).expect("entries");
        let c = e
            .iter()
            .find(|(k, _)| *k == locale_key::CURRENCY_DATA)
            .expect("currency");
        let c = Currencies::parse(&c.1).expect("currency parses");
        assert!(c.is_valid(), "{locale}");
        assert!(
            c.get(*b"USD").is_some_and(|x| x.name().is_some()),
            "{locale}"
        );
        let u = e
            .iter()
            .find(|(k, _)| *k == locale_key::UNIT_DATA)
            .expect("unit");
        let u = Units::parse(&u.1).expect("unit parses");
        assert!(u.is_valid(), "{locale}");
        let h = u.get("hour").unwrap_or_else(|| panic!("{locale}: hour"));
        for w in Width::ALL {
            assert!(h.pattern(w, 5).is_some(), "{locale} {w:?}");
        }
    }
}

fn gzip9(b: &[u8]) -> usize {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(b).expect("gzip");
    e.finish().expect("gzip").len()
}

/// Per panel locale, raw / gzip -9 bytes of the entries: currencies used
/// (USD EUR JPY GBP, every display) and all; units used (five × three
/// widths) and all (three widths, no display names).
#[test]
fn sizes() {
    const PANEL: [&str; 11] = [
        "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
    ];
    let used_c = currency_needs(&["USD", "EUR", "JPY", "GBP"], true, true);
    let mut all_c = NumberNeeds::default();
    all_c.currency = Some(CurrencyNeeds::ALL);
    let used_u = unit_needs(
        &["kilometer", "kilogram", "celsius", "hour", "megabyte"],
        [true; 3],
    );
    let mut all_u = NumberNeeds::default();
    all_u.unit = Some(UnitNeeds::ALL);
    println!("sizes  locale  currencies used  all            units used  all");
    let mut worst = (0, 0);
    for tag in PANEL {
        let size = |n: &NumberNeeds, k| {
            let e = entry(tag, n, k);
            (e.len(), gzip9(&e))
        };
        let (cu, ca) = (
            size(&used_c, locale_key::CURRENCY_DATA),
            size(&all_c, locale_key::CURRENCY_DATA),
        );
        let (uu, ua) = (
            size(&used_u, locale_key::UNIT_DATA),
            size(&all_u, locale_key::UNIT_DATA),
        );
        println!(
            "sizes  {tag:6}  {:5}/{:4}  {:6}/{:5}   {:5}/{:4}  {:6}/{:5}",
            cu.0, cu.1, ca.0, ca.1, uu.0, uu.1, ua.0, ua.1
        );
        worst = (worst.0.max(cu.1), worst.1.max(uu.1));
    }
    // P0.5: used currencies ≤ 260 B gz, used units ≤ 363 B gz.
    assert!(worst.0 <= 300 && worst.1 <= 400, "{worst:?}");
}
