//! P0.5 probe — the **core numeric semantics** of `:number`, `:integer` and
//! `:offset` (spec: `third_party/message-format-wg/spec/functions/number.md`)
//! over `fixed_decimal`, with locale-neutral output (ASCII digits, `.`,
//! `-`/`+`, no grouping), plus what selection needs: the exact-match
//! serialization and the CLDR plural operands of the *formatted* number.
//!
//! Digit options follow ECMA-402 `SetNumberFormatDigitOptions` /
//! `FormatNumericToString` (MF2 took its option names and meanings from
//! `Intl.NumberFormat`); where ECMA-402 throws, this emits *Bad Option* and
//! ignores the offending option.
//!
//! `no_std` (+ `alloc`, which `fixed_decimal` needs), `forbid(unsafe_code)`,
//! fmt-free and panic-free.
#![no_std]
#![forbid(unsafe_code)]

pub use fixed_decimal::{Decimal, Sign, UnsignedDecimal};
use fixed_decimal::{RoundingIncrement, SignedRoundingMode, UnsignedRoundingMode};

/// Which function is being resolved.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Number,
    Integer,
    Offset,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Select {
    Plural,
    Ordinal,
    Exact,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SignDisplay {
    Auto,
    Always,
    ExceptZero,
    Negative,
    Never,
}

/// `useGrouping`. Core output never groups; `fn-number` honours it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    Auto,
    Always,
    Never,
    Min2,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RoundingPriority {
    Auto,
    MorePrecision,
    LessPrecision,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RoundingMode {
    Ceil,
    Floor,
    Expand,
    Trunc,
    HalfCeil,
    HalfFloor,
    HalfExpand,
    HalfTrunc,
    HalfEven,
}

/// Errors, as small enum values.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Error {
    BadOperand,
    BadOption,
    /// The resolved value does not support selection (`select` set by a
    /// variable or inherited from an operand).
    BadSelector,
    UnsupportedOperation,
    BadVariantKey,
}

pub trait ErrSink {
    fn push(&mut self, e: Error);
}

/// An option value: a literal, or a variable resolved to a string or an
/// integer (digit-size options accept implementation-defined numbers).
#[derive(Clone, Copy)]
pub enum OptValue<'a> {
    Literal(&'a str),
    VarStr(&'a str),
    VarInt(i64),
}

/// The *resolved options* of a numeric value; `None` = not set. They travel
/// with the value into any function that takes it as its operand.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct NumOpts {
    pub select: Option<Select>,
    pub sign_display: Option<SignDisplay>,
    pub use_grouping: Option<Grouping>,
    pub min_int: Option<u8>,
    pub min_frac: Option<u8>,
    pub max_frac: Option<u8>,
    pub min_sig: Option<u8>,
    pub max_sig: Option<u8>,
    pub strip_if_integer: Option<bool>,
    pub rounding_priority: Option<RoundingPriority>,
    /// One of 1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000.
    pub rounding_increment: Option<u16>,
    pub rounding_mode: Option<RoundingMode>,
}

/// Defaults of the fraction digits (`:number` 0/3, `:integer` 0/0;
/// `fn-number` sets `:percent` 0/0 and `:currency` from the currency).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FracDefaults {
    pub min: u8,
    pub max: u8,
}

/// The resolved value of a numeric expression.
#[derive(Clone)]
pub struct NumValue {
    pub value: Decimal,
    pub opts: NumOpts,
    pub frac: FracDefaults,
    /// `false` when `select` came from a variable or from the operand, and
    /// for functions that do not select.
    pub selectable: bool,
    /// Power of ten applied when formatting and selecting (`:percent` = 2);
    /// the numeric value itself stays unscaled.
    pub scale: i16,
}

