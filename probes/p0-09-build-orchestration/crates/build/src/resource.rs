//! Minimal loader for the resource container's **working grammar**
//! (plans/05-tooling.md §2): frontmatter + `@locale`, `[section]` heads,
//! `id = value` entries with indented continuation lines, `#` comments,
//! `@property` lines, and the value escapes. Comments and properties are
//! recognised and dropped (the probe does not export translator context).

use std::path::Path;

use crate::error::Error;

/// One entry: full dotted id and its MF2 source (resource escapes resolved,
/// MF2's own escapes passed through untouched).
#[derive(Debug, Clone)]
pub struct Entry {
    /// Full dotted id.
    pub id: String,
    /// MF2 message source.
    pub source: String,
    /// 1-based line of the entry.
    pub line: usize,
}

/// A parsed resource file.
#[derive(Debug, Clone)]
pub struct Resource {
    /// `@locale` from the frontmatter.
    pub locale: String,
    /// Entries in file order.
    pub entries: Vec<Entry>,
}

fn is_id_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || !c.is_ascii()
}

fn err(file: &Path, line: usize, message: impl Into<String>) -> Error {
    Error::Resource {
        file: file.to_path_buf(),
        line,
        message: message.into(),
    }
}

/// Resolves the resource-level escapes of a value (05 §2). MF2's own escapes
/// (`\\`, `\{`, `\|`, `\}`) pass through untouched; an escaped line break is
/// removed (the continuation's indentation was already stripped).
fn unescape(file: &Path, line: usize, raw: &str) -> Result<String, Error> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            if c == '\r'
                || (c.is_control() && c != '\n' && c != '\t')
                || c == '\u{2028}'
                || c == '\u{2029}'
            {
                return Err(err(file, line, "raw control character in value"));
            }
            out.push(c);
            continue;
        }
        let Some(e) = chars.next() else {
            return Err(err(file, line, "dangling `\\` at end of value"));
        };
        match e {
            '\\' | '{' | '|' | '}' => {
                out.push('\\');
                out.push(e);
            }
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            ' ' | '\t' => out.push(e),
            '\n' => {}
            'x' | 'u' | 'U' => {
                let n = match e {
                    'x' => 2,
                    'u' => 4,
                    _ => 6,
                };
                let hex: String = chars.by_ref().take(n).collect();
                let cp = u32::from_str_radix(&hex, 16)
                    .ok()
                    .filter(|_| hex.len() == n)
                    .and_then(char::from_u32)
                    .ok_or_else(|| err(file, line, format!("bad `\\{e}` escape")))?;
                out.push(cp);
            }
            other => return Err(err(file, line, format!("unknown escape `\\{other}`"))),
        }
    }
    Ok(out)
}

/// Parses one resource file.
pub fn parse(file: &Path, text: &str) -> Result<Resource, Error> {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let mut locale: Option<String> = None;
    let mut seen_front = false;
    let mut section = String::new();
    let mut entries = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let lineno = i + 1;
        i += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(err(file, lineno, "continuation line without an entry"));
        }
        if line == "---" {
            if seen_front {
                return Err(err(file, lineno, "second frontmatter separator"));
            }
            seen_front = true;
            continue;
        }
        if let Some(prop) = line.strip_prefix('@') {
            // Properties may continue like entries; skip their continuations.
            while i < lines.len() && (lines[i].starts_with(' ') || lines[i].starts_with('\t')) {
                i += 1;
            }
            let (name, value) = prop.split_once(char::is_whitespace).unwrap_or((prop, ""));
            if !seen_front && name == "locale" {
                locale = Some(value.trim().to_owned());
            }
            continue;
        }
        if let Some(head) = line.strip_prefix('[') {
            let id = head
                .strip_suffix(']')
                .ok_or_else(|| err(file, lineno, "unterminated section head"))?;
            if id.is_empty() || !id.chars().all(is_id_char) {
                return Err(err(file, lineno, format!("bad section id `{id}`")));
            }
            section = id.to_owned();
            continue;
        }
        let Some((id, first)) = line.split_once('=') else {
            return Err(err(file, lineno, "expected `id = value`"));
        };
        let id = id.trim_end();
        if id.is_empty() || id == "---" || !id.chars().all(is_id_char) {
            return Err(err(file, lineno, format!("bad message id `{id}`")));
        }
        let first = first.trim_start_matches([' ', '\t']);
        let mut raw = String::from(first);
        let mut started = !first.is_empty();
        while i < lines.len() && (lines[i].starts_with(' ') || lines[i].starts_with('\t')) {
            let cont = lines[i].trim_start_matches([' ', '\t']);
            if started {
                raw.push('\n');
            }
            raw.push_str(cont);
            started = true;
            i += 1;
        }
        let source = unescape(file, lineno, &raw)?;
        let full = if section.is_empty() {
            id.to_owned()
        } else {
            format!("{section}.{id}")
        };
        entries.push(Entry {
            id: full,
            source,
            line: lineno,
        });
    }
    let locale = locale.ok_or_else(|| err(file, 1, "missing `@locale` in the frontmatter"))?;
    Ok(Resource { locale, entries })
}

#[cfg(test)]
mod tests {
    use super::parse;
    use std::path::Path;

    #[test]
    fn grammar() {
        let src = "# c\n@locale en\n---\n\nchat-send = Send\n\n@param $count - n\nusers =\n  .input {$count :integer}\n  .match $count\n  one {{{$count} user}}\n  *   {{{$count} users}}\n\n[hotkeys]\n# x\nrelease = Release {#kbd}?{/kbd} to \\\n  close\n";
        let r = parse(Path::new("t.mf2"), src).unwrap();
        assert_eq!(r.locale, "en");
        assert_eq!(r.entries[0].id, "chat-send");
        assert_eq!(
            r.entries[1].source,
            ".input {$count :integer}\n.match $count\none {{{$count} user}}\n*   {{{$count} users}}"
        );
        assert_eq!(r.entries[2].id, "hotkeys.release");
        assert_eq!(r.entries[2].source, "Release {#kbd}?{/kbd} to close");
    }
}
