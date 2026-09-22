//! `:datetime`, `:date`, `:time` and unannotated date/time values through
//! the public path: `mf2::compile_str`, a `Formatter` over a registry of
//! this crate's handlers, the neutral backend. Zones with a test host that
//! knows a few (EU and US daylight-saving rules, a fixed offset, an offset
//! with seconds).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::type_complexity,
    clippy::unnecessary_wraps
)]

use mf2::{
    Arg, BidiStrategy, CustomValue, Date, DateFields, DateLength, DateStyle, DateTime,
    DateTimeOptions, Dir, FormatContext, FormatError, Formatter, Function, Host, Part, PartSink,
    Registry, Time, TimePrecision, TimeZone, Value, ZoneOption, ZoneStyle,
};
use mf2_fn_datetime::{DATE, DATES, DATETIME, DateTimeFunction, Neutral, Plan, TIME};

use FormatError::{BadOperand, BadOption, BadSelector, UnresolvedVariable};

static FUNCTIONS: [(&str, &dyn Function); 5] = [
    ("date", &DATE),
    ("datetime", &DATETIME),
    ("number", &mf2::functions::NUMBER),
    ("string", &mf2::functions::STRING),
    ("time", &TIME),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_dates(&DATES);
/// No unannotated date handler.
static BARE: Registry = Registry::new(&FUNCTIONS);

const HOUR: i32 = 3600;
const DAY_MS: i64 = 86_400_000;

/// Milliseconds since the epoch of `y-m-d h:00` UTC.
fn utc_ms(y: i32, m: u8, d: u8, h: i64) -> i64 {
    Date::new(y, m, d).unwrap().days_since_epoch() * DAY_MS + h * 3_600_000
}

/// The day of the last Sunday of a 31-day month.
fn last_sunday(y: i32, m: u8) -> u8 {
    let w = Date::new(y, m, 31).unwrap().weekday();
    31 - w % 7
}

/// The day of the `n`th Sunday of a month.
fn nth_sunday(y: i32, m: u8, n: u8) -> u8 {
    let w = Date::new(y, m, 1).unwrap().weekday();
    1 + (7 - w) % 7 + 7 * (n - 1)
}

/// A host with a few zones.
struct Zones;

impl Host for Zones {
    fn nfc<'a>(&self, s: &'a str, buf: &'a mut String) -> &'a str {
        mf2::host_std::HOST.nfc(s, buf)
    }

    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        mf2::host_std::HOST.f64_to_text(x, buf)
    }

    fn zone_offset(&self, zone: &str, t: i64) -> Option<i32> {
        let y = DateTime::from_epoch_ms(t)?.date.year();
        match zone {
            // EU: summer time from the last Sunday of March to the last
            // Sunday of October, 01:00 UTC.
            "Europe/Paris" => {
                let start = utc_ms(y, 3, last_sunday(y, 3), 1);
                let end = utc_ms(y, 10, last_sunday(y, 10), 1);
                Some(if (start..end).contains(&t) {
                    2 * HOUR
                } else {
                    HOUR
                })
            }
            // US since 2007: the second Sunday of March, 02:00 local, to the
            // first Sunday of November, 02:00 local.
            "America/New_York" => {
                let start = utc_ms(y, 3, nth_sunday(y, 3, 2), 7);
                let end = utc_ms(y, 11, nth_sunday(y, 11, 1), 6);
                Some(if (start..end).contains(&t) {
                    -4 * HOUR
                } else {
                    -5 * HOUR
                })
            }
            "Asia/Kolkata" => Some(5 * HOUR + 1800),
            // Paris mean time (LMT): an offset with seconds.
            "Test/Seconds" => Some(561),
            // +01:00, but +02:00 for six hours: two transitions in a day.
            "Test/Blip" => {
                let start = utc_ms(2006, 6, 1, 10);
                Some(if (start..start + 6 * 3_600_000).contains(&t) {
                    2 * HOUR
                } else {
                    HOUR
                })
            }
            _ => None,
        }
    }
}

static ZONES: Zones = Zones;

fn context(host: &'static dyn Host, zone: TimeZone) -> FormatContext {
    let mut cx = FormatContext::new(host);
    cx.bidi = BidiStrategy::None;
    cx.time_zone = zone;
    cx
}

/// The std host (no zone data), UTC.
fn std_utc() -> FormatContext {
    context(&mf2::host_std::HOST, TimeZone::UTC)
}

/// The zone host, UTC.
fn zones_utc() -> FormatContext {
    context(&ZONES, TimeZone::UTC)
}

