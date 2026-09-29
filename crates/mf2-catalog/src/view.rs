//! Views over MESSAGES (`plans/02-catalog-format.md` §2.2): small `Copy`
//! cursors the evaluator walks without building a tree. They never panic
//! and never allocate; a malformed record yields one `Err(Malformed)` and
//! ends its iterator (the runtime then formats the fallback for that message
//! only). Each step reads only the bytes it returns (plus, for an expression
//! or markup, a walk over its options to find what follows), so a whole walk
//! is linear in the record.

// Views are small `Copy` cursors by design (the evaluator keeps positions),
// so the iterators over them are `Copy` too.
#![allow(clippy::copy_iterator)]

use mf2_model::MarkupKind;

use crate::bytes::{Cur, u32_at};
use crate::format::tag;
use crate::reader::{Catalog, StrRef};

/// A MESSAGES record is malformed (truncated, a bad tag, a count past its
/// data). Only the message that holds it is affected.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Malformed;

/// A variable reference.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VarRef {
    /// An external variable: its slot (the manifest's slot order), which is
    /// the position of the call site's argument.
    External(u32),
    /// A `.local`: its position among the message's `.local` declarations.
    Local(u32),
}

impl VarRef {
    #[inline]
    fn from_raw(r: u32) -> Self {
        if r & 1 == 0 {
            VarRef::External(r >> 1)
        } else {
            VarRef::Local(r >> 1)
        }
    }
}

/// An operand, or an option value: a literal or a variable.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Operand {
    /// A literal's value (as written, escapes processed).
    Literal(StrRef),
    /// A variable.
    Variable(VarRef),
}

/// A variant key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KeyView {
    /// `*`.
    CatchAll,
    /// A literal key, NFC-normalized.
    Literal(StrRef),
}

/// A message's variable names (NAMES, §2.6): slot names in slot order
/// (NFC), then local names in declaration order (as declared). Every access
/// is O(1).
#[derive(Clone, Copy)]
pub struct Names<'a> {
    table: &'a [u8],
    externals: u32,
    locals: u32,
}

/// The number of external and local names; the name table is left out.
impl core::fmt::Debug for Names<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Names")
            .field("externals", &self.externals)
            .field("locals", &self.locals)
            .finish_non_exhaustive()
    }
}

impl<'a> Names<'a> {
    /// No names (simple and absent messages).
    pub const EMPTY: Names<'static> = Names {
        table: &[],
        externals: 0,
        locals: 0,
    };

    /// Reads the entry at `names_ref` (0 = none); `None` if malformed.
    pub(crate) fn at(names: &'a [u8], names_ref: u32) -> Option<Names<'a>> {
        let Some(off) = names_ref.checked_sub(1) else {
            return Some(Names {
                table: &[],
                externals: 0,
                locals: 0,
            });
        };
        let mut c = Cur::new(names, off as usize);
        let externals = c.varint()?;
        let locals = c.varint()?;
        let n = (externals as usize).checked_add(locals as usize)?;
        let table = c.take(n.checked_mul(4)?)?;
        Some(Names {
            table,
            externals,
            locals,
        })
    }

    /// The number of slots.
    pub fn external_count(&self) -> u32 {
        self.externals
    }

    /// The number of `.local` declarations.
    pub fn local_count(&self) -> u32 {
        self.locals
    }

    /// The name of external slot `slot` (NFC).
    pub fn external(&self, slot: u32) -> Option<StrRef> {
        if slot >= self.externals {
            return None;
        }
        u32_at(self.table, (slot as usize).checked_mul(4)?).map(StrRef)
    }

    /// The name of local `index`, as declared.
    pub fn local(&self, index: u32) -> Option<StrRef> {
        if index >= self.locals {
            return None;
        }
        let i = (self.externals as usize).checked_add(index as usize)?;
        u32_at(self.table, i.checked_mul(4)?).map(StrRef)
    }

    /// The name of a variable reference.
    pub fn var(&self, v: VarRef) -> Option<StrRef> {
        match v {
            VarRef::External(s) => self.external(s),
            VarRef::Local(i) => self.local(i),
        }
    }
}

/// The start of a message record, decoded.
#[derive(Clone, Copy)]
pub(crate) struct Head {
    pub(crate) names_ref: u32,
    /// Read by the decoder only.
    #[cfg_attr(not(feature = "decode"), allow(dead_code))]
    pub(crate) cold: Option<u32>,
    pub(crate) decl_count: u32,
    pub(crate) decls_at: usize,
}

