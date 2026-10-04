//! The number LOCALE entries, v1:
//! the §4.5 vectors from hand-written input through the encoders, back
//! through the client views, and the views' robustness — every truncation
//! and every single-byte mutation of the vectors is read without a panic.

use mf2_catalog::number::{AffixPart, Grouping, Patterns, SignShown, Style, Symbols};
use mf2_catalog::writer::number::{PatternSpec, SymbolsSpec, patterns, symbols};

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex"))
        .collect()
}

const WEST: Grouping = Grouping {
    primary: 3,
    secondary: 3,
};

fn spec<'a>(
    decimal: &'a str,
    group: &'a str,
    minus: &'a str,
    digits: Option<&'a str>,
) -> SymbolsSpec<'a> {
    SymbolsSpec {
        grouping: WEST,
        minimum_grouping_digits: 1,
        decimal,
        group,
        minus,
        plus: "+",
        percent: "%",
        digits,
    }
}

#[test]
fn symbols_vectors() {
    let en = spec(".", ",", "-", None);
    assert_eq!(
        symbols(&en).expect("en"),
        hex("33 01 01 2E 01 2C 01 2D 01 2B 01 25")
    );
    let fr = spec(",", "\u{202f}", "-", None);
    assert_eq!(
        symbols(&fr).expect("fr"),
        hex("33 01 01 2C 03 E2 80 AF 01 2D 01 2B 01 25")
    );
    let arab = SymbolsSpec {
        plus: "\u{61c}+",
        percent: "٪\u{61c}",
        ..spec("٫", "٬", "\u{61c}-", Some("٠١٢٣٤٥٦٧٨٩"))
    };
    let bytes = hex(
        "33 01 02 D9 AB 02 D9 AC 03 D8 9C 2D 03 D8 9C 2B 04 D9 AA D8 9C \
         D9 A0 D9 A1 D9 A2 D9 A3 D9 A4 D9 A5 D9 A6 D9 A7 D9 A8 D9 A9",
    );
    assert_eq!(symbols(&arab).expect("ar-u-nu-arab"), bytes);
    let s = Symbols::parse(&bytes).expect("parses");
    assert_eq!(
        (s.decimal(), s.group(), s.minus(), s.plus(), s.percent()),
        ("٫", "٬", "\u{61c}-", "\u{61c}+", "٪\u{61c}")
    );
    let digits: Vec<&str> = (0..10).map(|d| s.digits().digit(d)).collect();
    assert_eq!(digits.concat(), "٠١٢٣٤٥٦٧٨٩");
    let hi = SymbolsSpec {
        grouping: Grouping {
            primary: 3,
            secondary: 2,
        },
        ..en
    };
    assert_eq!(
        symbols(&hi).expect("hi"),
        hex("23 01 01 2E 01 2C 01 2D 01 2B 01 25")
    );
    let pl = SymbolsSpec {
        minimum_grouping_digits: 2,
        ..spec(",", "\u{a0}", "-", None)
    };
    assert_eq!(
        symbols(&pl).expect("pl"),
        hex("33 02 01 2C 02 C2 A0 01 2D 01 2B 01 25")
    );
}

fn text(s: &str) -> AffixPart<'_> {
    AffixPart::Text(s)
}

