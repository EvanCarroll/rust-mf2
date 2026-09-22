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
    /// Break a long value at a space outside `{…}` once the line is this many
    /// bytes wide, with an escaped line break. `None` writes each line whole.
    pub wrap: Option<usize>,
    /// What a continuation line is indented with.
    pub indent: &'static str,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            wrap: None,
            indent: "  ",
        }
    }
}

impl Style {
    /// [`Style::default`], wrapping values at `columns` bytes.
    pub fn wrapped(columns: usize) -> Self {
        Style {
            wrap: Some(columns),
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
            if entry.comment.is_some() || !entry.meta.is_empty() {
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
    let start_col = out.len() - out.rfind('\n').map_or(0, |i| i + 1);
    let mut col = start_col;
    let mut line_has_content = false;
    let mut depth = 0usize;
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => {
                // An empty continuation line would end the value, so a line
                // break with nothing on either side is written as an escape.
                if !line_has_content || matches!(chars.peek(), None | Some('\n')) {
                    out.push_str("\\n");
                    col += 2;
                    line_has_content = true;
                } else {
                    out.push('\n');
                    out.push_str(style.indent);
                    col = style.indent.len();
                    line_has_content = false;
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
                out.push('\\');
                out.push(next);
                col += 2;
                line_has_content = true;
            }
            ' ' | '\t' if !line_has_content => {
                // Leading whitespace would be stripped with the indentation.
                out.push('\\');
                out.push(if c == ' ' { ' ' } else { 't' });
                col += 2;
                line_has_content = true;
            }
            '\t' => {
                out.push_str("\\t");
                col += 2;
                line_has_content = true;
            }
            '\r' => {
                out.push_str("\\r");
                col += 2;
                line_has_content = true;
            }
            '\u{2028}' | '\u{2029}' => {
                push_hex(out, c);
                col += 6;
                line_has_content = true;
            }
            c if c.is_control() => {
                push_hex(out, c);
                col += 4;
                line_has_content = true;
            }
            c => {
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth = depth.saturating_sub(1);
                }
                out.push(c);
                col += c.len_utf8();
                line_has_content = true;
                if c == ' '
                    && depth == 0
                    && chars.peek().is_some()
                    && let Some(wrap) = style.wrap
                    && col >= wrap
                {
                    out.push_str("\\\n");
                    out.push_str(style.indent);
                    col = style.indent.len();
                    line_has_content = false;
                }
            }
        }
    }
    Ok(())
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
