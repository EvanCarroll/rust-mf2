//! The A/B baseline of owner decision 1 (feature `fixed-decimal`): the same
//! interface as the own buffer, over `fixed_decimal` 0.7 — what P0.5
//! measured. Never enabled by a client (it keeps `fixed_decimal`'s panic
//! paths, which is why the own buffer exists).

use fixed_decimal::{RoundingIncrement, Sign, SignedRoundingMode, UnsignedRoundingMode};

use super::{INPUT_DIGITS, Increment, MAX_MAGNITUDE, ParseError, RoundingMode, split_literal};
use crate::sink::Sink;

/// A canonical decimal (no leading or trailing zeros stored).
#[derive(Clone)]
pub(crate) struct Decimal(fixed_decimal::Decimal);

fn mode(m: RoundingMode) -> SignedRoundingMode {
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

impl Decimal {
    fn canonical(mut d: fixed_decimal::Decimal) -> Decimal {
        d.absolute.trim_start();
        d.absolute.trim_end();
        Decimal(d)
    }

    pub(crate) fn from_u64(n: u64) -> Decimal {
        Decimal::canonical(fixed_decimal::Decimal::from(n))
    }

    pub(crate) fn parse(s: &[u8]) -> Result<Decimal, ParseError> {
        let lit = split_literal(s)?;
        let digits = lit.int.iter().chain(lit.frac.iter());
        let first = digits.clone().position(|&c| c != b'0');
        let last = digits
            .clone()
            .enumerate()
            .filter(|&(_, &c)| c != b'0')
            .map(|(i, _)| i)
            .last();
        if let (Some(a), Some(b)) = (first, last)
            && b - a + 1 > INPUT_DIGITS
        {
            return Err(ParseError::Limit);
        }
        let mantissa_len = if lit.frac.is_empty() {
            lit.int.len()
        } else {
            lit.int.len() + 1 + lit.frac.len()
        };
        let start = usize::from(lit.neg);
        let mantissa = s
            .get(start..start + mantissa_len)
            .ok_or(ParseError::Syntax)?;
        let mut d =
            fixed_decimal::Decimal::try_from_utf8(mantissa).map_err(|_| ParseError::Limit)?;
        d.absolute
            .multiply_pow10(i16::try_from(lit.exp).map_err(|_| ParseError::Limit)?);
        if lit.neg {
            d.sign = Sign::Negative;
        }
        let d = Decimal::canonical(d);
        if !d.is_zero()
            && (i32::from(d.low()) < -MAX_MAGNITUDE || i32::from(d.high()) > MAX_MAGNITUDE)
        {
            return Err(ParseError::Limit);
        }
        Ok(d)
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.absolute.is_zero()
    }

    pub(crate) fn negative(&self) -> bool {
        self.0.sign == Sign::Negative
    }

    pub(crate) fn set_negative(&mut self, neg: bool) {
        self.0.sign = if neg { Sign::Negative } else { Sign::None };
    }

    pub(crate) fn high(&self) -> i16 {
        if self.is_zero() {
            0
        } else {
            self.0.absolute.nonzero_magnitude_start()
        }
    }

    pub(crate) fn low(&self) -> i16 {
        if self.is_zero() {
            0
        } else {
            self.0.absolute.nonzero_magnitude_end()
        }
    }

    pub(crate) fn digit_at(&self, m: i16) -> u8 {
        self.0.absolute.digit_at(m)
    }

    pub(crate) fn shift(&mut self, k: i16) -> bool {
        self.0.absolute.multiply_pow10(k);
        true
    }

    pub(crate) fn is_integer(&self) -> bool {
        self.is_zero() || self.low() >= 0
    }

    pub(crate) fn to_i64(&self) -> Option<i64> {
        if !self.is_integer() || self.high() >= 18 {
            return None;
        }
        let mut n: i64 = 0;
        let mut m = self.high();
        while m >= 0 {
            n = n * 10 + i64::from(self.digit_at(m));
            m -= 1;
        }
        Some(if self.negative() { -n } else { n })
    }

    pub(crate) fn round(&mut self, pos: i16, m: RoundingMode, inc: Increment) {
        let inc = match inc {
            Increment::One => RoundingIncrement::MultiplesOf1,
            Increment::Two => RoundingIncrement::MultiplesOf2,
            Increment::Five => RoundingIncrement::MultiplesOf5,
            Increment::TwentyFive => RoundingIncrement::MultiplesOf25,
        };
        self.0.round_with_mode_and_increment(pos, mode(m), inc);
        self.0.absolute.trim_start();
        self.0.absolute.trim_end();
    }

    /// `self + other` through `i64` (P0.5's `add_small`): `None` past 17
    /// digits of span.
    pub(crate) fn add(&self, other: &Decimal) -> Option<Decimal> {
        let lo = self.low().min(other.low()).min(0);
        let hi = self.high().max(other.high()).max(0);
        if hi - lo > 17 {
            return None;
        }
        let scaled = |d: &Decimal| {
            let mut n: i64 = 0;
            let mut m = hi;
            while m >= lo {
                n = n * 10 + i64::from(d.digit_at(m));
                m -= 1;
            }
            if d.negative() { -n } else { n }
        };
        let sum = scaled(self) + scaled(other);
        let mut d = fixed_decimal::Decimal::from(sum);
        d.absolute.multiply_pow10(lo);
        if sum == 0 {
            d.sign = if self.negative() && other.negative() {
                Sign::Negative
            } else {
                Sign::None
            };
        }
        Some(Decimal::canonical(d))
    }

    pub(crate) fn write_digits(&self, hi: i16, lo: i16, out: &mut dyn Sink) {
        let mut buf = [0u8; 64];
        let mut n = 0;
        let mut m = hi;
        while m >= lo {
            if m == -1
                && let Some(b) = buf.get_mut(n)
            {
                *b = b'.';
                n += 1;
            }
            if let Some(b) = buf.get_mut(n) {
                *b = b'0' + self.digit_at(m);
                n += 1;
            }
            if n >= buf.len() - 1 {
                out.push_str(core::str::from_utf8(buf.get(..n).unwrap_or(&[])).unwrap_or(""));
                n = 0;
            }
            m -= 1;
        }
        out.push_str(core::str::from_utf8(buf.get(..n).unwrap_or(&[])).unwrap_or(""));
    }

    pub(crate) fn write_plain(&self, out: &mut dyn Sink) {
        if self.negative() {
            out.push_str("-");
        }
        self.write_digits(self.high().max(0), self.low().min(0), out);
    }
}
