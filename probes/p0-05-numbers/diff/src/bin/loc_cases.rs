//! Emits :percent / :currency / :unit cases with numloc's output (JSON lines)
//! for scripts/intl-loc.cjs to compare with node's Intl.NumberFormat.
use numcore as nc;
use serde_json::json;

#[derive(Default)]
struct E(Vec<nc::Error>);
impl nc::ErrSink for E {
    fn push(&mut self, e: nc::Error) {
        self.0.push(e);
    }
}
struct S(String);
impl numloc::StrSink for S {
    fn push_str(&mut self, s: &str) {
        self.0.push_str(s);
    }
    fn push_char(&mut self, c: char) {
        self.0.push(c);
    }
}

/// Harness-only plural stub per locale family: enough for currency names and
/// unit patterns in these samples (the real evaluator is P0.4's).
fn category(loc: &str) -> impl Fn(&nc::PluralOperands, bool) -> &'static str {
    let l = loc.to_owned();
    move |op, _| match l.as_str() {
        "ja" => "other",
        "fr" if op.i <= 1 => "one",
        _ if op.i == 1 && op.v == 0 => "one",
        _ => "other",
    }
}

fn main() {
    let values = ["0", "1", "-1", "2", "12.5", "1234.5", "-1234.567", "1000000", "0.005"];
    let locales = ["en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy"];
    let cases: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("percent", vec![]),
        ("percent", vec![("maximumFractionDigits", "1")]),
        ("currency", vec![("currency", "EUR")]),
        ("currency", vec![("currency", "USD")]),
        ("currency", vec![("currency", "JPY")]),
        ("currency", vec![("currency", "GBP"), ("currencyDisplay", "code")]),
        ("currency", vec![("currency", "USD"), ("currencyDisplay", "narrowSymbol")]),
        ("currency", vec![("currency", "EUR"), ("currencySign", "accounting")]),
        ("currency", vec![("currency", "EUR"), ("currencyDisplay", "name")]),
        ("unit", vec![("unit", "kilometer")]),
        ("unit", vec![("unit", "kilogram"), ("unitDisplay", "long")]),
        ("unit", vec![("unit", "celsius"), ("unitDisplay", "narrow")]),
        ("unit", vec![("unit", "hour"), ("unitDisplay", "long")]),
    ];
    for loc in locales {
        let data = std::fs::read(format!("out/locale/{loc}.all-used.bin")).expect("run locdata first");
        let cat = category(loc);
        for (func, opts) in &cases {
            for v in values {
                let o: Vec<(&str, nc::OptValue<'_>)> = opts.iter().map(|&(k, v)| (k, nc::OptValue::Literal(v))).collect();
                let mut e = E::default();
                let op = numloc::LocOperand::Core(nc::Operand::Str(v));
                let lv = match *func {
                    "percent" => numloc::resolve_percent(&op, &o, &mut e),
                    "currency" => numloc::currency::resolve(&op, &o, &data, &mut e),
                    _ => numloc::unit::resolve(&op, &o, &mut e),
                };
                let Some(lv) = lv else { continue };
                let mut s = S(String::new());
                numloc::format(&lv, &data, false, &cat, &mut s, &mut e);
                let obj: serde_json::Map<String, serde_json::Value> = opts.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect();
                println!("{}", json!({"loc": loc, "f": func, "v": v, "o": obj, "out": s.0}));
            }
        }
    }
}
