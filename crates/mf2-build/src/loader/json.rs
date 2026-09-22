//! The flat JSON loader: `{ "id": "source" }`, the shape every
//! translation-management system speaks (`plans/05-tooling.md` §2).
//!
//! The reader here is small and strict on purpose. It accepts exactly an
//! object of strings, and it keeps the byte offset of every key and every
//! value, with a [`ValueMap`] through the JSON escapes — so a message error
//! in a JSON corpus names a line and a column just as one in a `.mf2` file
//! does.

use std::path::Path;

use mf2_resource::{Segment, Span, ValueMap};

use crate::error::{Error, Result};
use crate::loader::{Loaded, Loader, Record, SourceFile};

/// `locales/<tag>.json`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlatJson;

impl Loader for FlatJson {
    fn name(&self) -> &'static str {
        "flat JSON"
    }

    fn load(&self, path: &Path) -> Result<Loaded> {
        let file = SourceFile::read(path)?;
        let entries = read(&file.text).map_err(|e| {
            let at = file.position(u32::try_from(e.at).unwrap_or(u32::MAX));
            Error::Json {
                path: path.to_path_buf(),
                line: at.line,
                column: at.column,
                message: e.message,
            }
        })?;
        let mut loaded = Loaded {
            declared: vec![None],
            ..Loaded::default()
        };
        for entry in entries {
            loaded.records.push(Record {
                id: entry.id,
                source: entry.source,
                file: 0,
                span: Span {
                    start: entry.id_span.start,
                    end: entry.value_span.end,
                },
                id_span: entry.id_span,
                value_span: entry.value_span,
                map: entry.map,
                comment: None,
                meta: Vec::new(),
            });
        }
        loaded.files.push(file);
        Ok(loaded)
    }
}

/// One `"id": "source"` pair.
#[derive(Debug)]
pub struct Pair {
    /// The id.
    pub id: String,
    /// The MF2 source.
    pub source: String,
    /// The id's string in the file, quotes included.
    pub id_span: Span,
    /// The value's string in the file, quotes included.
    pub value_span: Span,
    /// Where the value's characters came from.
    pub map: ValueMap,
}

/// Why a flat JSON file could not be read.
#[derive(Debug)]
pub struct JsonError {
    /// The byte it went wrong at.
    pub at: usize,
    /// What was expected.
    pub message: String,
}

/// Reads `{"id": "source", …}`, keeping every offset.
pub fn read(text: &str) -> std::result::Result<Vec<Pair>, JsonError> {
    let mut r = Reader {
        b: text.as_bytes(),
        text,
        pos: text.strip_prefix('\u{feff}').map_or(0, |_| 3),
    };
    let mut out = Vec::new();
    r.ws();
    r.expect(b'{', "an object of ids to messages")?;
    r.ws();
    if r.peek() == Some(b'}') {
        r.pos += 1;
        r.ws();
        return r.end(out);
    }
    loop {
        r.ws();
        let (id, id_span, _) = r.string("an id")?;
        r.ws();
        r.expect(b':', "`:` after an id")?;
        r.ws();
        let (source, value_span, map) = r.string("a message, as a string")?;
        out.push(Pair {
            id,
            source,
            id_span,
            value_span,
            map,
        });
        r.ws();
        match r.peek() {
            Some(b',') => r.pos += 1,
            Some(b'}') => {
                r.pos += 1;
                r.ws();
                return r.end(out);
            }
            _ => return Err(r.fail("`,` or `}`")),
        }
    }
}

/// Writes entries as flat JSON: keys sorted, two-space indent, one pair per
/// line (`mf2 export`).
pub fn write<'a>(entries: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut pairs: Vec<(&str, &str)> = entries.into_iter().collect();
    pairs.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let mut out = String::from("{\n");
    for (i, (id, source)) in pairs.iter().enumerate() {
        out.push_str("  ");
        push_json_string(&mut out, id);
        out.push_str(": ");
        push_json_string(&mut out, source);
        if i + 1 < pairs.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("}\n");
    out
}