/// A function operand as the runtime sees it.
#[derive(Clone, Copy)]
pub enum Operand<'a> {
    /// A literal, or a string-valued variable: must match `number-literal`.
    Str(&'a str),
    Int(i64),
    #[cfg(feature = "f64")]
    Float(f64),
    /// The resolved value of another numeric expression.
    Num(&'a NumValue),
    /// Anything else (booleans, dates, …) or no operand at all.
    Other,
}

/// `number-literal = ["-"] (%x30 / (%x31-39 *DIGIT)) ["." 1*DIGIT] [%i"e" ["-" / "+"] 1*DIGIT]`.
#[cfg_attr(feature = "noinline", inline(never))]
pub fn parse_number_literal(s: &str) -> Option<Decimal> {
    let b = s.as_bytes();
    let mut i = usize::from(b.first() == Some(&b'-'));
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        _ => return None,
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    let mantissa_end = i;
    let mut exp: i32 = 0;
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        let neg = match b.get(i) {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let start = i;
        while let Some(d) = b.get(i).filter(|d| d.is_ascii_digit()) {
            // Implementation limit: |exponent| ≤ 9999 (fixed_decimal is i16-indexed).
            exp = exp * 10 + i32::from(d - b'0');
            if exp > 9999 {
                return None;
            }
            i += 1;
        }
        if i == start {
            return None;
        }
        if neg {
            exp = -exp;
        }
    }
    if i != b.len() {
        return None;
    }
    let mut d = Decimal::try_from_utf8(b.get(..mantissa_end)?).ok()?;
    #[allow(clippy::cast_possible_truncation)]
    d.absolute.multiply_pow10(exp as i16);
    Some(d)
}

// ---------------------------------------------------------------- options

const K_SELECT: u8 = 0;
const K_SIGN: u8 = 1;
const K_GROUP: u8 = 2;
const K_MIN_INT: u8 = 3;
const K_MIN_FRAC: u8 = 4;
const K_MAX_FRAC: u8 = 5;
const K_MIN_SIG: u8 = 6;
const K_MAX_SIG: u8 = 7;
const K_TZD: u8 = 8;
const K_PRIORITY: u8 = 9;
const K_INCREMENT: u8 = 10;
const K_MODE: u8 = 11;

/// Option-set bits: which functions accept an option (spec number.md).
pub const NUM: u8 = 1;
pub const INT: u8 = 2;
pub const PCT: u8 = 4;
pub const CUR: u8 = 8;
pub const UNIT: u8 = 16;

/// (name, functions it applies to, kind). `:offset` takes only add/subtract;
/// `:currency`/`:unit`'s own options (`currency`, `unit`, …) are `fn-number`'s.
const OPTION_NAMES: [(&[u8], u8, u8); 12] = [
    (b"select", NUM | INT, K_SELECT),
    (b"signDisplay", NUM | INT | PCT | UNIT, K_SIGN),
    (b"useGrouping", NUM | INT | PCT | CUR | UNIT, K_GROUP),
    (b"minimumIntegerDigits", NUM | INT | CUR | UNIT, K_MIN_INT),
    (b"minimumFractionDigits", NUM | PCT | UNIT, K_MIN_FRAC),
    (b"maximumFractionDigits", NUM | PCT | UNIT, K_MAX_FRAC),
    (b"minimumSignificantDigits", NUM | PCT | CUR | UNIT, K_MIN_SIG),
    (b"maximumSignificantDigits", NUM | INT | PCT | CUR | UNIT, K_MAX_SIG),
    (b"trailingZeroDisplay", NUM | PCT | CUR, K_TZD),
    (b"roundingPriority", NUM | PCT | CUR | UNIT, K_PRIORITY),
    (b"roundingIncrement", NUM | CUR | UNIT, K_INCREMENT),
    (b"roundingMode", NUM | PCT | CUR | UNIT, K_MODE),
];

/// How one numeric function resolves (closed world: a handler is one `Spec`).
#[derive(Clone, Copy)]
pub struct Spec {
    /// Option-set bit (`NUM`, `INT`, `PCT`, `CUR`, `UNIT`; 0 = `:offset`).
    pub bit: u8,
    /// Fraction-digit defaults; `None` keeps the operand's (`:offset`).
    pub frac: Option<FracDefaults>,
    /// Round the value to an integer (`:integer`).
    pub integer: bool,
    /// Supports selection (`:currency` and `:unit` do not).
    pub selectable: bool,
    /// Power of ten applied when formatting/selecting (`:percent` = 2).
    pub scale: i16,
}

pub const NUMBER: Spec = Spec { bit: NUM, frac: Some(FracDefaults { min: 0, max: 3 }), integer: false, selectable: true, scale: 0 };
pub const INTEGER: Spec = Spec { bit: INT, frac: Some(FracDefaults { min: 0, max: 0 }), integer: true, selectable: true, scale: 0 };
pub const OFFSET: Spec = Spec { bit: 0, frac: None, integer: false, selectable: true, scale: 0 };

const SELECTS: [(&[u8], Select); 3] = [
    (b"plural", Select::Plural),
    (b"ordinal", Select::Ordinal),
    (b"exact", Select::Exact),
];
const SIGNS: [(&[u8], SignDisplay); 5] = [
    (b"auto", SignDisplay::Auto),
    (b"always", SignDisplay::Always),
    (b"exceptZero", SignDisplay::ExceptZero),
    (b"negative", SignDisplay::Negative),
    (b"never", SignDisplay::Never),
];
const GROUPINGS: [(&[u8], Grouping); 4] = [
    (b"auto", Grouping::Auto),
    (b"always", Grouping::Always),
    (b"never", Grouping::Never),
    (b"min2", Grouping::Min2),
];
const TZDS: [(&[u8], bool); 2] = [(b"auto", false), (b"stripIfInteger", true)];
const PRIORITIES: [(&[u8], RoundingPriority); 3] = [
    (b"auto", RoundingPriority::Auto),
    (b"morePrecision", RoundingPriority::MorePrecision),
    (b"lessPrecision", RoundingPriority::LessPrecision),
];
const MODES: [(&[u8], RoundingMode); 9] = [
    (b"ceil", RoundingMode::Ceil),
    (b"floor", RoundingMode::Floor),
    (b"expand", RoundingMode::Expand),
    (b"trunc", RoundingMode::Trunc),
    (b"halfCeil", RoundingMode::HalfCeil),
    (b"halfFloor", RoundingMode::HalfFloor),
    (b"halfExpand", RoundingMode::HalfExpand),
    (b"halfTrunc", RoundingMode::HalfTrunc),
    (b"halfEven", RoundingMode::HalfEven),
];
const INCREMENTS: [u16; 15] = [1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000];

fn find<T: Copy>(table: &[(&[u8], T)], v: &[u8]) -> Option<T> {
    table.iter().find(|(k, _)| *k == v).map(|&(_, t)| t)
}

/// A *digit size option*: `"0" / ("1"-"9") [DIGIT]`, or an integer variable.
#[cfg_attr(feature = "noinline", inline(never))]
pub fn digit_size(v: OptValue<'_>) -> Option<u8> {
    match v {
        OptValue::Literal(s) | OptValue::VarStr(s) => match *s.as_bytes() {
            [d @ b'0'..=b'9'] => Some(d - b'0'),
            [a @ b'1'..=b'9', b @ b'0'..=b'9'] => Some((a - b'0') * 10 + (b - b'0')),
            _ => None,
        },
        OptValue::VarInt(n) => u8::try_from(n).ok().filter(|n| *n <= 99),
    }
}

fn text<'a>(v: OptValue<'a>) -> Option<&'a str> {
    match v {
        OptValue::Literal(s) | OptValue::VarStr(s) => Some(s),
        OptValue::VarInt(_) => None,
    }
}

