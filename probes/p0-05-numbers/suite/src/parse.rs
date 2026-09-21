//! A minimal MF2 parser — just enough of the syntax for the functions/*.json
//! suite files (declarations, `.match`, placeholders with operands,
//! annotations, options and attributes; no markup). Dev-only probe code.

#[derive(Debug, Clone)]
pub enum Operand {
    Lit(String),
    Var(String),
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: String,
    pub opts: Vec<(String, Operand)>,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub operand: Option<Operand>,
    pub ann: Option<Annotation>,
}

#[derive(Debug, Clone)]
pub enum Part {
    Text(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Decl {
    Input(String, Expr),
    Local(String, Expr),
}

#[derive(Debug, Clone)]
pub enum Key {
    Lit(String),
    Star,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub keys: Vec<Key>,
    pub pattern: Vec<Part>,
}

#[derive(Debug, Clone)]
pub enum Body {
    Pattern(Vec<Part>),
    Match(Vec<String>, Vec<Variant>),
}

#[derive(Debug, Clone)]
pub struct Message {
    pub decls: Vec<Decl>,
    pub body: Body,
}

struct P<'a> {
    s: &'a [char],
    i: usize,
}

type R<T> = Result<T, String>;

impl P<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) -> R<()> {
        if self.eat(c) { Ok(()) } else { Err(format!("expected {c:?} at {}", self.i)) }
    }
    fn ws(&mut self) -> bool {
        let start = self.i;
        while matches!(self.peek(), Some(' ' | '\t' | '\r' | '\n' | '\u{3000}')) {
            self.i += 1;
        }
        self.i > start
    }
    fn starts_with(&self, t: &str) -> bool {
        t.chars().enumerate().all(|(k, c)| self.s.get(self.i + k) == Some(&c))
    }
    fn name(&mut self) -> R<String> {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '+') {
                self.i += 1;
            } else {
                break;
            }
        }
        if self.i == start {
            return Err(format!("expected name at {}", self.i));
        }
        Ok(self.s[start..self.i].iter().collect())
    }
    fn identifier(&mut self) -> R<String> {
        let mut n = self.name()?;
        if self.eat(':') {
            n.push(':');
            n.push_str(&self.name()?);
        }
        Ok(n)
    }
    fn literal(&mut self) -> R<String> {
        if self.eat('|') {
            let mut out = String::new();
            loop {
                match self.peek() {
                    None => return Err("unterminated quoted literal".into()),
                    Some('|') => {
                        self.i += 1;
                        return Ok(out);
                    }
                    Some('\\') => {
                        self.i += 1;
                        out.extend(self.peek());
                        self.i += 1;
                    }
                    Some(c) => {
                        out.push(c);
                        self.i += 1;
                    }
                }
            }
        }
        self.name()
    }
    fn operand_or_lit(&mut self) -> R<Operand> {
        if self.eat('$') { Ok(Operand::Var(self.name()?)) } else { Ok(Operand::Lit(self.literal()?)) }
    }
    fn expr(&mut self) -> R<Expr> {
        self.expect('{')?;
        self.ws();
        let operand = if matches!(self.peek(), Some(':' | '@' | '}')) { None } else { Some(self.operand_or_lit()?) };
        self.ws();
        let mut ann = None;
        if self.eat(':') {
            let name = self.identifier()?;
            let mut opts = Vec::new();
            loop {
                let save = self.i;
                if !self.ws() || matches!(self.peek(), Some('@' | '}')) {
                    self.i = save;
                    break;
                }
                let k = self.identifier()?;
                self.ws();
                self.expect('=')?;
                self.ws();
                opts.push((k, self.operand_or_lit()?));
            }
            ann = Some(Annotation { name, opts });
        }
        // Attributes: parsed and dropped (they never affect formatting).
        loop {
            self.ws();
            if !self.eat('@') {
                break;
            }
            self.identifier()?;
            let save = self.i;
            self.ws();
            if self.eat('=') {
                self.ws();
                self.literal()?;
            } else {
                self.i = save;
            }
        }
        self.ws();
        self.expect('}')?;
        Ok(Expr { operand, ann })
    }
    fn pattern(&mut self, quoted: bool) -> R<Vec<Part>> {
        let mut parts = Vec::new();
        let mut text = String::new();
        loop {
            match self.peek() {
                None => break,
                Some('}') if quoted => break,
                Some('{') => {
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    parts.push(Part::Expr(self.expr()?));
                }
                Some('\\') => {
                    self.i += 1;
                    text.extend(self.peek());
                    self.i += 1;
                }
                Some(c) => {
                    text.push(c);
                    self.i += 1;
                }
            }
        }
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        Ok(parts)
    }
    fn quoted_pattern(&mut self) -> R<Vec<Part>> {
        self.expect('{')?;
        self.expect('{')?;
        let p = self.pattern(true)?;
        self.expect('}')?;
        self.expect('}')?;
        Ok(p)
    }
}

pub fn parse(src: &str) -> R<Message> {
    let chars: Vec<char> = src.chars().collect();
    let mut p = P { s: &chars, i: 0 };
    p.ws();
    if !(p.starts_with(".") || p.starts_with("{{")) {
        p.i = 0;
        return Ok(Message { decls: Vec::new(), body: Body::Pattern(p.pattern(false)?) });
    }
    let mut decls = Vec::new();
    loop {
        p.ws();
        if p.starts_with(".input") {
            p.i += 6;
            p.ws();
            let e = p.expr()?;
            let Some(Operand::Var(v)) = e.operand.clone() else { return Err(".input needs a variable".into()) };
            decls.push(Decl::Input(v, e));
        } else if p.starts_with(".local") {
            p.i += 6;
            p.ws();
            p.expect('$')?;
            let v = p.name()?;
            p.ws();
            p.expect('=')?;
            p.ws();
            decls.push(Decl::Local(v, p.expr()?));
        } else {
            break;
        }
    }
    if p.starts_with("{{") {
        let body = Body::Pattern(p.quoted_pattern()?);
        return Ok(Message { decls, body });
    }
    if !p.starts_with(".match") {
        return Err(format!("expected body at {}", p.i));
    }
    p.i += 6;
    let mut sels = Vec::new();
    loop {
        p.ws();
        if !p.eat('$') {
            break;
        }
        sels.push(p.name()?);
    }
    let mut variants = Vec::new();
    loop {
        p.ws();
        if p.peek().is_none() {
            break;
        }
        let mut keys = Vec::new();
        while !p.starts_with("{{") {
            if p.eat('*') {
                keys.push(Key::Star);
            } else {
                keys.push(Key::Lit(p.literal()?));
            }
            p.ws();
        }
        variants.push(Variant { keys, pattern: p.quoted_pattern()? });
    }
    Ok(Message { decls, body: Body::Match(sels, variants) })
}
