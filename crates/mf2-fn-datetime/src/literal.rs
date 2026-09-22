//! The text grammars of the date/time functions: *date/time literal values*
//! (datetime.md, "Date and Time Operands"), the RFC 3339 `time-numoffset`
//! of a `timeZone` value, and the `uvalue` of a `calendar` value. Byte
//! scans only (no `str` pattern machinery, 05 §8).

use mf2_runtime::{Date, DateTime, Time};

/// Two ASCII digits as 0–99.
const fn two(a: u8, b: u8) -> Option<u8> {
    if a.is_ascii_digit() && b.is_ascii_digit() {
        Some((a - b'0') * 10 + (b - b'0'))
    } else {
        None
    }
}

/// Parses a *date/time literal value*: the whole of `s` matches the spec's
/// regular expression
///
/// ```text
/// (?!0000)[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])
///   (T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9](\.[0-9]{1,3})?
///    (Z|[+-]((0[0-9]|1[0-3]):[0-5][0-9]|14:00))?)?
/// ```
///
/// and names a day that exists (`2023-02-29` does not). A date without a
/// time is at 00:00:00; a value without an offset is floating. Nothing
/// else is accepted — no other ISO 8601 form (the spec's MAY), no zone name
/// — and `-00:00` is offset 0. The result carries no options.
pub fn parse_literal<'a>(s: &str) -> Option<DateTime<'a>> {
    let (date, rest) = s.as_bytes().split_at_checked(10)?;
    let &[y1, y2, y3, y4, b'-', m1, m2, b'-', d1, d2] = date else {
        return None;
    };
    let year = u16::from(two(y1, y2)?) * 100 + u16::from(two(y3, y4)?);
    if year == 0 {
        return None;
    }
    // `Date::new` checks the month and that the day exists in it.
    let date = Date::new(i32::from(year), two(m1, m2)?, two(d1, d2)?)?;
    let [b'T', h1, h2, b':', n1, n2, b':', s1, s2, tail @ ..] = rest else {
        return rest
            .is_empty()
            .then(|| DateTime::floating(date, Time::MIDNIGHT));
    };
    // One to three fraction digits, read as milliseconds.
    let (millisecond, tail) = match tail {
        [b'.', rest @ ..] => {
            let n = rest
                .iter()
                .take(3)
                .take_while(|b| b.is_ascii_digit())
                .count();
            if n == 0 {
                return None;
            }
            let mut ms = 0u16;
            for i in 0..3 {
                let digit = rest.get(i).filter(|_| i < n).map_or(0, |b| b - b'0');
                ms = ms * 10 + u16::from(digit);
            }
            (ms, rest.get(n..)?)
        }
        tail => (0, tail),
    };
    let time = Time::new(two(*h1, *h2)?, two(*n1, *n2)?, two(*s1, *s2)?, millisecond)?;
    let value = DateTime::floating(date, time);
    match tail {
        [] => Some(value),
        [b'Z'] => value.with_offset(0),
        _ => {
            let offset = offset(tail)?;
            // The literal's offsets: 00:00–13:59, or 14:00 exactly.
            if offset.unsigned_abs() > 14 * 3600 {
                return None;
            }
            value.with_offset(offset)
        }
    }
}

/// RFC 3339 `time-numoffset` — `("+" / "-") time-hour ":" time-minute`,
/// hour 00–23, minute 00–59 — as seconds east of UTC.
pub(crate) fn offset(b: &[u8]) -> Option<i32> {
    let &[sign, h1, h2, b':', m1, m2] = b else {
        return None;
    };
    let (h, m) = (two(h1, h2)?, two(m1, m2)?);
    if h > 23 || m > 59 {
        return None;
    }
    let seconds = i32::from(h) * 3600 + i32::from(m) * 60;
    match sign {
        b'+' => Some(seconds),
        b'-' => Some(-seconds),
        _ => None,
    }
}

