//! The parser: MF2 source → the flat arena of [`crate::cst`].
//!
//! Byte-oriented and single-pass. The grammar (`spec/message.abnf`) has no
//! recursive productions, and no function here calls itself, directly or
//! indirectly: call depth is bounded by the grammar (message → declaration →
//! expression → function → option → literal), never by the input. Lookahead
//! is bounded to a run of whitespace/bidi marks, which is then consumed, so
//! every byte is scanned a bounded number of times: time is linear.
//!
//! Recovery keeps going after an error: an unusable character inside a
//! placeholder skips to its closing `}`; junk between declarations skips to
//! the next keyword or `{{`. Every consumed byte ends up in a token, so the
//! CST stays lossless even for malformed input.
//!
//! The position only advances through [`Parser::token_to`], which also emits
//! the token — that is what makes the tokens tile the source.

use alloc::vec::Vec;

use mf2_model::{Diagnostic, ErrorKind, Span};

use crate::chars;
use crate::code;
use crate::cst::{Node, SyntaxKind as K};

/// Where a pattern ends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    /// At the end of the source (a simple message).
    Eof,
    /// At `}}` (a quoted pattern).
    Quoted,
}

/// Where an expression appears.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// A placeholder in a pattern: markup allowed.
    Placeholder,
    /// The value of a declaration: markup not allowed.
    Declaration,
}

/// What an expression turned out to hold (for `.input`'s check).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operand {
    /// `{:fn}`.
    None,
    Variable,
    Literal,
    Markup,
    /// Already diagnosed.
    Invalid,
}

/// Parses `src` into `nodes` (cleared first), reporting syntax errors into
/// `diags` (cleared first). `full` keeps trivia and punctuation tokens (a
/// lossless CST); without it only the tokens the data model needs are kept.
pub(crate) fn parse_into(
    src: &str,
    nodes: &mut Vec<Node>,
    diags: &mut Vec<Diagnostic>,
    full: bool,
) {
    nodes.clear();
    diags.clear();
    if u32::try_from(src.len()).is_err() {
        diags.push(Diagnostic {
            kind: ErrorKind::Syntax,
            code: code::SOURCE_TOO_LONG,
            span: None,
        });
        return;
    }
    let mut p = Parser {
        src,
        b: src.as_bytes(),
        pos: 0,
        nodes,
        diags,
        full,
    };
    p.message();
    debug_assert_eq!(p.pos, src.len(), "the parser consumes the whole source");
}

struct Parser<'s, 'b> {
    src: &'s str,
    b: &'s [u8],
    pos: usize,
    nodes: &'b mut Vec<Node>,
    diags: &'b mut Vec<Diagnostic>,
    full: bool,
}

// Offsets fit in u32: `parse_into` rejects longer sources.
#[allow(clippy::cast_possible_truncation)]
fn u32_of(x: usize) -> u32 {
    x as u32
}

impl Parser<'_, '_> {
    // ── input ────────────────────────────────────────────────────────────

    fn byte(&self, i: usize) -> Option<u8> {
        self.b.get(i).copied()
    }

    fn cur(&self) -> Option<u8> {
        self.byte(self.pos)
    }

    fn char_at(&self, i: usize) -> Option<char> {
        self.src.get(i..)?.chars().next()
    }

    /// The offset just past the character at `i` (the source length at or
    /// past the end).
    fn next_char_end(&self, i: usize) -> usize {
        self.char_at(i).map_or(self.b.len(), |c| i + c.len_utf8())
    }

    /// `ws` or `bidi` at `i`: `(length, is_ws)`.
    fn trivia_char(&self, i: usize) -> Option<(usize, bool)> {
        chars::trivia_at(self.b, i)
    }

    /// Skips `*(ws / bidi)` from `i`: `(end, contains ws)`. A run satisfies
    /// the required-whitespace production `s` exactly when it contains ws.
    fn scan_trivia(&self, mut i: usize) -> (usize, bool) {
        let mut ws = false;
        while let Some((n, is_ws)) = self.trivia_char(i) {
            i += n;
            ws |= is_ws;
        }
        (i, ws)
    }

    /// Skips `*bidi` from `i`.
    fn scan_bidi(&self, mut i: usize) -> usize {
        while let Some((n, false)) = self.trivia_char(i) {
            i += n;
        }
        i
    }

