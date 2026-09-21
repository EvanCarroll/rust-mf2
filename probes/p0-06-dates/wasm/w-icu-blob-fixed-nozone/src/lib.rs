//! datetime-icu, fixed calendar, no time-zone names, data from a runtime blob.
use wasm_bindgen::prelude::*;
include!("../../errs.rs");

#[wasm_bindgen]
pub fn fmt(blob: &[u8], locale: &str, func: u8, opts: &str, operand: &str) -> String {
    let mut e = Errs(0);
    let mut s = String::new();
    if let Some((p, z)) = mf2dt::harness::plan(func, opts, operand, &mut e) {
        // The catalog would carry its data locale; the harness uses the
        // language subtag of the requested locale.
        let data_locale = locale.split('-').next().unwrap_or("und").parse().unwrap_or_default();
        match icu_provider_blob::BlobDataProvider::try_new_from_blob(blob.into()) {
            Ok(inner) => {
                let provider = dt_icu::OneLocale { inner, locale: data_locale };
                if let Err(err) = dt_icu::format_fixed_nozone_buffer(&provider, locale, &p, &z, &mut s) {
                    mf2dt::ErrSink::push(&mut e, err);
                }
            }
            Err(_) => mf2dt::ErrSink::push(&mut e, mf2dt::Error::UnsupportedOperation),
        }
    }
    finish(s, &e)
}
