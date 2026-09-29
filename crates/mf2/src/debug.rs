//! `Debug` for the call-site types, written through `write_str` alone: no
//! `core::fmt` number, float or string-escape code, and no builder. What a
//! browser build pays for a `{:?}` on a description is then the formatter's
//! driver and these few functions — about 1 KB gz, where the derived form
//! brings core's float formatter and its panic paths, 12–16 KB gz.
//!
//! What that gives up, against a derived `Debug`:
//! * a float shows at most six fraction digits (`1e-7` prints `0.0`);
//! * a quote or a newline inside a text is not escaped;
//! * a date prints as `DateTimeValue(2026-09-28T14:05:09.007)`;
//! * `{:#?}` prints what `{:?}` prints.
//!
//! Integers, `2.5`, `0.1`, `123456.789`, `Unset` and `Custom(..)` print as a
//! derived `Debug` would. A description shows its `MsgId` number, not the
//! message's id, which is not in the build.

use core::fmt::{Formatter, Result, Write};

use mf2_runtime::MsgId;

/// `n` in decimal.
pub(crate) fn uint(f: &mut Formatter<'_>, n: u64) -> Result {
    if n >= 10 {
        uint(f, n / 10)?;
    }
    f.write_char(digit(n % 10))
}

/// `n` in decimal, with its sign.
pub(crate) fn int(f: &mut Formatter<'_>, n: i64) -> Result {
    if n < 0 {
        f.write_char('-')?;
    }
    uint(f, n.unsigned_abs())
}

/// `n` in decimal on exactly `width` digits, zero-padded.
pub(crate) fn fixed(f: &mut Formatter<'_>, n: u64, width: u32) -> Result {
    if width > 1 {
        fixed(f, n / 10, width - 1)?;
    }
    f.write_char(digit(n % 10))
}

/// `x`, to six fraction digits at most (trailing zeros dropped).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "a Debug approximation: the float is checked finite and below 1e18 first"
)]
pub(crate) fn float(f: &mut Formatter<'_>, x: f64) -> Result {
    if x.is_nan() {
        return f.write_str("NaN");
    }
    if x.is_sign_negative() {
        f.write_char('-')?;
    }
    let a = x.abs();
    if a.is_infinite() {
        return f.write_str("inf");
    }
    if a >= 1e18 {
        return f.write_str("1e18..");
    }
    let mut whole = a as u64;
    let mut frac = ((a - whole as f64) * 1_000_000.0 + 0.5) as u64;
    if frac >= 1_000_000 {
        whole += 1;
        frac = 0;
    }
    uint(f, whole)?;
    f.write_char('.')?;
    let mut width = 6;
    while width > 1 && frac.is_multiple_of(10) {
        frac /= 10;
        width -= 1;
    }
    fixed(f, frac, width)
}

/// `s` between double quotes, as it is.
pub(crate) fn quoted(f: &mut Formatter<'_>, s: &str) -> Result {
    f.write_char('"')?;
    f.write_str(s)?;
    f.write_char('"')
}

/// A message id, as `MsgId(n)`.
pub(crate) fn msg_id(f: &mut Formatter<'_>, id: MsgId) -> Result {
    f.write_str("MsgId(")?;
    uint(f, u64::from(id.raw()))?;
    f.write_char(')')
}

fn digit(d: u64) -> char {
    char::from_digit(u32::try_from(d).unwrap_or(0), 10).unwrap_or('0')
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use core::fmt;

    use super::{float, int};

    struct Float(f64);
    impl fmt::Debug for Float {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            float(f, self.0)
        }
    }

    struct Int(i64);
    impl fmt::Debug for Int {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            int(f, self.0)
        }
    }

    fn shown(value: &dyn fmt::Debug) -> String {
        let mut out = String::new();
        fmt::write(&mut out, format_args!("{value:?}")).unwrap_or_default();
        out
    }

    #[test]
    fn numbers_print_as_a_derived_debug_would_within_the_stated_limits() {
        for (x, text) in [
            (2.5, "2.5"),
            (0.1, "0.1"),
            (123_456.789, "123456.789"),
            (-3.0, "-3.0"),
            (0.0, "0.0"),
            (1e-7, "0.0"),
            (0.999_999_9, "1.0"),
            (f64::NAN, "NaN"),
            (f64::NEG_INFINITY, "-inf"),
        ] {
            assert_eq!(shown(&Float(x)), text, "{x}");
        }
        for (n, text) in [
            (0, "0"),
            (42, "42"),
            (-7, "-7"),
            (i64::MIN, "-9223372036854775808"),
        ] {
            assert_eq!(shown(&Int(n)), text, "{n}");
        }
    }
}
