//! Core numeric semantics: parse operand, resolve options, format neutrally,
//! and run the selection path (exact match + plural operands).
use wasm_bindgen::prelude::*;
include!("../../opts.rs");

struct Out(String);
impl numcore::ByteSink for Out {
    fn byte(&mut self, b: u8) {
        self.0.push(char::from(b));
    }
}

#[wasm_bindgen]
pub fn fmt(data: &[u8], locale: &str, func_code: u8, opts: &str, operand: &str, key: &str) -> String {
    let mut buf = [("", numcore::OptValue::Literal("")); 8];
    let n = parse_opts(opts, &mut buf);
    let mut e = Errs(u32::from(data.len() as u8 ^ locale.len() as u8));
    let mut out = Out(String::new());
    let o = buf.get(..n).unwrap_or(&[]);
    // An integer argument when the operand text is `#<digits>` (models Arg::Int).
    let op = match operand.strip_prefix('#') {
        Some(d) => numcore::Operand::Int(d.bytes().fold(0i64, |a, c| a * 10 + i64::from(c & 15))),
        None => numcore::Operand::Str(operand),
    };
    if let Some(v) = numcore::resolve(func(func_code), &op, o, &mut e) {
        let f = numcore::format_digits(&v, &mut e);
        numcore::write_neutral(&f, &mut out);
        e.0 ^= select_bits(&v, &f, key);
    }
    out.0.push(char::from_u32(0x100 + (e.0 & 0xffff)).unwrap_or('?'));
    out.0
}
