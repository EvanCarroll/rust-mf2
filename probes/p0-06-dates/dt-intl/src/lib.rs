//! P0.6 probe — the `datetime-intl` backend: an [`mf2dt::Plan`] and a resolved
//! value become text through the host's ECMA-402 `Intl.DateTimeFormat`.
//!
//! Option mapping MF2 → ECMA-402:
//! * `dateFields=year-month-day` (or no date) with `timePrecision` minute/second
//!   (or no time) and no `timeZoneStyle` → `dateStyle` = `dateLength`,
//!   `timeStyle` = `short` (minute) / `medium` (second). These are CLDR's
//!   standard date/time formats, which is what the semantic skeletons resolve
//!   to as well.
//! * otherwise component options (ECMA-402 forbids mixing styles with
//!   components, and `timeZoneName` is a component): `year` numeric (2-digit
//!   when short), `month` long/short/numeric by length, `day` numeric,
//!   `weekday` long (long) / short, `hour` numeric, `minute`/`second` 2-digit,
//!   `timeZoneName` long/short.
//! * `hour12` → `hour12`; `calendar` → `calendar`; `timeZone` → `timeZone`
//!   (`UTC`, `±hh:mm`, or the IANA id; the host converts instants).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use js_sys::{Array, Date, Object, Reflect};
use mf2dt::{DateFields, Error, Length, Plan, Precision, ZoneStyle, Zoned};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
extern "C" {
    /// `new Intl.DateTimeFormat(locales, options)`, with the `RangeError`
    /// (unknown zone / calendar) caught instead of crossing the wasm frames.
    #[wasm_bindgen(js_namespace = Intl, js_name = DateTimeFormat)]
    type Dtf;
    #[wasm_bindgen(constructor, catch, js_namespace = Intl, js_class = DateTimeFormat)]
    fn new_dtf(locales: &Array, options: &Object) -> Result<Dtf, JsValue>;
    /// `dtf.format(date)` — the bound `format` getter, called directly.
    #[wasm_bindgen(method, js_class = DateTimeFormat, js_name = format)]
    fn format_date(this: &Dtf, date: &Date) -> String;
}

fn set(o: &Object, k: &str, v: &JsValue) {
    // Reflect.set on a plain object cannot throw.
    let _ = Reflect::set(o, &JsValue::from_str(k), v);
}

fn s(v: &str) -> JsValue {
    JsValue::from_str(v)
}

/// `±hh:mm` for a fixed offset, written by hand (fmt-free).
fn offset_id(mins: i32) -> String {
    let a = mins.unsigned_abs();
    let (h, m) = (a / 60 % 100, a % 60);
    let d = |n: u32| char::from_digit(n % 10, 10).unwrap_or('0');
    let mut s = String::with_capacity(6);
    s.push(if mins < 0 { '-' } else { '+' });
    for c in [d(h / 10), d(h), ':', d(m / 10), d(m)] {
        s.push(c);
    }
    s
}

/// Build the `Intl.DateTimeFormat` options for `plan`.
pub fn options(plan: &Plan<'_>) -> Object {
    let o = Object::new();
    let date_styleable = matches!(plan.date, None | Some(DateFields::YearMonthDay));
    let time_styleable = matches!(plan.time, None | Some(Precision::Minute | Precision::Second));
    let len = match plan.length {
        Length::Long => "long",
        Length::Medium => "medium",
        Length::Short => "short",
    };
    if date_styleable && time_styleable && plan.zone.is_none() {
        if plan.date.is_some() {
            set(&o, "dateStyle", &s(len));
        }
        match plan.time {
            Some(Precision::Minute) => set(&o, "timeStyle", &s("short")),
            Some(Precision::Second) => set(&o, "timeStyle", &s("medium")),
            _ => {}
        }
    } else {
        if let Some(d) = plan.date {
            let (y, m, day, wd) = match d {
                DateFields::Weekday => (false, false, false, true),
                DateFields::DayWeekday => (false, false, true, true),
                DateFields::MonthDay => (false, true, true, false),
                DateFields::MonthDayWeekday => (false, true, true, true),
                DateFields::YearMonthDay => (true, true, true, false),
                DateFields::YearMonthDayWeekday => (true, true, true, true),
            };
            if y {
                let v = if plan.length == Length::Short { "2-digit" } else { "numeric" };
                set(&o, "year", &s(v));
            }
            if m {
                let v = match plan.length {
                    Length::Long => "long",
                    Length::Medium => "short",
                    Length::Short => "numeric",
                };
                set(&o, "month", &s(v));
            }
            if day {
                set(&o, "day", &s("numeric"));
            }
            if wd {
                let v = if plan.length == Length::Long { "long" } else { "short" };
                set(&o, "weekday", &s(v));
            }
        }
        if let Some(p) = plan.time {
            set(&o, "hour", &s("numeric"));
            if p != Precision::Hour {
                set(&o, "minute", &s("2-digit"));
            }
            if p == Precision::Second {
                set(&o, "second", &s("2-digit"));
            }
        }
        if let Some(z) = plan.zone {
            let v = match z {
                ZoneStyle::Long => "long",
                ZoneStyle::Short => "short",
            };
            set(&o, "timeZoneName", &s(v));
        }
    }
    if let Some(h12) = plan.ov.hour12 {
        set(&o, "hour12", &JsValue::from_bool(h12));
    }
    if let Some(cal) = plan.ov.calendar {
        set(&o, "calendar", &s(cal));
    }
    o
}

