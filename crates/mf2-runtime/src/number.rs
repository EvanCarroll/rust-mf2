//! The core numeric semantics (`number.md`; `plans/03-runtime.md` §5.1):
//! `:number`, `:integer` and `:offset` — operand rules, every digit and
//! rounding option, `signDisplay`, option inheritance, `select` = `exact` /
//! `plural` / `ordinal`, the exact-match serialization and the plural
//! operands of the *formatted* number — with neutral output (ASCII digits,
//! `.`, `-`/`+`, no grouping). Ported from P0.5 over the digit backend of
//! [`decimal`].

mod decimal;
mod options;

use mf2_catalog::format::locale_key;

use crate::error::FormatError;
use crate::function::{FnContext, Options};
use crate::host::Host;
use crate::plural::{self, Category, OperandsBuilder};
use crate::sink::{ErrorSink, Sink, SubPartSink};
use crate::value::Value;

use decimal::{Decimal, Increment, ParseError, split_literal};
use options::{
    DigitPlan, FracDefaults, INT, Kind, NUM, NumOpts, OPTIONS, PCT, RoundingType, Select,
    SignDisplay, apply, digit_plan, digit_size, select_named,
};

/// An exact decimal and, once a numeric handler resolved it, its resolved
/// options and its display form. Opaque: the digit backend is internal
/// (owner decision 1).
#[derive(Clone)]
pub struct Number {
    /// The numeric value (after `:integer`'s rounding and `:offset`'s
    /// adjustment), unscaled.
    pub(crate) value: Decimal,
    /// Set by a numeric handler: the resolved state; `None` for a bare number.
    resolved: Option<Resolved>,
}

/// What a numeric handler resolved.
#[derive(Clone)]
struct Resolved {
    opts: NumOpts,
    frac: FracDefaults,
    /// A power of ten applied when formatting and selecting (P4 `:percent`).
    scale: i16,
    /// `false` when `select` came from a variable or from the operand.
    selectable: bool,
    display: Display,
}

/// The digits to display: rounded, with the visible magnitude range and the
/// sign after `signDisplay`.
#[derive(Clone)]
struct Display {
    dec: Decimal,
    /// The lowest visible magnitude (≤ 0).
    lo: i16,
    /// The highest visible magnitude (≥ 0).
    hi: i16,
    sign: SignOut,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SignOut {
    None,
    Minus,
    Plus,
}

impl Number {
    /// A `number-literal`; `None` if `s` is not one (or is past the
    /// implementation limits: 40 significant digits, exponent ±9999).
    pub fn parse(number_literal: &str) -> Option<Number> {
        Decimal::parse(number_literal.as_bytes())
            .ok()
            .map(Number::bare_of)
    }

    /// `n`.
    pub fn from_i64(n: i64) -> Number {
        let mut d = Decimal::from_u64(n.unsigned_abs());
        d.set_negative(n < 0);
        Number::bare_of(d)
    }

    /// The finite `x` (the shortest decimal that round-trips it, through
    /// the host); `None` for NaN and infinities.
    pub fn from_f64(x: f64, host: &dyn Host) -> Option<Number> {
        if !x.is_finite() {
            return None;
        }
        if x == 0.0 {
            let mut d = Decimal::from_u64(0);
            d.set_negative(x.is_sign_negative());
            return Some(Number::bare_of(d));
        }
        let mut buf = [0u8; 32];
        let text = host.f64_to_text(x, &mut buf)?;
        Number::parse(text)
    }

    /// Whether the value is negative (`-0` included).
    pub fn is_negative(&self) -> bool {
        self.value.negative()
    }

    /// Whether the value is an integer.
    pub fn is_integer(&self) -> bool {
        self.value.is_integer()
    }

    /// The value as an `i64`, when it is an integer below 10^18 in magnitude.
    pub fn to_i64(&self) -> Option<i64> {
        self.value.to_i64()
    }

