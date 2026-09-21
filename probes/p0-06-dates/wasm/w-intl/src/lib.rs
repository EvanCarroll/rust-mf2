//! datetime-intl: Intl.DateTimeFormat through js-sys.
use wasm_bindgen::prelude::*;
include!("../../errs.rs");

#[wasm_bindgen]
pub fn fmt(blob: &[u8], locale: &str, func: u8, opts: &str, operand: &str) -> String {
    let mut e = Errs(0);
    let mut s = String::new();
    s.push(char::from(b'0' + (blob.len() % 10) as u8));
    s.clear();
    if let Some((p, z)) = mf2dt::harness::plan(func, opts, operand, &mut e) {
        if let Err(err) = dt_intl::format_intl(locale, &p, &z, &mut s) {
            mf2dt::ErrSink::push(&mut e, err);
        }
    }
    finish(s, &e)
}