fn run_in(
    registry: &Registry,
    cx: &FormatContext,
    src: &str,
    args: &[(&str, Arg<'_>)],
) -> (String, Vec<FormatError>) {
    let m = mf2::compile_str(src, "en").unwrap_or_else(|e| panic!("{src}: {e}"));
    let f = Formatter::new(&m.catalog, registry, cx);
    let mut out = String::new();
    let mut errors = Vec::new();
    f.write_named(mf2::Compiled::ID, args, &mut out, &mut errors);
    (out, errors)
}

fn run(cx: &FormatContext, src: &str, args: &[(&str, Arg<'_>)]) -> (String, Vec<FormatError>) {
    run_in(&REGISTRY, cx, src, args)
}

/// Formats `src` (std host, UTC) and requires no errors.
fn ok(src: &str) -> String {
    ok_in(&std_utc(), src, &[])
}

fn ok_in(cx: &FormatContext, src: &str, args: &[(&str, Arg<'_>)]) -> String {
    let (s, e) = run(cx, src, args);
    assert!(e.is_empty(), "{src}: {e:?} ({s:?})");
    s
}

/// Formats `src` and returns the output and its errors.
fn err(src: &str) -> (String, Vec<FormatError>) {
    run(&std_utc(), src, &[])
}

/// What an expression part holds.
#[derive(Debug, Default)]
struct Seen {
    kind: String,
    dir: Option<Dir>,
    text: String,
    value: Option<(Date, Time, Option<i32>, Option<String>, OptionsSeen)>,
}

/// A copy of resolved options (their zone name owned).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct OptionsSeen {
    date: Option<DateStyle>,
    time: Option<TimePrecision>,
    zone_style: Option<ZoneStyle>,
    time_zone: Option<String>,
    hour12: Option<bool>,
    calendar: Option<String>,
}

fn zone_text(z: ZoneOption<'_>) -> String {
    match z {
        ZoneOption::Input => "input".into(),
        ZoneOption::Utc => "UTC".into(),
        ZoneOption::Offset(o) => format!("{o:+}s"),
        ZoneOption::Named(n) => n.into(),
    }
}

fn options_seen(o: &DateTimeOptions<'_>) -> OptionsSeen {
    OptionsSeen {
        date: o.date,
        time: o.time,
        zone_style: o.time_zone_style,
        time_zone: o.time_zone.map(zone_text),
        hour12: o.hour12,
        calendar: o.calendar.map(Into::into),
    }
}

struct Probe(Vec<Seen>);

impl PartSink for Probe {
    fn part(&mut self, part: Part<'_>) {
        if let Part::Expression(e) = part {
            let mut text = String::new();
            e.write(&mut text);
            let value = match e.value() {
                Value::DateTime(d) => Some((
                    d.date,
                    d.time,
                    d.offset,
                    d.zone.map(Into::into),
                    options_seen(&d.options),
                )),
                _ => None,
            };
            self.0.push(Seen {
                kind: e.kind().into(),
                dir: Some(e.dir()),
                text,
                value,
            });
        }
    }
}

/// The expression parts of `src`.
fn parts(cx: &FormatContext, src: &str, args: &[(&str, Arg<'_>)]) -> Vec<Seen> {
    let m = mf2::compile_str(src, "en").unwrap();
    let f = Formatter::new(&m.catalog, &REGISTRY, cx);
    let mut p = Probe(Vec::new());
    f.parts_named(mf2::Compiled::ID, args, &mut p, &mut Vec::new());
    p.0
}

/// The resolved options of the one expression of `src`.
fn resolved(src: &str) -> OptionsSeen {
    let p = parts(&std_utc(), src, &[]);
    assert_eq!(p.len(), 1, "{src}");
    p.into_iter().next().unwrap().value.unwrap().4
}

fn style(fields: DateFields, length: DateLength) -> Option<DateStyle> {
    Some(DateStyle { fields, length })
}

// ───────────────────────────────────────────────────────────── the suite ──

#[test]
fn suite_cases() {
    // functions/date.json
    assert_eq!(err("{:date}"), ("{:date}".into(), vec![BadOperand]));
    assert_eq!(err("{horse :date}"), ("{|horse|}".into(), vec![BadOperand]));
    assert_eq!(ok("{|2006-01-02| :date}"), "2006-01-02");
    assert_eq!(ok("{|2006-01-02T15:04:06| :date}"), "2006-01-02");
    assert_eq!(ok("{|2006-01-02| :date length=long}"), "2006-01-02");
    assert_eq!(
        ok(".local $d = {|2006-01-02| :date length=long} {{{$d}}}"),
        "2006-01-02"
    );
    assert_eq!(
        ok(
            ".local $d = {|2006-01-02| :datetime dateLength=long timePrecision=second} {{{$d :date}}}"
        ),
        "2006-01-02"
    );
    // functions/time.json
    assert_eq!(err("{:time}"), ("{:time}".into(), vec![BadOperand]));
    assert_eq!(err("{horse :time}"), ("{|horse|}".into(), vec![BadOperand]));
    assert_eq!(ok("{|2006-01-02T15:04:06| :time}"), "15:04");
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :time precision=second}"),
        "15:04:06"
    );
    assert_eq!(
        ok(".local $t = {|2006-01-02T15:04:06| :time precision=second} {{{$t}}}"),
        "15:04:06"
    );
    assert_eq!(
        ok(
            ".local $t = {|2006-01-02T15:04:06| :datetime dateLength=long timePrecision=second} {{{$t :time}}}"
        ),
        "15:04"
    );
    // functions/datetime.json
    assert_eq!(err("{:datetime}"), ("{:datetime}".into(), vec![BadOperand]));
    struct Bool;
    impl CustomValue for Bool {}
    let (s, e) = run(&std_utc(), "{$x :datetime}", &[("x", Arg::Custom(&Bool))]);
    assert_eq!((s.as_str(), e.as_slice()), ("{$x}", &[BadOperand][..]));
    assert_eq!(
        err("{horse :datetime}"),
        ("{|horse|}".into(), vec![BadOperand])
    );
    assert_eq!(ok("{|2006-01-02T15:04:06| :datetime}"), "2006-01-02 15:04");
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :datetime dateLength=long}"),
        "2006-01-02 15:04"
    );
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :datetime timePrecision=second}"),
        "2006-01-02 15:04:06"
    );
    let dt = mf2_fn_datetime::parse_literal("2006-01-02T15:04:06").unwrap();
    assert_eq!(
        ok_in(&std_utc(), "{$dt :datetime}", &[("dt", Arg::DateTime(&dt))]),
        "2006-01-02 15:04"
    );
}

// ─────────────────────────────────────────────────────────────── options ──

#[test]
fn defaults() {
    use DateFields::YearMonthDay as Ymd;
    use DateLength::Medium;
    let dt = resolved("{|2006-01-02T15:04:06| :datetime}");
    assert_eq!(
        dt,
        OptionsSeen {
            date: style(Ymd, Medium),
            time: Some(TimePrecision::Minute),
            ..OptionsSeen::default()
        }
    );
    let d = resolved("{|2006-01-02T15:04:06| :date}");
    assert_eq!(
        d,
        OptionsSeen {
            date: style(Ymd, Medium),
            ..OptionsSeen::default()
        }
    );
    let t = resolved("{|2006-01-02T15:04:06| :time}");
    assert_eq!(
        t,
        OptionsSeen {
            time: Some(TimePrecision::Minute),
            ..OptionsSeen::default()
        }
    );
}

#[test]
fn every_option_value() {
    for (v, f, text) in [
        ("weekday", DateFields::Weekday, "Mon 15:04"),
        ("day-weekday", DateFields::DayWeekday, "---02 Mon 15:04"),
        ("month-day", DateFields::MonthDay, "--01-02 15:04"),
        (
            "month-day-weekday",
            DateFields::MonthDayWeekday,
            "--01-02 Mon 15:04",
        ),
        (
            "year-month-day",
            DateFields::YearMonthDay,
            "2006-01-02 15:04",
        ),
        (
            "year-month-day-weekday",
            DateFields::YearMonthDayWeekday,
            "2006-01-02 Mon 15:04",
        ),
    ] {
        let src = format!("{{|2006-01-02T15:04:06| :datetime dateFields={v}}}");
        assert_eq!(ok(&src), text);
        assert_eq!(resolved(&src).date, style(f, DateLength::Medium));
        let src = format!("{{|2006-01-02T15:04:06| :date fields={v}}}");
        assert_eq!(ok(&src), text.trim_end_matches(" 15:04"));
        assert_eq!(resolved(&src).date, style(f, DateLength::Medium));
    }
    for (v, l) in [
        ("long", DateLength::Long),
        ("medium", DateLength::Medium),
        ("short", DateLength::Short),
    ] {
        let src = format!("{{|2006-01-02T15:04:06| :datetime dateLength={v}}}");
        assert_eq!(ok(&src), "2006-01-02 15:04");
        assert_eq!(resolved(&src).date, style(DateFields::YearMonthDay, l));
        let src = format!("{{|2006-01-02| :date length={v}}}");
        assert_eq!(resolved(&src).date, style(DateFields::YearMonthDay, l));
    }
    for (v, p, text) in [
        ("hour", TimePrecision::Hour, "15"),
        ("minute", TimePrecision::Minute, "15:04"),
        ("second", TimePrecision::Second, "15:04:06"),
    ] {
        let src = format!("{{|2006-01-02T15:04:06| :datetime timePrecision={v}}}");
        assert_eq!(ok(&src), format!("2006-01-02 {text}"));
        assert_eq!(resolved(&src).time, Some(p));
        let src = format!("{{|2006-01-02T15:04:06| :time precision={v}}}");
        assert_eq!(ok(&src), text);
        assert_eq!(resolved(&src).time, Some(p));
    }
    for (v, s, text) in [
        ("long", ZoneStyle::Long, "UTC"),
        ("short", ZoneStyle::Short, "Z"),
    ] {
        let src = format!("{{|2006-01-02T15:04:06| :datetime timeZoneStyle={v}}}");
        assert_eq!(ok(&src), format!("2006-01-02 15:04 {text}"));
        assert_eq!(resolved(&src).zone_style, Some(s));
        let src = format!("{{|2006-01-02T15:04:06| :time timeZoneStyle={v}}}");
        assert_eq!(ok(&src), format!("15:04 {text}"));
    }
    assert_eq!(ok("{|2006-01-02T15:04:06| :time hour12=true}"), "03:04 PM");
    assert_eq!(ok("{|2006-01-02T15:04:06| :time hour12=false}"), "15:04");
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :datetime hour12=true timePrecision=second}"),
        "2006-01-02 03:04:06 PM"
    );
    assert_eq!(ok("{|2006-01-02T00:30:00| :time hour12=true}"), "12:30 AM");
    assert_eq!(
        ok("{|2006-01-02T12:00:00| :time hour12=true precision=hour}"),
        "12 PM"
    );
    assert_eq!(
        resolved("{|2006-01-02| :time hour12=false}").hour12,
        Some(false)
    );
    for f in ["datetime", "date", "time"] {
        let src = format!("{{|2006-01-02| :{f} calendar=japanese}}");
        assert_eq!(resolved(&src).calendar.as_deref(), Some("japanese"));
        let src = format!("{{|2006-01-02| :{f} calendar=islamic-umalqura}}");
        assert_eq!(resolved(&src).calendar.as_deref(), Some("islamic-umalqura"));
    }
    // The neutral backend ignores the calendar.
    assert_eq!(ok("{|2006-01-02| :date calendar=japanese}"), "2006-01-02");
}

