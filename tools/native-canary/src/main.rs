//! What `cargo xtask native-canaries` reads the symbols of: one call site
//! per function family, each behind the feature that serves it, formatted in
//! a named language so that nothing but the row's features is linked.

mf2::include_generated!();

fn main() {
    let locale = language();
    let mut out = String::new();
    out.push_str(&locale.format(&tr!("plain")));
    out.push_str(&locale.format(&tr!("items", count = 3)));
    out.push_str(&locale.format(&tr!("greeting", name = "Ada")));
    #[cfg(all(
        any(feature = "native-datetime-iso", feature = "native-datetime-icu"),
        not(feature = "no-date-message")
    ))]
    if let Some(when) = mf2::DateTimeValue::instant(1_767_225_600_000) {
        out.push_str(&locale.format(&tr!("published", when = when)));
    }
    // `compile`: a message written at run time, which is the only thing that
    // puts the normalization tables in a binary (`plan/01` §4.3).
    #[cfg(feature = "compile")]
    if let Ok(compiled) = mf2::compile_str("hello", "en") {
        std::hint::black_box(&compiled.catalog);
    }
    // `tui` and `ratatui`: a message drawn as a Ratatui `Line` — made from the
    // formatted `String`, or converted by `mf2::ratatui`.
    #[cfg(all(feature = "tui", not(feature = "ratatui")))]
    let line = ratatui_core::text::Line::from(locale.format(&tr!("plain")));
    #[cfg(feature = "ratatui")]
    let line: ratatui_core::text::Line<'static> = tr!("plain").into();
    #[cfg(feature = "tui")]
    std::hint::black_box(line.width());
    std::hint::black_box(out.len());
}

/// The language: English, or with `cli` the one `--lang` names — parsed by
/// the application from text, or with `clap` by `mf2`'s value parser.
fn language() -> Locale {
    #[cfg(all(feature = "cli", not(feature = "clap")))]
    {
        let matches = clap::Command::new("native-canary")
            .arg(clap::Arg::new("lang").long("lang"))
            .get_matches();
        matches
            .get_one::<String>("lang")
            .and_then(|tag| tag.parse::<Locale>().ok())
            .unwrap_or(Locale::En)
    }
    #[cfg(feature = "clap")]
    {
        let matches = clap::Command::new("native-canary")
            .arg(
                clap::Arg::new("lang")
                    .long("lang")
                    .value_parser(clap::value_parser!(Locale)),
            )
            .get_matches();
        matches
            .get_one::<Locale>("lang")
            .copied()
            .unwrap_or(Locale::En)
    }
    #[cfg(not(feature = "cli"))]
    {
        Locale::En
    }
}
