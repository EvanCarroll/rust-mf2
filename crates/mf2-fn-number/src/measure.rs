//! `:currency` and `:unit` (`functions/number.md`; `plans/03-runtime.md`
//! §2.7, §5): the runtime's numeric core resolves the number
//! (`NumberSpec::currency`, `NumberSpec::UNIT`), this module the currency or
//! unit and the options only these functions have, into a `Measure`; the
//! catalog's `currency.data` / `unit.data` entries (02 §4.6–§4.7) and the
//! number patterns write it.
//!
//! Decisions where the spec leaves a choice (tested in
//! `tests/measure.rs` and `conformance/extra/functions/unit.json`):
//!
//! * a `currency` / `unit` value that is not well-formed (`3ALPHA`; a
//!   lower-case `[a-z0-9]` identifier with `-` between parts) is *Bad
//!   Option* and ignored — then, with no currency or unit, *Bad Operand*;
//! * `currency` on a value that has one is *Bad Option* (number.md: MUST
//!   NOT override); `unit` on a value with another unit likewise (number.md:
//!   MUST NOT substitute a unit without converting); the same unit again is
//!   no error;
//! * `usage` is *Unsupported Operation*: no conversion, the unit stays;
//! * a currency the catalog has no data for formats with its code as the
//!   symbol and name and CLDR's default fraction digits (UTS #35's
//!   fallback); a unit it has no data for (and cannot compose as
//!   `X-per-Y` from two it has) is *Unsupported Operation* when formatted;
//! * neither selects (*Bad Selector*); the part kind is `number`.

use mf2_catalog::Catalog;
use mf2_catalog::currency::{Currencies, Currency};
use mf2_catalog::format::locale_key;
use mf2_catalog::number::{Patterns, Style, Symbols, Template, TemplatePart};
use mf2_catalog::unit::{Unit, Units, Width};
use mf2_runtime::{
    Category, Digits, Dir, ErrorSink, FnContext, FormatError, Function, Measure, MeasureUnit,
    Number, NumberSpec, Options, Sink, SubPartSink, Value, plural_category,
};

use crate::localize::{self, Out, Seps, Symbol, ends_with_currency, starts_with_currency};

// ─────────────────────────────────────────────────────── the flags ──
//
// `Measure::flags`, this crate's encoding of the options the runtime does
// not know (inherited by a later `:currency` / `:unit` over the value):
// bits 0–2 `currencyDisplay`, 3–4 `currencySign`, 5–11 `fractionDigits`
// (0 unset, 1 `auto`, 2 + n), 12–13 `unitDisplay`. 0 = unset everywhere.

const DISPLAY_SHIFT: u32 = 0;
const SIGN_SHIFT: u32 = 3;
const DIGITS_SHIFT: u32 = 5;
const WIDTH_SHIFT: u32 = 12;

/// `currencyDisplay`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Display {
    NarrowSymbol = 1,
    Symbol = 2,
    Name = 3,
    Code = 4,
    Never = 5,
}

const DISPLAYS: [(&str, Display); 5] = [
    ("narrowSymbol", Display::NarrowSymbol),
    ("symbol", Display::Symbol),
    ("name", Display::Name),
    ("code", Display::Code),
    ("never", Display::Never),
];

const WIDTHS: [(&str, Width); 3] = [
    ("short", Width::Short),
    ("narrow", Width::Narrow),
    ("long", Width::Long),
];

fn field(flags: u32, shift: u32, bits: u32) -> u32 {
    (flags >> shift) & ((1 << bits) - 1)
}

fn set(flags: &mut u32, shift: u32, bits: u32, v: u32) {
    let mask = ((1 << bits) - 1) << shift;
    *flags = (*flags & !mask) | ((v << shift) & mask);
}

fn display_of(flags: u32) -> Display {
    match field(flags, DISPLAY_SHIFT, 3) {
        1 => Display::NarrowSymbol,
        3 => Display::Name,
        4 => Display::Code,
        5 => Display::Never,
        _ => Display::Symbol,
    }
}

fn accounting(flags: u32) -> bool {
    field(flags, SIGN_SHIFT, 2) == 2
}

fn width_of(flags: u32) -> Width {
    match field(flags, WIDTH_SHIFT, 2) {
        2 => Width::Narrow,
        3 => Width::Long,
        _ => Width::Short,
    }
}

fn width_code(w: Width) -> u32 {
    match w {
        Width::Short => 1,
        Width::Narrow => 2,
        Width::Long => 3,
    }
}

// ─────────────────────────────────────────────────────── helpers ──

