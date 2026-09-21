//! The serializer: data model → canonical MF2 source.
//!
//! Canonical form:
//!
//! * a pattern message without declarations is a **simple message** — unless
//!   its text, after leading whitespace and bidi marks, starts with `.`
//!   (which would read as a keyword); then it is a quoted pattern `{{…}}`;
//! * otherwise one declaration per line (`.input {$x :number}`,
//!   `.local $y = {$x}`), then `{{…}}`, or `.match $a $b` and one variant per
//!   line (`key key {{…}}`);
//! * single spaces between the parts of an expression (`{$x :fn opt=v @a}`),
//!   `{#name /}` for standalone markup;
//! * literals unquoted when they match `unquoted-literal`, else `|…|` with `\`
//!   and `|` escaped; text escapes exactly `\`, `{` and `}`.
//!
//! `parse_model(serialize(m)?)` gives back `m` for every model a parse can
//! produce. A catch-all key's `value` (set only by other formats) is not
//! representable in MF2 syntax and is dropped: the key is written `*`.

use alloc::string::String;

use mf2_model::{
    Attributes, Declaration, Expression, FunctionRef, Key, Literal, Markup, MarkupKind, Message,
    OptionValue, Options, Pattern, PatternPart,
};

use crate::chars;
use crate::error::{Error, NameRole};

/// Serializes `message` as canonical MF2 source.
///
/// Fails if MF2 syntax cannot represent the model: U+0000 in text or a
/// literal, an invalid name, an input declaration whose name differs from its
/// variable, or a select message without selectors, variants or keys.
pub fn serialize(message: &Message<'_>) -> Result<String, Error> {
    let mut w = Writer(String::new());
    match message {
        Message::Pattern(m) if m.declarations.is_empty() => {
            w.pattern(&m.pattern)?;
            if reads_as_complex(&w.0) {
                let mut quoted = String::with_capacity(w.0.len() + 4);
                quoted.push_str("{{");
                quoted.push_str(&w.0);
                quoted.push_str("}}");
                return Ok(quoted);
            }
        }
        Message::Pattern(m) => {
            w.declarations(&m.declarations)?;
            w.quoted_pattern(&m.pattern)?;
        }
        Message::Select(m) => {
            if m.selectors.is_empty() {
                return Err(Error::NoSelectors);
            }
            if m.variants.is_empty() {
                return Err(Error::NoVariants);
            }
            w.declarations(&m.declarations)?;
            w.0.push_str(".match");
            for s in &m.selectors {
                w.0.push_str(" $");
                w.name(&s.name, NameRole::Variable)?;
            }
            for v in &m.variants {
                if v.keys.is_empty() {
                    return Err(Error::NoKeys);
                }
                w.0.push('\n');
                for (i, k) in v.keys.iter().enumerate() {
                    if i > 0 {
                        w.0.push(' ');
                    }
                    match k {
                        Key::CatchAll(_) => w.0.push('*'),
                        Key::Literal(l) => w.literal(l)?,
                    }
                }
                w.0.push(' ');
                w.quoted_pattern(&v.value)?;
            }
        }
    }
    Ok(w.0)
}

/// Whether text written as a simple message would parse as a complex one:
/// its first character after whitespace and bidi marks is `.`.
fn reads_as_complex(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while let Some((n, _)) = chars::trivia_at(b, i) {
        i += n;
    }
    b.get(i) == Some(&b'.')
}

struct Writer(String);