#[test]
fn neutral_pieces() {
    assert_eq!(ok("{|2006-01-08| :date fields=weekday}"), "Sun");
    assert_eq!(ok("{|2006-01-07| :date fields=day-weekday}"), "---07 Sat");
    assert_eq!(ok("{|0001-01-01| :date}"), "0001-01-01");
    let far = DateTime::floating(Date::new(10_000, 1, 1).unwrap(), Time::MIDNIGHT);
    assert_eq!(
        ok_in(&std_utc(), "{$d :date}", &[("d", Arg::DateTime(&far))]),
        "+010000-01-01"
    );
    let bce = DateTime::floating(Date::new(-44, 3, 15).unwrap(), Time::MIDNIGHT);
    assert_eq!(
        ok_in(&std_utc(), "{$d :date}", &[("d", Arg::DateTime(&bce))]),
        "-000044-03-15"
    );
    // Milliseconds never show.
    assert_eq!(
        ok("{|2006-01-02T15:04:06.789| :time precision=second}"),
        "15:04:06"
    );
    // Parts: kind `datetime`, direction ltr, no sub-parts.
    let p = parts(&std_utc(), "a {|2006-01-02| :date} b", &[]);
    assert_eq!(p.len(), 1);
    assert_eq!(
        (p[0].kind.as_str(), p[0].dir, p[0].text.as_str()),
        ("datetime", Some(Dir::Ltr), "2006-01-02")
    );
    // An rtl catalog isolates it (ltr in rtl).
    let m = mf2::compile_str("{|2006-01-02| :date}", "ar").unwrap();
    let cx = FormatContext::new(&mf2::host_std::HOST);
    let mut s = String::new();
    Formatter::new(&m.catalog, &REGISTRY, &cx).write(
        mf2::Compiled::ID,
        &[],
        &mut s,
        &mut Vec::new(),
    );
    assert_eq!(s, "\u{2066}2006-01-02\u{2069}");
}

#[test]
fn literal_only_options() {
    // A variable sets a non-override option: Bad Option, ignored.
    for (src, text) in [
        (
            ".local $v = {|month-day|} {{{|2006-01-02| :date fields=$v}}}",
            "2006-01-02",
        ),
        (
            ".local $v = {|long|} {{{|2006-01-02| :date length=$v}}}",
            "2006-01-02",
        ),
        (
            ".local $v = {|second|} {{{|2006-01-02T15:04:06| :time precision=$v}}}",
            "15:04",
        ),
        (
            ".local $v = {|short|} {{{|2006-01-02T15:04:06| :time timeZoneStyle=$v}}}",
            "15:04",
        ),
        (
            ".local $v = {|weekday|} {{{|2006-01-02T15:04:06| :datetime dateFields=$v}}}",
            "2006-01-02 15:04",
        ),
        (
            ".local $v = {|hour|} {{{|2006-01-02T15:04:06| :datetime timePrecision=$v}}}",
            "2006-01-02 15:04",
        ),
    ] {
        assert_eq!(err(src), (text.into(), vec![BadOption]), "{src}");
    }
    let (s, e) = run(
        &std_utc(),
        "{|2006-01-02| :datetime dateLength=$l}",
        &[("l", Arg::Str("long"))],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("2006-01-02 00:00", &[BadOption][..])
    );
    assert_eq!(
        resolved(".local $v = {|long|} {{{|2006-01-02| :date length=$v}}}").date,
        style(DateFields::YearMonthDay, DateLength::Medium)
    );
}

