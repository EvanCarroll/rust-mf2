//! Date/time values (`datetime.md`): what an
//! application passes ([`crate::Arg::DateTime`]), what `mf2-fn-datetime`
//! resolves (the value with its [`DateTimeOptions`]), the formatting
//! context's [`TimeZone`], and the request a host's date formatter receives.
//! The semantics — literal parsing, option rules, zone conversion — are
//! `mf2-fn-datetime`'s; this module holds the types and the civil-calendar
//! arithmetic both sides need.

use crate::sink::Sink;
use crate::text::write_u64;

/// The largest year magnitude a [`Date`] holds (an implementation limit,
/// wide enough for any instant an `i64` of milliseconds can name).
const MAX_YEAR: i32 = 999_999;

/// Milliseconds per day.
const DAY_MS: i64 = 86_400_000;

/// A civil date in the proleptic Gregorian calendar (ISO 8601).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

/// Whether `year` is a Gregorian leap year.
const fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// The number of days of `month` (1–12) in `year`.
const fn month_days(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl Date {
    /// The date `year-month-day`; `None` unless the month is 1–12, the day
    /// exists in that month, and `|year|` ≤ 999,999.
    pub const fn new(year: i32, month: u8, day: u8) -> Option<Date> {
        if year < -MAX_YEAR || year > MAX_YEAR || month < 1 || month > 12 {
            return None;
        }
        if day < 1 || day > month_days(year, month) {
            return None;
        }
        Some(Date { year, month, day })
    }

    /// The year (astronomical: 0 is 1 BCE).
    pub const fn year(self) -> i32 {
        self.year
    }

    /// The month, 1–12.
    pub const fn month(self) -> u8 {
        self.month
    }

    /// The day of the month, 1–31.
    pub const fn day(self) -> u8 {
        self.day
    }

    /// Days since 1970-01-01 (negative before it).
    pub const fn days_since_epoch(self) -> i64 {
        // Howard Hinnant's `days_from_civil`: exact over the whole range.
        let m = self.month as i64;
        let y = self.year as i64 - if m <= 2 { 1 } else { 0 };
        let era = (if y >= 0 { y } else { y - 399 }) / 400;
        let yoe = y - era * 400;
        let mp = if m > 2 { m - 3 } else { m + 9 };
        let doy = (153 * mp + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The date `days` after 1970-01-01; `None` past the year limit.
    pub const fn from_days_since_epoch(days: i64) -> Option<Date> {
        // Hinnant's `civil_from_days`. Past ±400 million days the year
        // limit is exceeded anyway; the bound keeps the arithmetic in range.
        if days < -400_000_000 || days > 400_000_000 {
            return None;
        }
        let z = days + 719_468;
        let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
        if year < -(MAX_YEAR as i64) || year > MAX_YEAR as i64 {
            return None;
        }
        // In range by the checks above: 1 ≤ month ≤ 12, 1 ≤ day ≤ 31.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (year, month, day) = (year as i32, month as u8, day as u8);
        Date::new(year, month, day)
    }

    /// The ISO weekday: 1 = Monday … 7 = Sunday.
    pub const fn weekday(self) -> u8 {
        // 1970-01-01 was a Thursday (4).
        let w = (self.days_since_epoch() + 3).rem_euclid(7);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let w = w as u8;
        w + 1
    }
}

/// A wall-clock time, to the millisecond (a date/time literal has at most
/// three fraction digits).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Time {
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
}

impl Time {
    /// 00:00:00.000 — the time of a literal that has none (datetime.md).
    pub const MIDNIGHT: Time = Time {
        hour: 0,
        minute: 0,
        second: 0,
        millisecond: 0,
    };

    /// `hour:minute:second.millisecond`; `None` unless 0–23, 0–59, 0–59,
    /// 0–999.
    pub const fn new(hour: u8, minute: u8, second: u8, millisecond: u16) -> Option<Time> {
        if hour > 23 || minute > 59 || second > 59 || millisecond > 999 {
            return None;
        }
        Some(Time {
            hour,
            minute,
            second,
            millisecond,
        })
    }

    /// The hour, 0–23.
    pub const fn hour(self) -> u8 {
        self.hour
    }

    /// The minute, 0–59.
    pub const fn minute(self) -> u8 {
        self.minute
    }

    /// The second, 0–59.
    pub const fn second(self) -> u8 {
        self.second
    }

    /// The millisecond, 0–999.
    pub const fn millisecond(self) -> u16 {
        self.millisecond
    }

    /// Milliseconds since midnight.
    pub const fn ms_of_day(self) -> i64 {
        ((self.hour as i64 * 60 + self.minute as i64) * 60 + self.second as i64) * 1000
            + self.millisecond as i64
    }

    /// The time `ms` milliseconds after midnight (`0 ≤ ms < 86,400,000`).
    pub const fn from_ms_of_day(ms: i64) -> Option<Time> {
        if ms < 0 || ms >= DAY_MS {
            return None;
        }
        // In range by the check: every quotient fits its field.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (h, m, s, milli) = (
            (ms / 3_600_000) as u8,
            (ms / 60_000 % 60) as u8,
            (ms / 1000 % 60) as u8,
            (ms % 1000) as u16,
        );
        Time::new(h, m, s, milli)
    }
}

/// The largest UTC offset magnitude, in seconds (exclusive): a day.
const MAX_OFFSET: i32 = 86_400;

/// A date/time value: an application's argument ([`crate::Arg::DateTime`],
/// [`crate::CustomValue::as_date_time`]), a literal a date/time function
/// parsed, or what `:datetime`, `:date` or `:time` resolved — the value with
/// its [`DateTimeOptions`]. Build it with the constructors.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub struct DateTime<'a> {
    /// The civil date (in the value's offset, or floating).
    pub date: Date,
    /// The wall-clock time (00:00 when the value has no time).
    pub time: Time,
    /// Seconds east of UTC; `None`: floating — no offset, so the formatting
    /// time zone applies without conversion (datetime.md, "Date and Time
    /// Operands").
    pub offset: Option<i32>,
    /// The IANA time zone the value is in, when it names one.
    pub zone: Option<&'a str>,
    /// What a date/time function resolved; empty for an argument, except
    /// for the override options an application value may carry.
    pub options: DateTimeOptions<'a>,
}