impl Writer {
    fn declarations(&mut self, declarations: &[Declaration<'_>]) -> Result<(), Error> {
        for d in declarations {
            match d {
                Declaration::Input(x) => {
                    if x.name != x.value.arg.name {
                        return Err(Error::InputNameMismatch);
                    }
                    self.0.push_str(".input {$");
                    self.name(&x.value.arg.name, NameRole::Variable)?;
                    self.expression_tail(x.value.function.as_ref(), &x.value.attributes)?;
                }
                Declaration::Local(x) => {
                    self.0.push_str(".local $");
                    self.name(&x.name, NameRole::Variable)?;
                    self.0.push_str(" = ");
                    self.expression(&x.value)?;
                }
            }
            self.0.push('\n');
        }
        Ok(())
    }

    fn quoted_pattern(&mut self, p: &Pattern<'_>) -> Result<(), Error> {
        self.0.push_str("{{");
        self.pattern(p)?;
        self.0.push_str("}}");
        Ok(())
    }

    fn pattern(&mut self, p: &Pattern<'_>) -> Result<(), Error> {
        for part in p.parts() {
            match part {
                PatternPart::Text(t) => self.text(t)?,
                PatternPart::Expression(e) => self.expression(e)?,
                PatternPart::Markup(m) => self.markup(m)?,
                _ => return Err(Error::UnknownNode),
            }
        }
        Ok(())
    }

    fn text(&mut self, t: &str) -> Result<(), Error> {
        for c in t.chars() {
            match c {
                '\0' => return Err(Error::Nul),
                '\\' | '{' | '}' => {
                    self.0.push('\\');
                    self.0.push(c);
                }
                c => self.0.push(c),
            }
        }
        Ok(())
    }

    fn expression(&mut self, e: &Expression<'_>) -> Result<(), Error> {
        self.0.push('{');
        match e {
            Expression::Literal(x) => {
                self.literal(&x.arg)?;
                self.expression_tail(x.function.as_ref(), &x.attributes)
            }
            Expression::Variable(x) => {
                self.0.push('$');
                self.name(&x.arg.name, NameRole::Variable)?;
                self.expression_tail(x.function.as_ref(), &x.attributes)
            }
            Expression::Function(x) => {
                self.function(&x.function)?;
                self.attributes(&x.attributes)?;
                self.0.push('}');
                Ok(())
            }
            _ => Err(Error::UnknownNode),
        }
    }

    /// ` :function opts @attrs}` after an operand.
    fn expression_tail(
        &mut self,
        function: Option<&FunctionRef<'_>>,
        attributes: &Attributes<'_>,
    ) -> Result<(), Error> {
        if let Some(f) = function {
            self.0.push(' ');
            self.function(f)?;
        }
        self.attributes(attributes)?;
        self.0.push('}');
        Ok(())
    }

    fn function(&mut self, f: &FunctionRef<'_>) -> Result<(), Error> {
        self.0.push(':');
        self.name(&f.name, NameRole::Function)?;
        self.options(&f.options)
    }

    fn options(&mut self, options: &Options<'_>) -> Result<(), Error> {
        for (name, value) in options.iter() {
            self.0.push(' ');
            self.name(name, NameRole::Option)?;
            self.0.push('=');
            match value {
                OptionValue::Literal(l) => self.literal(l)?,
                OptionValue::Variable(v) => {
                    self.0.push('$');
                    self.name(&v.name, NameRole::Variable)?;
                }
            }
        }
        Ok(())
    }

    fn attributes(&mut self, attributes: &Attributes<'_>) -> Result<(), Error> {
        for (name, value) in attributes.iter() {
            self.0.push_str(" @");
            self.name(name, NameRole::Attribute)?;
            if let Some(l) = value {
                self.0.push('=');
                self.literal(l)?;
            }
        }
        Ok(())
    }

    fn markup(&mut self, m: &Markup<'_>) -> Result<(), Error> {
        self.0.push_str(match m.kind {
            MarkupKind::Close => "{/",
            MarkupKind::Open | MarkupKind::Standalone => "{#",
        });
        self.name(&m.name, NameRole::Markup)?;
        self.options(&m.options)?;
        self.attributes(&m.attributes)?;
        self.0.push_str(match m.kind {
            MarkupKind::Standalone => " /}",
            MarkupKind::Open | MarkupKind::Close => "}",
        });
        Ok(())
    }

    fn literal(&mut self, l: &Literal<'_>) -> Result<(), Error> {
        if chars::is_unquoted_literal(&l.value) {
            self.0.push_str(&l.value);
            return Ok(());
        }
        self.0.push('|');
        for c in l.value.chars() {
            match c {
                '\0' => return Err(Error::Nul),
                '\\' | '|' => {
                    self.0.push('\\');
                    self.0.push(c);
                }
                c => self.0.push(c),
            }
        }
        self.0.push('|');
        Ok(())
    }

    /// A variable name must be a `name`; the others may be namespaced
    /// `identifier`s.
    fn name(&mut self, name: &str, role: NameRole) -> Result<(), Error> {
        let ok = match role {
            NameRole::Variable => chars::is_name(name),
            _ => chars::is_identifier(name),
        };
        if !ok {
            return Err(Error::InvalidName(role));
        }
        self.0.push_str(name);
        Ok(())
    }
}
