//! Fluent → MF2, one locale at a time, by the mapping of
//! `plans/05-tooling.md` §6.1.
//!
//! "Faithful" means: for the same arguments, the converted message selects
//! the variant `fluent-bundle` 0.16 selects and writes the same text around
//! it. So references are inlined the way `fluent-bundle` resolves them,
//! selects whose selector is known at conversion are decided here, and a
//! variant `fluent-bundle` can never reach is dropped with a warning.
//!
//! `fluent-syntax`'s AST has no spans, but parsing a `&str` makes every
//! identifier and text a slice of the source, so a construct's position is
//! its slice's address minus the source's.

mod datetime;
mod hoist;
mod number;

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use fluent_syntax::ast;
use fluent_syntax::unicode::unescape_unicode_to_string;
use mf2_model::{
    Attributes, Expression, FunctionRef, Literal, LiteralExpression, VariableExpression,
    VariableRef,
};
use mf2_resource::{Comment, Detached, Entry, Id, Meta, Resource, Section, Span, ValueMap};
use mf2_runtime::{Category, Operands, plural_category};

use self::hoist::{Arm, Flat, Part, Select};
use self::number::{Arg, Num, Opts};
use super::report::{Code, Finding, Report};

/// One `.ftl` file of a locale.
pub(crate) struct File {
    /// The path the report names.
    pub(crate) path: PathBuf,
    /// The `.mf2` file's name, without the extension (`menus.file`).
    pub(crate) name: String,
    pub(crate) text: String,
}

/// One `.mf2` resource to write.
pub(crate) struct Output {
    pub(crate) name: String,
    pub(crate) text: String,
}

/// Switches for the negative controls: a mapping row turned off must show
/// up in the report.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Options {
    /// A Fluent function to treat as unknown.
    pub(crate) without_function: Option<&'static str>,
}

/// Everything one locale's conversion reads.
struct Cx<'s> {
    locale: &'s str,
    files: &'s [File],
    messages: HashMap<&'s str, (usize, &'s ast::Message<&'s str>)>,
    terms: HashMap<&'s str, (usize, &'s ast::Term<&'s str>)>,
    cardinal: Vec<u8>,
    ordinal: Vec<u8>,
    options: Options,
}