/// A string value's text with the value's lifetime (literals, string
/// arguments, application values' `as_str`).
fn str_of<'a>(v: &Value<'a>) -> Option<&'a str> {
    match v {
        Value::Str(s) | Value::Decimal(s) => Some(s),
        Value::Custom(c) => c.as_str(),
        _ => None,
    }
}

/// A well-formed currency code (`currency_code = 3ALPHA`, case-insensitive),
/// upper-cased.
fn currency_code(s: &str) -> Option<[u8; 3]> {
    match *s.as_bytes() {
        [a, b, c]
            if a.is_ascii_alphabetic() && b.is_ascii_alphabetic() && c.is_ascii_alphabetic() =>
        {
            Some([
                a.to_ascii_uppercase(),
                b.to_ascii_uppercase(),
                c.to_ascii_uppercase(),
            ])
        }
        _ => None,
    }
}

/// A well-formed unit identifier, as CLDR writes them: parts of lower-case
/// ASCII letters and digits joined by `-` (`kilometer-per-hour`,
/// `liter-per-100-kilometer`), at most 64 bytes.
fn is_unit_id(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 64 || b.first() == Some(&b'-') || b.last() == Some(&b'-') {
        return false;
    }
    let mut prev_dash = false;
    for &c in b {
        let dash = c == b'-';
        if !(dash || c.is_ascii_lowercase() || c.is_ascii_digit()) || (dash && prev_dash) {
            return false;
        }
        prev_dash = dash;
    }
    true
}

/// A *digit size option* value (`"0"`, `"1"`–`"99"`, or an integer).
fn digit_size(v: &Value<'_>) -> Option<u8> {
    match v {
        Value::Int(n) => u8::try_from(*n).ok().filter(|n| *n <= 99),
        Value::Number(n) => n
            .to_i64()
            .and_then(|n| u8::try_from(n).ok())
            .filter(|n| *n <= 99),
        _ => match *str_of(v)?.as_bytes() {
            [d @ b'0'..=b'9'] => Some(d - b'0'),
            [a @ b'1'..=b'9', b @ b'0'..=b'9'] => Some((a - b'0') * 10 + (b - b'0')),
            _ => None,
        },
    }
}

/// The plural category of `d` as shown, by the catalog's cardinal rules.
fn category(catalog: &Catalog, d: &Digits<'_>) -> Category {
    let rules = catalog
        .locale_entry(locale_key::PLURAL_CARDINAL)
        .unwrap_or(&[]);
    plural_category(rules, &d.operands())
}

/// The operand's measure, when it is one (a previous `:currency` / `:unit`,
/// or an application value).
fn operand_measure<'a>(operand: Option<&Value<'a>>) -> Option<Measure<'a>> {
    match operand? {
        Value::Measure(m) => Some(m.clone()),
        Value::Custom(c) => c
            .as_measure()
            .map(|m| Measure::new(m.number, m.unit, m.flags)),
        _ => None,
    }
}

// ────────────────────────────────────────────────────── :currency ──

/// `:currency` (`functions/number.md`).
#[derive(Clone, Copy, Default, Debug)]
pub struct CurrencyFunction;

/// `:currency`.
pub static CURRENCY: CurrencyFunction = CurrencyFunction;

