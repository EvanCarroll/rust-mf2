//! UTS #35 plural operands and their extraction from a decimal string.
//!
//! In the product the operands come from the number formatter (the formatted
//! digits decide the category); this string form exists for the CLDR sample
//! tests and accepts the sample syntax, including the compact-exponent forms
//! `1c6`, `1.20050c3` and `1.1e6`.

/// 10^18: integer and fraction values at or above it are kept as
/// `10^18 + (value mod 10^18)`.
///
/// That is exact for every relation whose modulus divides 10^18 (CLDR only uses
/// powers of ten up to 10^6) and for every comparison against a rule literal
/// below 10^18, because such a value compares greater than all of them.
pub const BIG: u64 = 1_000_000_000_000_000_000;

/// The operands of a (non-negative) decimal number `n` (UTS #35 Part 3 §5.1.1).
/// `n` itself is not stored: it is integral iff `t == 0`, and then equals `i`.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Operands {
    /// Integer digits of `n`.
    pub i: u64,
    /// Visible fraction digits, with trailing zeros, as an integer.
    pub f: u64,
    /// Visible fraction digits, without trailing zeros, as an integer.
    pub t: u64,
    /// Number of visible fraction digits, with trailing zeros.
    pub v: u32,
    /// Number of visible fraction digits, without trailing zeros.
    pub w: u32,
    /// Compact decimal exponent (`c`; `e` is its synonym).
    pub e: u32,
}

/// A decimal accumulator that is exact below [`BIG`] and otherwise keeps the
/// value modulo 10^18 plus a sticky "big" flag.
#[derive(Clone, Copy, Default)]
struct Acc {
    low: u64,
    big: bool,
}

impl Acc {
    fn push(&mut self, digit: u8) {
        // low < 10^18, so low * 10 + 9 < 1.9 * 10^19 < 2^64: no wrap occurs.
        let x = self.low.wrapping_mul(10).wrapping_add(u64::from(digit));
        if x >= BIG {
            self.big = true;
            self.low = x.wrapping_rem(BIG);
        } else {
            self.low = x;
        }
    }

    fn value(self) -> u64 {
        // low < 10^18, so the sum < 2 * 10^18 < 2^64.
        if self.big { self.low.wrapping_add(BIG) } else { self.low }
    }
}

fn digits(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(u8::is_ascii_digit)
}

fn split_at_byte(s: &[u8], pred: impl Fn(u8) -> bool) -> (&[u8], Option<&[u8]>) {
    match s.iter().position(|&b| pred(b)) {
        Some(p) => match (s.get(..p), s.get(p.wrapping_add(1)..)) {
            (Some(a), Some(b)) => (a, Some(b)),
            _ => (s, None),
        },
        None => (s, None),
    }
}

impl Operands {
    /// Operands of `-? digit+ ('.' digit+)? ([ce] digit+)?`; `None` if the input
    /// does not match. The exponent moves the decimal point right, keeping
    /// visible trailing zeros: `1.20050c3` is `1200.50` with `e = 3`.
    #[must_use]
    pub fn parse(s: &[u8]) -> Option<Self> {
        let s = match s.split_first() {
            Some((b'-', rest)) => rest,
            _ => s,
        };
        let (mantissa, exponent) = split_at_byte(s, |b| b == b'c' || b == b'e');
        let e = match exponent {
            None => 0u32,
            Some(x) if digits(x) => x.iter().try_fold(0u32, |acc, &b| {
                acc.checked_mul(10)?.checked_add(u32::from(b.wrapping_sub(b'0')))
            })?,
            Some(_) => return None,
        };
        let (int, frac) = split_at_byte(mantissa, |b| b == b'.');
        let frac = match frac {
            None => &[][..],
            Some(x) if digits(x) => x,
            Some(_) => return None,
        };
        if !digits(int) {
            return None;
        }

        // Position of the decimal point in int ++ frac after the shift.
        let point = int.len().saturating_add(usize::try_from(e).ok()?);
        let mut i = Acc::default();
        let mut f = Acc::default();
        let mut t = Acc::default();
        let (mut v, mut w, mut zeros) = (0u32, 0u32, 0u32);
        for (k, &b) in int.iter().chain(frac.iter()).enumerate() {
            let d = b.wrapping_sub(b'0');
            if k < point {
                i.push(d);
            } else {
                f.push(d);
                v = v.saturating_add(1);
                if d == 0 {
                    zeros = zeros.saturating_add(1);
                } else {
                    for _ in 0..zeros {
                        t.push(0);
                    }
                    t.push(d);
                    zeros = 0;
                    w = v;
                }
            }
        }
        // An exponent beyond the digits pads the integer with zeros. After 18
        // pushes of 0 the low part is 0 and further pushes change nothing, so
        // the padding is capped (exactly) at 19.
        let total = int.len().saturating_add(frac.len());
        for _ in 0..point.saturating_sub(total).min(19) {
            i.push(0);
        }
        Some(Self { i: i.value(), f: f.value(), t: t.value(), v, w, e })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::arithmetic_side_effects, clippy::pedantic)]
