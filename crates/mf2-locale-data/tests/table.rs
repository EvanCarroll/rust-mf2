//! The committed `data/` tables against their CLDR sources.
//!
//! * `data/plurals.txt`, `data/directions.txt` and `data/matching.txt` (and
//!   `mf2`'s `src/matching/cldr.rs`, the last packed as `mf2` reads it) are
//!   exactly what `cargo xtask locale-data` generates from the vendored CLDR
//!   JSON: they cannot drift from `third_party/cldr-json`.
//! * `data/numbers.txt`, `data/currencies.txt` and `data/units.txt` cover
//!   every CLDR locale and are generated from the `cargo xtask cldr-sync`
//!   cache (`target/xtask-cache/cldr-json`, never vendored). Offline, it is held to `third_party/` where the vendored
//!   files reach: its digits are exactly `numberingSystems.json`'s, and every
//!   locale of the vendored panel (`en es de fr ar he ja hi ru pl cy`)
//!   resolves, through the table's parent chain, to exactly its record in
//!   the vendored `numbers.json` (and `currencies.json`, `units.json`).
//!   Full regeneration needs the cache: `numbers_table_regenerates_from_the_cache`
//!   and `measure_tables_regenerate_from_the_cache` are `#[ignore]`d for that
//!   reason (run `cargo xtask cldr-sync`, then `cargo test -p
//!   mf2-locale-data --features extract --test table -- --ignored`); `cargo
//!   xtask locale-data` itself checks every locale's round trip when it
//!   writes the table.

use std::fs;
use std::path::{Path, PathBuf};

use mf2_locale_data::extract::{NumberInputs, locale_record, numbers_table, numeric_systems};
use mf2_locale_data::number::Table;

/// The probe locale panel vendored in `third_party/cldr-json` (its PIN).
const PANEL: [&str; 11] = [
    "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn committed(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join(name),
    )
    .expect("committed table")
}

fn vendored(path: &str) -> String {
    fs::read_to_string(root().join("third_party/cldr-json").join(path)).expect("vendored CLDR")
}

#[test]
fn numbers_table_matches_the_vendored_cldr() {
    let text = committed("numbers.txt");
    let table = Table::parse(&text).expect("the table parses");
    assert_eq!(table.cldr, "48.2.1");
    let numeric = numeric_systems(&vendored("cldr-core/supplemental/numberingSystems.json"))
        .expect("numbering systems");
    assert_eq!(
        table
            .digits
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.to_string()))
            .collect::<std::collections::BTreeMap<_, _>>(),
        numeric,
        "digits lines"
    );
    for locale in PANEL {
        let json = vendored(&format!("cldr-numbers-full/main/{locale}/numbers.json"));
        let want = locale_record(locale, &json, &numeric).expect("record");
        assert_eq!(table.resolved(locale), want, "{locale}");
    }
    let likely: serde_json::Value =
        serde_json::from_str(&vendored("cldr-core/supplemental/likelySubtags.json"))
            .expect("likely");
    for locale in PANEL {
        let full = likely
            .pointer(&format!("/supplemental/likelySubtags/{locale}"))
            .and_then(serde_json::Value::as_str)
            .expect("likely subtags");
        assert_eq!(
            table.likely.get(locale).copied(),
            full.split('-').nth(1),
            "likely script of {locale}"
        );
    }
}

/// The whole table from the cache (see the module comment).
#[test]
#[ignore = "needs the cldr-sync cache (target/xtask-cache/cldr-json): run `cargo xtask cldr-sync`"]
fn numbers_table_regenerates_from_the_cache() {
    let cache = root().join("target/xtask-cache/cldr-json/cldr-json");
    let main = cache.join("cldr-numbers-full/main");
    let mut names: Vec<String> = fs::read_dir(&main)
        .expect("the cldr-sync cache")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let locales: Vec<(String, String)> = names
        .into_iter()
        .map(|n| {
            let json = fs::read_to_string(main.join(&n).join("numbers.json")).expect("numbers");
            (n, json)
        })
        .collect();
    let read = |p: &str| fs::read_to_string(cache.join(p)).expect("cache file");
    let inputs = NumberInputs {
        locales: &locales,
        parent_locales: &read("cldr-core/supplemental/parentLocales.json"),
        likely_subtags: &vendored("cldr-core/supplemental/likelySubtags.json"),
        numbering_systems: &vendored("cldr-core/supplemental/numberingSystems.json"),
        available_locales: &read("cldr-core/availableLocales.json"),
        default_content: &read("cldr-core/defaultContent.json"),
    };
    let fresh = numbers_table(&inputs, "48.2.1").expect("generates");
    assert!(
        fresh == committed("numbers.txt"),
        "data/numbers.txt is stale: run `cargo xtask locale-data`"
    );
}

