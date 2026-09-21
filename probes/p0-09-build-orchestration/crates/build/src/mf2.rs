//! A small MF2 scanner: "parses enough MF2" to extract each message's
//! external variables, markup names and functions, and to lower the message
//! into the probe's catalog model. Not a conforming parser (that is
//! `mf2-syntax`, Phase 1): it accepts the reference workload's constructs —
//! simple and quoted patterns, placeholders with options and attributes,
//! markup, `.input`, `.local`, `.match` with any number of selectors.

use std::collections::{BTreeMap, BTreeSet};

use p09_catalog::Func;

/// An operand as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// `$name`.
    Var(String),
    /// A literal (quoted or unquoted).
    Lit(String),
    /// No operand (`{:fn}`).
    None,
}

/// A pattern part, with variables still named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NPart {
    /// Unescaped text.
    Text(String),
    /// An expression placeholder.
    Expr(Operand),
    /// `{#name}`.
    Open(String),
    /// `{/name}`.
    Close(String),
    /// `{#name /}`.
    Standalone(String),
}

/// A variant key as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NKey {
    /// `*`.
    Star,
    /// A literal.
    Lit(String),
}

/// A lowered message, variables still named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NBody {
    /// One pattern.
    Pattern(Vec<NPart>),
    /// `.match`: selectors (variable name, function) and variants.
    Select {
        /// Selector variables and their selection function.
        selectors: Vec<(Operand, Func)>,
        /// (keys, pattern) per variant.
        variants: Vec<(Vec<NKey>, Vec<NPart>)>,
    },
}

/// What the scanner extracts from one message.
#[derive(Debug, Clone)]
pub struct Scanned {
    /// The lowered body; locals already resolved to their operands.
    pub body: NBody,
    /// External variables.
    pub externals: BTreeSet<String>,
    /// Markup names.
    pub markup: BTreeSet<String>,
    /// Function names (without the `:`).
    pub functions: BTreeSet<String>,
}

struct Scanner<'a> {
    s: &'a str,
    i: usize,
    vars: BTreeSet<String>,
    markup: BTreeSet<String>,
    functions: BTreeSet<String>,
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(c, '_' | '-' | '.' | '+' | ':')
        || (!c.is_ascii() && !c.is_whitespace())
}

type R<T> = Result<T, String>;

