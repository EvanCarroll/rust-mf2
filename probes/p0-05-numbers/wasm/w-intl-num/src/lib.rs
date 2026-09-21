//! Comparison: core semantics in Rust (so selection and digits stay exact and
//! single-sourced) + ECMA-402 `Intl.NumberFormat` for the localization step:
//! the rounded digits go to Intl as an exact decimal string with fraction and
//! integer digits pinned; `:percent` = `style: "percent"` on the digits × 10⁻².
use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::*;
include!("../../opts.rs");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Intl, js_name = NumberFormat)]
    type Nf;
    #[wasm_bindgen(constructor, catch, js_namespace = Intl, js_class = NumberFormat)]
    fn new_nf(locales: &Array, options: &Object) -> Result<Nf, JsValue>;
    #[wasm_bindgen(method, js_class = NumberFormat, js_name = format)]
    fn format_str(this: &Nf, value: &str) -> String;
}

struct Digits(String);
impl numcore::ByteSink for Digits {
    fn byte(&mut self, b: u8) {
        self.0.push(char::from(b));
    }
}

fn set(o: &Object, k: &str, v: &JsValue) {
    let _ = Reflect::set(o, &JsValue::from_str(k), v);
}

#[wasm_bindgen]
pub fn fmt(data: &[u8], locale: &str, func_code: u8, opts: &str, operand: &str, key: &str) -> String {
    let mut buf = [("", numcore::OptValue::Literal("")); 8];
    let n = parse_opts(opts, &mut buf);
    let o = buf.get(..n).unwrap_or(&[]);
    let mut e = Errs(u32::from(data.len() as u8));
    let mut out = String::new();
    let op = match operand.strip_prefix('#') {
        Some(d) => numcore::Operand::Int(d.bytes().fold(0i64, |a, c| a * 10 + i64::from(c & 15))),
        None => numcore::Operand::Str(operand),
    };
    let percent = func_code == 3;
    let spec = if percent { &PERCENT } else { match func_code { 1 => &numcore::INTEGER, 2 => &numcore::OFFSET, _ => &numcore::NUMBER } };
    if let Some(v) = numcore::resolve_spec(spec, &op, o, &mut e) {
        let f = numcore::format_digits(&v, &mut e);
        let mut d = Digits(String::new());
        numcore::write_neutral(&f, &mut d);
        let r = f.abs.magnitude_range();
        let frac = (-*r.start()).max(0) as u32;
        let int = (*r.end()).max(0) as u32 + 1;
        let opts = Object::new();
        set(&opts, "minimumFractionDigits", &JsValue::from(frac));
        set(&opts, "maximumFractionDigits", &JsValue::from(frac));
        set(&opts, "minimumIntegerDigits", &JsValue::from(int.min(21)));
        let g = match v.opts.use_grouping {
            Some(numcore::Grouping::Never) => JsValue::FALSE,
            Some(numcore::Grouping::Always) => JsValue::from_str("always"),
            Some(numcore::Grouping::Min2) => JsValue::from_str("min2"),
            _ => JsValue::from_str("auto"),
        };
        set(&opts, "useGrouping", &g);
        if f.sign == numcore::SignOut::Plus {
            set(&opts, "signDisplay", &JsValue::from_str("always"));
        }
        if percent {
            set(&opts, "style", &JsValue::from_str("percent"));
            d.0.push_str("e-2");
        }
        match Nf::new_nf(&Array::of1(&JsValue::from_str(locale)), &opts) {
            Ok(nf) => out.push_str(&nf.format_str(&d.0)),
            Err(_) => e.0 |= 1 << 8,
        }
        e.0 ^= select_bits(&v, &f, key);
    }
    out.push(char::from_u32(0x100 + (e.0 & 0xffff)).unwrap_or('?'));
    out
}

const PERCENT: numcore::Spec = numcore::Spec {
    bit: numcore::PCT,
    frac: Some(numcore::FracDefaults { min: 0, max: 0 }),
    integer: false,
    selectable: true,
    scale: 2,
};
