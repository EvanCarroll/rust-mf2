//! The numeric options (`number.md`) as resolved values, and ECMA-402's
//! `SetNumberFormatDigitOptions` (MF2 took its option names and meanings from
//! `Intl.NumberFormat`): where ECMA-402 throws, this reports *Bad Option* and
//! ignores the offending option. Ported from P0.5.

use super::decimal::{Increment, RoundingMode};
use crate::error::FormatError;
use crate::sink::ErrorSink;
use crate::value::Value;

/// `select`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Select {
    Plural,
    Ordinal,
    Exact,
}

/// `signDisplay`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum SignDisplay {
    Auto,
    Always,
    ExceptZero,
    Negative,
    Never,
}

/// `useGrouping`. Core output never groups; `fn-number` (P4) honours it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Grouping {
    Auto,
    Always,
    Never,
    Min2,
}

/// `roundingPriority`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum RoundingPriority {
    Auto,
    MorePrecision,
    LessPrecision,
}

/// The resolved options of a numeric value; `None` = not set. They travel
/// with the value into a function that takes it as its operand.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub(crate) struct NumOpts {
    pub(crate) select: Option<Select>,
    pub(crate) sign_display: Option<SignDisplay>,
    pub(crate) use_grouping: Option<Grouping>,
    pub(crate) min_int: Option<u8>,
    pub(crate) min_frac: Option<u8>,
    pub(crate) max_frac: Option<u8>,
    pub(crate) min_sig: Option<u8>,
    pub(crate) max_sig: Option<u8>,
    pub(crate) strip_if_integer: Option<bool>,
    pub(crate) rounding_priority: Option<RoundingPriority>,
    /// 1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500 or 5000.
    pub(crate) rounding_increment: Option<u16>,
    pub(crate) rounding_mode: Option<RoundingMode>,
}

/// Default fraction digits (`:number` 0–3, `:integer` 0–0; P4: `:percent`,
/// `:currency`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct FracDefaults {
    pub(crate) min: u8,
    pub(crate) max: u8,
}

/// Option kinds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Select,
    Sign,
    Grouping,
    MinInt,
    MinFrac,
    MaxFrac,
    MinSig,
    MaxSig,
    TrailingZero,
    Priority,
    Increment,
    Mode,
}

/// Which functions an option applies to (bits).
pub(crate) const NUM: u8 = 1;
pub(crate) const INT: u8 = 2;
pub(crate) const PCT: u8 = 4;
pub(crate) const CUR: u8 = 8;
pub(crate) const UNIT: u8 = 16;

/// `(name, functions, kind)`; `:offset` has only `add`/`subtract`.
pub(crate) const OPTIONS: [(&str, u8, Kind); 12] = [
    ("select", NUM | INT, Kind::Select),
    ("signDisplay", NUM | INT | PCT | UNIT, Kind::Sign),
    ("useGrouping", NUM | INT | PCT | CUR | UNIT, Kind::Grouping),
    ("minimumIntegerDigits", NUM | INT | CUR | UNIT, Kind::MinInt),
    ("minimumFractionDigits", NUM | PCT | UNIT, Kind::MinFrac),
    ("maximumFractionDigits", NUM | PCT | UNIT, Kind::MaxFrac),
    (
        "minimumSignificantDigits",
        NUM | PCT | CUR | UNIT,
        Kind::MinSig,
    ),
    (
        "maximumSignificantDigits",
        NUM | INT | PCT | CUR | UNIT,
        Kind::MaxSig,
    ),
    ("trailingZeroDisplay", NUM | PCT | CUR, Kind::TrailingZero),
    ("roundingPriority", NUM | PCT | CUR | UNIT, Kind::Priority),
    ("roundingIncrement", NUM | CUR | UNIT, Kind::Increment),
    ("roundingMode", NUM | PCT | CUR | UNIT, Kind::Mode),
];

const SELECTS: [(&str, Select); 3] = [
    ("plural", Select::Plural),
    ("ordinal", Select::Ordinal),
    ("exact", Select::Exact),
];
const SIGNS: [(&str, SignDisplay); 5] = [
    ("auto", SignDisplay::Auto),
    ("always", SignDisplay::Always),
    ("exceptZero", SignDisplay::ExceptZero),
    ("negative", SignDisplay::Negative),
    ("never", SignDisplay::Never),
];
const GROUPINGS: [(&str, Grouping); 4] = [
    ("auto", Grouping::Auto),
    ("always", Grouping::Always),
    ("never", Grouping::Never),
    ("min2", Grouping::Min2),
];
const TRAILING: [(&str, bool); 2] = [("auto", false), ("stripIfInteger", true)];
const PRIORITIES: [(&str, RoundingPriority); 3] = [
    ("auto", RoundingPriority::Auto),
    ("morePrecision", RoundingPriority::MorePrecision),
    ("lessPrecision", RoundingPriority::LessPrecision),
];
const MODES: [(&str, RoundingMode); 9] = [
    ("ceil", RoundingMode::Ceil),
    ("floor", RoundingMode::Floor),
    ("expand", RoundingMode::Expand),
    ("trunc", RoundingMode::Trunc),
    ("halfCeil", RoundingMode::HalfCeil),
    ("halfFloor", RoundingMode::HalfFloor),
    ("halfExpand", RoundingMode::HalfExpand),
    ("halfTrunc", RoundingMode::HalfTrunc),
    ("halfEven", RoundingMode::HalfEven),
];
const INCREMENTS: [u16; 15] = [
    1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000,
];

fn find<T: Copy>(table: &[(&str, T)], v: &str) -> Option<T> {
    table.iter().find(|(k, _)| *k == v).map(|&(_, t)| t)
}

