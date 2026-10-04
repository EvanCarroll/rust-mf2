//! The options of `:datetime`, `:date` and `:time` (datetime.md), read from
//! an expression: which function takes which option, the literal-only rule,
//! the values each takes. Ported from P0.6 (`mf2dt::resolve`).

use mf2_runtime::{
    DateFields, DateLength, ErrorSink, FormatError, OptionValue, Options, TimePrecision, Value,
    ZoneOption, ZoneStyle, is_zone_name,
};

use crate::literal::{is_uvalue, offset};

/// Which function resolves.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Kind {
    /// `:datetime`.
    DateTime,
    /// `:date`.
    Date,
    /// `:time`.
    Time,
}

impl Kind {
    const fn bit(self) -> u8 {
        match self {
            Kind::DateTime => DT,
            Kind::Date => D,
            Kind::Time => T,
        }
    }
}

const DT: u8 = 1;
const D: u8 = 2;
const T: u8 = 4;

/// An option's meaning.
#[derive(Clone, Copy)]
enum Opt {
    Fields,
    Length,
    Precision,
    ZoneStyle,
    TimeZone,
    Hour12,
    Calendar,
}

/// `(name, the functions that take it, meaning)`. `hour12` is on
/// `:datetime` and `:time` only; `timeZoneStyle` likewise; the date and
/// time options have a name per function. An option a function does not
/// take is unknown to it, so ignored.
const NAMES: [(&str, u8, Opt); 10] = [
    ("dateFields", DT, Opt::Fields),
    ("fields", D, Opt::Fields),
    ("dateLength", DT, Opt::Length),
    ("length", D, Opt::Length),
    ("timePrecision", DT, Opt::Precision),
    ("precision", T, Opt::Precision),
    ("timeZoneStyle", DT | T, Opt::ZoneStyle),
    ("timeZone", DT | D | T, Opt::TimeZone),
    ("hour12", DT | T, Opt::Hour12),
    ("calendar", DT | D | T, Opt::Calendar),
];

const FIELDS: [(&str, DateFields); 6] = [
    ("weekday", DateFields::Weekday),
    ("day-weekday", DateFields::DayWeekday),
    ("month-day", DateFields::MonthDay),
    ("month-day-weekday", DateFields::MonthDayWeekday),
    ("year-month-day", DateFields::YearMonthDay),
    ("year-month-day-weekday", DateFields::YearMonthDayWeekday),
];

const LENGTHS: [(&str, DateLength); 3] = [
    ("long", DateLength::Long),
    ("medium", DateLength::Medium),
    ("short", DateLength::Short),
];

const PRECISIONS: [(&str, TimePrecision); 3] = [
    ("hour", TimePrecision::Hour),
    ("minute", TimePrecision::Minute),
    ("second", TimePrecision::Second),
];

const ZONE_STYLES: [(&str, ZoneStyle); 2] =
    [("long", ZoneStyle::Long), ("short", ZoneStyle::Short)];

const BOOLS: [(&str, bool); 2] = [("true", true), ("false", false)];

fn lookup<V: Copy>(table: &[(&str, V)], s: &str) -> Option<V> {
    table.iter().find(|(k, _)| *k == s).map(|&(_, v)| v)
}

/// An option value's text, with the value's lifetime: a string (a literal,
/// a string argument, `:string`'s value) or an application value's
/// `as_str`.
fn text<'a>(v: &Value<'a>) -> Option<&'a str> {
    match v {
        Value::Str(s) => Some(s),
        Value::Custom(c) => c.as_str(),
        _ => None,
    }
}

/// A `timeZone` value: `input`, `UTC`, an RFC 3339 `time-numoffset`, or an
/// RFC 9557 `time-zone-name` (case-sensitive, as both RFCs spell them).
pub(crate) fn time_zone(s: &str) -> Option<ZoneOption<'_>> {
    match s {
        "input" => Some(ZoneOption::Input),
        "UTC" => Some(ZoneOption::Utc),
        _ => match offset(s.as_bytes()) {
            Some(o) => Some(ZoneOption::Offset(o)),
            None => is_zone_name(s).then_some(ZoneOption::Named(s)),
        },
    }
}

/// What an expression's own options set; `None` = not set (or ignored).
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Own<'a> {
    pub(crate) fields: Option<DateFields>,
    pub(crate) length: Option<DateLength>,
    pub(crate) precision: Option<TimePrecision>,
    pub(crate) zone_style: Option<ZoneStyle>,
    pub(crate) time_zone: Option<ZoneOption<'a>>,
    pub(crate) hour12: Option<bool>,
    pub(crate) calendar: Option<&'a str>,
}

