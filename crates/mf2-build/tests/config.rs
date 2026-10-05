//! `mf2.toml` (Phase 5a, A3): every key of `mf2.toml` read,
//! and anything else refused with the key named.

use std::collections::BTreeSet;
use std::path::Path;

use mf2_build::{Config, DataSet, Error, Level, Lint, Missing, Strip};

fn parse(text: &str) -> Config {
    Config::parse(text, Path::new("mf2.toml")).expect("a valid configuration")
}

fn error(text: &str) -> String {
    match Config::parse(text, Path::new("mf2.toml")) {
        Err(Error::Config { message, .. }) => message,
        Err(other) => panic!("wrong error: {other}"),
        Ok(_) => panic!("expected an error from:\n{text}"),
    }
}

const FULL: &str = r#"
source_locale = "en"

[fallback]
"es-MX" = ["es", "en"]

[catalog]
strip = ["cold", "ids"]
missing = "fallback"

[locale_data]
currencies = "used"
units = ["USD", "EUR"]

[lints]
plain-numbers = "warn"
unpaired-markup = "error"
unused-id = "allow"

[functions]
"app:emoji" = "my_app_i18n::functions::emoji"
"#;

#[test]
fn every_key_of_the_plan_is_read() {
    let config = parse(FULL);
    assert_eq!(config.source_locale, "en");
    assert_eq!(config.fallback["es-MX"], ["es", "en"]);
    assert_eq!(
        config.catalog.strip,
        [Strip::Cold, Strip::Ids]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(config.catalog.missing, Missing::Fallback);
    assert_eq!(config.locale_data.currencies, DataSet::Used);
    assert_eq!(
        config.locale_data.units,
        DataSet::Listed(["USD".to_owned(), "EUR".to_owned()].into_iter().collect())
    );
    assert_eq!(config.level(Lint::PlainNumbers), Level::Warn);
    assert_eq!(config.level(Lint::UnpairedMarkup), Level::Error);
    assert_eq!(config.level(Lint::UnusedId), Level::Allow);
    // A lint nobody configured keeps its default.
    assert_eq!(
        config.level(Lint::MissingTranslation),
        Lint::MissingTranslation.default_level()
    );
    assert_eq!(
        config.functions["app:emoji"],
        "my_app_i18n::functions::emoji"
    );
}

#[test]
fn the_defaults_are_the_documented_ones() {
    let config = parse("");
    assert_eq!(config, Config::default());
    assert_eq!(config.source_locale, "en");
    // Production client catalogs.
    assert!(config.catalog.strip.contains(&Strip::Cold));
    assert!(config.catalog.strip.contains(&Strip::Ids));
    assert_eq!(config.catalog.missing, Missing::Fallback);
    assert_eq!(config.locale_data.currencies, DataSet::Used);
    assert_eq!(config.level(Lint::GatedFunction), Level::Error);
    assert_eq!(config.level(Lint::PlainNumbers), Level::Warn);
}

#[test]
fn an_unknown_key_names_itself() {
    let message = error("source_local = \"en\"\n");
    assert!(message.contains("source_local"), "{message}");
    assert!(message.starts_with("1:1: "), "no position: {message}");

    let message = error("[catalog]\nstrip = [\"cold\"]\nmising = \"id\"\n");
    assert!(message.contains("mising"), "{message}");
    assert!(message.starts_with("3:1: "), "no position: {message}");

    let message = error("[lints]\nnoisy-numbers = \"warn\"\n");
    assert!(message.contains("noisy-numbers"), "{message}");
}

#[test]
fn a_wrong_value_names_what_was_expected() {
    let message = error("[catalog]\nmissing = \"whatever\"\n");
    assert!(message.contains("whatever"), "{message}");
    let message = error("[catalog]\nstrip = [\"names\"]\n");
    assert!(message.contains("names"), "{message}");
    let message = error("[locale_data]\ncurrencies = \"most\"\n");
    assert!(message.contains("most"), "{message}");
    let message = error("[lints]\nplain-numbers = \"shout\"\n");
    assert!(message.contains("shout"), "{message}");
}

#[test]
fn a_lint_the_build_relies_on_cannot_be_turned_down() {
    let message = error("[lints]\ngated-function = \"warn\"\n");
    assert!(message.contains("gated-function"), "{message}");
    assert!(message.contains("cannot be set below"), "{message}");
    // One that is configurable, on the other hand, may go all the way down:
    // custom functions are legal.
    let config = parse("[lints]\nunknown-function = \"allow\"\n");
    assert_eq!(config.level(Lint::UnknownFunction), Level::Allow);
}

#[test]
fn a_locale_may_not_fall_back_to_itself() {
    let message = error("[fallback]\n\"es\" = [\"es\"]\n");
    assert!(message.contains("itself"), "{message}");
}

#[test]
fn a_chain_ends_at_the_source_locale() {
    let config = parse(FULL);
    assert_eq!(config.chain("es-MX"), ["es", "en"]);
    // Not configured: the tag's own parents, then the source locale.
    assert_eq!(config.chain("de-AT"), ["de", "en"]);
    assert_eq!(config.chain("pl"), ["en"]);
    // The source locale falls back to nothing.
    assert!(config.chain("en").is_empty());
}

#[test]
fn what_init_writes_reads_back_as_itself() {
    let config = parse(FULL);
    let written = config.to_toml();
    assert_eq!(
        Config::parse(&written, Path::new("mf2.toml")).expect("what we wrote"),
        config,
        "wrote:\n{written}"
    );
    assert!(written.contains("source_locale = \"en\""), "{written}");
}

#[test]
fn a_missing_file_is_the_defaults() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("no-config");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let _ = std::fs::remove_file(dir.join("mf2.toml"));
    assert_eq!(Config::load(&dir).expect("defaults"), Config::default());
}
