//! Canonical equivalence with a catalog key, decided from the map the
//! catalog carries (`plan/01` §4.3) rather than from the full normalization
//! tables. MF2 compares `:string` selector values with variant keys, and
//! argument names with the names a message declares, in NFC; the strings on
//! the catalog's side are normalized when the catalog is written, so all the
//! runtime needs is "is this string canonically equivalent to that key".
//!
//! Client-path code: `no_std`, no allocation, no `fmt`, no panicking index.
//!
//! Two strings are canonically equivalent exactly when their NFD forms are
//! equal. The quick check settles the common case; otherwise both sides are
//! decomposed with the map, and each run of combining marks is compared
//! class by class, which is what canonical ordering (a stable sort by
//! combining class) would have produced — without sorting anything.

use mf2_catalog::nfc_map::{DecompChars, NfcMap};

use crate::text::nfc_quick;

/// Whether `value` is canonically equivalent to `key`, a string the catalog
/// holds in NFC, given the catalog's `map`.
pub fn equivalent(map: NfcMap<'_>, value: &str, key: &str) -> bool {
    if value == key {
        return true;
    }
    // Every code point below U+0300 is its own NFC form and cannot combine,
    // so two such strings are equivalent only if they are equal.
    if nfc_quick(value) && nfc_quick(key) {
        return false;
    }
    let mut a = Decomp::new(map, value);
    let mut b = Decomp::new(map, key);
    loop {
        match (a.peek(), b.peek()) {
            (Step::End, Step::End) => return true,
            (Step::Item(x, cx), Step::Item(y, cy)) => {
                if cx == 0 && cy == 0 {
                    // Starters keep their place under canonical ordering.
                    if x != y {
                        return false;
                    }
                    a.bump();
                    b.bump();
                } else if cx == 0 || cy == 0 || !runs_equal(&mut a, &mut b) {
                    return false;
                }
            }
            // One side ended, or holds a character the map does not reach.
            _ => return false,
        }
    }
}

/// Like [`equivalent`], for a `key` that may lie outside the map's domain:
/// `None` when `key` holds a character whose decomposition the map does not
/// reach, so the map cannot decide. An identical `value`, and two strings
/// below U+0300, still get an answer: neither needs the map. (A catalog
/// whose keys and names are all below U+0300 carries the empty map, which
/// reaches nothing.) The runtime's own call sites pass keys
/// the catalog holds and use [`equivalent`]; this is for a custom selector's
/// keys (`FnContext::equivalent`).
pub(crate) fn check(map: NfcMap<'_>, value: &str, key: &str) -> Option<bool> {
    if value == key {
        return Some(true);
    }
    if nfc_quick(value) && nfc_quick(key) {
        return Some(false);
    }
    // The domain is the strings whose NFD characters the map holds: a
    // Hangul syllable always decomposes (by arithmetic), so its jamo are
    // looked up too.
    let reached = key.chars().all(|ch| {
        map.decomposition(ch)
            .is_some_and(|mut d| d.all(|c| map.decomposition(c).is_some()))
    });
    if reached {
        Some(equivalent(map, value, key))
    } else {
        None
    }
}

/// What is at a cursor.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    /// A character of the decomposition, with its combining class.
    Item(char, u8),
    /// A character the map does not reach: nothing can match.
    Unmapped,
    /// The end of the string.
    End,
}

/// A cursor over a string's NFD, one character at a time, without canonical
/// ordering (`runs_equal` accounts for it). Cheap to clone, so a run can be
/// walked more than once.
#[derive(Clone)]
struct Decomp<'a> {
    map: NfcMap<'a>,
    rest: core::str::Chars<'a>,
    cur: Option<DecompChars<'a>>,
    head: Step,
}

impl<'a> Decomp<'a> {
    fn new(map: NfcMap<'a>, s: &'a str) -> Decomp<'a> {
        let mut d = Decomp {
            map,
            rest: s.chars(),
            cur: None,
            head: Step::End,
        };
        d.head = d.advance();
        d
    }

    fn peek(&self) -> Step {
        self.head
    }

    /// Moves to the next character; staying put at the end and at a
    /// character the map does not reach.
    fn bump(&mut self) {
        if matches!(self.head, Step::Item(_, _)) {
            self.head = self.advance();
        }
    }

    fn advance(&mut self) -> Step {
        loop {
            if let Some(chars) = self.cur.as_mut() {
                if let Some(ch) = chars.next() {
                    return Step::Item(ch, self.map.ccc(ch));
                }
                self.cur = None;
            }
            match self.rest.next() {
                None => return Step::End,
                Some(ch) => match self.map.decomposition(ch) {
                    None => return Step::Unmapped,
                    Some(chars) => self.cur = Some(chars),
                },
            }
        }
    }
}

