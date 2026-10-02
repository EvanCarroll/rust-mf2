//! The XLIFF 2 document: the tree `mf2 export --format xliff` writes, and
//! what `mf2 import` reads back of one (the tooling design §6.3).
//!
//! Only the core is used — no module — so every document this writes
//! validates against `third_party/xliff/schemas/xliff_core_2.0.xsd` alone.
//! Structural elements are indented; `<source>`, `<target>`, `<data>` and
//! `<note>` are written on one line each, since their whitespace is content
//! (the root says `xml:space="preserve"`).

use std::borrow::Cow;

use quick_xml::XmlVersion;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

/// XLIFF 2's namespace (2.1 keeps 2.0's).
pub(crate) const NAMESPACE: &str = "urn:oasis:names:tc:xliff:document:2.0";

/// A whole document.
#[derive(Debug, Default)]
pub(crate) struct Doc {
    /// `srcLang`.
    pub(crate) src_lang: String,
    /// `trgLang`.
    pub(crate) trg_lang: String,
    /// One per resource file.
    pub(crate) files: Vec<File>,
}

/// A `<file>`.
#[derive(Debug, Default)]
pub(crate) struct File {
    pub(crate) id: String,
    /// The source resource's file name.
    pub(crate) original: String,
    /// `false` writes `translate="no"`.
    pub(crate) translate: bool,
    pub(crate) notes: Vec<Note>,
    pub(crate) items: Vec<Item>,
}

/// What a `<file>` or `<group>` holds.
#[derive(Debug)]
pub(crate) enum Item {
    Group(Group),
    Unit(Unit),
}

/// A `<group>`: a section, or a message with `.match`.
#[derive(Debug, Default)]
pub(crate) struct Group {
    pub(crate) id: String,
    pub(crate) name: String,
    /// `mf2:section` or `mf2:select`.
    pub(crate) kind: &'static str,
    pub(crate) translate: bool,
    pub(crate) notes: Vec<Note>,
    pub(crate) items: Vec<Item>,
}

/// A `<unit>`: a message, or one variant of one.
#[derive(Debug, Default)]
pub(crate) struct Unit {
    pub(crate) id: String,
    /// The message id, or the variant's keys; `None` when a document read
    /// back does not say.
    pub(crate) name: Option<String>,
    pub(crate) translate: bool,
    pub(crate) notes: Vec<Note>,
    /// `<originalData>`: data id and the expression's MF2 text, in order.
    pub(crate) data: Vec<(String, String)>,
    pub(crate) source: Vec<Inline>,
    /// `None` when the segment has no `<target>`.
    pub(crate) target: Option<Vec<Inline>>,
}

/// A `<note>`.
#[derive(Debug)]
pub(crate) struct Note {
    /// `comment`, `param` or `property`.
    pub(crate) category: &'static str,
    pub(crate) text: String,
}

/// Inline content of a `<source>` or `<target>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Inline {
    Text(String),
    /// `<ph>`: a placeholder or standalone markup.
    Ph(Code),
    /// `<pc>`: markup opened and closed around its content.
    Pc {
        code: Code,
        /// `dataRefEnd`.
        end: Option<String>,
        /// `dispEnd`.
        disp_end: Option<String>,
        children: Vec<Inline>,
    },
    /// `<sc>`.
    Sc {
        code: Code,
        isolated: bool,
    },
    /// `<ec>`; `code.id` is `startRef` when it is not isolated.
    Ec {
        code: Code,
        isolated: bool,
    },
}

/// What every inline code carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Code {
    pub(crate) id: String,
    /// `dataRef` (`dataRefStart` on a `<pc>`).
    pub(crate) data_ref: Option<String>,
    pub(crate) copy_of: Option<String>,
    /// `disp` (`dispStart` on a `<pc>`).
    pub(crate) disp: Option<String>,
    /// `type="fmt"`: markup rather than a placeholder.
    pub(crate) fmt: bool,
}

// ─────────────────────────────── writing ────────────────────────────────