/// Reads `options` for `kind`: an option the function does not take is
/// ignored; one whose value is not valid, or — except for the override
/// options `timeZone`, `hour12`, `calendar` — was not set by a literal, is a
/// *Bad Option* and ignored.
pub(crate) fn read<'a>(kind: Kind, options: Options<'_, 'a>, errs: &mut dyn ErrorSink) -> Own<'a> {
    let mut own = Own::default();
    for (name, value) in options.iter() {
        let Some(opt) = NAMES
            .iter()
            .find(|(n, mask, _)| *n == name && mask & kind.bit() != 0)
            .map(|&(_, _, o)| o)
        else {
            continue;
        };
        if !set(&mut own, opt, value) {
            errs.error(FormatError::BadOption);
        }
    }
    own
}

/// The kind of the function `name` (`datetime`, `date`, `time`).
pub(crate) fn kind(name: &str) -> Option<Kind> {
    match name {
        "datetime" => Some(Kind::DateTime),
        "date" => Some(Kind::Date),
        "time" => Some(Kind::Time),
        _ => None,
    }
}

/// What `kind`'s literal options set: `literal(name)` is the literal value
/// of the option `name`, `None` when the expression has none (or sets it by
/// a variable). An invalid value is ignored, as at run time.
pub(crate) fn read_literals<'s>(kind: Kind, literal: &dyn Fn(&str) -> Option<&'s str>) -> Own<'s> {
    let mut own = Own::default();
    for &(name, mask, opt) in &NAMES {
        if mask & kind.bit() == 0 {
            continue;
        }
        if let Some(s) = literal(name) {
            let _ = set_text(&mut own, opt, s);
        }
    }
    own
}

/// Sets `opt` from `value`; `false`: a Bad Option.
fn set<'a>(own: &mut Own<'a>, opt: Opt, value: OptionValue<'_, 'a>) -> bool {
    let override_option = matches!(opt, Opt::TimeZone | Opt::Hour12 | Opt::Calendar);
    let Some(s) = text(value.value) else {
        return false;
    };
    if !value.literal && !override_option {
        return false;
    }
    set_text(own, opt, s)
}

/// Sets `opt` from its text `s`; `false`: not a value it takes.
fn set_text<'a>(own: &mut Own<'a>, opt: Opt, s: &'a str) -> bool {
    let ok = match opt {
        Opt::Fields => lookup(&FIELDS, s).map(|v| own.fields = Some(v)),
        Opt::Length => lookup(&LENGTHS, s).map(|v| own.length = Some(v)),
        Opt::Precision => lookup(&PRECISIONS, s).map(|v| own.precision = Some(v)),
        Opt::ZoneStyle => lookup(&ZONE_STYLES, s).map(|v| own.zone_style = Some(v)),
        Opt::TimeZone => time_zone(s).map(|v| own.time_zone = Some(v)),
        Opt::Hour12 => lookup(&BOOLS, s).map(|v| own.hour12 = Some(v)),
        Opt::Calendar => is_uvalue(s).then(|| own.calendar = Some(s)),
    };
    ok.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_values() {
        assert_eq!(time_zone("input"), Some(ZoneOption::Input));
        assert_eq!(time_zone("UTC"), Some(ZoneOption::Utc));
        assert_eq!(time_zone("+05:30"), Some(ZoneOption::Offset(19_800)));
        assert_eq!(time_zone("-23:59"), Some(ZoneOption::Offset(-86_340)));
        assert_eq!(
            time_zone("America/New_York"),
            Some(ZoneOption::Named("America/New_York"))
        );
        // Well-formed names that are not IANA ids are still names: the
        // host decides whether it knows them.
        assert_eq!(time_zone("utc"), Some(ZoneOption::Named("utc")));
        assert_eq!(time_zone("Input"), Some(ZoneOption::Named("Input")));
        // `Z` is a well-formed name (it starts with a letter), not UTC.
        assert_eq!(time_zone("Z"), Some(ZoneOption::Named("Z")));
        for bad in ["", "+24:00", "+5:30", "05:30", "Europe/", "1abc", "a b"] {
            assert_eq!(time_zone(bad), None, "{bad:?}");
        }
    }
}
