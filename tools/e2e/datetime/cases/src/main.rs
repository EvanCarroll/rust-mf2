//! The `datetime-intl` browser comparison, native side (plans/11 A6): for
//! every panel locale and message, the one-message catalog (`compile_str`,
//! with its `icu.blob`) and the text ICU4X gives for it — what the server
//! renders — written as JSON for `tools/e2e/checks/datetime.mjs`, which
//! formats the same catalogs in the browser through `Intl.DateTimeFormat`.
//!
//!   e2e-datetime-cases <out.json>

mod error;

use std::fmt::Write as _;

use mf2::{BidiStrategy, FormatContext, Formatter, Function, Registry, TimeZone};
use serde_json::json;

use crate::error::Error;

/// The date functions over the default backend: ICU4X from the blob
/// (`datetime-icu`).
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
            let mapping = if fields == "year-month-day" { Styles } else { Components };
            m.push((format!("{{{at} :date fields={fields} length={length}}}"), mapping));
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
            m.push((format!("{{{at} :time precision={precision}{hour12}}}"), mapping));
        }
    }
    m.push((
        format!("{{{at} :datetime dateFields=month-day-weekday timePrecision=hour}}"),
        Components,
    ));
    m.push((format!("{{{at} :date calendar=japanese length=long}}"), Styles));
    for zone in ["UTC", "|America/New_York|", "|Europe/Paris|", "|Asia/Kolkata|", "|+05:30|"] {
        m.push((format!("{{{at} :time timeZone={zone}}}"), Styles));
        for style in ["long", "short"] {
            m.push((format!("{{{at} :time timeZone={zone} timeZoneStyle={style}}}"), Zoned));
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

fn main() -> Result<(), Error> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cases.json".into());
    let mut cx = FormatContext::new(&mf2::host_std::HOST);
    cx.bidi = BidiStrategy::None;
    cx.time_zone = TimeZone::UTC;
    let mut cases = Vec::new();
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
                "icuErrors": errors.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>(),
            }));
        }
    }
    let n = cases.len();
    std::fs::write(
        &out,
        serde_json::to_string(&json!({ "cldr": "48.2.1", "cases": cases }))?,
    )?;
    eprintln!("e2e-datetime-cases: {n} cases → {out}");
    Ok(())
}
