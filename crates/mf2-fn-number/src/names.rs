//! The number split (`plan/08` §6): with the feature `intl-names` on
//! `wasm32-unknown-unknown` ([`INTL_NAMES`]), `:currency` and `:unit` take
//! from the host's number formatter (the browser's `Intl.NumberFormat`) what
//! the Rust path reads from the catalog's `currency.data` and `unit.data`:
//! the currency symbol or name, the unit's name, where they stand and the
//! spacing around them, and a currency's own fraction digits. The digits,
//! the rounding and plural selection stay in Rust: the formatter's parts are
//! streamed through, and its run of digits (`integer`, `group`, `decimal`,
//! `fraction`) is replaced by the digits the runtime resolved, written with
//! the catalog's `number.symbols`; its sign keeps its place and takes the
//! catalog's symbol. Without a formatter (an engine without
//! `Intl.NumberFormat` v3), or when it refuses the unit, the value is Rust's
//! digits and then the currency code or the unit identifier, as the Rust
//! path writes them without data — so that the Rust path's currency and
//! unit layout, and its reading of the data, are not linked into such a
//! client.
//!
//! The client reads no `currency.data` or `unit.data`, so the build puts
//! them in the server-only table (`mf2-build`'s `intl-names`).

use mf2_catalog::Catalog;
use mf2_catalog::number::{Grouping as Sizes, Patterns, SignShown, Style, Symbols};
use mf2_runtime::{
    Digits, FnContext, Grouping, INTL_NUMBERS, Measure, MeasureUnit, NumberOut, SubPartSink,
};

use crate::intl;
use crate::localize::{self, Out, Seps};
use crate::measure::{Display, accounting, display_of};

/// Whether `:currency` and `:unit` take their names from the host: the
/// feature `intl-names` on `wasm32-unknown-unknown`, unless the whole
/// `intl` option ([`INTL_NUMBERS`]) is on, which formats everything there.
pub(crate) const INTL_NAMES: bool = !INTL_NUMBERS
    && cfg!(all(
        feature = "intl-names",
        target_arch = "wasm32",
        target_os = "unknown"
    ));

/// The formatter's parts with its digits replaced by Rust's.
struct Merge<'m, 'o> {
    sym: &'m Symbols<'m>,
    seps: Seps<'m>,
    sizes: Sizes,
    digits: &'m Digits<'m>,
    grouping: Option<Grouping>,
    out: &'m mut Out<'o>,
    /// Rust's digits are written (once, at the formatter's first digit).
    written: bool,
}

impl SubPartSink for Merge<'_, '_> {
    fn sub_part(&mut self, kind: &str, text: &str) {
        match kind {
            "integer" | "group" | "decimal" | "fraction" => {
                if !self.written {
                    self.written = true;
                    localize::digits(
                        self.sym,
                        self.seps,
                        self.sizes,
                        self.digits,
                        self.grouping,
                        self.out,
                    );
                }
            }
            // The formatter's sign position, Rust's sign and the catalog's
            // symbol (the two apply the same `signDisplay` to the same
            // rounded value; an accounting negative has no sign part).
            "minusSign" | "plusSign" => match localize::shown(self.digits) {
                SignShown::Minus => {
                    self.out.put("minusSign", self.sym.minus());
                }
                SignShown::Plus => {
                    self.out.put("plusSign", self.sym.plus());
                }
                SignShown::None => {}
            },
            // `currency`, `unit`, `literal`: the formatter's.
            _ => self.out.put(kind, text),
        }
    }
}

/// The grouping sizes of the pattern a `:currency` value is laid out with
/// in the Rust path, the locale's otherwise (a unit, a currency name).
fn sizes(catalog: &Catalog, sym: &Symbols<'_>, m: &Measure<'_>) -> Sizes {
    let display = display_of(m.flags);
    if !matches!(m.unit, MeasureUnit::Currency(_)) || display == Display::Name {
        return sym.grouping();
    }
    let base = match (accounting(m.flags), display == Display::Never) {
        (false, false) => Style::Currency,
        (true, false) => Style::Accounting,
        (false, true) => Style::CurrencyNoSymbol,
        (true, true) => Style::AccountingNoSymbol,
    };
    Patterns::of(catalog)
        .and_then(|p| p.resolve(base))
        .map_or(sym.grouping(), |p| p.grouping())
}

