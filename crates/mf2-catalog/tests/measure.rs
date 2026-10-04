//! `currency.data` and `unit.data`, v1: byte-exact encodings of hand-written input,
//! the views' reading of them (lookups, CLDR's fallbacks, the omitted forms), the writer's
//! refusals, and the views' robustness — every truncation and single-byte
//! mutation is read without a panic.

use mf2_catalog::currency::Currencies;
use mf2_catalog::number::{AffixPart, Grouping, Style, TemplatePart};
use mf2_catalog::unit::{Units, Width};
use mf2_catalog::writer::currency::{CurrenciesSpec, CurrencySpec, currencies};
use mf2_catalog::writer::number::PatternSpec;
use mf2_catalog::writer::unit::{UnitSpec, UnitWidthSpec, UnitsSpec, units};

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn unhex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex"))
        .collect()
}

use TemplatePart::{Arg0, Arg1, Text};

fn usd_jpy(names: bool, narrow: bool) -> CurrenciesSpec<'static> {
    CurrenciesSpec {
        default_digits: 2,
        narrow,
        names,
        name_patterns: vec![(5, vec![Arg0, Text(" "), Arg1])],
        currencies: vec![
            CurrencySpec {
                code: "USD",
                digits: 2,
                rounding: 0,
                symbol: "$",
                narrow: "$",
                name: "US Dollar",
                names: vec![(1, "US dollar"), (5, "US dollars")],
                edges: 0,
                pattern: None,
                decimal: None,
                group: None,
            },
            CurrencySpec {
                code: "JPY",
                digits: 0,
                rounding: 0,
                symbol: "¥",
                narrow: "¥",
                name: "Japanese Yen",
                names: vec![(1, "Japanese yen"), (5, "Japanese yen")],
                edges: 0,
                pattern: None,
                decimal: None,
                group: None,
            },
        ],
    }
}

/// §4.8: `en`'s JPY and USD with names, in both flag settings.
#[test]
fn currency_vectors() {
    let bytes = currencies(&usd_jpy(true, false)).expect("encodes");
    assert_eq!(
        hex(&bytes),
        "02 02 01 05 03 01 20 02 02 00 4A 50 59 00 00 00 00 55 53 44 21 00 00 00 \
         A0 00 02 C2 A5 0C 4A 61 70 61 6E 65 73 65 20 59 65 6E 01 05 0C 4A 61 70 61 6E 65 \
         73 65 20 79 65 6E \
         A2 00 01 24 09 55 53 20 44 6F 6C 6C 61 72 02 01 09 55 53 20 64 6F 6C 6C 61 72 05 \
         0A 55 53 20 64 6F 6C 6C 61 72 73"
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    );
    let c = Currencies::parse(&bytes).expect("parses");
    assert!(c.is_valid() && c.names_carried() && !c.narrow_carried());
    assert_eq!(c.len(), 2);
    assert_eq!(c.default_digits(), 2);
    let tpl: Vec<_> = c.name_pattern(1).expect("other").parts().collect();
    assert_eq!(tpl, [Arg0, Text(" "), Arg1]);
    let usd = c.get(*b"USD").expect("USD");
    assert_eq!(
        (usd.code(), usd.symbol(), usd.fraction_digits()),
        ("USD", "$", 2)
    );
    assert_eq!(usd.name(), Some("US Dollar"));
    assert_eq!(usd.name_for(1), Some("US dollar"));
    assert_eq!(usd.name_for(3), Some("US dollars"), "few → other");
    assert_eq!(usd.narrow_symbol(), None, "not carried");
    let jpy = c.get(*b"JPY").expect("JPY");
    assert_eq!(jpy.fraction_digits(), 0);
    // JPY's `one` equals its `other`: dropped, the fallback gives it back.
    assert_eq!(jpy.name_for(1), Some("Japanese yen"));
    assert!(c.get(*b"EUR").is_none());
    assert_eq!(c.codes().collect::<Vec<_>>(), [*b"JPY", *b"USD"]);

    // Symbols only: 2 × 7 index bytes and 3–4 bytes a record.
    let lean = currencies(&usd_jpy(false, false)).expect("encodes");
    assert_eq!(
        hex(&lean),
        "00 02 02 00 4A 50 59 00 00 00 00 55 53 44 05 00 00 00 20 00 02 C2 A5 22 00 01 24"
    );
    let c = Currencies::parse(&lean).expect("parses");
    assert!(c.is_valid());
    assert_eq!(c.get(*b"USD").expect("USD").name(), None);
}