impl<'a> DateTime<'a> {
    /// A floating date and time.
    pub const fn floating(date: Date, time: Time) -> DateTime<'a> {
        DateTime {
            date,
            time,
            offset: None,
            zone: None,
            options: DateTimeOptions::NONE,
        }
    }

    /// The instant `ms` milliseconds after 1970-01-01T00:00:00Z, in UTC
    /// (datetime.md: an offset from an epoch includes UTC as its zone).
    pub const fn from_epoch_ms(ms: i64) -> Option<DateTime<'a>> {
        let Some(date) = Date::from_days_since_epoch(ms.div_euclid(DAY_MS)) else {
            return None;
        };
        let Some(time) = Time::from_ms_of_day(ms.rem_euclid(DAY_MS)) else {
            return None;
        };
        Some(DateTime {
            date,
            time,
            offset: Some(0),
            zone: None,
            options: DateTimeOptions::NONE,
        })
    }

    /// The instant, in milliseconds since the epoch; `None` when floating.
    pub const fn to_epoch_ms(&self) -> Option<i64> {
        match self.offset {
            Some(o) => Some(
                self.date.days_since_epoch() * DAY_MS + self.time.ms_of_day() - o as i64 * 1000,
            ),
            None => None,
        }
    }

    /// The same wall time at UTC offset `seconds`; `None` unless
    /// `|seconds|` < 86,400.
    pub const fn with_offset(self, seconds: i32) -> Option<Self> {
        if seconds <= -MAX_OFFSET || seconds >= MAX_OFFSET {
            return None;
        }
        Some(DateTime {
            offset: Some(seconds),
            ..self
        })
    }

    /// The same value, in the IANA zone `zone`. A value with an offset is
    /// an instant, and stays one: a date/time function shows it at the
    /// zone's wall time, converting through [`Host::zone_offset`] when the
    /// offset is not the zone's. A floating value's wall time is placed in
    /// `zone`.
    ///
    /// [`Host::zone_offset`]: crate::Host::zone_offset
    #[must_use]
    pub const fn in_zone(self, zone: &'a str) -> Self {
        DateTime {
            zone: Some(zone),
            ..self
        }
    }

    /// Writes the ISO 8601 / RFC 9557 text: `2006-01-02T15:04:06`, with
    /// `.mmm` when there are milliseconds, `Z` or `±hh:mm` when the value
    /// has an offset, `[zone]` when it names one — how an unannotated
    /// date/time formats.
    pub fn write_iso(&self, out: &mut dyn Sink) {
        let y = self.date.year;
        if (0..=9999).contains(&y) {
            write_padded(y.unsigned_abs(), 4, out);
        } else {
            out.push_str(if y < 0 { "-" } else { "+" });
            write_padded(y.unsigned_abs(), 6, out);
        }
        out.push_str("-");
        write_padded(u32::from(self.date.month), 2, out);
        out.push_str("-");
        write_padded(u32::from(self.date.day), 2, out);
        out.push_str("T");
        write_padded(u32::from(self.time.hour), 2, out);
        out.push_str(":");
        write_padded(u32::from(self.time.minute), 2, out);
        out.push_str(":");
        write_padded(u32::from(self.time.second), 2, out);
        if self.time.millisecond != 0 {
            out.push_str(".");
            write_padded(u32::from(self.time.millisecond), 3, out);
        }
        match self.offset {
            Some(0) => out.push_str("Z"),
            Some(o) => write_offset(o, out),
            None => {}
        }
        if let Some(z) = self.zone {
            out.push_str("[");
            out.push_str(z);
            out.push_str("]");
        }
    }
}