/// Compares the runs of combining marks both cursors are standing in, and
/// leaves both just past them. Canonical ordering is a stable sort of such a
/// run by combining class, so two runs are equal after it exactly when, for
/// each class in turn, the characters of that class appear in the same order
/// on both sides.
fn runs_equal(a: &mut Decomp<'_>, b: &mut Decomp<'_>) -> bool {
    // The lengths bound the work: a key is as long as the catalog made it.
    let (Some(na), Some(nb)) = (run_len(a.clone()), run_len(b.clone())) else {
        return false;
    };
    if na != nb {
        return false;
    }
    let mut done = 0u8;
    loop {
        let (ca, cb) = (next_class(a.clone(), done), next_class(b.clone(), done));
        if ca != cb {
            return false;
        }
        let Some(class) = ca else {
            break;
        };
        let (mut wa, mut wb) = (a.clone(), b.clone());
        loop {
            let (x, y) = (take_class(&mut wa, class), take_class(&mut wb, class));
            if x != y {
                return false;
            }
            if x.is_none() {
                break;
            }
        }
        done = class;
    }
    skip_run(a);
    skip_run(b);
    true
}

/// How many combining marks the run holds, or `None` if the map does not
/// reach one of them (or what follows: either way nothing can match).
fn run_len(mut c: Decomp<'_>) -> Option<usize> {
    let mut n = 0usize;
    while let Step::Item(_, ccc) = c.peek() {
        if ccc == 0 {
            return Some(n);
        }
        n = n.checked_add(1)?;
        c.bump();
    }
    match c.peek() {
        Step::End => Some(n),
        _ => None,
    }
}

/// The smallest combining class above `done` left in the run.
fn next_class(mut c: Decomp<'_>, done: u8) -> Option<u8> {
    let mut best: Option<u8> = None;
    while let Step::Item(_, ccc) = c.peek() {
        if ccc == 0 {
            break;
        }
        if ccc > done && best.is_none_or(|b| ccc < b) {
            best = Some(ccc);
        }
        c.bump();
    }
    best
}

/// The next character of class `class` in the run, moving the cursor past
/// it.
fn take_class(c: &mut Decomp<'_>, class: u8) -> Option<char> {
    while let Step::Item(ch, ccc) = c.peek() {
        if ccc == 0 {
            return None;
        }
        c.bump();
        if ccc == class {
            return Some(ch);
        }
    }
    None
}

