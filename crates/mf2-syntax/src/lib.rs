//! `mf2-syntax` — MessageFormat 2 syntax for mf2-two: a parser to a lossless
//! concrete syntax tree with error recovery, lowering to the data model of
//! [`mf2_model`], the six Data Model Errors, a serializer back to MF2 source,
//! and the variable analysis the build's manifest needs.
//!
//! | Entry point | Gives |
//! |---|---|
//! | [`parse_cst`] / [`Parser::parse_cst`] | a lossless [`Cst`] (tooling: formatter, diagnostics, editors) |
//! | [`parse_model`] / [`Parser::parse_model`] / [`Frontend`] | the data model plus syntax *or* data-model diagnostics (what the build uses) |
//! | [`validate`] | the Data Model Errors of a model built in code |
//! | [`serialize`] | canonical MF2 source for a model |
//! | [`analyze`] | external and local variables, markup and functions of a model |
//!
//! Both parse entry points run one byte-oriented, single-pass, non-recursive
//! parser writing a flat arena ([`cst`]); `parse_model` keeps no trivia and
//! lowers the arena to the model. A [`Parser`] keeps the arena between calls,
//! so a pass over many messages allocates it once. A message without `{`,
//! `}`, `\` or a leading `.` (79 % of real messages) takes a fast path: one
//! scan, and a borrowed single-text model with no allocation.
//!
//! Diagnostics carry a stable detail code: see [`code`].
//!
//! `#![no_std]` + `alloc`; never linked into the client wasm.

#![warn(missing_docs)]
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod analyze;
mod chars;
pub mod code;
pub mod cst;
mod error;
mod lower;
mod norm;
mod parser;
mod serialize;
mod validate;

use alloc::borrow::Cow;
use alloc::vec::Vec;

use mf2_model::{Diagnostic, Diagnostics, Frontend, Message, Parsed, Pattern, PatternMessage};

pub use analyze::{Analysis, Name, analyze};
pub use cst::{Cst, CstRef, Node, SyntaxKind, SyntaxNode};
pub use error::{Error, NameRole};
pub use serialize::serialize;
pub use validate::validate;

/// Parses `source` into a lossless CST (with its syntax diagnostics).
pub fn parse_cst(source: &str) -> Cst<'_> {
    let mut nodes = Vec::new();
    let mut diagnostics = Vec::new();
    parser::parse_into(source, &mut nodes, &mut diagnostics, true);
    Cst {
        src: source,
        nodes,
        diagnostics,
    }
}

/// Parses `source` into the data model and validates it.
///
/// With a syntax error: no model, and every syntax error the parser could
/// recover to. Otherwise: the model, and every data-model error. Spans are
/// byte offsets into `source`.
pub fn parse_model(source: &str) -> Parsed<'_> {
    if let Some(parsed) = simple_text(source) {
        return parsed;
    }
    Parser::new().parse_model(source)
}

/// A parser that keeps its arena between calls (the "reused" state of the
/// parser gate): a pass over many messages grows it once.
#[derive(Clone, Debug, Default)]
pub struct Parser {
    nodes: Vec<Node>,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    /// A parser with an empty arena (does not allocate).
    pub const fn new() -> Self {
        Parser {
            nodes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// A parser whose arena has room for `nodes` entries.
    pub fn with_capacity(nodes: usize) -> Self {
        Parser {
            nodes: Vec::with_capacity(nodes),
            diagnostics: Vec::new(),
        }
    }

    /// Parses `source` into a lossless CST held in this parser's arena.
    pub fn parse_cst<'p, 'src>(&'p mut self, source: &'src str) -> CstRef<'p, 'src> {
        parser::parse_into(source, &mut self.nodes, &mut self.diagnostics, true);
        CstRef {
            src: source,
            nodes: &self.nodes,
            diagnostics: &self.diagnostics,
        }
    }

    /// See [`parse_model`]; reuses this parser's arena.
    pub fn parse_model<'src>(&mut self, source: &'src str) -> Parsed<'src> {
        if let Some(parsed) = simple_text(source) {
            return parsed;
        }
        parser::parse_into(source, &mut self.nodes, &mut self.diagnostics, false);
        if !self.diagnostics.is_empty() || self.nodes.is_empty() {
            return Parsed {
                message: None,
                diagnostics: Diagnostics::from(core::mem::take(&mut self.diagnostics)),
            };
        }
        model_from_arena(source, &self.nodes)
    }
}

impl Frontend for Parser {
    fn parse<'src>(&mut self, source: &'src str) -> Parsed<'src> {
        self.parse_model(source)
    }
}

/// The fast path: a message with no `{`, `}`, `\` or U+0000 whose first
/// non-whitespace character is not `.` is a simple message of one text run
/// (possibly empty), valid by construction.
fn simple_text(source: &str) -> Option<Parsed<'_>> {
    let b = source.as_bytes();
    if chars::find_text_end(b, 0) != b.len() {
        return None;
    }
    let mut i = 0;
    while let Some((n, _)) = chars::trivia_at(b, i) {
        i += n;
    }
    if b.get(i) == Some(&b'.') {
        return None;
    }
    Some(Parsed {
        message: Some(Message::Pattern(PatternMessage {
            declarations: Vec::new(),
            pattern: Pattern::from_text(Cow::Borrowed(source)),
        })),
        diagnostics: Diagnostics::new(),
    })
}

/// Lowers an arena without syntax errors and validates the model, mapping
/// each data-model error to its span.
pub(crate) fn model_from_arena<'src>(source: &'src str, nodes: &[Node]) -> Parsed<'src> {
    let arena = lower::Arena { src: source, nodes };
    let message = arena.message();
    let mut found = Vec::new();
    validate::check(&message, &mut |kind, code, loc| {
        found.push((kind, code, loc));
    });
    if found.is_empty() {
        return Parsed {
            message: Some(message),
            diagnostics: Diagnostics::new(),
        };
    }
    let mut locator = lower::Locator::new(&arena);
    let mut diagnostics: Vec<Diagnostic> = found
        .iter()
        .map(|(kind, code, loc)| Diagnostic::new(*kind, *code, locator.resolve(loc)))
        .collect();
    // Stable: equal positions keep the checks' order.
    diagnostics.sort_by_key(|d| d.span.map_or(0, |s| s.start));
    Parsed {
        message: Some(message),
        diagnostics: Diagnostics::from(diagnostics),
    }
}
