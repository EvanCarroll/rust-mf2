//! The evaluator: walks a MESSAGES entry in place (no intermediate tree, no
//! allocation), resolving declarations lazily, selecting with the spec's
//! algorithm, and emitting text / parts with the Default Bidi Strategy.

use mf2b_format::{decl, part};
use plural_eval::{Category, Operands, select as plural_select};

use crate::catalog::{Catalog, Cur, Dir, Entry, FnId, MsgId};
use crate::error::FormatError;
use crate::num;

/// A positional argument (slot order from the manifest).
#[derive(Clone, Copy)]
pub enum Arg<'a> {
    Str(&'a str),
    Int(i64),
    Unset,
}

/// Minimal text sink (not `core::fmt::Write`).
pub trait Sink {
    fn push_str(&mut self, s: &str);
}

impl Sink for alloc::string::String {
    #[inline]
    fn push_str(&mut self, s: &str) {
        alloc::string::String::push_str(self, s);
    }
}

pub trait ErrorSink {
    fn error(&mut self, e: FormatError);
}

/// Discards errors (the release client's policy, plans/03 §8).
pub struct NoErrors;

impl ErrorSink for NoErrors {
    #[inline]
    fn error(&mut self, _: FormatError) {}
}

impl ErrorSink for alloc::vec::Vec<FormatError> {
    fn error(&mut self, e: FormatError) {
        self.push(e);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BidiStrategy {
    Default,
    None,
}

/// Directionality of a resolved value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueDir {
    Ltr,
    Rtl,
    Unknown,
}

/// Bidi isolation controls.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Isolate {
    Lri,
    Rli,
    Fsi,
    Pdi,
}

impl Isolate {
    pub const fn as_str(self) -> &'static str {
        match self {
            Isolate::Lri => "\u{2066}",
            Isolate::Rli => "\u{2067}",
            Isolate::Fsi => "\u{2068}",
            Isolate::Pdi => "\u{2069}",
        }
    }
}

