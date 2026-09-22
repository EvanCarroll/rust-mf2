//! CLDR number patterns (UTS #35 Part 3 §3.2) → what a `number.patterns`
//! record keeps: the grouping of the positive subpattern's integer part and
//! the affixes of both subpatterns, with `-`, `%` and `¤` as placeholders.
//!
//! Everything else in the number part (minimum digits, fraction digits) is
//! MF2's business, not the pattern's: `:percent` fixes its fraction digits,
//! `:currency` takes them from the currency. What `number.patterns` v1 cannot
//! express is refused: padding (`*`), scientific and significant-digit
//! number parts (`E`, `@`), `‰`, `+` and multi-`¤` placeholders. CLDR 48.2.1
//! uses none of them in the patterns we extract; the extractor fails on
//! them, so a CLDR update that does is noticed.

use mf2_catalog::number::{AffixPart, Grouping};
use mf2_catalog::writer::number::PatternSpec;

use crate::error::Error;

/// One affix token (owned: quoting can change the text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Text(String),
    Sign,
    Percent,
    Currency,
}

/// A parsed pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub grouping: Grouping,
    pub positive: (Vec<Token>, Vec<Token>),
    pub negative: Option<(Vec<Token>, Vec<Token>)>,
}

impl Parsed {
    /// The encoder's input, borrowing the tokens' text.
    pub fn spec(&self) -> PatternSpec<'_> {
        fn parts(ts: &[Token]) -> Vec<AffixPart<'_>> {
            ts.iter()
                .map(|t| match t {
                    Token::Text(s) => AffixPart::Text(s),
                    Token::Sign => AffixPart::Sign,
                    Token::Percent => AffixPart::Percent,
                    Token::Currency => AffixPart::Currency,
                })
                .collect()
        }
        PatternSpec {
            grouping: self.grouping,
            positive: (parts(&self.positive.0), parts(&self.positive.1)),
            negative: self.negative.as_ref().map(|(p, s)| (parts(p), parts(s))),
        }
    }
}

fn bad(pattern: &str, message: &'static str) -> Error {
    Error::Pattern {
        pattern: pattern.to_owned(),
        message,
    }
}

fn is_number_char(c: char) -> bool {
    matches!(c, '#' | '0'..='9' | ',' | '.')
}

