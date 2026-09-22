//! The own digit buffer (owner decision 1): an exact, canonical decimal of
//! up to `CAP` significant digits, stored inline. Every operation is a short
//! loop over at most `CAP + 4` digits; nothing allocates or can panic.

use core::cmp::Ordering;

use super::{INPUT_DIGITS, Increment, MAX_MAGNITUDE, ParseError, RoundingMode, split_literal};
use crate::sink::Sink;

/// Stored capacity: operands have at most [`INPUT_DIGITS`] significant
/// digits; rounding to an increment of 25 can add two.
const CAP: usize = INPUT_DIGITS + 4;

/// Working width of rounding and addition (a carry digit and slack).
const WORK: usize = CAP + 4;

/// An exact decimal: `±0.d₁d₂…dₙ` scaled so the last digit sits at
/// magnitude `low`. Canonical: `digits[..len]` has no leading or trailing
/// zero; zero is `len == 0` (and may be negative: `-0`). `Clone` only, like
/// the `fixed_decimal` backend, so the numeric code is one code path.
#[derive(Clone)]
pub(crate) struct Decimal {
    /// Digit values `0..=9`, most significant first.
    digits: [u8; CAP],
    len: u8,
    /// The magnitude of `digits[len - 1]`; 0 for zero.
    low: i16,
    neg: bool,
}

/// The first `width` digits of the little-endian `w`, reversed.
fn most_significant_first(w: &[u8; WORK], width: usize) -> [u8; WORK] {
    let mut out = [0u8; WORK];
    for (slot, d) in out.iter_mut().zip(w.iter().take(width).rev()) {
        *slot = *d;
    }
    out
}

impl Decimal {
    const ZERO: Decimal = Decimal {
        digits: [0; CAP],
        len: 0,
        low: 0,
        neg: false,
    };

    /// A canonical decimal from the digits of `a` then `b` (most significant
    /// first, zeros anywhere; each byte minus `zero` is the digit, so ASCII
    /// and digit values both work), whose last digit sits at magnitude
    /// `last`. `Err` past `limit` significant digits or the magnitude range.
    /// One non-generic function for every caller (B1: code size).
    #[allow(clippy::many_single_char_names)]
    fn canonical(
        neg: bool,
        a: &[u8],
        b: &[u8],
        zero: u8,
        last: i64,
        limit: usize,
    ) -> Result<Decimal, ParseError> {
        let count = a.len() + b.len();
        let at = |k: usize| -> u8 {
            a.get(k)
                .or_else(|| b.get(k.wrapping_sub(a.len())))
                .map_or(0, |c| c.wrapping_sub(zero))
        };
        let Some(first_nz) = (0..count).find(|&k| at(k) != 0) else {
            let mut z = Decimal::ZERO;
            z.neg = neg;
            return Ok(z);
        };
        let last_nz = (0..count).rev().find(|&k| at(k) != 0).unwrap_or(first_nz);
        let n = last_nz - first_nz + 1;
        if n > limit || n > CAP {
            return Err(ParseError::Limit);
        }
        let trailing = i64::try_from(count - 1 - last_nz).map_err(|_| ParseError::Limit)?;
        let low = last + trailing;
        let high = low + i64::try_from(n).map_err(|_| ParseError::Limit)? - 1;
        let max = i64::from(MAX_MAGNITUDE);
        if low < -max || high > max {
            return Err(ParseError::Limit);
        }
        let mut d = Decimal::ZERO;
        d.neg = neg;
        for (i, slot) in d.digits.iter_mut().take(n).enumerate() {
            *slot = at(first_nz + i);
        }
        d.len = u8::try_from(n).map_err(|_| ParseError::Limit)?;
        d.low = i16::try_from(low).map_err(|_| ParseError::Limit)?;
        Ok(d)
    }

    /// `n`, non-negative.
    pub(crate) fn from_u64(n: u64) -> Decimal {
        let mut buf = [0u8; 20];
        let mut at = buf.len();
        let mut n = n;
        while n > 0 && at > 0 {
            at -= 1;
            // `n % 10 < 10`.
            #[allow(clippy::cast_possible_truncation)]
            let d = (n % 10) as u8;
            if let Some(b) = buf.get_mut(at) {
                *b = d;
            }
            n /= 10;
        }
        let digits = buf.get(at..).unwrap_or(&[]);
        Decimal::canonical(false, digits, &[], 0, 0, CAP).unwrap_or(Decimal::ZERO)
    }

    /// A `number-literal`.
    pub(crate) fn parse(s: &[u8]) -> Result<Decimal, ParseError> {
        let lit = split_literal(s)?;
        let frac = i64::try_from(lit.frac.len()).map_err(|_| ParseError::Limit)?;
        Decimal::canonical(
            lit.neg,
            lit.int,
            lit.frac,
            b'0',
            i64::from(lit.exp) - frac,
            INPUT_DIGITS,
        )
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.len == 0
    }

    pub(crate) fn negative(&self) -> bool {
        self.neg
    }

    pub(crate) fn set_negative(&mut self, neg: bool) {
        self.neg = neg;
    }