impl<'a> Scanner<'a> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }
    fn peek2(&self) -> Option<char> {
        let mut it = self.s[self.i..].chars();
        it.next();
        it.next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += c.len_utf8();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) -> R<()> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(format!("expected `{c}` at byte {}", self.i))
        }
    }
    fn ws(&mut self) {
        while self.peek().is_some_and(|c| {
            c.is_whitespace()
                || matches!(
                    c,
                    '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{2066}'..='\u{2069}'
                )
        }) {
            self.bump();
        }
    }
    fn name(&mut self) -> R<String> {
        let start = self.i;
        while self.peek().is_some_and(is_name_char) {
            self.bump();
        }
        if self.i == start {
            return Err(format!("expected a name at byte {start}"));
        }
        Ok(self.s[start..self.i].to_owned())
    }
    fn quoted(&mut self) -> R<String> {
        self.expect('|')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err("unterminated `|literal|`".into()),
                Some('|') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some(c @ ('\\' | '|' | '{' | '}')) => out.push(c),
                    _ => return Err("bad escape in literal".into()),
                },
                Some(c) => out.push(c),
            }
        }
    }
    fn literal(&mut self) -> R<String> {
        if self.peek() == Some('|') {
            self.quoted()
        } else {
            self.name()
        }
    }

    /// The inside of `{ … }` after the `{`; consumes the `}`.
    fn placeholder(&mut self) -> R<(NPart, Option<String>)> {
        self.ws();
        let part = match self.peek() {
            Some('#') => {
                self.bump();
                let n = self.name()?;
                self.markup.insert(n.clone());
                let standalone = self.rest_of_placeholder()?;
                return Ok((
                    if standalone {
                        NPart::Standalone(n)
                    } else {
                        NPart::Open(n)
                    },
                    None,
                ));
            }
            Some('/') => {
                self.bump();
                let n = self.name()?;
                self.markup.insert(n.clone());
                self.rest_of_placeholder()?;
                return Ok((NPart::Close(n), None));
            }
            Some('$') => {
                self.bump();
                let n = self.name()?;
                self.vars.insert(n.clone());
                NPart::Expr(Operand::Var(n))
            }
            Some(':') => NPart::Expr(Operand::None),
            Some(_) => NPart::Expr(Operand::Lit(self.literal()?)),
            None => return Err("unterminated placeholder".into()),
        };
        let func = self.annotation()?;
        self.rest_of_placeholder()?;
        Ok((part, func))
    }

    /// An optional `:function` after an operand.
    fn annotation(&mut self) -> R<Option<String>> {
        self.ws();
        if self.peek() == Some(':') {
            self.bump();
            let f = self.name()?;
            self.functions.insert(f.clone());
            return Ok(Some(f));
        }
        Ok(None)
    }

    /// Options, attributes, `/` — up to and including `}`. Returns whether
    /// the placeholder was self-closing (`/}`). Records `$var` option values.
    fn rest_of_placeholder(&mut self) -> R<bool> {
        let mut slash = false;
        loop {
            self.ws();
            match self.peek() {
                None => return Err("unterminated placeholder".into()),
                Some('}') => {
                    self.bump();
                    return Ok(slash);
                }
                Some('/') => {
                    self.bump();
                    slash = true;
                }
                Some('|') => {
                    self.quoted()?;
                }
                Some('$') => {
                    self.bump();
                    let n = self.name()?;
                    self.vars.insert(n);
                }
                Some('=' | '@') => {
                    self.bump();
                }
                Some(c) if is_name_char(c) => {
                    self.name()?;
                }
                Some(c) => return Err(format!("unexpected `{c}` in placeholder")),
            }
        }
    }

    /// A pattern; `quoted` = inside `{{ … }}` (consumes the closing `}}`).
    fn pattern(&mut self, quoted: bool) -> R<Vec<NPart>> {
        let mut parts = Vec::new();
        let mut text = String::new();
        loop {
            match self.peek() {
                None if quoted => return Err("unterminated `{{pattern}}`".into()),
                None => break,
                Some('\\') => {
                    self.bump();
                    match self.bump() {
                        Some(c @ ('\\' | '{' | '|' | '}')) => text.push(c),
                        _ => return Err("bad escape in text".into()),
                    }
                }
                Some('{') => {
                    self.bump();
                    if !text.is_empty() {
                        parts.push(NPart::Text(std::mem::take(&mut text)));
                    }
                    let (part, _func) = self.placeholder()?;
                    parts.push(part);
                }
                Some('}') if quoted && self.peek2() == Some('}') => {
                    self.bump();
                    self.bump();
                    break;
                }
                Some('}') => return Err("unescaped `}` in text".into()),
                Some(c) => {
                    self.bump();
                    text.push(c);
                }
            }
        }
        if !text.is_empty() {
            parts.push(NPart::Text(text));
        }
        Ok(parts)
    }

    fn complex(&mut self) -> R<NBody> {
        // name → (operand, function) for .local; name → function for .input
        let mut locals: BTreeMap<String, (Operand, Option<String>)> = BTreeMap::new();
        let mut inputs: BTreeMap<String, Option<String>> = BTreeMap::new();
        loop {
            self.ws();
            if self.s[self.i..].starts_with(".input") {
                self.i += ".input".len();
                self.ws();
                self.expect('{')?;
                let (part, func) = self.placeholder()?;
                let NPart::Expr(Operand::Var(n)) = part else {
                    return Err(".input needs a variable".into());
                };
                inputs.insert(n, func);
            } else if self.s[self.i..].starts_with(".local") {
                self.i += ".local".len();
                self.ws();
                self.expect('$')?;
                let n = self.name()?;
                self.ws();
                self.expect('=')?;
                self.ws();
                self.expect('{')?;
                let (part, func) = self.placeholder()?;
                let NPart::Expr(op) = part else {
                    return Err(".local needs an expression".into());
                };
                locals.insert(n, (op, func));
            } else if self.s[self.i..].starts_with(".match") {
                self.i += ".match".len();
                let mut selectors = Vec::new();
                loop {
                    self.ws();
                    if self.peek() != Some('$') {
                        break;
                    }
                    self.bump();
                    let n = self.name()?;
                    self.vars.insert(n.clone());
                    selectors.push(n);
                }
                if selectors.is_empty() {
                    return Err(".match needs a selector".into());
                }
                let mut variants = Vec::new();
                loop {
                    self.ws();
                    if self.peek().is_none() {
                        break;
                    }
                    let mut keys = Vec::new();
                    while self.peek() != Some('{') {
                        if self.eat('*') {
                            keys.push(NKey::Star);
                        } else {
                            keys.push(NKey::Lit(self.literal()?));
                        }
                        self.ws();
                    }
                    if keys.len() != selectors.len() {
                        return Err("variant key count differs from selector count".into());
                    }
                    self.expect('{')?;
                    self.expect('{')?;
                    let pattern = self.pattern(true)?;
                    variants.push((keys, pattern));
                }
                let resolve = |n: &String| -> (Operand, Func) {
                    let (op, f) = match locals.get(n) {
                        Some((op, f)) => (op.clone(), f.clone()),
                        None => (Operand::Var(n.clone()), inputs.get(n).cloned().flatten()),
                    };
                    let func = match f.as_deref() {
                        Some("integer") => Func::Integer,
                        Some("number" | "offset") => Func::Number,
                        _ => Func::Plain,
                    };
                    (op, func)
                };
                let selectors = selectors.iter().map(resolve).collect();
                let variants = variants
                    .into_iter()
                    .map(|(k, p)| (k, resolve_locals(p, &locals)))
                    .collect();
                self.finish_vars(&locals);
                return Ok(NBody::Select {
                    selectors,
                    variants,
                });
            } else if self.s[self.i..].starts_with("{{") {
                self.i += 2;
                let p = self.pattern(true)?;
                self.ws();
                if self.peek().is_some() {
                    return Err("trailing content after quoted pattern".into());
                }
                self.finish_vars(&locals);
                return Ok(NBody::Pattern(resolve_locals(p, &locals)));
            } else {
                return Err(format!("unexpected content at byte {}", self.i));
            }
        }
    }

    /// Locals are not external variables.
    fn finish_vars(&mut self, locals: &BTreeMap<String, (Operand, Option<String>)>) {
        for n in locals.keys() {
            self.vars.remove(n);
        }
    }
}

