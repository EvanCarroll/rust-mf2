//! The parser: a resource file's lines to a [`Resource`].
//!
//! The grammar is line-oriented (`plans/05-tooling.md` §2), so the parser is
//! one pass over the lines with three pieces of state: the comment and the
//! properties waiting for something to attach to, and whether the frontmatter
//! is still open. It never stops at the first error — `mf2 check` reports a
//! file's errors together.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use mf2_model::Span;

use crate::code;
use crate::diagnostic::Diagnostic;
use crate::error::{Error, Role};
use crate::model::{
    Comment, Detached, Entry, Head, Id, Meta, Resource, Section, Segment, ValueMap, is_id_char,
};

/// One line: `start..end` is its content, without the line terminator and
/// the CR of a CRLF.
#[derive(Clone, Copy, Debug)]
struct Line {
    start: usize,
    end: usize,
}

fn split_lines(src: &str) -> Vec<Line> {
    let b = src.as_bytes();
    let mut lines = Vec::new();
    let mut pos = 0;
    while pos < b.len() {
        let (end, next) = match b[pos..].iter().position(|&c| c == b'\n') {
            Some(off) => {
                let nl = pos + off;
                let end = if nl > pos && b[nl - 1] == b'\r' {
                    nl - 1
                } else {
                    nl
                };
                (end, nl + 1)
            }
            None => (b.len(), b.len()),
        };
        lines.push(Line { start: pos, end });
        pos = next;
    }
    lines
}

/// Parses `src` into a resource whose values are the MF2 source as written,
/// plus the syntax errors found.
///
/// **Attachment** (`plans/05-tooling.md` §2): a comment attaches to the next
/// frontmatter separator, section head or entry unless an empty line
/// intervenes — properties may. A property attaches to the next one of those
/// three; other properties may sit in between, comments and empty lines may
/// not. A comment an empty line cut off is kept as [`Detached`].
///
/// **Cooking**: a value loses its continuation lines' indentation, every line
/// break in it becomes one LF, and the container's escapes become their
/// characters — except `\\`, `\{`, `\|` and `\}`, which pass through to the
/// message parser untouched. [`Entry::map`] maps a cooked offset back to the
/// file.
pub fn parse(src: &str) -> (Resource<'_, Cow<'_, str>>, Vec<Diagnostic>) {
    let mut p = Parser {
        src,
        b: src.as_bytes(),
        lines: split_lines(src),
        diags: Vec::new(),
    };
    if u32::try_from(src.len()).is_err() {
        p.diags.push(Diagnostic::new(code::SOURCE_TOO_LONG, 0, 0));
        return (Resource::default(), p.diags);
    }
    let resource = p.run();
    (resource, p.diags)
}

struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    lines: Vec<Line>,
    diags: Vec<Diagnostic>,
}

/// What is waiting to be attached to the next head, entry or frontmatter.
#[derive(Default)]
struct Pending<'a> {
    comment: Option<Comment<'a>>,
    meta: Vec<Meta<'a>>,
}

impl<'a> Parser<'a> {
    fn diag(&mut self, code: u16, start: usize, end: usize) {
        self.diags
            .push(Diagnostic::new(code, clamp32(start), clamp32(end)));
    }