/// A JSON string literal, escaping only what JSON requires.
pub fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str("\\u");
                for shift in (0..4).rev() {
                    let nibble = ((c as u32) >> (4 * shift)) & 0xF;
                    out.push(char::from_digit(nibble, 16).unwrap_or('0'));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Reader<'a> {
    text: &'a str,
    b: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn fail(&self, expected: &str) -> JsonError {
        JsonError {
            at: self.pos,
            message: match self.peek() {
                Some(_) => {
                    let rest = &self.text[self.pos..];
                    let got: String = rest.chars().take(12).collect();
                    format!("expected {expected}, found {got:?}")
                }
                None => format!("expected {expected}, found the end of the file"),
            },
        }
    }

    fn expect(&mut self, byte: u8, expected: &str) -> std::result::Result<(), JsonError> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            return Ok(());
        }
        Err(self.fail(expected))
    }

    fn end(&mut self, out: Vec<Pair>) -> std::result::Result<Vec<Pair>, JsonError> {
        if self.pos == self.b.len() {
            Ok(out)
        } else {
            Err(self.fail("the end of the file"))
        }
    }

    /// One JSON string, with its span (quotes included) and the map from its
    /// characters back to the file.
    fn string(
        &mut self,
        expected: &str,
    ) -> std::result::Result<(String, Span, ValueMap), JsonError> {
        let open = self.pos;
        self.expect(b'"', expected)?;
        let content = self.pos;
        let mut value = String::new();
        let mut segments: Vec<Segment> = Vec::new();
        let mut run = self.pos;
        let mut escaped = false;
        loop {
            let Some(byte) = self.peek() else {
                self.pos = open;
                return Err(self.fail("a closing quote"));
            };
            match byte {
                b'"' => {
                    push_run(&mut value, &mut segments, self.text, run, self.pos);
                    self.pos += 1;
                    let span = Span {
                        start: as_u32(open),
                        end: as_u32(self.pos),
                    };
                    let map = if escaped {
                        if value.is_empty() {
                            ValueMap::Empty
                        } else {
                            ValueMap::Segments(segments)
                        }
                    } else if value.is_empty() {
                        ValueMap::Empty
                    } else {
                        ValueMap::Linear {
                            base: as_u32(content),
                        }
                    };
                    return Ok((value, span, map));
                }
                b'\\' => {
                    push_run(&mut value, &mut segments, self.text, run, self.pos);
                    escaped = true;
                    let at = self.pos;
                    let c = self.escape()?;
                    push_char(&mut value, &mut segments, c, at);
                    run = self.pos;
                }
                b if b < 0x20 => {
                    return Err(JsonError {
                        at: self.pos,
                        message: format!(
                            "a control character (U+{b:04X}) must be escaped in a JSON string"
                        ),
                    });
                }
                _ => self.pos += 1,
            }
        }
    }

    /// One escape, `self.pos` at the `\`.
    fn escape(&mut self) -> std::result::Result<char, JsonError> {
        self.pos += 1;
        let Some(byte) = self.peek() else {
            return Err(self.fail("an escape"));
        };
        self.pos += 1;
        Ok(match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let high = self.hex4()?;
                match high {
                    0xD800..=0xDBFF => {
                        if self.peek() != Some(b'\\') {
                            return Err(self.fail("the low half of a surrogate pair"));
                        }
                        self.pos += 1;
                        if self.peek() != Some(b'u') {
                            return Err(self.fail("the low half of a surrogate pair"));
                        }
                        self.pos += 1;
                        let low = self.hex4()?;
                        if !(0xDC00..=0xDFFF).contains(&low) {
                            return Err(self.fail("the low half of a surrogate pair"));
                        }
                        let scalar = 0x1_0000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                        char::from_u32(scalar).ok_or_else(|| self.fail("a Unicode scalar value"))?
                    }
                    _ => char::from_u32(high).ok_or_else(|| self.fail("a Unicode scalar value"))?,
                }
            }
            _ => {
                self.pos -= 2;
                return Err(self.fail("a JSON escape"));
            }
        })
    }

    fn hex4(&mut self) -> std::result::Result<u32, JsonError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let Some(byte) = self.peek() else {
                return Err(self.fail("four hexadecimal digits"));
            };
            let Some(digit) = char::from(byte).to_digit(16) else {
                return Err(self.fail("four hexadecimal digits"));
            };
            value = value * 16 + digit;
            self.pos += 1;
        }
        Ok(value)
    }
}

