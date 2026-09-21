//! P0.6 probe — the backend-independent half of `:datetime` / `:date` / `:time`
//! (spec: `third_party/message-format-wg/spec/functions/datetime.md`, Draft).
//!
//! Operand parsing, option resolution (literal-only rules, override options and
//! their inheritance), and time-zone resolution to wall-clock fields. A backend
//! (ICU4X or `Intl`) only turns a [`Plan`] + [`Zoned`] value into text.
//!
//! `no_std`, fmt-free, panic-free: no `format!`, no `Debug`/`Display`, no
//! `unwrap`, no panicking indexing.
#![no_std]
#![forbid(unsafe_code)]

/// Which function is being resolved.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Func {
    DateTime,
    Date,
    Time,
}

/// MF2 `dateFields` / `fields`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DateFields {
    Weekday,
    DayWeekday,
    MonthDay,
    MonthDayWeekday,
    YearMonthDay,
    YearMonthDayWeekday,
}

/// MF2 `dateLength` / `length`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Length {
    Long,
    Medium,
    Short,
}

/// MF2 `timePrecision` / `precision`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Hour,
    Minute,
    Second,
}

/// MF2 `timeZoneStyle`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ZoneStyle {
    Long,
    Short,
}

/// MF2 `timeZone` override option.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimeZoneOpt<'a> {
    /// `input`: the operand's own offset.
    Input,
    /// `UTC`.
    Utc,
    /// An RFC 3339 `time-numoffset`, in minutes east of UTC.
    Offset(i32),
    /// An RFC 9557 `time-zone-name` (IANA id); needs a time-zone database.
    Named(&'a str),
}

/// The date/time *override options* — the only options an operand passes on.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Overrides<'a> {
    pub time_zone: Option<TimeZoneOpt<'a>>,
    pub hour12: Option<bool>,
    pub calendar: Option<&'a str>,
}

/// Fully resolved formatting plan; what a backend consumes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Plan<'a> {
    pub func: Func,
    pub date: Option<DateFields>,
    pub length: Length,
    pub time: Option<Precision>,
    pub zone: Option<ZoneStyle>,
    pub ov: Overrides<'a>,
}

/// An option value as the runtime sees it: set by a literal, or by a variable
/// (whose resolved value is a string here).
#[derive(Clone, Copy)]
pub enum OptValue<'a> {
    Literal(&'a str),
    Variable(&'a str),
}

impl<'a> OptValue<'a> {
    fn text(self) -> &'a str {
        match self {
            OptValue::Literal(s) | OptValue::Variable(s) => s,
        }
    }
}

/// Errors, as small enum values (no strings on the client path).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Error {
    BadOperand,
    BadOption,
    UnsupportedOperation,
}

/// Where errors go; formatting never fails, errors are collected.
pub trait ErrSink {
    fn push(&mut self, e: Error);
}

const DT: u8 = 1;
const D: u8 = 2;
const T: u8 = 4;

/// (option name, functions it applies to, kind). Table-driven to keep code small.
const OPTION_NAMES: [(&[u8], u8, u8); 10] = [
    (b"dateFields", DT, 0),
    (b"fields", D, 0),
    (b"dateLength", DT, 1),
    (b"length", D, 1),
    (b"timePrecision", DT, 2),
    (b"precision", T, 2),
    (b"timeZoneStyle", DT | T, 3),
    (b"timeZone", DT | D | T, 4),
    (b"hour12", DT | T, 5),
    (b"calendar", DT | D | T, 6),
];
const DATE_FIELDS: [(&[u8], DateFields); 6] = [
    (b"weekday", DateFields::Weekday),
    (b"day-weekday", DateFields::DayWeekday),
    (b"month-day", DateFields::MonthDay),
    (b"month-day-weekday", DateFields::MonthDayWeekday),
    (b"year-month-day", DateFields::YearMonthDay),
    (b"year-month-day-weekday", DateFields::YearMonthDayWeekday),
];
const LENGTHS: [(&[u8], Length); 3] = [
    (b"long", Length::Long),
    (b"medium", Length::Medium),
    (b"short", Length::Short),
];
const PRECISIONS: [(&[u8], Precision); 3] = [
    (b"hour", Precision::Hour),
    (b"minute", Precision::Minute),
    (b"second", Precision::Second),
];
const ZONE_STYLES: [(&[u8], ZoneStyle); 2] = [(b"long", ZoneStyle::Long), (b"short", ZoneStyle::Short)];
const BOOLS: [(&[u8], bool); 2] = [(b"true", true), (b"false", false)];