/// Writes `n` with at least `width` digits.
fn write_padded(n: u32, width: u32, out: &mut dyn Sink) {
    let mut digits = 1;
    let mut m = n;
    while m >= 10 {
        m /= 10;
        digits += 1;
    }
    while digits < width {
        out.push_str("0");
        digits += 1;
    }
    write_u64(u64::from(n), out);
}

/// Writes a UTC offset as `±hh:mm`, with `:ss` when it has seconds.
pub(crate) fn write_offset(seconds: i32, out: &mut dyn Sink) {
    out.push_str(if seconds < 0 { "-" } else { "+" });
    let s = seconds.unsigned_abs();
    write_padded(s / 3600, 2, out);
    out.push_str(":");
    write_padded(s / 60 % 60, 2, out);
    if !s.is_multiple_of(60) {
        out.push_str(":");
        write_padded(s % 60, 2, out);
    }
}

/// What `:datetime`, `:date` or `:time` resolved (datetime.md). The
/// override options (`time_zone`, `hour12`, `calendar`) travel with the
/// value into a later date/time expression that takes it as its operand.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[non_exhaustive]
pub struct DateTimeOptions<'a> {
    /// The date part; `None`: none (`:time`), or not resolved.
    pub date: Option<DateStyle>,
    /// The time part; `None`: none (`:date`), or not resolved.
    pub time: Option<TimePrecision>,
    /// `timeZoneStyle`; `None`: no time-zone indicator.
    pub time_zone_style: Option<ZoneStyle>,
    /// `timeZone`.
    pub time_zone: Option<ZoneOption<'a>>,
    /// `hour12`.
    pub hour12: Option<bool>,
    /// `calendar`: a Unicode calendar identifier.
    pub calendar: Option<&'a str>,
}

impl DateTimeOptions<'_> {
    /// No options.
    pub const NONE: DateTimeOptions<'static> = DateTimeOptions {
        date: None,
        time: None,
        time_zone_style: None,
        time_zone: None,
        hour12: None,
        calendar: None,
    };

    /// Whether a date/time function resolved the value (it has a date or a
    /// time part).
    pub const fn is_resolved(&self) -> bool {
        self.date.is_some() || self.time.is_some()
    }
}

/// The date part: `dateFields` / `fields` and `dateLength` / `length`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DateStyle {
    /// The fields shown.
    pub fields: DateFields,
    /// Their length.
    pub length: DateLength,
}

/// `dateFields` / `fields`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum DateFields {
    /// `weekday`.
    Weekday,
    /// `day-weekday`.
    DayWeekday,
    /// `month-day`.
    MonthDay,
    /// `month-day-weekday`.
    MonthDayWeekday,
    /// `year-month-day` (the default).
    YearMonthDay,
    /// `year-month-day-weekday`.
    YearMonthDayWeekday,
}

/// `dateLength` / `length`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum DateLength {
    /// `long`.
    Long,
    /// `medium` (the default).
    Medium,
    /// `short`.
    Short,
}

