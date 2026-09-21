//! Throwaway MF2 parser (spec/message.abnf) for the constructs the reference
//! workload uses — text, escapes, `{$v}`, `{$v :fn opts}`, literals (quoted and
//! unquoted), function-only expressions, attributes, markup open / close /
//! standalone with options, `.input` / `.local`, `.match` with literal and `*`
//! keys. It is lenient about required whitespace and performs no data-model
//! validation. Not product code (the product parser is `mf2-syntax`, Phase 1).

use crate::error::ParseError;
use crate::model::{Attribute, Decl, Expr, FunctionRef, Key, Markup, MarkupKind, Message, Operand, Part, Pattern, Variant};

struct P<'a> {
    s: &'a str,
    i: usize,
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{3000}')
}

fn is_bidi(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{2066}'..='\u{2069}')
}

fn is_name_start(c: char) -> bool {
    let u = c as u32;
    c.is_ascii_alphabetic()
        || c == '+'
        || c == '_'
        || (0xA1..=0x61B).contains(&u)
        || (0x61D..=0x167F).contains(&u)
        || (0x1681..=0x1FFF).contains(&u)
        || (0x200B..=0x200D).contains(&u)
        || (0x2010..=0x2027).contains(&u)
        || (0x2030..=0x205E).contains(&u)
        || (0x2060..=0x2065).contains(&u)
        || (0x206A..=0x2FFF).contains(&u)
        || (0x3001..=0xD7FF).contains(&u)
        || (0xE000..=0xFDCF).contains(&u)
        || (0xFDF0..=0xFFFD).contains(&u)
        || (u >= 0x10000 && (u & 0xFFFE) != 0xFFFE)
}

