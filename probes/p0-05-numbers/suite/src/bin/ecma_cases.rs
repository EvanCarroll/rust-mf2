//! Emits random (value, options) cases with numcore's neutral output, one JSON
//! object per line, for scripts/ecma-diff.cjs to compare against ECMA-402
//! `Intl.NumberFormat('en', {useGrouping: false, ...})`.
use numcore as nc;
use serde_json::json;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[derive(Default)]
struct E(Vec<nc::Error>);
impl nc::ErrSink for E {
    fn push(&mut self, e: nc::Error) {
        self.0.push(e);
    }
}
struct S(String);
impl nc::ByteSink for S {
    fn byte(&mut self, b: u8) {
        self.0.push(char::from(b));
    }
}

fn value(r: &mut Rng) -> String {
    const EDGE: [&str; 16] = ["0", "-0", "1", "0.5", "1.5", "2.5", "-2.5", "0.05", "9.995", "999.9995", "0.000123456", "123456789.987654321", "1e-7", "-0.0004", "5", "99.5"];
    if r.below(5) == 0 {
        return EDGE[r.below(EDGE.len() as u64) as usize].to_owned();
    }
    let len = 1 + r.below(10) as usize;
    let mut digits: String = (0..len).map(|_| char::from(b'0' + r.below(10) as u8)).collect();
    let point = r.below(len as u64 + 4) as usize;
    if point > 0 && point < len {
        digits.insert(point, '.');
    } else if point >= len {
        digits = format!("0.{}{}", "0".repeat(point - len), digits);
    }
    // normalise leading zeros for number-literal syntax
    let mut s = digits.trim_start_matches('0').to_owned();
    if s.is_empty() || s.starts_with('.') {
        s.insert(0, '0');
    }
    if r.below(3) == 0 { format!("-{s}") } else { s }
}

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20000);
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    const MODES: [&str; 9] = ["ceil", "floor", "expand", "trunc", "halfCeil", "halfFloor", "halfExpand", "halfTrunc", "halfEven"];
    const SIGNS: [&str; 5] = ["auto", "always", "exceptZero", "negative", "never"];
    const PRIO: [&str; 3] = ["auto", "morePrecision", "lessPrecision"];
    const INCS: [u16; 15] = [1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000];
    for _ in 0..n {
        let v = value(&mut r);
        let mut o: Vec<(String, String)> = Vec::new();
        let mut add = |k: &str, val: String| o.push((k.to_owned(), val));
        if r.below(4) == 0 { add("minimumIntegerDigits", (1 + r.below(5)).to_string()); }
        let inc = if r.below(6) == 0 { INCS[r.below(15) as usize] } else { 1 };
        if inc != 1 {
            let fd = r.below(4).to_string();
            add("minimumFractionDigits", fd.clone());
            add("maximumFractionDigits", fd);
            add("roundingIncrement", inc.to_string());
        } else {
            if r.below(3) == 0 { add("minimumFractionDigits", r.below(5).to_string()); }
            if r.below(3) == 0 { add("maximumFractionDigits", r.below(7).to_string()); }
            if r.below(3) == 0 { add("minimumSignificantDigits", (1 + r.below(6)).to_string()); }
            if r.below(3) == 0 { add("maximumSignificantDigits", (1 + r.below(8)).to_string()); }
            if r.below(3) == 0 { add("roundingPriority", PRIO[r.below(3) as usize].to_owned()); }
        }
        if r.below(2) == 0 { add("roundingMode", MODES[r.below(9) as usize].to_owned()); }
        if r.below(3) == 0 { add("signDisplay", SIGNS[r.below(5) as usize].to_owned()); }
        if r.below(4) == 0 { add("trailingZeroDisplay", "stripIfInteger".to_owned()); }
        let opts: Vec<(&str, nc::OptValue<'_>)> = o.iter().map(|(k, v)| (k.as_str(), nc::OptValue::Literal(v.as_str()))).collect();
        let mut e = E::default();
        let out = nc::resolve(nc::Func::Number, &nc::Operand::Str(&v), &opts, &mut e).map(|nv| {
            let f = nc::format_digits(&nv, &mut e);
            let mut s = S(String::new());
            nc::write_neutral(&f, &mut s);
            s.0
        });
        let obj: serde_json::Map<String, serde_json::Value> = o.iter().map(|(k, v)| {
            let jv = v.parse::<u64>().map_or_else(|_| json!(v), |n| json!(n));
            (k.clone(), jv)
        }).collect();
        println!("{}", json!({"v": v, "o": obj, "out": out, "errs": e.0.len()}));
    }
}