/// Rust's digits (the catalog's symbols, or neutral without them), then the
/// currency code or the unit identifier.
fn fallback(sym: Option<&Symbols<'_>>, digits: &Digits<'_>, m: &Measure<'_>, out: &mut Out<'_>) {
    match sym {
        Some(sym) => localize::write_number(
            sym,
            None,
            None,
            Seps::of(sym),
            sym.grouping(),
            digits,
            m.number.grouping(),
            out,
        ),
        None => localize::neutral(digits, out),
    }
    let (gap, kind) = match m.unit {
        MeasureUnit::Currency(_) => ("\u{a0}", "currency"),
        MeasureUnit::Unit(_) => (" ", "unit"),
    };
    out.put("literal", gap);
    out.put(kind, m.unit.as_str());
}

/// Writes the `:currency` or `:unit` value `m` with the host's names and
/// Rust's digits; without a formatter, a formatter that refuses the value,
/// or number symbols in the catalog, the [`fallback`].
pub(crate) fn measure(cx: &FnContext<'_>, m: &Measure<'_>, out: &mut Out<'_>) {
    let catalog = cx.catalog();
    let Some(digits) = m.number.digits() else {
        return;
    };
    let Some(sym) = Symbols::of(catalog) else {
        return fallback(None, &digits, m, out);
    };
    let mut merge = Merge {
        sym: &sym,
        seps: Seps::of(&sym),
        sizes: sizes(catalog, &sym, m),
        digits: &digits,
        grouping: m.number.grouping(),
        out,
        written: false,
    };
    if !m
        .number
        .format_by_host(cx, intl::style(m), false, NumberOut::Parts(&mut merge))
    {
        // Nothing was written.
        return fallback(Some(merge.sym), merge.digits, m, merge.out);
    }
    if !merge.written {
        // The formatter wrote no digits (it does not happen for a finite
        // value): Rust's after its text, so that the number still shows.
        localize::digits(
            merge.sym,
            merge.seps,
            merge.sizes,
            merge.digits,
            merge.grouping,
            merge.out,
        );
    }
}