fn find<T: Copy>(table: &[(&[u8], T)], v: &[u8]) -> Option<T> {
    table.iter().find(|(k, _)| *k == v).map(|&(_, t)| t)
}

/// Resolve the options of `func`. `inherited` are the override options carried
/// by the operand when it is itself a resolved date/time value.
pub fn resolve<'a>(
    func: Func,
    opts: &[(&'a str, OptValue<'a>)],
    inherited: Overrides<'a>,
    errs: &mut impl ErrSink,
) -> Plan<'a> {
    let fbit = match func {
        Func::DateTime => DT,
        Func::Date => D,
        Func::Time => T,
    };
    let mut plan = Plan {
        func,
        date: if func == Func::Time { None } else { Some(DateFields::YearMonthDay) },
        length: Length::Medium,
        time: if func == Func::Date { None } else { Some(Precision::Minute) },
        zone: None,
        ov: inherited,
    };
    // Expression options win over inherited ones.
    let mut ov = Overrides::default();
    for &(name, value) in opts {
        let Some(kind) = OPTION_NAMES
            .iter()
            .find(|(n, mask, _)| *n == name.as_bytes() && mask & fbit != 0)
            .map(|&(_, _, k)| k)
        else {
            // Unknown options are ignored (spec: formatting.md).
            continue;
        };
        let text = value.text();
        let v = text.as_bytes();
        // Kinds 0–3 MUST be set by a literal; 4–6 are override options.
        if kind < 4 && matches!(value, OptValue::Variable(_)) {
            errs.push(Error::BadOption);
            continue;
        }
        let ok = match kind {
            0 => find(&DATE_FIELDS, v).map(|x| plan.date = Some(x)),
            1 => find(&LENGTHS, v).map(|x| plan.length = x),
            2 => find(&PRECISIONS, v).map(|x| plan.time = Some(x)),
            3 => find(&ZONE_STYLES, v).map(|x| plan.zone = Some(x)),
            4 => parse_time_zone(text).map(|x| ov.time_zone = Some(x)),
            5 => find(&BOOLS, v).map(|x| ov.hour12 = Some(x)),
            _ => is_uvalue(v).then(|| ov.calendar = Some(text)),
        };
        if ok.is_none() {
            errs.push(Error::BadOption);
        }
    }
    if ov.time_zone.is_some() {
        plan.ov.time_zone = ov.time_zone;
    }
    if ov.hour12.is_some() {
        plan.ov.hour12 = ov.hour12;
    }
    if ov.calendar.is_some() {
        plan.ov.calendar = ov.calendar;
    }
    plan
}

/// `input` | `UTC` | RFC 3339 `time-numoffset` | RFC 9557 `time-zone-name`.
pub fn parse_time_zone(v: &str) -> Option<TimeZoneOpt<'_>> {
    let b = v.as_bytes();
    if b == b"input" {
        return Some(TimeZoneOpt::Input);
    }
    if b == b"UTC" {
        return Some(TimeZoneOpt::Utc);
    }
    if let Some(off) = parse_numoffset(b) {
        return Some(TimeZoneOpt::Offset(off));
    }
    // time-zone-name = time-zone-part *("/" time-zone-part)
    // time-zone-part = (ALPHA / "." / "_") *13(ALPHA / "." / "_" / DIGIT / "-" / "+")
    //                  but not "." or ".."
    if b.is_empty() {
        return None;
    }
    for part in b.split(|&c| c == b'/') {
        let (first, rest) = part.split_first()?;
        if !(first.is_ascii_alphabetic() || *first == b'.' || *first == b'_') {
            return None;
        }
        if rest.len() > 13 || part == b"." || part == b".." {
            return None;
        }
        if !rest
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'+'))
        {
            return None;
        }
    }
    Some(TimeZoneOpt::Named(v))
}

/// `("+" / "-") time-hour ":" time-minute` → minutes east of UTC.
fn parse_numoffset(b: &[u8]) -> Option<i32> {
    let [sign, h1, h2, b':', m1, m2] = *b else {
        return None;
    };
    let neg = match sign {
        b'+' => false,
        b'-' => true,
        _ => return None,
    };
    let h = two(h1, h2)?;
    let m = two(m1, m2)?;
    if h > 23 || m > 59 {
        return None;
    }
    let mins = i32::from(h) * 60 + i32::from(m);
    Some(if neg { -mins } else { mins })
}