    /// Writes the exact value in plain neutral digits: `-1234.5`, `0.001`,
    /// `-0` — no exponent, no rounding.
    pub fn write_plain(&self, out: &mut dyn Sink) {
        self.value.write_plain(out);
    }

    fn bare_of(value: Decimal) -> Number {
        Number {
            value,
            resolved: None,
        }
    }

    /// The value alone, without resolved options.
    pub(crate) fn bare(&self) -> Number {
        Number::bare_of(self.value.clone())
    }

    /// The plain value as sub-parts (`minusSign`, `integer`, `decimal`,
    /// `fraction`).
    pub(crate) fn plain_parts(&self, out: &mut dyn SubPartSink) {
        let d = &self.value;
        let sign = if d.negative() {
            SignOut::Minus
        } else {
            SignOut::None
        };
        write_parts(d, sign, d.high().max(0), d.low().min(0), out);
    }

    /// Whether a numeric handler resolved this number and it may select.
    pub(crate) fn selectable(&self) -> bool {
        self.resolved.as_ref().is_some_and(|r| r.selectable)
    }

    /// Writes the display form (a bare number: its plain value).
    pub(crate) fn write_display(&self, out: &mut dyn Sink) {
        match &self.resolved {
            Some(r) => {
                let d = &r.display;
                match d.sign {
                    SignOut::Minus => out.push_str("-"),
                    SignOut::Plus => out.push_str("+"),
                    SignOut::None => {}
                }
                d.dec.write_digits(d.hi, d.lo, out);
            }
            None => self.write_plain(out),
        }
    }

