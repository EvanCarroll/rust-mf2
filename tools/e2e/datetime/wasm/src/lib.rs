//! The `datetime-intl` browser comparison, browser side (plans/11 A6): a
//! case's catalog formatted with `:date`, `:time`, `:datetime` over the
//! `Intl` backend — `Intl.DateTimeFormat` through `mf2-host-web`, whose
//! zone offsets come from the browser's own data — in UTC, no bidi
//! isolation, as the native side formats it with ICU4X.

use mf2::{BidiStrategy, Catalog, FormatContext, Formatter, Function, MsgId, Registry};
use wasm_bindgen::prelude::wasm_bindgen;

static FUNCTIONS: [(&str, &dyn Function); 3] = [
    ("date", &mf2_fn_datetime::DATE),
    ("datetime", &mf2_fn_datetime::DATETIME),
    ("time", &mf2_fn_datetime::TIME),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// Formats the one message of `catalog` (manifest hash `hash`, decimal):
/// its text, a tab, and its errors (`Debug` names, comma-separated); or
/// `ERROR …` when the catalog does not load.
#[wasm_bindgen]
pub fn format(catalog: Vec<u8>, hash: &str) -> String {
    let Ok(hash) = hash.parse::<u64>() else {
        return "ERROR hash".into();
    };
    let catalog = match Catalog::new(catalog, hash) {
        Ok(c) => c,
        Err(e) => return format!("ERROR {e:?}"),
    };
    let mut cx = FormatContext::new(&mf2::host_web::INTL_HOST);
    cx.bidi = BidiStrategy::None;
    let f = Formatter::new(&catalog, &REGISTRY, &cx);
    let mut text = String::new();
    let mut errors = Vec::new();
    // A one-message catalog's message (`mf2::Compiled::ID`, build side).
    f.write(MsgId::from_raw(0), &[], &mut text, &mut errors);
    let errors: Vec<String> = errors.iter().map(|e| format!("{e:?}")).collect();
    format!("{text}\t{}", errors.join(","))
}

/// The backend the statics use (to show it is `Intl`).
#[wasm_bindgen]
pub fn backend() -> String {
    core::any::type_name::<mf2_fn_datetime::DefaultBackend>().into()
}
