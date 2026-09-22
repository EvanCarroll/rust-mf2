//! The digit backend of the numeric semantics — the seam of owner decision 1
//! (`plans/10-phase-3-work-order.md`, A5 and A5b).
//!
//! The default is this crate's own buffer ([`own`]): an exact decimal of up
//! to [`INPUT_DIGITS`] significant digits, stored inline — `Copy`, no
//! allocation, no panic path. Feature `fixed-decimal` swaps in `fixed_decimal`
//! behind the same interface; it exists only as the A/B baseline.
//!
//! Magnitudes are powers of ten: the digit at magnitude `m` is worth `d ×
//! 10^m`. A value is canonical: no leading or trailing zero digit is stored,
//! so `low`/`high` are the least and most significant *nonzero* magnitudes.

#[cfg(not(feature = "fixed-decimal"))]
mod own;
#[cfg(not(feature = "fixed-decimal"))]
pub(crate) use own::Decimal;

#[cfg(feature = "fixed-decimal")]
mod fixed;
#[cfg(feature = "fixed-decimal")]
pub(crate) use fixed::Decimal;

/// The most significant digits an operand may have (an implementation limit,
/// `number.md`: past it, Unsupported Operation). i64 needs 19, f64 17.
pub(crate) const INPUT_DIGITS: usize = 40;

/// The largest exponent a `number-literal` may carry (an implementation
/// limit; it bounds the plain output of a value to about 10,000 digits).
pub(crate) const MAX_EXPONENT: i32 = 9999;

/// The largest magnitude a value may reach (exponent + digits + slack).
pub(crate) const MAX_MAGNITUDE: i32 = MAX_EXPONENT + 2 * 64;

/// Why a `number-literal` did not become a [`Decimal`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ParseError {
    /// Not `number-literal`: Bad Operand.
    Syntax,
    /// Valid, but past [`INPUT_DIGITS`] or [`MAX_EXPONENT`]: Unsupported
    /// Operation.
    Limit,
}

/// `roundingMode` (ECMA-402 names).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum RoundingMode {
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

/// The mantissa of a rounding increment: every `roundingIncrement` value is
/// `1`, `2`, `5` or `25` times a power of ten.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Increment {
    One,
    Two,
    Five,
    TwentyFive,
}

impl Increment {
    #[cfg_attr(feature = "fixed-decimal", allow(dead_code))]
    pub(crate) const fn value(self) -> u8 {
        match self {
            Increment::One => 1,
            Increment::Two => 2,
            Increment::Five => 5,
            Increment::TwentyFive => 25,
        }
    }
}

/// A `number-literal`, split: `["-"] int ["." frac] [e exp]`.
pub(crate) struct Literal<'s> {
    pub(crate) neg: bool,
    /// The integer digits (ASCII), never empty.
    pub(crate) int: &'s [u8],
    /// The fraction digits (ASCII), possibly empty.
    pub(crate) frac: &'s [u8],
    pub(crate) exp: i32,
}