#[test]
fn patterns_vectors() {
    use AffixPart::{Currency, Percent, Sign};
    let p = |prefix: Vec<AffixPart<'static>>, suffix: Vec<AffixPart<'static>>| PatternSpec {
        grouping: WEST,
        positive: (prefix, suffix),
        negative: None,
    };
    let fr = patterns(&[(Style::Percent, p(vec![], vec![text("\u{a0}"), Percent]))]).expect("fr");
    assert_eq!(fr, hex("01 06 33 00 03 C2 A0 02"));
    // en, every style: `¤#,##0.00`, `¤ #,##0.00`, `#,##0.00`,
    // `¤#,##0.00;(¤#,##0.00)`, `¤ #,##0.00;(¤ #,##0.00)`, `#,##0.00;(#,##0.00)`.
    let paren = |pre: Vec<AffixPart<'static>>| {
        let mut neg = vec![text("(")];
        neg.extend(pre.iter().copied());
        PatternSpec {
            grouping: WEST,
            positive: (pre, vec![]),
            negative: Some((neg, vec![text(")")])),
        }
    };
    let en = patterns(&[
        (Style::Percent, p(vec![], vec![Percent])),
        (Style::Currency, p(vec![Currency], vec![])),
        (
            Style::CurrencyAlpha,
            p(vec![Currency, text("\u{a0}")], vec![]),
        ),
        (Style::CurrencyNoSymbol, p(vec![], vec![])),
        (Style::Accounting, paren(vec![Currency])),
        (
            Style::AccountingAlpha,
            paren(vec![Currency, text("\u{a0}")]),
        ),
        (Style::AccountingNoSymbol, paren(vec![])),
    ])
    .expect("en");
    assert_eq!(
        en,
        hex(
            "01 04 33 00 01 02  02 04 33 01 03 00  03 06 33 03 03 C2 A0 00  04 03 33 00 00  \
             05 09 33 01 03 00 02 28 03 01 29  06 0D 33 03 03 C2 A0 00 04 28 03 C2 A0 01 29  \
             07 07 33 00 00 01 28 01 29"
        )
    );
    // ar (latn) currency: `‏#,##0.00 ¤;‏-#,##0.00 ¤`.
    let ar = patterns(&[(
        Style::Currency,
        PatternSpec {
            grouping: WEST,
            positive: (vec![text("\u{200f}")], vec![text("\u{a0}"), Currency]),
            negative: Some((vec![text("\u{200f}"), Sign], vec![text("\u{a0}"), Currency])),
        },
    )])
    .expect("ar");
    assert_eq!(
        ar,
        hex("02 12 33 03 E2 80 8F 03 C2 A0 03 04 E2 80 8F 01 03 C2 A0 03")
    );
    let v = Patterns::new(&ar).get(Style::Currency).expect("parses");
    let minus = v.signed(SignShown::Minus);
    assert_eq!(
        minus.prefix.parts().collect::<Vec<_>>(),
        [text("\u{200f}"), Sign]
    );
    assert_eq!(
        minus.suffix.parts().collect::<Vec<_>>(),
        [text("\u{a0}"), Currency]
    );
    assert!(!v.signed(SignShown::Plus).sign_first);
    assert!(!v.signed(SignShown::None).sign_first);
}

/// Every accessor of both views, folded so nothing is optimised away.
fn touch(b: &[u8]) -> usize {
    let mut n = 0;
    if let Some(s) = Symbols::parse(b) {
        n += s.decimal().len() + s.group().len() + s.minus().len() + s.plus().len();
        n += s.percent().len() + usize::from(s.minimum_grouping_digits());
        n += (0..=10).map(|d| s.digits().digit(d).len()).sum::<usize>();
        n += (0..20).filter(|&m| s.grouping().separator_after(m)).count();
    }
    let p = Patterns::new(b);
    n += usize::from(p.is_valid()) + p.styles().count();
    for style in Style::ALL {
        if let Some(pat) = p.resolve(style) {
            for sign in [SignShown::None, SignShown::Minus, SignShown::Plus] {
                let s = pat.signed(sign);
                n += s.prefix.parts().count() + s.suffix.parts().count();
                n += usize::from(s.prefix.has_sign()) + usize::from(s.suffix.has_currency());
            }
        }
    }
    n
}

#[test]
fn views_never_panic() {
    let vectors = [
        hex("33 01 01 2C 03 E2 80 AF 01 2D 01 2B 01 25"),
        hex(
            "33 01 02 D9 AB 02 D9 AC 03 D8 9C 2D 03 D8 9C 2B 04 D9 AA D8 9C \
             D9 A0 D9 A1 D9 A2 D9 A3 D9 A4 D9 A5 D9 A6 D9 A7 D9 A8 D9 A9",
        ),
        hex("01 04 33 00 01 02  05 09 33 01 03 00 02 28 03 01 29"),
        hex("02 12 33 03 E2 80 8F 03 C2 A0 03 04 E2 80 8F 01 03 C2 A0 03"),
    ];
    let mut sum = 0;
    for v in &vectors {
        for len in 0..=v.len() {
            sum += touch(&v[..len]);
        }
        for at in 0..v.len() {
            let mut m = v.clone();
            for x in 0..=255u8 {
                m[at] = x;
                sum += touch(&m);
            }
        }
    }
    assert!(sum > 0);
}