#[test]
fn bad_option_values() {
    for src in [
        "{|2006-01-02T15:04:06| :datetime dateFields=year}",
        "{|2006-01-02T15:04:06| :datetime dateLength=full}",
        "{|2006-01-02T15:04:06| :datetime timePrecision=millisecond}",
        "{|2006-01-02T15:04:06| :datetime timeZoneStyle=generic}",
        "{|2006-01-02T15:04:06| :datetime dateLength=Long}",
        "{|2006-01-02T15:04:06| :datetime hour12=yes}",
        "{|2006-01-02T15:04:06| :datetime hour12=TRUE}",
        "{|2006-01-02T15:04:06| :datetime calendar=ab}",
        "{|2006-01-02T15:04:06| :datetime calendar=islamic_civil}",
        "{|2006-01-02T15:04:06| :datetime timeZone=|+24:00|}",
        "{|2006-01-02T15:04:06| :datetime timeZone=|1abc|}",
        "{|2006-01-02T15:04:06| :datetime timeZone=|a b|}",
    ] {
        assert_eq!(
            err(src),
            ("2006-01-02 15:04".into(), vec![BadOption]),
            "{src}"
        );
    }
    assert_eq!(
        err("{|2006-01-02| :date fields=weekdays}"),
        ("2006-01-02".into(), vec![BadOption])
    );
    assert_eq!(
        err("{|2006-01-02T15:04:06| :time precision=minutes}"),
        ("15:04".into(), vec![BadOption])
    );
    // Two bad options, two errors; a good one still applies.
    assert_eq!(
        err("{|2006-01-02T15:04:06| :datetime dateLength=x timePrecision=second hour12=x}"),
        ("2006-01-02 15:04:06".into(), vec![BadOption, BadOption])
    );
}

#[test]
fn unknown_options_are_ignored() {
    // Options another date/time function takes, and others.
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :date hour12=true}"),
        "2006-01-02"
    );
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :date precision=second timePrecision=second}"),
        "2006-01-02"
    );
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :date timeZoneStyle=long dateLength=long dateFields=weekday}"),
        "2006-01-02"
    );
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :time dateFields=weekday fields=weekday length=long}"),
        "15:04"
    );
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :datetime fields=weekday precision=hour length=long}"),
        "2006-01-02 15:04"
    );
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :datetime foo=bar ns:opt=1 year=numeric}"),
        "2006-01-02 15:04"
    );
    // An unknown option's bad value is no error either.
    assert_eq!(ok("{|2006-01-02| :date hour12=maybe}"), "2006-01-02");
    assert_eq!(resolved("{|2006-01-02| :date hour12=true}").hour12, None);
}

#[test]
fn override_options_from_variables() {
    let cx = zones_utc();
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06| :time hour12=$h}",
            &[("h", Arg::Str("true"))]
        ),
        "03:04 PM"
    );
    assert_eq!(
        ok_in(
            &cx,
            ".local $h = {|true|} {{{|2006-01-02T15:04:06| :time hour12=$h}}}",
            &[]
        ),
        "03:04 PM"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06Z| :time timeZone=$z}",
            &[("z", Arg::Str("+05:30"))]
        ),
        "20:34"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06Z| :time timeZone=$z}",
            &[("z", Arg::Str("Asia/Kolkata"))]
        ),
        "20:34"
    );
    assert_eq!(
        resolved(".local $c = {|buddhist|} {{{|2006-01-02| :date calendar=$c}}}")
            .calendar
            .as_deref(),
        Some("buddhist")
    );
    // An application value with a string form.
    struct Text(&'static str);
    impl CustomValue for Text {
        fn as_str(&self) -> Option<&str> {
            Some(self.0)
        }
    }
    let t = Text("+01:00");
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06Z| :time timeZone=$z}",
            &[("z", Arg::Custom(&t))]
        ),
        "16:04"
    );
    // Not a string, or not a valid value: Bad Option, ignored.
    let (s, e) = run(
        &cx,
        "{|2006-01-02T15:04:06| :time hour12=$h}",
        &[("h", Arg::Int(1))],
    );
    assert_eq!((s.as_str(), e.as_slice()), ("15:04", &[BadOption][..]));
    let (s, e) = run(
        &cx,
        "{|2006-01-02T15:04:06| :time timeZone=$z}",
        &[("z", Arg::Str("nowhere at all"))],
    );
    assert_eq!((s.as_str(), e.as_slice()), ("15:04", &[BadOption][..]));
    // An unresolved variable: Unresolved Variable, then the option is dropped
    // with Bad Option (the runtime's).
    let (s, e) = run(&cx, "{|2006-01-02T15:04:06| :time hour12=$h}", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("15:04", &[UnresolvedVariable, BadOption][..])
    );
}

// ─────────────────────────────────────────────────────────── inheritance ──

#[test]
fn inheritance() {
    // A resolved value formats with its own options; a new expression over
    // it starts from its function's defaults (only override options travel).
    let d = ".local $d = {|2006-01-02| :date length=long fields=year-month-day-weekday}";
    assert_eq!(
        resolved(&format!("{d} {{{{{{$d}}}}}}")).date,
        style(DateFields::YearMonthDayWeekday, DateLength::Long)
    );
    assert_eq!(ok(&format!("{d} {{{{{{$d}}}}}}")), "2006-01-02 Mon");
    assert_eq!(
        resolved(&format!("{d} {{{{{{$d :date}}}}}}")).date,
        style(DateFields::YearMonthDay, DateLength::Medium)
    );
    assert_eq!(ok(&format!("{d} {{{{{{$d :date}}}}}}")), "2006-01-02");
    let t =
        ".local $t = {|2006-01-02T15:04:06| :datetime timePrecision=second timeZoneStyle=short}";
    assert_eq!(ok(&format!("{t} {{{{{{$t}}}}}}")), "2006-01-02 15:04:06 Z");
    assert_eq!(ok(&format!("{t} {{{{{{$t :time}}}}}}")), "15:04");
    assert_eq!(
        ok(&format!("{t} {{{{{{$t :datetime}}}}}}")),
        "2006-01-02 15:04"
    );
    // Override options are inherited; the expression's own win.
    let h = ".local $d = {|2006-01-02T15:04:06| :datetime hour12=true calendar=japanese}";
    assert_eq!(ok(&format!("{h} {{{{{{$d :time}}}}}}")), "03:04 PM");
    assert_eq!(
        ok(&format!("{h} {{{{{{$d :time hour12=false}}}}}}")),
        "15:04"
    );
    let o = resolved(&format!("{h} {{{{{{$d :date}}}}}}"));
    assert_eq!(
        (o.hour12, o.calendar.as_deref()),
        (Some(true), Some("japanese"))
    );
    let o = resolved(&format!("{h} {{{{{{$d :date calendar=gregory}}}}}}"));
    assert_eq!(o.calendar.as_deref(), Some("gregory"));
    // A :date keeps the hour12 it inherited, for a later :time.
    assert_eq!(
        ok(
            ".local $a = {|2006-01-02T15:04:06| :datetime hour12=true} .local $b = {$a :date} {{{$b :time}}}"
        ),
        "03:04 PM"
    );
    // The time zone: inherited, so not converted again; the expression's own
    // converts from where the value is.
    let z = ".local $d = {|2006-01-02T15:04:06Z| :datetime timeZone=|+05:00|}";
    assert_eq!(ok(&format!("{z} {{{{{{$d}}}}}}")), "2006-01-02 20:04");
    assert_eq!(ok(&format!("{z} {{{{{{$d :time}}}}}}")), "20:04");
    assert_eq!(
        ok(&format!("{z} {{{{{{$d :time timeZone=UTC}}}}}}")),
        "15:04"
    );
    assert_eq!(
        ok(&format!(
            "{z} {{{{{{$d :time timeZone=|-01:00| timeZoneStyle=short}}}}}}"
        )),
        "14:04 -01:00"
    );
    assert_eq!(
        resolved(&format!("{z} {{{{{{$d :date}}}}}}"))
            .time_zone
            .as_deref(),
        Some("+18000s")
    );
    // An argument's override options are inherited too.
    let mut arg = mf2_fn_datetime::parse_literal("2006-01-02T15:04:06Z").unwrap();
    arg.options.hour12 = Some(true);
    arg.options.time_zone = Some(ZoneOption::Offset(HOUR));
    arg.options.date = Some(DateStyle {
        fields: DateFields::Weekday,
        length: DateLength::Long,
    });
    assert_eq!(
        ok_in(&std_utc(), "{$a :datetime}", &[("a", Arg::DateTime(&arg))]),
        "2006-01-02 04:04 PM"
    );
    // A :date value is a :time operand (and the reverse): its whole
    // date/time is kept.
    assert_eq!(
        ok(".local $d = {|2006-01-02T15:04:06| :date} {{{$d :time}}}"),
        "15:04"
    );
    assert_eq!(
        ok(".local $t = {|2006-01-02T15:04:06| :time} {{{$t :date}}}"),
        "2006-01-02"
    );
    assert_eq!(
        ok(".local $d = {|2006-01-02| :date} {{{$d :time}}}"),
        "00:00"
    );
    // Through .input.
    assert_eq!(
        ok_in(
            &std_utc(),
            ".input {$d :date length=long} {{{$d} {$d :time}}}",
            &[("d", Arg::Str("2006-01-02T15:04:06"))]
        ),
        "2006-01-02 15:04"
    );
}