/// Format through `Intl.DateTimeFormat`. The constructor throws a `RangeError`
/// for an unknown zone or calendar; that is caught and reported as Bad Option.
pub fn format_intl(locale: &str, plan: &Plan<'_>, z: &Zoned<'_>, out: &mut String) -> Result<(), Error> {
    let o = options(plan);
    let epoch_ms = match *z {
        Zoned::Fixed { civil, offset } => {
            if offset == 0 {
                set(&o, "timeZone", &s("UTC"));
            } else {
                set(&o, "timeZone", &s(&offset_id(offset)));
            }
            civil.epoch_ms_as_utc() - i64::from(offset) * 60_000
        }
        Zoned::Named { instant_ms: Some(ms), zone, .. } => {
            set(&o, "timeZone", &s(zone));
            ms
        }
        // Floating wall-clock time in a named zone: ECMA-402 has no
        // "wall-clock → instant in zone" operation (Temporal has); a probe gap.
        Zoned::Named { instant_ms: None, .. } => return Err(Error::UnsupportedOperation),
    };
    let dtf = Dtf::new_dtf(&Array::of1(&s(locale)), &o).map_err(|_| Error::BadOption)?;
    #[allow(clippy::cast_precision_loss)]
    let date = Date::new(&JsValue::from_f64(epoch_ms as f64));
    out.push_str(&dtf.format_date(&date));
    Ok(())
}

/// Leaner alternative: one JS import; the options travel as a JSON string built
/// by hand in Rust (fmt-free). Same mapping as [`options`].
pub mod inline {
    use super::{DateFields, Error, Length, Plan, Precision, ZoneStyle, Zoned, offset_id};
    use alloc::string::String;
    use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen(inline_js = "export function mf2_dtf(l, o, t) { try { return new Intl.DateTimeFormat(l, JSON.parse(o)).format(t); } catch (e) { return undefined; } }")]
    extern "C" {
        fn mf2_dtf(locale: &str, options_json: &str, epoch_ms: f64) -> Option<String>;
    }

    fn kv(o: &mut String, k: &str, v: &str) {
        o.push(if o.is_empty() { '{' } else { ',' });
        o.push('"');
        o.push_str(k);
        o.push_str("\":\"");
        o.push_str(v);
        o.push('"');
    }

    pub fn format_intl_inline(
        locale: &str,
        plan: &Plan<'_>,
        z: &Zoned<'_>,
        out: &mut String,
    ) -> Result<(), Error> {
        let mut o = String::new();
        let styleable = matches!(plan.date, None | Some(DateFields::YearMonthDay))
            && matches!(plan.time, None | Some(Precision::Minute | Precision::Second))
            && plan.zone.is_none();
        let len = match plan.length {
            Length::Long => "long",
            Length::Medium => "medium",
            Length::Short => "short",
        };
        if styleable {
            if plan.date.is_some() {
                kv(&mut o, "dateStyle", len);
            }
            match plan.time {
                Some(Precision::Minute) => kv(&mut o, "timeStyle", "short"),
                Some(Precision::Second) => kv(&mut o, "timeStyle", "medium"),
                _ => {}
            }
        } else {
            if let Some(d) = plan.date {
                let (y, m, day, wd) = match d {
                    DateFields::Weekday => (false, false, false, true),
                    DateFields::DayWeekday => (false, false, true, true),
                    DateFields::MonthDay => (false, true, true, false),
                    DateFields::MonthDayWeekday => (false, true, true, true),
                    DateFields::YearMonthDay => (true, true, true, false),
                    DateFields::YearMonthDayWeekday => (true, true, true, true),
                };
                if y {
                    kv(&mut o, "year", if plan.length == Length::Short { "2-digit" } else { "numeric" });
                }
                if m {
                    kv(&mut o, "month", match plan.length {
                        Length::Long => "long",
                        Length::Medium => "short",
                        Length::Short => "numeric",
                    });
                }
                if day {
                    kv(&mut o, "day", "numeric");
                }
                if wd {
                    kv(&mut o, "weekday", if plan.length == Length::Long { "long" } else { "short" });
                }
            }
            if let Some(p) = plan.time {
                kv(&mut o, "hour", "numeric");
                if p != Precision::Hour {
                    kv(&mut o, "minute", "2-digit");
                }
                if p == Precision::Second {
                    kv(&mut o, "second", "2-digit");
                }
            }
            if let Some(zs) = plan.zone {
                kv(&mut o, "timeZoneName", if zs == ZoneStyle::Long { "long" } else { "short" });
            }
        }
        if let Some(h12) = plan.ov.hour12 {
            // JSON boolean, not a string.
            o.push(if o.is_empty() { '{' } else { ',' });
            o.push_str(if h12 { "\"hour12\":true" } else { "\"hour12\":false" });
        }
        if let Some(cal) = plan.ov.calendar {
            kv(&mut o, "calendar", cal);
        }
        let epoch_ms = match *z {
            Zoned::Fixed { civil, offset } => {
                if offset == 0 {
                    kv(&mut o, "timeZone", "UTC");
                } else {
                    kv(&mut o, "timeZone", &offset_id(offset));
                }
                civil.epoch_ms_as_utc() - i64::from(offset) * 60_000
            }
            Zoned::Named { instant_ms: Some(ms), zone, .. } => {
                // `zone` passed MF2's well-formedness check: no quote or backslash.
                kv(&mut o, "timeZone", zone);
                ms
            }
            Zoned::Named { instant_ms: None, .. } => return Err(Error::UnsupportedOperation),
        };
        o.push('}');
        #[allow(clippy::cast_precision_loss)]
        let t = mf2_dtf(locale, &o, epoch_ms as f64).ok_or(Error::BadOption)?;
        out.push_str(&t);
        Ok(())
    }
}