/// A pattern or select message in MESSAGES.
#[derive(Clone, Copy)]
pub struct MsgView<'a> {
    cat: &'a Catalog,
    at: usize,
    select: bool,
}

/// Where the message is and whether it selects; the catalog is left out.
impl core::fmt::Debug for MsgView<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MsgView")
            .field("at", &self.at)
            .field("select", &self.select)
            .finish_non_exhaustive()
    }
}

impl<'a> MsgView<'a> {
    pub(crate) fn new(cat: &'a Catalog, at: usize, select: bool) -> Self {
        MsgView { cat, at, select }
    }

    /// The MESSAGES section.
    #[inline]
    fn bytes(&self) -> &'a [u8] {
        self.cat.messages.of(self.cat.as_bytes())
    }

    /// Whether this is a `.match` message.
    pub fn is_select(&self) -> bool {
        self.select
    }

    /// The catalog this message is in.
    pub fn catalog(&self) -> &'a Catalog {
        self.cat
    }

    pub(crate) fn head(&self) -> Result<Head, Malformed> {
        let mut c = Cur::new(self.bytes(), self.at);
        let names_ref = c.varint().ok_or(Malformed)?;
        let decls = c.varint().ok_or(Malformed)?;
        let cold = if decls & 1 == 1 {
            Some(c.varint().ok_or(Malformed)?)
        } else {
            None
        };
        Ok(Head {
            names_ref,
            cold,
            decl_count: decls >> 1,
            decls_at: c.pos(),
        })
    }

    /// This message's NAMES entry; `None` if it is malformed.
    pub(crate) fn try_names(&self) -> Option<Names<'a>> {
        let head = self.head().ok()?;
        Names::at(self.cat.names.of(self.cat.as_bytes()), head.names_ref)
    }

    /// This message's variable names (empty if its entry is malformed).
    pub fn names(&self) -> Names<'a> {
        self.try_names().unwrap_or(Names::EMPTY)
    }

    /// The declarations, in order.
    pub fn declarations(&self) -> Declarations<'a> {
        match self.head() {
            Ok(h) => Declarations {
                c: Cur::new(self.bytes(), h.decls_at),
                left: h.decl_count,
                locals: 0,
                failed: false,
                select: self.select,
            },
            Err(Malformed) => Declarations {
                c: Cur::new(&[], 0),
                left: 0,
                locals: 0,
                failed: true,
                select: self.select,
            },
        }
    }

    /// The body: walks the declarations to find it.
    pub fn body(&self) -> Result<Body<'a>, Malformed> {
        self.declarations().body()
    }
}

/// A message body.
#[derive(Clone, Copy, Debug)]
pub enum Body<'a> {
    /// A pattern message's pattern.
    Pattern(PatternView<'a>),
    /// A select message's selectors and variants.
    Select(SelectView<'a>),
}

/// A declaration.
#[derive(Clone, Copy, Debug)]
pub enum DeclView<'a> {
    /// `.input {$x …}`: the expression's operand is the variable.
    Input(ExprView<'a>),
    /// `.local $x = {…}`: `index` is its position among the `.local`s (its
    /// name is `Names::local(index)`).
    Local {
        /// Position among the message's `.local` declarations.
        index: u32,
        /// The bound expression.
        expr: ExprView<'a>,
    },
}

/// The declarations of a message; after them, [`Declarations::body`].
#[derive(Clone, Copy)]
pub struct Declarations<'a> {
    c: Cur<'a>,
    left: u32,
    locals: u32,
    failed: bool,
    select: bool,
}

/// How many declarations are left; the cursor is left out.
impl core::fmt::Debug for Declarations<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Declarations")
            .field("left", &self.left)
            .field("locals", &self.locals)
            .field("select", &self.select)
            .finish_non_exhaustive()
    }
}

impl<'a> Declarations<'a> {
    /// How many declarations remain.
    pub fn len(&self) -> u32 {
        self.left
    }

    /// Whether none remain.
    pub fn is_empty(&self) -> bool {
        self.left == 0
    }

