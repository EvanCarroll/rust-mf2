//! The evaluator: walks one message's views in place
//! (`plans/03-runtime.md` §2.6). Declarations resolve lazily, each at most
//! once, without recursion; selection is the spec's algorithm; string and
//! parts output share this one walker through [`Out`].

use mf2_catalog::{
    Body, Catalog, DeclView, ExprView, KeyView, Keys, MarkupView, MsgView, Names, Operand,
    OptionsView, PartView, PatternView, SelectView, StrRef, VarRef,
};
use mf2_model::Dir;

use crate::error::FormatError;
use crate::format::BidiStrategy;
use crate::function::{FnContext, Function, OptionEntries, OptionList, OptionValue, Options};
use crate::host::Host;
use crate::parts::{ExpressionPart, FallbackSource, Isolation, MarkupPart};
use crate::scratch::Scratch;
use crate::sink::ErrorSink;
use crate::unannotated;
use crate::value::{Arg, Value};

/// Where one walker's output goes: a [`crate::Sink`] or a
/// [`crate::PartSink`].
pub(crate) trait Out {
    /// Catalog text; `false` if the string is invalid (nothing written).
    fn text(&mut self, catalog: &Catalog, r: StrRef) -> bool;
    /// A formatted placeholder, inside `iso` … PDI when isolated.
    fn expression(&mut self, part: ExpressionPart<'_>, iso: Option<Isolation>);
    /// A fallback value, inside `iso` … PDI when isolated.
    fn fallback(&mut self, source: FallbackSource<'_>, iso: Option<Isolation>);
    /// Markup (string output writes nothing).
    fn markup(&mut self, part: MarkupPart<'_>);
    /// Whether [`Out::markup`] wants the part built.
    fn wants_markup(&self) -> bool;
}

/// A message's arguments: positional, or named and mapped to slots.
#[derive(Clone, Copy)]
pub(crate) enum Args<'e, 'a> {
    Positional(&'e [Arg<'a>]),
    /// The named arguments and, per slot, the index of its argument
    /// (`u32::MAX`: none).
    Named(&'e [(&'e str, Arg<'a>)], &'e Scratch<u32, 8>),
}

impl<'a> Args<'_, 'a> {
    fn get(&self, slot: u32) -> Arg<'a> {
        let slot = slot as usize;
        let arg = match self {
            Args::Positional(a) => a.get(slot).copied(),
            Args::Named(a, map) => map
                .get(slot)
                .and_then(|&i| a.get(i as usize))
                .map(|(_, arg)| *arg),
        };
        arg.unwrap_or(Arg::Unset)
    }
}

/// What a walk needs from the formatter and the call.
pub(crate) struct Env<'e, 'a> {
    pub(crate) catalog: &'a Catalog,
    pub(crate) registry: &'e crate::function::Registry,
    pub(crate) host: &'static dyn Host,
    pub(crate) bidi: BidiStrategy,
    pub(crate) names: Names<'a>,
    pub(crate) args: Args<'e, 'a>,
}

impl<'a> Env<'_, 'a> {
    fn cx(&self, dir: Option<Dir>) -> FnContext<'a> {
        FnContext {
            catalog: self.catalog,
            host: self.host,
            dir,
        }
    }

    fn text(&self, r: StrRef) -> Option<&'a str> {
        self.catalog.text(r)
    }

    fn var_name(&self, v: VarRef) -> Option<&'a str> {
        self.names.var(v).and_then(|r| self.catalog.text(r))
    }
}

/// What a variable, expression or declaration resolved to.
pub(crate) struct Resolved<'a> {
    pub(crate) value: Value<'a>,
    /// The handler that resolved it; `None`: unannotated.
    pub(crate) handler: Option<&'static dyn Function>,
    /// `u:dir` = `ltr`, `rtl` or `auto`: the direction, and isolation.
    pub(crate) udir: Option<Dir>,
    /// `u:id`.
    pub(crate) id: Option<&'a str>,
}

impl<'a> Resolved<'a> {
    fn plain(value: Value<'a>) -> Self {
        Resolved {
            value,
            handler: None,
            udir: None,
            id: None,
        }
    }
}

/// Where a variable's value lives.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    /// Declaration `i`.
    Decl(u32),
    /// Argument slot `s`.
    Arg(u32),
}