    fn text(&self, line: Line) -> &'a str {
        &self.src[line.start..line.end]
    }

    fn run(&mut self) -> Resource<'a, Cow<'a, str>> {
        let mut resource = Resource::<Cow<'a, str>>::default();
        let mut sections: Vec<Section<'a, Cow<'a, str>>> = alloc::vec![Section::default()];
        let mut pending = Pending::default();
        let mut in_frontmatter = true;
        let mut i = 0;

        while i < self.lines.len() {
            let line = self.lines[i];
            let text = self.text(line);
            let first = self.b.get(line.start).copied();

            // An empty line — nothing, or only spaces and tabs — detaches
            // whatever was waiting.
            if text.bytes().all(|c| c == b' ' || c == b'\t') {
                self.flush_empty(&mut pending, &mut sections);
                i += 1;
                continue;
            }

            match first {
                Some(b'#') => {
                    self.take_comment(&mut pending, line);
                    i += 1;
                    continue;
                }
                Some(b' ' | b'\t') => {
                    self.diag(code::STRAY_CONTINUATION, line.start, line.end);
                    i += 1;
                    continue;
                }
                _ => {}
            }

            if text == "---" {
                if in_frontmatter {
                    resource.comment = pending.comment.take();
                    resource.meta = core::mem::take(&mut pending.meta);
                    in_frontmatter = false;
                } else {
                    self.diag(code::UNEXPECTED_FRONTMATTER, line.start, line.end);
                }
                i += 1;
                continue;
            }

            if first == Some(b'@') {
                let (meta, next) = self.parse_property(i);
                if let Some(meta) = meta {
                    pending.meta.push(meta);
                }
                i = next;
                continue;
            }

            in_frontmatter = false;

            if first == Some(b'[') {
                if let Some(head) = self.parse_head(line, &mut pending) {
                    sections.push(Section {
                        head: Some(head),
                        ..Section::default()
                    });
                }
                i += 1;
                continue;
            }

            let (entry, next) = self.parse_entry(i, &mut pending);
            if let Some(entry) = entry
                && let Some(last) = sections.last_mut()
            {
                last.entries.push(entry);
            }
            i = next;
        }

        // A comment or a property at the end of the file attaches to nothing.
        self.flush_empty(&mut pending, &mut sections);

        // The headless first section exists only if the file put something in
        // it before its first `[section]`.
        if sections
            .first()
            .is_some_and(|s| s.head.is_none() && s.entries.is_empty() && s.detached.is_empty())
        {
            sections.remove(0);
        }
        resource.sections = sections;
        resource
    }

    /// An empty line: the waiting comment describes nothing, and a waiting
    /// property has lost what it described.
    fn flush_empty(
        &mut self,
        pending: &mut Pending<'a>,
        sections: &mut [Section<'a, Cow<'a, str>>],
    ) {
        if let Some(comment) = pending.comment.take()
            && let Some(section) = sections.last_mut()
        {
            section.detached.push(Detached {
                before: section.entries.len(),
                comment,
            });
        }
        if let (Some(first), Some(last)) = (pending.meta.first(), pending.meta.last()) {
            let (start, end) = (first.span.start as usize, last.span.end as usize);
            self.diag(code::DETACHED_PROPERTY, start, end);
            pending.meta.clear();
        }
    }

    /// A `#` line: its own comment, or one more line of the one above it.
    fn take_comment(&mut self, pending: &mut Pending<'a>, line: Line) {
        // A comment is text like any other: it may not carry raw controls,
        // which nothing could write back.
        for (i, c) in self.src[line.start..line.end].char_indices() {
            if c.is_control() {
                self.diag(
                    code::RAW_CONTROL,
                    line.start + i,
                    line.start + i + c.len_utf8(),
                );
            } else if c == '\u{2028}' || c == '\u{2029}' {
                self.diag(
                    code::RAW_LINE_SEPARATOR,
                    line.start + i,
                    line.start + i + c.len_utf8(),
                );
            }
        }
        let mut text = &self.src[line.start + 1..line.end];
        if let Some(rest) = text.strip_prefix(' ') {
            text = rest;
        }
        match &mut pending.comment {
            // Adjacent lines are one comment; a comment that properties
            // separate from an earlier one joins it, since both describe what
            // follows and an element carries one comment.
            Some(c) => {
                let joined = c.text.to_mut();
                joined.push('\n');
                joined.push_str(text);
                c.span.end = clamp32(line.end);
            }
            None => {
                pending.comment = Some(Comment {
                    text: Cow::Borrowed(text),
                    span: Span {
                        start: clamp32(line.start),
                        end: clamp32(line.end),
                    },
                });
            }
        }
    }

    /// `[` id `]`.
    fn parse_head(&mut self, line: Line, pending: &mut Pending<'a>) -> Option<Head<'a>> {
        let inner_start = line.start + 1;
        let Some(close) = self.find_unescaped(inner_start, line.end, b']') else {
            self.diag(code::UNTERMINATED_SECTION, line.start, line.end);
            return None;
        };
        let rest = &self.src[close + 1..line.end];
        if !rest.bytes().all(|c| c == b' ' || c == b'\t') {
            self.diag(code::TRAILING_AFTER_SECTION, close + 1, line.end);
        }
        let id = self.read_id(inner_start, close)?;
        Some(Head {
            id,
            comment: pending.comment.take(),
            meta: core::mem::take(&mut pending.meta),
            span: Span {
                start: clamp32(line.start),
                end: clamp32(line.end),
            },
        })
    }

    /// `id = value`, with its continuation lines.
    fn parse_entry(
        &mut self,
        i: usize,
        pending: &mut Pending<'a>,
    ) -> (Option<Entry<'a, Cow<'a, str>>>, usize) {
        let line = self.lines[i];
        let Some(eq) = self.find_unescaped(line.start, line.end, b'=') else {
            self.diag(code::EXPECTED_EQUALS, line.start, line.end);
            return (None, i + 1);
        };
        let mut id_end = eq;
        while id_end > line.start && matches!(self.b[id_end - 1], b' ' | b'\t') {
            id_end -= 1;
        }
        let id = self.read_id(line.start, id_end);
        let value_start = self.skip_blanks(eq + 1, line.end);
        let (value, value_span, map, next) = self.parse_value(i, value_start, line.end);
        let Some(id) = id else {
            pending.comment = None;
            pending.meta.clear();
            return (None, next);
        };
        let entry = Entry {
            id,
            value,
            comment: pending.comment.take(),
            meta: core::mem::take(&mut pending.meta),
            span: Span {
                start: clamp32(line.start),
                end: value_span.end,
            },
            id_span: Span {
                start: clamp32(line.start),
                end: clamp32(id_end),
            },
            value_span,
            map,
        };
        (Some(entry), next)
    }

    /// `@name`, optionally followed by a value.
    fn parse_property(&mut self, i: usize) -> (Option<Meta<'a>>, usize) {
        let line = self.lines[i];
        let name_start = line.start + 1;
        let mut name_end = name_start;
        for c in self.src[name_start..line.end].chars() {
            if !is_id_char(c) {
                break;
            }
            name_end += c.len_utf8();
        }
        if name_end == name_start {
            self.diag(code::EXPECTED_PROPERTY_NAME, line.start, line.end);
            return (None, i + 1);
        }
        let name = Cow::Borrowed(&self.src[name_start..name_end]);
        let value_start = self.skip_blanks(name_end, line.end);
        let has_inline = value_start < line.end;
        let has_continuation = self
            .lines
            .get(i + 1)
            .is_some_and(|l| self.is_continuation(*l));
        if !has_inline && !has_continuation {
            let meta = Meta {
                name,
                value: None,
                span: Span {
                    start: clamp32(line.start),
                    end: clamp32(line.end),
                },
                value_span: None,
            };
            return (Some(meta), i + 1);
        }
        let (value, value_span, _map, next) = self.parse_value(i, value_start, line.end);
        let meta = Meta {
            name,
            value: Some(value),
            span: Span {
                start: clamp32(line.start),
                end: value_span.end,
            },
            value_span: Some(value_span),
        };
        (Some(meta), next)
    }

    /// The value starting at `start` on line `i`, through its last
    /// continuation line.
    fn parse_value(
        &mut self,
        i: usize,
        start: usize,
        end: usize,
    ) -> (Cow<'a, str>, Span, ValueMap, usize) {
        let mut cook = Cooker::new(self.src);
        let mut li = i;
        let (mut s, mut e) = (start, end);
        loop {
            let join = s < e && self.cook_line(&mut cook, s, e);
            let last_end = e;
            let next = li + 1;
            match self.lines.get(next).copied() {
                Some(l) if self.is_continuation(l) => {
                    if !join && !cook.is_empty() {
                        cook.push_char('\n', self.lines[li].end);
                    }
                    li = next;
                    s = self.skip_blanks(l.start, l.end);
                    e = l.end;
                }
                _ => {
                    if join {
                        self.diag(
                            code::DANGLING_LINE_BREAK,
                            last_end.saturating_sub(1),
                            last_end,
                        );
                    }
                    let (value, map) = cook.finish();
                    let span = Span {
                        start: clamp32(start),
                        end: clamp32(last_end),
                    };
                    return (value, span, map, next);
                }
            }
        }
    }

    /// Cooks one value line into `cook`; `true` if it ended in an escaped
    /// line break.
    fn cook_line(&mut self, cook: &mut Cooker<'a>, start: usize, end: usize) -> bool {
        let mut run = start;
        let mut i = start;
        while i < end {
            let b = self.b[i];
            if b == b'\\' {
                cook.push_src(run, i);
                if i + 1 == end {
                    return true;
                }
                match decode_escape(&self.src[i + 1..end]) {
                    Escape::Pass(n) => {
                        cook.push_src(i, i + 1 + n);
                        i += 1 + n;
                    }
                    Escape::Char(c, n) => {
                        cook.push_char(c, i);
                        i += 1 + n;
                    }
                    Escape::Bad(n, why) => {
                        self.diag(why, i, i + 1 + n);
                        // Keep the `\` and read on from the next character.
                        cook.push_src(i, i + 1);
                        i += 1;
                    }
                }
                run = i;
            } else if b < 0x20 || b == 0x7F {
                self.diag(code::RAW_CONTROL, i, i + 1);
                i += 1;
            } else if b == 0xE2
                && self.b.get(i + 1) == Some(&0x80)
                && matches!(self.b.get(i + 2), Some(0xA8 | 0xA9))
            {
                self.diag(code::RAW_LINE_SEPARATOR, i, i + 3);
                i += 3;
            } else {
                i += 1;
            }
        }
        cook.push_src(run, end);
        false
    }

    /// The id written in `start..end`, with its escapes read and its
    /// unescaped symbols reported.
    fn read_id(&mut self, start: usize, end: usize) -> Option<Id<'a>> {
        let text = &self.src[start..end];
        let mut chars = text.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => {
                    if let Some((j, escaped)) = chars.next()
                        && escaped.is_control()
                    {
                        self.diag(code::RAW_CONTROL, start + j, start + j + escaped.len_utf8());
                    }
                }
                '.' => {}
                c if is_id_char(c) => {}
                c if c.is_control() => {
                    self.diag(code::RAW_CONTROL, start + i, start + i + c.len_utf8());
                }
                c => self.diag(code::UNESCAPED_ID_CHAR, start + i, start + i + c.len_utf8()),
            }
        }
        if text == "---" {
            self.diag(code::ID_LIKE_FRONTMATTER, start, end);
        }
        match Id::read(text) {
            Ok(id) => Some(id),
            Err(Error::Empty(Role::Id)) => {
                self.diag(code::EMPTY_ID, start, end);
                None
            }
            Err(_) => {
                self.diag(code::EMPTY_ID_PART, start, end);
                None
            }
        }
    }

    /// The first `needle` in `from..to` that no `\` escapes.
    fn find_unescaped(&self, from: usize, to: usize, needle: u8) -> Option<usize> {
        let mut i = from;
        while i < to {
            match self.b[i] {
                b'\\' => i += 2,
                c if c == needle => return Some(i),
                _ => i += 1,
            }
        }
        None
    }

    fn skip_blanks(&self, mut from: usize, to: usize) -> usize {
        while from < to && matches!(self.b[from], b' ' | b'\t') {
            from += 1;
        }
        from
    }

    /// Whether `line` continues the value above it: it is indented and holds
    /// something other than blanks.
    fn is_continuation(&self, line: Line) -> bool {
        line.start < line.end
            && matches!(self.b[line.start], b' ' | b'\t')
            && self.skip_blanks(line.start, line.end) < line.end
    }
}

