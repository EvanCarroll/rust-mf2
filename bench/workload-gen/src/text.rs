//! Text synthesis: English text of an exact byte length, kebab-case keys of an
//! exact length, and synthetic text for the other locales.

use crate::rng::Rng;
use crate::vocab;

/// Longest word the exact-length fill relies on.
const MAX_WORD: usize = 13;

/// Words grouped by byte length: `by_len[n]` holds the words of `n` bytes.
#[derive(Debug)]
pub struct Lexicon {
    by_len: Vec<Vec<&'static str>>,
    all: Vec<&'static str>,
}

impl Lexicon {
    /// Builds a lexicon; panics if some length in `1..=max` has no word
    /// (a bug in the word tables, caught by the unit tests).
    pub fn new(words: &[&'static str], min: usize, max: usize) -> Self {
        let mut by_len: Vec<Vec<&'static str>> = vec![Vec::new(); max + 1];
        let mut all = Vec::new();
        for &w in words {
            if w.len() <= max {
                by_len[w.len()].push(w);
                all.push(w);
            }
        }
        for (len, list) in by_len.iter().enumerate().skip(min) {
            assert!(!list.is_empty(), "no word of {len} bytes");
        }
        Self { by_len, all }
    }

    /// English text vocabulary.
    pub fn english() -> Self {
        Self::new(vocab::EN_WORDS, 1, MAX_WORD)
    }

    /// Key vocabulary (lengths 2–12).
    pub fn keys() -> Self {
        Self::new(vocab::KEY_WORDS, 2, 12)
    }

    fn exact(&self, rng: &mut Rng, len: usize) -> Option<&'static str> {
        let list = self.by_len.get(len)?;
        if list.is_empty() {
            None
        } else {
            Some(*rng.pick(list))
        }
    }

    fn at_most(&self, rng: &mut Rng, max: usize) -> &'static str {
        // Rejection sampling over all words; falls back to the exact bucket.
        for _ in 0..16 {
            let w = *rng.pick(&self.all);
            if w.len() <= max {
                return w;
            }
        }
        (1..=max.min(self.by_len.len() - 1))
            .rev()
            .find_map(|l| self.exact(rng, l))
            .expect("lexicon covers short lengths")
    }

    /// Words joined by single spaces whose total is exactly `bytes` (≥ 1).
    pub fn fill(&self, rng: &mut Rng, bytes: usize) -> Vec<&'static str> {
        assert!(bytes >= 1, "fill(0)");
        let mut words = Vec::new();
        let mut remaining = bytes;
        loop {
            if remaining <= MAX_WORD {
                // Finish with one exact word, or occasionally two words.
                if remaining >= 5 && rng.chance(50) {
                    let first = self.at_most(rng, remaining - 2);
                    words.push(first);
                    remaining -= first.len() + 1;
                }
                words.push(
                    self.exact(rng, remaining)
                        .expect("every length 1..=13 exists"),
                );
                return words;
            }
            // Leave at least 2 bytes (space + one-byte word) for what follows.
            let w = self.at_most(rng, remaining - 2);
            words.push(w);
            remaining -= w.len() + 1;
        }
    }

    /// A kebab-case key of exactly `len` bytes (≥ 2).
    pub fn key(&self, rng: &mut Rng, len: usize) -> String {
        let mut parts: Vec<&'static str> = Vec::new();
        let mut remaining = len.max(2);
        loop {
            if remaining <= 12 {
                if remaining >= 7 && rng.chance(60) {
                    let first = self.at_most(rng, remaining - 3);
                    parts.push(first);
                    remaining -= first.len() + 1;
                }
                parts.push(
                    self.exact(rng, remaining)
                        .expect("every key length 2..=12 exists"),
                );
                return parts.join("-");
            }
            if remaining == 13 {
                // 13 = 6 + 1 + 6 — avoids a one-letter tail.
                parts.push(self.exact(rng, 6).expect("6-letter key word"));
                remaining = 6;
                continue;
            }
            let w = self.at_most(rng, (remaining - 3).min(10));
            parts.push(w);
            remaining -= w.len() + 1;
        }
    }
}

/// Upper-cases the first character if it is ASCII.
pub fn capitalize(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut chars = word.chars();
    if let Some(c) = chars.next() {
        out.push(c.to_ascii_uppercase());
    }
    out.extend(chars);
    out
}

/// Synthetic text of roughly `bytes` bytes made of the locale's syllables.
/// Never empty, never starts or ends with whitespace.
pub fn synthetic(rng: &mut Rng, locale: &vocab::RealLocale, bytes: usize) -> String {
    let target = bytes.max(1);
    let mut out = String::new();
    while out.len() < target {
        if !out.is_empty() && locale.spaced {
            out.push(' ');
        }
        let syllables = rng.range(1, 3);
        for _ in 0..syllables {
            out.push_str(rng.pick(locale.syllables));
        }
    }
    out
}

/// en-XA transformation of one text run: ASCII letters accented.
pub fn accent(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            'a'..='z' => out.push_str(vocab::ACCENTS_LOWER[(c as usize) - ('a' as usize)]),
            'A'..='Z' => out.push_str(vocab::ACCENTS_UPPER[(c as usize) - ('A' as usize)]),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Lexicon, accent};
    use crate::rng::Rng;

    #[test]
    fn fill_is_exact() {
        let lex = Lexicon::english();
        let mut rng = Rng::new(7);
        for bytes in 1..300 {
            let words = lex.fill(&mut rng, bytes);
            assert_eq!(words.join(" ").len(), bytes, "{words:?}");
        }
    }

    #[test]
    fn key_is_exact() {
        let lex = Lexicon::keys();
        let mut rng = Rng::new(7);
        for len in 2..60 {
            let key = lex.key(&mut rng, len);
            assert_eq!(key.len(), len, "{key}");
            assert!(key.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'));
            assert!(!key.starts_with('-') && !key.ends_with('-') && !key.contains("--"));
        }
    }

    #[test]
    fn accents() {
        assert_eq!(accent("Save {x}"), "Šåṽé {ẋ}");
    }
}