impl Function for CurrencyFunction {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        let inherited = operand_measure(operand).and_then(|m| match m.unit {
            MeasureUnit::Currency(c) => Some((c, m.flags)),
            MeasureUnit::Unit(_) => None,
        });
        let mut code = inherited.map(|(c, _)| c);
        let mut flags = inherited.map_or(0, |(_, f)| f);
        for (name, v) in options.iter() {
            match name {
                "currency" => match str_of(v.value).and_then(currency_code) {
                    // MUST NOT override the currency of a value that has one.
                    Some(_) if inherited.is_some() => errs.error(FormatError::BadOption),
                    Some(c) => code = Some(c),
                    None => errs.error(FormatError::BadOption),
                },
                "currencyDisplay" => {
                    match str_of(v.value).and_then(|t| DISPLAYS.iter().find(|(k, _)| *k == t)) {
                        Some(&(_, d)) => set(&mut flags, DISPLAY_SHIFT, 3, d as u32),
                        None => errs.error(FormatError::BadOption),
                    }
                }
                "currencySign" => match str_of(v.value) {
                    Some("standard") => set(&mut flags, SIGN_SHIFT, 2, 1),
                    Some("accounting") => set(&mut flags, SIGN_SHIFT, 2, 2),
                    _ => errs.error(FormatError::BadOption),
                },
                "fractionDigits" => {
                    if str_of(v.value) == Some("auto") {
                        set(&mut flags, DIGITS_SHIFT, 7, 1);
                    } else if let Some(n) = digit_size(v.value) {
                        set(&mut flags, DIGITS_SHIFT, 7, 2 + u32::from(n));
                    } else {
                        errs.error(FormatError::BadOption);
                    }
                }
                _ => {}
            }
        }
        let Some(code) = code else {
            // A numeric operand without a currency (number.md).
            errs.error(FormatError::BadOperand);
            return None;
        };
        let digits = match field(flags, DIGITS_SHIFT, 7) {
            n if n >= 2 => u8::try_from(n - 2).unwrap_or(2),
            _ => auto_digits(cx.catalog(), code),
        };
        let number = Number::resolve(NumberSpec::currency(digits), cx, operand, options, errs)?;
        Some(Value::Measure(Measure::new(
            number,
            MeasureUnit::Currency(code),
            flags,
        )))
    }

    fn formattable(&self, _cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        match value {
            Value::Measure(m) if matches!(m.unit, MeasureUnit::Currency(_)) => Ok(()),
            _ => Err(FormatError::MessageFunctionError),
        }
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Value::Measure(m) = value {
            write_currency(cx.catalog(), m, &mut Out::Text(out));
        }
    }

    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if let Value::Measure(m) = value {
            write_currency(cx.catalog(), m, &mut Out::Parts(out));
        }
    }

    fn part_kind(&self) -> &'static str {
        "number"
    }

    fn dir(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> Dir {
        Dir::Ltr
    }
}

/// The currency's own fraction digits (`fractionDigits=auto`): from the
/// catalog's data, else CLDR's default, else 2.
fn auto_digits(catalog: &Catalog, code: [u8; 3]) -> u8 {
    Currencies::of(catalog).map_or(2, |c| {
        c.get(code)
            .map_or(c.default_digits(), |x| x.fraction_digits())
    })
}

fn write_currency(catalog: &Catalog, m: &Measure<'_>, out: &mut Out<'_>) {
    let Some(d) = m.number.digits() else {
        return;
    };
    let code = m.unit.as_str();
    let Some(sym) = Symbols::of(catalog) else {
        // No number data: the core's digits and the code.
        localize::neutral(&d, out);
        out.put("literal", "\u{a0}");
        out.put("currency", code);
        return;
    };
    let bytes = match m.unit {
        MeasureUnit::Currency(c) => c,
        MeasureUnit::Unit(_) => return,
    };
    let data = Currencies::of(catalog);
    let cur: Option<Currency<'_>> = data.and_then(|c| c.get(bytes));
    let display = display_of(m.flags);
    let mut seps = Seps::of(&sym);
    if let Some(c) = cur {
        if let Some(dec) = c.decimal() {
            seps.decimal = dec;
        }
        if let Some(g) = c.group() {
            seps.group = g;
        }
    }
    if display == Display::Name {
        return write_currency_name(catalog, &sym, seps, cur, code, &d, m, out);
    }
    // The symbol text and whether its ends are letter-like.
    let (text, first, last) = match (display, cur) {
        (Display::Never, _) => ("", false, false),
        (Display::Code, _) | (_, None) => (code, true, true),
        (Display::NarrowSymbol, Some(c)) => match c.narrow_symbol() {
            Some(n) => (n, c.narrow_edges().first, c.narrow_edges().last),
            None => (c.symbol(), c.symbol_edges().first, c.symbol_edges().last),
        },
        (_, Some(c)) => (c.symbol(), c.symbol_edges().first, c.symbol_edges().last),
    };
    let accounting = accounting(m.flags);
    let (base, alpha) = match (accounting, display == Display::Never) {
        (false, false) => (Style::Currency, Some(Style::CurrencyAlpha)),
        (true, false) => (Style::Accounting, Some(Style::AccountingAlpha)),
        (false, true) => (Style::CurrencyNoSymbol, None),
        (true, true) => (Style::AccountingNoSymbol, None),
    };
    let patterns = Patterns::of(catalog);
    // The currency's own standard pattern replaces the locale's.
    let own = if base == Style::Currency {
        cur.and_then(|c| c.pattern())
    } else {
        None
    };
    let mut spacing = true;
    let pattern = own.or_else(|| {
        let p = patterns.and_then(|p| p.resolve(base))?;
        // A letter-like end next to the digits takes the
        // `…alphaNextToNumber` pattern when the locale has one.
        let s = p.signed(localize::shown(&d));
        let touching =
            (last && ends_with_currency(s.prefix)) || (first && starts_with_currency(s.suffix));
        if touching && let Some(a) = alpha.and_then(|a| patterns.and_then(|p| p.get(a))) {
            spacing = false;
            return Some(a);
        }
        Some(p)
    });
    let sizes = pattern.map_or(sym.grouping(), |p| p.grouping());
    let symbol = Symbol {
        text,
        first,
        last,
        spacing,
    };
    if pattern.is_none() {
        // No currency pattern in the catalog: the symbol before the number.
        out.put("currency", text);
        if !text.is_empty() && last {
            out.put("literal", "\u{a0}");
        }
    }
    localize::write_number(
        &sym,
        pattern,
        Some(symbol),
        seps,
        sizes,
        &d,
        m.number.grouping(),
        out,
    );
}