fn clamp32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// One container escape.
#[derive(Clone, Copy)]
enum Escape {
    /// `\\`, `\{`, `\|`, `\}`: both characters go to the message parser.
    Pass(usize),
    /// The escape stands for this character.
    Char(char, usize),
    /// Not an escape: how many bytes it spans, and why.
    Bad(usize, u16),
}

/// Reads the escape that follows a `\`. `rest` starts at the byte after it
/// and never crosses a line break.
fn decode_escape(rest: &str) -> Escape {
    let Some(c) = rest.chars().next() else {
        return Escape::Bad(0, code::INVALID_ESCAPE);
    };
    match c {
        // MF2's own escapes are the message parser's business, not ours.
        '\\' | '{' | '|' | '}' => Escape::Pass(1),
        'n' => Escape::Char('\n', 1),
        'r' => Escape::Char('\r', 1),
        't' => Escape::Char('\t', 1),
        // An escaped space or tab is how a value line keeps leading
        // whitespace that the indentation rule would otherwise strip.
        ' ' | '\t' => Escape::Char(c, 1),
        'x' => hex_escape(rest, 2),
        'u' => hex_escape(rest, 4),
        'U' => hex_escape(rest, 6),
        _ => Escape::Bad(0, code::INVALID_ESCAPE),
    }
}