fn two(a: u8, b: u8) -> Option<u8> {
    if a.is_ascii_digit() && b.is_ascii_digit() {
        Some((a - b'0') * 10 + (b - b'0'))
    } else {
        None
    }
}

/// `uvalue = 3*8alphanum *("-" 3*8alphanum)`.
fn is_uvalue(b: &[u8]) -> bool {
    !b.is_empty()
        && b.split(|&c| c == b'-')
            .all(|p| (3..=8).contains(&p.len()) && p.iter().all(u8::is_ascii_alphanumeric))
}

/// Civil (proleptic Gregorian) wall-clock fields.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millis: u16,
}

/// A resolved date/time operand: wall-clock fields plus the offset, if any
/// (`None` = floating time).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DtValue {
    pub civil: Civil,
    pub offset: Option<i32>,
}

/// Parse a *date/time literal value* (the spec's regular expression, whole
/// string), rejecting impossible days of month. Time defaults to 00:00:00.
pub fn parse_operand(s: &str) -> Option<DtValue> {
    let b = s.as_bytes();
    let (date, rest) = b.split_at_checked(10)?;
    let [y1, y2, y3, y4, b'-', mo1, mo2, b'-', d1, d2] = *date else {
        return None;
    };
    let year = i32::from(two(y1, y2)?) * 100 + i32::from(two(y3, y4)?);
    let month = two(mo1, mo2)?;
    let day = two(d1, d2)?;
    if year == 0 || !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let mut civil = Civil {
        year,
        month,
        day,
        hour: 0,
        minute: 0,
        second: 0,
        millis: 0,
    };
    if rest.is_empty() {
        return Some(DtValue {
            civil,
            offset: None,
        });
    }
    let (time, mut rest) = rest.split_at_checked(9)?;
    let [b'T', h1, h2, b':', mi1, mi2, b':', s1, s2] = *time else {
        return None;
    };
    civil.hour = two(h1, h2)?;
    civil.minute = two(mi1, mi2)?;
    civil.second = two(s1, s2)?;
    if civil.hour > 23 || civil.minute > 59 || civil.second > 59 {
        return None;
    }
    if let Some((b'.', frac)) = rest.split_first() {
        let n = frac.iter().take_while(|c| c.is_ascii_digit()).count();
        if !(1..=3).contains(&n) {
            return None;
        }
        let (digits, tail) = frac.split_at_checked(n)?;
        let mut ms: u16 = 0;
        for i in 0..3 {
            ms = ms * 10 + digits.get(i).map_or(0, |d| u16::from(d - b'0'));
        }
        civil.millis = ms;
        rest = tail;
    }
    let offset = match rest {
        [] => None,
        [b'Z'] => Some(0),
        _ => {
            let off = parse_numoffset(rest)?;
            // Spec regex: hours 00–13, or exactly 14:00.
            if off.abs() > 14 * 60 {
                return None;
            }
            Some(off)
        }
    };
    Some(DtValue { civil, offset })
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u8) -> u8 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 (H. Hinnant's `days_from_civil`).
pub fn days_from_civil(y: i32, m: u8, d: u8) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
pub fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    ((y + i64::from(m <= 2)) as i32, m as u8, d as u8)
}

impl Civil {
    /// Milliseconds since the epoch, reading these fields as UTC.
    pub fn epoch_ms_as_utc(&self) -> i64 {
        let days = days_from_civil(self.year, self.month, self.day);
        let secs = days * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second);
        secs * 1000 + i64::from(self.millis)
    }

    /// Inverse of [`Civil::epoch_ms_as_utc`].
    pub fn from_epoch_ms(ms: i64) -> Civil {
        let days = ms.div_euclid(86_400_000);
        let rem = ms.rem_euclid(86_400_000);
        let (year, month, day) = civil_from_days(days);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Civil {
            year,
            month,
            day,
            hour: (rem / 3_600_000) as u8,
            minute: (rem / 60_000 % 60) as u8,
            second: (rem / 1000 % 60) as u8,
            millis: (rem % 1000) as u16,
        }
    }

    /// ISO weekday, Monday = 1 … Sunday = 7.
    pub fn iso_weekday(&self) -> u8 {
        let d = days_from_civil(self.year, self.month, self.day);
        // 1970-01-01 was a Thursday (4).
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let w = ((d + 3).rem_euclid(7) + 1) as u8;
        w
    }
}