/// `currencyDisplay=name`: the locale's name pattern (`{0} {1}`) with the
/// number in the decimal pattern and the display name for its plural
/// category.
#[allow(clippy::too_many_arguments)]
fn write_currency_name(
    catalog: &Catalog,
    sym: &Symbols<'_>,
    seps: Seps<'_>,
    cur: Option<Currency<'_>>,
    code: &str,
    d: &Digits<'_>,
    m: &Measure<'_>,
    out: &mut Out<'_>,
) {
    let cat = category(catalog, d) as u8;
    let name = cur.and_then(|c| c.name_for(cat)).unwrap_or(code);
    let template = Currencies::of(catalog).and_then(|c| c.name_pattern(cat));
    let number = |out: &mut Out<'_>| {
        localize::write_number(
            sym,
            None,
            None,
            seps,
            sym.grouping(),
            d,
            m.number.grouping(),
            out,
        );
    };
    if let Some(t) = template {
        fill(t, out, number, |out| out.put("currency", name));
    } else {
        number(out);
        out.put("literal", " ");
        out.put("currency", name);
    }
}

/// Writes `t` with `arg0` for `{0}` and `arg1` for `{1}`; literal text is a
/// `literal` part when blank, else `unit` (Intl's parts for unit and
/// currency-name patterns).
fn fill(
    t: Template<'_>,
    out: &mut Out<'_>,
    mut arg0: impl FnMut(&mut Out<'_>),
    mut arg1: impl FnMut(&mut Out<'_>),
) {
    for p in t.parts() {
        match p {
            TemplatePart::Arg0 => arg0(out),
            TemplatePart::Arg1 => arg1(out),
            TemplatePart::Text(s) => {
                let blank = s.chars().all(char::is_whitespace);
                out.put(if blank { "literal" } else { "unit" }, s);
            }
        }
    }
}

// ────────────────────────────────────────────────────────── :unit ──

/// `:unit` (`functions/number.md`, Draft).
#[derive(Clone, Copy, Default, Debug)]
pub struct UnitFunction;

/// `:unit`.
pub static UNIT: UnitFunction = UnitFunction;

impl Function for UnitFunction {
    fn resolve<'a>(
        &self,
        cx: &FnContext<'_>,
        operand: Option<&Value<'a>>,
        options: &Options<'_, 'a>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Value<'a>> {
        let inherited = operand_measure(operand).and_then(|m| match m.unit {
            MeasureUnit::Unit(u) => Some((u, m.flags)),
            MeasureUnit::Currency(_) => None,
        });
        let mut unit = inherited.map(|(u, _)| u);
        let mut flags = inherited.map_or(0, |(_, f)| f);
        for (name, v) in options.iter() {
            match name {
                "unit" => match str_of(v.value).filter(|u| is_unit_id(u)) {
                    // No other unit without converting (number.md).
                    Some(u) if inherited.is_some_and(|(i, _)| i != u) => {
                        errs.error(FormatError::BadOption);
                    }
                    Some(u) => unit = Some(u),
                    None => errs.error(FormatError::BadOption),
                },
                "unitDisplay" => {
                    match str_of(v.value).and_then(|t| WIDTHS.iter().find(|(k, _)| *k == t)) {
                        Some(&(_, w)) => set(&mut flags, WIDTH_SHIFT, 2, width_code(w)),
                        None => errs.error(FormatError::BadOption),
                    }
                }
                // Conversion is optional and not implemented (number.md,
                // "Unit Conversion"): the value keeps its unit.
                "usage" => errs.error(FormatError::UnsupportedOperation),
                _ => {}
            }
        }
        let Some(unit) = unit else {
            // A numeric operand without a unit (number.md).
            errs.error(FormatError::BadOperand);
            return None;
        };
        let number = Number::resolve(NumberSpec::UNIT, cx, operand, options, errs)?;
        Some(Value::Measure(Measure::new(
            number,
            MeasureUnit::Unit(unit),
            flags,
        )))
    }

    fn formattable(&self, cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        match value {
            Value::Measure(m) => match m.unit {
                MeasureUnit::Unit(id) => {
                    let units = Units::of(cx.catalog());
                    if Symbols::of(cx.catalog()).is_none()
                        || units.is_some_and(|u| resolve_unit(&u, id).is_some())
                    {
                        Ok(())
                    } else {
                        Err(FormatError::UnsupportedOperation)
                    }
                }
                MeasureUnit::Currency(_) => Err(FormatError::MessageFunctionError),
            },
            _ => Err(FormatError::MessageFunctionError),
        }
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Value::Measure(m) = value {
            write_unit(cx.catalog(), m, &mut Out::Text(out));
        }
    }

    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if let Value::Measure(m) = value {
            write_unit(cx.catalog(), m, &mut Out::Parts(out));
        }
    }

    fn part_kind(&self) -> &'static str {
        "number"
    }

    fn dir(&self, _cx: &FnContext<'_>, _value: &Value<'_>) -> Dir {
        Dir::Ltr
    }
}