/// `timePrecision` / `precision`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum TimePrecision {
    /// `hour`.
    Hour,
    /// `minute` (the default).
    Minute,
    /// `second`.
    Second,
}

/// `timeZoneStyle`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum ZoneStyle {
    /// `long`.
    Long,
    /// `short`.
    Short,
}

/// A time zone as an option value (`timeZone`) or a formatting target.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum ZoneOption<'a> {
    /// `input`: the operand's own zone.
    Input,
    /// `UTC`.
    Utc,
    /// A UTC offset, in seconds east.
    Offset(i32),
    /// An IANA zone: a well-formed RFC 9557 `time-zone-name`. As the
    /// formatting context's zone only, also a POSIX TZ rule
    /// ([`TimeZone::rules`]), which the host evaluates as it does a name
    /// ([`Host::zone_offset`](crate::Host::zone_offset)).
    Named(&'a str),
}

/// The longest zone name a [`TimeZone`] holds (IANA's longest is 32), and
/// the longest POSIX TZ rule.
const MAX_ZONE_NAME: usize = 64;

/// The formatting context's time zone: the default of `timeZone`.
/// Owned — a per-request zone need not be
/// `'static`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeZone {
    repr: Repr,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Repr {
    Utc,
    Offset(i32),
    /// An IANA name; or a POSIX TZ rule ([`TimeZone::rules`]), marked by
    /// [`RULE`] in the buffer's last byte, which no name has (a name is
    /// ASCII, and a rule at most 63 bytes). Held as a name is, so that a
    /// build that never makes a rule compiles every use of a zone as it did
    /// before rules existed.
    Named {
        len: u8,
        name: [u8; MAX_ZONE_NAME],
    },
}

/// The last byte of a rule's buffer ([`Repr::Named`]).
const RULE: u8 = 0xFF;

impl TimeZone {
    /// UTC.
    pub const UTC: TimeZone = TimeZone { repr: Repr::Utc };

    /// A fixed offset, in seconds east of UTC; `None` unless `|seconds|` <
    /// 86,400.
    pub const fn offset(seconds: i32) -> Option<TimeZone> {
        if seconds <= -MAX_OFFSET || seconds >= MAX_OFFSET {
            return None;
        }
        Some(TimeZone {
            repr: Repr::Offset(seconds),
        })
    }

    /// The IANA zone `name`; `None` unless it is a well-formed RFC 9557
    /// `time-zone-name` of at most 64 bytes.
    pub fn named(name: &str) -> Option<TimeZone> {
        if !is_zone_name(name) || name.len() > MAX_ZONE_NAME {
            return None;
        }
        let mut buf = [0u8; MAX_ZONE_NAME];
        for (b, &c) in buf.iter_mut().zip(name.as_bytes()) {
            *b = c;
        }
        Some(TimeZone {
            repr: Repr::Named {
                len: u8::try_from(name.len()).ok()?,
                name: buf,
            },
        })
    }

    /// A zone that follows the POSIX TZ rule `rule`: an offset for standard
    /// time and, where the rule has one, another for daylight saving time
    /// between two dates each year (`EST5EDT,M3.2.0,M11.1.0`). What a
    /// native application's system zone is when it has no IANA name, so
    /// that its dates follow the system's changes of offset rather than the
    /// offset in force when it started.
    ///
    /// The host evaluates it, as it does an IANA name: `mf2-host-std` does;
    /// a host that does not (the browser's) leaves a date in it a *Bad
    /// Option* with a fallback, as for a zone it does not know. `None`
    /// unless `rule` is 1 to 63 printable ASCII characters; whether it is a
    /// rule the host can read, only the host knows.
    pub fn rules(rule: &str) -> Option<TimeZone> {
        if rule.is_empty()
            || rule.len() >= MAX_ZONE_NAME
            || !rule.bytes().all(|b| b.is_ascii_graphic())
        {
            return None;
        }
        let mut name = [0u8; MAX_ZONE_NAME];
        name.get_mut(..rule.len())?.copy_from_slice(rule.as_bytes());
        if let Some(last) = name.last_mut() {
            *last = RULE;
        }
        Some(TimeZone {
            repr: Repr::Named {
                len: u8::try_from(rule.len()).ok()?,
                name,
            },
        })
    }