/// Checks `number-literal` (`number.md`, "Numeric Operands") and splits it.
pub(crate) fn split_literal(b: &[u8]) -> Result<Literal<'_>, ParseError> {
    let neg = b.first() == Some(&b'-');
    let rest = if neg { b.get(1..).unwrap_or(&[]) } else { b };
    let mut i = 0;
    match rest.first() {
        Some(b'0') => i = 1,
        Some(b'1'..=b'9') => {
            while rest.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        _ => return Err(ParseError::Syntax),
    }
    let int = rest.get(..i).unwrap_or(&[]);
    let mut frac: &[u8] = &[];
    if rest.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while rest.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return Err(ParseError::Syntax);
        }
        frac = rest.get(start..i).unwrap_or(&[]);
    }
    let mut exp: i32 = 0;
    let mut limit = false;
    if matches!(rest.get(i), Some(b'e' | b'E')) {
        i += 1;
        let neg_exp = match rest.get(i) {
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
        while let Some(&d) = rest.get(i).filter(|d| d.is_ascii_digit()) {
            if !limit {
                exp = exp * 10 + i32::from(d - b'0');
                limit = exp > MAX_EXPONENT;
            }
            i += 1;
        }
        if i == start {
            return Err(ParseError::Syntax);
        }
        if neg_exp {
            exp = -exp;
        }
    }
    if i != rest.len() {
        return Err(ParseError::Syntax);
    }
    if limit {
        return Err(ParseError::Limit);
    }
    Ok(Literal {
        neg,
        int,
        frac,
        exp,
    })
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::{Decimal, Increment, ParseError, RoundingMode};
    use alloc::string::String;

    fn plain(d: &Decimal) -> String {
        let mut s = String::new();
        d.write_plain(&mut s);
        s
    }

    fn p(s: &str) -> Decimal {
        match Decimal::parse(s.as_bytes()) {
            Ok(d) => d,
            Err(e) => panic!("{s}: {e:?}"),
        }
    }

    #[test]
    fn parse_and_print() {
        for (src, want) in [
            ("0", "0"),
            ("-0", "-0"),
            ("4.2", "4.2"),
            ("-4.20", "-4.2"),
            ("0.42e+1", "4.2"),
            ("1E3", "1000"),
            ("1e-2", "0.01"),
            ("10", "10"),
            ("0.000", "0"),
            ("120.0500", "120.05"),
            ("1e21", "1000000000000000000000"),
            ("1.5e-7", "0.00000015"),
        ] {
            assert_eq!(plain(&p(src)), want, "{src}");
        }
        for bad in [
            "00", "042", "1.", "1e", "1E", "1.e", "1.2e", "1.e3", "1e+", "1e-", "1.0e2.0", "foo",
            ".1", "+1", "0x1", "", "-", "1 ", " 1",
        ] {
            assert_eq!(
                Decimal::parse(bad.as_bytes()).err(),
                Some(ParseError::Syntax),
                "{bad}"
            );
        }
        assert_eq!(Decimal::parse(b"1e10000").err(), Some(ParseError::Limit));
        assert_eq!(plain(&p("1e0000000009")), "1000000000");
        let long = "1234567890123456789012345678901234567890";
        assert_eq!(plain(&p(long)), long);
        let longer = "12345678901234567890123456789012345678901";
        assert_eq!(
            Decimal::parse(longer.as_bytes()).err(),
            Some(ParseError::Limit)
        );
        // Leading and trailing zeros do not count.
        assert_eq!(
            plain(&p(
                "0.000000000000000000001234567890123456789012345678901234567890000"
            )),
            "0.00000000000000000000123456789012345678901234567890123456789"
        );
    }

    fn rounded(s: &str, pos: i16, mode: RoundingMode, inc: Increment) -> String {
        let mut d = p(s);
        d.round(pos, mode, inc);
        plain(&d)
    }

    #[test]
    fn rounding_modes() {
        use RoundingMode::*;
        let one = Increment::One;
        // ECMA-402's table for roundingMode, at 0 fraction digits.
        let table: &[(&str, [&str; 9])] = &[
            // value   ceil  floor expand trunc hCeil hFloor hExpand hTrunc hEven
            (
                "-1.5",
                ["-1", "-2", "-2", "-1", "-1", "-2", "-2", "-1", "-2"],
            ),
            (
                "-0.5",
                ["-0", "-1", "-1", "-0", "-0", "-1", "-1", "-0", "-0"],
            ),
            ("0.4", ["1", "0", "1", "0", "0", "0", "0", "0", "0"]),
            ("0.5", ["1", "0", "1", "0", "1", "0", "1", "0", "0"]),
            ("0.6", ["1", "0", "1", "0", "1", "1", "1", "1", "1"]),
            ("1.5", ["2", "1", "2", "1", "2", "1", "2", "1", "2"]),
            ("2.5", ["3", "2", "3", "2", "3", "2", "3", "2", "2"]),
        ];
        let modes = [
            Ceil, Floor, Expand, Trunc, HalfCeil, HalfFloor, HalfExpand, HalfTrunc, HalfEven,
        ];
        for (v, wants) in table {
            for (m, want) in modes.iter().zip(wants) {
                assert_eq!(rounded(v, 0, *m, one), *want, "{v} {m:?}");
            }
        }
        assert_eq!(rounded("9.995", -2, HalfExpand, one), "10");
        assert_eq!(rounded("999.5", 0, HalfExpand, one), "1000");
        assert_eq!(rounded("0.004", 0, Ceil, one), "1");
        assert_eq!(rounded("0.004", 0, HalfExpand, one), "0");
        assert_eq!(rounded("1234", 2, HalfExpand, one), "1200");
        assert_eq!(rounded("1.25000000000000000001", -1, HalfEven, one), "1.3");
        assert_eq!(rounded("1.25", -1, HalfEven, one), "1.2");
        assert_eq!(rounded("1.35", -1, HalfEven, one), "1.4");
    }

    #[test]
    fn increments() {
        use RoundingMode::*;
        // Multiples of 0.05, 0.25, 2, …
        assert_eq!(rounded("1.23", -2, HalfExpand, Increment::Five), "1.25");
        assert_eq!(rounded("1.22", -2, HalfExpand, Increment::Five), "1.2");
        assert_eq!(rounded("1.225", -2, HalfExpand, Increment::Five), "1.25");
        assert_eq!(rounded("1.225", -2, HalfEven, Increment::Five), "1.2");
        assert_eq!(rounded("1.275", -2, HalfEven, Increment::Five), "1.3");
        assert_eq!(rounded("1.3", -1, HalfExpand, Increment::TwentyFive), "2.5");
        assert_eq!(rounded("1.1", -1, HalfExpand, Increment::TwentyFive), "0");
        assert_eq!(rounded("1.125", -2, HalfEven, Increment::TwentyFive), "1");
        assert_eq!(rounded("1.375", -2, HalfEven, Increment::TwentyFive), "1.5");
        assert_eq!(rounded("3", 0, HalfEven, Increment::Two), "4");
        assert_eq!(rounded("5", 0, HalfEven, Increment::Two), "4");
        assert_eq!(rounded("7", 0, HalfEven, Increment::Two), "8");
        assert_eq!(rounded("7", 0, Trunc, Increment::Two), "6");
        assert_eq!(rounded("-7", 0, Floor, Increment::Two), "-8");
        assert_eq!(rounded("-7", 0, Ceil, Increment::Two), "-6");
        assert_eq!(rounded("99.9", 1, HalfExpand, Increment::Five), "100");
        assert_eq!(rounded("0.1", 2, Ceil, Increment::TwentyFive), "2500");
        assert_eq!(rounded("12", -3, HalfExpand, Increment::Five), "12");
    }

    #[test]
    fn addition() {
        let add = |a: &str, b: &str| {
            let d = p(a).add(&p(b)).map(|d| plain(&d));
            d.unwrap_or_default()
        };
        assert_eq!(add("41", "1"), "42");
        assert_eq!(add("52", "-10"), "42");
        assert_eq!(add("1", "-3"), "-2");
        assert_eq!(add("-1.5", "2"), "0.5");
        assert_eq!(add("0.25", "-1"), "-0.75");
        assert_eq!(add("999", "99"), "1098");
        assert_eq!(add("-0", "0"), "0");
        assert_eq!(add("1", "-1"), "0");
        // The own buffer adds exactly up to 40 digits; `fixed_decimal`'s
        // baseline (P0.5's `add_small`) only through `i64`.
        #[cfg(not(feature = "fixed-decimal"))]
        assert_eq!(add("1e30", "1"), "1000000000000000000000000000001");
        assert!(p("1e45").add(&p("1")).is_none());
    }
}