    /// The display form as sub-parts.
    pub(crate) fn display_parts(&self, out: &mut dyn SubPartSink) {
        match &self.resolved {
            Some(r) => write_parts(
                &r.display.dec,
                r.display.sign,
                r.display.hi,
                r.display.lo,
                out,
            ),
            None => self.plain_parts(out),
        }
    }
}

/// Writes digits `hi..=lo` of `d` as Intl-style sub-parts, in chunks of at
/// most 64 digits (a longer run arrives as several parts of one kind).
fn write_parts(d: &Decimal, sign: SignOut, hi: i16, lo: i16, out: &mut dyn SubPartSink) {
    match sign {
        SignOut::Minus => out.sub_part("minusSign", "-"),
        SignOut::Plus => out.sub_part("plusSign", "+"),
        SignOut::None => {}
    }
    digits_part(d, hi, 0, "integer", out);
    if lo < 0 {
        out.sub_part("decimal", ".");
        digits_part(d, -1, lo, "fraction", out);
    }
}

fn digits_part(d: &Decimal, hi: i16, lo: i16, kind: &str, out: &mut dyn SubPartSink) {
    let mut buf = [0u8; 64];
    let mut n = 0;
    let mut m = hi;
    while m >= lo {
        if let Some(b) = buf.get_mut(n) {
            *b = b'0' + d.digit_at(m);
            n += 1;
        }
        if n == buf.len() {
            out.sub_part(kind, core::str::from_utf8(&buf).unwrap_or(""));
            n = 0;
        }
        m -= 1;
    }
    if n > 0 {
        out.sub_part(
            kind,
            core::str::from_utf8(buf.get(..n).unwrap_or(&[])).unwrap_or(""),
        );
    }
}

/// The error a `number-literal` operand gives, if any: *Bad Operand* for
/// one that does not match the production, *Unsupported Operation* past the
/// implementation limits.
pub(crate) fn literal_error(s: &str) -> Option<FormatError> {
    match Decimal::parse(s.as_bytes()) {
        Ok(_) => None,
        Err(ParseError::Syntax) => Some(FormatError::BadOperand),
        Err(ParseError::Limit) => Some(FormatError::UnsupportedOperation),
    }
}

/// Whether `key` matches the production `number-literal` (past our limits
/// included: such a key is a number, it just never equals a value).
fn is_number_literal(key: &str) -> bool {
    !matches!(split_literal(key.as_bytes()), Err(ParseError::Syntax))
}

// ─────────────────────────────────────────────────────────── resolution ──

/// How one numeric function resolves (closed world: a handler is a `Spec`).
#[derive(Clone, Copy)]
pub(crate) struct Spec {
    /// Option-set bit (`NUM`, `INT`, `PCT`, `CUR`, `UNIT`); 0 = `:offset`.
    bit: u8,
    /// Fraction-digit defaults; `None` keeps the operand's (`:offset`).
    frac: Option<FracDefaults>,
    /// Round the value to an integer (`:integer`).
    integer: bool,
    /// Supports selection (`:currency` and `:unit` do not).
    selectable: bool,
    /// Power of ten applied when formatting and selecting (`:percent` = 2).
    scale: i16,
}

pub(crate) const NUMBER: Spec = Spec {
    bit: NUM,
    frac: Some(FracDefaults { min: 0, max: 3 }),
    integer: false,
    selectable: true,
    scale: 0,
};

pub(crate) const INTEGER: Spec = Spec {
    bit: INT,
    frac: Some(FracDefaults { min: 0, max: 0 }),
    integer: true,
    selectable: true,
    scale: 0,
};

pub(crate) const OFFSET: Spec = Spec {
    bit: 0,
    frac: None,
    integer: false,
    selectable: true,
    scale: 0,
};

/// Resolves a numeric expression; `None` = a fallback value (reported).
pub(crate) fn resolve(
    spec: Spec,
    cx: &FnContext<'_>,
    operand: Option<&Value<'_>>,
    options: Options<'_, '_>,
    errs: &mut dyn ErrorSink,
) -> Option<Number> {
    let is_offset = spec.bit == 0;
    let Some(operand) = operand else {
        errs.error(FormatError::BadOperand);
        return None;
    };
    let (mut value, mut o, inherited, mut selectable) = match operand {
        Value::Number(Number {
            value,
            resolved: Some(r),
        }) => {
            let mut o = r.opts;
            let mut selectable = r.selectable;
            if o.select.is_some() {
                // `select` set by an operand: Bad Option and no selection for
                // the selecting functions (number.md, "Number Selection");
                // `:offset` passes it on.
                if spec.bit & (NUM | INT) != 0 {
                    errs.error(FormatError::BadOption);
                    selectable = false;
                }
                if !is_offset {
                    o.select = None;
                }
            }
            if spec.bit == INT {
                o.min_frac = None;
                o.max_frac = None;
                o.min_sig = None;
            }
            if spec.bit == PCT {
                o.min_int = None;
                o.rounding_increment = None;
            }
            (value.clone(), o, Some((r.frac, r.scale)), selectable)
        }
        other => match numeric_operand(other, cx.host()) {
            Ok(d) => (d, NumOpts::default(), None, true),
            Err(e) => {
                errs.error(e);
                return None;
            }
        },
    };
    let mut offset: Option<(bool, u8)> = None;
    let mut offset_bad = false;
    for (name, v) in options.iter() {
        if is_offset {
            let sub = match name {
                "add" => false,
                "subtract" => true,
                _ => continue, // other options are ignored
            };
            match (digit_size(v.value), offset) {
                (Some(d), None) => offset = Some((sub, d)),
                _ => offset_bad = true,
            }
            continue;
        }
        let Some(kind) = OPTIONS
            .iter()
            .find(|(n, mask, _)| *n == name && mask & spec.bit != 0)
            .map(|&(_, _, k)| k)
        else {
            continue; // not an option of this function: ignored
        };
        if kind == Kind::Select {
            // MUST be a literal; otherwise Bad Option and no selection.
            if v.literal {
                match v.value.as_str().and_then(select_named) {
                    Some(x) => o.select = Some(x),
                    None => errs.error(FormatError::BadOption),
                }
            } else {
                errs.error(FormatError::BadOption);
                selectable = false;
            }
            continue;
        }
        if !apply(&mut o, kind, v.value) {
            errs.error(FormatError::BadOption);
        }
    }
    if spec.integer {
        // The resolved value of `:integer` is the integer value.
        let mode = o.rounding_mode.unwrap_or(decimal::RoundingMode::HalfExpand);
        value.round(0, mode, Increment::One);
    }
    let (frac, scale) = match (spec.frac, inherited) {
        (Some(f), _) => (f, spec.scale),
        (None, Some(inherited)) => inherited,
        (None, None) => (FracDefaults { min: 0, max: 3 }, 0),
    };
    if is_offset {
        let Some((sub, d)) = offset.filter(|_| !offset_bad) else {
            errs.error(FormatError::BadOption);
            return None;
        };
        let mut delta = Decimal::from_u64(u64::from(d));
        delta.set_negative(sub);
        let Some(sum) = value.add(&delta) else {
            // An implementation limit (number.md allows Unsupported Operation).
            errs.error(FormatError::UnsupportedOperation);
            return None;
        };
        value = sum;
    }
    let plan = digit_plan(&o, frac, errs);
    let display = display(
        &value,
        scale,
        &plan,
        o.sign_display.unwrap_or(SignDisplay::Auto),
    );
    Some(Number {
        value,
        resolved: Some(Resolved {
            opts: o,
            frac,
            scale,
            selectable: selectable && spec.selectable,
            display,
        }),
    })
}

/// The numeric value of an operand that is not a resolved number.
fn numeric_operand(v: &Value<'_>, host: &dyn Host) -> Result<Decimal, FormatError> {
    match v {
        Value::Str(s) | Value::Decimal(s) => Decimal::parse(s.as_bytes()).map_err(|e| match e {
            ParseError::Syntax => FormatError::BadOperand,
            ParseError::Limit => FormatError::UnsupportedOperation,
        }),
        Value::Int(n) => {
            let mut d = Decimal::from_u64(n.unsigned_abs());
            d.set_negative(*n < 0);
            Ok(d)
        }
        _ => v
            .to_number(host)
            .map(|n| n.value)
            .ok_or(FormatError::BadOperand),
    }
}

// ──────────────────────────────────────────────────────────── display ──

/// ECMA-402 `ToRawFixed`: `(rounded, rounding magnitude)`.
fn raw_fixed(x: &Decimal, p: &DigitPlan) -> (Decimal, i16, i16) {
    let mut d = x.clone();
    let (inc, k) = p.increment;
    let max = -i16::from(p.max_frac);
    d.round(max + k, p.mode, inc);
    let lo = (-i16::from(p.min_frac)).min(d.low()).min(0);
    (d, lo, max)
}

/// ECMA-402 `ToRawPrecision`: `(rounded, lowest visible, rounding magnitude)`.
fn raw_precision(x: &Decimal, p: &DigitPlan) -> (Decimal, i16, i16) {
    let mut d = x.clone();
    let e = d.high();
    d.round(e - i16::from(p.max_sig) + 1, p.mode, Increment::One);
    let e2 = d.high();
    let lo = (e2 - i16::from(p.min_sig) + 1).min(d.low()).min(0);
    (d, lo, e2 - i16::from(p.max_sig) + 1)
}

fn display(value: &Decimal, scale: i16, p: &DigitPlan, sign: SignDisplay) -> Display {
    let mut x = value.clone();
    if scale != 0 {
        x.shift(scale);
    }
    let (d, mut lo, _) = match p.ty {
        RoundingType::Fraction => raw_fixed(&x, p),
        RoundingType::Significant => raw_precision(&x, p),
        RoundingType::More | RoundingType::Less => {
            let s = raw_precision(&x, p);
            let f = raw_fixed(&x, p);
            let fixed_more_precise = f.2 < s.2;
            if (p.ty == RoundingType::More) == fixed_more_precise {
                f
            } else {
                s
            }
        }
    };
    if p.strip_if_integer && d.is_integer() {
        lo = 0;
    }
    let hi = d.high().max(0).max(i16::from(p.min_int) - 1);
    let zero = d.is_zero();
    let negative = d.negative();
    let sign = match sign {
        SignDisplay::ExceptZero if zero => SignOut::None,
        SignDisplay::Auto | SignDisplay::Always | SignDisplay::ExceptZero if negative => {
            SignOut::Minus
        }
        SignDisplay::Always | SignDisplay::ExceptZero => SignOut::Plus,
        SignDisplay::Negative if negative && !zero => SignOut::Minus,
        _ => SignOut::None,
    };
    Display {
        dec: d,
        lo,
        hi,
        sign,
    }
}

// ────────────────────────────────────────────────────────── selection ──

/// Compares written bytes with a key.
struct Cmp<'k> {
    key: &'k [u8],
    pos: usize,
    ok: bool,
}

