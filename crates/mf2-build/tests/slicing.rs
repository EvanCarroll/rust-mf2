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
    build_panel_with(name, body, tags, features, Config::default())
}

/// [`build_panel`] with `config` in place of the defaults.
fn build_panel_with(
    name: &str,
    body: &str,
    tags: &[&str],
    features: &Features,
    mut config: Config,
) -> mf2_build::Outcome {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slicing")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    for tag in tags {
        let path = root.join("locales").join(tag).join("main.mf2");
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&path, format!("@locale {tag}\n---\n\n{body}")).expect("write");
    }
    tags[0].clone_into(&mut config.source_locale);
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

/// The unannotated hooks (`with_numbers` / `with_dates`, #90) belong in the
/// registry only when a placeholder can actually receive a raw number or
/// date. A declaration annotates the variable before the pattern sees it, so
/// `{$n}` under `.input {$n :integer}` never reaches them — and a corpus that
/// annotates everything through declarations should pay nothing for them
/// (B1′).
#[test]
fn a_declaration_annotates_the_placeholders_that_use_it() {
    let cases: [(&str, bool); 5] = [
        // Bare, undeclared: an external argument with no function.
        ("bare = You have {$n} left", true),
        // Declared with a function: resolved before the pattern.
        (
            "declared =\n  .input {$n :integer}\n  {{You have {$n} left}}",
            false,
        ),
        // `.local $m = {$n}` renames; the chain still ends at `:integer`.
        (
            "renamed =\n  .input {$n :integer}\n  .local $m = {$n}\n  {{You have {$m} left}}",
            false,
        ),
        // Declared with no function at all: still unannotated.
        ("plain =\n  .input {$n}\n  {{You have {$n} left}}", true),
        // A literal is text, never a number.
        ("literal = You have {|5|} left", false),
    ];
    for (source, expected) in cases {
        let root = common::corpus(
            &format!("unannotated-{}", source.split(' ').next().unwrap_or("x")),
            "source_locale = \"en\"\n",
            &[("en", source)],
        );
        let outcome = Build::at(&root, out_dir("unannotated"))
            .features(Features::parse("fn-number,host-std-datetime-iso"))
            .run()
            .expect("builds");
        assert!(outcome.report.is_clean(), "{}", outcome.report.to_text());
        let slice = &outcome.catalogs[0].slice;
        assert_eq!(
            slice.unannotated, expected,
            "unannotated should be {expected} for:\n{source}"
        );
        // What the flag is for: the hooks appear in the generated registry
        // exactly when it is set.
        assert_eq!(
            outcome.generated.contains("with_numbers"),
            expected,
            "the registry disagrees with the slice for:\n{source}"
        );
    }
}

#[test]
fn an_explicit_list_narrows_a_variable_currency() {
    use mf2_build::{DataSet, Lint};
    use mf2_locale_data::number::Selection;

    let features = Features::parse("fn-number");
    let body = "price = {$n :currency currency=$code}\n";
    let every = build_panel("dynamic-every", body, &["en"], &features);

    let mut config = Config::default();
    let listed: std::collections::BTreeSet<String> =
        ["EUR", "USD"].iter().map(|c| (*c).to_owned()).collect();
    config.locale_data.currencies = DataSet::Listed(listed.clone());
    let narrow = build_panel_with("dynamic-listed", body, &["en"], &features, config);

    let codes = |o: &mf2_build::Outcome| {
        o.catalog("en")
            .expect("en")
            .slice
            .needs
            .numbers
            .currency
            .as_ref()
            .map(|c| c.codes.clone())
    };
    assert_eq!(codes(&every), Some(Selection::All));
    assert_eq!(codes(&narrow), Some(Selection::Listed(listed)));

    let size = |o: &mf2_build::Outcome| {
        o.catalog("en")
            .expect("en")
            .locale_entries
            .iter()
            .find(|(k, _)| *k == locale_key::CURRENCY_DATA)
            .map(|(_, n)| *n)
            .expect("a currency entry")
    };
    assert!(
        size(&narrow) * 10 < size(&every),
        "listed {} bytes, every {} bytes",
        size(&narrow),
        size(&every)
    );

    // Following the warning's advice silences it.
    let warns = |o: &mf2_build::Outcome| {
        o.report
            .diagnostics
            .iter()
            .any(|d| d.lint == Some(Lint::DynamicCurrency))
    };
    assert!(warns(&every));
    assert!(!warns(&narrow));
}

// `plan/08` §4.1, §4.2: the date slice goes where it is read.

/// A corpus that formats a date, in two locales.
#[cfg(feature = "icu-blob")]
const DATES: &str = "plain = Save\nwhen = {$d :date}\n";

