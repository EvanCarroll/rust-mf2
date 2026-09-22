//! The committed `data/plurals.txt` and `data/directions.txt` are exactly what `cargo xtask
//! locale-data` generates from the vendored CLDR JSON: the table cannot
//! drift from `third_party/cldr-json`.

use std::fs;
use std::path::Path;

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
