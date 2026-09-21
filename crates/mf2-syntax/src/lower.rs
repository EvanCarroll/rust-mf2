//! Lowering: the arena → the data model, and validation locations → spans.
//!
//! Strings are borrowed from the source (`Cow::Borrowed`) unless an escape
//! makes the cooked value differ from the source text; names exclude the bidi
//! marks around them (they are separate tokens); nothing is normalized. The
//! same code lowers a full CST and the trimmed arena of `parse_model` — it
//! looks only at nodes and at semantic tokens.
//!
//! Every vector is allocated once, at its final size (children are counted
//! first), so lowering a message with `n` collections costs `n` allocations.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use mf2_model::{
    Attributes, CatchAllKey, Declaration, Expression, FunctionExpression, FunctionRef,
    InputDeclaration, Key, Literal, LiteralExpression, LocalDeclaration, Markup, MarkupKind,
    Message, OptionValue, Options, Pattern, PatternMessage, PatternPart, SelectMessage, Span,
    VariableExpression, VariableRef, Variant,
};

use crate::cst::{Node, SyntaxKind as K};
use crate::validate::{ExprLoc, Loc};

/// The source and an arena that parsed without syntax errors.
#[derive(Clone, Copy)]
pub(crate) struct Arena<'s, 'n> {
    pub(crate) src: &'s str,
    pub(crate) nodes: &'n [Node],
}

/// Child indices of a node, in order.
#[derive(Clone)]
struct Children<'n> {
    nodes: &'n [Node],
    next: u32,
    end: u32,
}

impl Iterator for Children<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.next >= self.end {
            return None;
        }
        let i = self.next;
        self.next = self.nodes.get(i as usize).map_or(self.end, |n| n.last);
        Some(i as usize)
    }
}