/// Apply one option (already known to belong to the function) to `o`.
/// Returns `false` for an invalid value (→ *Bad Option*, option ignored).
#[cfg_attr(feature = "noinline", inline(never))]
fn apply(o: &mut NumOpts, kind: u8, v: OptValue<'_>) -> bool {
    let t = text(v).map(str::as_bytes);
    let ok = match kind {
        K_SIGN => t.and_then(|t| find(&SIGNS, t)).map(|x| o.sign_display = Some(x)),
        K_GROUP => t.and_then(|t| find(&GROUPINGS, t)).map(|x| o.use_grouping = Some(x)),
        K_MIN_INT => digit_size(v).map(|x| o.min_int = Some(x)),
        K_MIN_FRAC => digit_size(v).map(|x| o.min_frac = Some(x)),
        K_MAX_FRAC => digit_size(v).map(|x| o.max_frac = Some(x)),
        // Implementation limit: significant digits ≥ 1.
        K_MIN_SIG => digit_size(v).filter(|x| *x >= 1).map(|x| o.min_sig = Some(x)),
        K_MAX_SIG => digit_size(v).filter(|x| *x >= 1).map(|x| o.max_sig = Some(x)),
        K_TZD => t.and_then(|t| find(&TZDS, t)).map(|x| o.strip_if_integer = Some(x)),
        K_PRIORITY => t
            .and_then(|t| find(&PRIORITIES, t))
            .map(|x| o.rounding_priority = Some(x)),
        K_INCREMENT => {
            let n = match v {
                OptValue::VarInt(n) => u16::try_from(n).ok(),
                OptValue::Literal(s) | OptValue::VarStr(s) => parse_u16(s.as_bytes()),
            };
            n.filter(|n| INCREMENTS.contains(n))
                .map(|x| o.rounding_increment = Some(x))
        }
        K_MODE => t.and_then(|t| find(&MODES, t)).map(|x| o.rounding_mode = Some(x)),
        _ => None,
    };
    ok.is_some()
}

fn parse_u16(b: &[u8]) -> Option<u16> {
    if b.is_empty() || b.len() > 4 || b.first() == Some(&b'0') {
        return None;
    }
    b.iter().try_fold(0u16, |acc, &d| {
        d.is_ascii_digit().then(|| acc * 10 + u16::from(d - b'0'))
    })
}

