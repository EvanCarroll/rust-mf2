//! The core numeric semantics (`number.md`):
//! `:number`, `:integer` and `:offset` — operand rules, every digit and
//! rounding option, `signDisplay`, option inheritance, `select` = `exact` /
//! `plural` / `ordinal`, the exact-match serialization and the plural
//! operands of the *formatted* number — with neutral output (ASCII digits,
//! `.`, `-`/`+`, no grouping). Ported from P0.5 over the digit backend of
//! [`decimal`].
//!
//! The display, `:integer`'s rounding and the plural category come from a
//! backend with one interface: the Rust one (`display.rs`), or — feature
//! `intl` on `wasm32-unknown-unknown`, [`crate::INTL_NUMBERS`] — the host's
//! number formatter (`intl.rs`), and
//! then the Rust rounding, digit output and plural evaluator are not linked.

mod decimal;
#[cfg(not(all(feature = "intl", target_arch = "wasm32", target_os = "unknown")))]
mod display;
#[cfg(all(feature = "intl", target_arch = "wasm32", target_os = "unknown"))]
mod intl;
mod measure;
mod options;
mod request;

#[cfg(not(all(feature = "intl", target_arch = "wasm32", target_os = "unknown")))]
use display as backend;
#[cfg(all(feature = "intl", target_arch = "wasm32", target_os = "unknown"))]
use intl as backend;

use crate::error::FormatError;
use crate::function::{FnContext, Options};
use crate::host::Host;
use crate::plural::{self, Category, OperandsBuilder};
use crate::sink::{ErrorSink, Sink, SubPartSink};
use crate::value::Value;

use decimal::{Decimal, ParseError, split_literal};
use options::{
    CUR, FracDefaults, INT, Kind, NUM, NumOpts, OPTIONS, PCT, Select, UNIT, apply, digit_plan,
    select_named,
};

pub(crate) use options::digit_size;

pub use decimal::RoundingMode;
pub use measure::{Measure, MeasureUnit};
pub use options::{Grouping, RoundingPriority, SignDisplay};
pub use request::{
    CurrencyDisplay, DigitOptions, NumberOut, NumberRequest, NumberStyle, UnitDisplay,
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
    /// What the backend keeps: the rounded display (Rust), or the digit
    /// plan and the plural category once asked (`intl`).
    shown: backend::Shown,
}

/// The sign, whether the value is an integer, and whether a numeric handler
/// resolved it; not the digits, whose writer a client's formatting calls,
/// so that a client's build compiles that as before.
impl core::fmt::Debug for Number {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Number")
            .field("negative", &self.value.negative())
            .field("resolved", &self.resolved.is_some())
            .finish_non_exhaustive()
    }
}

/// The sign a formatted number shows, after `signDisplay`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sign {
    /// No sign.
    None,
    /// A minus sign.
    Minus,
    /// A plus sign.
    Plus,
}

/// Digits to show: a resolved number's
/// rounded display digits ([`Number::digits`]), or a number's exact value
/// ([`Number::exact_digits`]) — what `mf2-fn-number` localizes.
#[derive(Clone, Copy)]
pub struct Digits<'n> {
    dec: &'n Decimal,
    lo: i16,
    hi: i16,
    sign: Sign,
}