fn as_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn push_run(value: &mut String, segments: &mut Vec<Segment>, text: &str, from: usize, to: usize) {
    if from >= to {
        return;
    }
    record(segments, value.len(), from, to - from);
    value.push_str(&text[from..to]);
}

fn push_char(value: &mut String, segments: &mut Vec<Segment>, c: char, at: usize) {
    record(segments, value.len(), at, c.len_utf8());
    value.push(c);
}

fn record(segments: &mut Vec<Segment>, cooked: usize, src: usize, len: usize) {
    let (cooked, src, len) = (as_u32(cooked), as_u32(src), as_u32(len));
    if let Some(last) = segments.last_mut()
        && last.cooked + last.len == cooked
        && last.src + last.len == src
    {
        last.len += len;
        return;
    }
    segments.push(Segment { cooked, src, len });
}

#[cfg(test)]
mod tests {
    use super::{read, write};

    #[test]
    fn a_flat_object_reads_with_its_offsets() {
        let text = "{\n  \"a.b\": \"hello\",\n  \"c\": \"\"\n}\n";
        let pairs = read(text).expect("reads");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].id, "a.b");
        assert_eq!(pairs[0].source, "hello");
        assert_eq!(
            &text[pairs[0].value_span.start as usize..pairs[0].value_span.end as usize],
            "\"hello\""
        );
        assert_eq!(pairs[0].map.source_offset(0), Some(12));
        assert_eq!(pairs[1].source, "");
    }

    #[test]
    fn escapes_map_back_to_where_they_were_written() {
        // Built from pieces so that no escape in this file is the one under
        // test.
        let bs = '\\';
        let text = format!("{{\"k\": \"a{bs}nb{bs}u00e9c\"}}");
        let pairs = read(&text).expect("reads");
        assert_eq!(pairs[0].source, "a\nb\u{e9}c");
        let at = |i: u32| pairs[0].map.source_offset(i).expect("mapped") as usize;
        assert_eq!(&text[at(0)..=at(0)], "a");
        assert_eq!(&text[at(1)..at(1) + 2], format!("{bs}n"));
        assert_eq!(&text[at(2)..=at(2)], "b");
        assert_eq!(&text[at(3)..at(3) + 2], format!("{bs}u"));
        assert_eq!(&text[at(5)..=at(5)], "c");
    }

    #[test]
    fn a_surrogate_pair_is_one_character() {
        let bs = '\\';
        let text = format!("{{\"k\": \"{bs}uD83D{bs}uDE00\"}}");
        let pairs = read(&text).expect("reads");
        assert_eq!(pairs[0].source, "\u{1F600}");
        let lone = format!("{{\"k\": \"{bs}uD83D\"}}");
        assert!(read(&lone).is_err(), "a lone surrogate is not a character");
    }

    #[test]
    fn anything_but_an_object_of_strings_is_refused() {
        for bad in [
            "[]",
            "{\"a\": 1}",
            "{\"a\": null}",
            "{\"a\": {\"b\": \"c\"}}",
            "{\"a\": \"b\",}",
            "{\"a\"}",
            "{\"a\": \"b\"} trailing",
            "{\"a\": \"b\n\"}",
        ] {
            assert!(read(bad).is_err(), "{bad:?} should not read");
        }
        assert!(read("{}").expect("an empty object is fine").is_empty());
        assert!(read("\u{feff}{}").is_ok(), "a BOM is fine");
    }

    #[test]
    fn what_it_writes_it_reads() {
        let out = write([("b", "two\nlines"), ("a", "say \"hi\"")]);
        assert!(out.starts_with("{\n  \"a\":"), "sorted by id:\n{out}");
        let pairs = read(&out).expect("reads");
        assert_eq!(pairs[0].source, "say \"hi\"");
        assert_eq!(pairs[1].source, "two\nlines");
    }
}