/// [`DATES`] built and written (`Build::run`) under `features` and `emit`,
/// and the directory it was written to.
#[cfg(feature = "icu-blob")]
fn build_dates(
    name: &str,
    features: &str,
    emit: mf2_build::Emit,
) -> (mf2_build::Outcome, std::path::PathBuf) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slicing")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    for tag in ["en", "fr"] {
        let path = root.join("locales").join(tag).join("main.mf2");
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&path, format!("@locale {tag}\n---\n\n{DATES}")).expect("write");
    }
    let mut config = Config::default();
    "en".clone_into(&mut config.source_locale);
    let out = out_dir(&format!("slicing-{name}"));
    let outcome = Build::at(&root, &out)
        .config(config)
        .features(Features::parse(features))
        .emit(emit)
        .run()
        .expect("the corpus builds");
    assert!(
        outcome.report.is_clean(),
        "{name}:\n{}",
        outcome.report.to_text()
    );
    (outcome, out)
}

#[cfg(feature = "icu-blob")]
#[test]
fn icu4x_on_the_server_alone_keeps_the_slice_out_of_the_browsers_catalog() {
    use mf2_build::Emit;
    let (split, out) = build_dates(
        "dates-server-icu",
        "ssr,leptos-client-datetime-intl,leptos-server-datetime-icu",
        Emit::Both,
    );
    // The same application with no date formatter in native code: its
    // browser build (a server build without one fails `:date`).
    let (plain, _) = build_dates(
        "dates-no-native",
        "hydrate,leptos-client-datetime-intl",
        Emit::Both,
    );
    for catalog in &split.catalogs {
        let tag = &catalog.tag;
        assert!(
            catalog
                .locale_entries
                .iter()
                .all(|(key, _)| *key != locale_key::ICU_BLOB),
            "{tag}: the browser's catalog carries entry 48: {:?}",
            catalog.locale_entries
        );
        let same = plain.catalog(tag).expect("the same locale");
        assert!(
            same.server.is_empty(),
            "{tag}: no native formatter, no table"
        );
        assert_eq!(
            catalog.bytes, same.bytes,
            "{tag}: not the catalog built with no date formatter in native code"
        );
        assert_eq!(catalog.file_name(), same.file_name());
        // The slice is in the server-only table, written beside the catalog
        // and named in the module.
        assert!(
            catalog
                .server_entries
                .iter()
                .any(|(key, n)| *key == locale_key::ICU_BLOB && *n > 0),
            "{tag}: {:?}",
            catalog.server_entries
        );
        let name = catalog.server_file_name().expect("a server-only table");
        assert_eq!(
            std::fs::read(out.join(&name)).expect("written beside the catalog"),
            catalog.server
        );
        assert!(
            split.generated.contains(&name),
            "{tag}: the module embeds {name}"
        );
        // The server reads the slice through the table.
        let table: &'static [u8] = Box::leak(catalog.server.clone().into_boxed_slice());
        let reader = mf2_catalog::Catalog::new(catalog.bytes.clone(), split.manifest_hash)
            .expect("the catalog loads");
        assert!(reader.locale_entry(locale_key::ICU_BLOB).is_none());
        let reader = reader.with_server_data(table).expect("the table loads");
        assert!(
            reader
                .locale_entry(locale_key::ICU_BLOB)
                .is_some_and(|b| !b.is_empty())
        );
    }
    // A site holds what a browser reads and nothing else.
    let site = out_dir("slicing-dates-server-icu-site");
    split.publish(&site).expect("published");
    let names: Vec<String> = std::fs::read_dir(&site)
        .expect("the site")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|n| !n.ends_with(".server")),
        "a site holds a server-only table: {names:?}"
    );
}

#[cfg(feature = "icu-blob")]
#[test]
fn icu4x_in_the_browser_keeps_the_slice_in_the_catalog() {
    use mf2_build::Emit;
    for (name, features, emit) in [
        (
            "dates-browser-icu",
            "ssr,leptos-client-datetime-icu,leptos-server-datetime-icu",
            Emit::Both,
        ),
        // A native application: one reader, one file.
        (
            "dates-native-icu",
            "native,native-datetime-icu",
            Emit::Native,
        ),
    ] {
        let (outcome, out) = build_dates(name, features, emit);
        for catalog in &outcome.catalogs {
            assert!(
                catalog
                    .locale_entries
                    .iter()
                    .any(|(key, _)| *key == locale_key::ICU_BLOB),
                "{name} {}: {:?}",
                catalog.tag,
                catalog.locale_entries
            );
            assert!(catalog.server.is_empty(), "{name} {}", catalog.tag);
            assert!(catalog.server_entries.is_empty(), "{name} {}", catalog.tag);
            assert_eq!(catalog.server_file_name(), None);
        }
        let tables: Vec<_> = std::fs::read_dir(&out)
            .expect("the output")
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".server"))
            .collect();
        assert!(tables.is_empty(), "{name}: {tables:?}");
        assert!(!outcome.generated.contains("MF2_SERVER_DATA"), "{name}");
    }
}
