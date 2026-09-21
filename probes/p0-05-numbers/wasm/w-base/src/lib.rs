//! Base: same exported signatures, no numeric code. Every size is a delta to this.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn fmt(data: &[u8], locale: &str, func: u8, opts: &str, operand: &str, key: &str) -> String {
    let mut s = String::with_capacity(locale.len() + opts.len() + operand.len() + key.len() + 2);
    s.push_str(locale);
    s.push_str(opts);
    s.push_str(operand);
    s.push_str(key);
    s.push(char::from(b'0' + func % 10));
    s.push(char::from(b'0' + (data.len() % 10) as u8));
    s
}