    fn step(&mut self) -> Result<DeclView<'a>, Malformed> {
        let t = self.c.u8().ok_or(Malformed)?;
        let local = t & tag::LOCAL != 0;
        if t & tag::KIND_MASK != tag::EXPRESSION || t & !(tag::EXPRESSION_BITS | tag::LOCAL) != 0 {
            return Err(Malformed);
        }
        let expr = expr(&mut self.c, t)?;
        if local {
            let index = self.locals;
            self.locals = self.locals.checked_add(1).ok_or(Malformed)?;
            Ok(DeclView::Local { index, expr })
        } else if matches!(expr.operand, Some(Operand::Variable(_))) {
            Ok(DeclView::Input(expr))
        } else {
            Err(Malformed)
        }
    }

    /// Skips the remaining declarations and reads the body.
    pub fn body(mut self) -> Result<Body<'a>, Malformed> {
        while self.left > 0 {
            self.next().ok_or(Malformed)??;
        }
        if self.failed {
            return Err(Malformed);
        }
        let mut c = self.c;
        if !self.select {
            return pattern(&mut c).map(Body::Pattern);
        }
        let nsel = c.varint().ok_or(Malformed)?;
        let sel_at = c.pos();
        for _ in 0..nsel {
            c.varint().ok_or(Malformed)?;
        }
        let nvar = c.varint().ok_or(Malformed)?;
        Ok(Body::Select(SelectView {
            b: c.bytes(),
            sel_at,
            nsel,
            var_at: c.pos(),
            nvar,
        }))
    }
}

impl<'a> Iterator for Declarations<'a> {
    type Item = Result<DeclView<'a>, Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let r = self.step();
        if r.is_err() {
            self.failed = true;
            self.left = 0;
        }
        Some(r)
    }
}

/// Reads an expression after its tag `t`.
fn expr<'a>(c: &mut Cur<'a>, t: u8) -> Result<ExprView<'a>, Malformed> {
    let has_fn = t & tag::FUNCTION != 0;
    let operand = match (t & tag::OP_MASK) >> tag::OP_SHIFT {
        tag::OP_NONE if has_fn => None,
        tag::OP_LITERAL => Some(Operand::Literal(StrRef(c.varint().ok_or(Malformed)?))),
        tag::OP_VARIABLE => Some(Operand::Variable(VarRef::from_raw(
            c.varint().ok_or(Malformed)?,
        ))),
        _ => return Err(Malformed),
    };
    let function = if has_fn {
        let index = c.varint().ok_or(Malformed)?;
        Some(FunctionView {
            index,
            options: options(c)?,
        })
    } else {
        None
    };
    Ok(ExprView { operand, function })
}

/// Reads `Options` and moves past them.
fn options<'a>(c: &mut Cur<'a>) -> Result<OptionsView<'a>, Malformed> {
    let n = c.varint().ok_or(Malformed)?;
    let at = c.pos();
    for _ in 0..n {
        c.varint().ok_or(Malformed)?;
        c.varint().ok_or(Malformed)?;
    }
    Ok(OptionsView {
        c: Cur::new(c.bytes(), at),
        left: n,
    })
}

/// Reads a `Pattern` header and moves past the whole pattern.
fn pattern<'a>(c: &mut Cur<'a>) -> Result<PatternView<'a>, Malformed> {
    let n = c.varint().ok_or(Malformed)?;
    Ok(PatternView {
        b: c.bytes(),
        at: c.pos(),
        n,
    })
}

/// An expression: an operand, a function, or both.
#[derive(Clone, Copy, Debug)]
pub struct ExprView<'a> {
    operand: Option<Operand>,
    function: Option<FunctionView<'a>>,
}

impl<'a> ExprView<'a> {
    /// The operand; `None` for a function-only expression.
    pub fn operand(&self) -> Option<Operand> {
        self.operand
    }

    /// The function; `None` for a bare literal or variable.
    pub fn function(&self) -> Option<FunctionView<'a>> {
        self.function
    }
}

/// A function reference: an index into FUNCS ([`Catalog::function`]) and
/// its options.
#[derive(Clone, Copy, Debug)]
pub struct FunctionView<'a> {
    index: u32,
    options: OptionsView<'a>,
}

impl<'a> FunctionView<'a> {
    /// The FUNCS index.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// The options, in source order.
    pub fn options(&self) -> OptionsView<'a> {
        self.options
    }
}

/// Options in source order: `(name, value)`, names in NFC.
#[derive(Clone, Copy)]
pub struct OptionsView<'a> {
    c: Cur<'a>,
    left: u32,
}

/// How many options are left; the cursor is left out.
impl core::fmt::Debug for OptionsView<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OptionsView")
            .field("left", &self.left)
            .finish_non_exhaustive()
    }
}

