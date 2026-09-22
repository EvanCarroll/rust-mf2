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
    Category, Digits, Dir, ErrorSink, FnContext, FormatError, Function, INTL_NUMBERS, Measure,
    MeasureUnit, Number, NumberOut, NumberSpec, Options, Sink, SubPartSink, Value, plural_category,
};

use crate::intl;
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
pub(crate) enum Display {
    NarrowSymbol = 1,
    Symbol = 2,
    Name = 3,
    Code = 4,
    Never = 5,
}

/// The keyword options' values and their codes in the flags.
const DISPLAYS: [(&str, u32); 5] = [
    ("narrowSymbol", Display::NarrowSymbol as u32),
    ("symbol", Display::Symbol as u32),
    ("name", Display::Name as u32),
    ("code", Display::Code as u32),
    ("never", Display::Never as u32),
];
const SIGNS: [(&str, u32); 2] = [("standard", 1), ("accounting", 2)];
const WIDTHS: [(&str, u32); 3] = [("short", 1), ("narrow", 2), ("long", 3)];

fn field(flags: u32, shift: u32, bits: u32) -> u32 {
    (flags >> shift) & ((1 << bits) - 1)
}

fn set(flags: &mut u32, shift: u32, bits: u32, v: u32) {
    let mask = ((1 << bits) - 1) << shift;
    *flags = (*flags & !mask) | ((v << shift) & mask);
}

pub(crate) fn display_of(flags: u32) -> Display {
    match field(flags, DISPLAY_SHIFT, 3) {
        1 => Display::NarrowSymbol,
        3 => Display::Name,
        4 => Display::Code,
        5 => Display::Never,
        _ => Display::Symbol,
    }
}

pub(crate) fn accounting(flags: u32) -> bool {
    field(flags, SIGN_SHIFT, 2) == 2
}

/// `fractionDigits` unset or `auto`: the currency's own digits.
pub(crate) fn own_digits(flags: u32) -> bool {
    field(flags, DIGITS_SHIFT, 7) < 2
}

