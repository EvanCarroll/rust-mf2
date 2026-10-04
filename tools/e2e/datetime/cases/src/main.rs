//! The `intl` date formatter's browser comparison, native side (plans/11 A6): for
//! the suite's date files (functions/{date,time,datetime}.json, with their
//! params and expected errors) and every panel locale × message, the
//! one-message catalog (`compile_str`, with its `icu.blob`) and the text
//! ICU4X gives for it — what the server renders — written as JSON for
//! `tools/e2e/checks/datetime.mjs`, which formats the same catalogs in the
//! browser through `Intl.DateTimeFormat`.
//!
//! Beside it, `speed.json`: the catalogs and argument values of the speed
//! harness (plan/08 §9; `tools/e2e/datetime/web/speed.html`), with ICU4X's
//! text for the first value.
//!
//!   e2e-datetime-cases <out.json>     # also writes speed.json beside it

mod error;

use std::fmt::Write as _;

use std::path::Path;

use mf2::{
    Arg, BidiStrategy, CustomValue, FormatContext, FormatError, Formatter, Function, Registry,
    TimeZone,
};
use serde_json::json;

use crate::error::Error;

/// The date functions over the default backend: ICU4X from the blob
/// (`icu`).
static FUNCTIONS: [(&str, &dyn Function); 3] = [
    ("date", &mf2_fn_datetime::DATE),
    ("datetime", &mf2_fn_datetime::DATETIME),
    ("time", &mf2_fn_datetime::TIME),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// The locale panel (plans/01-conformance.md §5).
const PANEL: [&str; 11] = [
    "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
];

/// How `mf2-host-web` maps a case onto `Intl.DateTimeFormat`: with
/// `dateStyle` / `timeStyle` (the date `year-month-day` or none, the time to
/// the minute or second or none, no zone style, not a time alone with
/// `hour12` — the mapping is exact),
/// with components (it is not), or with a zone style (components too, and
/// `timeZoneName`).
#[derive(Clone, Copy)]
enum Mapping {
    Styles,
    Components,
    Zoned,
}

impl Mapping {
    fn name(self) -> &'static str {
        match self {
            Mapping::Styles => "styles",
            Mapping::Components => "components",
            Mapping::Zoned => "zoned",
        }
    }
}

/// The messages: every date field set and length, every time precision,
/// `hour12`, zone styles in UTC, a named zone and an offset, a calendar.
fn messages() -> Vec<(String, Mapping)> {
    use Mapping::{Components, Styles, Zoned};
    let mut m = Vec::new();
    let at = "|2006-01-02T15:04:06Z|";
    for length in ["long", "medium", "short"] {
        for fields in [
            "weekday",
            "day-weekday",
            "month-day",
            "month-day-weekday",
            "year-month-day",
            "year-month-day-weekday",
        ] {
            let mapping = if fields == "year-month-day" {
                Styles
            } else {
                Components
            };
            m.push((
                format!("{{{at} :date fields={fields} length={length}}}"),
                mapping,
            ));
        }
        m.push((format!("{{{at} :datetime dateLength={length}}}"), Styles));
        m.push((
            format!("{{{at} :datetime dateLength={length} timePrecision=second}}"),
            Styles,
        ));
    }
    for precision in ["hour", "minute", "second"] {
        for hour12 in ["", " hour12=true", " hour12=false"] {
            // A time alone with `hour12` maps to components (mf2-host-web).
            let mapping = if precision == "hour" || !hour12.is_empty() {
                Components
            } else {
                Styles
            };
            m.push((
                format!("{{{at} :time precision={precision}{hour12}}}"),
                mapping,
            ));
        }
    }
    m.push((
        format!("{{{at} :datetime dateFields=month-day-weekday timePrecision=hour}}"),
        Components,
    ));
    m.push((
        format!("{{{at} :date calendar=japanese length=long}}"),
        Styles,
    ));
    for zone in [
        "UTC",
        "|America/New_York|",
        "|Europe/Paris|",
        "|Asia/Kolkata|",
        "|+05:30|",
    ] {
        m.push((format!("{{{at} :time timeZone={zone}}}"), Styles));
        for style in ["long", "short"] {
            m.push((
                format!("{{{at} :time timeZone={zone} timeZoneStyle={style}}}"),
                Zoned,
            ));
            m.push((
                format!("{{{at} :datetime timeZone={zone} timeZoneStyle={style}}}"),
                Zoned,
            ));
        }
    }
    m
}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        let _ = write!(s, "{x:02x}");
    }
    s
}

