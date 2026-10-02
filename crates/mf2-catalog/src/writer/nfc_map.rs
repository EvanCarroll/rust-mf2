//! The canonical-equivalence map's builder (build side): from a catalog's
//! variant keys and argument names, the bytes `nfc_map::NfcMap` reads
//! (`plan/01` §4.3).
//!
//! Two strings are canonically equivalent exactly when their NFD forms are
//! equal, and a character's decomposition is a fixed sequence. So a string
//! can match one of the keys only if every character in it decomposes into
//! characters of those keys' NFD forms. Call that set of characters S: the
//! map holds every code point whose full canonical decomposition stays
//! inside S, with that decomposition, and the combining classes of S. Every
//! character of S decomposes to itself (it came out of an NFD form), so S is
//! in the map too, and the runtime can decompose both sides with the map
//! alone — no composition tables and no full normalization tables.
//!
//! When nothing in the map is at or above U+0300 the map is left out
//! entirely: the runtime's quick check then settles every comparison, since
//! a string holding a character at or above U+0300 decomposes to something
//! the map does not reach and so matches no key. That is the usual case for
//! an application selecting on ASCII keys.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::{canonical_combining_class, decompose_canonical};

use crate::nfc_map::{HANGUL_S_BASE, HANGUL_S_COUNT};

/// The highest code point.
const MAX_CODE_POINT: u32 = 0x10_FFFF;
/// Below this, a character is its own NFC form and cannot combine, which is
/// the runtime's quick check (`nfc_quick`: every UTF-8 byte below 0xCC).
const QUICK_LIMIT: u32 = 0x300;

/// The map for `keys` (variant keys and argument names, in any order and in
/// any normalization), or an empty vector when the map would say nothing.
pub fn build<'a, I>(keys: I) -> Vec<u8>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut set: BTreeSet<char> = BTreeSet::new();
    for key in keys {
        set.extend(key.nfd());
    }
    if set.is_empty() {
        return Vec::new();
    }

    // Every code point whose full canonical decomposition stays inside the
    // set. Hangul syllables are left out: the reader decomposes them by
    // arithmetic instead of carrying 11,172 entries.
    let mut entries: BTreeMap<char, Vec<char>> = BTreeMap::new();
    let mut reaches_slow_path = false;
    let mut buf: Vec<char> = Vec::new();
    for cp in 0..=MAX_CODE_POINT {
        if cp.wrapping_sub(HANGUL_S_BASE) < HANGUL_S_COUNT {
            continue;
        }
        let Some(ch) = char::from_u32(cp) else {
            continue;
        };
        buf.clear();
        decompose_canonical(ch, |c| buf.push(c));
        if !buf.iter().all(|c| set.contains(c)) {
            continue;
        }
        if cp >= QUICK_LIMIT {
            reaches_slow_path = true;
        }
        entries.insert(ch, buf.clone());
    }
    if !reaches_slow_path {
        return Vec::new();
    }

    // The combining classes of the characters that can appear in a
    // decomposition: the set itself (class 0 is the reader's default).
    let classes: Vec<(char, u8)> = set
        .iter()
        .map(|c| (*c, canonical_combining_class(*c)))
        .filter(|(_, ccc)| *ccc != 0)
        .collect();

    // The pool, with identical decompositions shared.
    let mut pool: Vec<char> = Vec::new();
    let mut shared: BTreeMap<Vec<char>, u32> = BTreeMap::new();
    let mut table: Vec<(char, u8, u32)> = Vec::new();
    for (ch, decomp) in &entries {
        if decomp.as_slice() == [*ch] {
            table.push((*ch, 0, 0));
            continue;
        }
        let off = *shared.entry(decomp.clone()).or_insert_with(|| {
            let at = u32::try_from(pool.len()).unwrap_or(u32::MAX);
            pool.extend_from_slice(decomp);
            at
        });
        let len = u8::try_from(decomp.len()).unwrap_or(u8::MAX);
        table.push((*ch, len, off));
    }

    let mut out = Vec::with_capacity(12 + table.len() * 8 + classes.len() * 4 + pool.len() * 3);
    out.extend_from_slice(&count(table.len()).to_le_bytes());
    out.extend_from_slice(&count(classes.len()).to_le_bytes());
    out.extend_from_slice(&count(pool.len()).to_le_bytes());
    for (ch, len, off) in &table {
        push_code_point(&mut out, *ch);
        out.push(*len);
        out.extend_from_slice(&off.to_le_bytes());
    }
    for (ch, ccc) in &classes {
        push_code_point(&mut out, *ch);
        out.push(*ccc);
    }
    for ch in &pool {
        push_code_point(&mut out, *ch);
    }
    out
}

/// A table length, which is at most the number of code points.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// `ch` as three little-endian bytes.
fn push_code_point(out: &mut Vec<u8>, ch: char) {
    let [x0, x1, x2, _] = u32::from(ch).to_le_bytes();
    out.extend_from_slice(&[x0, x1, x2]);
}
