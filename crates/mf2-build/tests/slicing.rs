//! Locale data, sliced over a corpus (Phase 5a, A6).
//!
//! `mf2::compile_str` computes one message's needs; `mf2-build` computes a
//! locale's, over its *flattened* message set. This holds the second to the
//! first — the corpus's number needs are exactly the union of its messages' —
//! and measures budget B8 on the reference workload and on a corpus that uses
//! every numeric function.

mod common;

use std::path::Path;

use common::{out_dir, workload};
use mf2_build::loader::Loader;
use mf2_build::{Build, Config, Features, catalog};
use mf2_catalog::format::locale_key;
use mf2_locale_data::number::NumberNeeds;

/// The 11 locales `third_party/cldr-json` vendors in full
/// (`plans/11-phase-4-work-order.md`), which is what B8 is stated over.
const PANEL: [&str; 11] = [
    "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
];

/// A corpus that uses every numeric function there is, so that the entries
/// it needs are the widest a real one could ask for.
const EVERY_NUMERIC: &str = "\
plain = Save
count = {$n :number}
whole = {$n :integer}
shifted = {$n :offset subtract=1}
share = {$n :percent}
price = {$n :currency currency=EUR}
price2 = {$n :currency currency=USD currencyDisplay=name}
price3 = {$n :currency currency=JPY currencySign=accounting}
price4 = {$n :currency currency=GBP currencyDisplay=narrowSymbol}
far = {$n :unit unit=kilometer}
heavy = {$n :unit unit=kilogram unitDisplay=long}
fast = {$n :unit unit=kilometer-per-hour}
cardinal =
  .input {$n :integer}
  .match $n
  one {{one}}
  *   {{many}}
ordinal =
  .input {$n :integer select=ordinal}
  .match $n
  one {{first}}
  *   {{nth}}
unannotated = You have {$n}
";

/// Writes a one-file corpus for every locale of `tags`, all with the same
/// messages, and builds it.
fn build_panel(name: &str, body: &str, tags: &[&str], features: &Features) -> mf2_build::Outcome {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slicing")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    for tag in tags {
        let path = root.join("locales").join(tag).join("main.mf2");
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&path, format!("@locale {tag}\n---\n\n{body}")).expect("write");
    }
    let mut config = Config {
        source_locale: tags[0].to_owned(),
        ..Config::default()
    };
    // Every locale here has the same messages; the plural variants are the
    // source's, which other locales' categories do not match.
    config.lints.insert(
        mf2_build::Lint::MissingPluralCategory,
        mf2_build::Level::Allow,
    );
    let outcome = Build::at(&root, out_dir(&format!("slicing-{name}")))
        .config(config)
        .features(features.clone())
        .check()
        .expect("the corpus is readable");
    assert!(
        outcome.report.is_clean(),
        "{name}:\n{}",
        outcome.report.to_text()
    );
    outcome
}

#[test]
fn a_locales_number_needs_are_the_union_of_its_messages() {
    // What `mf2-build` slices for the corpus...
    let features = Features::parse("fn-number");
    let outcome = build_panel("union", EVERY_NUMERIC, &["en"], &features);
    let catalog = outcome.catalog("en").expect("en");
    let corpus = &catalog.slice.needs.numbers;

    // ...and what `compile_str` would compute, message by message, summed.
    let mut union = NumberNeeds::default();
    for source in EVERY_NUMERIC.lines().filter_map(|l| l.split_once(" = ")) {
        if let Some(model) = mf2_syntax::parse_model(source.1).message {
            union.add_message(&model);
        }
    }
    // The `.match` messages are multi-line; add them from the built corpus's
    // own models instead of re-splitting the text.
    for record in &mf2_build::loader::resource::Resources
        .load(
            &Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join("slicing")
                .join("union")
                .join("locales")
                .join("en"),
        )
        .expect("the locale")
        .records
    {
        if let Some(model) = mf2_syntax::parse_model(&record.source).message {
            union.add_message(&model);
        }
    }

    assert_eq!(corpus.symbols, union.symbols);
    assert_eq!(corpus.percent, union.percent);
    assert_eq!(
        corpus.currency.as_ref().map(|c| c.codes.clone()),
        union.currency.as_ref().map(|c| c.codes.clone()),
        "the currencies differ"
    );
    assert_eq!(
        corpus.unit.as_ref().map(|u| u.ids.clone()),
        union.unit.as_ref().map(|u| u.ids.clone()),
        "the units differ"
    );
    assert_eq!(
        corpus
            .currency
            .as_ref()
            .map(|c| (c.accounting, c.hidden, c.narrow, c.names)),
        union
            .currency
            .as_ref()
            .map(|c| (c.accounting, c.hidden, c.narrow, c.names)),
        "the currency displays differ"
    );
}

#[test]
fn a_corpus_that_formats_no_numbers_carries_no_number_data() {
    let features = Features::parse("fn-number");
    let outcome = build_panel(
        "text-only",
        "plain = Save\nother = Cancel\n",
        &["en"],
        &features,
    );
    let catalog = outcome.catalog("en").expect("en");
    assert!(
        catalog.locale_entries.is_empty(),
        "a corpus of plain text carries {:?}",
        catalog.locale_entries
    );
}