enum Arg<'s> {
    Literal(Literal<'s>),
    Variable(VariableRef<'s>),
}

fn is_markup(k: K) -> bool {
    k.is_markup()
}

fn is_declaration(k: K) -> bool {
    matches!(k, K::InputDeclaration | K::LocalDeclaration)
}

fn is_literal(k: K) -> bool {
    matches!(k, K::QuotedLiteral | K::UnquotedLiteral)
}

impl<'s> Arena<'s, '_> {
    fn node(&self, i: usize) -> Node {
        self.nodes.get(i).copied().unwrap_or(Node {
            start: 0,
            end: 0,
            last: 0,
            kind: K::Error,
        })
    }

    fn kind(&self, i: usize) -> K {
        self.node(i).kind
    }

    fn text(&self, i: usize) -> &'s str {
        self.src.get(self.node(i).range()).unwrap_or("")
    }

    fn children(&self, i: usize) -> Children<'_> {
        Children {
            nodes: self.nodes,
            next: u32::try_from(i + 1).unwrap_or(u32::MAX),
            end: self.node(i).last,
        }
    }

    fn child(&self, i: usize, pred: impl Fn(K) -> bool) -> Option<usize> {
        self.children(i).find(|&c| pred(self.kind(c)))
    }

    fn count(&self, i: usize, pred: impl Fn(K) -> bool) -> usize {
        self.children(i).filter(|&c| pred(self.kind(c))).count()
    }

    // ── messages ─────────────────────────────────────────────────────────

    pub(crate) fn message(&self) -> Message<'s> {
        let root = 0;
        if self.kind(root) == K::SimpleMessage {
            let pattern = self
                .child(root, |k| k == K::Pattern)
                .map_or_else(Pattern::new, |p| self.pattern(p));
            return Message::Pattern(PatternMessage {
                declarations: Vec::new(),
                pattern,
            });
        }
        let mut declarations = Vec::with_capacity(self.count(root, is_declaration));
        let mut body = None;
        for c in self.children(root) {
            match self.kind(c) {
                K::InputDeclaration => declarations.push(self.input_declaration(c)),
                K::LocalDeclaration => declarations.push(self.local_declaration(c)),
                K::QuotedPattern | K::Matcher => body = Some(c),
                _ => {}
            }
        }
        match body {
            Some(m) if self.kind(m) == K::Matcher => Message::Select(self.select(m, declarations)),
            Some(q) => Message::Pattern(PatternMessage {
                declarations,
                pattern: self.quoted_pattern(q),
            }),
            // Only reachable with a syntax error (no body), which is never lowered.
            None => Message::Pattern(PatternMessage {
                declarations,
                pattern: Pattern::new(),
            }),
        }
    }

    fn input_declaration(&self, d: usize) -> Declaration<'s> {
        let value = match self
            .child(d, |k| k == K::Expression)
            .map(|e| self.expression(e))
        {
            Some(Expression::Variable(v)) => v,
            // Only reachable with a syntax error (diagnosed by the parser).
            _ => VariableExpression {
                arg: VariableRef {
                    name: Cow::Borrowed(""),
                },
                function: None,
                attributes: Attributes::new(),
            },
        };
        Declaration::Input(InputDeclaration {
            name: value.arg.name.clone(),
            value,
        })
    }

    fn local_declaration(&self, d: usize) -> Declaration<'s> {
        let name = self
            .child(d, |k| k == K::Variable)
            .map_or(Cow::Borrowed(""), |v| self.variable(v).name);
        let value = self
            .child(d, |k| k == K::Expression)
            .map_or_else(missing_expression, |e| self.expression(e));
        Declaration::Local(LocalDeclaration { name, value })
    }

    fn select(&self, m: usize, declarations: Vec<Declaration<'s>>) -> SelectMessage<'s> {
        let mut selectors = Vec::with_capacity(self.count(m, |k| k == K::Variable));
        let mut variants = Vec::with_capacity(self.count(m, |k| k == K::Variant));
        for c in self.children(m) {
            match self.kind(c) {
                K::Variable => selectors.push(self.variable(c)),
                K::Variant => variants.push(self.variant(c)),
                _ => {}
            }
        }
        SelectMessage {
            declarations,
            selectors,
            variants,
        }
    }

    fn variant(&self, v: usize) -> Variant<'s> {
        let mut keys = Vec::with_capacity(self.count(v, |k| k == K::Star || is_literal(k)));
        let mut value = Pattern::new();
        for c in self.children(v) {
            match self.kind(c) {
                K::Star => keys.push(Key::CatchAll(CatchAllKey { value: None })),
                K::QuotedLiteral | K::UnquotedLiteral => keys.push(Key::Literal(self.literal(c))),
                K::QuotedPattern => value = self.quoted_pattern(c),
                _ => {}
            }
        }
        Variant { keys, value }
    }

    fn quoted_pattern(&self, q: usize) -> Pattern<'s> {
        self.child(q, |k| k == K::Pattern)
            .map_or_else(Pattern::new, |p| self.pattern(p))
    }

    // ── patterns ─────────────────────────────────────────────────────────

    /// The parts of a pattern node as `(first, last)` child ranges: a text
    /// part is a run of adjacent `Text`/`Escape` tokens (consecutive arena
    /// indices), a placeholder is one node.
    fn parts(&self, p: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        let mut children = self.children(p).peekable();
        core::iter::from_fn(move || {
            loop {
                let c = children.next()?;
                match self.kind(c) {
                    K::Text | K::Escape => {
                        let mut last = c;
                        while let Some(&n) = children.peek() {
                            if !matches!(self.kind(n), K::Text | K::Escape) {
                                break;
                            }
                            last = n;
                            children.next();
                        }
                        return Some((c, last));
                    }
                    k if k == K::Expression || is_markup(k) => return Some((c, c)),
                    _ => {}
                }
            }
        })
    }

    fn pattern(&self, p: usize) -> Pattern<'s> {
        let mut out = Pattern::with_capacity(self.parts(p).count());
        for (first, last) in self.parts(p) {
            let part = match self.kind(first) {
                K::Text | K::Escape => PatternPart::Text(self.cooked(first, last)),
                K::Expression => PatternPart::Expression(self.expression(first)),
                _ => PatternPart::Markup(self.markup(first)),
            };
            out.push(part);
        }
        out
    }

    /// The cooked value of the consecutive text / literal-text / escape
    /// tokens `first..=last`: borrowed from the source unless it holds an
    /// escape next to other text.
    fn cooked(&self, first: usize, last: usize) -> Cow<'s, str> {
        if first == last {
            let text = self.text(first);
            return match self.kind(first) {
                // The escaped character follows the backslash in the source.
                K::Escape => Cow::Borrowed(text.get(1..).unwrap_or("")),
                _ => Cow::Borrowed(text),
            };
        }
        let len = (first..=last).map(|i| self.node(i).range().len()).sum();
        let mut s = String::with_capacity(len);
        for i in first..=last {
            match self.kind(i) {
                K::Escape => s.push_str(self.text(i).get(1..).unwrap_or("")),
                _ => s.push_str(self.text(i)),
            }
        }
        Cow::Owned(s)
    }

    // ── expressions ──────────────────────────────────────────────────────

    fn expression(&self, e: usize) -> Expression<'s> {
        let mut arg = None;
        let mut function = None;
        for c in self.children(e) {
            match self.kind(c) {
                K::Variable => arg = Some(Arg::Variable(self.variable(c))),
                K::QuotedLiteral | K::UnquotedLiteral => arg = Some(Arg::Literal(self.literal(c))),
                K::Function => function = Some(self.function(c)),
                _ => {}
            }
        }
        let attributes = self.attributes(e);
        match (arg, function) {
            (Some(Arg::Literal(arg)), function) => Expression::Literal(LiteralExpression {
                arg,
                function,
                attributes,
            }),
            (Some(Arg::Variable(arg)), function) => Expression::Variable(VariableExpression {
                arg,
                function,
                attributes,
            }),
            (None, Some(function)) => Expression::Function(FunctionExpression {
                function,
                attributes,
            }),
            // Only reachable with a syntax error (diagnosed by the parser).
            (None, None) => missing_expression(),
        }
    }

    fn markup(&self, m: usize) -> Markup<'s> {
        let kind = match self.kind(m) {
            K::MarkupStandalone => MarkupKind::Standalone,
            K::MarkupClose => MarkupKind::Close,
            _ => MarkupKind::Open,
        };
        Markup {
            kind,
            name: self
                .child(m, |k| k == K::Identifier)
                .map_or(Cow::Borrowed(""), |i| self.identifier(i)),
            options: self.options(m),
            attributes: self.attributes(m),
        }
    }

    fn function(&self, f: usize) -> FunctionRef<'s> {
        FunctionRef {
            name: self
                .child(f, |k| k == K::Identifier)
                .map_or(Cow::Borrowed(""), |i| self.identifier(i)),
            options: self.options(f),
        }
    }

    fn options(&self, parent: usize) -> Options<'s> {
        let mut out = Options::with_capacity(self.count(parent, |k| k == K::Option));
        for o in self.children(parent).filter(|&c| self.kind(c) == K::Option) {
            let mut name = Cow::Borrowed("");
            let mut value = OptionValue::Literal(Literal {
                value: Cow::Borrowed(""),
            });
            for c in self.children(o) {
                match self.kind(c) {
                    K::Identifier => name = self.identifier(c),
                    K::Variable => value = OptionValue::Variable(self.variable(c)),
                    K::QuotedLiteral | K::UnquotedLiteral => {
                        value = OptionValue::Literal(self.literal(c));
                    }
                    _ => {}
                }
            }
            out.push(name, value);
        }
        out
    }

    fn attributes(&self, parent: usize) -> Attributes<'s> {
        let mut out = Attributes::with_capacity(self.count(parent, |k| k == K::Attribute));
        for a in self
            .children(parent)
            .filter(|&c| self.kind(c) == K::Attribute)
        {
            let mut name = Cow::Borrowed("");
            let mut value = None;
            for c in self.children(a) {
                match self.kind(c) {
                    K::Identifier => name = self.identifier(c),
                    K::QuotedLiteral | K::UnquotedLiteral => value = Some(self.literal(c)),
                    _ => {}
                }
            }
            out.push(name, value);
        }
        out
    }

    // ── names and literals ───────────────────────────────────────────────

    fn variable(&self, v: usize) -> VariableRef<'s> {
        VariableRef {
            name: self
                .child(v, |k| k == K::Name)
                .map_or(Cow::Borrowed(""), |n| Cow::Borrowed(self.text(n))),
        }
    }

    /// `name` or `namespace:name`: borrowed when the source spells it without
    /// bidi marks around the `:`.
    fn identifier(&self, id: usize) -> Cow<'s, str> {
        let mut names = self.children(id).filter(|&c| self.kind(c) == K::Name);
        match (names.next(), names.next()) {
            (Some(n), None) => Cow::Borrowed(self.text(n)),
            (Some(ns), Some(n)) => {
                let (a, b) = (self.node(ns), self.node(n));
                // One byte apart ⇒ that byte is the `:` (bidi marks are
                // multi-byte).
                if b.start == a.end + 1 {
                    Cow::Borrowed(self.src.get(a.start as usize..b.end as usize).unwrap_or(""))
                } else {
                    let (ns, name) = (self.text(ns), self.text(n));
                    let mut s = String::with_capacity(ns.len() + 1 + name.len());
                    s.push_str(ns);
                    s.push(':');
                    s.push_str(name);
                    Cow::Owned(s)
                }
            }
            _ => Cow::Borrowed(""),
        }
    }

    fn literal(&self, l: usize) -> Literal<'s> {
        if self.kind(l) == K::UnquotedLiteral {
            return Literal {
                value: Cow::Borrowed(self.text(l)),
            };
        }
        let mut content = self
            .children(l)
            .filter(|&c| matches!(self.kind(c), K::LiteralText | K::Escape));
        let value = match content.next() {
            None => Cow::Borrowed(""),
            Some(first) => {
                let last = content.last().unwrap_or(first);
                self.cooked(first, last)
            }
        };
        Literal { value }
    }

    // ── validation locations ─────────────────────────────────────────────

    fn message_pattern(&self) -> Option<usize> {
        if self.kind(0) == K::SimpleMessage {
            self.child(0, |k| k == K::Pattern)
        } else {
            let q = self.child(0, |k| k == K::QuotedPattern)?;
            self.child(q, |k| k == K::Pattern)
        }
    }

    fn span_of(&self, i: usize) -> Span {
        self.node(i).span()
    }
}

