//! A browser's number formatter, through the host the generated module names
//! (`host::HOST`), never one chosen here: the check fails if `mf2` stops
//! naming the `Intl` number host for `intl`, or names it for another
//! formatter. Built once for each of the browser's number formatters.

use mf2::{Arg, Catalog, FormatContext, Formatter};
use mf2_catalog::format::locale_key;
use wasm_bindgen::prelude::wasm_bindgen;

mf2::include_generated!();

/// The `en` catalog a browser downloads (copied by the build script).
static EN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/intl-host-en.mf2b"));

/// The LOCALE entries a number formatter can read, by name.
const ENTRIES: [(u32, &str); 6] = [
    (locale_key::PLURAL_CARDINAL, "plural.cardinal"),
    (locale_key::PLURAL_ORDINAL, "plural.ordinal"),
    (locale_key::NUMBER_SYMBOLS, "number.symbols"),
    (locale_key::NUMBER_PATTERNS, "number.patterns"),
    (locale_key::CURRENCY_DATA, "currency.data"),
    (locale_key::UNIT_DATA, "unit.data"),
];

/// The type of the host the generated module names.
#[wasm_bindgen]
pub fn host_type() -> String {
    core::any::type_name_of_val(&host::HOST).into()
}

/// Whether the runtime asks the host to format numbers (`intl` alone).
#[wasm_bindgen]
pub fn by_host() -> bool {
    mf2::INTL_NUMBERS
}

/// The number entries the browser's catalog carries, comma-separated in
/// [`ENTRIES`]' order; `ERROR …` when the catalog does not load.
#[wasm_bindgen]
pub fn entries() -> String {
    let catalog = match Catalog::new(EN.to_vec(), MANIFEST_HASH) {
        Ok(c) => c,
        Err(e) => return format!("ERROR {e:?}"),
    };
    let found: Vec<&str> = ENTRIES
        .iter()
        .filter(|(key, _)| catalog.locale_entry(*key).is_some())
        .map(|(_, name)| *name)
        .collect();
    found.join(",")
}

/// Formats message `id` in `en` with `$n` the decimal `n` and `$when` the
/// date `when`: its text, a tab, and its errors (`Debug`), comma-separated;
/// `ERROR …` when the catalog or the id does not load.
#[wasm_bindgen]
pub fn format(id: &str, n: &str, when: &str) -> String {
    let catalog = match Catalog::new(EN.to_vec(), MANIFEST_HASH) {
        Ok(c) => c,
        Err(e) => return format!("ERROR {e:?}"),
    };
    let Some(msg) = catalog.lookup(id) else {
        return format!("ERROR no message {id}");
    };
    let cx = FormatContext::new(&host::HOST);
    let f = Formatter::new(&catalog, registry(), &cx);
    let args = [("n", Arg::Decimal(n)), ("when", Arg::Str(when))];
    let mut text = String::new();
    let mut errors = Vec::new();
    f.write_named(msg, &args, &mut text, &mut errors);
    let errors: Vec<String> = errors.iter().map(|e| format!("{e:?}")).collect();
    format!("{text}\t{}", errors.join(","))
}