/// The document as XML text.
pub(crate) fn write(doc: &Doc) -> String {
    let mut w = Out(Writer::new(Vec::new()));
    w.event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));
    w.newline(0);
    let root = start(
        "xliff",
        &[
            ("xmlns", NAMESPACE),
            ("version", "2.1"),
            ("srcLang", &doc.src_lang),
            ("trgLang", &doc.trg_lang),
            ("xml:space", "preserve"),
        ],
    );
    w.event(Event::Start(root));
    for file in &doc.files {
        w.newline(1);
        let mut attrs = vec![
            ("id", file.id.as_str()),
            ("original", file.original.as_str()),
            ("canResegment", "no"),
        ];
        if !file.translate {
            attrs.push(("translate", "no"));
        }
        w.event(Event::Start(start("file", &attrs)));
        w.notes(&file.notes, 2);
        w.items(&file.items, 2);
        w.newline(1);
        w.event(Event::End(BytesEnd::new("file")));
    }
    w.newline(0);
    w.event(Event::End(BytesEnd::new("xliff")));
    w.newline(0);
    // Every byte written came from a `&str` or ASCII, so this cannot fail.
    String::from_utf8(w.0.into_inner()).unwrap_or_default()
}

struct Out(Writer<Vec<u8>>);

impl Out {
    fn event(&mut self, event: Event<'_>) {
        // Writing into a `Vec` does not fail.
        let _ = self.0.write_event(event);
    }

    fn newline(&mut self, depth: usize) {
        let mut s = String::from("\n");
        for _ in 0..depth {
            s.push_str("  ");
        }
        self.event(Event::Text(BytesText::from_escaped(s)));
    }

    fn notes(&mut self, notes: &[Note], depth: usize) {
        if notes.is_empty() {
            return;
        }
        self.newline(depth);
        self.event(Event::Start(BytesStart::new("notes")));
        for note in notes {
            self.newline(depth + 1);
            self.event(Event::Start(start("note", &[("category", note.category)])));
            self.text(&note.text, false);
            self.event(Event::End(BytesEnd::new("note")));
        }
        self.newline(depth);
        self.event(Event::End(BytesEnd::new("notes")));
    }

    fn items(&mut self, items: &[Item], depth: usize) {
        for item in items {
            match item {
                Item::Group(group) => self.group(group, depth),
                Item::Unit(unit) => self.unit(unit, depth),
            }
        }
    }

    fn group(&mut self, group: &Group, depth: usize) {
        self.newline(depth);
        let mut attrs = vec![
            ("id", group.id.as_str()),
            ("name", group.name.as_str()),
            ("type", group.kind),
        ];
        if !group.translate {
            attrs.push(("translate", "no"));
        }
        self.event(Event::Start(start("group", &attrs)));
        self.notes(&group.notes, depth + 1);
        self.items(&group.items, depth + 1);
        self.newline(depth);
        self.event(Event::End(BytesEnd::new("group")));
    }

    fn unit(&mut self, unit: &Unit, depth: usize) {
        self.newline(depth);
        let mut attrs = vec![("id", unit.id.as_str())];
        if let Some(name) = &unit.name {
            attrs.push(("name", name.as_str()));
        }
        if !unit.translate {
            attrs.push(("translate", "no"));
        }
        self.event(Event::Start(start("unit", &attrs)));
        self.notes(&unit.notes, depth + 1);
        if !unit.data.is_empty() {
            self.newline(depth + 1);
            self.event(Event::Start(BytesStart::new("originalData")));
            for (id, text) in &unit.data {
                self.newline(depth + 2);
                self.event(Event::Start(start("data", &[("id", id)])));
                self.text(text, true);
                self.event(Event::End(BytesEnd::new("data")));
            }
            self.newline(depth + 1);
            self.event(Event::End(BytesEnd::new("originalData")));
        }
        self.newline(depth + 1);
        let segment = if unit.target.is_some() {
            start("segment", &[("state", "translated")])
        } else {
            BytesStart::new("segment")
        };
        self.event(Event::Start(segment));
        self.newline(depth + 2);
        self.event(Event::Start(BytesStart::new("source")));
        self.inlines(&unit.source);
        self.event(Event::End(BytesEnd::new("source")));
        if let Some(target) = &unit.target {
            self.newline(depth + 2);
            self.event(Event::Start(BytesStart::new("target")));
            self.inlines(target);
            self.event(Event::End(BytesEnd::new("target")));
        }
        self.newline(depth + 1);
        self.event(Event::End(BytesEnd::new("segment")));
        self.newline(depth);
        self.event(Event::End(BytesEnd::new("unit")));
    }