    /// The most significant nonzero magnitude; 0 for zero.
    pub(crate) fn high(&self) -> i16 {
        if self.len == 0 {
            0
        } else {
            // |low| + CAP ≤ MAX_MAGNITUDE + CAP < i16::MAX.
            self.low.saturating_add(i16::from(self.len) - 1)
        }
    }

    /// The least significant nonzero magnitude; 0 for zero.
    pub(crate) fn low(&self) -> i16 {
        if self.len == 0 { 0 } else { self.low }
    }

    /// The digit at magnitude `m`.
    pub(crate) fn digit_at(&self, m: i16) -> u8 {
        self.digit_at32(i32::from(m))
    }

    fn digit_at32(&self, m: i32) -> u8 {
        if self.len == 0 {
            return 0;
        }
        let high = i32::from(self.high());
        if m < i32::from(self.low) || m > high {
            return 0;
        }
        usize::try_from(high - m)
            .ok()
            .and_then(|i| self.digits.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// Multiplies by `10^k`; `false` (unchanged) past the magnitude range.
    pub(crate) fn shift(&mut self, k: i16) -> bool {
        if self.len == 0 {
            return true;
        }
        let low = i32::from(self.low) + i32::from(k);
        let high = i32::from(self.high()) + i32::from(k);
        if low < -MAX_MAGNITUDE || high > MAX_MAGNITUDE {
            return false;
        }
        match i16::try_from(low) {
            Ok(low) => {
                self.low = low;
                true
            }
            Err(_) => false,
        }
    }

    /// `true` if the value is an integer.
    pub(crate) fn is_integer(&self) -> bool {
        self.len == 0 || self.low >= 0
    }

    /// The value as an `i64` when it is an integer below 10^18 in magnitude.
    pub(crate) fn to_i64(&self) -> Option<i64> {
        if !self.is_integer() || self.high() >= 18 {
            return None;
        }
        let mut n: i64 = 0;
        let mut m = self.high();
        // At most 18 digits: n < 10^18 never overflows.
        while m >= 0 {
            n = n * 10 + i64::from(self.digit_at(m));
            m -= 1;
        }
        Some(if self.neg { -n } else { n })
    }

    /// Rounds to a multiple of `inc × 10^pos` under `mode` (ECMA-402's
    /// rounding, with `fixed_decimal`'s increment semantics). Zero keeps its
    /// sign, and so does a value that rounds to zero (`-0.4` → `-0`).
    // The names are the arithmetic's: `t0`/`t1` the lowest kept digits, `u`
    // their value, `a` the remainder mod the increment, `g` the comparison
    // with half, `w` the working digits.
    #[allow(clippy::many_single_char_names)]
    pub(crate) fn round(&mut self, pos: i16, mode: RoundingMode, inc: Increment) {
        if self.len == 0 {
            return;
        }
        let pos = i32::from(pos);
        let low = i32::from(self.low);
        let high = i32::from(self.high());
        // The dropped part F (below `pos`), against half a unit.
        let f_zero = low >= pos;
        let d1 = self.digit_at32(pos - 1);
        let sticky = low < pos - 1;
        let f_half = if f_zero {
            Ordering::Less
        } else if d1 > 5 || (d1 == 5 && sticky) {
            Ordering::Greater
        } else if d1 == 5 {
            Ordering::Equal
        } else {
            Ordering::Less
        };
        let t0 = self.digit_at32(pos);
        let t1 = self.digit_at32(pos + 1);
        let u = 10 * t1 + t0;
        // `a` = T mod inc (T: the value in units of 10^pos, truncated); `g`:
        // (a + F) / inc against ½; `odd`: the parity of T div inc.
        let (a, g, odd) = match inc {
            Increment::One => (0, f_half, t0 % 2 == 1),
            Increment::Two => {
                let a = t0 % 2;
                let g = match (a, f_zero) {
                    (1, true) => Ordering::Equal,
                    (1, false) => Ordering::Greater,
                    _ => Ordering::Less,
                };
                (a, g, ((u % 4 - a) / 2) % 2 == 1)
            }
            Increment::Five => {
                let a = t0 % 5;
                let g = match a {
                    3.. => Ordering::Greater,
                    2 => f_half,
                    _ => Ordering::Less,
                };
                (a, g, t0 >= 5)
            }
            Increment::TwentyFive => {
                let a = u % 25;
                let g = match a {
                    13.. => Ordering::Greater,
                    12 => f_half,
                    _ => Ordering::Less,
                };
                (a, g, ((u - a) / 25) % 2 == 1)
            }
        };
        if a == 0 && f_zero {
            return; // already a multiple
        }
        let up = match mode {
            RoundingMode::Ceil => !self.neg,
            RoundingMode::Floor => self.neg,
            RoundingMode::Expand => true,
            RoundingMode::Trunc => false,
            RoundingMode::HalfCeil => g == Ordering::Greater || (g == Ordering::Equal && !self.neg),
            RoundingMode::HalfFloor => g == Ordering::Greater || (g == Ordering::Equal && self.neg),
            RoundingMode::HalfExpand => g != Ordering::Less,
            RoundingMode::HalfTrunc => g == Ordering::Greater,
            RoundingMode::HalfEven => g == Ordering::Greater || (g == Ordering::Equal && odd),
        };
        // T' = T − a (+ inc), little-endian from magnitude `pos`.
        let mut w = [0u8; WORK];
        let top = high.max(pos + 1) + 1;
        let Ok(width) = usize::try_from(top - pos + 1) else {
            return;
        };
        if width > WORK {
            return; // cannot happen: `pos ≥ low − 1` whenever work is left
        }
        for (i, slot) in w.iter_mut().enumerate().take(width) {
            let m = pos + i32::try_from(i).unwrap_or(0);
            *slot = self.digit_at32(m);
        }
        let u2 = u - a;
        let [w0, w1, ..] = &mut w;
        *w0 = u2 % 10;
        *w1 = u2 / 10;
        if up {
            let mut carry = inc.value();
            for slot in w.iter_mut().take(width) {
                if carry == 0 {
                    break;
                }
                let s = *slot + carry;
                *slot = s % 10;
                carry = s / 10;
            }
        }
        let msd = most_significant_first(&w, width);
        if let Ok(d) = Decimal::canonical(
            self.neg,
            msd.get(..width).unwrap_or(&[]),
            &[],
            0,
            i64::from(pos),
            CAP,
        ) {
            *self = d;
        }
    }

    /// `self + other`, exactly; `None` past [`INPUT_DIGITS`] significant
    /// digits (Unsupported Operation, `number.md` `:offset`).
    #[allow(clippy::many_single_char_names)]
    pub(crate) fn add(&self, other: &Decimal) -> Option<Decimal> {
        // A zero addend leaves the other's value and sign; only zero + zero
        // decides the sign of zero (-0 + -0 = -0, else +0), as IEEE 754 and
        // the `fixed_decimal` baseline do.
        if other.len == 0 {
            let mut r = self.clone();
            if self.len == 0 {
                r.neg = self.neg && other.neg;
            }
            return Some(r);
        }
        if self.len == 0 {
            return Some(other.clone());
        }
        let lo = i32::from(self.low.min(other.low));
        let hi = i32::from(self.high().max(other.high())) + 1;
        let width = usize::try_from(hi - lo + 1).ok()?;
        if width > WORK {
            return None;
        }
        let mut x = [0u8; WORK];
        let mut y = [0u8; WORK];
        for i in 0..width {
            let m = lo + i32::try_from(i).ok()?;
            *x.get_mut(i)? = self.digit_at32(m);
            *y.get_mut(i)? = other.digit_at32(m);
        }
        let mut r = [0u8; WORK];
        let neg = if self.neg == other.neg {
            let mut carry = 0;
            for i in 0..width {
                let s = x.get(i)? + y.get(i)? + carry;
                *r.get_mut(i)? = s % 10;
                carry = s / 10;
            }
            self.neg
        } else {
            // Subtract the smaller magnitude from the larger.
            let mut ord = Ordering::Equal;
            for i in (0..width).rev() {
                ord = x.get(i)?.cmp(y.get(i)?);
                if ord != Ordering::Equal {
                    break;
                }
            }
            let (big, small, neg) = match ord {
                Ordering::Equal => return Some(Decimal::ZERO),
                Ordering::Greater => (&x, &y, self.neg),
                Ordering::Less => (&y, &x, other.neg),
            };
            let mut borrow = 0;
            for i in 0..width {
                let s = small.get(i)? + borrow;
                let b = *big.get(i)?;
                if b >= s {
                    *r.get_mut(i)? = b - s;
                    borrow = 0;
                } else {
                    *r.get_mut(i)? = b + 10 - s;
                    borrow = 1;
                }
            }
            neg
        };
        let msd = most_significant_first(&r, width);
        Decimal::canonical(
            neg,
            msd.get(..width).unwrap_or(&[]),
            &[],
            0,
            i64::from(lo),
            INPUT_DIGITS,
        )
        .ok()
    }

    /// Writes the digits of magnitudes `hi` down to `lo` (inclusive), with a
    /// `.` before magnitude −1; zeros outside the stored digits.
    pub(crate) fn write_digits(&self, hi: i16, lo: i16, out: &mut dyn Sink) {
        let mut buf = [0u8; 64];
        let mut n = 0;
        let mut m = i32::from(hi);
        let lo = i32::from(lo);
        while m >= lo {
            if m == -1
                && i32::from(hi) >= 0
                && let Some(b) = buf.get_mut(n)
            {
                *b = b'.';
                n += 1;
            }
            if let Some(b) = buf.get_mut(n) {
                *b = b'0' + self.digit_at32(m);
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

    /// Writes the exact value: `-1234.5`, `0.001`, `-0`; no exponent.
    pub(crate) fn write_plain(&self, out: &mut dyn Sink) {
        if self.neg {
            out.push_str("-");
        }
        let hi = self.high().max(0);
        let lo = self.low().min(0);
        self.write_digits(hi, lo, out);
    }
}
