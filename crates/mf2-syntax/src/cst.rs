//! The concrete syntax tree: a flat, pre-order arena of nodes and tokens.
//!
//! Every byte of the source belongs to exactly one **token** (a leaf), and
//! the tokens, in order, spell the source: the CST is lossless
//! (`cst.to_string() == source`). Whitespace and bidi marks are
//! [`SyntaxKind::Trivia`] tokens; input the parser could not use is kept in
//! [`SyntaxKind::Error`] tokens. **Nodes** group tokens and other nodes; each
//! records the index one past its last descendant, so the tree is navigated
//! without pointers or recursion.
//!
//! [`crate::parse_model`] fills the same arena without trivia and punctuation
//! tokens (lowering does not need them); such an arena is not lossless.

use alloc::vec::Vec;
use core::fmt;

use mf2_model::{Diagnostic, Diagnostics, Parsed, Span};

/// What a node or token is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum SyntaxKind {
    // ── nodes ──────────────────────────────────────────────────────────
    /// Root of a simple message: one [`SyntaxKind::Pattern`].
    SimpleMessage,
    /// Root of a complex message: declarations, then a body.
    ComplexMessage,
    /// `.input {$name …}`.
    InputDeclaration,
    /// `.local $name = {…}`.
    LocalDeclaration,
    /// `{{` pattern `}}`.
    QuotedPattern,
    /// Text and placeholders.
    Pattern,
    /// `.match` selectors variants.
    Matcher,
    /// Keys and a quoted pattern.
    Variant,
    /// `{operand :function @attribute}`.
    Expression,
    /// `{#name …}`.
    MarkupOpen,
    /// `{#name … /}`.
    MarkupStandalone,
    /// `{/name …}`.
    MarkupClose,
    /// `:identifier options`.
    Function,
    /// `identifier = value`.
    Option,
    /// `@identifier [= literal]`.
    Attribute,
    /// `$name`.
    Variable,
    /// `[namespace :] name`.
    Identifier,
    /// `|…|`.
    QuotedLiteral,

    // ── tokens ─────────────────────────────────────────────────────────
    /// A run of text characters in a pattern (no escapes).
    Text,
    /// A two-character escape sequence: `\\`, `\{`, `\|` or `\}`.
    Escape,
    /// A run of characters inside a quoted literal (no escapes).
    LiteralText,
    /// An unquoted literal.
    UnquotedLiteral,
    /// A name (of a variable, or one part of an identifier).
    Name,
    /// The catch-all key `*`.
    Star,
    /// `.input`.
    KwInput,
    /// `.local`.
    KwLocal,
    /// `.match`.
    KwMatch,
    /// `{`.
    LBrace,
    /// `}`.
    RBrace,
    /// `{{`.
    LBrace2,
    /// `}}`.
    RBrace2,
    /// `|`.
    Pipe,
    /// `$`.
    Dollar,
    /// `:` (function sigil or namespace separator).
    Colon,
    /// `#`.
    Hash,
    /// `/` (markup-close sigil or standalone marker).
    Slash,
    /// `@`.
    At,
    /// `=`.
    Equals,
    /// Whitespace and bidi marks outside text and literals.
    Trivia,
    /// Input the parser could not use (a syntax error was reported for it).
    Error,
}

impl SyntaxKind {
    /// A leaf (token) kind.
    pub fn is_token(self) -> bool {
        (self as u8) >= (SyntaxKind::Text as u8)
    }

    /// A node kind.
    pub fn is_node(self) -> bool {
        !self.is_token()
    }

    /// Tokens the data model needs; the others (trivia, punctuation,
    /// keywords, errors) are left out of [`crate::parse_model`]'s arena.
    pub(crate) fn is_semantic_token(self) -> bool {
        matches!(
            self,
            SyntaxKind::Text
                | SyntaxKind::Escape
                | SyntaxKind::LiteralText
                | SyntaxKind::UnquotedLiteral
                | SyntaxKind::Name
                | SyntaxKind::Star
        )
    }

    /// A markup node kind.
    pub fn is_markup(self) -> bool {
        matches!(
            self,
            SyntaxKind::MarkupOpen | SyntaxKind::MarkupStandalone | SyntaxKind::MarkupClose
        )
    }
}

/// One entry of the arena.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Node {
    pub(crate) start: u32,
    pub(crate) end: u32,
    /// Index one past the last descendant (`index + 1` for a token).
    pub(crate) last: u32,
    pub(crate) kind: SyntaxKind,
}

impl Node {
    /// What it is.
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    /// Where it is.
    pub fn span(&self) -> Span {
        Span {
            start: self.start,
            end: self.end,
        }
    }