enum State<'a> {
    Pending,
    Needed,
    Done(Resolved<'a>),
    /// The same value as `Target` (a declaration in `Done`, or an argument).
    Alias(Target),
    Fallback,
}

struct Decl<'a> {
    expr: ExprView<'a>,
    state: State<'a>,
}

/// No `.input` for a slot.
const NONE: u32 = u32::MAX;

/// A message's declarations and how references reach them.
struct Table<'a> {
    decls: Scratch<Decl<'a>, 8>,
    /// `.local` index → declaration index.
    locals: Scratch<u32, 8>,
    /// Slot → the first `.input` of that slot, or [`NONE`].
    inputs: Scratch<u32, 8>,
}

/// Why a walk stopped: a malformed record, or no memory for a table.
#[derive(Clone, Copy)]
enum Stop {
    Malformed,
    Memory,
}

impl Stop {
    fn error(self) -> FormatError {
        match self {
            Stop::Malformed => FormatError::Malformed,
            Stop::Memory => FormatError::UnsupportedOperation,
        }
    }
}

impl From<mf2_catalog::Malformed> for Stop {
    fn from(_: mf2_catalog::Malformed) -> Self {
        Stop::Malformed
    }
}

fn push<T, const N: usize>(s: &mut Scratch<T, N>, t: T) -> Result<(), Stop> {
    if s.push(t) { Ok(()) } else { Err(Stop::Memory) }
}

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

impl<'a> Table<'a> {
    /// Reads the declarations and returns the table and the body.
    fn build(msg: MsgView<'a>, names: Names<'a>) -> Result<(Table<'a>, Body<'a>), Stop> {
        let mut t = Table {
            decls: Scratch::new(),
            locals: Scratch::new(),
            inputs: Scratch::new(),
        };
        let mut decls = msg.declarations();
        for d in decls.by_ref() {
            let i = index(t.decls.len());
            let expr = match d? {
                DeclView::Input(expr) => {
                    if let Some(Operand::Variable(VarRef::External(s))) = expr.operand() {
                        let ext = names.external_count() as usize;
                        if t.inputs.len() < ext {
                            for _ in t.inputs.len()..ext {
                                push(&mut t.inputs, NONE)?;
                            }
                        }
                        if let Some(slot) = t.inputs.get_mut(s as usize)
                            && *slot == NONE
                        {
                            *slot = i;
                        }
                    }
                    expr
                }
                DeclView::Local { expr, .. } => {
                    push(&mut t.locals, i)?;
                    expr
                }
            };
            push(
                &mut t.decls,
                Decl {
                    expr,
                    state: State::Pending,
                },
            )?;
        }
        let body = decls.body()?;
        Ok((t, body))
    }

    fn len(&self) -> u32 {
        index(self.decls.len())
    }

    /// Where reference `r` at position `p` (a declaration's index, or the
    /// body: [`Table::len`]) leads; `None`: nowhere (unresolved).
    fn target(&self, r: VarRef, p: u32) -> Option<Target> {
        match r {
            VarRef::External(s) => match self.inputs.get(s as usize) {
                Some(&d) if d < p => Some(Target::Decl(d)),
                _ => Some(Target::Arg(s)),
            },
            VarRef::Local(i) => match self.locals.get(i as usize) {
                Some(&d) if d < p => Some(Target::Decl(d)),
                _ => None,
            },
        }
    }

