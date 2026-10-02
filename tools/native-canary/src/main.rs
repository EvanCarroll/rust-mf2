//! What `cargo xtask native-canaries` reads the symbols of: one call site
//! per function family, each behind the feature that serves it, formatted in
//! a named language so that nothing but the row's features is linked.

mf2::include_generated!();

fn main() {
    let locale = Locale::En;
    let mut out = String::new();
    out.push_str(&locale.format(&tr!("plain")));
    out.push_str(&locale.format(&tr!("items", count = 3)));
    #[cfg(feature = "fn-datetime")]
    if let Some(when) = mf2::DateTimeValue::instant(1_767_225_600_000) {
        out.push_str(&locale.format(&tr!("published", when = when)));
    }
    std::hint::black_box(out.len());
}