#[test]
fn without_fn_number_no_number_entry_is_carried() {
    // The client formats digits with neutral symbols, so the locale's would
    // never be read (and `check` warns `neutral-numbers`).
    let outcome = build_panel(
        "no-fn-number",
        "count = {$n :number}\nunannotated = You have {$n}\n",
        &["pl"],
        &Features::default(),
    );
    let catalog = outcome.catalog("pl").expect("pl");
    assert!(
        catalog
            .locale_entries
            .iter()
            .all(|(key, _)| *key != locale_key::NUMBER_SYMBOLS),
        "{:?}",
        catalog.locale_entries
    );
}

#[test]
fn only_the_plural_kinds_the_selectors_use_are_carried() {
    let features = Features::parse("fn-number");
    let cardinal = build_panel(
        "cardinal",
        "x =\n  .input {$n :integer}\n  .match $n\n  one {{one}}\n  * {{many}}\n",
        &["pl"],
        &features,
    );
    let keys: Vec<u32> = cardinal
        .catalog("pl")
        .expect("pl")
        .locale_entries
        .iter()
        .map(|(k, _)| *k)
        .collect();
    assert!(keys.contains(&locale_key::PLURAL_CARDINAL), "{keys:?}");
    assert!(!keys.contains(&locale_key::PLURAL_ORDINAL), "{keys:?}");

    let ordinal = build_panel(
        "ordinal",
        "x =\n  .input {$n :integer select=ordinal}\n  .match $n\n  one {{1st}}\n  * {{nth}}\n",
        &["pl"],
        &features,
    );
    let keys: Vec<u32> = ordinal
        .catalog("pl")
        .expect("pl")
        .locale_entries
        .iter()
        .map(|(k, _)| *k)
        .collect();
    assert!(keys.contains(&locale_key::PLURAL_ORDINAL), "{keys:?}");
    assert!(!keys.contains(&locale_key::PLURAL_CARDINAL), "{keys:?}");
}

#[test]
fn a_select_from_a_variable_carries_both_rule_sets() {
    // Nothing can know which rules it asks for until it runs, so the catalog
    // carries both — which is why `check` says `dynamic-select` costs.
    let mut config = Config::default();
    config
        .lints
        .insert(mf2_build::Lint::DynamicSelect, mf2_build::Level::Allow);
    config.lints.insert(
        mf2_build::Lint::MissingPluralCategory,
        mf2_build::Level::Allow,
    );
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slicing")
        .join("dynamic-select");
    let _ = std::fs::remove_dir_all(&root);
    let path = root.join("locales/pl/main.mf2");
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    std::fs::write(
        &path,
        "@locale pl\n---\n\nx =\n  .input {$n :integer select=$how}\n  .match $n\n  one {{one}}\n  * {{many}}\n",
    )
    .expect("write");
    "pl".clone_into(&mut config.source_locale);
    let outcome = Build::at(&root, out_dir("slicing-dynamic-select"))
        .config(config)
        .features(Features::parse("fn-number"))
        .check()
        .expect("readable");
    assert!(outcome.report.is_clean(), "{}", outcome.report.to_text());
    let keys: Vec<u32> = outcome
        .catalog("pl")
        .expect("pl")
        .locale_entries
        .iter()
        .map(|(k, _)| *k)
        .collect();
    assert!(keys.contains(&locale_key::PLURAL_CARDINAL), "{keys:?}");
    assert!(keys.contains(&locale_key::PLURAL_ORDINAL), "{keys:?}");
}

/// Budget B8: plural + number symbols ≤ 0.5 KB gz per locale
/// (`plans/06-size-and-perf.md` §3).
#[test]
fn b8_plural_and_number_symbols_stay_under_half_a_kilobyte() {
    const LIMIT: usize = 512;
    let features = Features::parse("fn-number");

    // On the reference workload's four locales.
    let outcome = Build::at(workload(), out_dir("slicing-b8-workload"))
        .features(features.clone())
        .check()
        .expect("the workload builds");
    for catalog in &outcome.catalogs {
        let bytes = b8_bytes(catalog);
        assert!(
            bytes <= LIMIT,
            "{}: plural + number symbols are {bytes} B (B8 is {LIMIT})",
            catalog.tag
        );
        eprintln!("B8 workload {:>6}: {bytes:>4} B", catalog.tag);
    }

    // And on a corpus that uses every numeric function, over the panel.
    let panel = build_panel("b8-panel", EVERY_NUMERIC, &PANEL, &features);
    for catalog in &panel.catalogs {
        let bytes = b8_bytes(catalog);
        assert!(
            bytes <= LIMIT,
            "{}: plural + number symbols are {bytes} B (B8 is {LIMIT})",
            catalog.tag
        );
        let total: usize = catalog.locale_entries.iter().map(|(_, n)| n).sum();
        eprintln!(
            "B8 panel    {:>6}: {bytes:>4} B of plural + symbols, {total:>6} B of locale data",
            catalog.tag
        );
    }
}

/// What B8 counts: the plural rules and the number symbols, raw.
///
/// The budget is stated on gzip, and these entries are tens of bytes —
/// smaller than a gzip header — so the raw size is the honest measure and the
/// conservative one: gzipping them could only make the figure look better.
fn b8_bytes(catalog: &catalog::Catalog) -> usize {
    catalog
        .locale_entries
        .iter()
        .filter(|(key, _)| {
            matches!(
                *key,
                locale_key::PLURAL_CARDINAL
                    | locale_key::PLURAL_ORDINAL
                    | locale_key::NUMBER_SYMBOLS
            )
        })
        .map(|(_, bytes)| bytes)
        .sum()
}
