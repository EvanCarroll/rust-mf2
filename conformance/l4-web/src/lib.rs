//! Conformance layer L4 in the browser, for the `intl` client option:
//! `mf2-l4-runner`'s cases — compiled natively by `cargo xtask l4-web` —
//! formatted by the `intl` build in the engine, where the numeric functions
//! take their display, `:integer`'s rounding and the plural category from
//! `Intl.NumberFormat` and `Intl.PluralRules` through
//! `mf2_host_web::ZONES_NUMBERS_HOST` (dates: ICU4X over the catalog's blob,
//! zone offsets from the browser). One canonical record per case, the same line
//! `mf2-wasi-matches-native` writes, for the native side to judge.

use mf2_runtime::Host;
use wasm_bindgen::prelude::wasm_bindgen;

/// Whether the build is the `intl` one (`mf2_runtime::INTL_NUMBERS`) and the
/// engine has what it needs (`Intl.NumberFormat` v3).
#[wasm_bindgen]
pub fn intl_ready() -> bool {
    mf2_runtime::INTL_NUMBERS && mf2_host_web::NUMBERS_HOST.numbers().is_some()
}

/// Formats every case of `bundle` (`mf2_l4_runner::encode_cases`): one line
/// per case, `id TAB record` (`Record::line`), or `id TAB ERROR …`.
#[wasm_bindgen]
pub fn run(bundle: &[u8]) -> String {
    let cases = match mf2_l4_runner::decode_cases(bundle) {
        Ok(c) => c,
        Err(e) => return format!("ERROR {e}\n"),
    };
    let mut out = String::new();
    for case in &cases {
        out.push_str(&case.id);
        out.push('\t');
        match mf2_l4_runner::run_with(case, &mf2_host_web::ZONES_NUMBERS_HOST) {
            Ok(r) => out.push_str(&r.line()),
            Err(e) => {
                out.push_str("ERROR ");
                out.push_str(&e);
            }
        }
        out.push('\n');
    }
    out
}
