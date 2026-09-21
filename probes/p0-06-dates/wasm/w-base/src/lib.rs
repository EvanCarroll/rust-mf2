//! Base: same exported signature, no date code. Every size is a delta to this.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn fmt(blob: &[u8], locale: &str, func: u8, opts: &str, operand: &str) -> String {
    let mut s = String::with_capacity(locale.len() + opts.len() + operand.len() + 2);
    s.push_str(locale);
    s.push_str(opts);
    s.push_str(operand);
    s.push(char::from(b'0' + func % 10));
    s.push(char::from(b'0' + (blob.len() % 10) as u8));
    s
}
