//! The serializer: a [`Resource`] back to canonical resource source.
//!
//! The output is laid out the way `mf2 fmt` writes a file — one blank line
//! before a section head and before any entry that carries a comment or a
//! property — and reads back as the same model, so
//! `parse(&serialize(&r)) == r` (equality ignores spans; see
//! [`crate::model`]).

use alloc::string::String;

use crate::error::{Error, Role};
use crate::model::{Comment, Meta, Resource};

/// How [`serialize_with`] lays a file out.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// Wrap a value longer than this many bytes; `None` writes every value
    /// whole, however long. A short value is left on one line even when it
    /// passes [`Style::wrap_at`], so that wrapping marks the values that are
    /// genuinely long.
    pub wrap_over: Option<usize>,
    /// Where to break a value that is wrapped: after the first space outside
    /// `{…}` at or past this column, with an escaped line break.
    pub wrap_at: usize,
    /// What a continuation line is indented with.
    pub indent: &'static str,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            wrap_over: None,
            wrap_at: 76,
            indent: "  ",
        }
    }
}

impl Style {
    /// [`Style::default`], wrapping a value longer than `over` bytes at
    /// column `at`.
    pub fn wrapped(over: usize, at: usize) -> Self {
        Style {
            wrap_over: Some(over),
            wrap_at: at,
            ..Style::default()
        }
    }
}

/// The resource as a file, in the default style.
pub fn serialize<V: AsRef<str>>(resource: &Resource<'_, V>) -> Result<String, Error> {
    serialize_with(resource, &Style::default())
}

/// The resource as a file.
pub fn serialize_with<V: AsRef<str>>(
    resource: &Resource<'_, V>,
    style: &Style,
) -> Result<String, Error> {
    let mut out = String::new();
    if resource.comment.is_some() || !resource.meta.is_empty() {
        write_comment(&mut out, resource.comment.as_ref())?;
        for meta in &resource.meta {
            write_meta(&mut out, meta, style)?;
        }
        out.push_str("---\n");
    }
    for section in &resource.sections {
        if let Some(head) = &section.head {
            blank_line(&mut out);
            write_comment(&mut out, head.comment.as_ref())?;
            for meta in &head.meta {
                write_meta(&mut out, meta, style)?;
            }
            out.push('[');
            head.id.write_canonical(&mut out)?;
            out.push_str("]\n");
        }
        for (i, entry) in section.entries.iter().enumerate() {
            for detached in section.detached.iter().filter(|d| d.before == i) {
                blank_line(&mut out);
                write_comment(&mut out, Some(&detached.comment))?;
                out.push('\n');
            }
            // A blank line sets a comment off from what came before it; an
            // entry that only carries properties follows straight on, since
            // the properties are part of it.
            if entry.comment.is_some() {
                blank_line(&mut out);
            }
            write_comment(&mut out, entry.comment.as_ref())?;
            for meta in &entry.meta {
                write_meta(&mut out, meta, style)?;
            }
            entry.id.write_canonical(&mut out)?;
            let value = entry.value.as_ref();
            if value.is_empty() {
                out.push_str(" =\n");
            } else if starts_on_its_own_line(value) {
                // A value with line breaks in it reads better with every
                // line at the same indent, so it starts below the `=`. The
                // first line of such a value contributes nothing, which is
                // why a value that *begins* with a line break cannot use
                // this form.
                out.push_str(" =\n");
                out.push_str(style.indent);
                write_value(&mut out, value, style)?;
                out.push('\n');
            } else {
                out.push_str(" = ");
                write_value(&mut out, value, style)?;
                out.push('\n');
            }
        }
        let past = section.entries.len();
        for detached in section.detached.iter().filter(|d| d.before >= past) {
            blank_line(&mut out);
            write_comment(&mut out, Some(&detached.comment))?;
            out.push('\n');
        }
    }
    Ok(out)
}

/// Whether a value is laid out under its `=` rather than beside it.
fn starts_on_its_own_line(value: &str) -> bool {
    value.contains('\n') && !value.starts_with('\n')
}

/// A blank line, unless the output is empty or already ends in one.
fn blank_line(out: &mut String) {
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
}