mod tests {
    use super::{BIG, Operands};

    fn o(i: u64, v: u32, w: u32, f: u64, t: u64, e: u32) -> Option<Operands> {
        Some(Operands { i, f, t, v, w, e })
    }

    /// The operand table of UTS #35 Part 3 §5.1.1 (source → i v w f t e).
    #[test]
    fn uts35_table() {
        let p = |s: &str| Operands::parse(s.as_bytes());
        assert!(p("1") == o(1, 0, 0, 0, 0, 0));
        assert!(p("1.0") == o(1, 1, 0, 0, 0, 0));
        assert!(p("1.00") == o(1, 2, 0, 0, 0, 0));
        assert!(p("1.3") == o(1, 1, 1, 3, 3, 0));
        assert!(p("1.30") == o(1, 2, 1, 30, 3, 0));
        assert!(p("1.03") == o(1, 2, 2, 3, 3, 0));
        assert!(p("1.230") == o(1, 3, 2, 230, 23, 0));
        assert!(p("1200000") == o(1_200_000, 0, 0, 0, 0, 0));
        assert!(p("1.2c6") == o(1_200_000, 0, 0, 0, 0, 6));
        assert!(p("123c6") == o(123_000_000, 0, 0, 0, 0, 6));
        assert!(p("123c5") == o(12_300_000, 0, 0, 0, 0, 5));
        assert!(p("1200.50") == o(1200, 2, 1, 50, 5, 0));
        assert!(p("1.20050c3") == o(1200, 2, 1, 50, 5, 3));
        // `e` is a synonym separator; sign is dropped (n is the absolute value).
        assert!(p("1.1e6") == o(1_100_000, 0, 0, 0, 0, 6));
        assert!(p("-1.5") == o(1, 1, 1, 5, 5, 0));
        assert!(p("1.0000001c6") == o(1_000_000, 1, 1, 1, 1, 6));
        assert!(p("0.00") == o(0, 2, 0, 0, 0, 0));
    }

    #[test]
    fn rejects_malformed() {
        for s in ["", "-", ".", "1.", ".5", "1c", "1.5c", "1x", "1..2", "1c2.5", "c3", "1c99999999999"] {
            assert!(Operands::parse(s.as_bytes()).is_none(), "{s}");
        }
    }

    #[test]
    fn big_values_keep_low_digits() {
        let x = Operands::parse(b"12345678901234567890123").unwrap();
        assert!(x.i == BIG + 678_901_234_567_890_123);
        assert!(x.i % 1_000_000 == 890_123);
        let y = Operands::parse(b"1c40").unwrap();
        assert!(y.i == BIG && y.e == 40 && y.i.is_multiple_of(1_000_000));
        let z = Operands::parse(b"0c40").unwrap();
        assert!(z.i == 0);
    }
}