/// The split's own tests (`plan/08` §6), native: a stand-in for the
/// browser's formatter writes `Intl`-shaped parts with wrong digits, which
/// the merge must replace with the runtime's, in the catalog's symbols.
#[cfg(all(test, feature = "intl-names"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    extern crate std;

    use std::string::String;
    use std::vec::Vec;

    use mf2::{BidiStrategy, Compiled, FormatContext, FormatError, Formatter, Registry};
    use mf2_runtime::{
        Category, ErrorSink, FnContext, Function, Host, NumberFormatter, NumberOut, NumberRequest,
        NumberStyle, Options, Sink, Value, currency_digits_by_host,
    };

    use super::measure;
    use crate::localize::Out;
    use crate::{CURRENCY, UNIT};

    /// `Intl.NumberFormat`'s parts for en, with the digits always `9,999.99`
    /// (so that a test sees whose digits are shown), the currency's own
    /// digits for a neutral `0` (JPY: none), `km` for `kilometer`, and every
    /// other unit refused.
    struct Stand;

    impl NumberFormatter for Stand {
        fn format(&self, _locale: &str, r: &NumberRequest<'_>, out: NumberOut<'_>) -> bool {
            let NumberOut::Parts(out) = out else {
                return false;
            };
            let negative = r.value.starts_with('-');
            match r.style {
                NumberStyle::Currency { code, .. } if r.neutral && r.value == "0" => {
                    out.sub_part("currency", code);
                    out.sub_part("literal", "\u{a0}");
                    out.sub_part("integer", "0");
                    if code != "JPY" {
                        out.sub_part("decimal", ".");
                        out.sub_part("fraction", "00");
                    }
                }
                NumberStyle::Currency {
                    code, accounting, ..
                } => {
                    let symbol = match code {
                        "USD" => "$",
                        "EUR" => "€",
                        "JPY" => "¥",
                        _ => code,
                    };
                    if negative && accounting {
                        out.sub_part("literal", "(");
                    } else if negative {
                        out.sub_part("minusSign", "-");
                    }
                    out.sub_part("currency", symbol);
                    nines(out);
                    if negative && accounting {
                        out.sub_part("literal", ")");
                    }
                }
                NumberStyle::Unit {
                    unit: "kilometer", ..
                } => {
                    if negative {
                        out.sub_part("minusSign", "-");
                    }
                    nines(out);
                    out.sub_part("literal", " ");
                    out.sub_part("unit", "km");
                }
                _ => return false,
            }
            true
        }

        fn plural(&self, _locale: &str, _r: &NumberRequest<'_>) -> Option<Category> {
            None
        }
    }

    fn nines(out: &mut dyn mf2_runtime::SubPartSink) {
        out.sub_part("integer", "9");
        out.sub_part("group", ",");
        out.sub_part("integer", "999");
        out.sub_part("decimal", ".");
        out.sub_part("fraction", "99");
    }

    struct StandHost;

    impl Host for StandHost {
        fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
            mf2::host_std::HOST.f64_to_text(x, buf)
        }

        fn numbers(&self) -> Option<&dyn NumberFormatter> {
            Some(&Stand)
        }
    }

    static HOST: StandHost = StandHost;

    /// `:currency` (false) or `:unit` (true) resolved as ever, written
    /// through the split alone.
    struct Split(bool);

    impl Function for Split {
        fn resolve<'a>(
            &self,
            cx: &FnContext<'_>,
            operand: Option<&Value<'a>>,
            options: &Options<'_, 'a>,
            errs: &mut dyn ErrorSink,
        ) -> Option<Value<'a>> {
            if self.0 {
                UNIT.resolve(cx, operand, options, errs)
            } else {
                CURRENCY.resolve(cx, operand, options, errs)
            }
        }

        fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
            let Value::Measure(m) = value else {
                panic!("not a measure");
            };
            measure(cx, m, &mut Out::Text(out));
        }
    }

    /// `{|CODE| :digits}`: the currency's own digits from the host.
    struct OwnDigits;

    impl Function for OwnDigits {
        fn resolve<'a>(
            &self,
            _cx: &FnContext<'_>,
            operand: Option<&Value<'a>>,
            _options: &Options<'_, 'a>,
            _errs: &mut dyn ErrorSink,
        ) -> Option<Value<'a>> {
            match operand {
                Some(Value::Str(s)) => Some(Value::Str(*s)),
                _ => None,
            }
        }

        fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
            if let Value::Str(code) = value {
                out.push_str(match currency_digits_by_host(cx, code) {
                    Some(0) => "0",
                    Some(2) => "2",
                    Some(_) => "other",
                    None => "none",
                });
            }
        }
    }

    static FUNCTIONS: [(&str, &dyn Function); 3] = [
        ("currency", &Split(false)),
        ("digits", &OwnDigits),
        ("unit", &Split(true)),
    ];
    static REGISTRY: Registry = Registry::new(&FUNCTIONS);

    fn run(src: &str, locale: &str, host: &'static dyn Host) -> String {
        let m = mf2::compile_str(src, locale).expect("compiles");
        let mut cx = FormatContext::new(host);
        cx.bidi = BidiStrategy::None;
        let f = Formatter::new(&m.catalog, &REGISTRY, &cx);
        let mut out = String::new();
        let mut errs: Vec<FormatError> = Vec::new();
        f.write_named(Compiled::ID, &[], &mut out, &mut errs);
        assert!(errs.is_empty(), "{locale} {src}: {errs:?}");
        out
    }

    #[test]
    fn the_names_are_the_hosts_and_the_digits_rusts() {
        for (locale, src, want) in [
            ("en", "{-1234.5 :currency currency=USD}", "-$1,234.50"),
            ("en", "{1234.5 :currency currency=JPY}", "¥1,235"),
            (
                "en",
                "{-5 :currency currency=USD currencySign=accounting}",
                "($5.00)",
            ),
            ("en", "{1234.5 :unit unit=kilometer}", "1,234.5 km"),
            ("en", "{-2 :unit unit=kilometer}", "-2 km"),
            // The catalog's symbols and grouping (pl: a space, a decimal
            // comma, no group below five digits), the host's name.
            ("pl", "{1234.5 :unit unit=kilometer}", "1234,5 km"),
            ("pl", "{12345.5 :unit unit=kilometer}", "12\u{a0}345,5 km"),
        ] {
            assert_eq!(run(src, locale, &HOST), want, "{locale} {src}");
        }
    }

    #[test]
    fn a_unit_the_host_refuses_shows_its_identifier() {
        assert_eq!(run("{3 :unit unit=celsius}", "en", &HOST), "3 celsius");
    }

    #[test]
    fn without_a_formatter_the_code_shows() {
        assert_eq!(
            run("{5 :currency currency=EUR}", "en", &mf2::host_std::HOST),
            "5.00\u{a0}EUR"
        );
        assert_eq!(
            run("{-1234.5 :unit unit=kilometer}", "en", &mf2::host_std::HOST),
            "-1,234.5 kilometer"
        );
        assert_eq!(run("{|EUR| :digits}", "en", &mf2::host_std::HOST), "none");
    }

    #[test]
    fn a_currencys_own_digits_come_from_the_host() {
        assert_eq!(run("{|JPY| :digits}", "en", &HOST), "0");
        assert_eq!(run("{|EUR| :digits}", "en", &HOST), "2");
    }
}