    fn name_start_len(&self, i: usize) -> Option<usize> {
        let b = self.byte(i)?;
        if b < 0x80 {
            return chars::is_ascii_name_start(b).then_some(1);
        }
        let c = self.char_at(i)?;
        chars::is_name_start(c).then(|| c.len_utf8())
    }

    fn name_char_len(&self, i: usize) -> Option<usize> {
        let b = self.byte(i)?;
        if b < 0x80 {
            return chars::is_ascii_name_char(b).then_some(1);
        }
        let c = self.char_at(i)?;
        chars::is_name_start(c).then(|| c.len_utf8())
    }

    fn scan_name_chars(&self, mut i: usize) -> usize {
        while let Some(n) = self.name_char_len(i) {
            i += n;
        }
        i
    }

    /// `*`, `|` or a name character: where a key can start.
    fn is_key_start(&self, i: usize) -> bool {
        matches!(self.byte(i), Some(b'*' | b'|')) || self.name_char_len(i).is_some()
    }

    fn at_double(&self, i: usize, b: u8) -> bool {
        self.byte(i) == Some(b) && self.byte(i + 1) == Some(b)
    }

    // ── output ───────────────────────────────────────────────────────────

    fn start(&mut self, kind: K) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            start: u32_of(self.pos),
            end: u32_of(self.pos),
            last: 0,
            kind,
        });
        index
    }

    fn finish(&mut self, index: usize) {
        let last = u32_of(self.nodes.len());
        let end = u32_of(self.pos);
        if let Some(n) = self.nodes.get_mut(index) {
            n.end = end;
            n.last = last;
        }
    }

    fn finish_as(&mut self, index: usize, kind: K) {
        if let Some(n) = self.nodes.get_mut(index) {
            n.kind = kind;
        }
        self.finish(index);
    }

    /// Emits a token from the position to `end` and moves there. The only
    /// way the position advances.
    fn token_to(&mut self, kind: K, end: usize) {
        debug_assert!(end >= self.pos && end <= self.b.len());
        if self.full || kind.is_semantic_token() {
            let index = u32_of(self.nodes.len());
            self.nodes.push(Node {
                start: u32_of(self.pos),
                end: u32_of(end),
                last: index + 1,
                kind,
            });
        }
        self.pos = end;
    }

    fn token(&mut self, kind: K, len: usize) {
        self.token_to(kind, self.pos + len);
    }

    fn diag(&mut self, code: u16, start: usize, end: usize) {
        self.diags.push(Diagnostic {
            kind: ErrorKind::Syntax,
            code,
            span: Some(Span {
                start: u32_of(start),
                end: u32_of(end),
            }),
        });
    }

    /// Whether the last diagnostic reaches offset `at` (so a new one there
    /// would only be a consequence of it).
    fn error_reported_at(&self, at: usize) -> bool {
        self.diags
            .last()
            .and_then(|d| d.span)
            .is_some_and(|s| s.end as usize >= at)
    }

    /// A zero-width diagnostic at the position.
    fn diag_here(&mut self, code: u16) {
        self.diag(code, self.pos, self.pos);
    }

    /// Consumes trivia already scanned up to `end` (see [`Parser::scan_trivia`]).
    fn trivia_to(&mut self, end: usize) {
        if end > self.pos {
            self.token_to(K::Trivia, end);
        }
    }

    /// Consumes `o = *(ws / bidi)`; returns whether it held whitespace.
    fn o(&mut self) -> bool {
        let (end, ws) = self.scan_trivia(self.pos);
        if end > self.pos {
            self.token_to(K::Trivia, end);
        }
        ws
    }

    /// Consumes `*bidi` (the optional marks around a name).
    fn bidi(&mut self) {
        let end = self.scan_bidi(self.pos);
        if end > self.pos {
            self.token_to(K::Trivia, end);
        }
    }

    // ── message ──────────────────────────────────────────────────────────

    fn message(&mut self) {
        let (after, _) = self.scan_trivia(0);
        let complex = match self.byte(after) {
            Some(b'.') => true,
            Some(b'{') => self.byte(after + 1) == Some(b'{'),
            _ => false,
        };
        // Room for the whole arena in one allocation (a reused arena that is
        // large enough is left alone). Plain text needs three entries; with
        // structure, a token covers at least one byte and the nodes wrapping
        // tokens are fewer than the tokens, and the trimmed arena of the
        // model path keeps about half of them.
        let len = self.b.len();
        let plain = !complex && chars::find_text_end(self.b, 0) == len;
        self.nodes.reserve(match (plain, self.full) {
            (true, _) => 3,
            (false, true) => len + 8,
            (false, false) => len / 2 + 8,
        });
        if complex {
            self.complex_message();
        } else {
            // A simple message's leading whitespace is part of its text.
            let m = self.start(K::SimpleMessage);
            self.pattern(End::Eof);
            self.finish(m);
        }
    }

    fn complex_message(&mut self) {
        let m = self.start(K::ComplexMessage);
        self.o();
        let mut has_body = false;
        loop {
            match self.cur() {
                None => break,
                Some(b'.') => {
                    let kw_end = self.scan_name_chars(self.pos + 1);
                    match self.b.get(self.pos..kw_end) {
                        Some(b".input") => self.input_declaration(),
                        Some(b".local") => self.local_declaration(),
                        Some(b".match") => {
                            self.matcher();
                            has_body = true;
                            break;
                        }
                        _ => {
                            let s = self.pos;
                            self.diag(code::UNKNOWN_KEYWORD, s, kw_end);
                            self.token_to(K::Error, kw_end);
                            self.recover_top_level();
                        }
                    }
                }
                Some(b'{') if self.byte(self.pos + 1) == Some(b'{') => {
                    self.quoted_pattern();
                    has_body = true;
                    break;
                }
                Some(_) => {
                    // Junk where the declaration before already reported an
                    // error is the same error: skip it silently.
                    let s = self.pos;
                    if !self.error_reported_at(s) {
                        let e = self.next_char_end(s);
                        self.diag(code::UNEXPECTED_CHARACTER, s, e);
                    }
                    self.recover_top_level();
                }
            }
            self.o();
        }
        if !has_body {
            self.diag_here(code::MISSING_BODY);
        }
        self.o();
        let len = self.b.len();
        if self.pos < len {
            let s = self.pos;
            self.diag(code::CONTENT_AFTER_BODY, s, len);
            self.token_to(K::Error, len);
        }
        self.finish(m);
    }

    /// Skips junk between declarations up to the next `.`, `{{` or the end
    /// (placeholders and quoted literals are skipped whole), always consuming
    /// the current character if it is junk itself.
    fn recover_top_level(&mut self) {
        let start = self.pos;
        let mut i = start;
        loop {
            match self.byte(i) {
                None | Some(b'.') => break,
                Some(b'{') if self.byte(i + 1) == Some(b'{') => break,
                Some(b'{') => i = self.skip_placeholder(i),
                Some(b'|') => i = self.skip_quoted_literal(i),
                Some(b'\\') => i = self.next_char_end(i + 1),
                Some(_) => i = self.next_char_end(i),
            }
        }
        if i > start {
            self.token_to(K::Error, i);
        }
    }

    /// From a `{` to just past its `}` (or to the end), skipping quoted
    /// literals and escapes.
    fn skip_placeholder(&self, mut i: usize) -> usize {
        i += 1;
        loop {
            match self.byte(i) {
                None => return i,
                Some(b'}') => return i + 1,
                Some(b'|') => i = self.skip_quoted_literal(i),
                Some(b'\\') => i = self.next_char_end(i + 1),
                Some(_) => i = self.next_char_end(i),
            }
        }
    }

    /// From a `|` to just past its closing `|` (or to the end).
    fn skip_quoted_literal(&self, mut i: usize) -> usize {
        i += 1;
        loop {
            match self.byte(i) {
                None => return i,
                Some(b'|') => return i + 1,
                Some(b'\\') => i = self.next_char_end(i + 1),
                Some(_) => i = self.next_char_end(i),
            }
        }
    }

    // ── declarations ─────────────────────────────────────────────────────

    /// `input o variable-expression`, at `.input`.
    fn input_declaration(&mut self) {
        let d = self.start(K::InputDeclaration);
        self.token(K::KwInput, 6);
        self.o();
        if self.cur() == Some(b'{') && self.byte(self.pos + 1) != Some(b'{') {
            let s = self.pos;
            match self.expression(Ctx::Declaration) {
                Operand::Literal | Operand::None => {
                    self.diag(code::EXPECTED_VARIABLE_EXPRESSION, s, self.pos);
                }
                Operand::Variable | Operand::Markup | Operand::Invalid => {}
            }
        } else {
            self.diag_here(code::EXPECTED_EXPRESSION);
        }
        self.finish(d);
    }

    /// `local s variable o "=" o expression`, at `.local`.
    fn local_declaration(&mut self) {
        let d = self.start(K::LocalDeclaration);
        self.token(K::KwLocal, 6);
        let ws = self.o();
        if self.cur() == Some(b'$') {
            if !ws {
                self.diag_here(code::MISSING_WHITESPACE);
            }
            if self.variable() {
                self.o();
                let equals = self.cur() == Some(b'=');
                if equals {
                    self.token(K::Equals, 1);
                    self.o();
                } else {
                    self.diag_here(code::EXPECTED_EQUALS);
                }
                if self.cur() == Some(b'{') && self.byte(self.pos + 1) != Some(b'{') {
                    self.expression(Ctx::Declaration);
                } else if equals {
                    // Without the `=` either, one diagnostic says enough.
                    self.diag_here(code::EXPECTED_EXPRESSION);
                }
            }
        } else {
            self.diag_here(code::EXPECTED_VARIABLE);
        }
        self.finish(d);
    }

    // ── bodies ───────────────────────────────────────────────────────────

    /// `"{{" pattern "}}"`, at `{{`.
    fn quoted_pattern(&mut self) {
        let q = self.start(K::QuotedPattern);
        let open = self.pos;
        self.token(K::LBrace2, 2);
        self.pattern(End::Quoted);
        if self.at_double(self.pos, b'}') {
            self.token(K::RBrace2, 2);
        } else {
            self.diag(code::UNTERMINATED_QUOTED_PATTERN, open, self.pos);
        }
        self.finish(q);
    }

    /// `match-statement s variant *(o variant)`, at `.match`.
    fn matcher(&mut self) {
        let m = self.start(K::Matcher);
        let kw = self.pos;
        self.token(K::KwMatch, 6);
        let mut selectors = 0usize;
        loop {
            let (after, ws) = self.scan_trivia(self.pos);
            if self.byte(after) != Some(b'$') {
                break;
            }
            if !ws {
                self.diag_here(code::MISSING_WHITESPACE);
            }
            self.trivia_to(after);
            self.variable();
            selectors += 1;
        }
        if selectors == 0 {
            self.diag(code::EXPECTED_SELECTOR, kw, kw + 6);
        }
        let mut variants = 0usize;
        loop {
            let (after, ws) = self.scan_trivia(self.pos);
            if self.at_double(after, b'{') {
                // A quoted pattern with no keys: diagnose, and keep it in a
                // variant so that its contents are parsed.
                self.trivia_to(after);
                let v = self.start(K::Variant);
                self.diag_here(code::EXPECTED_KEY);
                self.quoted_pattern();
                self.finish(v);
            } else if self.is_key_start(after) {
                if variants == 0 && !ws {
                    self.diag_here(code::MISSING_WHITESPACE);
                }
                self.trivia_to(after);
                self.variant();
            } else {
                break;
            }
            variants += 1;
        }
        if variants == 0 {
            self.diag_here(code::EXPECTED_VARIANT);
        }
        self.finish(m);
    }

    /// `key *(s key) o quoted-pattern`, at a key.
    fn variant(&mut self) {
        let v = self.start(K::Variant);
        self.key();
        loop {
            let (after, ws) = self.scan_trivia(self.pos);
            if !self.is_key_start(after) {
                break;
            }
            if !ws {
                self.diag_here(code::MISSING_WHITESPACE);
            }
            self.trivia_to(after);
            self.key();
        }
        self.o();
        if self.at_double(self.pos, b'{') {
            self.quoted_pattern();
        } else {
            self.diag_here(code::EXPECTED_QUOTED_PATTERN);
        }
        self.finish(v);
    }

    /// `literal / "*"`, at a key start.
    fn key(&mut self) {
        match self.cur() {
            Some(b'*') => self.token(K::Star, 1),
            Some(b'|') => self.quoted_literal(),
            _ => self.unquoted_literal(),
        }
    }

    // ── patterns ─────────────────────────────────────────────────────────

    /// `*(text-char / escaped-char / placeholder)` up to `end`.
    fn pattern(&mut self, end: End) {
        let p = self.start(K::Pattern);
        loop {
            let i = chars::find_text_end(self.b, self.pos);
            if i > self.pos {
                self.token_to(K::Text, i);
            }
            match self.cur() {
                None => break,
                Some(b'{') => {
                    self.expression(Ctx::Placeholder);
                }
                Some(b'}') => {
                    if end == End::Quoted && self.byte(self.pos + 1) == Some(b'}') {
                        break;
                    }
                    let s = self.pos;
                    self.diag(code::UNESCAPED_CLOSE_BRACE, s, s + 1);
                    self.token(K::Error, 1);
                }
                Some(b'\\') => self.escape(),
                Some(_) => self.nul(),
            }
        }
        self.finish(p);
    }

    /// At `\`, in text or a quoted literal.
    fn escape(&mut self) {
        let s = self.pos;
        if let Some(b'\\' | b'{' | b'|' | b'}') = self.byte(s + 1) {
            self.token(K::Escape, 2);
        } else {
            let e = if s + 1 < self.b.len() {
                self.next_char_end(s + 1)
            } else {
                s + 1
            };
            self.diag(code::INVALID_ESCAPE, s, e);
            self.token_to(K::Error, e);
        }
    }

    /// At U+0000.
    fn nul(&mut self) {
        let s = self.pos;
        self.diag(code::NUL_CHARACTER, s, s + 1);
        self.token(K::Error, 1);
    }

    // ── expressions and markup ───────────────────────────────────────────

    /// At `{` (not `{{`): an expression, or markup in a placeholder.
    fn expression(&mut self, ctx: Ctx) -> Operand {
        let node = self.start(K::Expression);
        let open = self.pos;
        self.token(K::LBrace, 1);
        self.o();
        let (kind, operand) = match self.cur() {
            Some(b'#') => (self.markup(open, false, ctx), Operand::Markup),
            Some(b'/') => (self.markup(open, true, ctx), Operand::Markup),
            _ => (K::Expression, self.expression_body(open)),
        };
        self.finish_as(node, kind);
        operand
    }

    /// After `{ o`: `literal [s function]`, `variable [s function]` or
    /// `function`, then `*(s attribute) o "}"`.
    fn expression_body(&mut self, open: usize) -> Operand {
        let operand = match self.cur() {
            Some(b'$') => {
                if self.variable() {
                    Operand::Variable
                } else {
                    self.recover_in_placeholder(open);
                    return Operand::Invalid;
                }
            }
            Some(b'|') => {
                self.quoted_literal();
                Operand::Literal
            }
            Some(b':') => Operand::None,
            Some(b'}') => {
                self.diag(code::EMPTY_PLACEHOLDER, open, self.pos + 1);
                self.token(K::RBrace, 1);
                return Operand::Invalid;
            }
            None => {
                self.diag(code::UNTERMINATED_PLACEHOLDER, open, self.pos);
                return Operand::Invalid;
            }
            Some(b'@') => {
                let s = self.pos;
                self.diag(code::EXPECTED_OPERAND, s, s + 1);
                self.attributes_and_close(open, false);
                return Operand::Invalid;
            }
            Some(_) if self.name_char_len(self.pos).is_some() => {
                self.unquoted_literal();
                Operand::Literal
            }
            Some(_) => {
                self.unexpected_in_placeholder(open);
                return Operand::Invalid;
            }
        };
        let (after, ws) = self.scan_trivia(self.pos);
        if self.byte(after) == Some(b':') {
            if operand != Operand::None && !ws {
                self.diag_here(code::MISSING_WHITESPACE);
            }
            self.trivia_to(after);
            if !self.function() {
                self.recover_in_placeholder(open);
                return Operand::Invalid;
            }
        }
        self.attributes_and_close(open, true);
        operand
    }

    /// `*(s attribute) o "}"`; `need_ws` is whether the first attribute needs
    /// whitespace before it (something precedes it).
    fn attributes_and_close(&mut self, open: usize, mut need_ws: bool) {
        loop {
            let (after, ws) = self.scan_trivia(self.pos);
            match self.byte(after) {
                Some(b'@') => {
                    if need_ws && !ws {
                        self.diag_here(code::MISSING_WHITESPACE);
                    }
                    self.trivia_to(after);
                    if !self.attribute() {
                        self.recover_in_placeholder(open);
                        return;
                    }
                    need_ws = true;
                }
                Some(b'}') => {
                    self.trivia_to(after);
                    self.token(K::RBrace, 1);
                    return;
                }
                None => {
                    self.trivia_to(after);
                    self.diag(code::UNTERMINATED_PLACEHOLDER, open, self.pos);
                    return;
                }
                Some(_) => {
                    self.trivia_to(after);
                    self.unexpected_in_placeholder(open);
                    return;
                }
            }
        }
    }

    /// At `#` (`close == false`) or `/`: markup up to and including `}`.
    /// Returns the node kind (open, standalone or close).
    fn markup(&mut self, open: usize, close: bool, ctx: Ctx) -> K {
        let s = self.pos;
        if ctx == Ctx::Declaration {
            self.diag(code::MARKUP_NOT_ALLOWED, s, s + 1);
        }
        let kind = if close {
            self.token(K::Slash, 1);
            K::MarkupClose
        } else {
            self.token(K::Hash, 1);
            K::MarkupOpen
        };
        if !self.identifier() {
            self.recover_in_placeholder(open);
            return kind;
        }
        if !self.options() {
            self.recover_in_placeholder(open);
            return kind;
        }
        loop {
            let (after, ws) = self.scan_trivia(self.pos);
            match self.byte(after) {
                Some(b'@') => {
                    if !ws {
                        self.diag_here(code::MISSING_WHITESPACE);
                    }
                    self.trivia_to(after);
                    if !self.attribute() {
                        self.recover_in_placeholder(open);
                        return kind;
                    }
                }
                Some(b'/') if !close && self.byte(after + 1) == Some(b'}') => {
                    self.trivia_to(after);
                    self.token(K::Slash, 1);
                    self.token(K::RBrace, 1);
                    return K::MarkupStandalone;
                }
                Some(b'}') => {
                    self.trivia_to(after);
                    self.token(K::RBrace, 1);
                    return kind;
                }
                None => {
                    self.o();
                    self.diag(code::UNTERMINATED_PLACEHOLDER, open, self.pos);
                    return kind;
                }
                Some(_) => {
                    self.o();
                    self.unexpected_in_placeholder(open);
                    return kind;
                }
            }
        }
    }

    /// Diagnoses the character at the position, then recovers.
    fn unexpected_in_placeholder(&mut self, open: usize) {
        let s = self.pos;
        let e = self.next_char_end(s);
        self.diag(code::UNEXPECTED_CHARACTER, s, e);
        self.recover_in_placeholder(open);
    }

    /// Skips to the placeholder's `}` (consumed) — or to a `{` or the end,
    /// which leaves the placeholder unterminated. Quoted literals and escapes
    /// are skipped whole.
    fn recover_in_placeholder(&mut self, open: usize) {
        let start = self.pos;
        let mut i = start;
        loop {
            match self.byte(i) {
                None | Some(b'{' | b'}') => break,
                Some(b'|') => i = self.skip_quoted_literal(i),
                Some(b'\\') => i = self.next_char_end(i + 1),
                Some(_) => i = self.next_char_end(i),
            }
        }
        if i > start {
            self.token_to(K::Error, i);
        }
        if self.cur() == Some(b'}') {
            self.token(K::RBrace, 1);
        } else {
            self.diag(code::UNTERMINATED_PLACEHOLDER, open, self.pos);
        }
    }

    /// `":" identifier *(s option)`, at `:`. `false` if it could not be
    /// parsed (diagnosed).
    fn function(&mut self) -> bool {
        let f = self.start(K::Function);
        self.token(K::Colon, 1);
        let ok = self.identifier() && self.options();
        self.finish(f);
        ok
    }

    /// `*(s option)`. `false` if an option could not be parsed (diagnosed).
    fn options(&mut self) -> bool {
        loop {
            let (after, ws) = self.scan_trivia(self.pos);
            if self.name_start_len(after).is_none() {
                return true;
            }
            if !ws {
                self.diag_here(code::MISSING_WHITESPACE);
            }
            self.trivia_to(after);
            if !self.option() {
                return false;
            }
        }
    }

    /// `identifier o "=" o (literal / variable)`. `false` if it could not be
    /// parsed (diagnosed).
    fn option(&mut self) -> bool {
        let n = self.start(K::Option);
        let mut ok = self.identifier();
        if ok {
            let (after, _) = self.scan_trivia(self.pos);
            if self.byte(after) == Some(b'=') {
                self.trivia_to(after);
                self.token(K::Equals, 1);
                self.o();
                ok = match self.cur() {
                    Some(b'$') => self.variable(),
                    Some(b'|') => {
                        self.quoted_literal();
                        true
                    }
                    _ if self.name_char_len(self.pos).is_some() => {
                        self.unquoted_literal();
                        true
                    }
                    _ => {
                        self.diag_here(code::EXPECTED_OPTION_VALUE);
                        false
                    }
                };
            } else {
                self.diag(code::EXPECTED_EQUALS, after, after);
                ok = false;
            }
        }
        self.finish(n);
        ok
    }

    /// `"@" identifier [o "=" o literal]`, at `@`. `false` if it could not be
    /// parsed (diagnosed).
    fn attribute(&mut self) -> bool {
        let a = self.start(K::Attribute);
        self.token(K::At, 1);
        let mut ok = self.identifier();
        if ok {
            let (after, _) = self.scan_trivia(self.pos);
            if self.byte(after) == Some(b'=') {
                self.trivia_to(after);
                self.token(K::Equals, 1);
                self.o();
                ok = match self.cur() {
                    Some(b'|') => {
                        self.quoted_literal();
                        true
                    }
                    _ if self.name_char_len(self.pos).is_some() => {
                        self.unquoted_literal();
                        true
                    }
                    _ => {
                        self.diag_here(code::EXPECTED_LITERAL);
                        false
                    }
                };
            }
        }
        self.finish(a);
        ok
    }

    // ── names and literals ───────────────────────────────────────────────

    /// `"$" name`, at `$`. `false` if no name follows (diagnosed).
    fn variable(&mut self) -> bool {
        let v = self.start(K::Variable);
        self.token(K::Dollar, 1);
        let ok = self.name();
        self.finish(v);
        ok
    }

    /// `[namespace ":"] name`. `false` if a name is missing (diagnosed).
    fn identifier(&mut self) -> bool {
        let id = self.start(K::Identifier);
        let mut ok = self.name();
        if ok {
            let after = self.scan_bidi(self.pos);
            if self.byte(after) == Some(b':') {
                self.bidi();
                self.token(K::Colon, 1);
                ok = self.name();
            }
        }
        self.finish(id);
        ok
    }

    /// `[bidi] name-start *name-char`; the leading marks become trivia (the
    /// trailing ones are left to the caller's whitespace). `false` if no name
    /// starts here (diagnosed).
    fn name(&mut self) -> bool {
        self.bidi();
        if let Some(n) = self.name_start_len(self.pos) {
            let end = self.scan_name_chars(self.pos + n);
            self.token_to(K::Name, end);
            true
        } else {
            let s = self.pos;
            let e = if s < self.b.len() {
                self.next_char_end(s)
            } else {
                s
            };
            self.diag(code::EXPECTED_NAME, s, e);
            false
        }
    }

    /// `1*name-char`, at a name character.
    fn unquoted_literal(&mut self) {
        let end = self.scan_name_chars(self.pos);
        self.token_to(K::UnquotedLiteral, end);
    }

    /// `"|" *(quoted-char / escaped-char) "|"`, at `|`.
    fn quoted_literal(&mut self) {
        let lit = self.start(K::QuotedLiteral);
        let open = self.pos;
        self.token(K::Pipe, 1);
        loop {
            let i = chars::find_literal_end(self.b, self.pos);
            if i > self.pos {
                self.token_to(K::LiteralText, i);
            }
            match self.cur() {
                Some(b'|') => {
                    self.token(K::Pipe, 1);
                    break;
                }
                Some(b'\\') => self.escape(),
                Some(_) => self.nul(),
                None => {
                    self.diag(code::UNTERMINATED_QUOTED_LITERAL, open, self.pos);
                    break;
                }
            }
        }
        self.finish(lit);
    }
}
