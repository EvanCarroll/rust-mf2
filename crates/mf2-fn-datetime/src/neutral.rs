//! The neutral stub backend: deterministic, locale-independent text in
//! ISO 8601 pieces, for tests and for a build with no backend feature —
//! the date counterpart of the core's neutral digits.

use mf2_runtime::{DateFields, FnContext, Sink, TimePrecision, ZoneOption, ZoneStyle};

use crate::plan::{Backend, Plan};

/// The neutral backend. Its text is the same in every locale: the pieces
/// the options ask for, in the order date, weekday, time, zone, separated
/// by one space —
///
/// | Option | Piece |
/// |---|---|
/// | `year-month-day` | `2006-01-02` (a year outside 0–9999: `+010000`, `-000044`) |
/// | `month-day` | `--01-02` |
/// | `day-weekday` | `---02` |
/// | `weekday`, `…-weekday` | `Mon` … `Sun` |
/// | `hour` / `minute` / `second` | `15` / `15:04` / `15:04:06` |
/// | `hour12=true` | `03 PM` / `03:04 PM` / `03:04:06 PM` (`12 AM` at midnight) |
/// | `timeZoneStyle=short` | `Z` or `+05:30` (`+hh:mm:ss` if it has seconds); the zone's name if its offset is not known |
/// | `timeZoneStyle=long` | `UTC`, `+05:30`, or the zone's name (`Europe/Paris`) |
///
/// So `{|2006-01-02T15:04:06| :datetime}` is `2006-01-02 15:04`. The
/// length (`dateLength`, `length`) and `calendar` do not change it: it is
/// always the ISO (proleptic Gregorian) calendar.
#[derive(Clone, Copy, Default, Debug)]
pub struct Neutral;

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Writes `n` with at least `width` digits (`n` < 10⁶, `width` ≤ 6).
/// Constant divisors only, so no division-by-zero path.
fn digits(out: &mut dyn Sink, n: u32, width: usize) {
    const N: usize = 6;
    let mut buf = [0u8; N];
    let mut len = 0;
    let mut rest = n;
    while len < N {
        if let Some(b) = buf.get_mut(N - 1 - len) {
            // `rest % 10` < 10.
            #[allow(clippy::cast_possible_truncation)]
            let d = (rest % 10) as u8;
            *b = d;
        }
        rest /= 10;
        len += 1;
        if rest == 0 && len >= width {
            break;
        }
    }
    for &d in buf.get(N - len..).unwrap_or(&[]) {
        out.push_str(DIGITS.get(usize::from(d)).copied().unwrap_or("0"));
    }
}

/// Writes a UTC offset: `±hh:mm`, `:ss` when it has seconds.
fn offset(out: &mut dyn Sink, seconds: i32) {
    out.push_str(if seconds < 0 { "-" } else { "+" });
    let s = seconds.unsigned_abs();
    digits(out, s / 3600, 2);
    out.push_str(":");
    digits(out, s / 60 % 60, 2);
    if !s.is_multiple_of(60) {
        out.push_str(":");
        digits(out, s % 60, 2);
    }
}

impl Backend for Neutral {
    fn format(&self, _cx: &FnContext<'_>, plan: &Plan<'_>, out: &mut dyn Sink) {
        let o = plan.options;
        let mut first = true;
        let mut space = |out: &mut dyn Sink| {
            if !first {
                out.push_str(" ");
            }
            first = false;
        };
        if let Some(style) = o.date {
            let d = plan.date;
            let (year, month, weekday) = match style.fields {
                DateFields::YearMonthDayWeekday => (true, true, true),
                DateFields::MonthDay => (false, true, false),
                DateFields::MonthDayWeekday => (false, true, true),
                DateFields::DayWeekday | DateFields::Weekday => (false, false, true),
                // `YearMonthDay`, and (not exhaustive) a value this stub
                // does not know: the whole date.
                _ => (true, true, false),
            };
            if style.fields != DateFields::Weekday {
                space(out);
                if year {
                    let y = d.year();
                    if (0..=9999).contains(&y) {
                        digits(out, y.unsigned_abs(), 4);
                    } else {
                        out.push_str(if y < 0 { "-" } else { "+" });
                        digits(out, y.unsigned_abs(), 6);
                    }
                } else {
                    out.push_str("-");
                }
                out.push_str("-");
                if month {
                    digits(out, u32::from(d.month()), 2);
                }
                out.push_str("-");
                digits(out, u32::from(d.day()), 2);
            }
            if weekday {
                space(out);
                let w = usize::from(d.weekday().saturating_sub(1));
                out.push_str(WEEKDAYS.get(w).copied().unwrap_or(""));
            }
        }
        if let Some(precision) = o.time {
            space(out);
            let h = plan.time.hour();
            let twelve = o.hour12 == Some(true);
            let shown = match (twelve, h % 12) {
                (true, 0) => 12,
                (true, h12) => h12,
                (false, _) => h,
            };
            digits(out, u32::from(shown), 2);
            if precision != TimePrecision::Hour {
                out.push_str(":");
                digits(out, u32::from(plan.time.minute()), 2);
            }
            if precision == TimePrecision::Second {
                out.push_str(":");
                digits(out, u32::from(plan.time.second()), 2);
            }
            if twelve {
                out.push_str(if h < 12 { " AM" } else { " PM" });
            }
        }
        if let Some(style) = o.time_zone_style {
            space(out);
            match (style, plan.zone, plan.offset) {
                (ZoneStyle::Short, _, Some(0)) => out.push_str("Z"),
                (ZoneStyle::Short, _, Some(s)) | (ZoneStyle::Long, ZoneOption::Offset(s), _) => {
                    offset(out, s);
                }
                (_, ZoneOption::Named(n), _) => out.push_str(n),
                (ZoneStyle::Short, ZoneOption::Offset(s), None) => offset(out, s),
                // UTC and `input`, and a kind this stub does not know.
                _ => out.push_str("UTC"),
            }
        }
    }
}