fn resolve_locals(
    parts: Vec<NPart>,
    locals: &BTreeMap<String, (Operand, Option<String>)>,
) -> Vec<NPart> {
    parts
        .into_iter()
        .map(|p| match p {
            NPart::Expr(Operand::Var(n)) => match locals.get(&n) {
                Some((op, _)) => NPart::Expr(op.clone()),
                None => NPart::Expr(Operand::Var(n)),
            },
            other => other,
        })
        .collect()
}

/// Scans one message.
pub fn scan(src: &str) -> Result<Scanned, String> {
    let mut sc = Scanner {
        s: src,
        i: 0,
        vars: BTreeSet::new(),
        markup: BTreeSet::new(),
        functions: BTreeSet::new(),
    };
    let t = src.trim_start();
    let body = if t.starts_with('.') || t.starts_with("{{") {
        sc.complex()?
    } else {
        NBody::Pattern(sc.pattern(false)?)
    };
    Ok(Scanned {
        body,
        externals: sc.vars,
        markup: sc.markup,
        functions: sc.functions,
    })
}

#[cfg(test)]
mod tests {
    use super::{NBody, scan};

    #[test]
    fn simple_and_complex() {
        let s = scan("Hi {$name}, {#b}x{/b} {$n :number minimumFractionDigits=$d u:locale=en} \\{")
            .unwrap();
        assert_eq!(
            s.externals.iter().cloned().collect::<Vec<_>>(),
            ["d", "n", "name"]
        );
        assert_eq!(s.markup.iter().cloned().collect::<Vec<_>>(), ["b"]);
        assert_eq!(s.functions.iter().cloned().collect::<Vec<_>>(), ["number"]);
        let m = scan(".input {$count :integer}\n.local $x = {$who}\n.match $count\none {{{$count} {$x}}}\n* {{many}}").unwrap();
        assert_eq!(
            m.externals.iter().cloned().collect::<Vec<_>>(),
            ["count", "who"]
        );
        assert!(matches!(m.body, NBody::Select { .. }));
    }
}