    fn state(&self, d: u32) -> Option<&State<'a>> {
        self.decls.get(d as usize).map(|x| &x.state)
    }

    fn set(&mut self, d: u32, s: State<'a>) {
        if let Some(x) = self.decls.get_mut(d as usize) {
            x.state = s;
        }
    }

    fn is(&self, d: u32, pending: bool) -> bool {
        matches!(
            (self.state(d), pending),
            (Some(State::Pending), true) | (Some(State::Needed), false)
        )
    }

    /// The resolved value of declaration `d`, following an alias to another
    /// declaration.
    fn resolved(&self, d: u32) -> Option<&Resolved<'a>> {
        match self.state(d)? {
            State::Done(r) => Some(r),
            State::Alias(Target::Decl(t)) => match self.state(*t)? {
                State::Done(r) => Some(r),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Calls `f` for every variable reference of `expr`: its operand, then its
/// option values.
fn each_ref(expr: ExprView<'_>, mut f: impl FnMut(VarRef)) {
    if let Some(Operand::Variable(v)) = expr.operand() {
        f(v);
    }
    if let Some(func) = expr.function() {
        each_option_ref(func.options(), f);
    }
}

fn each_option_ref(options: OptionsView<'_>, mut f: impl FnMut(VarRef)) {
    for (_, value) in options.flatten() {
        if let Operand::Variable(v) = value {
            f(v);
        }
    }
}

/// Resolves declaration `d` if it is not yet: first marks what it depends
/// on (walking back from `d`), then resolves the marked declarations in
/// order — every dependency comes first, and nothing recurses.
fn force<'a>(env: &Env<'_, 'a>, table: &mut Table<'a>, d: u32, errs: &mut dyn ErrorSink) {
    if !table.is(d, true) {
        return;
    }
    table.set(d, State::Needed);
    let mut outstanding = 1usize;
    let mut j = d;
    let mut lowest = d;
    loop {
        if table.is(j, false) {
            outstanding -= 1;
            lowest = j;
            if let Some(expr) = table.decls.get(j as usize).map(|x| x.expr) {
                let mut marks: Scratch<u32, 8> = Scratch::new();
                each_ref(expr, |r| {
                    if let Some(Target::Decl(t)) = table.target(r, j)
                        && table.is(t, true)
                    {
                        let _ = marks.push(t);
                    }
                });
                for &t in marks.iter() {
                    if table.is(t, true) {
                        table.set(t, State::Needed);
                        outstanding += 1;
                    }
                }
            }
        }
        if outstanding == 0 || j == 0 {
            break;
        }
        j -= 1;
    }
    for k in lowest..=d {
        if table.is(k, false)
            && let Some(expr) = table.decls.get(k as usize).map(|x| x.expr)
        {
            let state = match resolve_expr(env, table, expr, k, errs) {
                Outcome::Done(r) => State::Done(r),
                Outcome::Alias(t) => State::Alias(t),
                Outcome::Fallback => State::Fallback,
            };
            table.set(k, state);
        }
    }
}

/// Resolves every declaration `expr` refers to (from the body).
fn force_refs<'a>(
    env: &Env<'_, 'a>,
    table: &mut Table<'a>,
    refs: impl FnOnce(&mut dyn FnMut(VarRef)),
    errs: &mut dyn ErrorSink,
) {
    let p = table.len();
    let mut targets: Scratch<u32, 8> = Scratch::new();
    refs(&mut |r| {
        if let Some(Target::Decl(d)) = table.target(r, p) {
            let _ = targets.push(d);
        }
    });
    for &d in targets.iter() {
        force(env, table, d, errs);
    }
}

enum Outcome<'a> {
    Done(Resolved<'a>),
    Alias(Target),
    Fallback,
}

/// A resolved operand, before a function sees it.
#[derive(Clone, Copy)]
enum Opnd<'a> {
    None,
    Literal(&'a str),
    Var(Target),
    Fallback,
}

/// Resolves variable `v` at position `p`; reports Unresolved Variable.
fn variable<'a>(
    env: &Env<'_, 'a>,
    table: &Table<'a>,
    v: VarRef,
    p: u32,
    errs: &mut dyn ErrorSink,
) -> Opnd<'a> {
    match table.target(v, p) {
        Some(Target::Arg(s)) => {
            if matches!(env.args.get(s), Arg::Unset) {
                errs.error(FormatError::UnresolvedVariable);
                Opnd::Fallback
            } else {
                Opnd::Var(Target::Arg(s))
            }
        }
        Some(Target::Decl(d)) => match table.state(d) {
            Some(State::Done(_)) => Opnd::Var(Target::Decl(d)),
            Some(State::Alias(t)) => Opnd::Var(*t),
            Some(State::Fallback) => Opnd::Fallback,
            // Not resolved before its use: only a malformed catalog.
            _ => {
                errs.error(FormatError::UnresolvedVariable);
                Opnd::Fallback
            }
        },
        None => {
            errs.error(FormatError::UnresolvedVariable);
            Opnd::Fallback
        }
    }
}

/// An option value before the list of [`OptionValue`]s borrows it.
enum OptSrc<'a> {
    Temp(Value<'a>),
    Decl(u32),
}

