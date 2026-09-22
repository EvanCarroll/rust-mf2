//! What a numeric function asks of the host's number formatter
//! (`plans/03-runtime.md` §2.7, "the `intl` option"; §5.3): the exact value
//! and the options the function resolved, in the terms of ECMA-402's
//! `Intl.NumberFormat` and `Intl.PluralRules` — MF2 took its option names
//! and meanings from there — and [`Number::format_by_host`], which builds
//! such a request. On every build; only the `intl` backend (`intl.rs`) and
//! `mf2-fn-number`'s `intl` path call it.

use alloc::string::String;

use super::decimal::{Decimal, RoundingMode};
use super::options::{DigitPlan, Grouping, RoundingPriority, RoundingType, Select, SignDisplay};
use super::{Number, Resolved, backend};
use crate::function::FnContext;
use crate::sink::{Sink, SubPartSink};

/// A number for a [`NumberFormatter`](crate::NumberFormatter) (the host's,
/// [`Host::numbers`](crate::Host::numbers)). Built by the runtime; a host
/// reads it.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct NumberRequest<'r> {
    /// The exact value in plain neutral digits, unrounded, no exponent
    /// (`-1234.5`, `0.001`, `-0`): `Intl.NumberFormat` v3 formats such a
    /// string exactly. Unscaled for [`NumberStyle::Percent`] (the style
    /// multiplies by 100); a plural request, or a neutral one for an exact
    /// key, carries the value selection sees (× 100 for `:percent`).
    pub value: &'r str,
    /// Decimal, percent, currency or unit.
    pub style: NumberStyle<'r>,
    /// The core's neutral output — ASCII digits, `.`, `-`/`+`, no grouping —
    /// instead of the locale's symbols (in the browser: locale `en`,
    /// `numberingSystem: "latn"`, `useGrouping: false`).
    pub neutral: bool,
    /// The digit options, resolved.
    pub digits: DigitOptions,
    /// `signDisplay`.
    pub sign: SignDisplay,
    /// `useGrouping` (a neutral request never groups).
    pub grouping: Grouping,
    /// `select=ordinal`: the plural rules' type for
    /// [`NumberFormatter::plural`](crate::NumberFormatter::plural).
    pub ordinal: bool,
}

/// ECMA-402's digit options as `SetNumberFormatDigitOptions` resolved them
/// for MF2 — so never a set `Intl` rejects: where it would throw, the
/// numeric function reported *Bad Option* and dropped or replaced the
/// option (`plans/03-runtime.md` §5.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub struct DigitOptions {
    /// `minimumIntegerDigits`, 1–21.
    pub minimum_integer: u8,
    /// `minimumFractionDigits`, `maximumFractionDigits` (0–100); `None`:
    /// the style's defaults (a currency's own digits) or, with
    /// `significant` and `priority` `Auto`, rounding by significant digits
    /// alone.
    pub fraction: Option<(u8, u8)>,
    /// `minimumSignificantDigits`, `maximumSignificantDigits` (1–21);
    /// `None`: rounding by fraction digits alone.
    pub significant: Option<(u8, u8)>,
    /// `roundingPriority` (not `Auto` exactly when both are set).
    pub priority: RoundingPriority,
    /// `roundingIncrement` (1: none).
    pub increment: u16,
    /// `roundingMode`.
    pub mode: RoundingMode,
    /// `trailingZeroDisplay: "stripIfInteger"`.
    pub strip_if_integer: bool,
}

/// How a [`NumberRequest`] is shown.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum NumberStyle<'a> {
    /// A plain number.
    Decimal,
    /// `:percent`: the value × 100, with the locale's percent pattern.
    Percent,
    /// `:currency`.
    Currency {
        /// A well-formed code, upper case.
        code: &'a str,
        /// `currencyDisplay`.
        display: CurrencyDisplay,
        /// `currencySign=accounting`.
        accounting: bool,
        /// `fractionDigits` unset or `auto`: the currency's own fraction
        /// digits, the formatter's — the request then has no fraction digits
        /// ([`DigitOptions::fraction`] `None`).
        own_digits: bool,
    },
    /// `:unit`.
    Unit {
        /// A well-formed unit identifier.
        unit: &'a str,
        /// `unitDisplay`.
        display: UnitDisplay,
    },
}

/// `:currency`'s `currencyDisplay`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CurrencyDisplay {
    /// `symbol` (the default).
    Symbol,
    /// `narrowSymbol`.
    NarrowSymbol,
    /// `name`.
    Name,
    /// `code`.
    Code,
    /// `never`: no currency shown.
    Never,
}

/// `:unit`'s `unitDisplay`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UnitDisplay {
    /// `short` (the default).
    Short,
    /// `narrow`.
    Narrow,
    /// `long`.
    Long,
}