    fn inlines(&mut self, inlines: &[Inline]) {
        for inline in inlines {
            match inline {
                Inline::Text(text) => self.text(text, true),
                Inline::Ph(code) => {
                    self.event(Event::Empty(code_start("ph", code, &[])));
                }
                Inline::Pc {
                    code,
                    end,
                    disp_end,
                    children,
                } => {
                    let mut e = BytesStart::new("pc");
                    push(&mut e, "id", &code.id);
                    if let Some(r) = &code.data_ref {
                        push(&mut e, "dataRefStart", r);
                    }
                    if let Some(r) = end {
                        push(&mut e, "dataRefEnd", r);
                    }
                    if let Some(d) = &code.disp {
                        push(&mut e, "dispStart", d);
                    }
                    if let Some(d) = disp_end {
                        push(&mut e, "dispEnd", d);
                    }
                    if code.fmt {
                        push(&mut e, "type", "fmt");
                    }
                    self.event(Event::Start(e));
                    self.inlines(children);
                    self.event(Event::End(BytesEnd::new("pc")));
                }
                Inline::Sc { code, isolated } => {
                    let extra: &[(&str, &str)] = if *isolated {
                        &[("isolated", "yes")]
                    } else {
                        &[]
                    };
                    self.event(Event::Empty(code_start("sc", code, extra)));
                }
                Inline::Ec { code, isolated } => {
                    let mut e = BytesStart::new("ec");
                    push(&mut e, if *isolated { "id" } else { "startRef" }, &code.id);
                    if *isolated {
                        push(&mut e, "isolated", "yes");
                    }
                    code_attrs(&mut e, code);
                    self.event(Event::Empty(e));
                }
            }
        }
    }

    /// Character data: `<cp>` for what XML cannot carry, and a carriage
    /// return as a reference so that a parser's line-end normalization keeps
    /// it. `cp` is only allowed in content that may hold inline elements
    /// (`<source>`, `<target>`, `<data>`); a note's unrepresentable character
    /// becomes U+FFFD — a note is context, never read back.
    fn text(&mut self, text: &str, cp: bool) {
        let mut run = String::new();
        for c in text.chars() {
            if xml_char(c) {
                escape_into(&mut run, c, false);
            } else if cp {
                if !run.is_empty() {
                    self.event(Event::Text(BytesText::from_escaped(std::mem::take(
                        &mut run,
                    ))));
                }
                let hex = format!("{:04X}", u32::from(c));
                self.event(Event::Empty(start("cp", &[("hex", &hex)])));
            } else {
                run.push('\u{FFFD}');
            }
        }
        if !run.is_empty() {
            self.event(Event::Text(BytesText::from_escaped(run)));
        }
    }
}

fn code_start<'a>(name: &'a str, code: &Code, extra: &[(&str, &str)]) -> BytesStart<'a> {
    let mut e = BytesStart::new(name);
    push(&mut e, "id", &code.id);
    for (k, v) in extra {
        push(&mut e, k, v);
    }
    code_attrs(&mut e, code);
    e
}

fn code_attrs(e: &mut BytesStart<'_>, code: &Code) {
    if let Some(r) = &code.data_ref {
        push(e, "dataRef", r);
    }
    if let Some(r) = &code.copy_of {
        push(e, "copyOf", r);
    }
    if let Some(d) = &code.disp {
        push(e, "disp", d);
    }
    if code.fmt {
        push(e, "type", "fmt");
    }
}

fn start<'a>(name: &'a str, attrs: &[(&str, &str)]) -> BytesStart<'a> {
    let mut e = BytesStart::new(name);
    for (k, v) in attrs {
        push(&mut e, k, v);
    }
    e
}

/// Adds an attribute, escaped so that a parser's attribute-value
/// normalization gives back exactly `value` (tabs and line ends as
/// references). A character XML cannot carry at all becomes U+FFFD; only a
/// `disp` can hold one, and it is a display hint that is never read back.
fn push(e: &mut BytesStart<'_>, key: &str, value: &str) {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        if xml_char(c) {
            escape_into(&mut escaped, c, true);
        } else {
            escaped.push('\u{FFFD}');
        }
    }
    e.push_attribute(Attribute {
        key: QName(key),
        value: Cow::Owned(escaped),
    });
}

fn escape_into(out: &mut String, c: char, attribute: bool) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' if attribute => out.push_str("&quot;"),
        '\r' => out.push_str("&#xD;"),
        '\n' if attribute => out.push_str("&#xA;"),
        '\t' if attribute => out.push_str("&#x9;"),
        c => out.push(c),
    }
}