/// A well-formed Unicode calendar identifier: a `uvalue`, `3*8alphanum
/// *("-" 3*8alphanum)` — the BCP 47 / ECMA-402 spelling (UTS 35's `sep`
/// also admits `_`, which `Intl.DateTimeFormat` rejects, so neither
/// backend could take it).
pub(crate) fn is_uvalue(s: &str) -> bool {
    let mut run = 0u8;
    for &c in s.as_bytes() {
        if c == b'-' {
            if run < 3 {
                return false;
            }
            run = 0;
        } else if c.is_ascii_alphanumeric() && run < 8 {
            run += 1;
        } else {
            return false;
        }
    }
    run >= 3
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Year, month, day, hour, minute, second, millisecond, offset.
    type Fields = (i32, u8, u8, u8, u8, u8, u16, Option<i32>);

    fn fields(s: &str) -> Option<Fields> {
        parse_literal(s).map(|d| {
            (
                d.date.year(),
                d.date.month(),
                d.date.day(),
                d.time.hour(),
                d.time.minute(),
                d.time.second(),
                d.time.millisecond(),
                d.offset,
            )
        })
    }

    #[test]
    fn literals() {
        assert_eq!(fields("2006-01-02"), Some((2006, 1, 2, 0, 0, 0, 0, None)));
        assert_eq!(
            fields("2006-01-02T15:04:06"),
            Some((2006, 1, 2, 15, 4, 6, 0, None))
        );
        assert_eq!(
            fields("2006-01-02T15:04:06Z"),
            Some((2006, 1, 2, 15, 4, 6, 0, Some(0)))
        );
        assert_eq!(fields("2006-01-02T15:04:06.1").unwrap().6, 100);
        assert_eq!(fields("2006-01-02T15:04:06.12").unwrap().6, 120);
        assert_eq!(fields("2006-01-02T15:04:06.123").unwrap().6, 123);
        assert_eq!(fields("2006-01-02T15:04:06.007Z").unwrap().6, 7);
        assert_eq!(
            fields("2006-01-02T15:04:06+14:00").unwrap().7,
            Some(14 * 3600)
        );
        assert_eq!(
            fields("2006-01-02T15:04:06-13:59").unwrap().7,
            Some(-(13 * 3600 + 59 * 60))
        );
        assert_eq!(fields("2006-01-02T15:04:06-00:00").unwrap().7, Some(0));
        assert_eq!(fields("0001-01-01"), Some((1, 1, 1, 0, 0, 0, 0, None)));
        assert_eq!(
            fields("9999-12-31T23:59:59.999"),
            Some((9999, 12, 31, 23, 59, 59, 999, None))
        );
        assert_eq!(fields("2024-02-29").unwrap().2, 29);
        assert_eq!(fields("2000-02-29").unwrap().2, 29);
        let d = parse_literal("2006-01-02").unwrap();
        assert!(!d.options.is_resolved());
        assert_eq!(d.zone, None);
    }

    #[test]
    fn not_literals() {
        for s in [
            "",
            "horse",
            "0000-01-01",
            "2006-00-01",
            "2006-13-01",
            "2006-01-00",
            "2006-01-32",
            "2023-02-29",
            "1900-02-29",
            "2006-04-31",
            "2006-1-02",
            "06-01-02",
            "+2006-01-02",
            "-2006-01-02",
            "20060102",
            "2006-01-02T",
            "2006-01-02 15:04:06",
            "2006-01-02t15:04:06",
            "2006-01-02T15:04",
            "2006-01-02T24:00:00",
            "2006-01-02T15:60:00",
            "2006-01-02T15:04:60",
            "2006-01-02T15:04:06.",
            "2006-01-02T15:04:06.1234",
            "2006-01-02T15:04:06,1",
            "2006-01-02Z",
            "2006-01-02+01:00",
            "2006-01-02T15:04:06z",
            "2006-01-02T15:04:06+14:01",
            "2006-01-02T15:04:06+15:00",
            "2006-01-02T15:04:06+01",
            "2006-01-02T15:04:06+0100",
            "2006-01-02T15:04:06+01:60",
            "2006-01-02T15:04:06Z ",
            " 2006-01-02",
            "2006-01-02T15:04:06[Europe/Paris]",
            "2006-01-02T15:04:06+01:00[Europe/Paris]",
            "２００６-01-02",
            "2006-01-02T15:04:06.1Z0",
        ] {
            assert!(parse_literal(s).is_none(), "{s:?}");
        }
    }

    #[test]
    fn offsets() {
        assert_eq!(offset(b"+00:00"), Some(0));
        assert_eq!(offset(b"-00:00"), Some(0));
        assert_eq!(offset(b"+05:30"), Some(19_800));
        assert_eq!(offset(b"-23:59"), Some(-86_340));
        for bad in [
            &b"+24:00"[..],
            b"+01:60",
            b"01:00",
            b"+1:00",
            b"+01:0",
            b"+0100",
            b"Z",
            b"",
            b"\xe2\x88\x9201:00",
        ] {
            assert_eq!(offset(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn uvalues() {
        for good in [
            "gregory",
            "iso8601",
            "islamic-civil",
            "abc",
            "abcdefgh",
            "Japanese",
            "a1b-c2d",
        ] {
            assert!(is_uvalue(good), "{good}");
        }
        for bad in [
            "",
            "ab",
            "abcdefghi",
            "islamic_civil",
            "-abc",
            "abc-",
            "abc--def",
            "ab-cde",
            "é_abc",
            "abc def",
        ] {
            assert!(!is_uvalue(bad), "{bad}");
        }
    }
}