    /// The zone as an option value: `Utc`, `Offset` or `Named` — a POSIX TZ
    /// rule ([`TimeZone::rules`]) as `Named`, which the host evaluates.
    pub fn as_option(&self) -> ZoneOption<'_> {
        match &self.repr {
            Repr::Utc => ZoneOption::Utc,
            Repr::Offset(s) => ZoneOption::Offset(*s),
            Repr::Named { len, name } => ZoneOption::Named(
                name.get(..usize::from(*len))
                    .and_then(|b| core::str::from_utf8(b).ok())
                    .unwrap_or(""),
            ),
        }
    }

    /// The POSIX TZ rule, for a zone made by [`TimeZone::rules`].
    fn rule(&self) -> Option<&str> {
        match &self.repr {
            Repr::Named { len, name } if name.last() == Some(&RULE) => name
                .get(..usize::from(*len))
                .and_then(|b| core::str::from_utf8(b).ok()),
            _ => None,
        }
    }
}

/// `TimeZone("Europe/Paris")`, `TimeZone(Offset(3600))`, `TimeZone(Utc)`, or
/// `TimeZone(Rules("EST5EDT,M3.2.0,M11.1.0"))`.
impl core::fmt::Debug for TimeZone {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(rule) = self.rule() {
            return f
                .write_str("TimeZone(Rules(")
                .and_then(|()| core::fmt::Debug::fmt(rule, f))
                .and_then(|()| f.write_str("))"));
        }
        match self.as_option() {
            ZoneOption::Named(n) => f.debug_tuple("TimeZone").field(&n).finish(),
            other => f.debug_tuple("TimeZone").field(&other).finish(),
        }
    }
}

impl Default for TimeZone {
    fn default() -> Self {
        TimeZone::UTC
    }
}

/// Whether `s` is a well-formed RFC 9557 `time-zone-name`: parts separated
/// by `/`, each starting with a letter, `.` or `_`, then letters, digits,
/// `.`, `_`, `-` or `+`, and neither `.` nor `..`.
pub fn is_zone_name(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() {
        return false;
    }
    let mut start = 0;
    let mut i = 0;
    while i <= b.len() {
        let end = i == b.len();
        if end || b.get(i) == Some(&b'/') {
            let part = b.get(start..i).unwrap_or(&[]);
            if !zone_part(part) {
                return false;
            }
            start = i + 1;
        }
        i += 1;
    }
    true
}

fn zone_part(p: &[u8]) -> bool {
    let initial = |c: u8| c.is_ascii_alphabetic() || c == b'.' || c == b'_';
    match p {
        [] | [b'.'] | [b'.', b'.'] => false,
        [first, rest @ ..] => {
            initial(*first)
                && rest
                    .iter()
                    .all(|&c| initial(c) || c.is_ascii_digit() || c == b'-' || c == b'+')
        }
    }
}

/// What a host's date formatter receives (`Host::format_date_time`,
/// the `intl` date formatter): an instant and how to show it.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct DateTimeRequest<'r> {
    /// The instant, in milliseconds since the epoch. For a floating value
    /// shown without a zone: its wall time read as UTC, with `zone` = UTC.
    pub epoch_ms: i64,
    /// The zone to show it in; never [`ZoneOption::Input`].
    pub zone: ZoneOption<'r>,
    /// The resolved options: the date and time parts, the zone style,
    /// `hour12`, `calendar`.
    pub options: &'r DateTimeOptions<'r>,
}

