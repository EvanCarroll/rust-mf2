//! Semantics only: operand parsing, option + zone resolution, neutral output.
use wasm_bindgen::prelude::*;
include!("../../errs.rs");

#[wasm_bindgen]
pub fn fmt(blob: &[u8], locale: &str, func: u8, opts: &str, operand: &str) -> String {
    let mut e = Errs(0);
    let mut s = String::new();
    s.push_str(locale);
    s.push(char::from(b'0' + (blob.len() % 10) as u8));
    if let Some((p, z)) = mf2dt::harness::plan(func, opts, operand, &mut e) {
        let c = match z {
            mf2dt::Zoned::Fixed { civil, .. } | mf2dt::Zoned::Named { civil, .. } => civil,
        };
        let mut b = [0u8; 19];
        mf2dt::write_neutral(&c, &mut b);
        for &c in &b {
            s.push(char::from(c));
        }
        s.push(char::from(b'a' + p.length as u8 + p.date.map_or(0, |d| d as u8) * 3));
        s.push(char::from(b'a' + p.time.map_or(0, |t| t as u8 + 1) + p.zone.map_or(0, |t| t as u8 + 1) * 4));
        s.push(char::from(b'a' + p.ov.hour12.map_or(0, |h| h as u8 + 1)));
        s.push_str(p.ov.calendar.unwrap_or(""));
    }
    finish(s, &e)
}
