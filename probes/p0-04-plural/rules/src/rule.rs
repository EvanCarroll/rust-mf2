//! UTS #35 (Part 3, §5.1 "Plural rules syntax") parser for CLDR's
//! `pluralRule-count-*` strings: a condition followed by optional samples.
//!
//! Grammar accepted (the current UTS #35 syntax; the legacy keywords `is`, `in`,
//! `not`, `within`, `mod` are not used by CLDR data and are rejected):
//!
//! ```text
//! rule          = condition samples
//! condition     = and_condition ('or' and_condition)*   | <empty>  (only for `other`)
//! and_condition = relation ('and' relation)*
//! relation      = operand ('%' value)? ('=' | '!=') range_list
//! operand       = 'n' | 'i' | 'f' | 't' | 'v' | 'w' | 'c' | 'e'
//! range_list    = (range | value) (',' range_list)*
//! range         = value '..' value
//! value         = digit+
//! samples       = ('@integer' sample_list)? ('@decimal' sample_list)?
//! sample_list   = sample_range (',' sample_range)* (',' ('…' | '...'))?
//! sample_range  = sample_value ('~' sample_value)?
//! sample_value  = digit+ ('.' digit+)? ([ce] digit+)?
//! ```

use plural_eval::Category;

use crate::error::ParseError;
use crate::samples::{SampleList, parse_samples};

/// A plural operand. `c` and `e` are synonyms and parse to [`Operand::C`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Operand {
    N,
    I,
    V,
    W,
    F,
    T,
    C,
}

/// `operand (% modulus)? (= | !=) items`, items as inclusive `(lo, hi)` pairs
/// (a single value is `(x, x)`), in source order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Relation {
    pub operand: Operand,
    pub modulus: Option<u64>,
    pub negated: bool,
    pub items: Vec<(u64, u64)>,
}

/// OR of ANDs. Empty = no condition (the `other` rule).
pub type Condition = Vec<Vec<Relation>>;

/// One `pluralRule-count-<category>` entry.
#[derive(Clone)]
pub struct Rule {
    pub category: Category,
    pub condition: Condition,
    pub integer: Option<SampleList>,
    pub decimal: Option<SampleList>,
}

/// Category from the CLDR key suffix (`zero`, `one`, `two`, `few`, `many`, `other`).
pub fn category(name: &str) -> Result<Category, ParseError> {
    Ok(match name {
        "zero" => Category::Zero,
        "one" => Category::One,
        "two" => Category::Two,
        "few" => Category::Few,
        "many" => Category::Many,
        "other" => Category::Other,
        _ => return Err(ParseError::UnknownCategory(name.to_owned())),
    })
}

/// Parses one rule string for `category`.
pub fn parse_rule(category: Category, text: &str) -> Result<Rule, ParseError> {
    let (cond_text, samples_text) = match text.find('@') {
        Some(at) => text.split_at(at),
        None => (text, ""),
    };
    let condition = parse_condition(cond_text)?;
    match (category == Category::Other, condition.is_empty()) {
        (true, false) => return Err(ParseError::OtherWithCondition),
        (false, true) => return Err(ParseError::MissingCondition),
        _ => {}
    }
    if condition.len() > usize::from(plural_eval::RULE_GROUPS_MASK) {
        return Err(ParseError::TooManyGroups);
    }
    let (integer, decimal) = parse_samples(samples_text)?;
    Ok(Rule { category, condition, integer, decimal })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tok {
    Operand(Operand),
    And,
    Or,
    Eq,
    Ne,
    Percent,
    DotDot,
    Comma,
    Num(u64),
    End,
}

struct Lexer<'a> {
    s: &'a [u8],
    pos: usize,
    peeked: Option<(usize, Tok)>,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self {
        Self { s: s.as_bytes(), pos: 0, peeked: None }
    }

    fn at(&self, k: usize) -> Option<u8> {
        self.s.get(k).copied()
    }

    fn lex(&mut self) -> Result<(usize, Tok), ParseError> {
        while self.at(self.pos).is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
        let start = self.pos;
        let Some(b) = self.at(start) else { return Ok((start, Tok::End)) };
        let tok = match b {
            b'0'..=b'9' => {
                let mut v = 0u64;
                while let Some(d @ b'0'..=b'9') = self.at(self.pos) {
                    v = v
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(u64::from(d - b'0')))
                        .ok_or(ParseError::Overflow { at: start })?;
                    self.pos += 1;
                }
                return Ok((start, Tok::Num(v)));
            }
            b'a'..=b'z' => {
                while self.at(self.pos).is_some_and(|b| b.is_ascii_lowercase()) {
                    self.pos += 1;
                }
                let word = self.s.get(start..self.pos).unwrap_or_default();
                return Ok((
                    start,
                    match word {
                        b"n" => Tok::Operand(Operand::N),
                        b"i" => Tok::Operand(Operand::I),
                        b"v" => Tok::Operand(Operand::V),
                        b"w" => Tok::Operand(Operand::W),
                        b"f" => Tok::Operand(Operand::F),
                        b"t" => Tok::Operand(Operand::T),
                        b"c" | b"e" => Tok::Operand(Operand::C),
                        b"and" => Tok::And,
                        b"or" => Tok::Or,
                        _ => return Err(ParseError::UnexpectedChar { at: start, ch: char::from(b) }),
                    },
                ));
            }
            b'=' => Tok::Eq,
            b'!' if self.at(start + 1) == Some(b'=') => {
                self.pos += 1;
                Tok::Ne
            }
            b'%' => Tok::Percent,
            b',' => Tok::Comma,
            b'.' if self.at(start + 1) == Some(b'.') => {
                self.pos += 1;
                Tok::DotDot
            }
            _ => return Err(ParseError::UnexpectedChar { at: start, ch: char::from(b) }),
        };
        self.pos += 1;
        Ok((start, tok))
    }

    fn peek(&mut self) -> Result<Tok, ParseError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lex()?);
        }
        Ok(self.peeked.map_or(Tok::End, |(_, t)| t))
    }

    fn next(&mut self) -> Result<(usize, Tok), ParseError> {
        match self.peeked.take() {
            Some(p) => Ok(p),
            None => self.lex(),
        }
    }

    fn value(&mut self) -> Result<u64, ParseError> {
        match self.next()? {
            (_, Tok::Num(v)) => Ok(v),
            (at, _) => Err(ParseError::Expected { at, expected: "a number" }),
        }
    }
}