/// `\xHH`, `\uHHHH`, `\UHHHHHH`. `rest` starts at the `x`, `u` or `U`.
fn hex_escape(rest: &str, digits: usize) -> Escape {
    let hex = &rest.as_bytes()[1..];
    if hex.len() < digits {
        return Escape::Bad(1 + hex.len(), code::BAD_HEX_ESCAPE);
    }
    let mut value = 0u32;
    for (i, &d) in hex.iter().take(digits).enumerate() {
        let Some(v) = char::from(d).to_digit(16) else {
            return Escape::Bad(1 + i, code::BAD_HEX_ESCAPE);
        };
        value = value * 16 + v;
    }
    match char::from_u32(value) {
        Some(c) => Escape::Char(c, 1 + digits),
        None => Escape::Bad(1 + digits, code::BAD_HEX_ESCAPE),
    }
}

/// Builds a cooked value and the map back to the file.
///
/// A value that is one run of the source — no escapes, no continuation lines,
/// which is the common case — is borrowed, and its map is
/// [`ValueMap::Linear`].
struct Cooker<'a> {
    src: &'a str,
    buf: Buf,
    segments: Vec<Segment>,
    len: u32,
}

enum Buf {
    Empty,
    /// One run of the source.
    One(usize, usize),
    Owned(String),
}