/// Converts one locale's files, in path order.
pub(crate) fn convert_locale(
    locale: &str,
    files: &[File],
    options: Options,
    report: &mut Report,
) -> Vec<Output> {
    let asts: Vec<ast::Resource<&str>> = files
        .iter()
        .map(
            |file| match fluent_syntax::parser::parse(file.text.as_str()) {
                Ok(resource) => resource,
                Err((resource, errors)) => {
                    for e in errors {
                        let (line, column) = line_column(&file.text, e.pos.start);
                        report.push(Finding {
                            code: Code::Junk,
                            locale: locale.to_owned(),
                            file: file.path.clone(),
                            line,
                            column,
                            id: None,
                            message: format!(
                                "not Fluent syntax: {}; the entry is left out",
                                e.kind
                            ),
                        });
                    }
                    resource
                }
            },
        )
        .collect();

    // The ids, first in path order; a later definition is reported, since
    // which one `fluent-bundle` keeps depends on the application's load
    // order.
    let mut cx = Cx {
        locale,
        files,
        messages: HashMap::new(),
        terms: HashMap::new(),
        cardinal: plural_entry(mf2_locale_data::plural::PluralKind::Cardinal, locale),
        ordinal: plural_entry(mf2_locale_data::plural::PluralKind::Ordinal, locale),
        options,
    };
    let mut duplicates: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (fi, resource) in asts.iter().enumerate() {
        for (ei, entry) in resource.body.iter().enumerate() {
            let (name, taken) = match entry {
                ast::Entry::Message(m) => (m.id.name, cx.messages.contains_key(m.id.name)),
                ast::Entry::Term(t) => (t.id.name, cx.terms.contains_key(t.id.name)),
                _ => continue,
            };
            if taken {
                duplicates.insert((fi, ei));
                let (line, column) = position(&files[fi].text, name);
                let shown = if matches!(entry, ast::Entry::Term(_)) {
                    format!("-{name}")
                } else {
                    name.to_owned()
                };
                report.push(Finding {
                    code: Code::DuplicateId,
                    locale: locale.to_owned(),
                    file: files[fi].path.clone(),
                    line,
                    column,
                    id: Some(shown.clone()),
                    message: format!(
                        "{shown} is defined again; the first definition, in path order, is kept"
                    ),
                });
                continue;
            }
            match entry {
                ast::Entry::Message(m) => {
                    cx.messages.insert(m.id.name, (fi, m));
                }
                ast::Entry::Term(t) => {
                    cx.terms.insert(t.id.name, (fi, t));
                }
                _ => {}
            }
        }
    }

    let mut outputs = Vec::new();
    for (fi, resource) in asts.iter().enumerate() {
        let mut out: Resource<'static, String> = Resource {
            comment: None,
            meta: vec![Meta {
                name: Cow::Borrowed("locale"),
                value: Some(Cow::Owned(locale.to_owned())),
                span: NO_SPAN,
                value_span: None,
            }],
            sections: Vec::new(),
        };
        let mut section: Section<'static, String> = Section::default();
        let mut resource_comments: Vec<String> = Vec::new();
        for (ei, entry) in resource.body.iter().enumerate() {
            match entry {
                ast::Entry::Message(m) if !duplicates.contains(&(fi, ei)) => {
                    let mut comment = m.comment.as_ref().map(comment_text);
                    let mut produced = Vec::new();
                    if let Some(value) = &m.value {
                        produced.push((m.id.name.to_owned(), value));
                    }
                    for attr in &m.attributes {
                        produced.push((format!("{}.{}", m.id.name, attr.id.name), &attr.value));
                    }
                    for (id, pattern) in produced {
                        let Some(source) = cx.entry(&id, fi, m.id.name, pattern, report) else {
                            continue;
                        };
                        section.entries.push(Entry {
                            id: Id::new(id.split('.').map(|p| Cow::Owned(p.to_owned())).collect()),
                            value: source,
                            comment: comment.take().map(|text| Comment {
                                text: Cow::Owned(text),
                                span: NO_SPAN,
                            }),
                            meta: Vec::new(),
                            span: NO_SPAN,
                            id_span: NO_SPAN,
                            value_span: NO_SPAN,
                            map: ValueMap::Empty,
                        });
                    }
                }
                ast::Entry::Comment(c) | ast::Entry::GroupComment(c) => {
                    section.detached.push(Detached {
                        before: section.entries.len(),
                        comment: Comment {
                            text: Cow::Owned(comment_text(c)),
                            span: NO_SPAN,
                        },
                    });
                }
                ast::Entry::ResourceComment(c) => resource_comments.push(comment_text(c)),
                // A term has no entry: it is inlined where it is used, and
                // its comment goes with it. Junk is already reported.
                _ => {}
            }
        }
        if !resource_comments.is_empty() {
            out.comment = Some(Comment {
                text: Cow::Owned(resource_comments.join("\n\n")),
                span: NO_SPAN,
            });
        }
        let empty = section.entries.is_empty() && section.detached.is_empty();
        *report.entries.entry(locale.to_owned()).or_default() += section.entries.len();
        out.sections.push(section);
        if empty && out.comment.is_none() {
            continue;
        }
        match mf2_resource::serialize_with(&out, &mf2_build::loader::resource::FMT_STYLE) {
            Ok(text) => outputs.push(Output {
                name: files[fi].name.clone(),
                text,
            }),
            Err(e) => report.push(Finding {
                code: Code::Junk,
                locale: locale.to_owned(),
                file: files[fi].path.clone(),
                line: 1,
                column: 1,
                id: None,
                message: format!("cannot be written as a resource: {e}"),
            }),
        }
    }
    outputs
}

const NO_SPAN: Span = Span { start: 0, end: 0 };

fn plural_entry(kind: mf2_locale_data::plural::PluralKind, locale: &str) -> Vec<u8> {
    mf2_locale_data::plural::plural_entry(kind, locale).unwrap_or_default()
}

