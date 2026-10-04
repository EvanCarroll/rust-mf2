//! Dates through the browser:
//! `Host::format_date_time` with `Intl.DateTimeFormat` (`datetime-intl`)
//! and `Host::zone_offset` from the browser's own zone data (`time-zones`).
//! One inline-JS module: the formatters are cached in it, one per locale
//! and option set; the options travel as a JSON string built here, in a
//! fixed buffer (no allocation, no `core::fmt`).
//!
//! The MF2 → ECMA-402 mapping (P0.6): when the plan is expressible with
//! styles — the date `year-month-day` or none, the time to the minute or
//! second or none, no zone style, not a time alone with `hour12` —
//! `dateStyle` = the length and `timeStyle` = `short` (minute) / `medium`
//! (second), CLDR's standard formats, which the semantic skeletons resolve
//! to as well; otherwise components (ECMA-402 does
//! not mix the two, and `timeZoneName` is a component): `year` numeric
//! (2-digit when short), `month` long / short / numeric by length, `day`
//! numeric, `weekday` long (long) / short, `hour` numeric, `minute` and
//! `second` 2-digit, `timeZoneName` long / short. `hour12`, `calendar` and
//! the zone (`UTC`, `±hh:mm`, or the IANA name) pass through.

#[cfg(feature = "datetime-intl")]
use mf2_runtime::{
    DateFields, DateLength, DateStyle, DateTimeRequest, Sink, TimePrecision, ZoneOption, ZoneStyle,
};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(inline_js = r#"
const formatters = new Map();
const zones = new Map();
export function mf2_dtf(locale, options, t) {
  const key = locale + '\u0000' + options;
  let f = formatters.get(key);
  if (f === undefined) {
    try { f = new Intl.DateTimeFormat(locale, JSON.parse(options)); } catch (e) { f = null; }
    formatters.set(key, f);
  }
  if (f === null) return undefined;
  try { return f.format(t); } catch (e) { return undefined; }
}
export function mf2_zone_offset(zone, t) {
  let f = zones.get(zone);
  if (f === undefined) {
    try {
      f = new Intl.DateTimeFormat('en-US', { timeZone: zone, timeZoneName: 'longOffset' });
    } catch (e) { f = null; }
    zones.set(zone, f);
  }
  if (f === null) return undefined;
  let name;
  try {
    name = f.formatToParts(t).find((p) => p.type === 'timeZoneName').value;
  } catch (e) { return undefined; }
  const m = /^GMT(?:([+\u2212-])(\d{1,2}):(\d{2})(?::(\d{2}))?)?$/.exec(name);
  if (m === null) return undefined;
  if (m[1] === undefined) return 0;
  const s = Number(m[2]) * 3600 + Number(m[3]) * 60 + Number(m[4] ?? 0);
  return m[1] === '+' ? s : -s;
}
"#)]
extern "C" {
    #[cfg(feature = "datetime-intl")]
    fn mf2_dtf(locale: &str, options: &str, t: f64) -> Option<alloc::string::String>;
    fn mf2_zone_offset(zone: &str, t: f64) -> Option<i32>;
}

/// Milliseconds as a JS number (exact below 2⁵³; `Intl`'s own range,
/// ±8.64 × 10¹⁵, is smaller, and beyond it the JS side answers `undefined`).
#[allow(clippy::cast_precision_loss)]
fn js_ms(ms: i64) -> f64 {
    ms as f64
}

/// The offset of `zone` at `epoch_ms` from the browser's data: its
/// `longOffset` name (`GMT`, `GMT+05:30`) through `Intl.DateTimeFormat`;
/// `None` for a zone the browser does not know.
pub(crate) fn zone_offset(zone: &str, epoch_ms: i64) -> Option<i32> {
    mf2_zone_offset(zone, js_ms(epoch_ms))
}

/// A JSON object written into a fixed buffer; too long → `None`.
#[cfg(feature = "datetime-intl")]
struct Json {
    buf: [u8; 512],
    len: usize,
    ok: bool,
    fields: u8,
}

#[cfg(feature = "datetime-intl")]
impl Json {
    fn new() -> Json {
        Json {
            buf: [0; 512],
            len: 0,
            ok: true,
            fields: 0,
        }
    }

    fn raw(&mut self, s: &str) {
        for &b in s.as_bytes() {
            let Some(slot) = self.buf.get_mut(self.len) else {
                self.ok = false;
                return;
            };
            *slot = b;
            self.len += 1;
        }
    }

    fn key(&mut self, k: &str) {
        self.raw(if self.fields == 0 { "{\"" } else { ",\"" });
        self.fields = self.fields.saturating_add(1);
        self.raw(k);
        self.raw("\":");
    }

    /// `"k":"v"` — `v` needs no escaping (option words, IANA names, offsets
    /// and `uvalue`s are all ASCII letters, digits, `/`, `_`, `-`, `+`, `:`).
    fn text(&mut self, k: &str, v: &str) {
        self.key(k);
        self.raw("\"");
        self.raw(v);
        self.raw("\"");
    }

    fn finish(&mut self) -> Option<&str> {
        self.raw(if self.fields == 0 { "{}" } else { "}" });
        if !self.ok {
            return None;
        }
        core::str::from_utf8(self.buf.get(..self.len)?).ok()
    }
}

/// `±hh:mm` of an offset in seconds.
#[cfg(feature = "datetime-intl")]
fn offset_id(seconds: i32, out: &mut [u8; 6]) -> &str {
    const D: &[u8; 10] = b"0123456789";
    let a = seconds.unsigned_abs() / 60;
    let (h, m) = (a / 60 % 100, a % 60);
    let digit = |n: u32| D.get(n as usize % 10).copied().unwrap_or(b'0');
    *out = [
        if seconds < 0 { b'-' } else { b'+' },
        digit(h / 10),
        digit(h),
        b':',
        digit(m / 10),
        digit(m),
    ];
    core::str::from_utf8(out).unwrap_or("+00:00")
}

/// The `Intl.DateTimeFormat` options of `r` (see the module docs); `false`
/// for an option value this version does not know (the runtime's option
/// enums are not exhaustive), which the browser is then not asked to guess.
#[cfg(feature = "datetime-intl")]
fn options(r: &DateTimeRequest<'_>, j: &mut Json) -> bool {
    let o = r.options;
    // A time alone with `hour12` takes components: V8 and JavaScriptCore
    // apply `hour12` to a 24-hour locale's `timeStyle` pattern by swapping
    // its hour field and keeping two digits (`de`: 03:04 PM), where CLDR's
    // 12-hour skeleton — ICU4X's, and Firefox's — gives 3:04 PM, as the
    // components do in all three engines.
    let styleable = matches!(
        o.date,
        None | Some(DateStyle {
            fields: DateFields::YearMonthDay,
            ..
        })
    ) && matches!(
        o.time,
        None | Some(TimePrecision::Minute | TimePrecision::Second)
    ) && o.time_zone_style.is_none()
        && !(o.date.is_none() && o.hour12.is_some());
    let length = o.date.map_or(DateLength::Medium, |d| d.length);
    let len = match length {
        DateLength::Long => "long",
        DateLength::Medium => "medium",
        DateLength::Short => "short",
        _ => return false,
    };
    if styleable {
        if o.date.is_some() {
            j.text("dateStyle", len);
        }
        match o.time {
            Some(TimePrecision::Minute) => j.text("timeStyle", "short"),
            Some(TimePrecision::Second) => j.text("timeStyle", "medium"),
            _ => {}
        }
    } else {
        if let Some(d) = o.date {
            let (year, month, day, weekday) = match d.fields {
                DateFields::Weekday => (false, false, false, true),
                DateFields::DayWeekday => (false, false, true, true),
                DateFields::MonthDay => (false, true, true, false),
                DateFields::MonthDayWeekday => (false, true, true, true),
                DateFields::YearMonthDay => (true, true, true, false),
                DateFields::YearMonthDayWeekday => (true, true, true, true),
                _ => return false,
            };
            if year {
                let v = if length == DateLength::Short {
                    "2-digit"
                } else {
                    "numeric"
                };
                j.text("year", v);
            }
            if month {
                let v = match length {
                    DateLength::Long => "long",
                    DateLength::Medium => "short",
                    DateLength::Short => "numeric",
                    _ => return false,
                };
                j.text("month", v);
            }
            if day {
                j.text("day", "numeric");
            }
            if weekday {
                let v = if length == DateLength::Long {
                    "long"
                } else {
                    "short"
                };
                j.text("weekday", v);
            }
        }
        if let Some(p) = o.time {
            j.text("hour", "numeric");
            if p != TimePrecision::Hour {
                j.text("minute", "2-digit");
            }
            if p == TimePrecision::Second {
                j.text("second", "2-digit");
            }
        }
        if let Some(z) = o.time_zone_style {
            let v = match z {
                ZoneStyle::Long => "long",
                ZoneStyle::Short => "short",
                _ => return false,
            };
            j.text("timeZoneName", v);
        }
    }
    if let Some(h12) = o.hour12 {
        j.key("hour12");
        j.raw(if h12 { "true" } else { "false" });
    }
    if let Some(cal) = o.calendar {
        j.text("calendar", cal);
    }
    let mut id = [0u8; 6];
    let zone = match r.zone {
        ZoneOption::Offset(s) => offset_id(s, &mut id),
        ZoneOption::Named(n) => n,
        ZoneOption::Utc | ZoneOption::Input => "UTC",
        _ => return false,
    };
    j.text("timeZone", zone);
    true
}

/// Formats `r` for `locale` with `Intl.DateTimeFormat`; `false` when the
/// browser cannot (an option or zone it rejects, an instant out of range).
#[cfg(feature = "datetime-intl")]
pub(crate) fn format(locale: &str, r: &DateTimeRequest<'_>, out: &mut dyn Sink) -> bool {
    let mut j = Json::new();
    if !options(r, &mut j) {
        return false;
    }
    let Some(options) = j.finish() else {
        return false;
    };
    match mf2_dtf(locale, options, js_ms(r.epoch_ms)) {
        Some(text) => {
            out.push_str(&text);
            true
        }
        None => false,
    }
}