fn write_comment(out: &mut String, comment: Option<&Comment<'_>>) -> Result<(), Error> {
    let Some(comment) = comment else {
        return Ok(());
    };
    for line in comment.text.split('\n') {
        if let Some(c) = line
            .chars()
            .find(|c| c.is_control() || *c == '\u{2028}' || *c == '\u{2029}')
        {
            return Err(Error::Unrepresentable(Role::Comment, c));
        }
        if line.is_empty() {
            out.push_str("#\n");
        } else {
            out.push_str("# ");
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(())
}

fn write_meta(out: &mut String, meta: &Meta<'_>, style: &Style) -> Result<(), Error> {
    if meta.name.is_empty() {
        return Err(Error::Empty(Role::PropertyName));
    }
    if let Some(c) = meta
        .name
        .chars()
        .find(|c| !crate::model::is_id_char(*c) && *c != '.')
    {
        return Err(Error::Unrepresentable(Role::PropertyName, c));
    }
    out.push('@');
    out.push_str(&meta.name);
    // A property with an empty value reads back as one with no value at all,
    // so the two are written the same way.
    match &meta.value {
        Some(value) if !value.is_empty() => {
            out.push(' ');
            write_value(out, value, style)?;
        }
        _ => {}
    }
    out.push('\n');
    Ok(())
}

/// Writes a value, escaping what the container would otherwise read as
/// structure and breaking it into continuation lines.
fn write_value(out: &mut String, value: &str, style: &Style) -> Result<(), Error> {
    // Only a long value is wrapped, and only one that is a single line: the
    // threshold is on the value, not on the line, so that adding a character
    // to an id never rewraps a paragraph — and a value that already has line
    // breaks has a structure of its own (a `.match` and its variants), which
    // wrapping would cut across.
    let wrap = style
        .wrap_over
        .filter(|over| value.len() > *over && !value.contains('\n'))
        .map(|_| style.wrap_at);
    let mut lines = Lines {
        col: out.len() - out.rfind('\n').map_or(0, |i| i + 1),
        line_has_content: false,
        indent: style.indent,
        wrap,
        out,
    };
    // Text is emitted a word at a time — everything up to and including a
    // space outside `{…}` — so that a word is never split and a line breaks
    // before the word that would pass the width, as a filled paragraph does.
    let mut word = String::new();
    let mut depth = 0usize;
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        // Leading whitespace is stripped with a continuation's indentation,
        // so the first character of a line escapes itself.
        let at_line_start = !lines.line_has_content && word.is_empty();
        match c {
            '\n' => {
                // An empty continuation line would end the value, so a line
                // break with nothing on either side is written as an escape.
                if at_line_start || matches!(chars.peek(), None | Some('\n')) {
                    word.push_str("\\n");
                } else {
                    lines.word(&word);
                    word.clear();
                    lines.hard_break();
                }
            }
            '\\' => {
                // Only MF2's four escapes pass through; every other `\` in a
                // value was written by the container itself.
                let Some(next) = chars.next() else {
                    return Err(Error::TrailingBackslash);
                };
                if !matches!(next, '\\' | '{' | '|' | '}') {
                    return Err(Error::LoneBackslash(next));
                }
                word.push('\\');
                word.push(next);
            }
            ' ' if at_line_start => word.push_str("\\ "),
            '\t' if at_line_start => word.push_str("\\t"),
            '\t' => word.push_str("\\t"),
            '\r' => word.push_str("\\r"),
            '\u{2028}' | '\u{2029}' => push_hex(&mut word, c),
            c if c.is_control() => push_hex(&mut word, c),
            c => {
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth = depth.saturating_sub(1);
                }
                word.push(c);
                // A space outside a placeholder ends a word — unless the next
                // character is one too, which would leave a continuation
                // line starting with whitespace.
                if c == ' ' && depth == 0 && !matches!(chars.peek(), None | Some(' ')) {
                    lines.word(&word);
                    word.clear();
                }
            }
        }
    }
    lines.word(&word);
    Ok(())
}

/// Lays escaped words out in lines, breaking with `\` before a word that
/// would pass the width.
struct Lines<'o> {
    out: &'o mut String,
    col: usize,
    line_has_content: bool,
    indent: &'static str,
    wrap: Option<usize>,
}

impl Lines<'_> {
    /// Emits one word, breaking the line first if it would not fit.
    fn word(&mut self, word: &str) {
        if word.is_empty() {
            return;
        }
        if let Some(wrap) = self.wrap
            && self.line_has_content
            && self.col + word.len() > wrap
        {
            self.out.push_str("\\\n");
            self.out.push_str(self.indent);
            self.col = self.indent.len();
        }
        self.out.push_str(word);
        self.col += word.len();
        self.line_has_content = true;
    }

    /// A line break that is part of the value: a real one, which reads back
    /// as one LF.
    fn hard_break(&mut self) {
        self.out.push('\n');
        self.out.push_str(self.indent);
        self.col = self.indent.len();
        self.line_has_content = false;
    }
}

/// `\xHH` for a control character, `\uHHHH` for U+2028 and U+2029.
fn push_hex(out: &mut String, c: char) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let u = c as u32;
    let (prefix, digits) = if u < 0x100 { ("\\x", 2) } else { ("\\u", 4) };
    out.push_str(prefix);
    for i in (0..digits).rev() {
        let nibble = (u >> (4 * i)) & 0xF;
        out.push(char::from(HEX[nibble as usize]));
    }
}