impl Digits<'_> {
    /// The sign (for display digits, after `signDisplay`).
    pub fn sign(&self) -> Sign {
        self.sign
    }

    /// The number of integer digits shown (≥ 1; for display digits, after
    /// `minimumIntegerDigits`).
    pub fn integer_count(&self) -> u16 {
        u16::try_from(i32::from(self.hi) + 1).unwrap_or(1)
    }

    /// The number of fraction digits shown.
    pub fn fraction_count(&self) -> u16 {
        u16::try_from(-i32::from(self.lo)).unwrap_or(0)
    }

    /// The digit (0–9) worth 10^`magnitude`: the integer digits are at
    /// magnitudes `0 .. integer_count`, the fraction digits at `-1 ..=
    /// -fraction_count`; 0 anywhere else.
    pub fn digit(&self, magnitude: i16) -> u8 {
        if magnitude < self.lo || magnitude > self.hi {
            return 0;
        }
        self.dec.digit_at(magnitude)
    }

    /// Whether every digit is 0.
    pub fn is_zero(&self) -> bool {
        self.dec.is_zero()
    }

    /// Writes the core's neutral output: ASCII digits, `.`, `-`/`+`, no
    /// grouping.
    pub fn write_neutral(&self, out: &mut dyn Sink) {
        match self.sign {
            Sign::Minus => out.push_str("-"),
            Sign::Plus => out.push_str("+"),
            Sign::None => {}
        }
        self.dec.write_digits(self.hi, self.lo, out);
    }

    /// The neutral output as sub-parts (`minusSign`, `plusSign`, `integer`,
    /// `decimal`, `fraction`).
    pub fn neutral_parts(&self, out: &mut dyn SubPartSink) {
        write_parts(self.dec, self.sign, self.hi, self.lo, out);
    }

    /// The CLDR plural operands of these digits as shown (`1.0` has `v = 1`):
    /// what a unit or currency name's plural form is chosen by
    /// ([`crate::plural_category`]).
    pub fn operands(&self) -> plural::Operands {
        let mut b = OperandsBuilder::default();
        let mut m = self.hi;
        while m >= self.lo {
            b.digit(self.dec.digit_at(m), m >= 0);
            m -= 1;
        }
        b.finish()
    }
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

    /// Resolves an expression under `spec`, as `:number` does
    /// (`number.md`): the operand rules, the options of `spec`'s function
    /// (any other option is ignored), inheritance from a number or measure
    /// operand, the digit plan and rounding. `None`: a fallback value, the
    /// reason reported through `errs`.
    pub fn resolve(
        spec: NumberSpec,
        cx: &FnContext<'_>,
        operand: Option<&Value<'_>>,
        options: &Options<'_, '_>,
        errs: &mut dyn ErrorSink,
    ) -> Option<Number> {
        resolve(spec, cx, operand, *options, errs)
    }

    /// The rounded digits to display; `None` for a number no handler
    /// resolved — and, with feature `intl` on `wasm32-unknown-unknown`
    /// ([`crate::INTL_NUMBERS`]), always: the host rounds and formats
    /// ([`Number::format_by_host`]).
    pub fn digits(&self) -> Option<Digits<'_>> {
        self.resolved.as_ref().and_then(backend::digits)
    }

    /// The exact value's digits, unrounded — how an unannotated number
    /// formats: `-1234.5`, `0.001`, `-0`.
    pub fn exact_digits(&self) -> Digits<'_> {
        let d = &self.value;
        Digits {
            dec: d,
            lo: d.low().min(0),
            hi: d.high().max(0),
            sign: if d.negative() {
                Sign::Minus
            } else {
                Sign::None
            },
        }
    }

    /// The resolved `useGrouping`; `None` when it is not set (or the number
    /// is bare).
    pub fn grouping(&self) -> Option<Grouping> {
        self.resolved.as_ref().and_then(|r| r.opts.use_grouping)
    }

    /// Whether a numeric handler resolved this number and it may select
    /// (formatting.md, "Resolve Selectors").
    pub fn is_selectable(&self) -> bool {
        self.selectable()
    }

    /// Match(`self`, `key`) for numeric selectors (`number.md`, "Number
    /// Selection"): exact numeric keys, then the plural or ordinal category
    /// of the formatted digits; *Bad Variant Key* for any other key.
    pub fn matches(&self, cx: &FnContext<'_>, key: &str, errs: &mut dyn ErrorSink) -> bool {
        matches(self, cx, key, errs)
    }

    /// `BetterThan(key1, key2)` for two matching keys: an exact (numeric)
    /// key beats a keyword.
    pub fn better_than(key1: &str, key2: &str) -> bool {
        better_than(key1, key2)
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
        self.exact_digits().neutral_parts(out);
    }

    /// Whether a numeric handler resolved this number and it may select.
    pub(crate) fn selectable(&self) -> bool {
        self.resolved.as_ref().is_some_and(|r| r.selectable)
    }

    /// Writes the display form in neutral symbols (a bare number: its
    /// plain value).
    pub(crate) fn write_display(&self, cx: &FnContext<'_>, out: &mut dyn Sink) {
        backend::write_neutral(self, cx, out);
    }

    /// The display form as sub-parts.
    pub(crate) fn display_parts(&self, cx: &FnContext<'_>, out: &mut dyn SubPartSink) {
        backend::neutral_parts(self, cx, out);
    }
}

