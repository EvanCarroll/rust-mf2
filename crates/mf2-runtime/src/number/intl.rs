//! The `intl` backend of the numeric functions
//! (owner decision 4): with feature `web-number-intl` and without
//! `web-number-builtin`, on `wasm32-unknown-unknown`
//! ([`crate::INTL_NUMBERS`]), the display, `:integer`'s rounding and the
//! plural category come from the host's number formatter
//! ([`Host::numbers`](crate::Host::numbers): the browser's
//! `Intl.NumberFormat` and `Intl.PluralRules`), so the Rust rounding, digit
//! output and plural evaluator are not linked. Option validation, the
//! operand rules, `:offset`, inheritance and exact-match keys stay in Rust
//! (`number.rs`). Same interface as the Rust backend (`display.rs`).
//!
//! A host without a number formatter (`Host::numbers` is `None`: the
//! default, or a browser without `Intl.NumberFormat` v3) gets the
//! value's exact digits in neutral symbols, keyword selection matches only
//! `other`, and every numeric resolution reports *Unsupported Operation*.

use core::cell::Cell;

use super::decimal::{Decimal, RoundingMode};
use super::options::{DigitPlan, Grouping, NumOpts, RoundingPriority, SignDisplay};
use super::request::{DigitOptions, INTL_DIGITS, NumberOut, NumberRequest, NumberStyle, Text};
use super::{Digits, Number, Resolved};
use crate::error::FormatError;
use crate::function::FnContext;
use crate::plural::Category;
use crate::sink::{ErrorSink, Sink, SubPartSink};

/// This backend formats through the host.
pub(super) const BY_HOST: bool = true;

/// What a resolved number keeps: the digit plan the host is asked with, and
/// the plural category once the host gave it (`Category as u8 + 1`; 0 = not
/// yet), so a selector asks once, not once per key.
#[derive(Clone)]
pub(super) struct Shown {
    plan: DigitPlan,
    category: Cell<u8>,
}

/// `:integer`'s value: `value` rounded to an integer with `mode` by the host
/// (0 fraction digits, neutral); unchanged when the host cannot.
pub(super) fn round_to_integer(value: &mut Decimal, mode: RoundingMode, cx: &FnContext<'_>) {
    if value.is_integer() {
        return;
    }
    let text = Text::plain(value);
    let request = NumberRequest {
        value: text.as_str(),
        style: NumberStyle::Decimal,
        neutral: true,
        digits: DigitOptions {
            minimum_integer: 1,
            fraction: Some((0, 0)),
            significant: None,
            priority: RoundingPriority::Auto,
            increment: 1,
            mode,
            strip_if_integer: false,
        },
        sign: SignDisplay::Auto,
        grouping: Grouping::Never,
        ordinal: false,
    };
    let mut out = Text::new();
    if let Some(f) = cx.host().numbers()
        && f.format(cx.locale(), &request, NumberOut::Text(&mut out))
        && let Ok(rounded) = Decimal::parse(out.as_str().as_bytes())
    {
        *value = rounded;
    }
}

/// *Unsupported Operation* when the host has no number formatter
/// (reported once per value, at resolution).
pub(super) fn check_host(cx: &FnContext<'_>, errs: &mut dyn ErrorSink) {
    if cx.host().numbers().is_none() {
        errs.error(FormatError::UnsupportedOperation);
    }
}

/// What a resolved number keeps.
pub(super) fn shown(_value: &Decimal, _scale: i16, plan: &DigitPlan, _sign: SignDisplay) -> Shown {
    Shown {
        plan: *plan,
        category: Cell::new(0),
    }
}

/// The host rounds and formats: no digits here.
pub(super) fn digits(_r: &Resolved) -> Option<Digits<'_>> {
    None
}

/// The digit plan the host is asked with.
pub(super) fn plan(r: &Resolved) -> DigitPlan {
    r.shown.plan
}

/// Writes a number in neutral symbols, from the host; its exact digits when
/// the host has no formatter.
pub(super) fn write_neutral(n: &Number, cx: &FnContext<'_>, out: &mut dyn Sink) {
    if !n.format_by_host(cx, NumberStyle::Decimal, true, NumberOut::Text(&mut *out)) {
        n.exact_digits().write_neutral(out);
    }
}

/// The neutral display as sub-parts.
pub(super) fn neutral_parts(n: &Number, cx: &FnContext<'_>, out: &mut dyn SubPartSink) {
    if !n.format_by_host(cx, NumberStyle::Decimal, true, NumberOut::Parts(&mut *out)) {
        n.exact_digits().neutral_parts(out);
    }
}

/// The value × 10^scale as plain text: what selection sees.
fn scaled(n: &Number, r: &Resolved) -> Text {
    if r.scale == 0 {
        return Text::plain(&n.value);
    }
    let mut v = n.value.clone();
    v.shift(r.scale);
    Text::plain(&v)
}

/// The display selection compares exact keys with: the value selection
/// sees (× 10^scale), in neutral symbols, from the host.
pub(super) fn write_selected(n: &Number, r: &Resolved, cx: &FnContext<'_>, out: &mut dyn Sink) {
    let text = scaled(n, r);
    let request = r.request(text.as_str(), NumberStyle::Decimal, true);
    if !cx
        .host()
        .numbers()
        .is_some_and(|f| f.format(cx.locale(), &request, NumberOut::Text(&mut *out)))
    {
        out.push_str(text.as_str());
    }
}

/// The plural category (cardinal or ordinal, as `select` says) of the value
/// selection sees under its digit options, from the host
/// (`Intl.PluralRules`), once per value; `other` when the host has none.
pub(super) fn category(n: &Number, r: &Resolved, cx: &FnContext<'_>) -> Category {
    let cached = r.shown.category.get();
    if cached != 0 {
        return Category::from_code(cached - 1);
    }
    let text = scaled(n, r);
    let request = r.request(text.as_str(), NumberStyle::Decimal, false);
    let c = cx
        .host()
        .numbers()
        .and_then(|f| f.plural(cx.locale(), &request))
        .unwrap_or(Category::Other);
    r.shown.category.set(c as u8 + 1);
    c
}

/// `Intl`'s limits on digit sizes (ECMA-402: integer and significant digits
/// 1–21; fraction digits 0–100 hold every MF2 value, 0–99): a value past
/// them is *Bad Option* and replaced by the limit, which becomes the
/// option's resolved value (`number.md`, "Digit Size Options").
pub(super) fn limit_digit_sizes(o: &mut NumOpts, errs: &mut dyn ErrorSink) {
    for v in [&mut o.min_int, &mut o.min_sig, &mut o.max_sig] {
        if let Some(n) = v
            && *n > INTL_DIGITS
        {
            errs.error(FormatError::BadOption);
            *n = INTL_DIGITS;
        }
    }
}
