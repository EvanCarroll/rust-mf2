//! A reader for the ABNF of `spec/message.abnf` (RFC 5234 with RFC 7405's
//! `%s"…"`), and a random generator of the strings it derives.
//!
//! Every generated string is well-formed MF2 *by definition* — the grammar is
//! the spec's — so a generated message that `mf2-syntax` rejects is a parser
//! bug. The generator is deterministic for a seed.
//!
//! Supported: rules and continuation lines, comments, alternation `/`,
//! concatenation, repetition (`*`, `n*m`, `n`), groups `( )`, options `[ ]`,
//! quoted strings (`"…"`, `%s"…"`, `%i"…"`), numeric values (`%x41`,
//! `%x41-5A`, `%x2E.69`, and `%d`/`%b`), and the core rules the spec uses
//! (ALPHA, DIGIT, SP, HTAB, CR, LF, plus DQUOTE, VCHAR, WSP).

use std::collections::BTreeMap;
use std::fmt;

/// A grammar element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Element {
    /// Any one of these.
    Alt(Vec<Element>),
    /// All of these, in order.
    Seq(Vec<Element>),
    /// `min` to `max` (unbounded if `None`) repetitions.
    Rep {
        min: u32,
        max: Option<u32>,
        element: Box<Element>,
    },
    /// A reference to a rule.
    Rule(String),
    /// A literal string (case as written).
    Str(String),
    /// One code point in `lo..=hi`.
    Range(u32, u32),
}

/// An error reading the ABNF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbnfError(pub String);

impl fmt::Display for AbnfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ABNF: {}", self.0)
    }
}

impl std::error::Error for AbnfError {}

/// The rules of an ABNF grammar, by (lower-case) name.
#[derive(Clone, Debug)]
pub struct Grammar {
    rules: BTreeMap<String, Element>,
}

impl Grammar {
    /// Reads an ABNF grammar; the core rules are added.
    pub fn parse(text: &str) -> Result<Self, AbnfError> {
        let mut rules = BTreeMap::new();
        for (name, body) in rule_texts(text) {
            let mut p = Cursor {
                s: body.as_bytes(),
                i: 0,
                rule: &name,
            };
            let element = p.alternation()?;
            p.ws();
            if p.i != p.s.len() {
                return Err(p.error("unexpected text"));
            }
            rules.insert(name.to_ascii_lowercase(), element);
        }
        for (name, element) in core_rules() {
            rules.entry(name.to_owned()).or_insert(element);
        }
        let grammar = Grammar { rules };
        grammar.check_references()?;
        Ok(grammar)
    }

    /// The rule named `name` (case-insensitive).
    pub fn rule(&self, name: &str) -> Option<&Element> {
        self.rules.get(&name.to_ascii_lowercase())
    }

    /// Rule names, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.rules.keys().map(String::as_str)
    }

    fn check_references(&self) -> Result<(), AbnfError> {
        fn walk(g: &Grammar, e: &Element) -> Result<(), AbnfError> {
            match e {
                Element::Alt(v) | Element::Seq(v) => v.iter().try_for_each(|e| walk(g, e)),
                Element::Rep { element, .. } => walk(g, element),
                Element::Rule(r) if g.rule(r).is_none() => {
                    Err(AbnfError(format!("undefined rule `{r}`")))
                }
                _ => Ok(()),
            }
        }
        self.rules.values().try_for_each(|e| walk(self, e))
    }
}

/// Splits the text into `(rule name, definition)`, comments removed and
/// continuation lines joined.
fn rule_texts(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = strip_comment(line);
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with([' ', '\t']) {
            if let Some((_, body)) = out.last_mut() {
                body.push(' ');
                body.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, body)) = line.split_once('=') {
            // `=/` (incremental alternative) is not used by the spec.
            out.push((name.trim().to_owned(), body.trim().to_owned()));
        }
    }
    out
}

/// Removes a `;` comment, ignoring `;` inside quoted strings.
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

fn core_rules() -> Vec<(&'static str, Element)> {
    use Element::{Alt, Range};
    vec![
        ("alpha", Alt(vec![Range(0x41, 0x5A), Range(0x61, 0x7A)])),
        ("digit", Range(0x30, 0x39)),
        ("sp", Range(0x20, 0x20)),
        ("htab", Range(0x09, 0x09)),
        ("cr", Range(0x0D, 0x0D)),
        ("lf", Range(0x0A, 0x0A)),
        ("dquote", Range(0x22, 0x22)),
        ("vchar", Range(0x21, 0x7E)),
        ("wsp", Alt(vec![Range(0x20, 0x20), Range(0x09, 0x09)])),
    ]
}

struct Cursor<'a> {
    s: &'a [u8],
    i: usize,
    rule: &'a str,
}