#[test]
fn currency_overrides_and_refusals() {
    let mut spec = usd_jpy(true, true);
    spec.currencies.push(CurrencySpec {
        code: "CVE",
        digits: 2,
        rounding: 5,
        symbol: "\u{200b}",
        narrow: "Esc",
        name: "CVE",
        names: vec![],
        edges: 0b0100,
        pattern: Some(PatternSpec {
            grouping: Grouping {
                primary: 3,
                secondary: 3,
            },
            positive: (vec![AffixPart::Currency], vec![]),
            negative: None,
        }),
        decimal: Some("$"),
        group: Some(" "),
    });
    let bytes = currencies(&spec).expect("encodes");
    let c = Currencies::parse(&bytes).expect("parses");
    assert!(c.is_valid());
    let cve = c.get(*b"CVE").expect("CVE");
    assert_eq!(
        (cve.decimal(), cve.group(), cve.rounding_increment()),
        (Some("$"), Some(" "), 5)
    );
    assert_eq!(cve.narrow_symbol(), Some("Esc"));
    assert!(cve.narrow_edges().first && !cve.symbol_edges().first);
    assert_eq!(cve.name(), Some("CVE"), "the code, not stored");
    let p = cve.pattern().expect("pattern");
    assert!(p.positive().prefix.has_currency());
    assert!(c.get(*b"USD").expect("USD").pattern().is_none());
    assert_eq!(
        c.get(*b"USD").expect("USD").narrow_symbol(),
        Some("$"),
        "narrow = symbol"
    );
    // Order does not matter; duplicates and bad codes are refused.
    let mut rev = spec.clone();
    rev.currencies.reverse();
    assert_eq!(currencies(&rev).expect("encodes"), bytes);
    let mut dup = spec.clone();
    dup.currencies.push(dup.currencies[0].clone());
    assert!(currencies(&dup).is_err());
    let mut bad = spec.clone();
    bad.currencies[0].code = "usd";
    assert!(currencies(&bad).is_err());
    let mut ctl = spec;
    ctl.currencies[0].symbol = "\u{1}";
    assert!(currencies(&ctl).is_err());
}

fn km(narrow_same: bool) -> UnitSpec<'static> {
    let short = UnitWidthSpec {
        name: Some("km"),
        per_unit: Some(vec![Arg0, Text("/km")]),
        patterns: vec![(5, vec![Arg0, Text(" km")]), (1, vec![Arg0, Text(" km")])],
    };
    let narrow = if narrow_same {
        short.clone()
    } else {
        UnitWidthSpec {
            name: Some("km"),
            per_unit: Some(vec![Arg0, Text("/km")]),
            patterns: vec![(5, vec![Arg0, Text("km")])],
        }
    };
    UnitSpec {
        id: "kilometer",
        widths: vec![
            UnitWidthSpec {
                name: Some("kilometers"),
                per_unit: Some(vec![Arg0, Text(" per kilometer")]),
                patterns: vec![
                    (1, vec![Arg0, Text(" kilometer")]),
                    (5, vec![Arg0, Text(" kilometers")]),
                ],
            },
            short,
            narrow,
        ],
    }
}

fn hour() -> UnitSpec<'static> {
    let one = |s: &'static str| UnitWidthSpec {
        name: None,
        per_unit: None,
        patterns: vec![(5, vec![Arg0, Text(s)])],
    };
    UnitSpec {
        id: "hour",
        widths: vec![one(" hours"), one(" hr"), one("h")],
    }
}

fn en_units(names: bool) -> UnitsSpec<'static> {
    UnitsSpec {
        widths: vec![Width::Long, Width::Short, Width::Narrow],
        names,
        per: vec![
            vec![Arg0, Text(" per "), Arg1],
            vec![Arg0, Text("/"), Arg1],
            vec![Arg0, Text("/"), Arg1],
        ],
        units: vec![km(true), hour()],
    }
}