/// A Fluent comment's text. A resource comment cannot hold a control
/// character, so a tab becomes a space and any other is dropped: losing it
/// is better than losing the file.
fn comment_text(c: &ast::Comment<&str>) -> String {
    c.content
        .iter()
        .map(|line| {
            line.trim_end_matches('\r')
                .chars()
                .filter_map(|ch| match ch {
                    '\t' => Some(' '),
                    ch if ch.is_control() || ch == '\u{2028}' || ch == '\u{2029}' => None,
                    ch => Some(ch),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl<'s> Cx<'s> {
    /// One entry's MF2 source, or `None` if something in it could not be
    /// converted (and is reported).
    fn entry(
        &self,
        id: &str,
        file: usize,
        anchor: &'s str,
        pattern: &'s ast::Pattern<&'s str>,
        report: &mut Report,
    ) -> Option<String> {
        let mut conv = Conv {
            cx: self,
            report,
            id,
            anchor: (file, anchor),
            file,
            stack: vec![pattern],
            env: Env::Message,
            failed: false,
            inlined: BTreeSet::new(),
            functions: BTreeSet::new(),
        };
        let flat = conv.pattern(pattern);
        if conv.failed {
            return None;
        }
        let Ok(message) = hoist::message(&flat) else {
            conv.say(
                Code::VariantLimit,
                anchor,
                format!(
                    "{id} would need more than {} variants; convert it by hand",
                    hoist::VARIANT_LIMIT
                ),
            );
            return None;
        };
        let source = match mf2_syntax::serialize(&message) {
            Ok(source) => source,
            Err(e) => {
                conv.say(Code::Junk, anchor, format!("cannot be written as MF2: {e}"));
                return None;
            }
        };
        debug_assert_eq!(
            mf2_syntax::parse_model(&source)
                .message
                .map(mf2_model::Message::into_owned),
            Some(message),
            "the MF2 source reads back as the message built: {source}"
        );
        let Conv {
            inlined, functions, ..
        } = conv;
        if !inlined.is_empty() {
            report.inlined.insert(id.to_owned(), inlined);
        }
        report.functions.extend(functions);
        Some(source)
    }
}

/// Whose arguments a variable reference reads: the message's (known at run
/// time), or those a term reference bound (known now).
#[derive(Clone)]
enum Env<'s> {
    Message,
    Term(Vec<(&'s str, Arg)>),
}

/// The conversion of one entry.
struct Conv<'c, 's> {
    cx: &'c Cx<'s>,
    report: &'c mut Report,
    id: &'c str,
    /// Where the entry is, for findings with no construct of their own.
    anchor: (usize, &'s str),
    /// The file the pattern being read is in.
    file: usize,
    /// The patterns being read, outermost first: `fluent-bundle`'s
    /// `traveled`, which finds a cycle.
    stack: Vec<&'s ast::Pattern<&'s str>>,
    env: Env<'s>,
    failed: bool,
    inlined: BTreeSet<String>,
    functions: BTreeSet<&'static str>,
}

/// What a selector is, as far as conversion can tell.
enum Selector {
    /// Known now: `fluent-bundle` would compare this with the keys.
    Const(Option<Arg>),
    /// Known at run time: a variable, with the options of the `NUMBER` it
    /// goes through (`None`: a bare variable, typed by its keys).
    Dynamic { var: String, number: Option<Opts> },
}

impl<'s> Conv<'_, 's> {
    fn say(&mut self, code: Code, at: &'s str, message: String) {
        if code.level() == mf2_build::Level::Error {
            self.failed = true;
        }
        // The construct's own file (a term may live in another), else the
        // entry's.
        let files = self.cx.files;
        let (file, (line, column)) = std::iter::once(self.file)
            .chain(0..files.len())
            .find_map(|fi| {
                let o = offset(&files[fi].text, at)?;
                Some((&files[fi], line_column(&files[fi].text, o)))
            })
            .unwrap_or_else(|| {
                let (fi, anchor) = self.anchor;
                (&files[fi], position(&files[fi].text, anchor))
            });
        self.report.push(Finding {
            code,
            locale: self.cx.locale.to_owned(),
            file: file.path.clone(),
            line,
            column,
            id: Some(self.id.to_owned()),
            message,
        });
    }

    fn pattern(&mut self, pattern: &'s ast::Pattern<&'s str>) -> Flat {
        let mut out = Flat::new();
        for element in &pattern.elements {
            match element {
                ast::PatternElement::TextElement { value } => {
                    out.push(Part::Text((*value).to_owned()));
                }
                ast::PatternElement::Placeable { expression } => {
                    out.extend(self.expression(expression));
                }
            }
        }
        out
    }

    fn expression(&mut self, expression: &'s ast::Expression<&'s str>) -> Flat {
        match expression {
            ast::Expression::Inline(e) => self.inline(e),
            ast::Expression::Select { selector, variants } => self.select(selector, variants),
        }
    }

    fn inline(&mut self, e: &'s ast::InlineExpression<&'s str>) -> Flat {
        use ast::InlineExpression as I;
        match e {
            I::StringLiteral { value } => vec![Part::Text(unescape(value))],
            I::NumberLiteral { value } => match Num::parse(value) {
                Some(n) => vec![number_literal(&n)],
                None => vec![Part::Text((*value).to_owned())],
            },
            I::VariableReference { id } => match self.variable(id.name) {
                Bound::Runtime => vec![Part::Expr(variable(id.name, None), None)],
                Bound::To(Arg::Str(s)) => vec![Part::Text(s)],
                Bound::To(Arg::Num(n)) => vec![number_literal(&n)],
                Bound::Unbound => vec![Part::Text(format!("{{${}}}", id.name))],
            },
            I::MessageReference { id, attribute } => {
                let Some(&(fi, message)) = self.cx.messages.get(id.name) else {
                    self.say(
                        Code::MissingReference,
                        id.name,
                        format!("there is no message {}", id.name),
                    );
                    return Flat::new();
                };
                let (pattern, label) = match attribute {
                    Some(a) => (
                        message
                            .attributes
                            .iter()
                            .find(|x| x.id.name == a.name)
                            .map(|x| &x.value),
                        format!("{}.{}", id.name, a.name),
                    ),
                    None => (message.value.as_ref(), id.name.to_owned()),
                };
                let Some(pattern) = pattern else {
                    let what = if attribute.is_some() {
                        format!("there is no attribute {label}")
                    } else {
                        format!("message {label} has no value, only attributes")
                    };
                    self.say(Code::MissingReference, id.name, what);
                    return Flat::new();
                };
                self.inline_pattern(pattern, fi, None, label, id.name)
            }
            I::TermReference {
                id,
                attribute: _,
                arguments,
            } => {
                let env = self.term_env(id.name, arguments.as_ref());
                let Some(&(fi, term)) = self.cx.terms.get(id.name) else {
                    self.say(
                        Code::MissingReference,
                        id.name,
                        format!("there is no term -{}", id.name),
                    );
                    return Flat::new();
                };
                self.inline_pattern(&term.value, fi, Some(env), format!("-{}", id.name), id.name)
            }
            I::FunctionReference { id, arguments } => self.function(id.name, arguments),
            I::Placeable { expression } => self.expression(expression),
        }
    }

    /// Reads a referenced pattern in place, as `fluent-bundle`'s `track`
    /// does: in the same arguments for a message, in the bound ones for a
    /// term; a pattern already being read is a cycle.
    fn inline_pattern(
        &mut self,
        pattern: &'s ast::Pattern<&'s str>,
        file: usize,
        env: Option<Env<'s>>,
        label: String,
        at: &'s str,
    ) -> Flat {
        if self.stack.iter().any(|p| std::ptr::eq(*p, pattern)) {
            self.say(
                Code::CyclicReference,
                at,
                format!("{label} refers back to itself"),
            );
            return Flat::new();
        }
        self.inlined.insert(label);
        self.stack.push(pattern);
        let file = std::mem::replace(&mut self.file, file);
        let env = env.map(|e| std::mem::replace(&mut self.env, e));
        let flat = self.pattern(pattern);
        if let Some(env) = env {
            self.env = env;
        }
        self.file = file;
        self.stack.pop();
        flat
    }

    /// The arguments a term reference binds: its named arguments; a
    /// positional one is ignored, as `fluent-bundle` ignores it.
    fn term_env(
        &mut self,
        name: &'s str,
        arguments: Option<&'s ast::CallArguments<&'s str>>,
    ) -> Env<'s> {
        let mut bound = Vec::new();
        if let Some(args) = arguments {
            if !args.positional.is_empty() {
                self.say(
                    Code::TermPositional,
                    name,
                    format!("-{name} is given a positional argument, which Fluent ignores"),
                );
            }
            for arg in &args.named {
                if let Some(value) = literal(&arg.value) {
                    bound.push((arg.name.name, value));
                }
            }
        }
        Env::Term(bound)
    }

    fn variable(&mut self, name: &'s str) -> Bound {
        match &self.env {
            Env::Message => Bound::Runtime,
            Env::Term(bound) => {
                if let Some((_, value)) = bound.iter().rev().find(|(n, _)| *n == name) {
                    Bound::To(value.clone())
                } else {
                    self.say(
                        Code::UnboundTermVariable,
                        name,
                        format!(
                            "${name} is not given to the term, so Fluent writes \"{{${name}}}\" \
                         as text; converted as that text"
                        ),
                    );
                    Bound::Unbound
                }
            }
        }
    }

    fn function(&mut self, name: &'s str, args: &'s ast::CallArguments<&'s str>) -> Flat {
        let known = self.cx.options.without_function != Some(name);
        match name {
            "NUMBER" if known => self.number(name, args),
            "DATETIME" if known => self.datetime(name, args),
            _ => {
                self.say(
                    Code::UnknownFunction,
                    name,
                    format!("{name}() is not a function MF2 has a counterpart for"),
                );
                Flat::new()
            }
        }
    }

    /// `NUMBER`'s operand and options, merged as `fluent-bundle` merges
    /// them; `None` if the operand is not a number `fluent-bundle` accepts.
    fn number_args(
        &mut self,
        name: &'s str,
        args: &'s ast::CallArguments<&'s str>,
    ) -> Option<(Operand, Opts)> {
        use ast::InlineExpression as I;
        let operand = match args.positional.first() {
            Some(I::VariableReference { id }) => match self.variable(id.name) {
                Bound::Runtime => Some(Operand::Var(id.name.to_owned())),
                Bound::To(Arg::Num(n)) => Some(Operand::Num(n)),
                Bound::To(Arg::Str(_)) | Bound::Unbound => None,
            },
            Some(I::NumberLiteral { value }) => Num::parse(value).map(Operand::Num),
            _ => None,
        };
        let Some(operand) = operand else {
            self.say(
                Code::NumberOperand,
                name,
                "NUMBER() needs a variable or a number literal; fluent-bundle gives an error for \
                 anything else"
                    .to_owned(),
            );
            return None;
        };
        let mut named = Vec::new();
        for arg in &args.named {
            if let Some(value) = literal(&arg.value) {
                named.push((arg.name.name, value));
            }
        }
        let mut opts = match &operand {
            Operand::Var(_) => Opts::default(),
            Operand::Num(n) => n.opts.clone(),
        };
        for ignored in opts.merge(&named) {
            self.say(
                Code::NumberOption,
                ignored.name,
                format!(
                    "fluent-bundle ignores the NUMBER option {} (unknown, or a value of the wrong \
                     type); dropped",
                    ignored.name
                ),
            );
        }
        Some((operand, opts))
    }

    fn number(&mut self, name: &'s str, args: &'s ast::CallArguments<&'s str>) -> Flat {
        let Some((operand, opts)) = self.number_args(name, args) else {
            return Flat::new();
        };
        let (function, losses) = number::placeholder(&opts);
        for loss in losses {
            match loss {
                number::Loss::CurrencyMissing => self.say(
                    Code::CurrencyMissing,
                    name,
                    "NUMBER(style: \"currency\") names no currency".to_owned(),
                ),
                number::Loss::Option(option) => self.say(
                    Code::NumberOption,
                    name,
                    format!(
                        "{option} has no :currency counterpart unless the minimum and maximum \
                         fraction digits are equal (fractionDigits); dropped"
                    ),
                ),
            }
        }
        let Some(function) = function else {
            return Flat::new();
        };
        self.functions.insert(static_name(&function.name));
        let part = match operand {
            Operand::Var(var) => Part::Expr(variable(&var, Some(function)), None),
            Operand::Num(n) => {
                let written = Num {
                    value: n.value,
                    opts,
                    text: None,
                }
                .as_string();
                Part::Expr(
                    Expression::Literal(LiteralExpression {
                        arg: Literal {
                            value: Cow::Owned(n.mf2_literal()),
                        },
                        function: Some(function),
                        attributes: Attributes::default(),
                    }),
                    Some(written),
                )
            }
        };
        vec![part]
    }

    fn datetime(&mut self, name: &'s str, args: &'s ast::CallArguments<&'s str>) -> Flat {
        use ast::InlineExpression as I;
        enum Operand {
            Var(String),
            Lit(String),
        }
        let operand = match args.positional.first() {
            Some(I::VariableReference { id }) => match self.variable(id.name) {
                Bound::Runtime => Some(Operand::Var(id.name.to_owned())),
                Bound::To(Arg::Str(s)) => Some(Operand::Lit(s)),
                Bound::To(Arg::Num(_)) | Bound::Unbound => None,
            },
            Some(I::StringLiteral { value }) => Some(Operand::Lit(unescape(value))),
            _ => None,
        };
        let Some(operand) = operand else {
            self.say(
                Code::DatetimeOption,
                name,
                "DATETIME() needs a variable or a string literal".to_owned(),
            );
            return Flat::new();
        };
        let named: Vec<(&str, String)> = args
            .named
            .iter()
            .filter_map(|a| match &a.value {
                I::StringLiteral { value } => Some((a.name.name, unescape(value))),
                I::NumberLiteral { value } => Some((a.name.name, (*value).to_owned())),
                _ => None,
            })
            .collect();
        let function = match datetime::map(&named) {
            Ok(f) => f,
            Err(option) => {
                self.say(
                    Code::DatetimeOption,
                    name,
                    format!("DATETIME option {option} has no MF2 counterpart"),
                );
                return Flat::new();
            }
        };
        self.say(
            Code::DatetimeApproximate,
            name,
            format!(
                "DATETIME() became :{}, the nearest MF2 form; check that it reads as intended",
                function.name
            ),
        );
        self.functions.insert(static_name(&function.name));
        let expression = match operand {
            Operand::Var(var) => variable(&var, Some(function)),
            Operand::Lit(value) => Expression::Literal(LiteralExpression {
                arg: Literal {
                    value: Cow::Owned(value),
                },
                function: Some(function),
                attributes: Attributes::default(),
            }),
        };
        vec![Part::Expr(expression, None)]
    }

    fn select(
        &mut self,
        selector: &'s ast::InlineExpression<&'s str>,
        variants: &'s [ast::Variant<&'s str>],
    ) -> Flat {
        let Some(selector) = self.selector(selector) else {
            return Flat::new();
        };
        match selector {
            Selector::Const(value) => {
                // `fluent-bundle`'s own loop: the first key that matches,
                // else the default.
                let chosen = value
                    .as_ref()
                    .and_then(|v| {
                        variants
                            .iter()
                            .find(|variant| self.matches(&variant.key, v))
                    })
                    .or_else(|| variants.iter().find(|v| v.default));
                match chosen {
                    Some(variant) => self.pattern(&variant.value),
                    None => Flat::new(),
                }
            }
            Selector::Dynamic { var, number } => {
                self.dynamic_select(var, number, selector_anchor(variants), variants)
            }
        }
    }

    /// Whether `fluent-bundle` takes `key` for the constant `value`
    /// (`FluentValue::matches`).
    fn matches(&self, key: &ast::VariantKey<&str>, value: &Arg) -> bool {
        match (key, value) {
            (ast::VariantKey::Identifier { name }, Arg::Str(s)) => name == s,
            (ast::VariantKey::NumberLiteral { value: k }, Arg::Num(n)) => {
                Num::parse(k).is_some_and(|k| k == *n)
            }
            (ast::VariantKey::Identifier { name }, Arg::Num(n)) => {
                Category::from_keyword(name).is_some_and(|c| Some(c) == self.category(n))
            }
            (ast::VariantKey::NumberLiteral { .. }, Arg::Str(_)) => false,
        }
    }

    /// The plural category `fluent-bundle` gives a number: its operands are
    /// those of the text it writes (`f64`'s, padded to the minimum fraction
    /// digits).
    fn category(&self, n: &Num) -> Option<Category> {
        let operands = Operands::parse(&n.as_string())?;
        let entry = if n.opts.ordinal {
            &self.cx.ordinal
        } else {
            &self.cx.cardinal
        };
        Some(plural_category(entry, &operands))
    }

    /// What the selector is; `None` if it cannot be converted (reported).
    fn selector(&mut self, e: &'s ast::InlineExpression<&'s str>) -> Option<Selector> {
        use ast::InlineExpression as I;
        match e {
            I::StringLiteral { value } => Some(Selector::Const(Some(Arg::Str(unescape(value))))),
            I::NumberLiteral { value } => Some(Selector::Const(Num::parse(value).map(Arg::Num))),
            I::VariableReference { id } => match self.variable(id.name) {
                Bound::Runtime => Some(Selector::Dynamic {
                    var: id.name.to_owned(),
                    number: None,
                }),
                Bound::To(value) => Some(Selector::Const(Some(value))),
                // `fluent-bundle` resolves it to an error, which matches no
                // key: the default is taken.
                Bound::Unbound => Some(Selector::Const(None)),
            },
            I::TermReference {
                id,
                attribute: Some(attribute),
                arguments,
            } => {
                // Fluent's grammatical-gender idiom: a term attribute, known
                // now.
                let env = self.term_env(id.name, arguments.as_ref());
                let label = format!("-{}.{}", id.name, attribute.name);
                let Some(&(fi, term)) = self.cx.terms.get(id.name) else {
                    self.say(
                        Code::MissingReference,
                        id.name,
                        format!("there is no term -{}", id.name),
                    );
                    return None;
                };
                let Some(attr) = term.attributes.iter().find(|a| a.id.name == attribute.name)
                else {
                    self.say(
                        Code::MissingReference,
                        attribute.name,
                        format!("there is no attribute {label}"),
                    );
                    return None;
                };
                let flat = self.inline_pattern(&attr.value, fi, Some(env), label.clone(), id.name);
                let mut text = String::new();
                for part in &flat {
                    match part {
                        Part::Text(t) | Part::Expr(_, Some(t)) => text.push_str(t),
                        _ => {
                            self.say(
                                Code::DateSelector,
                                attribute.name,
                                format!("{label} is selected on but has no fixed text"),
                            );
                            return None;
                        }
                    }
                }
                Some(Selector::Const(Some(Arg::Str(text))))
            }
            I::FunctionReference { id, arguments } => {
                let known = self.cx.options.without_function != Some(id.name);
                match id.name {
                    "NUMBER" if known => {
                        let (operand, opts) = self.number_args(id.name, arguments)?;
                        Some(match operand {
                            Operand::Var(var) => Selector::Dynamic {
                                var,
                                number: Some(opts),
                            },
                            Operand::Num(n) => Selector::Const(Some(Arg::Num(Num {
                                value: n.value,
                                opts,
                                text: n.text,
                            }))),
                        })
                    }
                    "DATETIME" if known => {
                        self.say(
                            Code::DateSelector,
                            id.name,
                            "MF2 dates do not select".to_owned(),
                        );
                        None
                    }
                    _ => {
                        self.say(
                            Code::UnknownFunction,
                            id.name,
                            format!("{}() is not a function MF2 has a counterpart for", id.name),
                        );
                        None
                    }
                }
            }
            // The rest is not a selector in Fluent's grammar.
            I::TermReference { .. } | I::MessageReference { .. } | I::Placeable { .. } => None,
        }
    }

    /// A select known only at run time: one column of the message's
    /// `.match`, with the variants `fluent-bundle` can never reach dropped.
    fn dynamic_select(
        &mut self,
        var: String,
        number: Option<Opts>,
        anchor: Option<&'s str>,
        variants: &'s [ast::Variant<&'s str>],
    ) -> Flat {
        // A bare variable has no type in Fluent: the keys say which.
        let (function, opts) = if let Some(opts) = number {
            (number::selector(&opts), Some(opts))
        } else {
            let numeric = variants.iter().any(|v| match &v.key {
                ast::VariantKey::NumberLiteral { .. } => true,
                ast::VariantKey::Identifier { name } => is_category(name) && *name != "other",
            });
            let textual = variants.iter().any(|v| match &v.key {
                ast::VariantKey::NumberLiteral { .. } => false,
                ast::VariantKey::Identifier { name } => !is_category(name),
            });
            if numeric && textual {
                self.say(
                    Code::MixedKeys,
                    anchor.unwrap_or(self.anchor.1),
                    format!(
                        "${var} is selected with plural keys and other keys at once, so \
                         whether it is a number or a string is known only at run time"
                    ),
                );
                return Flat::new();
            }
            if numeric {
                (number::selector(&Opts::default()), Some(Opts::default()))
            } else {
                (
                    FunctionRef {
                        name: Cow::Borrowed("string"),
                        options: mf2_model::Options::new(),
                    },
                    None,
                )
            }
        };
        self.functions.insert(static_name(&function.name));

        let mut arms = Vec::new();
        let mut default = None;
        let mut names: Vec<&str> = Vec::new();
        let mut numbers: Vec<Num> = Vec::new();
        let mut categories: Vec<Category> = Vec::new();
        for variant in variants {
            let (reachable, key, at) = match &variant.key {
                ast::VariantKey::Identifier { name } => {
                    let fresh = !names.contains(name);
                    names.push(name);
                    if let Some(c) = Category::from_keyword(name) {
                        categories.push(c);
                    }
                    (fresh, (*name).to_owned(), *name)
                }
                ast::VariantKey::NumberLiteral { value } => match (Num::parse(value), &opts) {
                    (Some(n), Some(opts)) => {
                        // `FluentNumber`'s equality: the key and the
                        // argument must agree on every option, and a
                        // category key before it already takes its number.
                        let reachable = n.opts == *opts
                            && !numbers.contains(&n)
                            && self.category(&n).is_none_or(|c| !categories.contains(&c));
                        let key = n.mf2_literal();
                        numbers.push(n);
                        (reachable, key, *value)
                    }
                    _ => (false, (*value).to_owned(), *value),
                },
            };
            if variant.default {
                default = Some(variant);
                // `*[1]`: MF2 prefers an exact number to a category, so the
                // number stays a key of its own.
                if reachable && matches!(variant.key, ast::VariantKey::NumberLiteral { .. }) {
                    let pattern = self.pattern(&variant.value);
                    arms.push(Arm {
                        key: Some(key),
                        pattern,
                    });
                }
            } else if reachable {
                let pattern = self.pattern(&variant.value);
                arms.push(Arm {
                    key: Some(key),
                    pattern,
                });
            } else {
                self.say(
                    Code::UnreachableVariant,
                    at,
                    format!(
                        "the variant [{at}] can never be chosen by fluent-bundle (an earlier key \
                         takes it, or its options differ); dropped"
                    ),
                );
            }
        }
        let Some(default) = default else {
            return Flat::new();
        };
        let pattern = self.pattern(&default.value);
        arms.push(Arm { key: None, pattern });
        vec![Part::Select(Box::new(Select {
            var,
            function,
            arms,
        }))]
    }
}

/// What a variable reference reads.
enum Bound {
    /// The message's argument, at run time.
    Runtime,
    /// A value the term reference bound.
    To(Arg),
    /// Nothing: a term's variable its reference did not bind.
    Unbound,
}

/// `NUMBER`'s operand.
enum Operand {
    Var(String),
    Num(Num),
}

fn is_category(name: &str) -> bool {
    Category::from_keyword(name).is_some()
}

fn selector_anchor<'s>(variants: &'s [ast::Variant<&'s str>]) -> Option<&'s str> {
    variants.first().map(|v| match &v.key {
        ast::VariantKey::Identifier { name } => *name,
        ast::VariantKey::NumberLiteral { value } => *value,
    })
}

/// A literal argument's value (named arguments are literals in Fluent's
/// grammar).
fn literal(e: &ast::InlineExpression<&str>) -> Option<Arg> {
    match e {
        ast::InlineExpression::StringLiteral { value } => Some(Arg::Str(unescape(value))),
        ast::InlineExpression::NumberLiteral { value } => Num::parse(value).map(Arg::Num),
        _ => None,
    }
}

/// A number literal as a placeholder: the text `fluent-bundle` writes for
/// it, as an MF2 literal.
fn number_literal(n: &Num) -> Part {
    let text = n.as_string();
    Part::Expr(
        Expression::Literal(LiteralExpression {
            arg: Literal {
                value: Cow::Owned(text.clone()),
            },
            function: None,
            attributes: Attributes::default(),
        }),
        Some(text),
    )
}

fn variable(name: &str, function: Option<FunctionRef<'static>>) -> Expression<'static> {
    Expression::Variable(VariableExpression {
        arg: VariableRef {
            name: Cow::Owned(name.to_owned()),
        },
        function,
        attributes: Attributes::default(),
    })
}

fn unescape(value: &str) -> String {
    unescape_unicode_to_string(value).into_owned()
}

/// The report's name for a function the converter writes.
fn static_name(name: &str) -> &'static str {
    mf2_build::features::BUILTINS
        .iter()
        .map(|(n, _)| *n)
        .find(|n| *n == name)
        .unwrap_or("string")
}

/// Where `slice` starts in `text`, if it is a slice of it.
fn offset(text: &str, slice: &str) -> Option<usize> {
    let base = text.as_ptr().addr();
    let at = slice.as_ptr().addr();
    (at >= base && at < base + text.len()).then(|| at - base)
}

/// The line and column of `slice` in `text`; the file's start if it is not
/// a slice of it.
fn position(text: &str, slice: &str) -> (u32, u32) {
    offset(text, slice).map_or((1, 1), |o| line_column(text, o))
}

/// One-based line and column (in characters) of a byte offset.
fn line_column(text: &str, offset: usize) -> (u32, u32) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(column).unwrap_or(u32::MAX),
    )
}
