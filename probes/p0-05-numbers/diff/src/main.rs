//! numloc vs icu_decimal, same values, same grouping strategy, per locale.
//! Also emits cases for scripts/intl-loc.cjs (percent/currency/unit vs node Intl).
use icu_decimal::options::{DecimalFormatterOptions, GroupingStrategy};
use icu_decimal::DecimalFormatter;
use numcore as nc;

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

fn values() -> Vec<String> {
    let mut v: Vec<String> = [
        "0", "-0", "1", "-1", "7", "12", "123", "999", "1000", "1234", "-1234", "9999", "10000", "12345", "99999",
        "100000", "123456", "1234567", "12345678", "123456789", "1234567890", "12345678901234", "0.5", "-0.5",
        "1.5", "0.001", "0.0005", "3.14159", "1234.5678", "-98765.4321", "1000000.001", "0.1", "100", "1e6",
        "1.23e-4", "5e10",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    // A deterministic spread of magnitudes.
    let mut x: u64 = 0x1234_5678_9abc_def1;
    for _ in 0..200 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let digits = (x % 10_000_000_000) as i64;
        let scale = (x >> 40) % 8;
        let s = if scale == 0 { digits.to_string() } else { format!("{}e-{}", digits, scale) };
        v.push(if x & 1 == 0 { s } else { format!("-{s}") });
    }
    v
}

fn main() {
    let locales = ["en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy"];
    let groupings = [
        ("auto", nc::Grouping::Auto, GroupingStrategy::Auto),
        ("always", nc::Grouping::Always, GroupingStrategy::Always),
        ("never", nc::Grouping::Never, GroupingStrategy::Never),
        ("min2", nc::Grouping::Min2, GroupingStrategy::Min2),
    ];
    let mut total = (0usize, 0usize);
    let mut shown = 0;
    for loc in locales {
        let data = std::fs::read(format!("out/locale/{loc}.all-used.bin")).expect("run locdata first");
        for native in [false, true] {
            let tag = if native {
                match loc {
                    "ar" => "ar-u-nu-arab",
                    "hi" => "hi-u-nu-deva",
                    _ => continue,
                }
            } else {
                loc
            };
            let l = icu_locale_core::Locale::try_from_str(tag).expect("locale");
            let (mut same, mut diff) = (0, 0);
            for (gname, g, gs) in groupings {
                let mut opts = DecimalFormatterOptions::default();
                opts.grouping_strategy = Some(gs);
                let f = DecimalFormatter::try_new((&l).into(), opts).expect("icu");
                for v in values() {
                    let mut e = E::default();
                    let o = [("useGrouping", nc::OptValue::Literal(gname)), ("maximumFractionDigits", nc::OptValue::Literal("6"))];
                    let Some(nv) = nc::resolve(nc::Func::Number, &nc::Operand::Str(&v), &o, &mut e) else { continue };
                    let fd = nc::format_digits(&nv, &mut e);
                    let Some(sym) = numloc::symbols(&data, native) else { panic!("no symbols for {tag}") };
                    let mut ours = S(String::new());
                    numloc::write(&fd, g, &sym, None, "", &mut ours);
                    // icu_decimal on the same rounded digits and sign.
                    let mut d = fixed_decimal::Decimal::new(fixed_decimal::Sign::None, fd.abs.clone());
                    d.sign = match fd.sign {
                        nc::SignOut::Minus => fixed_decimal::Sign::Negative,
                        nc::SignOut::Plus => fixed_decimal::Sign::Positive,
                        nc::SignOut::None => fixed_decimal::Sign::None,
                    };
                    let theirs = writeable::Writeable::write_to_string(&f.format(&d)).into_owned();
                    if ours.0 == theirs {
                        same += 1;
                    } else {
                        diff += 1;
                        if shown < 20 {
                            shown += 1;
                            println!("DIFF {tag} grouping={gname} {v}: ours={:?} icu={:?}", ours.0, theirs);
                        }
                    }
                }
            }
            println!("{tag:<14} {same:>5} identical, {diff} different");
            total.0 += same;
            total.1 += diff;
        }
    }
    println!("TOTAL {} identical, {} different", total.0, total.1);
}