#[test]
fn committed_table_matches_the_vendored_cldr() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cldr = root.join("third_party/cldr-json");
    let pin = fs::read_to_string(cldr.join("PIN")).expect("PIN");
    let tag = pin
        .lines()
        .find_map(|l| l.strip_prefix("tag"))
        .and_then(|l| l.trim().strip_prefix('='))
        .map(str::trim)
        .expect("tag in PIN");
    let read = |name: &str| {
        fs::read_to_string(cldr.join("cldr-core/supplemental").join(name)).expect("CLDR JSON")
    };
    let fresh =
        mf2_locale_data::extract::plurals_table(&read("plurals.json"), &read("ordinals.json"), tag)
            .expect("extracts");
    let committed =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/plurals.txt"))
            .expect("committed table");
    assert!(
        fresh == committed,
        "data/plurals.txt is stale: run `cargo xtask locale-data`"
    );
    let fresh = mf2_locale_data::extract::directions_table(
        &read("likelySubtags.json"),
        &fs::read_to_string(cldr.join("cldr-core/scriptMetadata.json")).expect("scriptMetadata"),
        tag,
    )
    .expect("extracts");
    let committed =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/directions.txt"))
            .expect("committed table");
    assert!(
        fresh == committed,
        "data/directions.txt is stale: run `cargo xtask locale-data`"
    );
    // Language matching: the table the
    // build cuts, and the whole of it as `mf2` carries it.
    let fresh = mf2_locale_data::extract::matching_table(
        &read("likelySubtags.json"),
        &read("languageMatching.json"),
        &read("territoryContainment.json"),
        tag,
    )
    .expect("extracts");
    let committed =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/matching.txt"))
            .expect("committed table");
    assert!(
        fresh == committed,
        "data/matching.txt is stale: run `cargo xtask locale-data`"
    );
    let module = mf2_locale_data::matching::Matching::parse(&fresh)
        .and_then(|m| m.mf2_module())
        .expect("encodes");
    let committed = fs::read_to_string(root.join("crates/mf2/src/matching/cldr.rs"))
        .expect("mf2's committed table");
    assert!(
        module == committed,
        "crates/mf2/src/matching/cldr.rs is stale: run `cargo xtask locale-data`"
    );
    assert_eq!(tag, "48.2.1");
    let v = mf2_locale_data::CLDR_VERSION;
    assert_eq!(format!("{}.{}.{}", v.major, v.minor, v.patch), tag);
}

#[test]
fn directions() {
    use mf2_locale_data::direction;
    use mf2_model::Dir;
    for (tag, want) in [
        ("en", Dir::Ltr),
        ("en-US", Dir::Ltr),
        ("en-XA", Dir::Ltr),
        ("ar", Dir::Rtl),
        ("ar-XB", Dir::Rtl),
        ("ar-EG", Dir::Rtl),
        ("he", Dir::Rtl),
        ("fa", Dir::Rtl),
        ("ur", Dir::Rtl),
        ("yi", Dir::Rtl),
        ("pa", Dir::Ltr),
        ("pa-PK", Dir::Rtl),
        ("pa-Arab", Dir::Rtl),
        ("az", Dir::Ltr),
        ("az-IR", Dir::Rtl),
        ("az-Arab", Dir::Rtl),
        ("sd", Dir::Rtl),
        ("sd-IN", Dir::Ltr),
        ("sd-Deva-PK", Dir::Ltr),
        ("ja", Dir::Ltr),
        ("und", Dir::Ltr),
        ("fr", Dir::Ltr),
        ("AR_eg", Dir::Rtl),
    ] {
        assert_eq!(direction(tag).expect("table"), want, "{tag}");
    }
}