/// The select mode named by a literal.
pub(crate) fn select_named(v: &str) -> Option<Select> {
    find(&SELECTS, v)
}

/// A *digit size option* (`number.md`): `"0" / ("1"-"9") [DIGIT]` as text,
/// or an integer value 0..=99.
pub(crate) fn digit_size(v: &Value<'_>) -> Option<u8> {
    match v {
        Value::Int(n) => u8::try_from(*n).ok().filter(|n| *n <= 99),
        Value::Number(n) => n
            .value
            .to_i64()
            .and_then(|n| u8::try_from(n).ok())
            .filter(|n| *n <= 99),
        Value::Float(x) => {
            if !(0.0..=99.0).contains(x) {
                return None;
            }
            // In range, so the cast is exact for an integral `x`, and the
            // comparison is exact on purpose: only an integral value is a
            // digit size.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = *x as u8;
            #[allow(clippy::float_cmp)]
            (f64::from(n) == *x).then_some(n)
        }
        _ => match *v.as_str()?.as_bytes() {
            [d @ b'0'..=b'9'] => Some(d - b'0'),
            [a @ b'1'..=b'9', b @ b'0'..=b'9'] => Some((a - b'0') * 10 + (b - b'0')),
            _ => None,
        },
    }
}

fn increment_of(v: &Value<'_>) -> Option<u16> {
    let n = match v {
        Value::Int(n) => u16::try_from(*n).ok()?,
        Value::Number(n) => u16::try_from(n.value.to_i64()?).ok()?,
        _ => {
            let b = v.as_str()?.as_bytes();
            if b.is_empty() || b.len() > 4 || b.first() == Some(&b'0') {
                return None;
            }
            b.iter().try_fold(0u16, |acc, &d| {
                d.is_ascii_digit().then(|| acc * 10 + u16::from(d - b'0'))
            })?
        }
    };
    INCREMENTS.contains(&n).then_some(n)
}

/// Applies one option of kind `kind` (other than `select`); `false` for an
/// invalid value (→ *Bad Option*, the option is ignored).
pub(crate) fn apply(o: &mut NumOpts, kind: Kind, v: &Value<'_>) -> bool {
    let t = v.as_str();
    let ok = match kind {
        Kind::Sign => t
            .and_then(|t| find(&SIGNS, t))
            .map(|x| o.sign_display = Some(x)),
        Kind::Grouping => t
            .and_then(|t| find(&GROUPINGS, t))
            .map(|x| o.use_grouping = Some(x)),
        Kind::MinInt => digit_size(v).map(|x| o.min_int = Some(x)),
        Kind::MinFrac => digit_size(v).map(|x| o.min_frac = Some(x)),
        Kind::MaxFrac => digit_size(v).map(|x| o.max_frac = Some(x)),
        // An implementation limit: significant digits ≥ 1.
        Kind::MinSig => digit_size(v)
            .filter(|x| *x >= 1)
            .map(|x| o.min_sig = Some(x)),
        Kind::MaxSig => digit_size(v)
            .filter(|x| *x >= 1)
            .map(|x| o.max_sig = Some(x)),
        Kind::TrailingZero => t
            .and_then(|t| find(&TRAILING, t))
            .map(|x| o.strip_if_integer = Some(x)),
        Kind::Priority => t
            .and_then(|t| find(&PRIORITIES, t))
            .map(|x| o.rounding_priority = Some(x)),
        Kind::Increment => increment_of(v).map(|x| o.rounding_increment = Some(x)),
        Kind::Mode => t
            .and_then(|t| find(&MODES, t))
            .map(|x| o.rounding_mode = Some(x)),
        Kind::Select => None,
    };
    ok.is_some()
}

/// How the digits are rounded (ECMA-402 `[[RoundingType]]`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum RoundingType {
    Fraction,
    Significant,
    More,
    Less,
}

/// ECMA-402 `SetNumberFormatDigitOptions`, resolved once per value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct DigitPlan {
    pub(crate) min_int: u8,
    pub(crate) min_frac: u8,
    pub(crate) max_frac: u8,
    pub(crate) min_sig: u8,
    pub(crate) max_sig: u8,
    pub(crate) ty: RoundingType,
    /// The increment's mantissa and its power of ten.
    pub(crate) increment: (Increment, i16),
    pub(crate) mode: RoundingMode,
    pub(crate) strip_if_integer: bool,
}

/// `m × 10^k` for a `roundingIncrement` value.
fn split_increment(n: u16) -> (Increment, i16) {
    let mut n = n;
    let mut k = 0;
    while n >= 10 && n.is_multiple_of(10) && n != 25 {
        n /= 10;
        k += 1;
    }
    let inc = match n {
        2 => Increment::Two,
        5 => Increment::Five,
        25 => Increment::TwentyFive,
        _ => Increment::One,
    };
    (inc, k)
}

/// The digit plan of options `o` with fraction defaults `frac`; conflicting
/// options report *Bad Option* (once, at resolution).
pub(crate) fn digit_plan(o: &NumOpts, frac: FracDefaults, errs: &mut dyn ErrorSink) -> DigitPlan {
    let increment = o.rounding_increment.unwrap_or(1);
    let mnfd_default = frac.min;
    // A rounding increment makes the maximum default to the minimum.
    let mxfd_default = if increment == 1 { frac.max } else { frac.min };
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
            errs.error(FormatError::BadOption);
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
                    errs.error(FormatError::BadOption);
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
        errs.error(FormatError::BadOption);
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
        increment: split_increment(increment),
        mode: o.rounding_mode.unwrap_or(RoundingMode::HalfExpand),
        strip_if_integer: o.strip_if_integer.unwrap_or(false),
    }
}