/// Resolved options: the values, `u:id` and `u:dir`.
struct Resolution<'a> {
    list: Scratch<(&'a str, OptSrc<'a>, bool), 8>,
    id: Option<&'a str>,
    udir: Option<Dir>,
}

/// A string value's text with the value's lifetime.
fn str_of<'a>(v: &Value<'a>) -> Option<&'a str> {
    match v {
        Value::Str(s) | Value::Decimal(s) => Some(s),
        Value::Custom(c) => c.as_str(),
        _ => None,
    }
}

/// Resolves `options` at position `p` (formatting.md, "Option
/// Resolution"): fallback values are dropped with Bad Option; `u:id` and
/// `u:dir` are taken out (`u:dir` on markup: Bad Option, ignored).
fn resolve_options<'a>(
    env: &Env<'_, 'a>,
    table: &Table<'a>,
    options: OptionsView<'a>,
    p: u32,
    markup: bool,
    errs: &mut dyn ErrorSink,
) -> Result<Resolution<'a>, Stop> {
    let mut res = Resolution {
        list: Scratch::new(),
        id: None,
        udir: None,
    };
    for o in options {
        let (name, value) = o?;
        let name = env.text(name).ok_or(Stop::Malformed)?;
        let (src, literal) = match value {
            Operand::Literal(r) => (
                OptSrc::Temp(Value::Str(env.text(r).ok_or(Stop::Malformed)?)),
                true,
            ),
            Operand::Variable(v) => match variable(env, table, v, p, errs) {
                Opnd::Var(Target::Decl(d)) => (OptSrc::Decl(d), false),
                Opnd::Var(Target::Arg(s)) => match Value::from_arg(env.args.get(s)) {
                    Some(v) => (OptSrc::Temp(v), false),
                    None => continue,
                },
                _ => {
                    errs.error(FormatError::BadOption);
                    continue;
                }
            },
        };
        match name {
            "u:id" | "u:dir" => {
                let text = match &src {
                    OptSrc::Temp(v) => str_of(v),
                    OptSrc::Decl(d) => table.resolved(*d).and_then(|r| str_of(&r.value)),
                };
                if name == "u:id" {
                    match text {
                        Some(s) => res.id = Some(s),
                        None => errs.error(FormatError::BadOption),
                    }
                } else if markup {
                    errs.error(FormatError::BadOption);
                } else {
                    match text {
                        Some("ltr") => res.udir = Some(Dir::Ltr),
                        Some("rtl") => res.udir = Some(Dir::Rtl),
                        Some("auto") => res.udir = Some(Dir::Auto),
                        Some("inherit") => res.udir = None,
                        _ => errs.error(FormatError::BadOption),
                    }
                }
            }
            _ => push(&mut res.list, (name, src, literal))?,
        }
    }
    Ok(res)
}

/// The list of [`OptionValue`]s over a [`Resolution`].
fn option_list<'o, 'a>(
    table: &'o Table<'a>,
    res: &'o Resolution<'a>,
) -> Result<OptionList<'o, 'a>, Stop> {
    let mut list = OptionList::new();
    for (name, src, literal) in res.list.iter() {
        let value = match src {
            OptSrc::Temp(v) => v,
            OptSrc::Decl(d) => match table.resolved(*d) {
                Some(r) => &r.value,
                None => continue,
            },
        };
        push(
            &mut list,
            (
                *name,
                OptionValue {
                    value,
                    literal: *literal,
                },
            ),
        )?;
    }
    Ok(list)
}

/// Resolves expression `expr` at position `p` (formatting.md,
/// "Expression Resolution"). Every declaration it refers to is resolved.
fn resolve_expr<'a>(
    env: &Env<'_, 'a>,
    table: &Table<'a>,
    expr: ExprView<'a>,
    p: u32,
    errs: &mut dyn ErrorSink,
) -> Outcome<'a> {
    match resolve_expr_or_stop(env, table, expr, p, errs) {
        Ok(o) => o,
        Err(stop) => {
            errs.error(stop.error());
            Outcome::Fallback
        }
    }
}