/// Resolve `:number` / `:integer` / `:offset`. `None` = fallback value (the
/// error has been pushed).
pub fn resolve(
    func: Func,
    operand: &Operand<'_>,
    opts: &[(&str, OptValue<'_>)],
    errs: &mut impl ErrSink,
) -> Option<NumValue> {
    let spec = match func {
        Func::Number => &NUMBER,
        Func::Integer => &INTEGER,
        Func::Offset => &OFFSET,
    };
    resolve_spec(spec, operand, opts, errs)
}

/// Resolve any numeric function described by `spec` (used by `fn-number` for
/// `:percent`, `:currency`, `:unit`, which add their own options on top).
#[cfg_attr(feature = "noinline", inline(never))]
pub fn resolve_spec(
    spec: &Spec,
    operand: &Operand<'_>,
    opts: &[(&str, OptValue<'_>)],
    errs: &mut impl ErrSink,
) -> Option<NumValue> {
    let is_offset = spec.bit == 0;
    let (value, mut o, inherited_frac, mut selectable) = match operand {
        Operand::Str(s) => (parse_number_literal(s), NumOpts::default(), None, true),
        Operand::Int(n) => (Some(Decimal::from(*n)), NumOpts::default(), None, true),
        #[cfg(feature = "f64")]
        Operand::Float(f) => (
            Decimal::try_from_f64(*f, fixed_decimal::FloatPrecision::RoundTrip).ok(),
            NumOpts::default(),
            None,
            true,
        ),
        Operand::Num(nv) => {
            let mut o = nv.opts;
            let mut sel = nv.selectable;
            if o.select.is_some() {
                // `select` set by an operand: Bad Option and no selection for
                // the selecting functions (spec: Number Selection); `:offset`
                // passes it on; the others discard it.
                if spec.bit & (NUM | INT) != 0 {
                    errs.push(Error::BadOption);
                    sel = false;
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
            (Some(nv.value.clone()), o, Some((nv.frac, nv.scale)), sel)
        }
        Operand::Other => (None, NumOpts::default(), None, true),
    };
    let Some(mut value) = value else {
        errs.push(Error::BadOperand);
        return None;
    };
    let mut offset: Option<(bool, u8)> = None;
    let mut offset_bad = false;
    for &(name, v) in opts {
        let n = name.as_bytes();
        if is_offset {
            let sub = match n {
                b"add" => false,
                b"subtract" => true,
                _ => continue, // unknown options are ignored
            };
            match (digit_size(v), offset) {
                (Some(d), None) => offset = Some((sub, d)),
                _ => offset_bad = true,
            }
            continue;
        }
        let Some(kind) = OPTION_NAMES
            .iter()
            .find(|(k, mask, _)| *k == n && mask & spec.bit != 0)
            .map(|&(_, _, k)| k)
        else {
            continue;
        };
        if kind == K_SELECT {
            // MUST be a literal; otherwise Bad Option and no selection.
            match v {
                OptValue::Literal(s) => match find(&SELECTS, s.as_bytes()) {
                    Some(x) => o.select = Some(x),
                    None => errs.push(Error::BadOption),
                },
                _ => {
                    errs.push(Error::BadOption);
                    selectable = false;
                }
            }
            continue;
        }
        if !apply(&mut o, kind, v) {
            errs.push(Error::BadOption);
        }
    }
    if spec.integer {
        // The resolved value of `:integer` is the integer value.
        value.round_with_mode(0, SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand));
        value.absolute.trim_end();
    }
    let (frac, scale) = match (spec.frac, inherited_frac) {
        (Some(f), _) => (f, spec.scale),
        (None, Some(inherited)) => inherited,
        (None, None) => (NUMBER.frac.unwrap_or(FracDefaults { min: 0, max: 3 }), 0),
    };
    if is_offset {
        let Some((sub, d)) = offset.filter(|_| !offset_bad) else {
            errs.push(Error::BadOption);
            return None;
        };
        let Some(sum) = add_small(&value, if sub { -i32::from(d) } else { i32::from(d) }) else {
            // Implementation limit (spec allows Unsupported Operation).
            errs.push(Error::UnsupportedOperation);
            return None;
        };
        value = sum;
    }
    Some(NumValue {
        value,
        opts: o,
        frac,
        selectable: selectable && spec.selectable,
        scale,
    })
}

/// `value + delta` for a small integer delta, exactly (`fixed_decimal` has no
/// addition): scale to an `i64` (≤ 18 significant digits — beyond that the
/// spec allows *Unsupported Operation*), add, and rebuild from ASCII.
#[cfg_attr(feature = "noinline", inline(never))]
fn add_small(value: &Decimal, delta: i32) -> Option<Decimal> {
    let r = value.absolute.magnitude_range();
    let (lo, hi) = ((*r.start()).min(0), (*r.end()).max(0));
    if hi - lo > 17 {
        return None;
    }
    let mut n: i64 = 0;
    let mut m = hi;
    while m >= lo {
        n = n * 10 + i64::from(value.absolute.digit_at(m));
        m -= 1;
    }
    if value.sign == Sign::Negative {
        n = -n;
    }
    let mut scale: i64 = 1;
    for _ in lo..0 {
        scale *= 10;
    }
    n += i64::from(delta) * scale;
    // ASCII, least significant digit first, then reversed.
    let mut buf = [0u8; 24];
    let mut len = 0;
    let neg = n < 0;
    let mut u = n.unsigned_abs();
    let frac = usize::try_from(-lo).unwrap_or(0);
    loop {
        if len == frac && frac > 0 {
            *buf.get_mut(len)? = b'.';
            len += 1;
        }
        #[allow(clippy::cast_possible_truncation)]
        let d = (u % 10) as u8;
        *buf.get_mut(len)? = b'0' + d;
        len += 1;
        u /= 10;
        if u == 0 && len > frac {
            break;
        }
    }
    if neg {
        *buf.get_mut(len)? = b'-';
        len += 1;
    }
    let digits = buf.get_mut(..len)?;
    digits.reverse();
    Decimal::try_from_utf8(digits).ok()
}

// ------------------------------------------------------------- formatting

/// Sign to display, after `signDisplay`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SignOut {
    None,
    Minus,
    Plus,
}

/// The digits to display: sign + rounded, padded absolute value.
pub struct Formatted {
    pub sign: SignOut,
    pub abs: UnsignedDecimal,
}

fn to_fd_mode(m: RoundingMode) -> SignedRoundingMode {
    use SignedRoundingMode as S;
    use UnsignedRoundingMode as U;
    match m {
        RoundingMode::Ceil => S::Ceil,
        RoundingMode::Floor => S::Floor,
        RoundingMode::HalfCeil => S::HalfCeil,
        RoundingMode::HalfFloor => S::HalfFloor,
        RoundingMode::Expand => S::Unsigned(U::Expand),
        RoundingMode::Trunc => S::Unsigned(U::Trunc),
        RoundingMode::HalfExpand => S::Unsigned(U::HalfExpand),
        RoundingMode::HalfTrunc => S::Unsigned(U::HalfTrunc),
        RoundingMode::HalfEven => S::Unsigned(U::HalfEven),
    }
}

/// Increment `m × 10^k` → (fixed_decimal increment, k).
#[cfg_attr(feature = "noinline", inline(never))]
fn split_increment(n: u16) -> (RoundingIncrement, i16) {
    let mut n = n;
    let mut k = 0;
    while n >= 10 && n % 10 == 0 && n != 25 {
        n /= 10;
        k += 1;
    }
    let inc = match n {
        2 => RoundingIncrement::MultiplesOf2,
        5 => RoundingIncrement::MultiplesOf5,
        25 => RoundingIncrement::MultiplesOf25,
        _ => RoundingIncrement::MultiplesOf1,
    };
    (inc, k)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RoundingType {
    Fraction,
    Significant,
    More,
    Less,
}

/// ECMA-402 SetNumberFormatDigitOptions, errors → Bad Option (pushed by
/// [`digit_plan`]'s caller through `errs`).
struct DigitPlan {
    min_int: u8,
    min_frac: u8,
    max_frac: u8,
    min_sig: u8,
    max_sig: u8,
    ty: RoundingType,
    increment: u16,
    mode: SignedRoundingMode,
    strip_if_integer: bool,
}

#[cfg_attr(feature = "noinline", inline(never))]
fn digit_plan(v: &NumValue, errs: &mut impl ErrSink) -> DigitPlan {
    let o = &v.opts;
    let increment = o.rounding_increment.unwrap_or(1);
    let mnfd_default = v.frac.min;
    // A rounding increment makes the maximum default to the minimum.
    let mxfd_default = if increment == 1 { v.frac.max } else { v.frac.min };
    let priority = o.rounding_priority.unwrap_or(RoundingPriority::Auto);
    let has_sd = o.min_sig.is_some() || o.max_sig.is_some();
    let has_fd = o.min_frac.is_some() || o.max_frac.is_some();
    let need_sd = priority != RoundingPriority::Auto || has_sd;
    let need_fd = priority != RoundingPriority::Auto || !has_sd;
    let (mut min_sig, mut max_sig) = (1, 21);
    if need_sd && has_sd {
        min_sig = o.min_sig.unwrap_or(1);
        max_sig = o.max_sig.unwrap_or(21.max(min_sig));
        if min_sig > max_sig {
            errs.push(Error::BadOption);
            min_sig = max_sig;
        }
    }
    let (mut min_frac, mut max_frac) = (mnfd_default, mxfd_default);
    if need_fd && has_fd {
        match (o.min_frac, o.max_frac) {
            (None, Some(mx)) => {
                max_frac = mx;
                min_frac = mnfd_default.min(mx);
            }
            (Some(mn), None) => {
                min_frac = mn;
                max_frac = mxfd_default.max(mn);
            }
            (Some(mn), Some(mx)) => {
                if mn > mx {
                    errs.push(Error::BadOption);
                    min_frac = mx;
                } else {
                    min_frac = mn;
                }
                max_frac = mx;
            }
            (None, None) => {}
        }
    }
    let ty = match priority {
        RoundingPriority::Auto if need_sd => RoundingType::Significant,
        RoundingPriority::Auto => RoundingType::Fraction,
        RoundingPriority::MorePrecision => RoundingType::More,
        RoundingPriority::LessPrecision => RoundingType::Less,
    };
    let increment = if increment != 1 && (ty != RoundingType::Fraction || min_frac != max_frac) {
        errs.push(Error::BadOption);
        1
    } else {
        increment
    };
    DigitPlan {
        min_int: o.min_int.unwrap_or(1),
        min_frac,
        max_frac,
        min_sig,
        max_sig,
        ty,
        increment,
        mode: to_fd_mode(o.rounding_mode.unwrap_or(RoundingMode::HalfExpand)),
        strip_if_integer: o.strip_if_integer.unwrap_or(false),
    }
}

/// ToRawFixed: returns (result, rounding magnitude).
#[cfg_attr(feature = "noinline", inline(never))]
fn raw_fixed(x: &Decimal, p: &DigitPlan) -> (Decimal, i16) {
    let mut d = x.clone();
    let (inc, k) = split_increment(p.increment);
    let pos = -i16::from(p.max_frac) + k;
    d.round_with_mode_and_increment(pos, p.mode, inc);
    d.absolute.trim_end();
    d.absolute.pad_end(-i16::from(p.min_frac));
    (d, -i16::from(p.max_frac))
}

/// ToRawPrecision: returns (result, rounding magnitude).
#[cfg_attr(feature = "noinline", inline(never))]
fn raw_precision(x: &Decimal, p: &DigitPlan) -> (Decimal, i16) {
    let mut d = x.clone();
    let e = d.absolute.nonzero_magnitude_start();
    d.round_with_mode(e - i16::from(p.max_sig) + 1, p.mode);
    let e2 = d.absolute.nonzero_magnitude_start();
    d.absolute.trim_end();
    d.absolute.pad_end(e2 - i16::from(p.min_sig) + 1);
    (d, e2 - i16::from(p.max_sig) + 1)
}

/// The digits to display for `v` (rounded, padded, sign resolved). Errors
/// from conflicting digit options are pushed once here.
#[cfg_attr(feature = "noinline", inline(never))]
pub fn format_digits(v: &NumValue, errs: &mut impl ErrSink) -> Formatted {
    let p = digit_plan(v, errs);
    let scaled;
    let x = if v.scale == 0 {
        &v.value
    } else {
        let mut t = v.value.clone();
        t.absolute.multiply_pow10(v.scale);
        scaled = t;
        &scaled
    };
    let (mut d, _) = match p.ty {
        RoundingType::Fraction => raw_fixed(x, &p),
        RoundingType::Significant => raw_precision(x, &p),
        RoundingType::More | RoundingType::Less => {
            let (s, sm) = raw_precision(x, &p);
            let (f, fm) = raw_fixed(x, &p);
            let fixed_more_precise = fm < sm;
            if (p.ty == RoundingType::More) == fixed_more_precise {
                (f, fm)
            } else {
                (s, sm)
            }
        }
    };
    if p.strip_if_integer {
        d.absolute.trim_end_if_integer();
    }
    d.absolute.pad_start(i16::from(p.min_int));
    let zero = d.absolute.is_zero();
    let negative = d.sign == Sign::Negative;
    let sign = match v.opts.sign_display.unwrap_or(SignDisplay::Auto) {
        SignDisplay::Auto if negative => SignOut::Minus,
        SignDisplay::Always if negative => SignOut::Minus,
        SignDisplay::Always => SignOut::Plus,
        SignDisplay::ExceptZero if zero => SignOut::None,
        SignDisplay::ExceptZero if negative => SignOut::Minus,
        SignDisplay::ExceptZero => SignOut::Plus,
        SignDisplay::Negative if negative && !zero => SignOut::Minus,
        _ => SignOut::None,
    };
    Formatted {
        sign,
        abs: d.absolute,
    }
}

/// Byte sink (not `core::fmt::Write`).
pub trait ByteSink {
    fn byte(&mut self, b: u8);
}

/// Neutral rendering: `[-+]digits[.digits]`, ASCII, no grouping.
pub fn write_neutral(f: &Formatted, out: &mut impl ByteSink) {
    match f.sign {
        SignOut::Minus => out.byte(b'-'),
        SignOut::Plus => out.byte(b'+'),
        SignOut::None => {}
    }
    let r = f.abs.magnitude_range();
    let (lo, hi) = (*r.start(), (*r.end()).max(0));
    let mut m = hi;
    while m >= lo.min(0) {
        if m == -1 {
            out.byte(b'.');
        }
        out.byte(b'0' + f.abs.digit_at(m));
        m -= 1;
    }
}

// -------------------------------------------------------------- selection

/// CLDR plural operands (UTS #35), computed from the *formatted* number.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct PluralOperands {
    pub i: u64,
    pub v: u16,
    pub w: u16,
    pub f: u64,
    pub t: u64,
    pub c: u16,
}

/// Operand values keep their 18 least significant digits (enough for every
/// CLDR rule, which only uses small moduli and ranges), so the arithmetic
/// never overflows `u64` — no `checked_mul`, which on wasm32 would pull in
/// the 128-bit `__multi3`.
const KEEP: u64 = 1_000_000_000_000_000_000;

fn push_digit(acc: u64, d: u8) -> u64 {
    (acc % (KEEP / 10)) * 10 + u64::from(d)
}

#[cfg_attr(feature = "noinline", inline(never))]
pub fn plural_operands(f: &Formatted) -> PluralOperands {
    let r = f.abs.magnitude_range();
    let (lo, hi) = (*r.start(), *r.end());
    let mut op = PluralOperands::default();
    let mut m = hi;
    while m >= 0 {
        op.i = push_digit(op.i, f.abs.digit_at(m));
        m -= 1;
    }
    if lo < 0 {
        #[allow(clippy::cast_sign_loss)]
        let v = (-lo) as u16;
        op.v = v;
        let nz_end = if f.abs.is_zero() { 0 } else { f.abs.nonzero_magnitude_end() };
        #[allow(clippy::cast_sign_loss)]
        let w = if nz_end < 0 { (-nz_end) as u16 } else { 0 };
        op.w = w;
        let mut m = -1;
        while m >= lo {
            let d = f.abs.digit_at(m);
            op.f = push_digit(op.f, d);
            if m >= -i16::try_from(w).unwrap_or(0) {
                op.t = push_digit(op.t, d);
            }
            m -= 1;
        }
    }
    op
}

/// Does `key` match the production `number-literal`?
pub fn is_number_literal(key: &str) -> bool {
    parse_number_literal(key).is_some()
}

/// Exact-match serialization (spec: Exact Literal Match Serialization): the
/// integer form when the value is an integer and none of min-fraction,
/// min-integer, min/max-significant are set; otherwise (implementation-
/// defined) the neutral formatted string. Compared byte-wise with `key`.
#[cfg_attr(feature = "noinline", inline(never))]
pub fn exact_matches(v: &NumValue, f: &Formatted, key: &str) -> bool {
    struct Cmp<'k> {
        key: &'k [u8],
        pos: usize,
        ok: bool,
    }
    impl ByteSink for Cmp<'_> {
        fn byte(&mut self, b: u8) {
            self.ok &= self.key.get(self.pos) == Some(&b);
            self.pos += 1;
        }
    }
    let o = &v.opts;
    let plain = o.min_frac.is_none() && o.min_int.is_none() && o.min_sig.is_none() && o.max_sig.is_none();
    // `:percent` selects on the value × 100.
    let mut value = v.value.clone();
    value.absolute.multiply_pow10(v.scale);
    let is_int = value.absolute.is_zero() || value.absolute.nonzero_magnitude_end() >= 0;
    let mut c = Cmp {
        key: key.as_bytes(),
        pos: 0,
        ok: true,
    };
    if plain && is_int {
        let mut abs = value.absolute;
        abs.trim_start();
        abs.trim_end();
        let neg = value.sign == Sign::Negative && !abs.is_zero();
        let int = Formatted {
            sign: if neg { SignOut::Minus } else { SignOut::None },
            abs,
        };
        write_neutral(&int, &mut c);
    } else {
        write_neutral(f, &mut c);
    }
    c.ok && c.pos == c.key.len()
}

/// Plural category keywords.
pub const KEYWORDS: [&[u8]; 6] = [b"zero", b"one", b"two", b"few", b"many", b"other"];

/// Spec *Match* for numeric selectors. `category` is the plural-rule
/// evaluator (another probe's job): given operands and "ordinal?", it
/// returns the keyword. `Err` = Bad Variant Key.
#[cfg_attr(feature = "noinline", inline(never))]
pub fn matches(
    v: &NumValue,
    f: &Formatted,
    key: &str,
    category: &dyn Fn(&PluralOperands, bool) -> &'static str,
) -> Result<bool, Error> {
    if is_number_literal(key) {
        return Ok(exact_matches(v, f, key));
    }
    if KEYWORDS.contains(&key.as_bytes()) {
        return Ok(match v.opts.select.unwrap_or(Select::Plural) {
            Select::Exact => false,
            s => category(&plural_operands(f), s == Select::Ordinal).as_bytes() == key.as_bytes(),
        });
    }
    Err(Error::BadVariantKey)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::string::String;
    use std::vec::Vec;

    #[derive(Default)]
    struct E(Vec<Error>);
    impl ErrSink for E {
        fn push(&mut self, e: Error) {
            self.0.push(e);
        }
    }
    struct S(String);
    impl ByteSink for S {
        fn byte(&mut self, b: u8) {
            self.0.push(char::from(b));
        }
    }

    fn fmt(func: Func, operand: &str, opts: &[(&str, &str)]) -> String {
        let o: Vec<(&str, OptValue<'_>)> = opts.iter().map(|&(k, v)| (k, OptValue::Literal(v))).collect();
        let mut e = E::default();
        let Some(v) = resolve(func, &Operand::Str(operand), &o, &mut e) else {
            return String::from("FALLBACK");
        };
        let f = format_digits(&v, &mut e);
        let mut s = S(String::new());
        write_neutral(&f, &mut s);
        s.0
    }

    #[test]
    fn literals() {
        for ok in ["0", "-0", "4.2", "-4.20", "0.42e+1", "1E3", "1e-2", "10"] {
            assert!(parse_number_literal(ok).is_some(), "{ok}");
        }
        for bad in ["00", "042", "1.", "1e", "1E", "1.e", "1.2e", "1.e3", "1e+", "1e-", "1.0e2.0", "foo", ".1", "+1", "0x1", ""] {
            assert!(parse_number_literal(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn digits() {
        let n = Func::Number;
        assert_eq!(fmt(n, "4.2", &[]), "4.2");
        assert_eq!(fmt(n, "-4.20", &[]), "-4.2");
        assert_eq!(fmt(n, "0.42e+1", &[]), "4.2");
        assert_eq!(fmt(n, "4.2", &[("minimumFractionDigits", "2")]), "4.20");
        assert_eq!(fmt(n, "1.23456", &[]), "1.235");
        assert_eq!(fmt(n, "1234.5", &[("maximumSignificantDigits", "2")]), "1200");
        assert_eq!(fmt(n, "0.000123", &[("maximumSignificantDigits", "2")]), "0.00012");
        assert_eq!(fmt(n, "1.5", &[("minimumSignificantDigits", "3")]), "1.50");
        assert_eq!(fmt(n, "5", &[("minimumIntegerDigits", "3")]), "005");
        assert_eq!(fmt(n, "1.2", &[("minimumFractionDigits", "2"), ("trailingZeroDisplay", "stripIfInteger")]), "1.20");
        assert_eq!(fmt(n, "1.00", &[("minimumFractionDigits", "2"), ("trailingZeroDisplay", "stripIfInteger")]), "1");
        assert_eq!(fmt(n, "1.23", &[("maximumFractionDigits", "2"), ("minimumFractionDigits", "2"), ("roundingIncrement", "5")]), "1.25");
        assert_eq!(fmt(n, "1.3", &[("maximumFractionDigits", "1"), ("minimumFractionDigits", "1"), ("roundingIncrement", "25")]), "2.5");
        assert_eq!(fmt(n, "1234", &[("maximumFractionDigits", "0"), ("roundingIncrement", "100")]), "1200");
        assert_eq!(fmt(n, "2.5", &[("maximumFractionDigits", "0"), ("roundingMode", "halfEven")]), "2");
        assert_eq!(fmt(n, "-2.5", &[("maximumFractionDigits", "0"), ("roundingMode", "halfCeil")]), "-2");
        assert_eq!(fmt(n, "-2.1", &[("maximumFractionDigits", "0"), ("roundingMode", "floor")]), "-3");
        assert_eq!(fmt(n, "2.1", &[("maximumFractionDigits", "0"), ("roundingMode", "expand")]), "3");
        assert_eq!(fmt(n, "1", &[("signDisplay", "always")]), "+1");
        assert_eq!(fmt(n, "0", &[("signDisplay", "exceptZero")]), "0");
        assert_eq!(fmt(n, "-0.001", &[("maximumFractionDigits", "2")]), "-0");
        assert_eq!(fmt(n, "-0.001", &[("maximumFractionDigits", "2"), ("signDisplay", "negative")]), "0");
        assert_eq!(fmt(n, "-0", &[("signDisplay", "never")]), "0");
        // roundingPriority (ECMA-402 examples): 1.23456 with maxFD=3, maxSD=2
        let o = [("maximumFractionDigits", "3"), ("maximumSignificantDigits", "2")];
        assert_eq!(fmt(n, "1.23456", &[o[0], o[1], ("roundingPriority", "morePrecision")]), "1.235");
        assert_eq!(fmt(n, "1.23456", &[o[0], o[1], ("roundingPriority", "lessPrecision")]), "1.2");
        assert_eq!(fmt(Func::Integer, "4.2", &[]), "4");
        assert_eq!(fmt(Func::Integer, "-4.20", &[]), "-4");
        assert_eq!(fmt(Func::Integer, "0.42e+1", &[]), "4");
        assert_eq!(fmt(Func::Offset, "41", &[("add", "1")]), "42");
        assert_eq!(fmt(Func::Offset, "52", &[("subtract", "10")]), "42");
        assert_eq!(fmt(Func::Offset, "1", &[("subtract", "3")]), "-2");
        assert_eq!(fmt(Func::Offset, "-1.5", &[("add", "2")]), "0.5");
        assert_eq!(fmt(Func::Offset, "0.25", &[("subtract", "1")]), "-0.75");
        assert_eq!(fmt(Func::Offset, "999", &[("add", "99")]), "1098");
    }

    #[test]
    fn operands() {
        let mut e = E::default();
        let v = resolve(Func::Number, &Operand::Str("1.50"), &[("minimumFractionDigits", OptValue::Literal("1"))], &mut e);
        let Some(v) = v else { unreachable!() };
        let f = format_digits(&v, &mut e);
        let op = plural_operands(&f);
        assert_eq!((op.i, op.v, op.w, op.f, op.t), (1, 1, 1, 5, 5));
        let v = resolve(Func::Number, &Operand::Str("1.20"), &[("minimumFractionDigits", OptValue::Literal("2"))], &mut e);
        let Some(v) = v else { unreachable!() };
        let op = plural_operands(&format_digits(&v, &mut e));
        assert_eq!((op.i, op.v, op.w, op.f, op.t), (1, 2, 1, 20, 2));
    }
}
