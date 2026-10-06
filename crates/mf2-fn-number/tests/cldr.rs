//! The localized handlers on real CLDR 48.2.1 data, through `mf2::compile_str`
//! (which writes the locale's `number.*` entries): the locale panel,
//! numbering systems, and tags without data.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mf2::{Arg, BidiStrategy, Compiled, FormatContext, FormatError, Formatter, Function, Registry};

static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("percent", &mf2_fn_number::PERCENT),
    ("string", &mf2::functions::STRING),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_numbers(&mf2_fn_number::NUMBERS);

fn format(src: &str, locale: &str, args: &[(&str, Arg<'_>)]) -> (String, Vec<FormatError>) {
    let m = mf2::compile_str(src, locale).expect("compiles");
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.bidi = BidiStrategy::None;
    let f = Formatter::new(&m.catalog, &REGISTRY, &cx);
    let mut out = String::new();
    let mut errs = Vec::new();
    f.write_named(Compiled::ID, args, &mut out, &mut errs);
    (out, errs)
}

fn ok(src: &str, locale: &str) -> String {
    let (s, e) = format(src, locale, &[]);
    assert!(e.is_empty(), "{locale} {src}: {e:?}");
    s
}

#[test]
fn panel() {
    let src = "{-1234567.891 :number} {0.256 :percent}";
    let want = [
        ("en", "-1,234,567.891 26%"),
        ("es", "-1.234.567,891 26\u{a0}%"),
        ("de", "-1.234.567,891 26\u{a0}%"),
        ("fr", "-1\u{202f}234\u{202f}567,891 26\u{a0}%"),
        ("ar", "\u{200e}-1,234,567.891 26\u{200e}%\u{200e}"),
        ("he", "\u{200e}-1,234,567.891 26%"),
        ("ja", "-1,234,567.891 26%"),
        ("hi", "-12,34,567.891 26%"),
        ("ru", "-1\u{a0}234\u{a0}567,891 26\u{a0}%"),
        ("pl", "-1\u{a0}234\u{a0}567,891 26%"),
        ("cy", "-1,234,567.891 26%"),
    ];
    for (loc, exp) in want {
        assert_eq!(ok(src, loc), exp, "{loc}");
    }
}

#[test]
fn minimum_grouping_digits() {
    // es and pl group four-digit numbers only when asked.
    assert_eq!(ok("{1234 :integer}", "es"), "1234");
    assert_eq!(ok("{1234 :integer useGrouping=always}", "pl"), "1\u{a0}234");
    assert_eq!(ok("{1234 :integer}", "de"), "1.234");
}

#[test]
fn numbering_systems() {
    assert_eq!(ok("{1234.5 :number}", "ar-EG"), "١٬٢٣٤٫٥");
    assert_eq!(ok("{1234.5 :number}", "hi-u-nu-deva"), "१,२३४.५");
    assert_eq!(ok("{1234.5 :number}", "ar-u-nu-latn"), "1,234.5");
}

#[test]
fn syntax_90() {
    let (s, e) = format(
        "{$one} et {$two}",
        "fr",
        &[("one", Arg::Float(1.3)), ("two", Arg::Float(4.2))],
    );
    assert!(e.is_empty(), "{e:?}");
    assert_eq!(s, "1,3 et 4,2");
}

#[test]
fn tags_without_data_still_compile() {
    // Root's data (no grouping change, ASCII): a made-up or odd tag is not
    // an error for the build.
    for tag in [
        "und",
        "zz",
        "en-US",
        "x-private",
        "fr-CA",
        "sr-Latn",
        "zh-Hant-TW",
    ] {
        let (s, e) = format("{1234.5 :number}", tag, &[]);
        assert!(e.is_empty(), "{tag}: {e:?}");
        assert!(!s.is_empty(), "{tag}");
    }
}