fn resolve_expr_or_stop<'a>(
    env: &Env<'_, 'a>,
    table: &Table<'a>,
    expr: ExprView<'a>,
    p: u32,
    errs: &mut dyn ErrorSink,
) -> Result<Outcome<'a>, Stop> {
    let opnd = match expr.operand() {
        None => Opnd::None,
        Some(Operand::Literal(r)) => Opnd::Literal(env.text(r).ok_or(Stop::Malformed)?),
        Some(Operand::Variable(v)) => variable(env, table, v, p, errs),
    };
    let Some(func) = expr.function() else {
        return Ok(match opnd {
            Opnd::Literal(s) => Outcome::Done(Resolved::plain(Value::Str(s))),
            Opnd::Var(t) => Outcome::Alias(t),
            Opnd::Fallback | Opnd::None => Outcome::Fallback,
        });
    };
    let name = env.catalog.function(func.index()).ok_or(Stop::Malformed)?;
    let Some(handler) = env.registry.get(name) else {
        errs.error(FormatError::UnknownFunction);
        return Ok(Outcome::Fallback);
    };
    let res = resolve_options(env, table, func.options(), p, false, errs)?;
    let list = option_list(table, &res)?;
    let options = Options {
        entries: OptionEntries::new(&list),
    };
    let temp;
    let operand = match opnd {
        Opnd::Literal(s) => {
            temp = Value::Str(s);
            Some(&temp)
        }
        Opnd::Var(Target::Decl(d)) => table.resolved(d).map(|r| &r.value),
        Opnd::Var(Target::Arg(s)) => match Value::from_arg(env.args.get(s)) {
            Some(v) => {
                temp = v;
                Some(&temp)
            }
            None => None,
        },
        // The handler sees the fallback and decides (the suite: the numeric
        // functions report Bad Operand, `:string` takes `{$x}`;
        // plans/03-runtime.md §2.6).
        Opnd::Fallback => {
            temp = Value::Fallback(source(env, expr));
            Some(&temp)
        }
        Opnd::None => None,
    };
    let cx = env.cx(res.udir);
    Ok(match handler.resolve(&cx, operand, &options, errs) {
        Some(value) => Outcome::Done(Resolved {
            value,
            handler: Some(handler),
            udir: res.udir,
            id: res.id,
        }),
        None => Outcome::Fallback,
    })
}

/// What a fallback value for `expr` shows.
fn source<'a>(env: &Env<'_, 'a>, expr: ExprView<'a>) -> FallbackSource<'a> {
    let src = match expr.operand() {
        Some(Operand::Literal(r)) => env.text(r).map(FallbackSource::Literal),
        Some(Operand::Variable(v)) => env.var_name(v).map(FallbackSource::Variable),
        None => expr
            .function()
            .and_then(|f| env.catalog.function(f.index()))
            .map(FallbackSource::Function),
    };
    src.unwrap_or(FallbackSource::Unknown)
}

/// The Default Bidi Strategy (formatting.md, "Handling Bidirectional
/// Text"): the isolation around a value of direction `dir`.
fn isolation(env: &Env<'_, '_>, dir: Dir, isolate: bool) -> Option<Isolation> {
    if env.bidi == BidiStrategy::None {
        return None;
    }
    match dir {
        Dir::Ltr if env.catalog.dir() == Dir::Ltr && !isolate => None,
        Dir::Ltr => Some(Isolation::Lri),
        Dir::Rtl => Some(Isolation::Rli),
        Dir::Auto => Some(Isolation::Fsi),
    }
}

fn emit_fallback(env: &Env<'_, '_>, src: FallbackSource<'_>, out: &mut dyn Out) {
    out.fallback(src, isolation(env, Dir::Auto, false));
}

/// Formats resolved value `r` of a placeholder whose fallback shows `src`.
fn emit(
    env: &Env<'_, '_>,
    r: &Resolved<'_>,
    src: FallbackSource<'_>,
    out: &mut dyn Out,
    errs: &mut dyn ErrorSink,
) {
    let cx = env.cx(r.udir);
    let dir = match r.handler {
        Some(h) => match h.formattable(&cx, &r.value) {
            Ok(()) => r.udir.unwrap_or_else(|| h.dir(&cx, &r.value)),
            Err(e) => {
                errs.error(e);
                return emit_fallback(env, src, out);
            }
        },
        None => match unannotated::formattable(&r.value, env.host) {
            Ok(()) => r.udir.unwrap_or_else(|| unannotated::dir(&r.value)),
            Err(e) => {
                errs.error(e);
                return emit_fallback(env, src, out);
            }
        },
    };
    let iso = isolation(env, dir, r.udir.is_some());
    out.expression(
        ExpressionPart {
            value: &r.value,
            handler: r.handler,
            cx,
            dir,
            id: r.id,
        },
        iso,
    );
}

