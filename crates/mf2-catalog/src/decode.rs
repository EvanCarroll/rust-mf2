//! The model-rebuilding decoder (feature `decode`, build side; conformance
//! layer L3): a message of a [`Catalog`] back to the data model.
//!
//! It walks the same views the runtime walks, so reader and decoder cannot
//! disagree on the grammar, and applies the message's COLD record
//! (`plans/02-catalog-format.md` §2.5) site by site. With COLD stripped it
//! gives the formatting-relevant model — names and keys in NFC, no
//! attributes, no catch-all values — and says so ([`Decoded::cold_dropped`]).
//! Linear in the bytes it reads; strings are borrowed from the catalog.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use mf2_model::{
    Attributes, CatchAllKey, Declaration, Expression, FunctionExpression, FunctionRef,
    InputDeclaration, Key, Literal, LiteralExpression, LocalDeclaration, Markup, Message, MsgId,
    OptionValue, Options, Pattern, PatternMessage, PatternPart, SelectMessage, VariableExpression,
    VariableRef, Variant,
};

use crate::bytes::Cur;
use crate::error::DecodeError;
use crate::format::cold;
use crate::reader::{Catalog, Entry, StrRef};
use crate::view::{
    Body, DeclView, ExprView, FunctionView, KeyView, Malformed, MsgView, Names, Operand,
    OptionsView, PartView, PatternView, VarRef,
};

/// A decoded message and what a stripped catalog could not give back.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Decoded<'a> {
    /// The model.
    pub message: Message<'a>,
    /// The message has a COLD record the catalog does not carry (stripped):
    /// its attributes, catch-all values and non-NFC spellings are lost, and
    /// `message` is the formatting-relevant model.
    pub cold_dropped: bool,
}

/// Rebuilds the data model of message `id`. With COLD present (unstripped)
/// it equals the model that was written (F1).
pub fn decode(catalog: &Catalog, id: MsgId) -> Result<Message<'_>, DecodeError> {
    decode_report(catalog, id).map(|d| d.message)
}

/// [`decode`], reporting whether COLD data was dropped.
pub fn decode_report(catalog: &Catalog, id: MsgId) -> Result<Decoded<'_>, DecodeError> {
    let view = match catalog.get(id) {
        Entry::Absent => return Err(DecodeError::Absent),
        Entry::Simple(r) => {
            let text = str_of(catalog, r)?;
            return Ok(Decoded {
                message: Message::Pattern(PatternMessage {
                    declarations: Vec::new(),
                    pattern: Pattern::from_text(Cow::Borrowed(text)),
                }),
                cold_dropped: false,
            });
        }
        Entry::Pattern(v) | Entry::Select(v) => v,
    };
    Decoder::new(catalog, view)?.message(view)
}

fn str_of(cat: &Catalog, r: StrRef) -> Result<&str, DecodeError> {
    cat.text(r).ok_or(DecodeError::String)
}

impl From<Malformed> for DecodeError {
    fn from(_: Malformed) -> Self {
        DecodeError::Malformed
    }
}

/// One COLD override.
#[derive(Clone, Copy)]
struct Override {
    site: u32,
    kind: u8,
    /// Position of the payload in COLD.
    at: usize,
}

/// The overrides of one COLD record, read one at a time.
struct ColdRecord<'a> {
    c: Cur<'a>,
    left: u32,
    next: Option<Override>,
}

impl<'a> ColdRecord<'a> {
    fn new(cold: &'a [u8], at: usize) -> Result<Self, DecodeError> {
        let mut c = Cur::new(cold, at);
        let left = c.varint().ok_or(DecodeError::Cold)?;
        if left == 0 {
            return Err(DecodeError::Cold);
        }
        let mut r = ColdRecord {
            c,
            left,
            next: None,
        };
        r.advance(None)?;
        Ok(r)
    }

    /// Reads the next override's head; skips nothing (the payload is read
    /// by whoever consumes it, which leaves the cursor after it).
    fn advance(&mut self, prev: Option<u32>) -> Result<(), DecodeError> {
        if self.left == 0 {
            self.next = None;
            return Ok(());
        }
        self.left -= 1;
        let gap = self.c.varint().ok_or(DecodeError::Cold)?;
        let site = match prev {
            None => Some(gap),
            Some(p) => p.checked_add(1).and_then(|x| x.checked_add(gap)),
        }
        .ok_or(DecodeError::Cold)?;
        let kind = self.c.u8().ok_or(DecodeError::Cold)?;
        self.next = Some(Override {
            site,
            kind,
            at: self.c.pos(),
        });
        Ok(())
    }
}

/// The kind of a site (§2.5), which fixes the override kinds it accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Site {
    /// An expression or markup: ATTRIBUTES.
    Node,
    /// A variable, function, option name, markup name or literal key:
    /// SPELLING.
    Name,
    /// A catch-all key: CATCH-ALL.
    CatchAll,
}

struct Decoder<'a> {
    cat: &'a Catalog,
    names: Names<'a>,
    record: Option<ColdRecord<'a>>,
    dropped: bool,
    site: u32,
}

