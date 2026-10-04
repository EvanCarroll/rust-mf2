//! Loaders: a locale's files to records.
//!
//! `mf2-build` never sees a container. It asks a [`Loader`] for records —
//! id, source, spans, comment, properties, file — so a change to the W3C
//! draft, or a team that insists on JSON, reaches this module and no further.
//!
//! Two ship: [`resource::Resources`] over `mf2-resource` (`locales/<tag>/*.mf2`),
//! and [`json::FlatJson`] (`locales/<tag>.json`, `{id: source}`), which every
//! translation-management system speaks.

pub mod json;
pub mod resource;

use std::path::{Path, PathBuf};

use mf2_resource::{LineIndex, Span, ValueMap};

use crate::error::Result;

/// One file a loader read, kept so that a diagnostic can quote it.
#[derive(Debug)]
pub struct SourceFile {
    /// Where it is, as the report prints it.
    pub path: PathBuf,
    /// Its text.
    pub text: String,
    /// Its line starts.
    pub index: LineIndex,
}

impl SourceFile {
    /// A file with its line index built.
    pub fn new(path: impl Into<PathBuf>, text: String) -> SourceFile {
        let index = LineIndex::new(&text);
        SourceFile {
            path: path.into(),
            text,
            index,
        }
    }

    /// Reads a file.
    pub fn read(path: impl Into<PathBuf>) -> Result<SourceFile> {
        let path = path.into();
        let text = std::fs::read_to_string(&path)
            .map_err(|source| crate::error::Error::io(path.clone(), source))?;
        Ok(SourceFile::new(path, text))
    }

    /// The line and column of a byte offset.
    pub fn position(&self, at: u32) -> mf2_resource::Position {
        self.index.position(&self.text, at)
    }
}

/// One message, as a loader found it.
///
/// The source is the MF2 text with the container's own escapes read, so it
/// goes straight to `mf2-syntax`; [`Record::map`] takes an offset inside it
/// back to the file, which is what lets a message-level diagnostic name a
/// line and a column.
#[derive(Debug)]
pub struct Record {
    /// The full dotted id, as `tr!` writes it.
    pub id: String,
    /// The MF2 source.
    pub source: String,
    /// Which file, as an index into the loader's files.
    pub file: usize,
    /// The whole entry.
    pub span: Span,
    /// The id alone.
    pub id_span: Span,
    /// The value alone.
    pub value_span: Span,
    /// Where the source's bytes came from in the file.
    pub map: ValueMap,
    /// The comment above the entry — translator context.
    pub comment: Option<String>,
    /// The entry's properties (`@param`, `@do-not-translate`, …), and a
    /// `@do-not-translate` of its file or section, which covers it.
    pub meta: Vec<Property>,
}

impl Record {
    /// The span in the file of `start..end` inside [`Record::source`].
    pub fn source_span(&self, start: u32, end: u32) -> Span {
        self.map.source_span(start, end).unwrap_or(self.value_span)
    }

    /// Whether the entry carries `@do-not-translate`.
    pub fn do_not_translate(&self) -> bool {
        self.meta.iter().any(|p| p.name == "do-not-translate")
    }

    /// The `@param` description of `$name`, if the entry has one.
    ///
    /// A `@param` value reads `$name - description`, as the draft's example
    /// writes it.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.meta
            .iter()
            .filter(|p| p.name == "param")
            .filter_map(|p| p.value.as_deref())
            .find_map(|value| {
                let rest = value.strip_prefix('$')?;
                let (var, description) = split_param(rest);
                (var == name).then_some(description)
            })
    }
}

/// `count - How many.` → `("count", "How many.")`.
fn split_param(rest: &str) -> (&str, &str) {
    let end = rest
        .find(|c: char| !mf2_resource::is_id_char(c))
        .unwrap_or(rest.len());
    let (var, description) = rest.split_at(end);
    let description = description.trim_start();
    let description = description.strip_prefix('-').unwrap_or(description);
    (var, description.trim_start())
}

/// One `@name value` property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    /// The name, without the `@`.
    pub name: String,
    /// The value, or `None` for a bare `@name`.
    pub value: Option<String>,
}

/// A problem in a file, before any message was parsed.
#[derive(Clone, Copy, Debug)]
pub struct Problem {
    /// Which file.
    pub file: usize,
    /// Where in it.
    pub span: Span,
    /// The container's stable code (`mf2_resource::code`).
    pub code: u16,
    /// What it means.
    pub message: &'static str,
}

/// What a loader read from one locale.
#[derive(Debug, Default)]
pub struct Loaded {
    /// The files, in the order read.
    pub files: Vec<SourceFile>,
    /// The messages, in the order read.
    pub records: Vec<Record>,
    /// Container syntax errors.
    pub problems: Vec<Problem>,
    /// The `@locale` each file declared, where the container has one.
    pub declared: Vec<Option<String>>,
}

impl Loaded {
    /// The file a record came from.
    pub fn file(&self, record: &Record) -> &SourceFile {
        &self.files[record.file]
    }
}

/// Where a locale's messages come from.
///
/// One loader serves one container. `mf2-build` picks by what is on disk:
/// a directory per tag is [`resource::Resources`], a `<tag>.json` file is
/// [`json::FlatJson`].
pub trait Loader {
    /// What this container is called in a message.
    fn name(&self) -> &'static str;

    /// Reads one locale. `path` is the directory or the file the layout
    /// gives for the tag.
    fn load(&self, path: &Path) -> Result<Loaded>;
}

/// The loader for `path`: a directory of `.mf2` resources, or a flat JSON
/// file.
pub fn for_path(path: &Path) -> Box<dyn Loader> {
    if path.extension().is_some_and(|e| e == "json") {
        Box::new(json::FlatJson)
    } else {
        Box::new(resource::Resources)
    }
}

#[cfg(test)]
mod tests {
    use super::{Property, Record, split_param};
    use mf2_resource::{Span, ValueMap};

    fn record(meta: Vec<Property>) -> Record {
        Record {
            id: "x".to_owned(),
            source: String::new(),
            file: 0,
            span: Span { start: 0, end: 0 },
            id_span: Span { start: 0, end: 0 },
            value_span: Span { start: 0, end: 0 },
            map: ValueMap::Empty,
            comment: None,
            meta,
        }
    }

    #[test]
    fn a_param_property_names_its_variable() {
        assert_eq!(split_param("count - How many."), ("count", "How many."));
        assert_eq!(split_param("name"), ("name", ""));
        let r = record(vec![
            Property {
                name: "param".to_owned(),
                value: Some("$count - How many people are in the room.".to_owned()),
            },
            Property {
                name: "do-not-translate".to_owned(),
                value: None,
            },
        ]);
        assert_eq!(r.param("count"), Some("How many people are in the room."));
        assert_eq!(r.param("other"), None);
        assert!(r.do_not_translate());
    }
}
