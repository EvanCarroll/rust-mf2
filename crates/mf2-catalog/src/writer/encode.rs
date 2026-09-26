//! One pattern or select message → its MESSAGES record and its COLD record
//! (`plans/02-catalog-format.md` §2.2, §2.5).
//!
//! The encoder numbers the COLD sites exactly as the decoder does: in the
//! order their bytes appear in the record. Each site takes its number when
//! it is written, and its override (if any) is recorded right then, so the
//! overrides come out in site order.

use alloc::borrow::Cow;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use mf2_model::{
    Attributes, Declaration, Expression, FunctionRef, Key, Markup, MarkupKind, Message,
    OptionValue, Options, Pattern, PatternPart,
};

use super::nfc;
use super::pool::{Class, Pool};
use crate::error::WriteError;
use crate::format::{cold, tag};

/// Appends `v` as a minimal LEB128.
pub(crate) fn varint(mut v: u32, out: &mut Vec<u8>) {
    loop {
        let low = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

/// `usize` → `u32`, or [`WriteError::TooLarge`].
pub(crate) fn count(n: usize, what: &'static str) -> Result<u32, WriteError> {
    u32::try_from(n).map_err(|_| WriteError::TooLarge(what))
}

/// What encoding one message produced besides its bytes.
pub(crate) struct Encoded {
    /// The COLD record (`varint n · overrides`), if the message needs one.
    /// Its string references are 0 when `strip_cold` (nothing is interned).
    pub(crate) cold: Option<Vec<u8>>,
}

/// The per-message encoder.
pub(crate) struct MsgEncoder<'m, 'p> {
    pool: &'p mut Pool,
    /// The manifest's function set (NFC, ascending).
    functions: &'p [String],
    /// This message's slot names (NFC, ascending).
    slots: &'p [String],
    /// The message's index, for errors.
    index: usize,
    strip_cold: bool,
    /// NFC name → index of the last `.local` binding it so far.
    scope: BTreeMap<Cow<'m, str>, u32>,
    locals: Vec<&'m str>,
    /// The next site number.
    site: u32,
    /// The last site that got an override.
    last: Option<u32>,
    overrides: u32,
    cold: Vec<u8>,
}

impl<'m, 'p> MsgEncoder<'m, 'p> {
    pub(crate) fn new(
        pool: &'p mut Pool,
        functions: &'p [String],
        slots: &'p [String],
        index: usize,
        strip_cold: bool,
    ) -> Self {
        MsgEncoder {
            pool,
            functions,
            slots,
            index,
            strip_cold,
            scope: BTreeMap::new(),
            locals: Vec::new(),
            site: 0,
            last: None,
            overrides: 0,
            cold: Vec::new(),
        }
    }

    /// Encodes `Decl* · Body` into `out` (the caller writes the head).
    pub(crate) fn message(
        mut self,
        m: &'m Message<'m>,
        out: &mut Vec<u8>,
    ) -> Result<Encoded, WriteError> {
        for d in m.declarations() {
            self.declaration(d, out)?;
        }
        match m {
            Message::Pattern(p) => self.pattern(&p.pattern, out)?,
            Message::Select(s) => {
                varint(count(s.selectors.len(), "selectors")?, out);
                for sel in &s.selectors {
                    let r = self.var(&sel.name)?;
                    varint(r, out);
                }
                varint(count(s.variants.len(), "variants")?, out);
                for v in &s.variants {
                    varint(count(v.keys.len(), "keys")?, out);
                    for k in &v.keys {
                        self.key(k, out)?;
                    }
                    let mut p = Vec::new();
                    self.pattern(&v.value, &mut p)?;
                    varint(count(p.len(), "MESSAGES")?, out);
                    out.extend_from_slice(&p);
                }
            }
            _ => return Err(self.err_unsupported()),
        }
        let cold = if self.overrides == 0 {
            None
        } else {
            let mut record = Vec::with_capacity(self.cold.len() + 5);
            varint(self.overrides, &mut record);
            record.extend_from_slice(&self.cold);
            Some(record)
        };
        Ok(Encoded { cold })
    }

    fn err_unsupported(&self) -> WriteError {
        WriteError::Unsupported {
            message: self.index,
        }
    }

    /// Takes the next site number.
    fn site(&mut self) -> Result<u32, WriteError> {
        let s = self.site;
        self.site = s.checked_add(1).ok_or(WriteError::TooLarge("sites"))?;
        Ok(s)
    }

    /// Starts an override at `site`: its gap and kind.
    fn override_head(&mut self, site: u32, kind: u8) {
        let gap = match self.last {
            None => site,
            Some(l) => site - l - 1,
        };
        self.last = Some(site);
        self.overrides += 1;
        if !self.strip_cold {
            varint(gap, &mut self.cold);
            self.cold.push(kind);
        }
    }

    /// A string reference inside COLD (not interned when stripping).
    fn cold_str(&mut self, s: &str, class: Class) -> Result<u32, WriteError> {
        if self.strip_cold {
            // Still reject what no catalog can hold.
            if s.as_bytes().contains(&0) {
                return Err(WriteError::Nul(String::from(s)));
            }
            return Ok(0);
        }
        self.pool.r(s, class)
    }

    fn spelling(&mut self, site: u32, written: &str) -> Result<(), WriteError> {
        self.override_head(site, cold::SPELLING);
        let r = self.cold_str(written, Class::Ident)?;
        if !self.strip_cold {
            varint(r, &mut self.cold);
        }
        Ok(())
    }

    fn attributes(&mut self, site: u32, attrs: &'m Attributes<'m>) -> Result<(), WriteError> {
        if attrs.is_empty() {
            return Ok(());
        }
        self.override_head(site, cold::ATTRIBUTES);
        let mut buf = Vec::new();
        varint(count(attrs.len(), "attributes")?, &mut buf);
        for (name, value) in attrs.iter() {
            varint(self.cold_str(name, Class::Ident)?, &mut buf);
            match value {
                None => buf.push(0),
                Some(l) => varint(self.cold_str(&l.value, Class::Text)? + 1, &mut buf),
            }
        }
        if !self.strip_cold {
            self.cold.extend_from_slice(&buf);
        }
        Ok(())
    }

    /// A name site: the NFC form's `StrRef`, and a spelling override when the
    /// written form differs.
    fn name(&mut self, written: &str) -> Result<u32, WriteError> {
        let site = self.site()?;
        let n = nfc(written);
        let r = self.pool.r(&n, Class::Ident)?;
        if n != written {
            self.spelling(site, written)?;
        }
        Ok(r)
    }

    /// A variable site: resolves the reference (§2.2) → `index << 1 | local`.
    fn var(&mut self, written: &'m str) -> Result<u32, WriteError> {
        let site = self.site()?;
        let n = nfc(written);
        let (r, canonical): (u64, &str) = if let Some(&i) = self.scope.get(&*n) {
            let decl = self.locals.get(i as usize).copied().unwrap_or("");
            ((u64::from(i) << 1) | 1, decl)
        } else {
            match self
                .slots
                .binary_search_by(|s| s.as_bytes().cmp(n.as_bytes()))
            {
                Ok(slot) => ((slot as u64) << 1, self.slots.get(slot).map_or("", |s| s)),
                Err(_) => {
                    return Err(WriteError::UnknownVariable {
                        message: self.index,
                        name: String::from(written),
                    });
                }
            }
        };
        let r = u32::try_from(r)
            .ok()
            .filter(|r| r >> 31 == 0)
            .ok_or(WriteError::TooLarge("variable index"))?;
        if canonical != written {
            self.spelling(site, written)?;
        }
        Ok(r)
    }

    fn declaration(&mut self, d: &'m Declaration<'m>, out: &mut Vec<u8>) -> Result<(), WriteError> {
        match d {
            Declaration::Input(x) => {
                if x.name != x.value.arg.name {
                    return Err(WriteError::InputName {
                        message: self.index,
                        name: String::from(&*x.name),
                        variable: String::from(&*x.value.arg.name),
                    });
                }
                let f = if x.value.function.is_some() {
                    tag::FUNCTION
                } else {
                    0
                };
                out.push(tag::EXPRESSION | tag::OP_VARIABLE << tag::OP_SHIFT | f);
                let site = self.site()?;
                self.attributes(site, &x.value.attributes)?;
                let r = self.var(&x.value.arg.name)?;
                varint(r, out);
                if let Some(func) = &x.value.function {
                    self.function(func, out)?;
                }
            }
            Declaration::Local(x) => {
                out.push(expression_tag(&x.value) | tag::LOCAL);
                self.expression(&x.value, out)?;
                let index = count(self.locals.len(), "locals")?;
                self.locals.push(&x.name);
                self.scope.insert(nfc(&x.name), index);
            }
            _ => return Err(self.err_unsupported()),
        }
        Ok(())
    }

    /// An expression after its tag: its site, operand and function.
    fn expression(&mut self, e: &'m Expression<'m>, out: &mut Vec<u8>) -> Result<(), WriteError> {
        let site = self.site()?;
        self.attributes(site, e.attributes())?;
        let function = match e {
            Expression::Literal(x) => {
                varint(self.pool.r(&x.arg.value, Class::Text)?, out);
                x.function.as_ref()
            }
            Expression::Variable(x) => {
                let r = self.var(&x.arg.name)?;
                varint(r, out);
                x.function.as_ref()
            }
            Expression::Function(x) => Some(&x.function),
            _ => return Err(self.err_unsupported()),
        };
        if let Some(f) = function {
            self.function(f, out)?;
        }
        Ok(())
    }

    fn function(&mut self, f: &'m FunctionRef<'m>, out: &mut Vec<u8>) -> Result<(), WriteError> {
        let site = self.site()?;
        let n = nfc(&f.name);
        let index = self
            .functions
            .binary_search_by(|x| x.as_bytes().cmp(n.as_bytes()))
            .map_err(|_| WriteError::UnknownFunction {
                message: self.index,
                name: String::from(&*f.name),
            })?;
        varint(count(index, "functions")?, out);
        if n != f.name {
            self.spelling(site, &f.name)?;
        }
        self.options(&f.options, out)
    }

    fn options(&mut self, o: &'m Options<'m>, out: &mut Vec<u8>) -> Result<(), WriteError> {
        varint(count(o.len(), "options")?, out);
        for (name, value) in o.iter() {
            let r = self.name(name)?;
            varint(r, out);
            let v = match value {
                OptionValue::Literal(l) => self.pool.r(&l.value, Class::Ident)? << 1,
                OptionValue::Variable(v) => {
                    let r = self.var(&v.name)?;
                    r.checked_mul(2)
                        .map(|x| x | 1)
                        .ok_or(WriteError::TooLarge("variable index"))?
                }
                _ => return Err(self.err_unsupported()),
            };
            varint(v, out);
        }
        Ok(())
    }

    fn markup(&mut self, m: &'m Markup<'m>, out: &mut Vec<u8>) -> Result<(), WriteError> {
        let kind = match m.kind {
            MarkupKind::Open => tag::OPEN,
            MarkupKind::Standalone => tag::STANDALONE,
            MarkupKind::Close => tag::CLOSE,
        };
        let o = if m.options.is_empty() {
            0
        } else {
            tag::OPTIONS
        };
        out.push(kind | o);
        let site = self.site()?;
        self.attributes(site, &m.attributes)?;
        let r = self.name(&m.name)?;
        varint(r, out);
        if !m.options.is_empty() {
            self.options(&m.options, out)?;
        }
        Ok(())
    }

    fn key(&mut self, k: &'m Key<'m>, out: &mut Vec<u8>) -> Result<(), WriteError> {
        match k {
            Key::Literal(l) => {
                let r = self.name(&l.value)?;
                varint(r + 1, out);
            }
            Key::CatchAll(c) => {
                let site = self.site()?;
                out.push(0);
                if let Some(v) = &c.value {
                    self.override_head(site, cold::CATCH_ALL);
                    let r = self.cold_str(v, Class::Text)?;
                    if !self.strip_cold {
                        varint(r, &mut self.cold);
                    }
                }
            }
            _ => return Err(self.err_unsupported()),
        }
        Ok(())
    }

    fn pattern(&mut self, p: &'m Pattern<'m>, out: &mut Vec<u8>) -> Result<(), WriteError> {
        varint(count(p.len(), "parts")?, out);
        for part in p.parts() {
            match part {
                PatternPart::Text(t) => {
                    out.push(tag::TEXT);
                    varint(self.pool.r(t, Class::Text)?, out);
                }
                PatternPart::Expression(e) => {
                    out.push(expression_tag(e));
                    self.expression(e, out)?;
                }
                PatternPart::Markup(m) => self.markup(m, out)?,
                _ => return Err(self.err_unsupported()),
            }
        }
        Ok(())
    }
}

/// The tag of an expression (part or `.local`).
fn expression_tag(e: &Expression<'_>) -> u8 {
    let op = match e {
        Expression::Literal(_) => tag::OP_LITERAL,
        Expression::Variable(_) => tag::OP_VARIABLE,
        _ => tag::OP_NONE,
    };
    let f = if e.function().is_some() {
        tag::FUNCTION
    } else {
        0
    };
    tag::EXPRESSION | op << tag::OP_SHIFT | f
}