/// Where [`NumberFormatter::format`](crate::NumberFormatter::format) writes: text,
/// or sub-parts (`formatToParts`: `integer`, `group`, `decimal`,
/// `fraction`, `minusSign`, `plusSign`, `percentSign`, `currency`, `unit`,
/// `literal`, …).
pub enum NumberOut<'o> {
    /// The formatted text.
    Text(&'o mut dyn Sink),
    /// The formatted parts.
    Parts(&'o mut dyn SubPartSink),
}

/// Text on the stack — a number's plain digits, almost always short — that
/// spills to the heap past 64 bytes (through the guarded `Sink for String`).
pub(crate) struct Text {
    buf: [u8; 64],
    len: usize,
    heap: Option<String>,
}

impl Text {
    pub(crate) const fn new() -> Text {
        Text {
            buf: [0; 64],
            len: 0,
            heap: None,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match &self.heap {
            Some(s) => s,
            None => core::str::from_utf8(self.buf.get(..self.len).unwrap_or(&[])).unwrap_or(""),
        }
    }

    /// The plain text of `d`.
    pub(crate) fn plain(d: &Decimal) -> Text {
        let mut t = Text::new();
        d.write_plain(&mut t);
        t
    }
}

impl Sink for Text {
    fn push_str(&mut self, s: &str) {
        if let Some(h) = &mut self.heap {
            Sink::push_str(h, s);
            return;
        }
        let end = self.len + s.len();
        if end <= self.buf.len() {
            // A zip, not `copy_from_slice`: no length-mismatch panic path (B12).
            for (d, &b) in self.buf.iter_mut().skip(self.len).zip(s.as_bytes()) {
                *d = b;
            }
            self.len = end;
            return;
        }
        let mut h = String::new();
        Sink::push_str(&mut h, self.as_str());
        Sink::push_str(&mut h, s);
        self.heap = Some(h);
    }
}

/// `Intl`'s limit for integer and significant digits (ECMA-402: 1–21).
pub(crate) const INTL_DIGITS: u8 = 21;

impl DigitPlan {
    /// The plan as ECMA-402's options.
    fn options(&self) -> DigitOptions {
        let (inc, k) = self.increment;
        let mut increment = u16::from(inc.value());
        for _ in 0..k {
            increment = increment.saturating_mul(10);
        }
        let clamp = |n: u8| n.clamp(1, INTL_DIGITS);
        DigitOptions {
            minimum_integer: clamp(self.min_int),
            fraction: (self.ty != RoundingType::Significant)
                .then_some((self.min_frac, self.max_frac)),
            significant: (self.ty != RoundingType::Fraction)
                .then_some((clamp(self.min_sig), clamp(self.max_sig))),
            priority: match self.ty {
                RoundingType::More => RoundingPriority::MorePrecision,
                RoundingType::Less => RoundingPriority::LessPrecision,
                RoundingType::Fraction | RoundingType::Significant => RoundingPriority::Auto,
            },
            increment,
            mode: self.mode,
            strip_if_integer: self.strip_if_integer,
        }
    }
}

/// The digit options that show `d` exactly: all its fraction digits (up to
/// `Intl`'s 100), no rounding before them.
fn exact_options(d: &Decimal) -> DigitOptions {
    let f = u8::try_from(-i32::from(d.low().min(0)))
        .unwrap_or(100)
        .min(100);
    DigitOptions {
        minimum_integer: 1,
        fraction: Some((f, f)),
        significant: None,
        priority: RoundingPriority::Auto,
        increment: 1,
        mode: RoundingMode::HalfExpand,
        strip_if_integer: false,
    }
}

impl Resolved {
    /// The request for `value` (a resolved number's value, or the value
    /// selection sees) under these options.
    pub(super) fn request<'r>(
        &self,
        value: &'r str,
        style: NumberStyle<'r>,
        neutral: bool,
    ) -> NumberRequest<'r> {
        let mut digits = backend::plan(self).options();
        if let NumberStyle::Currency {
            own_digits: true, ..
        } = style
        {
            digits.fraction = None;
        }
        NumberRequest {
            value,
            style,
            neutral,
            digits,
            sign: self.opts.sign_display.unwrap_or(SignDisplay::Auto),
            grouping: self.opts.use_grouping.unwrap_or(Grouping::Auto),
            ordinal: self.opts.select == Some(Select::Ordinal),
        }
    }
}

impl Number {
    /// Formats this number with the host's number formatter
    /// ([`Host::numbers`](crate::Host::numbers)) in `style` —
    /// neutral, or with the catalog locale's symbols — as text or as
    /// sub-parts: a resolved number's display (its digit options,
    /// `signDisplay`, `useGrouping`), a bare number's exact value. `false`:
    /// the host has no number formatter (nothing written). The `intl` path
    /// of the numeric functions (`plans/03-runtime.md` §2.7); on any build
    /// it only asks the host.
    pub fn format_by_host(
        &self,
        cx: &FnContext<'_>,
        style: NumberStyle<'_>,
        neutral: bool,
        out: NumberOut<'_>,
    ) -> bool {
        let text = Text::plain(&self.value);
        let request = match &self.resolved {
            Some(r) => r.request(text.as_str(), style, neutral),
            None => NumberRequest {
                value: text.as_str(),
                style,
                neutral,
                digits: exact_options(&self.value),
                sign: SignDisplay::Auto,
                grouping: Grouping::Auto,
                ordinal: false,
            },
        };
        cx.host()
            .numbers()
            .is_some_and(|f| f.format(cx.locale(), &request, out))
    }
}
