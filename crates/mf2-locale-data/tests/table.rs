//! The committed `data/` tables against their CLDR sources.
//!
//! * `data/plurals.txt` and `data/directions.txt` are exactly what `cargo
//!   xtask locale-data` generates from the vendored CLDR JSON: they cannot
//!   drift from `third_party/cldr-json`.
//! * `data/numbers.txt` covers every CLDR locale and is generated from the
//!   `cargo xtask cldr-sync` cache (`target/xtask-cache/cldr-json`, never
//!   vendored). Offline, it is held to `third_party/` where the vendored
//!   files reach: its digits are exactly `numberingSystems.json`'s, and every
//!   locale of the vendored panel (`en es de fr ar he ja hi ru pl cy`)
//!   resolves, through the table's parent chain, to exactly its record in
//!   the vendored `numbers.json`. Full regeneration needs the cache:
//!   `numbers_table_regenerates_from_the_cache` is `#[ignore]`d for that
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