/// Source of a placeholder, for fallback output (`$name`, `|literal|`, `:fn`).
#[derive(Clone, Copy)]
pub enum Source<'a> {
    Variable(&'a str),
    Literal(&'a str),
    Function(&'a str),
    Unknown,
}

impl Source<'_> {
    /// Writes the fallback representation *without* the braces.
    pub fn write(&self, out: &mut dyn Sink) {
        match *self {
            Source::Variable(n) => {
                out.push_str("$");
                out.push_str(n);
            }
            Source::Literal(v) => {
                out.push_str("|");
                let mut rest = v;
                while let Some(i) = rest.find(['\\', '|']) {
                    out.push_str(rest.get(..i).unwrap_or(""));
                    out.push_str("\\");
                    let (c, r) = rest.get(i..).unwrap_or("").split_at_checked(1).unwrap_or(("", ""));
                    out.push_str(c);
                    rest = r;
                }
                out.push_str(rest);
                out.push_str("|");
            }
            Source::Function(n) => {
                out.push_str(":");
                out.push_str(n);
            }
            Source::Unknown => out.push_str("\u{FFFD}"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MarkupKind {
    Open,
    Standalone,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    String,
    Number,
}

/// An option value on markup.
#[derive(Clone, Copy)]
pub enum OptValue<'a> {
    Literal(&'a str),
    Arg(Arg<'a>),
}

/// Lazily decoded markup options.
#[derive(Clone, Copy)]
pub struct MarkupOptions<'a> {
    cat: &'a Catalog,
    args: &'a [Arg<'a>],
    at: usize,
    n: u32,
}

impl<'a> Iterator for MarkupOptions<'a> {
    type Item = (&'a str, OptValue<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        if self.n == 0 {
            return None;
        }
        self.n = self.n.saturating_sub(1);
        let mut c = Cur::new(self.cat.section(self.cat.messages), self.at);
        let name = self.cat.str_at(c.usize()?)?;
        let tag = c.var()?;
        let v = if tag == 0 {
            OptValue::Literal(self.cat.str_at(c.usize()?)?)
        } else {
            let vr = tag >> 1;
            let slot = (vr >> 1) as usize;
            OptValue::Arg(if vr & 1 == 0 { self.args.get(slot).copied().unwrap_or(Arg::Unset) } else { Arg::Unset })
        };
        self.at = c.i;
        Some((name, v))
    }
}

/// One formatted part (the shape `expParts` asserts).
pub enum Part<'a> {
    Text(&'a str),
    BidiIsolation(Isolate),
    Expression { kind: ValueKind, value: &'a str, dir: ValueDir },
    Fallback(Source<'a>),
    Markup { kind: MarkupKind, name: &'a str, options: MarkupOptions<'a> },
}

pub trait PartSink {
    fn part(&mut self, p: Part<'_>);
}

// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum NumMode {
    /// Not an annotated number: formats, cannot select.
    Raw,
    /// `:string` over a number.
    Str,
    Plural,
    Ordinal,
    Exact,
}

#[derive(Clone, Copy)]
enum Val<'a> {
    Str { s: &'a str, selectable: bool },
    Num { n: i64, mode: NumMode },
    Fallback(Source<'a>),
}

/// Output adapter shared by string and parts formatting (dyn, one copy of the walker).
trait Out {
    fn text(&mut self, s: &str);
    fn value(&mut self, v: &Val<'_>, msgdir: Dir, bidi: BidiStrategy);
    fn markup(&mut self, kind: MarkupKind, name: &str, options: MarkupOptions<'_>);
}

fn isolation(dir: ValueDir, msgdir: Dir, bidi: BidiStrategy) -> Option<Isolate> {
    if bidi == BidiStrategy::None {
        return None;
    }
    match dir {
        ValueDir::Ltr if msgdir == Dir::Ltr => None,
        ValueDir::Ltr => Some(Isolate::Lri),
        ValueDir::Rtl => Some(Isolate::Rli),
        ValueDir::Unknown => Some(Isolate::Fsi),
    }
}

fn val_dir(v: &Val<'_>) -> ValueDir {
    match v {
        Val::Num { mode, .. } if *mode != NumMode::Raw && *mode != NumMode::Str => ValueDir::Ltr,
        _ => ValueDir::Unknown,
    }
}

struct StrOut<'s> {
    out: &'s mut dyn Sink,
}

impl Out for StrOut<'_> {
    #[inline]
    fn text(&mut self, s: &str) {
        self.out.push_str(s);
    }
    fn value(&mut self, v: &Val<'_>, msgdir: Dir, bidi: BidiStrategy) {
        let iso = isolation(val_dir(v), msgdir, bidi);
        if let Some(i) = iso {
            self.out.push_str(i.as_str());
        }
        match *v {
            Val::Str { s, .. } => self.out.push_str(s),
            Val::Num { n, .. } => {
                let mut buf = [0u8; 20];
                self.out.push_str(num::fmt_i64(n, &mut buf));
            }
            Val::Fallback(src) => {
                self.out.push_str("{");
                src.write(self.out);
                self.out.push_str("}");
            }
        }
        if iso.is_some() {
            self.out.push_str(Isolate::Pdi.as_str());
        }
    }
    fn markup(&mut self, _: MarkupKind, _: &str, _: MarkupOptions<'_>) {}
}

struct PartsOut<'s> {
    out: &'s mut dyn PartSink,
}

impl Out for PartsOut<'_> {
    #[inline]
    fn text(&mut self, s: &str) {
        self.out.part(Part::Text(s));
    }
    fn value(&mut self, v: &Val<'_>, msgdir: Dir, bidi: BidiStrategy) {
        let dir = val_dir(v);
        let iso = isolation(dir, msgdir, bidi);
        if let Some(i) = iso {
            self.out.part(Part::BidiIsolation(i));
        }
        let mut buf = [0u8; 20];
        match *v {
            Val::Str { s, .. } => self.out.part(Part::Expression { kind: ValueKind::String, value: s, dir }),
            Val::Num { n, mode } => {
                let kind = if mode == NumMode::Str { ValueKind::String } else { ValueKind::Number };
                self.out.part(Part::Expression { kind, value: num::fmt_i64(n, &mut buf), dir });
            }
            Val::Fallback(src) => self.out.part(Part::Fallback(src)),
        }
        if iso.is_some() {
            self.out.part(Part::BidiIsolation(Isolate::Pdi));
        }
    }
    fn markup(&mut self, kind: MarkupKind, name: &str, options: MarkupOptions<'_>) {
        self.out.part(Part::Markup { kind, name, options });
    }
}

/// Formatting entry points.
pub struct Formatter<'c> {
    cat: &'c Catalog,
    bidi: BidiStrategy,
}

/// Per-call evaluation state over one MESSAGES entry.
struct Eval<'c> {
    cat: &'c Catalog,
    msg: &'c [u8],
    args: &'c [Arg<'c>],
    names_ref: usize,
    decls_at: usize,
    ndecl: u32,
}

const MAX_SELECTORS: usize = 8;

impl<'c> Formatter<'c> {
    pub fn new(cat: &'c Catalog, bidi: BidiStrategy) -> Self {
        Formatter { cat, bidi }
    }

    /// Fast path: the text of a `simple` message — no evaluator, no allocation.
    #[inline]
    pub fn simple(&self, id: MsgId) -> Option<&'c str> {
        match self.cat.get(id) {
            Entry::Simple(s) => Some(s),
            _ => None,
        }
    }

    pub fn write(&self, id: MsgId, args: &[Arg<'_>], out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        if let Entry::Simple(s) = self.cat.get(id) {
            out.push_str(s);
            return;
        }
        self.run(id, args, &mut StrOut { out }, errs);
    }

    pub fn parts(&self, id: MsgId, args: &[Arg<'_>], out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        self.run(id, args, &mut PartsOut { out }, errs);
    }

    fn run(&self, id: MsgId, args: &[Arg<'_>], out: &mut dyn Out, errs: &mut dyn ErrorSink) {
        let (at, select) = match self.cat.get(id) {
            Entry::Simple(s) => {
                out.text(s);
                return;
            }
            Entry::Pattern(at) => (at, false),
            Entry::Select(at) => (at, true),
            Entry::Absent => {
                errs.error(FormatError::MissingMessage);
                return;
            }
        };
        let msg = self.cat.section(self.cat.messages);
        let mut c = Cur::new(msg, at);
        let Some(ev) = Eval::open(self.cat, msg, args, &mut c) else {
            corrupt(out, self.cat.dir(), self.bidi, errs);
            return;
        };
        let pattern_at = if select { ev.select(&mut c, errs) } else { Some(c.i) };
        let ok = pattern_at.and_then(|p| ev.pattern(&mut Cur::new(msg, p), out, self.cat.dir(), self.bidi, errs));
        if ok.is_none() {
            corrupt(out, self.cat.dir(), self.bidi, errs);
        }
    }
}

fn corrupt(out: &mut dyn Out, msgdir: Dir, bidi: BidiStrategy, errs: &mut dyn ErrorSink) {
    errs.error(FormatError::Corrupt);
    out.value(&Val::Fallback(Source::Unknown), msgdir, bidi);
}

impl<'c> Eval<'c> {
    /// Reads the entry head and skips the declarations.
    fn open(cat: &'c Catalog, msg: &'c [u8], args: &'c [Arg<'c>], c: &mut Cur<'c>) -> Option<Self> {
        let names_ref = c.usize()?;
        let ndecl = c.var()?;
        let decls_at = c.i;
        let ev = Eval { cat, msg, args, names_ref, decls_at, ndecl };
        for _ in 0..ndecl {
            let h = c.u8()?;
            if h & decl::LOCAL == 0 {
                c.var()?; // slot
            }
            ev.skip_expr(c, h & !decl::LOCAL)?;
        }
        Some(ev)
    }

    fn skip_value(c: &mut Cur<'_>) -> Option<()> {
        if c.var()? == 0 {
            c.var()?;
        }
        Some(())
    }

    fn skip_options(c: &mut Cur<'_>) -> Option<()> {
        for _ in 0..c.var()? {
            c.var()?;
            Self::skip_value(c)?;
        }
        Some(())
    }

    fn skip_expr(&self, c: &mut Cur<'_>, h: u8) -> Option<()> {
        if (h & part::OPERAND_MASK) >> part::OPERAND_SHIFT != part::OPERAND_NONE {
            c.var()?;
        }
        if h & part::HAS_FUNCTION != 0 {
            c.var()?;
            Self::skip_options(c)?;
        }
        if h & part::HAS_COLD != 0 {
            c.var()?;
        }
        Some(())
    }

    fn name(&self, local: bool, idx: usize) -> Source<'c> {
        self.cat.name(self.names_ref, local, idx).map_or(Source::Unknown, Source::Variable)
    }

    /// Resolves a VarRef, seeing only the first `visible` declarations.
    fn var(&self, vr: u32, visible: u32, errs: &mut dyn ErrorSink) -> Val<'c>
    {
        let local = vr & 1 == 1;
        let idx = (vr >> 1) as usize;
        let mut c = Cur::new(self.msg, self.decls_at);
        let mut locals_seen = 0usize;
        for k in 0..visible.min(self.ndecl) {
            let Some(h) = c.u8() else { break };
            if h & decl::LOCAL != 0 {
                if local && locals_seen == idx {
                    return self.expr(&mut c, h & !decl::LOCAL, None, k, errs).unwrap_or(Val::Fallback(self.name(true, idx)));
                }
                locals_seen = locals_seen.saturating_add(1);
                if self.skip_expr(&mut c, h).is_none() {
                    break;
                }
            } else {
                let Some(slot) = c.usize() else { break };
                if !local && slot == idx {
                    let operand = self.arg(idx, errs);
                    return self.expr(&mut c, h, Some((operand, self.name(false, idx))), k, errs).unwrap_or(Val::Fallback(self.name(false, idx)));
                }
                if self.skip_expr(&mut c, h).is_none() {
                    break;
                }
            }
        }
        if local {
            errs.error(FormatError::UnresolvedVariable);
            return Val::Fallback(self.name(true, idx));
        }
        self.arg(idx, errs)
    }

    /// The raw argument in slot `idx`.
    fn arg(&self, idx: usize, errs: &mut dyn ErrorSink) -> Val<'c>
    {
        match self.args.get(idx).copied().unwrap_or(Arg::Unset) {
            Arg::Str(s) => Val::Str { s, selectable: false },
            Arg::Int(n) => Val::Num { n, mode: NumMode::Raw },
            Arg::Unset => {
                errs.error(FormatError::UnresolvedVariable);
                Val::Fallback(self.name(false, idx))
            }
        }
    }

    /// Resolves an expression at `c` (after its header byte `h`). `given` is
    /// the operand of an `.input` (implied, not encoded).
    fn expr(&self, c: &mut Cur<'c>, h: u8, given: Option<(Val<'c>, Source<'c>)>, visible: u32, errs: &mut dyn ErrorSink) -> Option<Val<'c>> {
        let (operand, src) = match (h & part::OPERAND_MASK) >> part::OPERAND_SHIFT {
            part::OPERAND_LITERAL => {
                let s = self.cat.str_at(c.usize()?)?;
                (Some(Val::Str { s, selectable: false }), Source::Literal(s))
            }
            part::OPERAND_VARIABLE => {
                let vr = c.var()?;
                let v = self.var(vr, visible, errs);
                (Some(v), self.name(vr & 1 == 1, (vr >> 1) as usize))
            }
            _ => match given {
                Some((v, src)) => (Some(v), src),
                None => (None, Source::Unknown),
            },
        };
        let mut result = operand;
        if h & part::HAS_FUNCTION != 0 {
            let (f, fname) = self.cat.func(c.usize()?);
            let mut mode = NumMode::Plural;
            for _ in 0..c.var()? {
                let name = self.cat.str_at(c.usize()?)?;
                let tag = c.var()?;
                if tag == 0 {
                    let v = self.cat.str_at(c.usize()?)?;
                    if name == "select" && matches!(f, FnId::Integer | FnId::Number) {
                        mode = match v {
                            "plural" => NumMode::Plural,
                            "ordinal" => NumMode::Ordinal,
                            "exact" => NumMode::Exact,
                            _ => {
                                errs.error(FormatError::BadOption);
                                NumMode::Plural
                            }
                        };
                    }
                }
            }
            let fsrc = match src {
                Source::Unknown => Source::Function(fname),
                s => s,
            };
            result = Some(match (f, operand) {
                (_, Some(Val::Fallback(s))) => Val::Fallback(s),
                (FnId::String, Some(Val::Str { s, .. })) => Val::Str { s, selectable: true },
                (FnId::String, Some(Val::Num { n, .. })) => Val::Num { n, mode: NumMode::Str },
                (FnId::Integer | FnId::Number, Some(Val::Num { n, .. })) => Val::Num { n, mode },
                (FnId::Integer | FnId::Number, Some(Val::Str { s, .. })) => match num::parse_int(s, f == FnId::Integer) {
                    Some(n) => Val::Num { n, mode },
                    None => {
                        errs.error(FormatError::BadOperand);
                        Val::Fallback(src)
                    }
                },
                (FnId::Unknown, _) => {
                    errs.error(FormatError::UnknownFunction);
                    Val::Fallback(fsrc)
                }
                (_, None) => {
                    errs.error(FormatError::BadOperand);
                    Val::Fallback(fsrc)
                }
            });
        }
        if h & part::HAS_COLD != 0 {
            c.var()?;
        }
        result
    }

    /// Pattern selection (spec: filter by SelectorsMatch, keep the best by
    /// SelectorsCompare, source order breaks ties). Returns the chosen
    /// pattern's offset.
    fn select(&self, c: &mut Cur<'c>, errs: &mut dyn ErrorSink) -> Option<usize>
    {
        let nsel = c.usize()?;
        if nsel > MAX_SELECTORS {
            errs.error(FormatError::UnsupportedOperation);
            return None;
        }
        let mut sels: [Option<Val<'c>>; MAX_SELECTORS] = [None; MAX_SELECTORS];
        for slot in sels.iter_mut().take(nsel) {
            let v = self.var(c.var()?, self.ndecl, errs);
            let selectable = match v {
                Val::Str { selectable, .. } => selectable,
                Val::Num { mode, .. } => mode != NumMode::Raw,
                Val::Fallback(_) => false,
            };
            if !selectable {
                errs.error(FormatError::BadSelector);
            }
            *slot = if selectable { Some(v) } else { None };
        }
        let nvar = c.var()?;
        let mut best: Option<(usize, usize)> = None; // (keys offset, pattern offset)
        for _ in 0..nvar {
            let keys_at = c.i;
            let mut all = true;
            for sel in sels.iter().take(nsel) {
                match self.key(c)? {
                    None => {}
                    Some(k) => all &= sel.as_ref().is_some_and(|v| self.matches(v, k)),
                }
            }
            let len = c.usize()?;
            let pattern_at = c.i;
            c.skip(len)?;
            if all {
                best = match best {
                    Some((bk, bp)) if !self.compare(&sels, nsel, keys_at, bk)? => Some((bk, bp)),
                    _ => Some((keys_at, pattern_at)),
                };
            }
        }
        best.map(|(_, p)| p)
    }

    /// Reads one key: `None` = catch-all.
    fn key(&self, c: &mut Cur<'c>) -> Option<Option<&'c str>> {
        match c.u8()? {
            0 => Some(None),
            1 => Some(Some(self.cat.str_at(c.usize()?)?)),
            _ => {
                let k = self.cat.str_at(c.usize()?)?;
                c.var()?; // COLD: original spelling
                Some(Some(k))
            }
        }
    }

    fn category(&self, n: i64, ordinal: bool) -> Category {
        let o = Operands { i: n.unsigned_abs(), f: 0, t: 0, v: 0, w: 0, e: 0 };
        plural_select(self.cat.plural(ordinal), &o)
    }

    /// Match(rv, key). Keys are stored NFC; string values are compared
    /// bytewise here (the product NFC-normalizes through `Host`).
    fn matches(&self, v: &Val<'_>, key: &str) -> bool {
        match *v {
            Val::Str { s, .. } => s == key,
            Val::Num { n, mode } => {
                let exact = num::parse_int(key, false) == Some(n) && num::is_int_literal(key);
                match mode {
                    NumMode::Str => {
                        let mut buf = [0u8; 20];
                        num::fmt_i64(n, &mut buf) == key
                    }
                    NumMode::Exact => exact,
                    NumMode::Plural => exact || self.category(n, false).as_str() == key,
                    NumMode::Ordinal => exact || self.category(n, true).as_str() == key,
                    NumMode::Raw => false,
                }
            }
            Val::Fallback(_) => false,
        }
    }

    /// BetterThan(rv, k1, k2) for two matching, different keys.
    fn better(v: &Val<'_>, k1: &str, k2: &str) -> bool {
        match *v {
            Val::Num { n, mode: NumMode::Plural | NumMode::Ordinal } => {
                let exact = |k: &str| num::is_int_literal(k) && num::parse_int(k, false) == Some(n);
                exact(k1) && !exact(k2)
            }
            _ => false,
        }
    }

    /// SelectorsCompare(res, keys1 @ a, keys2 @ b): is variant `a` better than `b`?
    fn compare(&self, sels: &[Option<Val<'c>>; MAX_SELECTORS], nsel: usize, a: usize, b: usize) -> Option<bool>
    {
        let mut ca = Cur::new(self.msg, a);
        let mut cb = Cur::new(self.msg, b);
        for sel in sels.iter().take(nsel) {
            let (k1, k2) = (self.key(&mut ca)?, self.key(&mut cb)?);
            match (k1, k2) {
                (None, Some(_)) => return Some(false),
                (Some(_), None) => return Some(true),
                (None, None) => {}
                (Some(x), Some(y)) if x == y => {}
                (Some(x), Some(y)) => return Some(sel.as_ref().is_some_and(|v| Self::better(v, x, y))),
            }
        }
        Some(false)
    }

    /// Formats the pattern at `c`.
    fn pattern(&self, c: &mut Cur<'c>, out: &mut dyn Out, msgdir: Dir, bidi: BidiStrategy, errs: &mut dyn ErrorSink) -> Option<()>
    {
        for _ in 0..c.var()? {
            let h = c.u8()?;
            match h & part::KIND_MASK {
                part::TEXT => out.text(self.cat.str_at(c.usize()?)?),
                part::EXPR => {
                    let v = self.expr(c, h, None, self.ndecl, errs)?;
                    out.value(&v, msgdir, bidi);
                }
                k => {
                    let kind = match k {
                        part::OPEN => MarkupKind::Open,
                        part::STANDALONE => MarkupKind::Standalone,
                        _ => MarkupKind::Close,
                    };
                    let name = self.cat.str_at(c.usize()?)?;
                    let mut options = MarkupOptions { cat: self.cat, args: &[], at: c.i, n: 0 };
                    if h & part::HAS_OPTIONS != 0 {
                        let n = c.var()?;
                        options.at = c.i;
                        options.n = n;
                        Self::skip_n_options(c, n)?;
                    }
                    if h & part::HAS_COLD != 0 {
                        c.var()?;
                    }
                    let options = MarkupOptions { args: self.args, ..options };
                    out.markup(kind, name, options);
                }
            }
        }
        Some(())
    }

    fn skip_n_options(c: &mut Cur<'_>, n: u32) -> Option<()> {
        for _ in 0..n {
            c.var()?;
            Self::skip_value(c)?;
        }
        Some(())
    }
}
