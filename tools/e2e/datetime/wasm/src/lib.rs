//! The `intl` date formatter's browser comparison, browser side (plans/11 A6): a
//! case's catalog, with its params, formatted with `:date`, `:time`,
//! `:datetime` and unannotated dates over the
//! `Intl` backend — `Intl.DateTimeFormat` through `mf2-host-web`, whose
//! zone offsets come from the browser's own data — in UTC, no bidi
//! isolation, as the native side formats it with ICU4X.

use mf2::{
    Arg, BidiStrategy, Catalog, CustomValue, DateTime, FormatContext, FormatError, Formatter,
    Function, MsgId, Registry,
};
use wasm_bindgen::prelude::wasm_bindgen;

static FUNCTIONS: [(&str, &dyn Function); 3] = [
    ("date", &mf2_fn_datetime::DATE),
    ("datetime", &mf2_fn_datetime::DATETIME),
    ("time", &mf2_fn_datetime::TIME),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_dates(&mf2_fn_datetime::DATES);

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

/// Formats the one message of `catalog` (manifest hash `hash`, decimal)
/// with the suite's `params` (a JSON array of `{name, type?, value}`; a
/// `datetime` value is a date/time literal): its text, a tab, and its
/// errors' suite names, comma-separated; or `ERROR …` when the catalog does
/// not load.
#[wasm_bindgen]
pub fn format(catalog: Vec<u8>, hash: &str, params: &str) -> String {
    let Ok(hash) = hash.parse::<u64>() else {
        return "ERROR hash".into();
    };
    let catalog = match Catalog::new(catalog, hash) {
        Ok(c) => c,
        Err(e) => return format!("ERROR {e:?}"),
    };
    let params: Vec<serde_json::Value> = serde_json::from_str(params).unwrap_or_default();
    let dates: Vec<Option<DateTime<'static>>> = params
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
    let mut cx = FormatContext::new(&mf2::host_web::INTL_HOST);
    cx.bidi = BidiStrategy::None;
    let f = Formatter::new(&catalog, &REGISTRY, &cx);
    let mut text = String::new();
    let mut errors = Vec::new();
    // A one-message catalog's message (`mf2::Compiled::ID`, build side).
    f.write_named(MsgId::from_raw(0), &args, &mut text, &mut errors);
    let errors: Vec<String> = errors.into_iter().map(suite_name).collect();
    format!("{text}\t{}", errors.join(","))
}

/// The backend the statics use (to show it is `Intl`).
#[wasm_bindgen]
pub fn backend() -> String {
    core::any::type_name::<mf2_fn_datetime::DefaultBackend>().into()
}