/// §4.8: two `en` units, all widths.
#[test]
fn unit_vectors() {
    let bytes = units(&en_units(false)).expect("encodes");
    assert_eq!(
        hex(&bytes),
        "07 07 01 20 70 65 72 20 02 03 01 2F 02 03 01 2F 02 02 00 00 00 00 00 1E 00 00 00 \
         04 68 6F 75 72 00 01 05 07 01 20 68 6F 75 72 73 00 01 05 04 01 20 68 72 00 01 05 \
         02 01 68 \
         09 6B 69 6C 6F 6D 65 74 65 72 04 0F 01 20 70 65 72 20 6B 69 6C 6F 6D 65 74 65 72 \
         02 01 0B 01 20 6B 69 6C 6F 6D 65 74 65 72 05 0C 01 20 6B 69 6C 6F 6D 65 74 65 72 \
         73 04 04 01 2F 6B 6D 01 05 04 01 20 6B 6D 01"
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    );
    let u = Units::parse(&bytes).expect("parses");
    assert!(u.is_valid() && !u.names_carried());
    assert_eq!(u.ids().collect::<Vec<_>>(), ["hour", "kilometer"]);
    let per: Vec<_> = u.per_pattern(Width::Long).expect("per").parts().collect();
    assert_eq!(per, [Arg0, Text(" per "), Arg1]);
    let k = u.get("kilometer").expect("kilometer");
    let p = |w, c| k.pattern(w, c).map(|t| t.encoded());
    assert_eq!(p(Width::Long, 1), Some("\u{1} kilometer"));
    assert_eq!(p(Width::Long, 3), Some("\u{1} kilometers"), "few → other");
    assert_eq!(p(Width::Short, 1), Some("\u{1} km"), "one = other: dropped");
    assert_eq!(p(Width::Narrow, 5), Some("\u{1} km"), "same as short");
    assert_eq!(
        k.per_unit_pattern(Width::Long).map(|t| t.encoded()),
        Some("\u{1} per kilometer")
    );
    assert_eq!(k.display_name(Width::Long), None, "names not carried");
    assert!(u.get("meter").is_none() && u.get("").is_none());
    let h = u.get("hour").expect("hour");
    assert_eq!(
        h.pattern(Width::Narrow, 1).map(|t| t.encoded()),
        Some("\u{1}h")
    );
    assert!(h.per_unit_pattern(Width::Short).is_none());

    let named = units(&en_units(true)).expect("encodes");
    let u = Units::parse(&named).expect("parses");
    assert!(u.is_valid());
    assert_eq!(
        u.get("kilometer").expect("km").display_name(Width::Long),
        Some("kilometers")
    );
    // One width only.
    let mut short = en_units(false);
    short.widths = vec![Width::Short];
    short.per = vec![vec![Arg0, Text("/"), Arg1]];
    for unit in &mut short.units {
        unit.widths = vec![unit.widths[1].clone()];
    }
    let u2 = units(&short).expect("encodes");
    let u = Units::parse(&u2).expect("parses");
    assert!(u.is_valid() && u.has_width(Width::Short) && !u.has_width(Width::Long));
    assert!(
        u.get("kilometer")
            .expect("km")
            .pattern(Width::Long, 5)
            .is_none()
    );
    // Refusals: no `other`, duplicate id, a width count that does not match.
    let mut no_other = en_units(false);
    no_other.units[1].widths[0].patterns = vec![(1, vec![Arg0])];
    assert!(units(&no_other).is_err());
    let mut dup = en_units(false);
    dup.units.push(hour());
    assert!(units(&dup).is_err());
    let mut short_block = en_units(false);
    short_block.units[0].widths.pop();
    assert!(units(&short_block).is_err());
}

fn touch(b: &[u8]) -> usize {
    let mut n = 0;
    if let Some(c) = Currencies::parse(b) {
        n += usize::from(c.is_valid()) + c.codes().count() + c.len();
        n += c.name_pattern(3).map_or(0, |t| t.parts().count());
        for code in [*b"CVE", *b"JPY", *b"USD", *b"ZZZ"] {
            if let Some(x) = c.get(code) {
                n += x.symbol().len() + x.narrow_symbol().map_or(0, str::len);
                n += x.name_for(1).map_or(0, str::len) + usize::from(x.fraction_digits());
                n += x.pattern().map_or(0, |p| usize::from(p.grouping().primary));
                n += x.decimal().map_or(0, str::len) + usize::from(x.symbol_edges().last);
            }
        }
    }
    if let Some(u) = Units::parse(b) {
        n += usize::from(u.is_valid()) + u.ids().count();
        for id in ["hour", "kilometer", "x"] {
            if let Some(x) = u.get(id) {
                for w in Width::ALL {
                    n += x.pattern(w, 1).map_or(0, |t| t.parts().count());
                    n += x.per_unit_pattern(w).map_or(0, |t| t.encoded().len());
                    n += x.display_name(w).map_or(0, str::len);
                }
            }
        }
        for w in Width::ALL {
            n += u.per_pattern(w).map_or(0, |t| t.parts().count());
        }
    }
    n
}

#[test]
fn views_never_panic() {
    let mut vectors = vec![
        currencies(&usd_jpy(true, true)).expect("c"),
        units(&en_units(true)).expect("u"),
    ];
    vectors.push(unhex("00 02 01 00 55 53 44 00 00 00 00 22 00 01 24"));
    let mut sum = 0;
    for v in &vectors {
        for len in 0..=v.len() {
            sum += touch(&v[..len]);
        }
        for at in 0..v.len() {
            let mut m = v.clone();
            for x in [0u8, 1, 2, 3, 0x7f, 0x80, 0xff] {
                m[at] = x;
                sum += touch(&m);
            }
        }
    }
    assert!(sum > 0);
}

#[test]
fn styles_are_unchanged() {
    // The pattern styles and the currency pattern share one body encoding.
    assert_eq!(Style::Currency as u8, 2);
}