impl<'r> DateTimeRequest<'r> {
    /// The request to show `epoch_ms` in `zone` with `options` (a date
    /// backend makes it; a later version may add fields).
    pub const fn new(
        epoch_ms: i64,
        zone: ZoneOption<'r>,
        options: &'r DateTimeOptions<'r>,
    ) -> DateTimeRequest<'r> {
        DateTimeRequest {
            epoch_ms,
            zone,
            options,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[test]
    fn civil_days_round_trip() {
        for days in [-719_468i64, -1, 0, 1, 10_957, 13_150, 2_932_896, -2_932_897] {
            let d = Date::from_days_since_epoch(days).unwrap();
            assert_eq!(d.days_since_epoch(), days, "{d:?}");
        }
        let d = Date::new(2006, 1, 2).unwrap();
        assert_eq!(d.days_since_epoch(), 13_150);
        assert_eq!(d.weekday(), 1); // a Monday
        assert_eq!(Date::new(1970, 1, 1).unwrap().weekday(), 4);
        assert!(Date::new(2023, 2, 29).is_none());
        assert!(Date::new(2024, 2, 29).is_some());
        assert!(Date::new(1900, 2, 29).is_none());
        assert!(Date::new(2000, 2, 29).is_some());
        assert!(Date::new(1_000_000, 1, 1).is_none());
    }

    #[test]
    fn epoch_round_trip() {
        for ms in [
            0i64,
            1,
            -1,
            1_136_214_246_000,
            -62_135_596_800_000,
            253_402_300_799_999,
        ] {
            let dt = DateTime::from_epoch_ms(ms).unwrap();
            assert_eq!(dt.to_epoch_ms(), Some(ms));
        }
        let dt = DateTime::from_epoch_ms(1_136_214_246_000).unwrap();
        let mut s = String::new();
        dt.write_iso(&mut s);
        assert_eq!(s, "2006-01-02T15:04:06Z");
        let dt = dt.with_offset(-7 * 3600).unwrap().in_zone("America/Denver");
        s.clear();
        dt.write_iso(&mut s);
        assert_eq!(s, "2006-01-02T15:04:06-07:00[America/Denver]");
        assert_eq!(dt.to_epoch_ms(), Some(1_136_214_246_000 + 7 * 3_600_000));
        assert!(DateTime::from_epoch_ms(i64::MAX).is_none());
        assert!(dt.with_offset(86_400).is_none());
    }

    #[test]
    fn iso_text() {
        let mut s = String::new();
        let dt = DateTime::floating(
            Date::new(-44, 3, 15).unwrap(),
            Time::new(9, 5, 0, 7).unwrap(),
        );
        dt.write_iso(&mut s);
        assert_eq!(s, "-000044-03-15T09:05:00.007");
    }

    #[test]
    fn zones() {
        for good in [
            "UTC",
            "America/New_York",
            "Etc/GMT+5",
            "America/Argentina/ComodRivadavia",
            "._x",
        ] {
            assert!(is_zone_name(good), "{good}");
            let z = TimeZone::named(good).unwrap();
            assert_eq!(z.as_option(), ZoneOption::Named(good));
        }
        for bad in ["", "/", "a/", "/a", "a//b", ".", "a/..", "1a", "a b", "é"] {
            assert!(!is_zone_name(bad), "{bad}");
            assert!(TimeZone::named(bad).is_none());
        }
        assert_eq!(TimeZone::default().as_option(), ZoneOption::Utc);
        assert_eq!(
            TimeZone::offset(3600).unwrap().as_option(),
            ZoneOption::Offset(3600)
        );
        assert!(TimeZone::offset(-86_400).is_none());
    }

    /// A POSIX TZ rule is carried as it is written, and the host sees it
    /// where it sees a zone's name; it is not a name.
    #[test]
    fn rules() {
        let rule = "EST5EDT,M3.2.0,M11.1.0";
        assert!(!is_zone_name(rule));
        let z = TimeZone::rules(rule).unwrap();
        assert_eq!(z.as_option(), ZoneOption::Named(rule));
        assert_ne!(Some(z), TimeZone::named("EST5EDT"));
        let mut shown = String::new();
        core::fmt::write(&mut shown, format_args!("{z:?}")).unwrap();
        assert_eq!(shown, "TimeZone(Rules(\"EST5EDT,M3.2.0,M11.1.0\"))");
        let longest = "<+0330>-3:30<+0430>,J79/24,J263/24-and-then-some-to-sixty-three";
        assert_eq!(longest.len(), 63);
        assert_eq!(
            TimeZone::rules(longest).unwrap().as_option(),
            ZoneOption::Named(longest)
        );
        for bad in [
            "",
            "EST 5",
            "CET-1CEST,M3.5.0,M10.5.0/3\n",
            "é",
            &"x".repeat(64),
        ] {
            assert!(TimeZone::rules(bad).is_none(), "{bad}");
        }
        // A name of the longest length is a name, not a rule.
        let name = "a".repeat(64);
        let mut shown = String::new();
        core::fmt::write(
            &mut shown,
            format_args!("{:?}", TimeZone::named(&name).unwrap()),
        )
        .unwrap();
        let mut expected = String::new();
        core::fmt::write(&mut expected, format_args!("TimeZone({name:?})")).unwrap();
        assert_eq!(shown, expected);
    }
}