/// Every panel locale resolves, through `data/currencies.txt` or
/// `data/units.txt` (and the parents of `data/numbers.txt`), to exactly its
/// vendored `currencies.json` / `units.json` record, CLDR's fallbacks
/// applied — every key, every field.
fn measure_matches_vendored(
    table: &str,
    kinds: &[&str],
    key_words: impl Fn(&str) -> usize + Copy,
    fields: &[&'static str],
    fallback: impl Fn(&str) -> Option<&'static str> + Copy,
    raw_of: impl Fn(&str) -> mf2_locale_data::extract::Raw,
) {
    use mf2_locale_data::blocks::{Blocks, keys, resolve};
    use mf2_locale_data::extract::effective;

    let numbers = committed("numbers.txt");
    let numbers = Table::parse(&numbers).expect("numbers.txt");
    let blocks = Blocks::index(table, kinds).expect("the table indexes");
    for locale in PANEL {
        let chain_blocks: Vec<_> = numbers
            .chain(locale)
            .into_iter()
            .map(|l| {
                blocks
                    .block(l, key_words, |_, f| fields.contains(&f))
                    .expect("block")
            })
            .collect();
        let chain: Vec<_> = chain_blocks.iter().collect();
        let raw = raw_of(locale);
        let mut have = std::collections::BTreeSet::new();
        for kind in kinds {
            have.extend(keys(&chain, kind));
        }
        let want: std::collections::BTreeSet<&str> = raw.keys().map(String::as_str).collect();
        assert_eq!(have, want, "{locale}: keys");
        for (key, f) in &raw {
            assert_eq!(
                resolve(&chain, key, fields, &fallback),
                effective(f, fields, fallback),
                "{locale} {key}"
            );
        }
    }
}

#[test]
fn currencies_table_matches_the_vendored_cldr() {
    let table = committed("currencies.txt");
    assert!(table.lines().any(|l| l == "cldr\t48.2.1"));
    measure_matches_vendored(
        &table,
        &["currency"],
        |_| 1,
        mf2_locale_data::currency::FIELDS,
        mf2_locale_data::currency::fallback,
        |l| {
            let json = vendored(&format!("cldr-numbers-full/main/{l}/currencies.json"));
            mf2_locale_data::extract::currency_raw(l, &json).expect("record")
        },
    );
}

#[test]
fn units_table_matches_the_vendored_cldr() {
    let table = committed("units.txt");
    assert!(table.lines().any(|l| l == "cldr\t48.2.1"));
    let mut fields = vec!["pattern"];
    fields.extend_from_slice(mf2_locale_data::unit::FIELDS);
    measure_matches_vendored(
        &table,
        &["per", "unit"],
        |kind| if kind == "unit" { 2 } else { 1 },
        &fields,
        mf2_locale_data::unit::fallback,
        |l| {
            let json = vendored(&format!("cldr-units-full/main/{l}/units.json"));
            let mut categories = std::collections::BTreeMap::new();
            mf2_locale_data::extract::unit_raw(l, &json, &mut categories).expect("record")
        },
    );
}

/// The currency and unit tables from the cache (see the module comment).
#[test]
#[ignore = "needs the cldr-sync cache (target/xtask-cache/cldr-json): run `cargo xtask cldr-sync`"]
fn measure_tables_regenerate_from_the_cache() {
    let cache = root().join("target/xtask-cache/cldr-json/cldr-json");
    let numbers_main = cache.join("cldr-numbers-full/main");
    let mut names: Vec<String> = fs::read_dir(&numbers_main)
        .expect("the cldr-sync cache")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let numbers = committed("numbers.txt");
    let fresh = mf2_locale_data::extract::currencies_table(
        &numbers_main,
        &names,
        &vendored("cldr-core/supplemental/currencyData.json"),
        &numbers,
        "48.2.1",
    )
    .expect("generates");
    assert!(
        fresh == committed("currencies.txt"),
        "data/currencies.txt is stale: run `cargo xtask locale-data`"
    );
    let fresh = mf2_locale_data::extract::units_table(
        &cache.join("cldr-units-full/main"),
        &names,
        &numbers,
        "48.2.1",
    )
    .expect("generates");
    assert!(
        fresh == committed("units.txt"),
        "data/units.txt is stale: run `cargo xtask locale-data`"
    );
}