/// Writes digits `hi..=lo` of `d` as Intl-style sub-parts, in chunks of at
/// most 64 digits (a longer run arrives as several parts of one kind).
fn write_parts(d: &Decimal, sign: Sign, hi: i16, lo: i16, out: &mut dyn SubPartSink) {
    match sign {
        Sign::Minus => out.sub_part("minusSign", "-"),
        Sign::Plus => out.sub_part("plusSign", "+"),
        Sign::None => {}
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

/// How one numeric function resolves: which
/// options it reads, its fraction-digit defaults, whether it rounds to an
/// integer, whether it selects, and the power of ten it applies. Closed
/// world: a handler is a spec.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NumberSpec {
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

impl NumberSpec {
    /// `:number`: 0–3 fraction digits, selects.
    pub const NUMBER: NumberSpec = NumberSpec {
        bit: NUM,
        frac: Some(FracDefaults { min: 0, max: 3 }),
        integer: false,
        selectable: true,
        scale: 0,
    };

    /// `:integer`: the value rounded to an integer, selects.
    pub const INTEGER: NumberSpec = NumberSpec {
        bit: INT,
        frac: Some(FracDefaults { min: 0, max: 0 }),
        integer: true,
        selectable: true,
        scale: 0,
    };

    /// `:offset`: `add` / `subtract`, the operand's other options kept.
    pub const OFFSET: NumberSpec = NumberSpec {
        bit: 0,
        frac: None,
        integer: false,
        selectable: true,
        scale: 0,
    };

    /// `:percent`: the value × 100 when formatting and selecting, 0–0
    /// fraction digits, plural selection only.
    pub const PERCENT: NumberSpec = NumberSpec {
        bit: PCT,
        frac: Some(FracDefaults { min: 0, max: 0 }),
        integer: false,
        selectable: true,
        scale: 2,
    };

    /// `:unit`: 0–3 fraction digits, not selectable.
    pub const UNIT: NumberSpec = NumberSpec {
        bit: UNIT,
        frac: Some(FracDefaults { min: 0, max: 3 }),
        integer: false,
        selectable: false,
        scale: 0,
    };

    /// `:currency` whose currency shows `fraction_digits` fraction digits
    /// (`fractionDigits`, or the currency's own for `auto`); not
    /// selectable.
    pub const fn currency(fraction_digits: u8) -> NumberSpec {
        NumberSpec {
            bit: CUR,
            frac: Some(FracDefaults {
                min: fraction_digits,
                max: fraction_digits,
            }),
            integer: false,
            selectable: false,
            scale: 0,
        }
    }
}

/// Resolves a numeric expression; `None` = a fallback value (reported).
pub(crate) fn resolve(
    spec: NumberSpec,
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
    let resolved_operand = match operand {
        Value::Number(n) | Value::Measure(Measure { number: n, .. }) => {
            n.resolved.as_ref().map(|r| (&n.value, r))
        }
        _ => None,
    };
    let (mut value, mut o, inherited, mut selectable) = match resolved_operand {
        Some((value, r)) => {
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
            if spec.bit == CUR {
                // `fractionDigits` decides (NumberSpec::currency).
                o.min_frac = None;
                o.max_frac = None;
            }
            (value.clone(), o, Some((r.frac, r.scale)), selectable)
        }
        None => match numeric_operand(operand, cx.host()) {
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
        let mode = o.rounding_mode.unwrap_or(RoundingMode::HalfExpand);
        backend::round_to_integer(&mut value, mode, cx);
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
    backend::limit_digit_sizes(&mut o, errs);
    let plan = digit_plan(&o, frac, errs);
    let shown = backend::shown(
        &value,
        scale,
        &plan,
        o.sign_display.unwrap_or(SignDisplay::Auto),
    );
    backend::check_host(cx, errs);
    Some(Number {
        value,
        resolved: Some(Resolved {
            opts: o,
            frac,
            scale,
            selectable: selectable && spec.selectable,
            shown,
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
/// display form, in neutral symbols.
fn exact_matches(n: &Number, r: &Resolved, cx: &FnContext<'_>, key: &str) -> bool {
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
        equals(key, |s| backend::write_selected(n, r, cx, s))
    }
}

/// Match(`n`, `key`) for numeric selectors (number.md, "Number Selection").
pub(crate) fn matches(n: &Number, cx: &FnContext<'_>, key: &str, errs: &mut dyn ErrorSink) -> bool {
    let Some(r) = &n.resolved else {
        return false;
    };
    if is_number_literal(key) {
        return exact_matches(n, r, cx, key);
    }
    let Some(keyword) = Category::from_keyword(key) else {
        errs.error(FormatError::BadVariantKey);
        return false;
    };
    if r.opts.select == Some(Select::Exact) {
        return false;
    }
    backend::category(n, r, cx) == keyword
}

/// `BetterThan(n, key1, key2)` for two matching keys: an exact
/// (numeric) key beats a keyword.
pub(crate) fn better_than(key1: &str, key2: &str) -> bool {
    is_number_literal(key1) && !is_number_literal(key2)
}

/// The sign and how many integer and fraction digits are shown; not the
/// digits, whose writer a client's formatting calls.
impl core::fmt::Debug for Digits<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Digits")
            .field("sign", &self.sign)
            .field("magnitudes", &(self.lo..=self.hi))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