impl Sink for Cmp<'_> {
    fn push_str(&mut self, s: &str) {
        let end = self.pos + s.len();
        self.ok &= self.key.get(self.pos..end) == Some(s.as_bytes());
        self.pos = end;
    }
}

impl Cmp<'_> {
    fn done(&self) -> bool {
        self.ok && self.pos == self.key.len()
    }
}

/// Whether `key` equals the text `write` produces.
pub(crate) fn equals(key: &str, write: impl FnOnce(&mut dyn Sink)) -> bool {
    let mut c = Cmp {
        key: key.as_bytes(),
        pos: 0,
        ok: true,
    };
    write(&mut c);
    c.done()
}

/// The exact-match serialization (number.md): the integer form when the
/// value is an integer and none of the min-fraction, min-integer, min/max-
/// significant options is set; otherwise (implementation-defined) the
/// display form.
fn exact_matches(n: &Number, r: &Resolved, key: &str) -> bool {
    let o = &r.opts;
    let plain =
        o.min_frac.is_none() && o.min_int.is_none() && o.min_sig.is_none() && o.max_sig.is_none();
    let mut value = n.value.clone();
    if r.scale != 0 {
        value.shift(r.scale);
    }
    if plain && value.is_integer() {
        if value.is_zero() {
            value.set_negative(false);
        }
        equals(key, |s| value.write_plain(s))
    } else {
        equals(key, |s| n.write_display(s))
    }
}