// ──────────────────────────────────────────────────────────────── errors ──

#[test]
fn bad_operands() {
    let cx = std_utc();
    for (src, args, want) in [
        ("{1 :date}", vec![], "{|1|}"),
        ("{|2006-02-30| :date}", vec![], "{|2006-02-30|}"),
        ("{|2006-01-02T15:04| :time}", vec![], "{|2006-01-02T15:04|}"),
        ("{|| :datetime}", vec![], "{||}"),
        ("{$n :date}", vec![("n", Arg::Int(20_060_102))], "{$n}"),
        ("{$n :time}", vec![("n", Arg::Float(1.5))], "{$n}"),
        ("{$n :datetime}", vec![("n", Arg::Decimal("2006"))], "{$n}"),
        ("{$s :datetime}", vec![("s", Arg::Str("tomorrow"))], "{$s}"),
        (".local $n = {1 :number} {{{$n :date}}}", vec![], "{$n}"),
    ] {
        let (s, e) = run(&cx, src, &args);
        assert_eq!(
            (s.as_str(), e.as_slice()),
            (want, &[BadOperand][..]),
            "{src}"
        );
    }
    // A fallback operand: its own error, then Bad Operand.
    let (s, e) = run(&cx, "{$x :date}", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{$x}", &[UnresolvedVariable, BadOperand][..])
    );
    let (s, e) = run(&cx, ".local $d = {horse :date} {{{$d :time}}}", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{$d}", &[BadOperand, BadOperand][..])
    );
    // A bad operand with bad options: the options' errors come first.
    let (s, e) = run(&cx, "{horse :date length=$l}", &[("l", Arg::Str("long"))]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{|horse|}", &[BadOption, BadOperand][..])
    );
    // Operands: an application value's date/time or string form.
    struct App(DateTime<'static>);
    impl CustomValue for App {
        fn as_date_time(&self) -> Option<DateTime<'_>> {
            Some(self.0)
        }
    }
    let app = App(DateTime::from_epoch_ms(1_136_214_246_000).unwrap());
    assert_eq!(
        ok_in(&cx, "{$a :datetime}", &[("a", Arg::Custom(&app))]),
        "2006-01-02 15:04"
    );
    struct Text;
    impl CustomValue for Text {
        fn as_str(&self) -> Option<&str> {
            Some("2006-01-02T15:04:06+02:00")
        }
    }
    assert_eq!(
        ok_in(&cx, "{$a :time}", &[("a", Arg::Custom(&Text))]),
        "13:04"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{$a :time}",
            &[("a", Arg::Str("2006-01-02T15:04:06-02:00"))]
        ),
        "17:04"
    );
    // `:string` of a date/time is a Bad Operand (no string form); a
    // `:string` of a literal is a date operand.
    assert_eq!(
        ok(".local $s = {|2006-01-02| :string} {{{$s :date}}}"),
        "2006-01-02"
    );
}

#[test]
fn not_selectable() {
    let (s, e) = err(".local $d = {|2006-01-02| :date} .match $d |2006-01-02| {{a}} * {{b}}");
    assert_eq!((s.as_str(), e.as_slice()), ("b", &[BadSelector][..]));
    let (s, e) = err(".input {$d :datetime} .match $d * {{any}}");
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("any", &[UnresolvedVariable, BadOperand, BadSelector][..])
    );
    let dt = mf2_fn_datetime::parse_literal("2006-01-02").unwrap();
    let (s, e) = run(
        &std_utc(),
        ".input {$d :date} .match $d |2006-01-02| {{a}} * {{b}}",
        &[("d", Arg::DateTime(&dt))],
    );
    assert_eq!((s.as_str(), e.as_slice()), ("b", &[BadSelector][..]));
}

// ─────────────────────────────────────────────────────────────── time zones ──