/// Maps validation locations to source spans. The node lists it needs are
/// built once; the option list of the last expression and the part list of
/// the last pattern are cached, so resolving many errors (they come grouped
/// by expression and pattern) stays linear.
pub(crate) struct Locator<'a, 's, 'n> {
    arena: &'a Arena<'s, 'n>,
    declarations: Vec<usize>,
    matcher: Option<usize>,
    selectors: Vec<usize>,
    variants: Vec<usize>,
    parts: Option<(Option<usize>, Vec<usize>)>,
    options: Option<(ExprLoc, Vec<usize>)>,
}

impl<'a, 's, 'n> Locator<'a, 's, 'n> {
    pub(crate) fn new(arena: &'a Arena<'s, 'n>) -> Self {
        let declarations = arena
            .children(0)
            .filter(|&c| is_declaration(arena.kind(c)))
            .collect();
        let matcher = arena.child(0, |k| k == K::Matcher);
        let (selectors, variants) = match matcher {
            Some(m) => (
                arena
                    .children(m)
                    .filter(|&c| arena.kind(c) == K::Variable)
                    .collect(),
                arena
                    .children(m)
                    .filter(|&c| arena.kind(c) == K::Variant)
                    .collect(),
            ),
            None => (Vec::new(), Vec::new()),
        };
        Locator {
            arena,
            declarations,
            matcher,
            selectors,
            variants,
            parts: None,
            options: None,
        }
    }

