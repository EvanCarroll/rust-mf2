//! The `number-intl` host wiring in a browser: messages formatted through the host
//! the generated module names (`host::HOST`), never one chosen here, so that
//! the check fails if `mf2`'s `number-intl` arms stop naming the `Intl` number
//! host.

use mf2::{Arg, Catalog, FormatContext, Formatter};
use wasm_bindgen::prelude::wasm_bindgen;

mf2::include_generated!();

/// The `en` catalog (copied by the build script).
static EN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/intl-host-en.mf2b"));

/// The type of the host the generated module names.
#[wasm_bindgen]
pub fn host_type() -> String {
    core::any::type_name_of_val(&host::HOST).into()
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