#[test]
fn time_zone_input() {
    let cx = zones_utc();
    // A floating operand: Bad Operand, the context's zone.
    let (s, e) = run(
        &cx,
        "{|2006-01-02T15:04:06| :time timeZone=input timeZoneStyle=short}",
        &[],
    );
    assert_eq!((s.as_str(), e.as_slice()), ("15:04 Z", &[BadOperand][..]));
    let paris = context(&ZONES, TimeZone::named("Europe/Paris").unwrap());
    let (s, e) = run(
        &paris,
        "{|2006-01-02T15:04:06| :time timeZone=input timeZoneStyle=long}",
        &[],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("15:04 Europe/Paris", &[BadOperand][..])
    );
    // Reported once: a later expression inheriting it is in the context's zone.
    let (s, e) = run(
        &cx,
        ".local $a = {|2006-01-02T15:04:06| :datetime timeZone=input} {{{$a} {$a :time timeZone=input}}}",
        &[],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("2006-01-02 15:04 15:04", &[BadOperand][..])
    );
    // An operand with an offset or a zone keeps it.
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06+01:00| :time timeZone=input timeZoneStyle=short}",
            &[]
        ),
        "15:04 +01:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06Z| :time timeZone=input timeZoneStyle=long}",
            &[]
        ),
        "15:04 UTC"
    );
    let ny = DateTime::from_epoch_ms(1_136_214_246_000)
        .unwrap()
        .with_offset(-5 * HOUR)
        .unwrap()
        .in_zone("America/New_York");
    // (the wall time of an argument is as given: here 15:04 at −05:00)
    assert_eq!(
        ok_in(
            &cx,
            "{$d :time timeZone=input timeZoneStyle=long}",
            &[("d", Arg::DateTime(&ny))]
        ),
        "15:04 America/New_York"
    );
    // `input` over a value a function moved: where it is now.
    assert_eq!(
        ok_in(
            &cx,
            ".local $d = {|2006-01-02T15:04:06Z| :datetime timeZone=|Asia/Kolkata|} {{{$d :time timeZone=input timeZoneStyle=long}}}",
            &[]
        ),
        "20:34 Asia/Kolkata"
    );
    assert_eq!(
        ok_in(
            &paris,
            ".local $d = {|2006-01-02T15:04:06Z| :datetime} {{{$d :time timeZone=input timeZoneStyle=long}}}",
            &[]
        ),
        "16:04 Europe/Paris"
    );
}

#[test]
fn conversions() {
    let cx = zones_utc();
    // Offsets to UTC (the context's zone) and to offsets.
    assert_eq!(
        ok_in(&cx, "{|2006-01-02T15:04:06+01:00| :time}", &[]),
        "14:04"
    );
    assert_eq!(
        ok_in(&cx, "{|2006-01-02T23:30:00-02:00| :datetime}", &[]),
        "2006-01-03 01:30"
    );
    assert_eq!(
        ok_in(&cx, "{|2006-01-02T15:04:06Z| :time timeZone=|+05:30|}", &[]),
        "20:34"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-01T00:30:00Z| :datetime timeZone=|-01:00|}",
            &[]
        ),
        "2005-12-31 23:30"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06+14:00| :time timeZone=UTC timeZoneStyle=long}",
            &[]
        ),
        "01:04 UTC"
    );
    // Floating values take the zone without conversion.
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06| :time timeZone=|+05:30| timeZoneStyle=short}",
            &[]
        ),
        "15:04 +05:30"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06| :time timeZone=UTC timeZoneStyle=short}",
            &[]
        ),
        "15:04 Z"
    );
    // To named zones, through the host.
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-07-01T12:00:00Z| :time timeZone=|Europe/Paris| timeZoneStyle=short}",
            &[]
        ),
        "14:00 +02:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-01T12:00:00Z| :time timeZone=|Europe/Paris| timeZoneStyle=short}",
            &[]
        ),
        "13:00 +01:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-07-01T12:00:00Z| :time timeZone=|Europe/Paris| timeZoneStyle=long}",
            &[]
        ),
        "14:00 Europe/Paris"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2021-03-14T06:59:59Z| :time timeZone=|America/New_York| precision=second}",
            &[]
        ),
        "01:59:59"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2021-03-14T07:00:00Z| :time timeZone=|America/New_York| precision=second}",
            &[]
        ),
        "03:00:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-01-02T15:04:06Z| :time timeZone=|Test/Seconds| timeZoneStyle=short precision=second}",
            &[]
        ),
        "15:13:27 +00:09:21"
    );
    let instant = DateTime::from_epoch_ms(1_136_214_246_000).unwrap();
    assert_eq!(
        ok_in(
            &cx,
            "{$i :datetime timeZone=|America/New_York| timeZoneStyle=short}",
            &[("i", Arg::DateTime(&instant))]
        ),
        "2006-01-02 10:04 -05:00"
    );
    // Named to named, to offsets and back.
    let z = ".local $p = {|2006-07-01T12:00:00Z| :datetime timeZone=|Europe/Paris|}";
    assert_eq!(
        ok_in(
            &cx,
            &format!("{z} {{{{{{$p :time timeZone=|America/New_York|}}}}}}"),
            &[]
        ),
        "08:00"
    );
    assert_eq!(
        ok_in(&cx, &format!("{z} {{{{{{$p :time timeZone=UTC}}}}}}"), &[]),
        "12:00"
    );
    assert_eq!(
        ok_in(&cx, &format!("{z} {{{{{{$p :time}}}}}}"), &[]),
        "14:00"
    );
    let p = parts(&cx, &format!("{z} {{{{{{$p}}}}}}"), &[]);
    let (_, _, offset, zone, o) = p[0].value.clone().unwrap();
    assert_eq!(
        (offset, zone.as_deref(), o.time_zone.as_deref()),
        (Some(2 * HOUR), Some("Europe/Paris"), Some("Europe/Paris"))
    );
    // A floating value placed in a named zone: its offset from the search.
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-07-01T12:00:00| :time timeZone=|Europe/Paris| timeZoneStyle=short}",
            &[]
        ),
        "12:00 +02:00"
    );
    let f = ".local $p = {|2006-07-01T12:00:00| :datetime timeZone=|Europe/Paris|}";
    assert_eq!(
        ok_in(&cx, &format!("{f} {{{{{{$p :time timeZone=UTC}}}}}}"), &[]),
        "10:00"
    );
    // An argument in a named zone without an offset.
    let wall = DateTime::floating(
        Date::new(2006, 7, 1).unwrap(),
        Time::new(12, 0, 0, 0).unwrap(),
    )
    .in_zone("Europe/Paris");
    assert_eq!(
        ok_in(
            &cx,
            "{$w :time timeZone=|America/New_York|}",
            &[("w", Arg::DateTime(&wall))]
        ),
        "06:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{$w :time timeZone=|Europe/Paris| timeZoneStyle=short}",
            &[("w", Arg::DateTime(&wall))]
        ),
        "12:00 +02:00"
    );
}

