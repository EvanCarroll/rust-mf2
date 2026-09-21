//! fn-number on: core semantics + localization (symbols, grouping, digits)
//! + :percent, data from the catalog's LOCALE entries (`data`).
use wasm_bindgen::prelude::*;
include!("../../opts.rs");

struct Out(String);
impl numloc::StrSink for Out {
    fn push_str(&mut self, s: &str) {
        self.0.push_str(s);
    }
    fn push_char(&mut self, c: char) {
        self.0.push(c);
    }
}

/// Harness plural stub (the evaluator is P0.4's budget).
fn cat(op: &numcore::PluralOperands, _ord: bool) -> &'static str {
    if op.i == 1 && op.v == 0 { "one" } else { "other" }
}

#[wasm_bindgen]
pub fn fmt(data: &[u8], locale: &str, func_code: u8, opts: &str, operand: &str, key: &str) -> String {
    let mut buf = [("", numcore::OptValue::Literal("")); 8];
    let n = parse_opts(opts, &mut buf);
    let o = buf.get(..n).unwrap_or(&[]);
    let mut e = Errs(0);
    let mut out = Out(String::new());
    let core_op = match operand.strip_prefix('#') {
        Some(d) => numcore::Operand::Int(d.bytes().fold(0i64, |a, c| a * 10 + i64::from(c & 15))),
        None => numcore::Operand::Str(operand),
    };
    let op = numloc::LocOperand::Core(core_op);
    let v = match func_code {
        3 => numloc::resolve_percent(&op, o, &mut e),
        #[cfg(feature = "cu")]
        4 => numloc::currency::resolve(&op, o, data, &mut e),
        #[cfg(feature = "cu")]
        5 => numloc::unit::resolve(&op, o, &mut e),
        c => numcore::resolve(func(c), &core_op, o, &mut e).map(|num| numloc::LocValue { num, kind: numloc::Kind::Number }),
    };
    if let Some(v) = v {
        numloc::format(&v, data, locale.len() > 5, &cat, &mut out, &mut e);
        let f = numcore::format_digits(&v.num, &mut e);
        e.0 ^= select_bits(&v.num, &f, key);
    }
    out.0.push(char::from_u32(0x100 + (e.0 & 0xffff)).unwrap_or('?'));
    out.0
}
