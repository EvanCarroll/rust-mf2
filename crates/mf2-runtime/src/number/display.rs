//! The Rust backend of the numeric functions (every build except feature
//! `intl` on `wasm32-unknown-unknown`, [`crate::INTL_NUMBERS`]): ECMA-402's
//! `ToRawFixed` / `ToRawPrecision` over the own digit buffer, the digits to
//! display, and the plural category from the catalog's rules. The `intl`
//! backend (`intl.rs`) has the same interface and asks the host instead.

use mf2_catalog::format::locale_key;

use super::decimal::{Decimal, Increment, RoundingMode};
use super::options::{DigitPlan, NumOpts, RoundingType, Select, SignDisplay, digit_plan};
use super::{Digits, Number, Resolved, Sign};
use crate::function::FnContext;
use crate::plural::{self, Category};
use crate::sink::{ErrorSink, NoErrors, Sink, SubPartSink};

/// What a resolved number keeps for formatting and selection: the digits
/// to display — rounded, with the visible magnitude range and the sign
/// after `signDisplay`.
#[derive(Clone)]
pub(super) struct Shown {
    dec: Decimal,
    /// The lowest visible magnitude (≤ 0).
    lo: i16,
    /// The highest visible magnitude (≥ 0).
    hi: i16,
    sign: Sign,
}

impl Shown {
    pub(super) fn digits(&self) -> Digits<'_> {
        Digits {
            dec: &self.dec,
            lo: self.lo,
            hi: self.hi,
            sign: self.sign,
        }
    }
}

/// `:integer`'s value: `value` rounded to an integer with `mode`.
pub(super) fn round_to_integer(value: &mut Decimal, mode: RoundingMode, _cx: &FnContext<'_>) {
    value.round(0, mode, Increment::One);
}

/// The display of `value` × 10^`scale` under `plan` and `sign`.
pub(super) fn shown(value: &Decimal, scale: i16, plan: &DigitPlan, sign: SignDisplay) -> Shown {
    let mut x = value.clone();
    if scale != 0 {
        x.shift(scale);
    }
    let p = plan;
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
        SignDisplay::ExceptZero if zero => Sign::None,
        SignDisplay::Auto | SignDisplay::Always | SignDisplay::ExceptZero if negative => {
            Sign::Minus
        }
        SignDisplay::Always | SignDisplay::ExceptZero => Sign::Plus,
        SignDisplay::Negative if negative && !zero => Sign::Minus,
        _ => Sign::None,
    };
    Shown {
        dec: d,
        lo,
        hi,
        sign,
    }
}

/// ECMA-402 `ToRawFixed`: `(rounded, lowest visible, rounding magnitude)`.
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

/// The rounded digits of a resolved number (the `intl` backend has none:
/// the same signature).
#[allow(clippy::unnecessary_wraps)]
pub(super) fn digits(r: &Resolved) -> Option<Digits<'_>> {
    Some(r.shown.digits())
}

/// The digit plan a resolved number was displayed with (for
/// [`Number::format_by_host`]): recomputed, since only the display is kept.
pub(super) fn plan(r: &Resolved) -> DigitPlan {
    digit_plan(&r.opts, r.frac, &mut NoErrors)
}

/// Writes a number in neutral symbols (a bare number: its plain value).
pub(super) fn write_neutral(n: &Number, _cx: &FnContext<'_>, out: &mut dyn Sink) {
    match &n.resolved {
        Some(r) => r.shown.digits().write_neutral(out),
        None => n.write_plain(out),
    }
}

/// The neutral display as sub-parts.
pub(super) fn neutral_parts(n: &Number, _cx: &FnContext<'_>, out: &mut dyn SubPartSink) {
    match &n.resolved {
        Some(r) => r.shown.digits().neutral_parts(out),
        None => n.plain_parts(out),
    }
}

/// The display selection compares exact keys with (in neutral symbols).
pub(super) fn write_selected(n: &Number, _r: &Resolved, cx: &FnContext<'_>, out: &mut dyn Sink) {
    write_neutral(n, cx, out);
}

/// The plural category (cardinal or ordinal, as `select` says) of the
/// displayed digits, by the catalog's rules.
pub(super) fn category(_n: &Number, r: &Resolved, cx: &FnContext<'_>) -> Category {
    let entry_key = match r.opts.select {
        Some(Select::Ordinal) => locale_key::PLURAL_ORDINAL,
        _ => locale_key::PLURAL_CARDINAL,
    };
    let rules = cx.catalog().locale_entry(entry_key).unwrap_or(&[]);
    plural::select(rules, &r.shown.digits().operands())
}

/// The host is not asked.
pub(super) fn check_host(_cx: &FnContext<'_>, _errs: &mut dyn ErrorSink) {}

/// The implementation's limits on digit sizes: the Rust path's are the
/// option parser's (0–99), nothing more to check.
pub(super) fn limit_digit_sizes(_o: &mut NumOpts, _errs: &mut dyn ErrorSink) {}
