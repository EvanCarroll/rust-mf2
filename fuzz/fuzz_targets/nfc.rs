//! Fuzz target `nfc`: the canonical-equivalence check against full NFC
//! (`plan/01` §4.3, the gate's second half).
//!
//! The runtime decides whether a string is canonically equivalent to one of a
//! catalog's keys from the small map the catalog carries, not from the full
//! normalization tables. This target is the differential test of that method:
//! it draws a key set and a candidate string from decomposable characters and
//! combining marks, has the writer build the map, and fails when the check
//! and comparing NFD forms with `unicode-normalization` disagree.
//!
//! The input is a sequence of **index** bytes split on `0xFF`: the last field
//! is the candidate, the earlier ones (at most eight) are the keys, and every
//! byte of a field picks a character of `ALPHABET` by remainder — so every
//! byte sequence is a valid input, and a seed need not know the table.
//! An input with no separator has no keys and is skipped.
//!
//! The keys are held in NFC, as a catalog holds them; the candidate is not,
//! since it stands for a value the program passed in. Every candidate — the
//! drawn string and each key — is compared with every key, so key-against-key
//! is covered too.
//!
//! Building the map is build-side work (it walks every code point), so it is
//! cached per key set and left off the clock, as source mode's compile is in
//! the `format` target. The check itself stays within the usual budget of
//! **CPU time** per byte compared (`../common/budget.rs`).

#![no_main]

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

#[path = "../common/budget.rs"]
mod budget;

use libfuzzer_sys::fuzz_target;
use mf2_catalog::nfc_map::NfcMap;
use mf2_catalog::writer::nfc_map::build;
use mf2_runtime::nfc_equivalent;
use unicode_normalization::UnicodeNormalization;

/// The characters a key or a candidate is drawn from: ASCII, precomposed
/// letters with one, two and three levels of decomposition, singletons,
/// combining marks of many different classes (including two that decompose
/// into marks themselves), Hangul syllables and jamo, and a few characters
/// with no canonical decomposition at all — which is the case that must make
/// a comparison fail outright.
const ALPHABET: &[char] = &[
    // ASCII, including trippy's keys.
    'a', 'e', 'h', 'q', 's', 'o', 'A', 'z',
    // Latin, one level.
    '\u{e9}', '\u{f6}', '\u{c5}', '\u{f1}', '\u{104}',
    // Latin, two and three levels.
    '\u{1d5}', '\u{1ec7}', '\u{1e14}',
    // Greek.
    '\u{3b1}', '\u{3ac}', '\u{1f70}', '\u{390}', '\u{1fd3}',
    // Singletons.
    '\u{212b}', '\u{2126}', '\u{1e9b}',
    // Combining marks: classes 230, 220, 202, 216, 240, 10, 7, 103, 129,
    // 130, 132, 30, and the two that decompose into marks (U+0344, U+0F73).
    '\u{301}', '\u{300}', '\u{307}', '\u{308}', '\u{323}', '\u{327}',
    '\u{31b}', '\u{345}', '\u{5b0}', '\u{93c}', '\u{e38}', '\u{f71}',
    '\u{f72}', '\u{f74}', '\u{618}', '\u{344}', '\u{f73}',
    // Hangul: syllables at both ends of the block, one without a trailing
    // jamo, and the jamo themselves.
    '\u{ac00}', '\u{ac01}', '\u{d7a3}', '\u{1100}', '\u{1161}', '\u{11a8}',
    // Nothing decomposes into these, and they decompose into nothing.
    '\u{c6}', '\u{4e00}', '\u{fb01}', '\u{1f600}',
];

/// The field separator in the input.
const SEP: u8 = 0xFF;
/// At most this many keys; the rest of the fields are ignored.
const MAX_KEYS: usize = 8;
/// At most this many characters per field.
const MAX_CHARS: usize = 512;
/// The cached maps are dropped once there are this many.
const MAX_CACHED: usize = 1024;

/// A field's characters.
fn field(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(MAX_CHARS)
        .filter_map(|b| ALPHABET.get(usize::from(*b) % ALPHABET.len()).copied())
        .collect()
}

fn nfd(s: &str) -> String {
    s.nfd().collect()
}

/// The map of `keys`, built once per key set.
fn map_bytes(keys: &[String]) -> Vec<u8> {
    thread_local! {
        static MAPS: RefCell<HashMap<Vec<String>, Vec<u8>>> = RefCell::new(HashMap::new());
    }
    MAPS.with_borrow_mut(|cache| {
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache
            .entry(keys.to_vec())
            .or_insert_with(|| build(keys.iter().map(String::as_str)))
            .clone()
    })
}

fuzz_target!(|data: &[u8]| {
    let fields: Vec<&[u8]> = data.split(|&b| b == SEP).collect();
    let Some((candidate, keys)) = fields.split_last() else {
        return;
    };
    if keys.is_empty() {
        return;
    }
    // The catalog's side is in NFC, and a key the writer never saw cannot be
    // selected on: an empty key is dropped.
    let keys: Vec<String> = keys
        .iter()
        .take(MAX_KEYS)
        .map(|f| field(f).nfc().collect::<String>())
        .filter(|k| !k.is_empty())
        .collect();
    if keys.is_empty() {
        return;
    }
    let candidate = field(candidate);

    let bytes = map_bytes(&keys);
    let map = NfcMap::from_bytes(&bytes).expect("the writer's map loads");
    assert_eq!(map.len_bytes(), bytes.len(), "the map's own length");
    assert_eq!(map.is_empty(), bytes.is_empty(), "an empty map has no bytes");

    let mut compared = 0usize;
    let cpu = budget::Cpu::start();
    for value in std::iter::once(&candidate).chain(keys.iter()) {
        for key in &keys {
            let got = nfc_equivalent(map, value, key);
            let want = nfd(value) == nfd(key);
            assert_eq!(
                got, want,
                "{value:?} vs {key:?} (keys {keys:?}, map {} B)",
                bytes.len()
            );
            compared += value.len() + key.len();
        }
    }
    budget::check(&cpu, compared, Duration::ZERO);
});