/// Splits `p` at its unquoted `;`.
fn subpatterns(p: &str) -> Result<Vec<&str>, Error> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    for (i, c) in p.char_indices() {
        match c {
            '\'' => quoted = !quoted,
            ';' if !quoted => {
                out.push(&p[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if quoted {
        return Err(bad(p, "unterminated quote"));
    }
    out.push(&p[start..]);
    if out.len() > 2 {
        return Err(bad(p, "more than two subpatterns"));
    }
    Ok(out)
}

/// One subpattern → (prefix tokens, number part, suffix tokens).
fn subpattern<'p>(whole: &str, sub: &'p str) -> Result<(Vec<Token>, &'p str, Vec<Token>), Error> {
    // The number part: the first maximal run of number characters outside quotes.
    let mut quoted = false;
    let mut num_start = None;
    let mut num_end = sub.len();
    for (i, c) in sub.char_indices() {
        if c == '\'' {
            quoted = !quoted;
            if num_start.is_some() {
                num_end = i;
                break;
            }
            continue;
        }
        let number = !quoted && is_number_char(c);
        match (num_start, number) {
            (None, true) => num_start = Some(i),
            (Some(_), false) => {
                num_end = i;
                break;
            }
            _ => {}
        }
    }
    let start = num_start.ok_or_else(|| bad(whole, "no number part"))?;
    let number = &sub[start..num_end];
    let prefix = affix(whole, &sub[..start])?;
    let suffix = affix(whole, &sub[num_end..])?;
    Ok((prefix, number, suffix))
}

/// Affix text → tokens (`'` quoting; `''` is a quote).
fn affix(whole: &str, s: &str) -> Result<Vec<Token>, Error> {
    let mut out: Vec<Token> = Vec::new();
    let mut text = String::new();
    let mut chars = s.chars().peekable();
    let mut quoted = false;
    let flush = |text: &mut String, out: &mut Vec<Token>| {
        if !text.is_empty() {
            out.push(Token::Text(std::mem::take(text)));
        }
    };
    while let Some(c) = chars.next() {
        if c == '\'' {
            if chars.peek() == Some(&'\'') {
                chars.next();
                text.push('\'');
            } else {
                quoted = !quoted;
            }
            continue;
        }
        if quoted {
            text.push(c);
            continue;
        }
        let token = match c {
            '-' => Token::Sign,
            '%' => Token::Percent,
            '¤' => {
                if chars.peek() == Some(&'¤') {
                    return Err(bad(whole, "`¤¤` (ISO code or name placeholders)"));
                }
                Token::Currency
            }
            '‰' => return Err(bad(whole, "`‰` (no per-mille style in MF2)")),
            '+' => return Err(bad(whole, "`+` in an affix")),
            '*' => return Err(bad(whole, "padding")),
            '#' | '0'..='9' | ',' | '.' | '@' | 'E' | ';' => {
                return Err(bad(whole, "a number character in an affix"));
            }
            c if c < ' ' => return Err(bad(whole, "a control character in an affix")),
            c => {
                text.push(c);
                continue;
            }
        };
        flush(&mut text, &mut out);
        out.push(token);
    }
    flush(&mut text, &mut out);
    Ok(out)
}

/// The grouping of a number part (`#,##,##0.###` → 3, 2).
fn grouping(whole: &str, number: &str) -> Result<Grouping, Error> {
    let int = number.split('.').next().unwrap_or("");
    let groups: Vec<&str> = int.split(',').collect();
    if groups.iter().skip(1).any(|g| g.is_empty()) {
        return Err(bad(whole, "an empty group"));
    }
    let size =
        |g: &str| u8::try_from(g.len()).map_err(|_| bad(whole, "a group of over 255 digits"));
    let g = match groups.as_slice() {
        [] | [_] => Grouping::NONE,
        [.., last] if groups.len() == 2 => {
            let p = size(last)?;
            Grouping {
                primary: p,
                secondary: p,
            }
        }
        [.., second, last] => Grouping {
            primary: size(last)?,
            secondary: size(second)?,
        },
    };
    if g.to_byte().is_none() {
        return Err(bad(whole, "a group of over 15 digits"));
    }
    Ok(g)
}

/// Parses one CLDR number pattern.
pub fn parse(p: &str) -> Result<Parsed, Error> {
    let subs = subpatterns(p)?;
    // `E` and `@` are not number characters here, so a scientific or
    // significant-digit pattern fails in `affix`.
    let (pos_prefix, number, pos_suffix) = subpattern(p, subs.first().copied().unwrap_or(""))?;
    let grouping = grouping(p, number)?;
    let negative = match subs.get(1) {
        Some(n) => {
            let (prefix, _, suffix) = subpattern(p, n)?;
            Some((prefix, suffix))
        }
        None => None,
    };
    Ok(Parsed {
        grouping,
        positive: (pos_prefix, pos_suffix),
        negative,
    })
}

#[cfg(test)]
mod tests {
    use super::{Parsed, Token, parse};
    use mf2_catalog::number::Grouping;

    fn g(primary: u8, secondary: u8) -> Grouping {
        Grouping { primary, secondary }
    }

    fn text(s: &str) -> Token {
        Token::Text(s.to_owned())
    }

    #[test]
    fn cldr_shapes() {
        assert_eq!(
            parse("#,##0.###").expect("decimal"),
            Parsed {
                grouping: g(3, 3),
                positive: (vec![], vec![]),
                negative: None,
            }
        );
        assert_eq!(parse("#,##,##0.###").expect("hi").grouping, g(3, 2));
        assert_eq!(parse("#,#0.###").expect("blo").grouping, g(2, 2));
        assert_eq!(parse("0.######").expect("none").grouping, Grouping::NONE);
        assert_eq!(
            parse("#,##0\u{a0}%").expect("fr").positive,
            (vec![], vec![text("\u{a0}"), Token::Percent])
        );
        let acc = parse("¤#,##0.00;(¤#,##0.00)").expect("en accounting");
        assert_eq!(acc.positive, (vec![Token::Currency], vec![]));
        assert_eq!(
            acc.negative,
            Some((vec![text("("), Token::Currency], vec![text(")")]))
        );
        let ar = parse("\u{200f}#,##0.00\u{a0}¤;\u{200f}-#,##0.00\u{a0}¤").expect("ar");
        assert_eq!(
            ar.negative,
            Some((
                vec![text("\u{200f}"), Token::Sign],
                vec![text("\u{a0}"), Token::Currency]
            ))
        );
        assert_eq!(
            parse("%\u{a0}#,#0;%\u{a0}-#,#0")
                .expect("blo percent")
                .negative,
            Some((vec![Token::Percent, text("\u{a0}"), Token::Sign], vec![]))
        );
        assert_eq!(
            parse("'#'#,##0").expect("quoted").positive.0,
            vec![text("#")]
        );
        assert_eq!(parse("''#,##0").expect("quote").positive.0, vec![text("'")]);
    }

    #[test]
    fn refused() {
        for p in [
            "#,##0‰",
            "+#,##0",
            "*x#,##0",
            "#E0",
            "@@#",
            "¤¤#,##0",
            "abc",
            "#,##0;#;#",
            "'#,##0",
            "#,,##0",
        ] {
            assert!(parse(p).is_err(), "{p}");
        }
    }
}