impl OptionsView<'_> {
    /// No options.
    const NONE: OptionsView<'static> = OptionsView {
        c: Cur::new(&[], 0),
        left: 0,
    };

    /// How many options remain.
    pub fn len(&self) -> u32 {
        self.left
    }

    /// Whether none remain.
    pub fn is_empty(&self) -> bool {
        self.left == 0
    }
}

impl Iterator for OptionsView<'_> {
    type Item = Result<(StrRef, Operand), Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let r = (|| {
            let name = StrRef(self.c.varint()?);
            let v = self.c.varint()?;
            let value = if v & 1 == 0 {
                Operand::Literal(StrRef(v >> 1))
            } else {
                Operand::Variable(VarRef::from_raw(v >> 1))
            };
            Some((name, value))
        })();
        if r.is_none() {
            self.left = 0;
        }
        Some(r.ok_or(Malformed))
    }
}

/// Markup: open, standalone or close, with its name (NFC) and options.
#[derive(Clone, Copy, Debug)]
pub struct MarkupView<'a> {
    kind: MarkupKind,
    name: StrRef,
    options: OptionsView<'a>,
}

impl<'a> MarkupView<'a> {
    /// Open, standalone or close.
    pub fn kind(&self) -> MarkupKind {
        self.kind
    }

    /// The markup name (NFC).
    pub fn name(&self) -> StrRef {
        self.name
    }

    /// The options, in source order.
    pub fn options(&self) -> OptionsView<'a> {
        self.options
    }
}

/// A pattern part.
#[derive(Clone, Copy, Debug)]
pub enum PartView<'a> {
    /// Literal text.
    Text(StrRef),
    /// A placeholder with an expression.
    Expression(ExprView<'a>),
    /// A placeholder with markup.
    Markup(MarkupView<'a>),
}

/// A pattern: iterate [`PatternView::parts`].
#[derive(Clone, Copy)]
pub struct PatternView<'a> {
    b: &'a [u8],
    at: usize,
    n: u32,
}

/// Where the pattern is and how many parts it has; the bytes are left out.
impl core::fmt::Debug for PatternView<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PatternView")
            .field("at", &self.at)
            .field("parts", &self.n)
            .finish_non_exhaustive()
    }
}

impl<'a> PatternView<'a> {
    /// The number of parts.
    pub fn len(&self) -> u32 {
        self.n
    }

    /// Whether the pattern is empty.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// The parts, in order.
    pub fn parts(&self) -> Parts<'a> {
        Parts {
            c: Cur::new(self.b, self.at),
            left: self.n,
        }
    }
}

/// The parts of a pattern.
#[derive(Clone, Copy)]
pub struct Parts<'a> {
    c: Cur<'a>,
    left: u32,
}

/// How many parts are left; the cursor is left out.
impl core::fmt::Debug for Parts<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Parts")
            .field("left", &self.left)
            .finish_non_exhaustive()
    }
}

impl<'a> Parts<'a> {
    fn step(&mut self) -> Result<PartView<'a>, Malformed> {
        let t = self.c.u8().ok_or(Malformed)?;
        match t & tag::KIND_MASK {
            tag::TEXT if t == tag::TEXT => {
                Ok(PartView::Text(StrRef(self.c.varint().ok_or(Malformed)?)))
            }
            tag::EXPRESSION if t & !tag::EXPRESSION_BITS == 0 => {
                expr(&mut self.c, t).map(PartView::Expression)
            }
            k @ (tag::OPEN | tag::STANDALONE | tag::CLOSE) if t & !tag::MARKUP_BITS == 0 => {
                let kind = match k {
                    tag::OPEN => MarkupKind::Open,
                    tag::STANDALONE => MarkupKind::Standalone,
                    _ => MarkupKind::Close,
                };
                let name = StrRef(self.c.varint().ok_or(Malformed)?);
                let options = if t & tag::OPTIONS != 0 {
                    options(&mut self.c)?
                } else {
                    OptionsView::NONE
                };
                Ok(PartView::Markup(MarkupView {
                    kind,
                    name,
                    options,
                }))
            }
            _ => Err(Malformed),
        }
    }
}

impl<'a> Iterator for Parts<'a> {
    type Item = Result<PartView<'a>, Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let r = self.step();
        if r.is_err() {
            self.left = 0;
        }
        Some(r)
    }
}