    /// The source span of a validation location.
    pub(crate) fn resolve(&mut self, loc: &Loc) -> Option<Span> {
        let a = self.arena;
        match *loc {
            Loc::Declaration(i) => {
                let d = *self.declarations.get(i)?;
                let v = if a.kind(d) == K::LocalDeclaration {
                    a.child(d, |k| k == K::Variable)?
                } else {
                    let e = a.child(d, |k| k == K::Expression)?;
                    a.child(e, |k| k == K::Variable)?
                };
                Some(a.span_of(v))
            }
            Loc::Selector(i) => Some(a.span_of(*self.selectors.get(i)?)),
            Loc::Variant(i) => {
                let v = *self.variants.get(i)?;
                let mut keys = a
                    .children(v)
                    .filter(|&c| a.kind(c) == K::Star || is_literal(a.kind(c)));
                match keys.next() {
                    Some(first) => {
                        let last = keys.last().unwrap_or(first);
                        Some(Span {
                            start: a.node(first).start,
                            end: a.node(last).end,
                        })
                    }
                    None => Some(a.span_of(v)),
                }
            }
            Loc::Matcher => {
                let start = a.node(self.matcher?).start;
                Some(Span {
                    start,
                    end: start + 6,
                })
            }
            Loc::Option { expr, index } => {
                if self.options.as_ref().is_none_or(|(e, _)| *e != expr) {
                    let owner = self.option_owner(expr)?;
                    let list = a
                        .children(owner)
                        .filter(|&c| a.kind(c) == K::Option)
                        .collect();
                    self.options = Some((expr, list));
                }
                let o = *self.options.as_ref()?.1.get(index)?;
                Some(a.span_of(o))
            }
        }
    }

    /// The function node (of an expression) or markup node holding the
    /// options of `expr`.
    fn option_owner(&mut self, expr: ExprLoc) -> Option<usize> {
        let a = self.arena;
        let e = match expr {
            ExprLoc::Declaration(i) => {
                a.child(*self.declarations.get(i)?, |k| k == K::Expression)?
            }
            ExprLoc::Part { variant, part } => {
                if self.parts.as_ref().is_none_or(|(v, _)| *v != variant) {
                    let p = match variant {
                        None => a.message_pattern()?,
                        Some(v) => {
                            let q = a.child(*self.variants.get(v)?, |k| k == K::QuotedPattern)?;
                            a.child(q, |k| k == K::Pattern)?
                        }
                    };
                    self.parts = Some((variant, a.parts(p).map(|(first, _)| first).collect()));
                }
                *self.parts.as_ref()?.1.get(part)?
            }
        };
        if a.kind(e) == K::Expression {
            a.child(e, |k| k == K::Function)
        } else {
            Some(e)
        }
    }
}

fn missing_expression<'s>() -> Expression<'s> {
    Expression::Literal(LiteralExpression {
        arg: Literal {
            value: Cow::Borrowed(""),
        },
        function: None,
        attributes: Attributes::new(),
    })
}