impl<'a> Cooker<'a> {
    fn new(src: &'a str) -> Self {
        Cooker {
            src,
            buf: Buf::Empty,
            segments: Vec::new(),
            len: 0,
        }
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends `src[start..end]` as itself.
    fn push_src(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        self.record(start, clamp32(end - start));
        self.buf = match core::mem::replace(&mut self.buf, Buf::Empty) {
            Buf::Empty => Buf::One(start, end),
            Buf::One(s, e) if e == start => Buf::One(s, end),
            Buf::One(s, e) => {
                let mut owned = String::with_capacity((e - s) + (end - start));
                owned.push_str(&self.src[s..e]);
                owned.push_str(&self.src[start..end]);
                Buf::Owned(owned)
            }
            Buf::Owned(mut owned) => {
                owned.push_str(&self.src[start..end]);
                Buf::Owned(owned)
            }
        };
        self.len += clamp32(end - start);
    }

    /// Appends `c`, which the source wrote differently at `src_at`: an
    /// escape, or the line break a continuation line stands for.
    fn push_char(&mut self, c: char, src_at: usize) {
        self.record(src_at, clamp32(c.len_utf8()));
        self.buf = match core::mem::replace(&mut self.buf, Buf::Empty) {
            Buf::Empty => {
                let mut owned = String::new();
                owned.push(c);
                Buf::Owned(owned)
            }
            Buf::One(s, e) => {
                let mut owned = String::from(&self.src[s..e]);
                owned.push(c);
                Buf::Owned(owned)
            }
            Buf::Owned(mut owned) => {
                owned.push(c);
                Buf::Owned(owned)
            }
        };
        self.len += clamp32(c.len_utf8());
    }

    /// Extends the last segment if this run follows it in both the cooked
    /// value and the source; otherwise starts a new one.
    fn record(&mut self, src: usize, len: u32) {
        let src = clamp32(src);
        if let Some(last) = self.segments.last_mut()
            && last.cooked + last.len == self.len
            && last.src + last.len == src
        {
            last.len += len;
            return;
        }
        self.segments.push(Segment {
            cooked: self.len,
            src,
            len,
        });
    }

    fn finish(self) -> (Cow<'a, str>, ValueMap) {
        match self.buf {
            Buf::Empty => (Cow::Borrowed(""), ValueMap::Empty),
            Buf::One(s, e) => (
                Cow::Borrowed(&self.src[s..e]),
                ValueMap::Linear { base: clamp32(s) },
            ),
            Buf::Owned(owned) => (Cow::Owned(owned), ValueMap::Segments(self.segments)),
        }
    }
}