/// How a unit is written: one CLDR unit, or `X-per-Y` composed from two.
enum Resolved<'u> {
    One(Unit<'u>),
    Per(Unit<'u>, Unit<'u>),
}

/// The unit `id` in `units`: itself, or the first split `X-per-Y` whose
/// parts both are there (as `mf2_locale_data::unit::composition` slices it).
fn resolve_unit<'u>(units: &Units<'u>, id: &str) -> Option<Resolved<'u>> {
    if let Some(u) = units.get(id) {
        return Some(Resolved::One(u));
    }
    let b = id.as_bytes();
    let mut at = 0;
    while at + 5 <= b.len() {
        if b.get(at..at + 5) == Some(b"-per-") {
            let (x, y) = (id.get(..at)?, id.get(at + 5..)?);
            if let (Some(x), Some(y)) = (units.get(x), units.get(y)) {
                return Some(Resolved::Per(x, y));
            }
        }
        at += 1;
    }
    None
}

/// The width to write: the one asked for if the catalog carries it, else
/// the first it has patterns in.
fn width_for(u: &Unit<'_>, want: Width) -> Width {
    if u.has_patterns(want) {
        return want;
    }
    [Width::Short, Width::Long, Width::Narrow]
        .into_iter()
        .find(|w| u.has_patterns(*w))
        .unwrap_or(want)
}

fn write_unit(catalog: &Catalog, measure: &Measure<'_>, out: &mut Out<'_>) {
    let Some(digits) = measure.number.digits() else {
        return;
    };
    let MeasureUnit::Unit(id) = measure.unit else {
        return;
    };
    let (Some(sym), Some(units)) = (Symbols::of(catalog), Units::of(catalog)) else {
        // No data: the core's digits and the identifier.
        localize::neutral(&digits, out);
        out.put("literal", " ");
        out.put("unit", id);
        return;
    };
    let seps = Seps::of(&sym);
    let grouping = measure.number.grouping();
    let cat = category(catalog, &digits) as u8;
    let number = |out: &mut Out<'_>| {
        localize::write_number(
            &sym,
            None,
            None,
            seps,
            sym.grouping(),
            &digits,
            grouping,
            out,
        );
    };
    let want = width_of(measure.flags);
    match resolve_unit(&units, id) {
        Some(Resolved::One(unit)) => {
            let w = width_for(&unit, want);
            match unit.pattern(w, cat) {
                Some(t) => fill(t, out, number, |_| {}),
                None => number(out),
            }
        }
        Some(Resolved::Per(num, den)) => {
            let w = width_for(&num, want);
            let numerator = |out: &mut Out<'_>| match num.pattern(w, cat) {
                Some(t) => fill(t, out, number, |_| {}),
                None => number(out),
            };
            if let Some(per) = den.per_unit_pattern(w) {
                fill(per, out, numerator, |_| {});
            } else {
                // The locale's `per` pattern (`{0}/{1}`) with the
                // denominator's singular form without its number.
                let denominator = den.pattern(w, Category::One as u8);
                match units.per_pattern(w) {
                    Some(per) => fill(per, out, numerator, |out| {
                        if let Some(t) = denominator {
                            for p in t.parts() {
                                if let TemplatePart::Text(s) = p {
                                    out.put("unit", s.trim());
                                }
                            }
                        }
                    }),
                    None => numerator(out),
                }
            }
        }
        // Not formattable (`formattable` said so): nothing.
        None => {}
    }
}