/// XML 1.0's `Char`.
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}')
        || c >= '\u{10000}'
}

// ─────────────────────────────── reading ────────────────────────────────

/// What was wrong with a document that could not be read at all.
#[derive(Debug)]
pub(crate) struct Malformed(pub(crate) String);

/// Whether `text` is an XML document whose root element is `xliff` — how
/// `mf2 import` tells XLIFF from flat JSON.
pub(crate) fn is_xliff(text: &str) -> bool {
    let mut reader = Reader::from_str(text);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                return e.local_name().as_ref() == "xliff";
            }
            Ok(Event::Eof) | Err(_) => return false,
            Ok(Event::Text(t)) if !t.trim().is_empty() => return false,
            Ok(_) => {}
        }
    }
}

/// Reads a document: its languages, and per file, its groups' ids and its
/// units — with their `<originalData>` and target, which is all an import
/// needs. Elements and attributes of other namespaces are skipped, as a
/// tool may add them.
pub(crate) fn read(text: &str) -> Result<Doc, Malformed> {
    let mut reader = Reader::from_str(text);
    let mut doc = Doc::default();
    let mut path: Vec<String> = Vec::new();
    let mut seen_root = false;
    loop {
        let event = reader.read_event().map_err(|e| malformed(&reader, &e))?;
        match event {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_owned();
                element(&mut doc, &path, &e, &mut seen_root)?;
                if path.is_empty() && name != "xliff" {
                    return Err(Malformed("the root element is not <xliff>".to_owned()));
                }
                if name == "target" && path.last().is_some_and(|p| p == "segment") {
                    let target = inlines(&mut reader)?;
                    append_target(&mut doc, target)?;
                    continue;
                }
                if name == "data" && path.last().is_some_and(|p| p == "originalData") {
                    let id = attr(&e, "id")?.unwrap_or_default();
                    let content = inlines(&mut reader)?;
                    let mut data = String::new();
                    for part in content {
                        match part {
                            Inline::Text(t) => data.push_str(&t),
                            _ => {
                                return Err(Malformed(
                                    "a <data> holds an element other than <cp>".to_owned(),
                                ));
                            }
                        }
                    }
                    if let Some(unit) = last_unit(&mut doc) {
                        unit.data.push((id, data));
                    }
                    continue;
                }
                path.push(name);
            }
            Event::Empty(e) => {
                element(&mut doc, &path, &e, &mut seen_root)?;
                if path.is_empty() {
                    return Err(Malformed("the root element is empty".to_owned()));
                }
                if e.local_name().as_ref() == "target" {
                    append_target(&mut doc, Vec::new())?;
                }
            }
            Event::End(_) => {
                path.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_root {
        return Err(Malformed("no <xliff> element".to_owned()));
    }
    Ok(doc)
}

fn malformed(reader: &Reader<&[u8]>, e: &quick_xml::Error) -> Malformed {
    Malformed(format!("at byte {}: {e}", reader.error_position()))
}

/// Records what a structural element says.
fn element(
    doc: &mut Doc,
    path: &[String],
    e: &BytesStart<'_>,
    seen_root: &mut bool,
) -> Result<(), Malformed> {
    let name = e.local_name();
    match name.as_ref() {
        "xliff" if path.is_empty() => {
            *seen_root = true;
            let version = attr(e, "version")?.unwrap_or_default();
            if !version.starts_with("2.") {
                return Err(Malformed(format!(
                    "version {version:?}: not an XLIFF 2 document"
                )));
            }
            doc.src_lang = attr(e, "srcLang")?.unwrap_or_default();
            doc.trg_lang = attr(e, "trgLang")?
                .ok_or_else(|| Malformed("the document has no trgLang".to_owned()))?;
        }
        "file" => doc.files.push(File {
            id: attr(e, "id")?.unwrap_or_default(),
            original: attr(e, "original")?.unwrap_or_default(),
            translate: true,
            ..File::default()
        }),
        "group" => {
            let group = Group {
                id: attr(e, "id")?.unwrap_or_default(),
                translate: true,
                ..Group::default()
            };
            file_of(doc)?.items.push(Item::Group(group));
        }
        "unit" => {
            let unit = Unit {
                id: attr(e, "id")?.unwrap_or_default(),
                name: attr(e, "name")?,
                translate: true,
                ..Unit::default()
            };
            file_of(doc)?.items.push(Item::Unit(unit));
        }
        _ => {}
    }
    Ok(())
}

fn file_of(doc: &mut Doc) -> Result<&mut File, Malformed> {
    doc.files
        .last_mut()
        .ok_or_else(|| Malformed("a <group> or <unit> outside any <file>".to_owned()))
}

fn last_unit(doc: &mut Doc) -> Option<&mut Unit> {
    match doc.files.last_mut()?.items.last_mut()? {
        Item::Unit(unit) => Some(unit),
        Item::Group(_) => None,
    }
}

/// A unit's segments are read in order; their targets, concatenated, are
/// the unit's target.
fn append_target(doc: &mut Doc, target: Vec<Inline>) -> Result<(), Malformed> {
    let unit =
        last_unit(doc).ok_or_else(|| Malformed("a <target> outside any <unit>".to_owned()))?;
    unit.target.get_or_insert_with(Vec::new).extend(target);
    Ok(())
}

/// The value of the attribute `name` (no namespace prefix), normalized as
/// XML says.
fn attr(e: &BytesStart<'_>, name: &str) -> Result<Option<String>, Malformed> {
    for a in e.attributes() {
        let a = a.map_err(|err| Malformed(err.to_string()))?;
        if a.key.as_ref() == name {
            let value = a
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|err| Malformed(err.to_string()))?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

/// Reads inline content up to the end tag of the element just opened: text, `<cp>`, the codes,
/// and the content of an `<mrk>` (an annotation a tool may add; the marker
/// itself means nothing to MF2). `<sm>` and `<em>` are dropped.
fn inlines(reader: &mut Reader<&[u8]>) -> Result<Vec<Inline>, Malformed> {
    let mut stack: Vec<(Vec<Inline>, Option<Inline>)> = vec![(Vec::new(), None)];
    loop {
        let event = reader.read_event().map_err(|e| malformed(reader, &e))?;
        let top = stack.last_mut().map(|(parts, _)| parts);
        let Some(parts) = top else {
            return Err(Malformed("unbalanced inline content".to_owned()));
        };
        match event {
            Event::Text(t) => push_text(parts, &t.xml10_content()),
            Event::CData(t) => push_text(parts, &t.xml10_content()),
            Event::GeneralRef(r) => {
                let c = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    Ok(None) => resolve_predefined_entity(&r)
                        .ok_or_else(|| Malformed(format!("unknown entity &{};", &*r)))?
                        .to_owned(),
                    Err(e) => return Err(Malformed(e.to_string())),
                };
                push_text(parts, &c);
            }
            Event::Empty(e) => {
                let part = match e.local_name().as_ref() {
                    "cp" => {
                        let hex = attr(&e, "hex")?.unwrap_or_default();
                        let c = u32::from_str_radix(&hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .ok_or_else(|| Malformed(format!("<cp hex={hex:?}>")))?;
                        push_text(parts, &c.to_string());
                        continue;
                    }
                    "ph" => Inline::Ph(code(&e, "dataRef", "disp")?),
                    "sc" => Inline::Sc {
                        code: code(&e, "dataRef", "disp")?,
                        isolated: attr(&e, "isolated")?.as_deref() == Some("yes"),
                    },
                    "ec" => {
                        let isolated = attr(&e, "isolated")?.as_deref() == Some("yes");
                        let mut c = code(&e, "dataRef", "disp")?;
                        if !isolated {
                            c.id = attr(&e, "startRef")?.unwrap_or_default();
                        }
                        Inline::Ec { code: c, isolated }
                    }
                    "pc" => Inline::Pc {
                        code: code(&e, "dataRefStart", "dispStart")?,
                        end: attr(&e, "dataRefEnd")?,
                        disp_end: attr(&e, "dispEnd")?,
                        children: Vec::new(),
                    },
                    // `<mrk/>`, `<sm/>`, `<em/>`: nothing of MF2's.
                    _ => continue,
                };
                parts.push(part);
            }
            Event::Start(e) => {
                let pending = match e.local_name().as_ref() {
                    "pc" => Some(Inline::Pc {
                        code: code(&e, "dataRefStart", "dispStart")?,
                        end: attr(&e, "dataRefEnd")?,
                        disp_end: attr(&e, "dispEnd")?,
                        children: Vec::new(),
                    }),
                    _ => None,
                };
                stack.push((Vec::new(), pending));
            }
            Event::End(_) => {
                let Some((children, pending)) = stack.pop() else {
                    return Err(Malformed("unbalanced inline content".to_owned()));
                };
                let Some((parent, _)) = stack.last_mut() else {
                    return Ok(children);
                };
                match pending {
                    Some(Inline::Pc {
                        code,
                        end,
                        disp_end,
                        ..
                    }) => parent.push(Inline::Pc {
                        code,
                        end,
                        disp_end,
                        children,
                    }),
                    // `<mrk>` and anything unknown: its content, in place.
                    _ => {
                        for child in children {
                            match child {
                                Inline::Text(t) => push_text(parent, &t),
                                other => parent.push(other),
                            }
                        }
                    }
                }
            }
            Event::Eof => return Err(Malformed("the document ends inside a segment".to_owned())),
            _ => {}
        }
    }
}

fn push_text(parts: &mut Vec<Inline>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text(last)) = parts.last_mut() {
        last.push_str(text);
    } else {
        parts.push(Inline::Text(text.to_owned()));
    }
}

fn code(e: &BytesStart<'_>, data_ref: &str, disp: &str) -> Result<Code, Malformed> {
    Ok(Code {
        id: attr(e, "id")?.unwrap_or_default(),
        data_ref: attr(e, data_ref)?,
        copy_of: attr(e, "copyOf")?,
        disp: attr(e, disp)?,
        fmt: attr(e, "type")?.as_deref() == Some("fmt"),
    })
}

#[cfg(test)]
mod tests {
    use super::{Code, Doc, File, Inline, Item, Unit, is_xliff, read, write};

    fn unit(source: Vec<Inline>, target: Option<Vec<Inline>>) -> Doc {
        let mut u = Unit {
            id: "u".to_owned(),
            name: Some("u".to_owned()),
            translate: true,
            source,
            target,
            ..Unit::default()
        };
        u.data.push(("d1".to_owned(), "{|a\u{1}\r\n|}".to_owned()));
        Doc {
            src_lang: "en".to_owned(),
            trg_lang: "pl".to_owned(),
            files: vec![File {
                id: "f1".to_owned(),
                original: "a.mf2".to_owned(),
                translate: true,
                notes: Vec::new(),
                items: vec![Item::Unit(u)],
            }],
        }
    }

    /// Text, codes and the characters XML makes hard come back as written.
    #[test]
    fn a_target_reads_back_as_written() {
        let ph = Code {
            id: "1".to_owned(),
            data_ref: Some("d1".to_owned()),
            disp: Some("{|a\u{1}\r\n|}".to_owned()),
            ..Code::default()
        };
        let target = vec![
            Inline::Text(" a\r\nb\t<&>\"\u{1}\u{FFFE} ".to_owned()),
            Inline::Ph(ph.clone()),
            Inline::Pc {
                code: Code {
                    fmt: true,
                    ..ph.clone()
                },
                end: Some("d1".to_owned()),
                disp_end: None,
                children: vec![Inline::Text("x".to_owned())],
            },
            Inline::Sc {
                code: ph.clone(),
                isolated: true,
            },
            Inline::Ec {
                code: ph,
                isolated: true,
            },
        ];
        let text = write(&unit(Vec::new(), Some(target.clone())));
        assert!(is_xliff(&text));
        let doc = read(&text).expect("well-formed");
        assert_eq!(doc.trg_lang, "pl");
        let Some(Item::Unit(u)) = doc.files[0].items.first() else {
            panic!("a unit");
        };
        assert_eq!(u.data, [("d1".to_owned(), "{|a\u{1}\r\n|}".to_owned())]);
        // `disp` cannot carry U+0001 and is not read back as data.
        let mut expected = target;
        for part in &mut expected {
            if let Inline::Ph(c) | Inline::Sc { code: c, .. } | Inline::Ec { code: c, .. } = part {
                c.disp = Some("{|a\u{FFFD}\r\n|}".to_owned());
            }
            if let Inline::Pc { code, .. } = part {
                code.disp = Some("{|a\u{FFFD}\r\n|}".to_owned());
            }
        }
        assert_eq!(u.target.as_ref(), Some(&expected));
    }

    #[test]
    fn json_and_other_roots_are_not_xliff() {
        assert!(!is_xliff("{\"a\": \"b\"}"));
        assert!(!is_xliff("<?xml version=\"1.0\"?><xlf/>"));
        assert!(is_xliff("<?xml version=\"1.0\"?>\n<xliff/>"));
    }
}
