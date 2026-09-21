//! Comparison: core semantics + icu_decimal (data from a runtime blob) as the
//! localization backend instead of numloc (decimal only; no percent).
use wasm_bindgen::prelude::*;
include!("../../opts.rs");

#[wasm_bindgen]
pub fn fmt(data: &[u8], locale: &str, func_code: u8, opts: &str, operand: &str, key: &str) -> String {
    let mut buf = [("", numcore::OptValue::Literal("")); 8];
    let n = parse_opts(opts, &mut buf);
    let o = buf.get(..n).unwrap_or(&[]);
    let mut e = Errs(0);
    let mut out = String::new();
    let op = match operand.strip_prefix('#') {
        Some(d) => numcore::Operand::Int(d.bytes().fold(0i64, |a, c| a * 10 + i64::from(c & 15))),
        None => numcore::Operand::Str(operand),
    };
    if let Some(v) = numcore::resolve(func(func_code), &op, o, &mut e) {
        let f = numcore::format_digits(&v, &mut e);
        let mut d = fixed_decimal::Decimal::new(fixed_decimal::Sign::None, f.abs.clone());
        d.sign = match f.sign {
            numcore::SignOut::Minus => fixed_decimal::Sign::Negative,
            numcore::SignOut::Plus => fixed_decimal::Sign::Positive,
            numcore::SignOut::None => fixed_decimal::Sign::None,
        };
        if let (Ok(loc), Ok(p)) = (
            icu_locale_core::Locale::try_from_str(locale),
            icu_provider_blob::BlobDataProvider::try_new_from_blob(data.into()),
        ) {
            let mut opts = icu_decimal::options::DecimalFormatterOptions::default();
            opts.grouping_strategy = Some(match v.opts.use_grouping {
                Some(numcore::Grouping::Never) => icu_decimal::options::GroupingStrategy::Never,
                Some(numcore::Grouping::Min2) => icu_decimal::options::GroupingStrategy::Min2,
                _ => icu_decimal::options::GroupingStrategy::Auto,
            });
            if let Ok(fm) = icu_decimal::DecimalFormatter::try_new_with_buffer_provider(&p, (&loc).into(), opts) {
                let _ = writeable::Writeable::write_to(&fm.format(&d), &mut out);
            }
        }
        e.0 ^= select_bits(&v, &f, key);
    }
    out.push(char::from_u32(0x100 + (e.0 & 0xffff)).unwrap_or('?'));
    out
}