    pub(crate) fn range(&self) -> core::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

/// A borrowed CST: the source, the arena and the syntax diagnostics.
#[derive(Clone, Copy, Debug)]
pub struct CstRef<'a, 'src> {
    pub(crate) src: &'src str,
    pub(crate) nodes: &'a [Node],
    pub(crate) diagnostics: &'a [Diagnostic],
}

impl<'a, 'src> CstRef<'a, 'src> {
    /// The source.
    pub fn source(&self) -> &'src str {
        self.src
    }

    /// The root node (`None` only for a source over `u32::MAX` bytes, which
    /// is rejected with [`crate::code::SOURCE_TOO_LONG`]).
    pub fn root(&self) -> Option<SyntaxNode<'a, 'src>> {
        (!self.nodes.is_empty()).then_some(SyntaxNode {
            src: self.src,
            nodes: self.nodes,
            index: 0,
        })
    }

    /// The whole arena, in pre-order.
    pub fn nodes(&self) -> &'a [Node] {
        self.nodes
    }

    /// The syntax errors, in source order of detection.
    pub fn diagnostics(&self) -> &'a [Diagnostic] {
        self.diagnostics
    }

    /// Whether any syntax error was reported.
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// The tokens (leaves), in source order.
    pub fn tokens(&self) -> impl Iterator<Item = SyntaxNode<'a, 'src>> + use<'a, 'src> {
        let (src, nodes) = (self.src, self.nodes);
        nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind.is_token())
            .map(move |(i, _)| SyntaxNode {
                src,
                nodes,
                // Arena indices fit in u32: sources over u32::MAX bytes are
                // rejected before parsing.
                index: u32::try_from(i).unwrap_or(u32::MAX),
            })
    }

    /// Lowers the CST to the data model and validates it (spans included);
    /// with a syntax error, no model and the syntax diagnostics.
    pub fn to_model(&self) -> Parsed<'src> {
        if self.has_errors() || self.nodes.is_empty() {
            return Parsed {
                message: None,
                diagnostics: Diagnostics::from(self.diagnostics.to_vec()),
            };
        }
        crate::model_from_arena(self.src, self.nodes)
    }
}

impl fmt::Display for CstRef<'_, '_> {
    /// Writes the tokens' text in order, which is the source.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self.tokens() {
            f.write_str(t.text())?;
        }
        Ok(())
    }
}

/// An owned CST (see [`crate::parse_cst`]).
#[derive(Clone, Debug)]
pub struct Cst<'src> {
    pub(crate) src: &'src str,
    pub(crate) nodes: Vec<Node>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl<'src> Cst<'src> {
    /// The borrowed view, with every accessor.
    pub fn view(&self) -> CstRef<'_, 'src> {
        CstRef {
            src: self.src,
            nodes: &self.nodes,
            diagnostics: &self.diagnostics,
        }
    }

    /// The source.
    pub fn source(&self) -> &'src str {
        self.src
    }

    /// The root node (see [`CstRef::root`]).
    pub fn root(&self) -> Option<SyntaxNode<'_, 'src>> {
        self.view().root()
    }

    /// The syntax errors.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether any syntax error was reported.
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// See [`CstRef::to_model`].
    pub fn to_model(&self) -> Parsed<'src> {
        self.view().to_model()
    }
}

impl fmt::Display for Cst<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.view(), f)
    }
}

/// A node or token of a CST, with navigation.
#[derive(Clone, Copy, Debug)]
pub struct SyntaxNode<'a, 'src> {
    src: &'src str,
    nodes: &'a [Node],
    index: u32,
}

impl<'a, 'src> SyntaxNode<'a, 'src> {
    fn node(&self) -> Node {
        self.nodes[self.index as usize]
    }

    /// Its index in the arena.
    pub fn index(&self) -> usize {
        self.index as usize
    }

    /// What it is.
    pub fn kind(&self) -> SyntaxKind {
        self.node().kind
    }

    /// Where it is.
    pub fn span(&self) -> Span {
        self.node().span()
    }

    /// The source text it covers.
    pub fn text(&self) -> &'src str {
        self.src.get(self.node().range()).unwrap_or("")
    }

    /// Whether it is a token (leaf).
    pub fn is_token(&self) -> bool {
        self.kind().is_token()
    }

    /// Its direct children, in order.
    pub fn children(&self) -> impl Iterator<Item = SyntaxNode<'a, 'src>> + use<'a, 'src> {
        let (src, nodes) = (self.src, self.nodes);
        let end = self.node().last;
        let mut next = self.index + 1;
        core::iter::from_fn(move || {
            if next >= end {
                return None;
            }
            let index = next;
            next = nodes.get(index as usize).map_or(end, |n| n.last);
            Some(SyntaxNode { src, nodes, index })
        })
    }

    /// Its descendants (not itself), in pre-order.
    pub fn descendants(&self) -> impl Iterator<Item = SyntaxNode<'a, 'src>> + use<'a, 'src> {
        let (src, nodes) = (self.src, self.nodes);
        (self.index + 1..self.node().last).map(move |index| SyntaxNode { src, nodes, index })
    }
}