/// An error's suite name (`BadOperand` → `bad-operand`).
fn suite_name(e: FormatError) -> String {
    let mut s = String::new();
    for (i, c) in format!("{e:?}").chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                s.push('-');
            }
            s.push(c.to_ascii_lowercase());
        } else {
            s.push(c);
        }
    }
    s
}

/// An application value with no conversions (the suite's `true`).
struct Opaque;

impl CustomValue for Opaque {}

static OPAQUE: Opaque = Opaque;

/// The suite's date files (functions/{date,time,datetime}.json): each test
/// as a case with its params, its expected errors and text, and ICU4X's
/// output — layer L4 of the `intl` date formatter, run in the browsers.
fn suite(repo: &Path, cx: &FormatContext, cases: &mut Vec<serde_json::Value>) -> Result<(), Error> {
    for file in ["date", "time", "datetime"] {
        let path = repo.join(format!(
            "third_party/message-format-wg/test/tests/functions/{file}.json"
        ));
        let data: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let defaults = &data["defaultTestProperties"];
        let Some(tests) = data["tests"].as_array() else {
            continue;
        };
        for (index, t) in tests.iter().enumerate() {
            let src = t["src"].as_str().unwrap_or_default();
            let locale = t["locale"]
                .as_str()
                .or_else(|| defaults["locale"].as_str())
                .unwrap_or("en-US");
            let params: Vec<serde_json::Value> =
                t["params"].as_array().cloned().unwrap_or_default();
            let m = mf2::compile_str(src, locale).map_err(|source| Error::Compile {
                src: src.to_owned(),
                locale: locale.to_owned(),
                source,
            })?;
            let dates: Vec<Option<mf2::DateTime<'static>>> = params
                .iter()
                .map(|p| {
                    (p["type"] == "datetime")
                        .then(|| p["value"].as_str().and_then(mf2_fn_datetime::parse_literal))
                        .flatten()
                })
                .collect();
            let args: Vec<(&str, Arg<'_>)> = params
                .iter()
                .zip(&dates)
                .map(|(p, d)| {
                    let name = p["name"].as_str().unwrap_or_default();
                    let arg = match (d, &p["value"]) {
                        (Some(d), _) => Arg::DateTime(d),
                        (None, serde_json::Value::String(s)) => Arg::Str(s),
                        _ => Arg::Custom(&OPAQUE),
                    };
                    (name, arg)
                })
                .collect();
            let f = Formatter::new(&m.catalog, &REGISTRY, cx);
            let mut text = String::new();
            let mut errors = Vec::new();
            f.write_named(mf2::Compiled::ID, &args, &mut text, &mut errors);
            let mut exp_errors: Vec<String> = t["expErrors"]
                .as_array()
                .or_else(|| defaults["expErrors"].as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e["type"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            exp_errors.sort();
            cases.push(json!({
                "locale": locale,
                "src": src,
                "mapping": "suite",
                "suite": format!("functions/{file}.json#{index}"),
                "params": params,
                "exp": t["exp"],
                "expErrors": exp_errors,
                "catalog": hex(m.catalog.as_bytes()),
                "hash": m.manifest.hash().to_string(),
                "icu": text,
                "icuErrors": errors.iter().map(|e| suite_name(*e)).collect::<Vec<_>>(),
            }));
        }
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cases.json".into());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let mut cx = FormatContext::new(&mf2::host_std::ZONES_HOST);
    cx.bidi = BidiStrategy::None;
    cx.time_zone = TimeZone::UTC;
    let mut cases = Vec::new();
    suite(&repo, &cx, &mut cases)?;
    for locale in PANEL {
        for (src, mapping) in messages() {
            let m = mf2::compile_str(&src, locale).map_err(|source| Error::Compile {
                src: src.clone(),
                locale: locale.to_owned(),
                source,
            })?;
            let f = Formatter::new(&m.catalog, &REGISTRY, &cx);
            let mut text = String::new();
            let mut errors = Vec::new();
            f.write(mf2::Compiled::ID, &[], &mut text, &mut errors);
            cases.push(json!({
                "locale": locale,
                "src": src,
                "mapping": mapping.name(),
                "catalog": hex(m.catalog.as_bytes()),
                "hash": m.manifest.hash().to_string(),
                "icu": text,
                "icuErrors": errors.iter().map(|e| suite_name(*e)).collect::<Vec<_>>(),
            }));
        }
    }
    let n = cases.len();
    std::fs::write(
        &out,
        serde_json::to_string(&json!({ "cldr": "48.2.1", "cases": cases }))?,
    )?;
    eprintln!("e2e-datetime-cases: {n} cases → {out}");
    let speed_out = Path::new(&out).with_file_name("speed.json");
    std::fs::write(&speed_out, serde_json::to_string(&speed(&cx)?)?)?;
    eprintln!(
        "e2e-datetime-cases: speed catalogs → {}",
        speed_out.display()
    );
    Ok(())
}

/// The speed harness's locales (plan/08 §9): Latin, and a script written
/// right to left.
const SPEED_LOCALES: [&str; 3] = ["en", "pl", "ar"];

/// The speed harness's messages: one date placeholder each, for a date, a
/// date and time, and a date and time with a zone name.
const SPEED_MESSAGES: [(&str, &str); 3] = [
    ("date", "{$d :date}"),
    ("datetime", "{$d :datetime}"),
    (
        "zone-name",
        "{$d :datetime timeZone=|America/New_York| timeZoneStyle=long}",
    ),
];

/// The argument values the harness cycles through: 100 floating date/time
/// literals, every month, many days and hours (so no engine formats one
/// value over and over).
fn speed_values() -> Vec<String> {
    (0..100u32)
        .map(|i| {
            format!(
                "2026-{:02}-{:02}T{:02}:{:02}:05",
                1 + i % 12,
                1 + i % 28,
                i % 24,
                (i * 7) % 60
            )
        })
        .collect()
}

/// `speed.json`: every locale × message's one-message catalog, and ICU4X's
/// text for the first value (what the server renders; the page compares
/// each build's text with it).
fn speed(cx: &FormatContext) -> Result<serde_json::Value, Error> {
    let values = speed_values();
    let first = values
        .first()
        .and_then(|v| mf2_fn_datetime::parse_literal(v));
    let mut catalogs = Vec::new();
    for locale in SPEED_LOCALES {
        for (name, src) in SPEED_MESSAGES {
            let m = mf2::compile_str(src, locale).map_err(|source| Error::Compile {
                src: src.to_owned(),
                locale: locale.to_owned(),
                source,
            })?;
            let f = Formatter::new(&m.catalog, &REGISTRY, cx);
            let mut text = String::new();
            let mut errors = Vec::new();
            let args: Vec<(&str, Arg<'_>)> =
                first.iter().map(|d| ("d", Arg::DateTime(d))).collect();
            f.write_named(mf2::Compiled::ID, &args, &mut text, &mut errors);
            catalogs.push(json!({
                "locale": locale,
                "name": name,
                "src": src,
                "catalog": hex(m.catalog.as_bytes()),
                "hash": m.manifest.hash().to_string(),
                "icu": text,
                "icuErrors": errors.iter().map(|e| suite_name(*e)).collect::<Vec<_>>(),
            }));
        }
    }
    Ok(json!({ "cldr": "48.2.1", "values": values, "catalogs": catalogs }))
}
