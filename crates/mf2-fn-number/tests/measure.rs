//! `:currency` and `:unit` on real CLDR 48.2.1 data through `mf2::compile_str`
//! (which slices `currency.data` / `unit.data` to the message's literal codes
//! and units), checked against node's `Intl.NumberFormat` (ICU 76.1, CLDR 46)
//! where both agree, and the spec's error rules.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mf2::{Arg, BidiStrategy, Compiled, FormatContext, FormatError, Formatter, Function, Registry};

static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("currency", &mf2_fn_number::CURRENCY),
    ("integer", &mf2_fn_number::INTEGER),
    ("number", &mf2_fn_number::NUMBER),
    ("string", &mf2::functions::STRING),
    ("unit", &mf2_fn_number::UNIT),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_numbers(&mf2_fn_number::NUMBERS);

fn run(src: &str, locale: &str, args: &[(&str, Arg<'_>)]) -> (String, Vec<FormatError>) {
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
    let (s, e) = run(src, locale, &[]);
    assert!(e.is_empty(), "{locale} {src}: {e:?}");
    s
}

#[test]
fn currency_like_intl() {
    for (locale, src, want) in [
        ("en", "{42 :currency currency=EUR}", "€42.00"),
        ("en", "{-1234.5 :currency currency=USD}", "-$1,234.50"),
        ("en", "{1234.5 :currency currency=JPY}", "¥1,235"),
        (
            "en",
            "{5 :currency currency=USD currencyDisplay=code}",
            "USD\u{a0}5.00",
        ),
        (
            "en",
            "{1 :currency currency=EUR currencyDisplay=name}",
            "1.00 euros",
        ),
        (
            "en",
            "{-5 :currency currency=USD currencySign=accounting}",
            "($5.00)",
        ),
        (
            "en",
            "{3.5 :currency currency=GBP currencyDisplay=narrowSymbol}",
            "£3.50",
        ),
        ("en", "{5 :currency currency=CHF}", "CHF\u{a0}5.00"),
        ("de", "{1234.5 :currency currency=EUR}", "1.234,50\u{a0}€"),
        ("de", "{1234.5 :currency currency=USD}", "1.234,50\u{a0}$"),
        ("de", "{5 :currency currency=CHF}", "5,00\u{a0}CHF"),
        (
            "fr",
            "{1234.5 :currency currency=EUR}",
            "1\u{202f}234,50\u{a0}€",
        ),
        (
            "fr",
            "{-5 :currency currency=EUR currencySign=accounting}",
            "(5,00\u{a0}€)",
        ),
        ("ja", "{1234 :currency currency=JPY}", "￥1,234"),
        ("hi", "{1234567.5 :currency currency=INR}", "₹12,34,567.50"),
        (
            "pl",
            "{2 :currency currency=PLN currencyDisplay=name}",
            "2,00 złotego polskiego",
        ),
        (
            "ru",
            "{21 :currency currency=RUB currencyDisplay=name}",
            "21,00 российского рубля",
        ),
        (
            "cy",
            "{2 :currency currency=GBP currencyDisplay=name}",
            "2.00 bunt Prydain",
        ),
    ] {
        assert_eq!(ok(src, locale), want, "{locale} {src}");
    }
}

#[test]
fn currency_options_and_inheritance() {
    assert_eq!(
        ok("{42 :currency currency=EUR fractionDigits=0}", "en"),
        "€42"
    );
    assert_eq!(
        ok("{42 :currency currency=EUR fractionDigits=3}", "en"),
        "€42.000"
    );
    assert_eq!(ok("{42 :currency currency=eur}", "en"), "€42.00");
    assert_eq!(
        ok(
            "{5 :currency currency=USD trailingZeroDisplay=stripIfInteger}",
            "en"
        ),
        "$5"
    );
    assert_eq!(
        ok("{42 :currency currency=EUR currencyDisplay=never}", "en"),
        "42.00"
    );
    // The currency and its options travel with the value.
    assert_eq!(
        ok(
            ".local $n = {42 :currency currency=EUR currencyDisplay=code} {{{$n :currency}}}",
            "en"
        ),
        "EUR\u{a0}42.00"
    );
    assert_eq!(
        ok(
            ".local $n = {4.5 :number} {{{$n :currency currency=EUR}}}",
            "en"
        ),
        "€4.50"
    );
    // MUST NOT override a value's currency: Bad Option, ignored.
    let (s, e) = run(
        ".local $n = {42 :currency currency=EUR} {{{$n :currency currency=USD}}}",
        "en",
        &[],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("€42.00", &[FormatError::BadOption][..])
    );
    // A code that is not well-formed: Bad Option, then no currency.
    let (s, e) = run("{42 :currency currency=EURO}", "en", &[]);
    assert_eq!(s, "{|42|}");
    assert_eq!(e, [FormatError::BadOption, FormatError::BadOperand]);
    // A well-formed code without data (a variable, so not sliced in): its
    // code, CLDR's default digits.
    let (s, e) = run(
        "{42 :currency currency=$k}",
        "en",
        &[("k", Arg::Str("XBT"))],
    );
    assert!(e.is_empty(), "{e:?}");
    assert_eq!(s, "XBT\u{a0}42.00");
}

#[test]
fn unit_like_intl() {
    for (locale, src, want) in [
        ("en", "{42 :unit unit=meter}", "42 m"),
        ("en", "{1 :unit unit=meter unitDisplay=long}", "1 meter"),
        (
            "en",
            "{42 :unit unit=kilometer-per-hour unitDisplay=narrow}",
            "42km/h",
        ),
        (
            "de",
            "{5 :unit unit=kilometer unitDisplay=long}",
            "5 Kilometer",
        ),
        // CLDR 48 writes U+202F here; node's CLDR 46 U+00A0.
        ("fr", "{-5 :unit unit=celsius}", "-5\u{202f}°C"),
        (
            "pl",
            "{2 :unit unit=kilometer unitDisplay=long}",
            "2 kilometry",
        ),
        (
            "pl",
            "{5 :unit unit=kilometer unitDisplay=long}",
            "5 kilometrów",
        ),
        ("ru", "{21 :unit unit=liter unitDisplay=long}", "21 литр"),
        (
            "en",
            "{3 :unit unit=meter-per-second unitDisplay=long}",
            "3 meters per second",
        ),
        ("ja", "{3 :unit unit=meter unitDisplay=long}", "3 メートル"),
    ] {
        assert_eq!(ok(src, locale), want, "{locale} {src}");
    }
}

#[test]
fn composed_units() {
    // CLDR has no liter-per-kilometer: X-per-Y from the two units.
    assert_eq!(ok("{3 :unit unit=liter-per-kilometer}", "en"), "3 L/km");
    // A unit with no data and no composition: Unsupported Operation.
    let (s, e) = run("{3 :unit unit=frobnicator}", "en", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{|3|}", &[FormatError::UnsupportedOperation][..])
    );
}

#[test]
fn unit_errors() {
    let (s, e) = run("{42 :unit unit=meter usage=road}", "en", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("42 m", &[FormatError::UnsupportedOperation][..])
    );
    let (s, e) = run(
        ".local $n = {42 :unit unit=meter} {{{$n :unit unit=foot}}}",
        "en",
        &[],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("42 m", &[FormatError::BadOption][..])
    );
    let (s, e) = run(
        ".local $n = {42 :unit unit=meter} .match $n * {{other}}",
        "en",
        &[],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("other", &[FormatError::BadSelector][..])
    );
}