fn parse_condition(text: &str) -> Result<Condition, ParseError> {
    let mut lx = Lexer::new(text);
    let mut or_groups = Vec::new();
    if lx.peek()? == Tok::End {
        return Ok(or_groups);
    }
    loop {
        let mut and_group = Vec::new();
        loop {
            and_group.push(parse_relation(&mut lx)?);
            if lx.peek()? == Tok::And {
                lx.next()?;
            } else {
                break;
            }
        }
        or_groups.push(and_group);
        match lx.next()? {
            (_, Tok::Or) => {}
            (_, Tok::End) => return Ok(or_groups),
            (at, _) => return Err(ParseError::Expected { at, expected: "`and`, `or` or end" }),
        }
    }
}

fn parse_relation(lx: &mut Lexer<'_>) -> Result<Relation, ParseError> {
    let operand = match lx.next()? {
        (_, Tok::Operand(o)) => o,
        (at, _) => return Err(ParseError::Expected { at, expected: "an operand" }),
    };
    let modulus = if lx.peek()? == Tok::Percent {
        lx.next()?;
        match lx.value()? {
            0 => return Err(ParseError::ZeroModulus),
            m => Some(m),
        }
    } else {
        None
    };
    let negated = match lx.next()? {
        (_, Tok::Eq) => false,
        (_, Tok::Ne) => true,
        (at, _) => return Err(ParseError::Expected { at, expected: "`=` or `!=`" }),
    };
    let mut items = Vec::new();
    loop {
        let lo = lx.value()?;
        let hi = if lx.peek()? == Tok::DotDot {
            lx.next()?;
            lx.value()?
        } else {
            lo
        };
        if hi < lo {
            return Err(ParseError::EmptyRange { lo, hi });
        }
        items.push((lo, hi));
        if lx.peek()? == Tok::Comma {
            lx.next()?;
        } else {
            return Ok(Relation { operand, modulus, negated, items });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_syntax() {
        let r = parse_rule(
            Category::Few,
            "v = 0 and i % 10 = 2..4 and i % 100 != 12..14 or f % 10 = 2..4 and f % 100 != 12..14 @integer 2~4, 22~24, … @decimal 0.2~0.4, 1.2~1.4, …",
        )
        .unwrap();
        assert_eq!(r.condition.len(), 2);
        assert_eq!(r.condition[0].len(), 3);
        assert_eq!(
            r.condition[0][2],
            Relation { operand: Operand::I, modulus: Some(100), negated: true, items: vec![(12, 14)] }
        );
        assert!(r.integer.as_ref().unwrap().open);
    }

    #[test]
    fn c_and_e_are_synonyms() {
        let a = parse_rule(Category::Many, "e = 0 and i != 0 or e != 0..5").unwrap();
        let b = parse_rule(Category::Many, "c = 0 and i != 0 or c != 0..5").unwrap();
        assert_eq!(a.condition, b.condition);
    }

    #[test]
    fn rejects_bad_rules() {
        assert!(parse_rule(Category::One, "").is_err());
        assert!(parse_rule(Category::Other, "n = 1").is_err());
        assert!(parse_rule(Category::One, "n is 1").is_err());
        assert!(parse_rule(Category::One, "n = 1 and").is_err());
        assert!(parse_rule(Category::One, "n % 0 = 1").is_err());
        assert!(parse_rule(Category::One, "n = 5..2").is_err());
        assert!(parse_rule(Category::One, "x = 1").is_err());
        assert!(parse_rule(Category::One, "n = 1 n = 2").is_err());
        assert!(parse_rule(Category::One, "n = 99999999999999999999").is_err());
    }
}