/// A select body: its selectors and its variants.
#[derive(Clone, Copy)]
pub struct SelectView<'a> {
    b: &'a [u8],
    sel_at: usize,
    nsel: u32,
    var_at: usize,
    nvar: u32,
}

/// How many selectors and variants it has; the bytes are left out.
impl core::fmt::Debug for SelectView<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SelectView")
            .field("selectors", &self.nsel)
            .field("variants", &self.nvar)
            .finish_non_exhaustive()
    }
}

impl<'a> SelectView<'a> {
    /// The selectors, in order.
    pub fn selectors(&self) -> Selectors<'a> {
        Selectors {
            c: Cur::new(self.b, self.sel_at),
            left: self.nsel,
        }
    }

    /// The variants, in source order.
    pub fn variants(&self) -> Variants<'a> {
        Variants {
            c: Cur::new(self.b, self.var_at),
            left: self.nvar,
        }
    }
}

/// The selectors of a select message.
#[derive(Clone, Copy)]
pub struct Selectors<'a> {
    c: Cur<'a>,
    left: u32,
}

/// How many selectors are left; the cursor is left out.
impl core::fmt::Debug for Selectors<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Selectors")
            .field("left", &self.left)
            .finish_non_exhaustive()
    }
}

impl Selectors<'_> {
    /// How many selectors remain.
    pub fn len(&self) -> u32 {
        self.left
    }

    /// Whether none remain.
    pub fn is_empty(&self) -> bool {
        self.left == 0
    }
}

impl Iterator for Selectors<'_> {
    type Item = Result<VarRef, Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let r = self.c.varint().map(VarRef::from_raw);
        if r.is_none() {
            self.left = 0;
        }
        Some(r.ok_or(Malformed))
    }
}

/// A variant: its keys and its pattern.
#[derive(Clone, Copy, Debug)]
pub struct VariantView<'a> {
    keys: Keys<'a>,
    pattern: PatternView<'a>,
}

impl<'a> VariantView<'a> {
    /// The keys, in order.
    pub fn keys(&self) -> Keys<'a> {
        self.keys
    }

    /// The pattern.
    pub fn pattern(&self) -> PatternView<'a> {
        self.pattern
    }
}

/// The variants of a select message. Skipping a variant's pattern is O(1).
#[derive(Clone, Copy)]
pub struct Variants<'a> {
    c: Cur<'a>,
    left: u32,
}

/// How many variants are left; the cursor is left out.
impl core::fmt::Debug for Variants<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Variants")
            .field("left", &self.left)
            .finish_non_exhaustive()
    }
}

impl<'a> Variants<'a> {
    /// How many variants remain.
    pub fn len(&self) -> u32 {
        self.left
    }

    /// Whether none remain.
    pub fn is_empty(&self) -> bool {
        self.left == 0
    }

    fn step(&mut self) -> Option<VariantView<'a>> {
        let nkeys = self.c.varint()?;
        let keys_at = self.c.pos();
        for _ in 0..nkeys {
            self.c.varint()?;
        }
        let plen = self.c.len()?;
        let mut p = Cur::new(self.c.take(plen)?, 0);
        let pattern = pattern(&mut p).ok()?;
        Some(VariantView {
            keys: Keys {
                c: Cur::new(self.c.bytes(), keys_at),
                left: nkeys,
            },
            pattern,
        })
    }
}

impl<'a> Iterator for Variants<'a> {
    type Item = Result<VariantView<'a>, Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let r = self.step();
        if r.is_none() {
            self.left = 0;
        }
        Some(r.ok_or(Malformed))
    }
}

/// The keys of a variant.
#[derive(Clone, Copy)]
pub struct Keys<'a> {
    c: Cur<'a>,
    left: u32,
}

/// How many keys are left; the cursor is left out.
impl core::fmt::Debug for Keys<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Keys")
            .field("left", &self.left)
            .finish_non_exhaustive()
    }
}

impl Keys<'_> {
    /// How many keys remain.
    pub fn len(&self) -> u32 {
        self.left
    }

    /// Whether none remain.
    pub fn is_empty(&self) -> bool {
        self.left == 0
    }
}

impl Iterator for Keys<'_> {
    type Item = Result<KeyView, Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let r = self.c.varint().map(|k| match k.checked_sub(1) {
            None => KeyView::CatchAll,
            Some(r) => KeyView::Literal(StrRef(r)),
        });
        if r.is_none() {
            self.left = 0;
        }
        Some(r.ok_or(Malformed))
    }
}