/// Formats one placeholder expression.
fn placeholder<'a>(
    env: &Env<'_, 'a>,
    table: &mut Table<'a>,
    expr: ExprView<'a>,
    out: &mut dyn Out,
    errs: &mut dyn ErrorSink,
) {
    force_refs(env, table, |f| each_ref(expr, f), errs);
    let p = table.len();
    let src = source(env, expr);
    match resolve_expr(env, table, expr, p, errs) {
        Outcome::Done(r) => emit(env, &r, src, out, errs),
        Outcome::Alias(Target::Decl(d)) => match table.resolved(d) {
            Some(r) => emit(env, r, src, out, errs),
            None => emit_fallback(env, src, out),
        },
        Outcome::Alias(Target::Arg(s)) => match Value::from_arg(env.args.get(s)) {
            Some(v) => emit(env, &Resolved::plain(v), src, out, errs),
            None => emit_fallback(env, src, out),
        },
        Outcome::Fallback => emit_fallback(env, src, out),
    }
}

/// Formats (for parts: builds) one markup placeholder. Its options resolve
/// in both modes, so their errors are reported either way.
fn markup<'a>(
    env: &Env<'_, 'a>,
    table: &mut Table<'a>,
    m: MarkupView<'a>,
    out: &mut dyn Out,
    errs: &mut dyn ErrorSink,
) -> Result<(), Stop> {
    force_refs(env, table, |f| each_option_ref(m.options(), f), errs);
    let p = table.len();
    let res = resolve_options(env, table, m.options(), p, true, errs)?;
    if out.wants_markup() {
        let name = env.text(m.name()).ok_or(Stop::Malformed)?;
        let list = option_list(table, &res)?;
        out.markup(MarkupPart {
            kind: m.kind(),
            name,
            id: res.id,
            options: OptionEntries::new(&list),
        });
    }
    Ok(())
}

/// Formats a pattern.
fn pattern<'a>(
    env: &Env<'_, 'a>,
    table: &mut Table<'a>,
    pat: PatternView<'a>,
    out: &mut dyn Out,
    errs: &mut dyn ErrorSink,
) -> Result<(), Stop> {
    for part in pat.parts() {
        match part? {
            PartView::Text(r) => {
                if !out.text(env.catalog, r) {
                    return Err(Stop::Malformed);
                }
            }
            PartView::Expression(e) => placeholder(env, table, e, out, errs),
            PartView::Markup(m) => markup(env, table, m, out, errs)?,
        }
    }
    Ok(())
}

/// A resolved selector: a declaration whose value selects, or none.
#[derive(Clone, Copy)]
enum Sel {
    Decl(u32),
    NoMatch,
}

/// Resolves the selectors (formatting.md, "Resolve Selectors").
fn selectors<'a>(
    env: &Env<'_, 'a>,
    table: &mut Table<'a>,
    sv: SelectView<'a>,
    errs: &mut dyn ErrorSink,
) -> Result<Scratch<Sel, 4>, Stop> {
    let p = table.len();
    let mut sels = Scratch::new();
    for r in sv.selectors() {
        let r = r?;
        let sel = match table.target(r, p) {
            Some(Target::Decl(d)) => {
                force(env, table, d, errs);
                let d = match table.state(d) {
                    Some(State::Alias(Target::Decl(t))) => *t,
                    _ => d,
                };
                match table.resolved(d) {
                    Some(res) if res.handler.is_some_and(|h| h.selectable(&res.value)) => {
                        Sel::Decl(d)
                    }
                    _ => {
                        errs.error(FormatError::BadSelector);
                        Sel::NoMatch
                    }
                }
            }
            Some(Target::Arg(s)) => {
                // An unannotated argument never selects.
                if matches!(env.args.get(s), Arg::Unset) {
                    errs.error(FormatError::UnresolvedVariable);
                }
                errs.error(FormatError::BadSelector);
                Sel::NoMatch
            }
            None => {
                errs.error(FormatError::UnresolvedVariable);
                errs.error(FormatError::BadSelector);
                Sel::NoMatch
            }
        };
        push(&mut sels, sel)?;
    }
    Ok(sels)
}