impl Cursor<'_> {
    fn error(&self, what: &str) -> AbnfError {
        AbnfError(format!(
            "rule `{}`, offset {}: {what}: {:?}",
            self.rule,
            self.i,
            String::from_utf8_lossy(&self.s[self.i.min(self.s.len())..])
        ))
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.i += 1;
        }
    }

    fn alternation(&mut self) -> Result<Element, AbnfError> {
        let mut alts = vec![self.concatenation()?];
        loop {
            self.ws();
            if self.peek() != Some(b'/') {
                break;
            }
            self.i += 1;
            alts.push(self.concatenation()?);
        }
        Ok(if alts.len() == 1 {
            alts.remove(0)
        } else {
            Element::Alt(alts)
        })
    }

    fn concatenation(&mut self) -> Result<Element, AbnfError> {
        let mut items = Vec::new();
        loop {
            self.ws();
            match self.peek() {
                None | Some(b'/' | b')' | b']') => break,
                _ => items.push(self.repetition()?),
            }
        }
        match items.len() {
            0 => Err(self.error("empty concatenation")),
            1 => Ok(items.remove(0)),
            _ => Ok(Element::Seq(items)),
        }
    }

    fn number(&mut self, radix: u32) -> Option<u32> {
        let start = self.i;
        while self.peek().is_some_and(|b| char::from(b).is_digit(radix)) {
            self.i += 1;
        }
        let digits = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        u32::from_str_radix(digits, radix).ok()
    }

    fn repetition(&mut self) -> Result<Element, AbnfError> {
        let min = self.number(10);
        let (min, max) = if self.peek() == Some(b'*') {
            self.i += 1;
            (min.unwrap_or(0), self.number(10))
        } else if let Some(n) = min {
            (n, Some(n))
        } else {
            return self.element();
        };
        let element = Box::new(self.element()?);
        Ok(Element::Rep { min, max, element })
    }

    fn element(&mut self) -> Result<Element, AbnfError> {
        match self.peek() {
            Some(b'(' | b'[') => {
                let optional = self.peek() == Some(b'[');
                self.i += 1;
                let inner = self.alternation()?;
                self.ws();
                let close = if optional { b']' } else { b')' };
                if self.peek() != Some(close) {
                    return Err(self.error("unclosed group"));
                }
                self.i += 1;
                Ok(if optional {
                    Element::Rep {
                        min: 0,
                        max: Some(1),
                        element: Box::new(inner),
                    }
                } else {
                    inner
                })
            }
            Some(b'"') => self.quoted(),
            Some(b'%') => {
                self.i += 1;
                match self.peek() {
                    Some(b's' | b'i') => {
                        self.i += 1;
                        self.quoted()
                    }
                    Some(b'x') => self.num_val(16),
                    Some(b'd') => self.num_val(10),
                    Some(b'b') => self.num_val(2),
                    _ => Err(self.error("bad %-value")),
                }
            }
            Some(b) if b.is_ascii_alphabetic() => {
                let start = self.i;
                while self
                    .peek()
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'-')
                {
                    self.i += 1;
                }
                let name = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                Ok(Element::Rule(name))
            }
            _ => Err(self.error("expected an element")),
        }
    }

    fn quoted(&mut self) -> Result<Element, AbnfError> {
        self.i += 1;
        let start = self.i;
        while self.peek().is_some_and(|b| b != b'"') {
            self.i += 1;
        }
        if self.peek() != Some(b'"') {
            return Err(self.error("unclosed string"));
        }
        let s = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
        self.i += 1;
        Ok(Element::Str(s))
    }

    fn num_val(&mut self, radix: u32) -> Result<Element, AbnfError> {
        self.i += 1;
        let first = self.number(radix).ok_or_else(|| self.error("bad number"))?;
        match self.peek() {
            Some(b'-') => {
                self.i += 1;
                let last = self.number(radix).ok_or_else(|| self.error("bad range"))?;
                Ok(Element::Range(first, last))
            }
            Some(b'.') => {
                let mut s = String::new();
                s.push(char::from_u32(first).ok_or_else(|| self.error("bad code point"))?);
                while self.peek() == Some(b'.') {
                    self.i += 1;
                    let n = self.number(radix).ok_or_else(|| self.error("bad number"))?;
                    s.push(char::from_u32(n).ok_or_else(|| self.error("bad code point"))?);
                }
                Ok(Element::Str(s))
            }
            _ => Ok(Element::Range(first, first)),
        }
    }
}

/// A small deterministic PRNG (`SplitMix64`).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// A generator for `seed`.
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    /// `true` with probability `num / den`.
    pub fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