#[test]
fn gaps_and_overlaps() {
    let cx = zones_utc();
    // Paris, 2006-03-26: 02:00 → 03:00. A wall time in the gap moves forward.
    let gap = "{|2006-03-26T02:30:00| :datetime timeZone=|Europe/Paris| timeZoneStyle=short}";
    assert_eq!(ok_in(&cx, gap, &[]), "2006-03-26 03:30 +02:00");
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-03-26T01:59:59| :time timeZone=|Europe/Paris| timeZoneStyle=short precision=second}",
            &[]
        ),
        "01:59:59 +01:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-03-26T03:00:00| :time timeZone=|Europe/Paris| timeZoneStyle=short}",
            &[]
        ),
        "03:00 +02:00"
    );
    // 2006-10-29: 03:00 → 02:00. A wall time in the overlap is the earlier instant.
    let fold = ".local $p = {|2006-10-29T02:30:00| :datetime timeZone=|Europe/Paris|}";
    assert_eq!(
        ok_in(
            &cx,
            &format!("{fold} {{{{{{$p :time timeZoneStyle=short}}}}}}"),
            &[]
        ),
        "02:30 +02:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            &format!("{fold} {{{{{{$p :time timeZone=UTC}}}}}}"),
            &[]
        ),
        "00:30"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-10-29T03:00:00| :time timeZone=|Europe/Paris| timeZoneStyle=short}",
            &[]
        ),
        "03:00 +01:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-10-29T01:59:00| :time timeZone=|Europe/Paris| timeZoneStyle=short}",
            &[]
        ),
        "01:59 +02:00"
    );
    // Two transitions within a day: the offset in between.
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-06-01T14:00:00| :time timeZone=|Test/Blip| timeZoneStyle=short}",
            &[]
        ),
        "14:00 +02:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2006-06-01T10:30:00| :time timeZone=|Test/Blip| timeZoneStyle=short}",
            &[]
        ),
        "10:30 +01:00"
    );
    // New York, 2021-11-07: 02:00 → 01:00; 2021-03-14: 02:00 → 03:00.
    assert_eq!(
        ok_in(
            &cx,
            "{|2021-11-07T01:30:00| :time timeZone=|America/New_York| timeZoneStyle=short}",
            &[]
        ),
        "01:30 -04:00"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{|2021-03-14T02:30:00| :time timeZone=|America/New_York| timeZoneStyle=short}",
            &[]
        ),
        "03:30 -04:00"
    );
}

#[test]
fn the_context_zone() {
    let instant = DateTime::from_epoch_ms(1_136_214_246_000).unwrap(); // 2006-01-02T15:04:06Z
    let args = [("i", Arg::DateTime(&instant))];
    let paris = context(&ZONES, TimeZone::named("Europe/Paris").unwrap());
    assert_eq!(
        ok_in(&paris, "{$i :time timeZoneStyle=long}", &args),
        "16:04 Europe/Paris"
    );
    assert_eq!(
        ok_in(&paris, "{$i :time timeZoneStyle=short}", &args),
        "16:04 +01:00"
    );
    assert_eq!(ok_in(&paris, "{$i}", &args), "2006-01-02 16:04");
    assert_eq!(ok_in(&paris, "{$i :time timeZone=UTC}", &args), "15:04");
    assert_eq!(
        ok_in(
            &paris,
            ".local $a = {$i :datetime} {{{$a :time timeZone=|Asia/Kolkata|} {$a :time}}}",
            &args
        ),
        "20:34 16:04"
    );
    // A floating value takes the context's zone as it is.
    assert_eq!(
        ok_in(
            &paris,
            "{|2006-01-02T15:04:06| :time timeZoneStyle=short}",
            &[]
        ),
        "15:04 +01:00"
    );
    let plus2 = context(&mf2::host_std::HOST, TimeZone::offset(2 * HOUR).unwrap());
    assert_eq!(
        ok_in(&plus2, "{$i :time timeZoneStyle=long}", &args),
        "17:04 +02:00"
    );
    assert_eq!(
        ok_in(
            &plus2,
            "{|2006-01-02T15:04:06| :time timeZoneStyle=short}",
            &[]
        ),
        "15:04 +02:00"
    );
    assert_eq!(
        ok_in(&plus2, "{$i :time timeZone=UTC timeZoneStyle=long}", &args),
        "15:04 UTC"
    );
    // The plan names the context's zone; the value records `None`.
    let p = parts(&paris, "{$i :time}", &args);
    let (_, _, offset, zone, o) = p[0].value.clone().unwrap();
    assert_eq!((offset, zone, o.time_zone), (Some(HOUR), None, None));
}

#[test]
fn without_zone_data() {
    let instant = DateTime::from_epoch_ms(1_136_214_246_000).unwrap();
    let args = [("i", Arg::DateTime(&instant))];
    let cx = std_utc();
    // Converting an instant to a named zone: Bad Option and a fallback value.
    assert_eq!(
        err("{|2006-01-02T15:04:06Z| :time timeZone=|Europe/Paris|}"),
        ("{|2006-01-02T15:04:06Z|}".into(), vec![BadOption])
    );
    let (s, e) = run(&cx, "{$i :datetime timeZone=|Asia/Kolkata|}", &args);
    assert_eq!((s.as_str(), e.as_slice()), ("{$i}", &[BadOption][..]));
    // A floating value placed in a named zone: no conversion, no error; the
    // offset is unknown, so the zone is named.
    assert_eq!(
        ok("{|2006-01-02T15:04:06| :time timeZone=|Europe/Paris| timeZoneStyle=short}"),
        "15:04 Europe/Paris"
    );
    // … but it cannot be converted onwards.
    let (s, e) = err(
        ".local $p = {|2006-01-02T15:04:06| :time timeZone=|Europe/Paris|} {{{$p} {$p :time timeZone=UTC}}}",
    );
    assert_eq!((s.as_str(), e.as_slice()), ("15:04 {$p}", &[BadOption][..]));
    // Already in the zone: nothing to convert.
    let paris = instant.with_offset(HOUR).unwrap().in_zone("Europe/Paris");
    assert_eq!(
        ok_in(
            &cx,
            "{$p :time timeZone=|Europe/Paris| timeZoneStyle=short}",
            &[("p", Arg::DateTime(&paris))]
        ),
        "15:04 +01:00"
    );
    // The context in a named zone: instants cannot be placed in it …
    let named = context(
        &mf2::host_std::HOST,
        TimeZone::named("Europe/Paris").unwrap(),
    );
    let (s, e) = run(&named, "{$i :time}", &args);
    assert_eq!((s.as_str(), e.as_slice()), ("{$i}", &[BadOption][..]));
    let (s, e) = run(&named, "{$i}", &args);
    assert_eq!((s.as_str(), e.as_slice()), ("{$i}", &[BadOption][..]));
    assert_eq!(ok_in(&named, "{$i :time timeZone=UTC}", &args), "15:04");
    // … floating values can, and stay there.
    let a = ".local $a = {|2006-01-02T15:04:06| :datetime}";
    assert_eq!(
        ok_in(
            &named,
            &format!("{a} {{{{{{$a :time timeZoneStyle=long}}}}}}"),
            &[]
        ),
        "15:04 Europe/Paris"
    );
    assert_eq!(
        ok_in(
            &named,
            &format!("{a} {{{{{{$a :time timeZone=input}}}}}}"),
            &[]
        ),
        "15:04"
    );
    let (s, e) = run(
        &named,
        &format!("{a} {{{{{{$a :time timeZone=UTC}}}}}}"),
        &[],
    );
    assert_eq!((s.as_str(), e.as_slice()), ("{$a}", &[BadOption][..]));
    // Year limits.
    let edge = DateTime::floating(
        Date::new(999_999, 12, 31).unwrap(),
        Time::new(23, 0, 0, 0).unwrap(),
    )
    .with_offset(-2 * HOUR)
    .unwrap();
    let (s, e) = run(&cx, "{$e :datetime}", &[("e", Arg::DateTime(&edge))]);
    assert_eq!((s.as_str(), e.as_slice()), ("{$e}", &[BadOperand][..]));
}