impl<'a> Decoder<'a> {
    fn new(cat: &'a Catalog, view: MsgView<'a>) -> Result<Self, DecodeError> {
        let head = view.head()?;
        let names = view.try_names().ok_or(DecodeError::Names)?;
        let (record, dropped) = match (head.cold, cat.cold) {
            (None, _) => (None, false),
            (Some(_), None) => (None, true),
            (Some(at), Some(span)) => (
                Some(ColdRecord::new(span.of(cat.as_bytes()), at as usize)?),
                false,
            ),
        };
        Ok(Decoder {
            cat,
            names,
            record,
            dropped,
            site: 0,
        })
    }

    fn str(&self, r: StrRef) -> Result<&'a str, DecodeError> {
        str_of(self.cat, r)
    }

    /// Takes the next site; returns its override's payload cursor if it has
    /// one.
    fn site(&mut self, kind: Site) -> Result<Option<(u8, Cur<'a>)>, DecodeError> {
        let site = self.site;
        self.site = site.checked_add(1).ok_or(DecodeError::Malformed)?;
        let Some(rec) = &mut self.record else {
            return Ok(None);
        };
        let Some(o) = rec.next else {
            return Ok(None);
        };
        if o.site != site {
            return if o.site < site {
                Err(DecodeError::Cold)
            } else {
                Ok(None)
            };
        }
        let ok = match kind {
            Site::Node => o.kind == cold::ATTRIBUTES,
            Site::Name => o.kind == cold::SPELLING,
            Site::CatchAll => o.kind == cold::CATCH_ALL,
        };
        if !ok {
            return Err(DecodeError::Cold);
        }
        Ok(Some((o.kind, Cur::new(rec.c.bytes(), o.at))))
    }

    /// After consuming an override's payload with `c`: move to the next one.
    fn consumed(&mut self, c: Cur<'a>) -> Result<(), DecodeError> {
        if let Some(rec) = &mut self.record {
            let prev = rec.next.map(|o| o.site);
            rec.c = c;
            rec.advance(prev)?;
        }
        Ok(())
    }

    /// A name site: the stored (NFC) name, or its spelling override.
    fn name(&mut self, stored: &'a str) -> Result<Cow<'a, str>, DecodeError> {
        match self.site(Site::Name)? {
            None => Ok(Cow::Borrowed(stored)),
            Some((_, mut c)) => {
                let r = c.varint().ok_or(DecodeError::Cold)?;
                self.consumed(c)?;
                Ok(Cow::Borrowed(self.str(StrRef(r))?))
            }
        }
    }

    /// An expression or markup site: its attributes.
    fn attributes(&mut self) -> Result<Attributes<'a>, DecodeError> {
        let Some((_, mut c)) = self.site(Site::Node)? else {
            return Ok(Attributes::new());
        };
        let m = c.len().ok_or(DecodeError::Cold)?;
        let mut attrs = Attributes::with_capacity(m.min(c.remaining()));
        for _ in 0..m {
            let name = self.str(StrRef(c.varint().ok_or(DecodeError::Cold)?))?;
            let value = match c.varint().ok_or(DecodeError::Cold)? {
                0 => None,
                a => Some(Literal {
                    value: Cow::Borrowed(self.str(StrRef(a - 1))?),
                }),
            };
            attrs.push(Cow::Borrowed(name), value);
        }
        self.consumed(c)?;
        Ok(attrs)
    }

    fn var(&mut self, v: VarRef) -> Result<Cow<'a, str>, DecodeError> {
        let stored = self.names.var(v).ok_or(DecodeError::Names)?;
        let stored = self.str(stored)?;
        self.name(stored)
    }

    fn message(mut self, view: MsgView<'a>) -> Result<Decoded<'a>, DecodeError> {
        let mut decls = view.declarations();
        let mut declarations = Vec::with_capacity((decls.len() as usize).min(64));
        for d in &mut decls {
            declarations.push(self.declaration(d?)?);
        }
        let message = match decls.body()? {
            Body::Pattern(p) => Message::Pattern(PatternMessage {
                declarations,
                pattern: self.pattern(p)?,
            }),
            Body::Select(s) => {
                let mut selectors = Vec::new();
                for v in s.selectors() {
                    selectors.push(VariableRef {
                        name: self.var(v?)?,
                    });
                }
                let mut variants = Vec::new();
                for v in s.variants() {
                    let v = v?;
                    let mut keys = Vec::new();
                    for k in v.keys() {
                        keys.push(match k? {
                            KeyView::Literal(r) => {
                                let stored = self.str(r)?;
                                Key::Literal(Literal {
                                    value: self.name(stored)?,
                                })
                            }
                            KeyView::CatchAll => Key::CatchAll(self.catch_all()?),
                        });
                    }
                    variants.push(Variant {
                        keys,
                        value: self.pattern(v.pattern())?,
                    });
                }
                Message::Select(SelectMessage {
                    declarations,
                    selectors,
                    variants,
                })
            }
        };
        if self.record.as_ref().is_some_and(|r| r.next.is_some()) {
            return Err(DecodeError::Cold);
        }
        Ok(Decoded {
            message,
            cold_dropped: self.dropped,
        })
    }

    fn catch_all(&mut self) -> Result<CatchAllKey<'a>, DecodeError> {
        match self.site(Site::CatchAll)? {
            None => Ok(CatchAllKey { value: None }),
            Some((_, mut c)) => {
                let r = c.varint().ok_or(DecodeError::Cold)?;
                self.consumed(c)?;
                Ok(CatchAllKey {
                    value: Some(Cow::Borrowed(self.str(StrRef(r))?)),
                })
            }
        }
    }

    fn declaration(&mut self, d: DeclView<'a>) -> Result<Declaration<'a>, DecodeError> {
        match d {
            DeclView::Input(e) => {
                let attributes = self.attributes()?;
                let Some(Operand::Variable(v)) = e.operand() else {
                    return Err(DecodeError::Malformed);
                };
                let name = self.var(v)?;
                let function = e.function().map(|f| self.function(f)).transpose()?;
                Ok(Declaration::Input(InputDeclaration {
                    name: name.clone(),
                    value: VariableExpression {
                        arg: VariableRef { name },
                        function,
                        attributes,
                    },
                }))
            }
            DeclView::Local { index, expr } => {
                let value = self.expression(expr)?;
                let name = self.names.local(index).ok_or(DecodeError::Names)?;
                Ok(Declaration::Local(LocalDeclaration {
                    name: Cow::Borrowed(self.str(name)?),
                    value,
                }))
            }
        }
    }

    fn expression(&mut self, e: ExprView<'a>) -> Result<Expression<'a>, DecodeError> {
        let attributes = self.attributes()?;
        let operand = match e.operand() {
            None => None,
            Some(Operand::Literal(r)) => Some(Operand2::Literal(self.str(r)?)),
            Some(Operand::Variable(v)) => Some(Operand2::Variable(self.var(v)?)),
        };
        let function = e.function().map(|f| self.function(f)).transpose()?;
        Ok(match (operand, function) {
            (Some(Operand2::Literal(value)), function) => Expression::Literal(LiteralExpression {
                arg: Literal {
                    value: Cow::Borrowed(value),
                },
                function,
                attributes,
            }),
            (Some(Operand2::Variable(name)), function) => {
                Expression::Variable(VariableExpression {
                    arg: VariableRef { name },
                    function,
                    attributes,
                })
            }
            (None, Some(function)) => Expression::Function(FunctionExpression {
                function,
                attributes,
            }),
            (None, None) => return Err(DecodeError::Malformed),
        })
    }

    fn function(&mut self, f: FunctionView<'a>) -> Result<FunctionRef<'a>, DecodeError> {
        let stored = self.cat.function(f.index()).ok_or(DecodeError::Function)?;
        let name = self.name(stored)?;
        Ok(FunctionRef {
            name,
            options: self.options(f.options())?,
        })
    }

    fn options(&mut self, o: OptionsView<'a>) -> Result<Options<'a>, DecodeError> {
        let mut out = Options::with_capacity((o.len() as usize).min(64));
        for opt in o {
            let (name, value) = opt?;
            let stored = self.str(name)?;
            let name = self.name(stored)?;
            let value = match value {
                Operand::Literal(r) => OptionValue::Literal(Literal {
                    value: Cow::Borrowed(self.str(r)?),
                }),
                Operand::Variable(v) => OptionValue::Variable(VariableRef { name: self.var(v)? }),
            };
            out.push(name, value);
        }
        Ok(out)
    }

    /// A pattern. An empty TEXT part, or two in a row, is malformed: the
    /// model's patterns have none (`Pattern::push` would merge them, and the
    /// decoded model would silently differ from the bytes).
    fn pattern(&mut self, p: PatternView<'a>) -> Result<Pattern<'a>, DecodeError> {
        let mut out = Pattern::with_capacity((p.len() as usize).min(64));
        let mut after_text = false;
        for part in p.parts() {
            let part = part?;
            let text = matches!(part, PartView::Text(_));
            if text && after_text {
                return Err(DecodeError::Malformed);
            }
            after_text = text;
            out.push(match part {
                PartView::Text(r) => {
                    let t = self.str(r)?;
                    if t.is_empty() {
                        return Err(DecodeError::Malformed);
                    }
                    PatternPart::Text(Cow::Borrowed(t))
                }
                PartView::Expression(e) => PatternPart::Expression(self.expression(e)?),
                PartView::Markup(m) => {
                    let attributes = self.attributes()?;
                    let stored = self.str(m.name())?;
                    let name = self.name(stored)?;
                    PatternPart::Markup(Markup {
                        kind: m.kind(),
                        name,
                        options: self.options(m.options())?,
                        attributes,
                    })
                }
            });
        }
        Ok(out)
    }
}

/// An operand while its expression is being rebuilt.
enum Operand2<'a> {
    Literal(&'a str),
    Variable(Cow<'a, str>),
}