/// `SelectorsMatch(sels, keys)`.
fn selectors_match<'a>(
    env: &Env<'_, 'a>,
    table: &Table<'a>,
    sels: &Scratch<Sel, 4>,
    keys: Keys<'a>,
    errs: &mut dyn ErrorSink,
) -> Result<bool, Stop> {
    for (i, key) in keys.enumerate() {
        let KeyView::Literal(k) = key? else {
            continue;
        };
        let k = env.text(k).ok_or(Stop::Malformed)?;
        let matched = match sels.get(i) {
            Some(Sel::Decl(d)) => match table.resolved(*d) {
                Some(r) => r
                    .handler
                    .is_some_and(|h| h.matches(&env.cx(r.udir), &r.value, k, errs)),
                None => false,
            },
            _ => false,
        };
        if !matched {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `SelectorsCompare(sels, keys1, keys2)`: is `keys1` the better match?
fn selectors_compare<'a>(
    env: &Env<'_, 'a>,
    table: &Table<'a>,
    sels: &Scratch<Sel, 4>,
    keys1: Keys<'a>,
    keys2: Keys<'a>,
) -> Result<bool, Stop> {
    for (i, (k1, k2)) in keys1.zip(keys2).enumerate() {
        let (k1, k2) = match (k1?, k2?) {
            (KeyView::CatchAll, KeyView::CatchAll) => continue,
            (KeyView::CatchAll, KeyView::Literal(_)) => return Ok(false),
            (KeyView::Literal(_), KeyView::CatchAll) => return Ok(true),
            (KeyView::Literal(a), KeyView::Literal(b)) => (a, b),
        };
        let k1 = env.text(k1).ok_or(Stop::Malformed)?;
        let k2 = env.text(k2).ok_or(Stop::Malformed)?;
        if k1 == k2 {
            continue;
        }
        let better = match sels.get(i) {
            Some(Sel::Decl(d)) => table.resolved(*d).is_some_and(|r| {
                r.handler
                    .is_some_and(|h| h.better_than(&env.cx(r.udir), &r.value, k1, k2))
            }),
            _ => false,
        };
        return Ok(better);
    }
    Ok(false)
}

/// Pattern selection (formatting.md): the best matching variant, the first
/// in source order among equals. `None`: no variant matched.
fn select<'a>(
    env: &Env<'_, 'a>,
    table: &mut Table<'a>,
    sv: SelectView<'a>,
    errs: &mut dyn ErrorSink,
) -> Result<Option<PatternView<'a>>, Stop> {
    let sels = selectors(env, table, sv, errs)?;
    let mut best: Option<(Keys<'a>, PatternView<'a>)> = None;
    for v in sv.variants() {
        let v = v?;
        let keys = v.keys();
        if keys.len() as usize != sels.len() {
            continue;
        }
        if !selectors_match(env, table, &sels, keys, errs)? {
            continue;
        }
        let better = match best {
            None => true,
            Some((best_keys, _)) => selectors_compare(env, table, &sels, keys, best_keys)?,
        };
        if better {
            best = Some((keys, v.pattern()));
        }
    }
    Ok(best.map(|(_, p)| p))
}

/// Formats message `msg`.
pub(crate) fn run<'a>(
    env: &Env<'_, 'a>,
    msg: MsgView<'a>,
    out: &mut dyn Out,
    errs: &mut dyn ErrorSink,
) {
    if let Err(stop) = run_or_stop(env, msg, out, errs) {
        errs.error(stop.error());
        emit_fallback(env, FallbackSource::Unknown, out);
    }
}

fn run_or_stop<'a>(
    env: &Env<'_, 'a>,
    msg: MsgView<'a>,
    out: &mut dyn Out,
    errs: &mut dyn ErrorSink,
) -> Result<(), Stop> {
    let (mut table, body) = Table::build(msg, env.names)?;
    let pat = match body {
        Body::Pattern(p) => p,
        Body::Select(sv) => {
            let Some(p) = select(env, &mut table, sv, errs)? else {
                errs.error(FormatError::MissingFallbackVariant);
                emit_fallback(env, FallbackSource::Unknown, out);
                return Ok(());
            };
            p
        }
    };
    pattern(env, &mut table, pat, out, errs)
}