// ──────────────────────────────────────────────────────── unannotated ──

#[test]
fn unannotated() {
    let cx = std_utc();
    let floating = mf2_fn_datetime::parse_literal("2006-01-02T15:04:06").unwrap();
    assert_eq!(
        ok_in(&cx, "{$d}", &[("d", Arg::DateTime(&floating))]),
        "2006-01-02 15:04"
    );
    let plus = mf2_fn_datetime::parse_literal("2006-01-02T15:04:06+01:00").unwrap();
    assert_eq!(
        ok_in(&cx, "{$d}", &[("d", Arg::DateTime(&plus))]),
        "2006-01-02 14:04"
    );
    // The value's own override options apply.
    let mut own = floating;
    own.options.hour12 = Some(true);
    assert_eq!(
        ok_in(&cx, "{$d}", &[("d", Arg::DateTime(&own))]),
        "2006-01-02 03:04 PM"
    );
    assert_eq!(
        ok_in(
            &cx,
            "{$d}",
            &[(
                "d",
                Arg::DateTime(&plus_opts(plus, ZoneOption::Offset(-HOUR)))
            )]
        ),
        "2006-01-02 13:04"
    );
    // … and an error in them makes it a fallback value.
    let mut input = floating;
    input.options.time_zone = Some(ZoneOption::Input);
    let (s, e) = run(&cx, "{$d}", &[("d", Arg::DateTime(&input))]);
    assert_eq!((s.as_str(), e.as_slice()), ("{$d}", &[BadOperand][..]));
    // Its non-override options are ignored.
    let mut styled = floating;
    styled.options.date = Some(DateStyle {
        fields: DateFields::Weekday,
        length: DateLength::Long,
    });
    styled.options.time = Some(TimePrecision::Second);
    assert_eq!(
        ok_in(&cx, "{$d}", &[("d", Arg::DateTime(&styled))]),
        "2006-01-02 15:04"
    );
    // Parts: the value as given, kind `datetime`, ltr.
    let p = parts(&cx, "{$d}", &[("d", Arg::DateTime(&floating))]);
    assert_eq!(
        (p[0].kind.as_str(), p[0].dir, p[0].text.as_str()),
        ("datetime", Some(Dir::Ltr), "2006-01-02 15:04")
    );
    // `:string` has no string form of it.
    let (s, e) = run(&cx, "{$d :string}", &[("d", Arg::DateTime(&floating))]);
    assert_eq!((s.as_str(), e.as_slice()), ("{$d}", &[BadOperand][..]));
    // Without the hook, an unannotated date/time is a Bad Operand.
    let (s, e) = run_in(&BARE, &cx, "{$d}", &[("d", Arg::DateTime(&floating))]);
    assert_eq!((s.as_str(), e.as_slice()), ("{$d}", &[BadOperand][..]));
    // A date string argument is a string, unannotated.
    assert_eq!(
        ok_in(&cx, "{$d}", &[("d", Arg::Str("2006-01-02"))]),
        "2006-01-02"
    );
}

fn plus_opts(mut d: DateTime<'static>, z: ZoneOption<'static>) -> DateTime<'static> {
    d.options.time_zone = Some(z);
    d
}

// ─────────────────────────────────────────────────────────────── backends ──

#[test]
fn another_backend() {
    use mf2::{FnContext, Sink};
    use mf2_fn_datetime::Backend;
    /// Writes the plan's request.
    struct Req;
    impl Backend for Req {
        fn format(&self, _cx: &FnContext<'_>, plan: &Plan<'_>, out: &mut dyn Sink) {
            let r = plan.request();
            out.push_str(&format!("{} {}", r.epoch_ms, zone_text(r.zone)));
        }
        fn dir(&self, _cx: &FnContext<'_>, _plan: &Plan<'_>) -> Dir {
            Dir::Auto
        }
        fn supports(&self, _cx: &FnContext<'_>, plan: &Plan<'_>) -> Result<(), FormatError> {
            match plan.options.calendar {
                Some("gregory") | None => Ok(()),
                Some(_) => Err(FormatError::UnsupportedOperation),
            }
        }
    }
    static D: DateTimeFunction<Req> = DateTimeFunction::datetime(Req);
    static F: [(&str, &dyn Function); 1] = [("datetime", &D)];
    static R: Registry = Registry::new(&F);
    let cx = zones_utc();
    let (s, e) = run_in(
        &R,
        &cx,
        "{|2006-01-02T15:04:06Z| :datetime timeZone=|Europe/Paris|}",
        &[],
    );
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("1136214246000 Europe/Paris", &[][..])
    );
    let (s, _) = run_in(&R, &cx, "{|2006-01-02T15:04:06| :datetime}", &[]);
    assert_eq!(s, "1136214246000 UTC");
    // Without zone data: the wall time as UTC.
    let (s, _) = run_in(
        &R,
        &std_utc(),
        "{|2006-01-02T15:04:06| :datetime timeZone=|Europe/Paris|}",
        &[],
    );
    assert_eq!(s, "1136214246000 UTC");
    let (s, e) = run_in(&R, &cx, "{|2006-01-02| :datetime calendar=hebrew}", &[]);
    assert_eq!(
        (s.as_str(), e.as_slice()),
        ("{|2006-01-02|}", &[FormatError::UnsupportedOperation][..])
    );
    // A handler over the neutral backend, named.
    static N: DateTimeFunction<Neutral> = DateTimeFunction::time(Neutral);
    static NF: [(&str, &dyn Function); 1] = [("t", &N)];
    static NR: Registry = Registry::new(&NF);
    let (s, e) = run_in(&NR, &std_utc(), "{|2006-01-02T15:04:06| :t}", &[]);
    assert_eq!((s.as_str(), e.as_slice()), ("15:04", &[][..]));
}
