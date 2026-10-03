//! Both canonical-equivalence paths agree (`plan/01` §4.3): the check the
//! runtime now makes from the map a catalog carries, and the one it used to
//! make through `Host::nfc` and the full normalization tables.
//!
//! The values are the suite's own strings — every message source, expected
//! result and parameter value, which is what a program passes in — and the
//! key sets are those the gate asks for plus the suite's own non-ASCII
//! strings. `Host::nfc` is the oracle while it still exists; once it is gone
//! the fuzz target (`fuzz/fuzz_targets/nfc.rs`) keeps the check honest
//! against `unicode-normalization` directly.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mf2::Host;
use mf2::host_std::HOST;
use mf2_catalog::nfc_map::NfcMap;
use mf2_catalog::writer::nfc_map::build;
use mf2_conformance::load_suite;
use mf2_runtime::nfc_equivalent;
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

/// The old path: the host's NFC. A catalog's keys and names are written in
/// NFC, so the comparison was "the value's NFC form equals the key".
fn nfc(s: &str) -> String {
    let mut buf = String::new();
    HOST.nfc(s, &mut buf).to_string()
}

/// Every string in a JSON value, keys included.
fn strings(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::String(s) => {
            out.insert(s.clone());
        }
        Value::Array(a) => {
            for x in a {
                strings(x, out);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                out.insert(k.clone());
                strings(x, out);
            }
        }
        _ => {}
    }
}

/// The suite's strings: sources, expected results and parameters.
fn suite_strings() -> Vec<String> {
    let suite = load_suite(&root()).expect("the suite loads");
    let mut set = BTreeSet::new();
    for t in suite.tests() {
        set.insert(t.src.clone());
        if let Some(e) = &t.exp {
            set.insert(e.clone());
        }
        if let Some(p) = &t.params {
            strings(p, &mut set);
        }
        if let Some(p) = &t.exp_parts {
            strings(p, &mut set);
        }
    }
    set.into_iter().collect()
}

/// The key sets of the gate (`plan/04` 13.1): ASCII only (an empty map),
/// Latin precomposed and combining, marks in both orders, Greek with tonos,
/// Hangul, and a singleton decomposition.
const SETS: &[&[&str]] = &[
    &[],
    &["h", "s", "q", "one", "other"],
    &["caf\u{e9}", "cafe\u{301}", "\u{c5}ngstr\u{f6}m"],
    &["q\u{323}\u{307}", "q\u{307}\u{323}"],
    &["\u{3ac}", "\u{3b1}\u{301}", "\u{1f70}"],
    &["\u{ac01}", "\u{ac00}", "\u{d4db}"],
    &["\u{212b}", "\u{2126}"],
];

/// Values beyond the suite's, where canonical ordering decides.
const VALUES: &[&str] = &[
    "",
    "q",
    "caf\u{e9}",
    "cafe\u{301}",
    "q\u{323}\u{307}",
    "q\u{307}\u{323}",
    "\u{1e0b}\u{323}",
    "\u{1e0d}\u{307}",
    "d\u{307}\u{323}",
    "\u{212b}",
    "A\u{30a}",
    "\u{3ac}",
    "\u{3b1}\u{301}",
    "\u{ac01}",
    "\u{1100}\u{1161}\u{11a8}",
    "x\u{301}\u{300}y\u{300}\u{301}",
];

#[test]
fn the_catalog_map_and_host_nfc_agree() {
    let suite = suite_strings();
    // Pairs of plain ASCII are settled by byte equality on both paths and
    // the second set above covers that shape, so the suite's contribution to
    // the key sets is the strings that are not plain ASCII.
    let interesting: Vec<&str> = suite
        .iter()
        .map(String::as_str)
        .filter(|s| !s.is_ascii())
        .collect();
    let mut sets: Vec<Vec<&str>> = SETS.iter().map(|s| s.to_vec()).collect();
    sets.push(interesting);

    let values: Vec<&str> = suite
        .iter()
        .map(String::as_str)
        .chain(VALUES.iter().copied())
        .collect();

    for (set, raw) in sets.iter().enumerate() {
        // The writer normalizes the keys before they reach the map.
        let keys: Vec<String> = raw.iter().map(|k| nfc(k)).collect();
        let bytes = build(keys.iter().map(String::as_str));
        let map = NfcMap::from_bytes(&bytes).expect("the builder's bytes are a whole map");
        for value in &values {
            let want = nfc(value);
            for key in &keys {
                assert_eq!(
                    nfc_equivalent(map, value, key),
                    want == *key,
                    "set {set}: {value:?} against {key:?}"
                );
            }
        }
    }
}