/// Moves past the run of combining marks.
fn skip_run(c: &mut Decomp<'_>) {
    while let Step::Item(_, ccc) = c.peek() {
        if ccc == 0 {
            break;
        }
        c.bump();
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    extern crate std;

    use alloc::string::String;
    use alloc::vec::Vec;

    use mf2_catalog::nfc_map::NfcMap;
    use mf2_catalog::writer::nfc_map::build;
    use unicode_normalization::UnicodeNormalization;

    use super::{check, equivalent};

    /// The key sets the gate asks for (`plan/04` 13.1): whether the map must
    /// come out empty, and the keys.
    const SETS: &[(&str, bool, &[&str])] = &[
        ("empty", true, &[]),
        ("ascii", true, &["h", "s", "q", "one", "other"]),
        (
            "latin",
            false,
            &["caf\u{e9}", "cafe\u{301}", "\u{c5}ngstr\u{f6}m"],
        ),
        ("marks", false, &["q\u{323}\u{307}", "q\u{307}\u{323}"]),
        ("greek", false, &["\u{3ac}", "\u{3b1}\u{301}", "\u{1f70}"]),
        ("hangul", false, &["\u{ac01}", "\u{ac00}", "\u{d4db}"]),
        ("singleton", false, &["\u{212b}", "\u{2126}"]),
    ];

    fn nfd(s: &str) -> String {
        s.nfd().collect()
    }

    /// One key set: the map's bytes, and each key with its NFD form.
    type KeySet = (Vec<u8>, Vec<(&'static str, String)>);

    /// The maps of `SETS`, with each key's NFD form.
    fn corpus() -> Vec<KeySet> {
        SETS.iter()
            .map(|(name, empty, keys)| {
                let bytes = build(keys.iter().copied());
                assert_eq!(bytes.is_empty(), *empty, "{name}: the map's size");
                let keys = keys.iter().map(|k| (*k, nfd(k))).collect();
                (bytes, keys)
            })
            .collect()
    }

    /// The gate: for every code point and every key set, the check agrees
    /// with comparing NFC forms (NFD forms here: equal NFD is the same
    /// relation, and what `unicode-normalization` computes either way).
    #[test]
    fn every_code_point() {
        let corpus = corpus();
        let maps: Vec<NfcMap<'_>> = corpus
            .iter()
            .map(|(bytes, _)| {
                NfcMap::from_bytes(bytes).expect("the builder's bytes are a whole map")
            })
            .collect();
        let mut value = String::new();
        for cp in 0..=0x10_FFFFu32 {
            let Some(ch) = char::from_u32(cp) else {
                continue;
            };
            value.clear();
            value.push(ch);
            let want = nfd(&value);
            for (map, (_, keys)) in maps.iter().zip(&corpus) {
                for (key, key_nfd) in keys {
                    assert_eq!(
                        equivalent(*map, &value, key),
                        want == *key_nfd,
                        "U+{cp:04X} against {key:?}"
                    );
                }
            }
        }
    }

    /// Strings of more than one character, where canonical ordering and the
    /// run comparison matter.
    #[test]
    fn strings() {
        let corpus = corpus();
        let values: &[&str] = &[
            "",
            "q",
            "cafe",
            "caf\u{e9}",
            "cafe\u{301}",
            "caf\u{65}\u{301}x",
            "q\u{323}\u{307}",
            "q\u{307}\u{323}",
            "q\u{307}\u{323}\u{301}",
            "q\u{323}\u{307}\u{307}",
            "\u{1e0b}\u{323}",
            "\u{1e0d}\u{307}",
            "d\u{307}\u{323}",
            "\u{212b}",
            "\u{c5}",
            "A\u{30a}",
            "\u{3ac}",
            "\u{3b1}\u{301}",
            "\u{ac01}",
            "\u{1100}\u{1161}\u{11a8}",
            "\u{1100}\u{1161}",
            "\u{d4db}",
            "\u{301}",
            "\u{301}q",
            "x\u{301}\u{300}y\u{300}\u{301}",
        ];
        for (bytes, keys) in &corpus {
            let map = NfcMap::from_bytes(bytes).expect("a whole map");
            for value in values {
                let want = nfd(value);
                for (key, key_nfd) in keys {
                    assert_eq!(
                        equivalent(map, value, key),
                        want == *key_nfd,
                        "{value:?} against {key:?}"
                    );
                }
                // A value is equivalent to itself, and to its own NFD and
                // NFC forms, when the map was built with it as a key.
                let own = build([*value]);
                let own = NfcMap::from_bytes(&own).expect("a whole map");
                assert!(equivalent(own, value, value), "{value:?} with itself");
                let nfc: String = value.nfc().collect();
                assert!(equivalent(own, &nfd(value), &nfc), "{value:?} both forms");
            }
        }
    }

    /// The helper's domain (`plan/01` §2 decision 6): a key holding a
    /// character the catalog never saw gets no answer; the same key in the
    /// catalog gets the exact one.
    #[test]
    fn outside_the_domain() {
        // "latin": é, e, U+0301, Å and the rest; never ñ, K or a jamo.
        let latin = build(["caf\u{e9}", "\u{c5}ngstr\u{f6}m"]);
        let latin = NfcMap::from_bytes(&latin).expect("a whole map");
        for (value, key) in [
            ("ni\u{f1}o", "nin\u{303}o"),
            ("\u{212a}", "K"),
            ("\u{1100}\u{1161}", "\u{ac00}"),
            ("x", "\u{ac00}"),
        ] {
            assert_eq!(check(latin, value, key), None, "{value:?} against {key:?}");
            // In a catalog that holds the key, the exact answer.
            let own = build([key]);
            let own = NfcMap::from_bytes(&own).expect("a whole map");
            let want = nfd(value) == nfd(key);
            assert_eq!(
                check(own, value, key),
                Some(want),
                "{value:?} against {key:?}"
            );
            assert_eq!(
                equivalent(own, value, key),
                want,
                "{value:?} against {key:?}"
            );
        }
        // Keys in the domain get the same answer as `equivalent`.
        for value in ["cafe\u{301}", "cafe", "A\u{30a}ngstr\u{f6}m", "\u{e9}"] {
            for key in ["caf\u{e9}", "\u{c5}ngstr\u{f6}m"] {
                assert_eq!(
                    check(latin, value, key),
                    Some(nfd(value) == nfd(key)),
                    "{value:?} against {key:?}"
                );
            }
        }
        // No map needed: an identical value, or two strings below U+0300.
        let empty = NfcMap::from_bytes(&[]).expect("the empty map");
        assert_eq!(check(empty, "\u{f1}", "\u{f1}"), Some(true));
        assert_eq!(check(empty, "one", "other"), Some(false));
        assert_eq!(check(empty, "\u{212a}", "K"), None);
    }
}
