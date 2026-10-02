//! `DATETIME` (the tooling design §6.1, "`DATETIME` options").
//!
//! Fluent's options are `Intl.DateTimeFormat`'s; MF2's `:date`, `:time` and
//! `:datetime` take semantic fields and lengths. This is the nearest mapping,
//! not an identity, which is why every converted `DATETIME` is reported with
//! `fluent-datetime-approximate`.

use std::borrow::Cow;

use mf2_model::{FunctionRef, Literal, OptionValue, Options};

/// The function and options a `DATETIME` becomes, or the option that has no
/// MF2 counterpart.
pub(super) fn map(named: &[(&str, String)]) -> Result<FunctionRef<'static>, String> {
    let get = |name: &str| {
        named
            .iter()
            .rev()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    };
    for (name, _) in named {
        if !matches!(
            *name,
            "dateStyle"
                | "timeStyle"
                | "weekday"
                | "year"
                | "month"
                | "day"
                | "hour"
                | "minute"
                | "second"
                | "timeZoneName"
                | "timeZone"
                | "hour12"
                | "hourCycle"
        ) {
            return Err((*name).to_owned());
        }
    }

    // What the date part says: a length and, where the fields are not the
    // default set, the fields.
    let date_fields = ["weekday", "year", "month", "day"];
    let has_date_field = date_fields.iter().any(|f| get(f).is_some());
    let mut length: Option<&'static str> = None;
    let mut fields: Option<&'static str> = None;
    if let Some(style) = get("dateStyle") {
        if has_date_field {
            return Err("dateStyle".to_owned());
        }
        length = Some(match style {
            "full" => {
                fields = Some("year-month-day-weekday");
                "long"
            }
            "long" => "long",
            "medium" => "medium",
            "short" => "short",
            _ => return Err("dateStyle".to_owned()),
        });
    } else if has_date_field {
        let set = (
            get("weekday").is_some(),
            get("year").is_some(),
            get("month").is_some(),
            get("day").is_some(),
        );
        fields = Some(match set {
            (true, false, false, false) => "weekday",
            (true, false, false, true) => "day-weekday",
            (false, false, true, true) => "month-day",
            (true, false, true, true) => "month-day-weekday",
            (false, true, true, true) => "year-month-day",
            (true, true, true, true) => "year-month-day-weekday",
            _ => {
                return Err(date_fields
                    .iter()
                    .filter(|f| get(f).is_some())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" + "));
            }
        });
        length = Some(match (get("month"), get("weekday")) {
            (Some("long"), _) | (None, Some("long")) => "long",
            (Some("short") | None, _) => "medium",
            (Some(_), _) => "short",
        });
    }

    // The time part: a precision, and a time zone style.
    let mut precision: Option<&'static str> = None;
    let mut zone_style: Option<&'static str> = None;
    let has_time_field = ["hour", "minute", "second"]
        .iter()
        .any(|f| get(f).is_some());
    if let Some(style) = get("timeStyle") {
        if has_time_field {
            return Err("timeStyle".to_owned());
        }
        match style {
            "full" => {
                precision = Some("second");
                zone_style = Some("long");
            }
            "long" => {
                precision = Some("second");
                zone_style = Some("short");
            }
            "medium" => precision = Some("second"),
            "short" => precision = Some("minute"),
            _ => return Err("timeStyle".to_owned()),
        }
    } else if has_time_field {
        if get("hour").is_none() {
            return Err(if get("minute").is_some() {
                "minute"
            } else {
                "second"
            }
            .to_owned());
        }
        precision = Some(if get("second").is_some() {
            "second"
        } else if get("minute").is_some() {
            "minute"
        } else {
            "hour"
        });
    }
    if let Some(name) = get("timeZoneName") {
        zone_style = Some(match name {
            "long" | "longGeneric" => "long",
            _ => "short",
        });
    }

    let has_date = length.is_some();
    let has_time = precision.is_some() || zone_style.is_some();
    let function = match (has_date, has_time) {
        (true, true) => "datetime",
        (false, true) => "time",
        // `Intl`'s default with no field at all: numeric year, month, day.
        (_, false) => "date",
    };
    if !has_date && !has_time {
        length = Some("short");
    }

    let mut options = Options::new();
    let mut put = |name: &'static str, value: &str| {
        options.push(
            Cow::Borrowed(name),
            OptionValue::Literal(Literal {
                value: Cow::Owned(value.to_owned()),
            }),
        );
    };
    let (fields_name, length_name, precision_name) = if function == "datetime" {
        ("dateFields", "dateLength", "timePrecision")
    } else {
        ("fields", "length", "precision")
    };
    if let Some(f) = fields {
        put(fields_name, f);
    }
    if let Some(l) = length {
        put(length_name, l);
    }
    if let Some(p) = precision {
        put(precision_name, p);
    } else if function == "time" {
        // A zone name alone: `Intl` shows the default time with it.
        put(precision_name, "minute");
    }
    if let Some(z) = zone_style {
        put("timeZoneStyle", z);
    }
    if let Some(z) = get("timeZone") {
        put("timeZone", z);
    }
    if function != "date" {
        let twelve = match (get("hour12"), get("hourCycle")) {
            (Some(v), _) => Some(v == "true"),
            (None, Some("h11" | "h12")) => Some(true),
            (None, Some(_)) => Some(false),
            (None, None) => None,
        };
        if let Some(t) = twelve {
            put("hour12", if t { "true" } else { "false" });
        }
    }
    Ok(FunctionRef {
        name: Cow::Borrowed(function),
        options,
    })
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::map;

    fn mapped(named: &[(&str, &str)]) -> String {
        let named: Vec<(&str, String)> = named.iter().map(|(n, v)| (*n, (*v).to_owned())).collect();
        match map(&named) {
            Ok(f) => {
                let mut s = format!(":{}", f.name);
                for (n, v) in f.options.iter() {
                    if let mf2_model::OptionValue::Literal(l) = v {
                        let _ = write!(s, " {n}={}", l.value);
                    }
                }
                s
            }
            Err(e) => format!("error {e}"),
        }
    }

    #[test]
    fn the_table() {
        assert_eq!(mapped(&[]), ":date length=short");
        assert_eq!(
            mapped(&[("dateStyle", "full")]),
            ":date fields=year-month-day-weekday length=long"
        );
        assert_eq!(mapped(&[("timeStyle", "short")]), ":time precision=minute");
        assert_eq!(
            mapped(&[("dateStyle", "medium"), ("timeStyle", "long")]),
            ":datetime dateLength=medium timePrecision=second timeZoneStyle=short"
        );
        assert_eq!(
            mapped(&[("month", "long"), ("day", "numeric")]),
            ":date fields=month-day length=long"
        );
        assert_eq!(mapped(&[("year", "numeric")]), "error year");
        assert_eq!(mapped(&[("minute", "2-digit")]), "error minute");
        assert_eq!(mapped(&[("era", "long")]), "error era");
        assert_eq!(
            mapped(&[("hour", "numeric"), ("hourCycle", "h23")]),
            ":time precision=hour hour12=false"
        );
    }
}