pub(crate) fn width_of(flags: u32) -> Width {
    match field(flags, WIDTH_SHIFT, 2) {
        2 => Width::Narrow,
        3 => Width::Long,
        _ => Width::Short,
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

/// The plural category of `d` as shown, by the catalog's cardinal rules.
fn category(catalog: &Catalog, d: &Digits<'_>) -> Category {
    let rules = catalog
        .locale_entry(locale_key::PLURAL_CARDINAL)
        .unwrap_or(&[]);
    plural_category(rules, &d.operands())
}

/// The operand's currency or unit and flags, when it is a measure (a
/// previous `:currency` / `:unit`, or an application value); its number is
/// `Number::resolve`'s.
fn operand_measure<'a>(operand: Option<&Value<'a>>) -> Option<(MeasureUnit<'a>, u32)> {
    match operand? {
        Value::Measure(m) => Some((m.unit, m.flags)),
        Value::Custom(c) => c.as_measure().map(|m| (m.unit, m.flags)),
        _ => None,
    }
}

/// Sets the `bits`-wide field at `shift` to the code of the keyword `text`
/// in `table`; *Bad Option* when it is not one.
fn keyword(
    text: Option<&str>,
    table: &[(&str, u32)],
    shift: u32,
    bits: u32,
    flags: &mut u32,
    errs: &mut dyn ErrorSink,
) {
    match text.and_then(|t| table.iter().find(|(k, _)| *k == t)) {
        Some(&(_, code)) => set(flags, shift, bits, code),
        None => errs.error(FormatError::BadOption),
    }
}

/// `:currency` (`unit` false) and `:unit` (`unit` true): the currency or
/// unit — the operand's, or the option's — and the options only these
/// functions have, then the number (`Number::resolve`).
fn resolve_measure<'a>(
    unit: bool,
    cx: &FnContext<'_>,
    operand: Option<&Value<'a>>,
    options: Options<'_, 'a>,
    errs: &mut dyn ErrorSink,
) -> Option<Value<'a>> {
    let inherited =
        operand_measure(operand).filter(|(u, _)| matches!(u, MeasureUnit::Unit(_)) == unit);
    let mut what = inherited.map(|(u, _)| u);
    let mut flags = inherited.map_or(0, |(_, f)| f);
    for (name, v) in options.iter() {
        let text = str_of(v.value);
        match (unit, name) {
            (false, "currency") => match text.and_then(currency_code) {
                // MUST NOT override the currency of a value that has one.
                Some(_) if inherited.is_some() => errs.error(FormatError::BadOption),
                Some(c) => what = Some(MeasureUnit::Currency(c)),
                None => errs.error(FormatError::BadOption),
            },
            (true, "unit") => match text.filter(|u| is_unit_id(u)).map(MeasureUnit::Unit) {
                // No other unit without converting (number.md).
                Some(u) if inherited.is_some_and(|(i, _)| i != u) => {
                    errs.error(FormatError::BadOption);
                }
                Some(u) => what = Some(u),
                None => errs.error(FormatError::BadOption),
            },
            (false, "currencyDisplay") => {
                keyword(text, &DISPLAYS, DISPLAY_SHIFT, 3, &mut flags, errs);
            }
            (false, "currencySign") => keyword(text, &SIGNS, SIGN_SHIFT, 2, &mut flags, errs),
            (true, "unitDisplay") => keyword(text, &WIDTHS, WIDTH_SHIFT, 2, &mut flags, errs),
            (false, "fractionDigits") => {
                if text == Some("auto") {
                    set(&mut flags, DIGITS_SHIFT, 7, 1);
                } else if let Some(n) = v.value.digit_size() {
                    set(&mut flags, DIGITS_SHIFT, 7, 2 + u32::from(n));
                } else {
                    errs.error(FormatError::BadOption);
                }
            }
            // Conversion is optional and not implemented (number.md,
            // "Unit Conversion"): the value keeps its unit.
            (true, "usage") => errs.error(FormatError::UnsupportedOperation),
            _ => {}
        }
    }
    let Some(what) = what else {
        // A numeric operand without a currency or unit (number.md).
        errs.error(FormatError::BadOperand);
        return None;
    };
    let spec = match what {
        MeasureUnit::Currency(code) => NumberSpec::currency(match field(flags, DIGITS_SHIFT, 7) {
            n if n >= 2 => u8::try_from(n - 2).unwrap_or(2),
            // `intl`: the currency's own digits are the formatter's
            // (`NumberStyle::Currency::own_digits`); the catalog's currency
            // data is not read.
            _ if INTL_NUMBERS => 2,
            _ => auto_digits(cx.catalog(), code),
        }),
        MeasureUnit::Unit(_) => NumberSpec::UNIT,
    };
    let number = Number::resolve(spec, cx, operand, &options, errs)?;
    Some(Value::Measure(Measure::new(number, what, flags)))
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
        resolve_measure(false, cx, operand, *options, errs)
    }

    fn formattable(&self, _cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        match value {
            Value::Measure(m) if matches!(m.unit, MeasureUnit::Currency(_)) => Ok(()),
            _ => Err(FormatError::MessageFunctionError),
        }
    }

    fn format(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn Sink) {
        if let Value::Measure(m) = value {
            if INTL_NUMBERS {
                return intl::measure(cx, m, NumberOut::Text(out));
            }
            write_currency(cx.catalog(), m, &mut Out::Text(out));
        }
    }

    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if let Value::Measure(m) = value {
            if INTL_NUMBERS {
                return intl::measure(cx, m, NumberOut::Parts(out));
            }
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
    let own = cur
        .and_then(|c| c.pattern())
        .filter(|_| base == Style::Currency);
    let mut pattern = own.or_else(|| patterns.and_then(|p| p.resolve(base)));
    // Where a letter-like end of the symbol touches the digits, CLDR's
    // currency spacing puts a U+00A0 between them — or, when the locale has
    // one, the `…alphaNextToNumber` pattern places the space itself. No
    // currency pattern in the catalog: the symbol goes before the number.
    let (mut before, mut after) = pattern.map_or((last, false), |p| {
        let s = p.signed(localize::shown(&d));
        (
            last && ends_with_currency(s.prefix),
            first && starts_with_currency(s.suffix),
        )
    });
    if own.is_none()
        && (before || after)
        && let Some(a) = alpha.and_then(|a| patterns.and_then(|p| p.get(a)))
    {
        pattern = Some(a);
        (before, after) = (false, false);
    }
    let sizes = pattern.map_or(sym.grouping(), |p| p.grouping());
    let symbol = Symbol {
        text,
        before,
        after,
    };
    if pattern.is_none() {
        out.put("currency", text);
        if before {
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
    let mut number = |out: &mut Out<'_>| {
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
        fill(t, out, &mut number, &mut |out| out.put("currency", name));
    } else {
        number(out);
        out.put("literal", " ");
        out.put("currency", name);
    }
}

/// Writes `t` with `arg0` for `{0}` and `arg1` for `{1}`; a text's blank
/// ends are `literal` parts, the rest `unit` (Intl's parts for unit and
/// currency-name patterns).
fn fill(
    t: Template<'_>,
    out: &mut Out<'_>,
    arg0: &mut dyn FnMut(&mut Out<'_>),
    arg1: &mut dyn FnMut(&mut Out<'_>),
) {
    for p in t.parts() {
        match p {
            TemplatePart::Arg0 => arg0(out),
            TemplatePart::Arg1 => arg1(out),
            TemplatePart::Text(s) => {
                let (lead, text, trail) = split_blank(s);
                out.put("literal", lead);
                out.put("unit", text);
                out.put("literal", trail);
            }
        }
    }
}

/// The spaces of CLDR's unit and name patterns: U+0020, U+00A0, U+2009,
/// U+202F.
const BLANKS: [&str; 4] = [" ", "\u{a0}", "\u{2009}", "\u{202f}"];

/// `text` as its leading blanks, the text between, and its trailing blanks.
fn split_blank(text: &str) -> (&str, &str, &str) {
    let mut rest = text;
    while let Some(r) = BLANKS.iter().find_map(|b| rest.strip_prefix(b)) {
        rest = r;
    }
    let (head, mut mid) = text
        .split_at_checked(text.len() - rest.len())
        .unwrap_or(("", text));
    while let Some(r) = BLANKS.iter().find_map(|b| mid.strip_suffix(b)) {
        mid = r;
    }
    let tail = rest.get(mid.len()..).unwrap_or("");
    (head, mid, tail)
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
        resolve_measure(true, cx, operand, *options, errs)
    }

    fn formattable(&self, cx: &FnContext<'_>, value: &Value<'_>) -> Result<(), FormatError> {
        match value {
            Value::Measure(m) => match m.unit {
                MeasureUnit::Unit(_) if INTL_NUMBERS => intl::unit_formattable(cx, m),
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
            if INTL_NUMBERS {
                return intl::measure(cx, m, NumberOut::Text(out));
            }
            write_unit(cx.catalog(), m, &mut Out::Text(out));
        }
    }

    fn format_parts(&self, cx: &FnContext<'_>, value: &Value<'_>, out: &mut dyn SubPartSink) {
        if let Value::Measure(m) = value {
            if INTL_NUMBERS {
                return intl::measure(cx, m, NumberOut::Parts(out));
            }
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
    // Not formattable (`formattable` said so): nothing.
    let (unit, per) = match resolve_unit(&units, id) {
        Some(Resolved::One(u)) => (u, None),
        Some(Resolved::Per(x, y)) => (x, Some(y)),
        None => return,
    };
    let seps = Seps::of(&sym);
    let grouping = measure.number.grouping();
    let cat = category(catalog, &digits) as u8;
    let mut number = |out: &mut Out<'_>| {
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
    let w = width_for(&unit, width_of(measure.flags));
    let mut numerator = |out: &mut Out<'_>| match unit.pattern(w, cat) {
        Some(t) => fill(t, out, &mut number, &mut |_| {}),
        None => number(out),
    };
    let Some(den) = per else {
        return numerator(out);
    };
    if let Some(p) = den.per_unit_pattern(w) {
        return fill(p, out, &mut numerator, &mut |_| {});
    }
    // Else the locale's `per` pattern (`{0}/{1}`) with the denominator's
    // singular form without its number.
    let Some(p) = units.per_pattern(w) else {
        return numerator(out);
    };
    let denominator = den.pattern(w, Category::One as u8);
    fill(p, out, &mut numerator, &mut |out| {
        for part in denominator.iter().flat_map(Template::parts) {
            if let TemplatePart::Text(s) = part {
                out.put("unit", split_blank(s).1);
            }
        }
    });
}