fn is_name_char(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '-' || c == '.'
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }
    fn at(&self, lit: &str) -> bool {
        self.s[self.i..].starts_with(lit)
    }
    fn eat(&mut self, lit: &str) -> bool {
        if self.at(lit) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, lit: &'static str) -> Result<(), ParseError> {
        if self.eat(lit) { Ok(()) } else { Err(self.err(lit)) }
    }
    fn err(&self, expected: &'static str) -> ParseError {
        ParseError { at: self.i, expected }
    }
    /// `o` / `s` (lenient: `s` is not enforced).
    fn ws(&mut self) {
        while let Some(c) = self.peek() {
            if is_ws(c) || is_bidi(c) {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn name(&mut self) -> Result<String, ParseError> {
        while self.peek().is_some_and(is_bidi) {
            self.bump();
        }
        let start = self.i;
        match self.peek() {
            Some(c) if is_name_start(c) => {
                self.bump();
            }
            _ => return Err(self.err("name")),
        }
        while self.peek().is_some_and(is_name_char) {
            self.bump();
        }
        let n = self.s[start..self.i].to_owned();
        while self.peek().is_some_and(is_bidi) {
            self.bump();
        }
        Ok(n)
    }

    fn identifier(&mut self) -> Result<String, ParseError> {
        let mut id = self.name()?;
        if self.peek() == Some(':') {
            self.bump();
            id.push(':');
            id.push_str(&self.name()?);
        }
        Ok(id)
    }

    fn escaped(&mut self) -> Result<char, ParseError> {
        match self.bump() {
            Some(c @ ('\\' | '{' | '|' | '}')) => Ok(c),
            _ => Err(self.err("escape")),
        }
    }

    fn literal(&mut self) -> Result<String, ParseError> {
        if self.eat("|") {
            let mut v = String::new();
            loop {
                match self.bump() {
                    Some('|') => return Ok(v),
                    Some('\\') => v.push(self.escaped()?),
                    Some('\0') | None => return Err(self.err("|")),
                    Some(c) => v.push(c),
                }
            }
        }
        let start = self.i;
        while self.peek().is_some_and(is_name_char) {
            self.bump();
        }
        if self.i == start {
            return Err(self.err("literal"));
        }
        Ok(self.s[start..self.i].to_owned())
    }

    fn variable(&mut self) -> Result<String, ParseError> {
        self.expect("$")?;
        self.name()
    }

    fn operand_value(&mut self) -> Result<Operand, ParseError> {
        if self.at("$") { Ok(Operand::Variable(self.variable()?)) } else { Ok(Operand::Literal(self.literal()?)) }
    }

    fn options(&mut self) -> Result<Vec<(String, Operand)>, ParseError> {
        let mut out = Vec::new();
        loop {
            let save = self.i;
            self.ws();
            if self.peek().is_some_and(|c| is_name_start(c) || is_bidi(c)) {
                let name = self.identifier()?;
                self.ws();
                self.expect("=")?;
                self.ws();
                out.push((name, self.operand_value()?));
            } else {
                self.i = save;
                return Ok(out);
            }
        }
    }

    fn attributes(&mut self) -> Result<Vec<Attribute>, ParseError> {
        let mut out = Vec::new();
        loop {
            let save = self.i;
            self.ws();
            if self.eat("@") {
                let name = self.identifier()?;
                let save2 = self.i;
                self.ws();
                let value = if self.eat("=") {
                    self.ws();
                    Some(self.literal()?)
                } else {
                    self.i = save2;
                    None
                };
                out.push(Attribute { name, value });
            } else {
                self.i = save;
                return Ok(out);
            }
        }
    }

    /// After `{` has been consumed: an expression or markup, up to and including `}`.
    fn placeholder(&mut self) -> Result<Part, ParseError> {
        self.ws();
        match self.peek() {
            Some('#') | Some('/') => {
                let close = self.bump() == Some('/');
                let name = self.identifier()?;
                let options = self.options()?;
                let attributes = self.attributes()?;
                self.ws();
                let kind = if close {
                    MarkupKind::Close
                } else if self.eat("/") {
                    MarkupKind::Standalone
                } else {
                    MarkupKind::Open
                };
                self.expect("}")?;
                Ok(Part::Markup(Markup { kind, name, options, attributes }))
            }
            _ => Ok(Part::Expr(self.expression_body()?)),
        }
    }

    /// After `{` o: operand? function? attributes o `}`.
    fn expression_body(&mut self) -> Result<Expr, ParseError> {
        self.ws();
        let operand = match self.peek() {
            Some(':') => None,
            Some('$') => Some(Operand::Variable(self.variable()?)),
            _ => Some(Operand::Literal(self.literal()?)),
        };
        let save = self.i;
        self.ws();
        let function = if self.eat(":") {
            let name = self.identifier()?;
            Some(FunctionRef { name, options: self.options()? })
        } else {
            self.i = save;
            None
        };
        if operand.is_none() && function.is_none() {
            return Err(self.err("operand or function"));
        }
        let attributes = self.attributes()?;
        self.ws();
        self.expect("}")?;
        Ok(Expr { operand, function, attributes })
    }

    /// Pattern text and placeholders up to `}}` (quoted) or the end (simple).
    fn pattern(&mut self, quoted: bool) -> Result<Pattern, ParseError> {
        let mut parts = Vec::new();
        let mut text = String::new();
        loop {
            if quoted && self.at("}}") {
                self.i += 2;
                break;
            }
            match self.bump() {
                None if !quoted => break,
                None | Some('\0') | Some('}') => return Err(self.err("text")),
                Some('\\') => text.push(self.escaped()?),
                Some('{') => {
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    parts.push(self.placeholder()?);
                }
                Some(c) => text.push(c),
            }
        }
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        Ok(parts)
    }

    fn message(&mut self) -> Result<Message, ParseError> {
        self.ws();
        if !(self.at(".") || self.at("{{")) {
            self.i = 0;
            let pattern = self.pattern(false)?;
            return Ok(Message::Pattern { decls: Vec::new(), pattern });
        }
        let mut decls = Vec::new();
        loop {
            self.ws();
            if self.eat(".input") {
                self.ws();
                self.expect("{")?;
                let e = self.expression_body()?;
                let Some(Operand::Variable(name)) = e.operand.clone() else {
                    return Err(self.err("variable in .input"));
                };
                decls.push(Decl::Input { name, expr: Expr { operand: None, ..e } });
            } else if self.eat(".local") {
                self.ws();
                let name = self.variable()?;
                self.ws();
                self.expect("=")?;
                self.ws();
                self.expect("{")?;
                decls.push(Decl::Local { name, expr: self.expression_body()? });
            } else if self.eat(".match") {
                let mut selectors = Vec::new();
                loop {
                    self.ws();
                    if self.at("$") {
                        selectors.push(self.variable()?);
                    } else {
                        break;
                    }
                }
                let mut variants = Vec::new();
                loop {
                    self.ws();
                    if self.peek().is_none() {
                        break;
                    }
                    let mut keys = Vec::new();
                    while !self.at("{{") {
                        if self.eat("*") {
                            keys.push(Key::CatchAll);
                        } else {
                            keys.push(Key::Literal(self.literal()?));
                        }
                        self.ws();
                    }
                    self.expect("{{")?;
                    let pattern = self.pattern(true)?;
                    variants.push(Variant { keys, pattern });
                }
                return Ok(Message::Select { decls, selectors, variants });
            } else if self.eat("{{") {
                let pattern = self.pattern(true)?;
                self.ws();
                if self.peek().is_some() {
                    return Err(self.err("end of message"));
                }
                return Ok(Message::Pattern { decls, pattern });
            } else {
                return Err(self.err("declaration or body"));
            }
        }
    }
}

/// Parses one MF2 message into the data model. Input decls store the
/// expression without its (implied) operand.
pub fn parse(src: &str) -> Result<Message, ParseError> {
    P { s: src, i: 0 }.message()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_and_escapes() {
        let m = parse("  Hello \\{world\\} \\\\").unwrap();
        assert_eq!(m, Message::Pattern { decls: vec![], pattern: vec![Part::Text("  Hello {world} \\".into())] });
    }

    #[test]
    fn placeholders_markup_literals() {
        let m = parse("a {$x} {#b}c{/b} {|q\\|| :string u:dir=rtl @a=|1|} {#img src=|x| /}").unwrap();
        let Message::Pattern { pattern, .. } = m else { panic!() };
        assert_eq!(pattern.len(), 10);
        assert!(matches!(&pattern[3], Part::Markup(Markup { kind: MarkupKind::Open, name, .. }) if name == "b"));
        assert!(matches!(&pattern[9], Part::Markup(Markup { kind: MarkupKind::Standalone, options, .. }) if options.len() == 1));
        let Part::Expr(e) = &pattern[7] else { panic!() };
        assert_eq!(e.operand, Some(Operand::Literal("q|".into())));
        assert_eq!(e.function.as_ref().unwrap().options[0].0, "u:dir");
        assert_eq!(e.attributes[0].value.as_deref(), Some("1"));
    }

    #[test]
    fn select_with_decls() {
        let m = parse(".input {$count :integer}\n.local $y = {$count :number minimumFractionDigits=1}\n.match $count\none {{One {$count}}}\n* {{Other}}").unwrap();
        let Message::Select { decls, selectors, variants } = m else { panic!() };
        assert_eq!(decls.len(), 2);
        assert_eq!(selectors, vec!["count"]);
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[1].keys, vec![Key::CatchAll]);
    }
}