/// The value to display after applying the `timeZone` rules.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Zoned<'a> {
    /// Wall-clock fields to show, plus the zone to name: a fixed offset in
    /// minutes (`UTC` = 0).
    Fixed { civil: Civil, offset: i32 },
    /// A named zone. `instant_ms` is set when the operand carries an offset
    /// (the host converts); otherwise the wall-clock fields are floating and
    /// must be interpreted in `zone` (needs tz rules).
    Named {
        civil: Civil,
        instant_ms: Option<i64>,
        zone: &'a str,
    },
}

/// Apply the `timeZone` semantics. `default` is the formatting context's
/// time zone (the server uses UTC when it does not know the visitor's zone).
pub fn resolve_zone<'a>(
    v: &DtValue,
    opt: Option<TimeZoneOpt<'a>>,
    default: TimeZoneOpt<'a>,
    errs: &mut impl ErrSink,
) -> Zoned<'a> {
    let mut target = opt.unwrap_or(default);
    if target == TimeZoneOpt::Input {
        match v.offset {
            Some(o) => target = TimeZoneOpt::Offset(o),
            None => {
                errs.push(Error::BadOperand);
                target = if default == TimeZoneOpt::Input {
                    TimeZoneOpt::Utc
                } else {
                    default
                };
            }
        }
    }
    let to_fixed = |o: i32| match v.offset {
        Some(vo) => {
            let instant = v.civil.epoch_ms_as_utc() - i64::from(vo) * 60_000;
            Civil::from_epoch_ms(instant + i64::from(o) * 60_000)
        }
        None => v.civil,
    };
    match target {
        TimeZoneOpt::Utc | TimeZoneOpt::Input => Zoned::Fixed {
            civil: to_fixed(0),
            offset: 0,
        },
        TimeZoneOpt::Offset(o) => Zoned::Fixed {
            civil: to_fixed(o),
            offset: o,
        },
        TimeZoneOpt::Named(zone) => Zoned::Named {
            civil: v.civil,
            instant_ms: v
                .offset
                .map(|vo| v.civil.epoch_ms_as_utc() - i64::from(vo) * 60_000),
            zone,
        },
    }
}

/// Neutral rendering (`YYYY-MM-DDTHH:MM:SS`), used by the size probe's
/// "semantics only" variant so that the resolution code is not eliminated.
pub fn write_neutral(c: &Civil, out: &mut [u8; 19]) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let y = c.year.rem_euclid(10_000) as u16;
    let digits = |n: u16| (b'0' + (n / 10 % 10) as u8, b'0' + (n % 10) as u8);
    let (a, b) = digits(y / 100);
    let (c1, d) = digits(y % 100);
    let (mo1, mo2) = digits(u16::from(c.month));
    let (d1, d2) = digits(u16::from(c.day));
    let (h1, h2) = digits(u16::from(c.hour));
    let (mi1, mi2) = digits(u16::from(c.minute));
    let (s1, s2) = digits(u16::from(c.second));
    *out = [
        a, b, c1, d, b'-', mo1, mo2, b'-', d1, d2, b'T', h1, h2, b':', mi1, mi2, b':', s1, s2,
    ];
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    struct V(Vec<Error>);
    impl ErrSink for V {
        fn push(&mut self, e: Error) {
            self.0.push(e);
        }
    }

    #[test]
    fn operands() {
        assert!(parse_operand("2006-01-02").is_some());
        assert!(parse_operand("2006-01-02T15:04:06").is_some());
        assert!(parse_operand("2006-01-02T15:04:06.1Z").is_some());
        assert!(parse_operand("2006-01-02T15:04:06+14:00").is_some());
        assert!(parse_operand("2006-01-02T15:04:06+14:01").is_none());
        assert!(parse_operand("0000-01-02").is_none());
        assert!(parse_operand("2006-02-30").is_none());
        assert!(parse_operand("horse").is_none());
        assert!(parse_operand("2006-01-02T15:04").is_none());
        assert!(parse_operand("2006-01-02Z").is_none());
    }

    #[test]
    fn epoch_roundtrip() {
        for ms in [0i64, 1_136_214_246_000, -86_400_001, 253_402_300_799_999] {
            assert_eq!(Civil::from_epoch_ms(ms).epoch_ms_as_utc(), ms);
        }
        let c = parse_operand("2006-01-02T15:04:06").map(|v| v.civil);
        assert_eq!(c.map(|c| c.iso_weekday()), Some(1));
    }

    #[test]
    fn options() {
        let mut e = V(Vec::new());
        let p = resolve(
            Func::DateTime,
            &[
                ("dateLength", OptValue::Literal("long")),
                ("timePrecision", OptValue::Variable("second")),
                ("hour12", OptValue::Variable("true")),
            ],
            Overrides::default(),
            &mut e,
        );
        assert!(p.length == Length::Long);
        assert!(p.time == Some(Precision::Minute));
        assert!(p.ov.hour12 == Some(true));
        assert!(e.0 == [Error::BadOption]);
    }

    #[test]
    fn zones() {
        let mut e = V(Vec::new());
        let v = parse_operand("2006-01-02T23:30:00-02:00").unwrap_or(DtValue {
            civil: Civil::from_epoch_ms(0),
            offset: None,
        });
        match resolve_zone(&v, None, TimeZoneOpt::Utc, &mut e) {
            Zoned::Fixed { civil, offset } => {
                assert_eq!((civil.day, civil.hour, civil.minute, offset), (3, 1, 30, 0));
            }
            Zoned::Named { .. } => unreachable!(),
        }
        let f = parse_operand("2006-01-02").unwrap_or(v);
        let _ = resolve_zone(&f, Some(TimeZoneOpt::Input), TimeZoneOpt::Utc, &mut e);
        assert!(e.0 == [Error::BadOperand]);
    }
}