/// Random strings derived from a [`Grammar`].
#[derive(Debug)]
pub struct Generator<'g> {
    grammar: &'g Grammar,
    /// The randomness.
    pub rng: Rng,
    /// Extra repetitions beyond a repetition's minimum, at most.
    pub max_extra: u32,
}

impl<'g> Generator<'g> {
    /// A generator over `grammar` seeded with `seed`.
    pub fn new(grammar: &'g Grammar, seed: u64) -> Self {
        Generator {
            grammar,
            rng: Rng::new(seed),
            max_extra: 4,
        }
    }

    /// A random string derived from rule `start`.
    pub fn generate(&mut self, start: &str) -> String {
        let mut out = String::new();
        if let Some(e) = self.grammar.rule(start) {
            self.element(e, &mut out, 0);
        }
        out
    }

    fn element(&mut self, e: &Element, out: &mut String, depth: u32) {
        // The spec's grammar has no recursive rules; the bound only guards
        // against a changed grammar.
        if depth > 64 {
            return;
        }
        match e {
            Element::Alt(alts) => {
                let pick = usize::try_from(self.rng.below(alts.len() as u64)).unwrap_or(0);
                self.element(&alts[pick], out, depth + 1);
            }
            Element::Seq(items) => {
                for item in items {
                    self.element(item, out, depth + 1);
                }
            }
            Element::Rep { min, max, element } => {
                let cap = max.unwrap_or(u32::MAX).min(min + self.max_extra);
                // Geometric-ish: short repetitions are the most common.
                let mut n = *min;
                while n < cap && self.rng.chance(3, 5) {
                    n += 1;
                }
                for _ in 0..n {
                    self.element(element, out, depth + 1);
                }
            }
            Element::Rule(name) => {
                if let Some(r) = self.grammar.rule(name) {
                    self.element(r, out, depth + 1);
                }
            }
            Element::Str(s) => out.push_str(s),
            Element::Range(lo, hi) => out.push(self.code_point(*lo, *hi)),
        }
    }

    /// A code point in `lo..=hi`, skipping surrogates, biased towards ASCII
    /// and the BMP (where the syntax-relevant characters are) and towards the
    /// range's ends.
    fn code_point(&mut self, lo: u32, hi: u32) -> char {
        for _ in 0..16 {
            let band = match self.rng.below(20) {
                0..=10 => (lo, hi.min(0x7F)),
                11..=13 => (lo.max(0x80), hi.min(0x7FF)),
                14..=16 => (lo.max(0x800), hi.min(0xFFFF)),
                17 => (lo, lo),
                18 => (hi, hi),
                _ => (lo.max(0x1_0000), hi),
            };
            if band.0 > band.1 {
                continue;
            }
            let u =
                band.0 + u32::try_from(self.rng.below(u64::from(band.1 - band.0) + 1)).unwrap_or(0);
            if let Some(c) = char::from_u32(u) {
                return c;
            }
        }
        // Every range in the spec contains a non-surrogate; fall back to its
        // first valid code point.
        (lo..=hi).find_map(char::from_u32).unwrap_or('a')
    }
}

#[cfg(test)]
mod tests {
    use super::{Element, Generator, Grammar};

    const MINI: &str = r#"
greeting = "hi" [ SP name ] ; a comment with "quotes"
name     = 1*ALPHA
         / %x41-43 %x44.45
kw       = %s".x"
num      = 2*3DIGIT
"#;

    #[test]
    fn reads_rules_and_continuations() {
        let g = Grammar::parse(MINI).expect("parses");
        assert!(g.rule("GREETING").is_some());
        assert_eq!(g.rule("kw"), Some(&Element::Str(".x".into())));
        let Some(Element::Alt(alts)) = g.rule("name") else {
            panic!("name is an alternation");
        };
        assert_eq!(
            alts[1],
            Element::Seq(vec![Element::Range(0x41, 0x43), Element::Str("DE".into())])
        );
        let Some(Element::Rep {
            min: 2,
            max: Some(3),
            ..
        }) = g.rule("num")
        else {
            panic!("num is 2*3DIGIT");
        };
    }

    #[test]
    fn rejects_undefined_rules() {
        assert!(Grammar::parse("a = b").is_err());
    }

    #[test]
    fn generates_deterministically() {
        let g = Grammar::parse(MINI).expect("parses");
        let a: Vec<String> = (0..20)
            .map(|s| Generator::new(&g, s).generate("greeting"))
            .collect();
        let b: Vec<String> = (0..20)
            .map(|s| Generator::new(&g, s).generate("greeting"))
            .collect();
        assert_eq!(a, b);
        assert!(a.iter().all(|s| s.starts_with("hi")));
        assert!(a.iter().any(|s| s.len() > 2));
    }
}