/// The plural operands of the display form.
fn operands(d: &Display) -> plural::Operands {
    let mut b = OperandsBuilder::default();
    let mut m = d.hi;
    while m >= d.lo {
        b.digit(d.dec.digit_at(m), m >= 0);
        m -= 1;
    }
    b.finish()
}

/// Match(`n`, `key`) for numeric selectors (number.md, "Number Selection").
pub(crate) fn matches(n: &Number, cx: &FnContext<'_>, key: &str, errs: &mut dyn ErrorSink) -> bool {
    let Some(r) = &n.resolved else {
        return false;
    };
    if is_number_literal(key) {
        return exact_matches(n, r, key);
    }
    let Some(keyword) = Category::from_keyword(key) else {
        errs.error(FormatError::BadVariantKey);
        return false;
    };
    let entry_key = match r.opts.select.unwrap_or(Select::Plural) {
        Select::Exact => return false,
        Select::Plural => locale_key::PLURAL_CARDINAL,
        Select::Ordinal => locale_key::PLURAL_ORDINAL,
    };
    let rules = cx.catalog().locale_entry(entry_key).unwrap_or(&[]);
    plural::select(rules, &operands(&r.display)) == keyword
}

/// `BetterThan(n, key1, key2)` for two matching keys: an exact
/// (numeric) key beats a keyword.
pub(crate) fn better_than(key1: &str, key2: &str) -> bool {
    is_number_literal(key1) && !is_number_literal(key2)
}

#[cfg(test)]
mod tests;