/// Probe harness only (not part of the semantics being measured on its own):
/// options as `name=value` separated by spaces; a value written `$x` models a
/// variable-set option whose resolved value is `x`.
pub mod harness {
    use super::{
        parse_operand, resolve, resolve_zone, ErrSink, Func, OptValue, Overrides, Plan, TimeZoneOpt,
        Zoned,
    };

    pub const MAX_OPTS: usize = 8;

    pub fn parse_opts<'a>(s: &'a str, out: &mut [(&'a str, OptValue<'a>); MAX_OPTS]) -> usize {
        // Byte-wise (no `str` pattern machinery: that would drag in panicking
        // slicing and fmt, which the product never needs — its options arrive
        // pre-split from the catalog).
        let b = s.as_bytes();
        let mut n = 0;
        let mut start = 0;
        let mut eq = None;
        for i in 0..=b.len() {
            let c = b.get(i).copied().unwrap_or(b' ');
            if c == b'=' && eq.is_none() {
                eq = Some(i);
            } else if c == b' ' {
                if let Some(e) = eq {
                    let k = s.get(start..e);
                    let v = s.get(e + 1..i);
                    if let (Some(k), Some(v), Some(slot)) = (k, v, out.get_mut(n)) {
                        let val = match v.strip_prefix('$') {
                            Some(var) => OptValue::Variable(var),
                            None => OptValue::Literal(v),
                        };
                        *slot = (k, val);
                        n += 1;
                    }
                }
                start = i + 1;
                eq = None;
            }
        }
        n
    }

    pub fn func(code: u8) -> Func {
        match code {
            1 => Func::Date,
            2 => Func::Time,
            _ => Func::DateTime,
        }
    }

    /// Operand parse + option resolution + zone resolution, with the context's
    /// default time zone = UTC. `None` = Bad Operand (fallback output).
    pub fn plan<'a>(
        func_code: u8,
        opts: &'a str,
        operand: &'a str,
        errs: &mut impl ErrSink,
    ) -> Option<(Plan<'a>, Zoned<'a>)> {
        let mut buf = [("", OptValue::Literal("")); MAX_OPTS];
        let n = parse_opts(opts, &mut buf);
        let Some(v) = parse_operand(operand) else {
            errs.push(super::Error::BadOperand);
            return None;
        };
        let p = resolve(func(func_code), buf.get(..n).unwrap_or(&[]), Overrides::default(), errs);
        let z = resolve_zone(&v, p.ov.time_zone, TimeZoneOpt::Utc, errs);
        Some((p, z))
    }
}
