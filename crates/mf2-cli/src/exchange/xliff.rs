//! `mf2 export --format xliff` and `mf2 import` of an XLIFF 2 document
//! (the tooling design §6.3, Phase 8 A6).
//!
//! Export writes one document per target locale: a `<file>` per source
//! resource, a `<group>` per section, a `<unit>` per message — or, for a
//! message that selects, a `<group>` of one unit per variant the target
//! language needs ([`variants`]). Text is text; every expression and every
//! markup is an inline code whose `<data>` is its MF2 text, so a translation
//! tool protects it.
//!
//! Import builds the same document in memory from the tree as it stands
//! and holds the one it reads against it: an id, a data or a variant the
//! export would not write is refused, per unit, with a code. What is left
//! is read back into patterns, and a message is rewritten only when its
//! data model changed — so an export imported back changes nothing.

mod variants;
pub(crate) mod xml;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mf2_build::loader::{json, resource};
use mf2_model::{
    Key, Markup, MarkupKind, Message, Pattern, PatternMessage, PatternPart, SelectMessage, Variant,
};
use mf2_resource::{Entry, Head, Id, Meta, Resource, Section, Span, ValueMap, serialize_with};

use self::variants::{offers, same_keys};
use self::xml::{Code, Doc, File, Group, Inline, Item, Note, Unit};
use super::Rewrite;
use crate::error::{Error, Result, read};

// ─────────────────────────────── a locale ───────────────────────────────

/// One locale's files, read.
struct Locale {
    tag: String,
    /// A flat JSON locale: one file, no sections.
    json: bool,
    files: Vec<LocaleFile>,
}

struct LocaleFile {
    path: PathBuf,
    /// The file name, `<name>.mf2` or `<tag>.json`.
    name: String,
    translate: bool,
    notes: Vec<Note>,
    sections: Vec<LocaleSection>,
}

struct LocaleSection {
    /// The head's id parts, `None` before the first head.
    head: Option<Vec<String>>,
    translate: bool,
    notes: Vec<Note>,
    entries: Vec<LocaleEntry>,
}

struct LocaleEntry {
    /// The full id, as `tr!` writes it.
    id: String,
    /// The entry's own id parts, under its section.
    own: Vec<String>,
    source: String,
    translate: bool,
    notes: Vec<Note>,
}

impl Locale {
    fn read(path: &Path, tag: &str) -> Result<Locale> {
        if path.is_file() {
            let text = read(path)?;
            let pairs = json::read(&text)
                .map_err(|e| Error::Usage(format!("{}: {}", path.display(), e.message)))?;
            let entries = pairs
                .into_iter()
                .map(|p| LocaleEntry {
                    own: vec![p.id.clone()],
                    id: p.id,
                    source: p.source,
                    translate: true,
                    notes: Vec::new(),
                })
                .collect();
            return Ok(Locale {
                tag: tag.to_owned(),
                json: true,
                files: vec![LocaleFile {
                    path: path.to_path_buf(),
                    name: file_name(path),
                    translate: true,
                    notes: Vec::new(),
                    sections: vec![LocaleSection {
                        head: None,
                        translate: true,
                        notes: Vec::new(),
                        entries,
                    }],
                }],
            });
        }
        let loaded = mf2_build::loader::for_path(path).load(path)?;
        let mut files = Vec::with_capacity(loaded.files.len());
        for file in &loaded.files {
            let (resource, diagnostics) = mf2_resource::parse(&file.text);
            if !diagnostics.is_empty() {
                return Err(syntax_error(&file.path));
            }
            let translate = !has_dnt(&resource.meta);
            let mut notes = Vec::new();
            if let Some(c) = &resource.comment {
                notes.push(comment(&c.text));
            }
            notes.extend(properties(&resource.meta, true));
            let mut sections = Vec::with_capacity(resource.sections.len());
            for section in &resource.sections {
                let (head, s_translate, s_notes) = match &section.head {
                    Some(h) => (
                        Some(parts(&h.id)),
                        !has_dnt(&h.meta),
                        h.comment
                            .iter()
                            .map(|c| comment(&c.text))
                            .chain(properties(&h.meta, false))
                            .collect(),
                    ),
                    None => (None, true, Vec::new()),
                };
                let entries = section
                    .entries
                    .iter()
                    .map(|e| {
                        let id = match &section.head {
                            Some(h) => h.id.join(&e.id).to_string(),
                            None => e.id.to_string(),
                        };
                        LocaleEntry {
                            id,
                            own: parts(&e.id),
                            source: e.value.to_string(),
                            translate: !has_dnt(&e.meta),
                            notes: e
                                .comment
                                .iter()
                                .map(|c| comment(&c.text))
                                .chain(properties(&e.meta, false))
                                .collect(),
                        }
                    })
                    .collect();
                sections.push(LocaleSection {
                    head,
                    translate: s_translate,
                    notes: s_notes,
                    entries,
                });
            }
            files.push(LocaleFile {
                path: file.path.clone(),
                name: file_name(&file.path),
                translate,
                notes,
                sections,
            });
        }
        Ok(Locale {
            tag: tag.to_owned(),
            json: false,
            files,
        })
    }

    fn entries(&self) -> impl Iterator<Item = &LocaleEntry> {
        self.files
            .iter()
            .flat_map(|f| f.sections.iter().flat_map(|s| s.entries.iter()))
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn syntax_error(path: &Path) -> Error {
    Error::Usage(format!(
        "{}: has a syntax error; run `mf2 check` first",
        path.display()
    ))
}

fn parts(id: &Id<'_>) -> Vec<String> {
    id.parts().iter().map(ToString::to_string).collect()
}

fn has_dnt(meta: &[Meta<'_>]) -> bool {
    meta.iter().any(|m| m.name == "do-not-translate")
}

fn comment(text: &str) -> Note {
    Note {
        category: "comment",
        text: text.to_owned(),
    }
}

/// `@param` as a `param` note, as written; every other property but
/// `@do-not-translate` (an attribute) and the frontmatter's `@locale` (the
/// document's languages) as a `property` note.
fn properties<'m>(meta: &'m [Meta<'_>], frontmatter: bool) -> impl Iterator<Item = Note> + 'm {
    meta.iter()
        .filter(move |m| m.name != "do-not-translate" && !(frontmatter && m.name == "locale"))
        .map(|m| {
            if m.name == "param" {
                Note {
                    category: "param",
                    text: m.value.as_deref().unwrap_or_default().to_owned(),
                }
            } else {
                let mut text = format!("@{}", m.name);
                if let Some(v) = &m.value {
                    text.push(' ');
                    text.push_str(v);
                }
                Note {
                    category: "property",
                    text,
                }
            }
        })
}

fn parse(id: &str, source: &str, path: &Path) -> Result<Message<'static>> {
    mf2_syntax::parse_model(source)
        .message
        .map(Message::into_owned)
        .ok_or_else(|| {
            Error::Usage(format!(
                "{}: message {id} has an error; run `mf2 check` first",
                path.display()
            ))
        })
}

// ─────────────────────────────── ids ────────────────────────────────────

/// An XLIFF id for a message or section id: itself when it is an ASCII
/// `NMTOKEN` without `:` (every id of the reference workload), else `x:`
/// and its UTF-8 in lowercase hex — `:` never occurs in the first form, so
/// the two cannot meet.
pub(crate) fn xliff_id(id: &str) -> String {
    if !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return id.to_owned();
    }
    let mut out = String::from("x:");
    for b in id.bytes() {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// A key as MF2 writes it: `*`, or the literal quoted as needed.
fn key_text(key: &Key<'_>) -> Result<String> {
    let message = Message::Select(SelectMessage {
        declarations: Vec::new(),
        selectors: vec![mf2_model::VariableRef { name: "k".into() }],
        variants: vec![Variant {
            keys: vec![key.clone()],
            value: Pattern::new(),
        }],
    });
    let text = mf2_syntax::serialize(&message).map_err(|e| Error::Usage(e.to_string()))?;
    Ok(text
        .strip_prefix(".match $k\n")
        .and_then(|t| t.strip_suffix(" {{}}"))
        .unwrap_or(&text)
        .to_owned())
}

fn keys_text(keys: &[Key<'_>]) -> Result<String> {
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        out.push(key_text(k)?);
    }
    Ok(out.join(" "))
}

/// One expression or markup as `mf2-syntax`'s serializer writes it.
fn part_text(part: &PatternPart<'_>) -> Result<String> {
    let mut pattern = Pattern::new();
    pattern.push(part.clone());
    let message = Message::Pattern(PatternMessage {
        declarations: Vec::new(),
        pattern,
    });
    mf2_syntax::serialize(&message).map_err(|e| Error::Usage(e.to_string()))
}

// ─────────────────────────────── export ─────────────────────────────────

/// A unit as the export wrote it, for the import to hold a document against.
struct UnitInfo {
    /// The message's full id.
    message: String,
    /// The variant's keys; `None` for a message's only unit.
    keys: Option<Vec<Key<'static>>>,
    name: String,
    source: Pattern<'static>,
    /// `false` when the unit, or the group or file it is in, says
    /// `translate="no"`.
    translate: bool,
    data: Vec<(String, String)>,
    /// Every code id of the source and the target → its data ids (two for a
    /// `<pc>`), for a copied code (`copyOf`).
    codes: HashMap<String, Vec<String>>,
}

/// A message as the export saw it.
struct MessageInfo {
    source: Message<'static>,
    target: Option<Message<'static>>,
    /// Where the source has it: its file, section and own id, and the ids of
    /// the entries before it in that section, nearest last — where a new
    /// translation goes.
    file: String,
    head: Option<Vec<String>>,
    own: Vec<String>,
    before: Vec<String>,
}

/// The document an export writes, and what the import needs to know of it.
struct Built {
    doc: Doc,
    files: BTreeSet<String>,
    groups: BTreeSet<(String, String)>,
    units: HashMap<(String, String), UnitInfo>,
    messages: BTreeMap<String, MessageInfo>,
    /// Message ids in source order.
    order: Vec<String>,
}

fn build(source: &Locale, target: &Locale) -> Result<Built> {
    let mut targets: HashMap<&str, Message<'static>> = HashMap::new();
    for file in &target.files {
        for section in &file.sections {
            for e in &section.entries {
                targets.insert(&e.id, parse(&e.id, &e.source, &file.path)?);
            }
        }
    }
    let mut built = Built {
        doc: Doc {
            src_lang: source.tag.clone(),
            trg_lang: target.tag.clone(),
            files: Vec::new(),
        },
        files: BTreeSet::new(),
        groups: BTreeSet::new(),
        units: HashMap::new(),
        messages: BTreeMap::new(),
        order: Vec::new(),
    };
    for (n, file) in source.files.iter().enumerate() {
        let file_id = format!("f{}", n + 1);
        let mut out = File {
            id: file_id.clone(),
            original: file.name.clone(),
            translate: file.translate,
            notes: file.notes.iter().map(clone_note).collect(),
            items: Vec::new(),
        };
        let mut section_ids: BTreeSet<String> = BTreeSet::new();
        for section in &file.sections {
            let mut items = Vec::new();
            let mut before: Vec<String> = Vec::new();
            for entry in &section.entries {
                let message = parse(&entry.id, &entry.source, &file.path)?;
                let translate = file.translate && section.translate && entry.translate;
                let target_message = targets.get(entry.id.as_str()).cloned();
                let item = message_item(
                    &mut built,
                    &file_id,
                    entry,
                    &message,
                    target_message.as_ref(),
                    &target.tag,
                    translate,
                )?;
                items.push(item);
                built.messages.insert(
                    entry.id.clone(),
                    MessageInfo {
                        source: message,
                        target: target_message,
                        file: file.name.clone(),
                        head: section.head.clone(),
                        own: entry.own.clone(),
                        before: before.clone(),
                    },
                );
                built.order.push(entry.id.clone());
                before.push(entry.id.clone());
            }
            match &section.head {
                None => out.items.extend(items),
                Some(head) => {
                    let name = head_text(head);
                    let mut id = format!("s:{}", xliff_id(&name));
                    let mut n = 1;
                    while !section_ids.insert(id.clone()) {
                        n += 1;
                        id = format!("s:{}:{n}", xliff_id(&name));
                    }
                    built.groups.insert((file_id.clone(), id.clone()));
                    out.items.push(Item::Group(Group {
                        id,
                        name,
                        kind: "mf2:section",
                        translate: section.translate,
                        notes: section.notes.iter().map(clone_note).collect(),
                        items,
                    }));
                }
            }
        }
        built.files.insert(file_id);
        built.doc.files.push(out);
    }
    Ok(built)
}

fn clone_note(n: &Note) -> Note {
    Note {
        category: n.category,
        text: n.text.clone(),
    }
}

/// A section head's id as written.
fn head_text(parts: &[String]) -> String {
    Id::new(parts.iter().map(|p| Cow::Borrowed(p.as_str())).collect()).to_string()
}

/// A message's unit, or its group of variant units. `translate` is what the
/// entry inherits from its file and section as well as its own.
fn message_item(
    built: &mut Built,
    file_id: &str,
    entry: &LocaleEntry,
    source: &Message<'static>,
    target: Option<&Message<'static>>,
    locale: &str,
    translate: bool,
) -> Result<Item> {
    let id = xliff_id(&entry.id);
    let notes: Vec<Note> = entry.notes.iter().map(clone_note).collect();
    let Some(offered) = offers(source, target, locale)? else {
        // Neither side selects: one unit.
        let Message::Pattern(s) = source else {
            return Err(Error::Usage("unreachable: a select without a group".into()));
        };
        let t = match target {
            Some(Message::Pattern(t)) => Some(&t.pattern),
            _ => None,
        };
        let unit = unit(
            built,
            file_id,
            &id,
            entry.id.clone(),
            &entry.id,
            None,
            &s.pattern,
            t,
            (entry.translate, translate),
            notes,
        )?;
        return Ok(Item::Unit(unit));
    };
    let mut items = Vec::with_capacity(offered.units.len());
    for (n, offer) in offered.units.iter().enumerate() {
        let name = keys_text(&offer.keys)?;
        let unit = unit(
            built,
            file_id,
            &format!("{id}:{}", n + 1),
            name,
            &entry.id,
            Some(offer.keys.clone()),
            offer.source,
            offer.target,
            (true, translate),
            Vec::new(),
        )?;
        items.push(Item::Unit(unit));
    }
    let mut notes = notes;
    if offered.selects_differently {
        notes.push(Note {
            category: "comment",
            text: "The source selects differently: every variant here is shown against the \
                   source's catch-all (*) variant."
                .to_owned(),
        });
    }
    built.groups.insert((file_id.to_owned(), id.clone()));
    Ok(Item::Group(Group {
        id,
        name: entry.id.clone(),
        kind: "mf2:select",
        translate: entry.translate,
        notes,
        items,
    }))
}

#[allow(clippy::too_many_arguments)]
fn unit(
    built: &mut Built,
    file_id: &str,
    id: &str,
    name: String,
    message: &str,
    keys: Option<Vec<Key<'static>>>,
    source: &Pattern<'_>,
    target: Option<&Pattern<'_>>,
    (own, translate): (bool, bool),
    notes: Vec<Note>,
) -> Result<Unit> {
    let mut coder = Coder::default();
    let source_inlines = coder.inlines(source, false)?;
    let target_inlines = target.map(|t| coder.inlines(t, true)).transpose()?;
    built.units.insert(
        (file_id.to_owned(), id.to_owned()),
        UnitInfo {
            message: message.to_owned(),
            keys,
            name: name.clone(),
            source: source.clone().into_owned(),
            translate,
            data: coder.data.clone(),
            codes: coder.codes.clone(),
        },
    );
    Ok(Unit {
        id: id.to_owned(),
        name: Some(name),
        translate: own,
        notes,
        data: coder.data,
        source: source_inlines,
        target: target_inlines,
    })
}

/// Writes one unit's patterns as inline content, numbering its codes and
/// its data.
#[derive(Default)]
struct Coder {
    data: Vec<(String, String)>,
    codes: HashMap<String, Vec<String>>,
    /// The source's codes: what each is (its kind and data ids) and its id,
    /// so that the same code in the target keeps the source's id.
    source_codes: Vec<(Vec<String>, String)>,
    next: u32,
}

impl Coder {
    fn data_ref(&mut self, text: String) -> String {
        if let Some((id, _)) = self.data.iter().find(|(_, t)| *t == text) {
            return id.clone();
        }
        let id = format!("d{}", self.data.len() + 1);
        self.data.push((id.clone(), text));
        id
    }

    fn inlines(&mut self, pattern: &Pattern<'_>, target: bool) -> Result<Vec<Inline>> {
        let parts = pattern.parts();
        let pairs = pair_markup(parts);
        let mut used = BTreeSet::new();
        self.range(parts, 0, parts.len(), &pairs, target, &mut used)
    }

    /// The id of a code: in the source, the next number; in the target, the
    /// id of the first source code that is the same code and not yet used.
    fn code_id(&mut self, sig: Vec<String>, target: bool, used: &mut BTreeSet<String>) -> String {
        if target
            && let Some((_, id)) = self
                .source_codes
                .iter()
                .find(|(s, id)| *s == sig && !used.contains(id))
        {
            let id = id.clone();
            used.insert(id.clone());
            return id;
        }
        self.next += 1;
        let id = self.next.to_string();
        if !target {
            self.source_codes.push((sig.clone(), id.clone()));
        }
        used.insert(id.clone());
        self.codes
            .insert(id.clone(), sig.into_iter().skip(1).collect());
        id
    }

    fn range(
        &mut self,
        parts: &[PatternPart<'_>],
        from: usize,
        to: usize,
        pairs: &[Option<usize>],
        target: bool,
        used: &mut BTreeSet<String>,
    ) -> Result<Vec<Inline>> {
        let mut out = Vec::new();
        let mut i = from;
        while i < to {
            let part = &parts[i];
            match part {
                PatternPart::Text(t) => out.push(Inline::Text(t.to_string())),
                PatternPart::Expression(_) => {
                    let text = part_text(part)?;
                    let r = self.data_ref(text.clone());
                    let id = self.code_id(vec!["ph".into(), r.clone()], target, used);
                    out.push(Inline::Ph(Code {
                        id,
                        data_ref: Some(r),
                        copy_of: None,
                        disp: Some(text),
                        fmt: false,
                    }));
                }
                PatternPart::Markup(m) => {
                    let text = part_text(part)?;
                    let r = self.data_ref(text.clone());
                    let code = |id: String, r: String, disp: String| Code {
                        id,
                        data_ref: Some(r),
                        copy_of: None,
                        disp: Some(disp),
                        fmt: true,
                    };
                    match (m.kind, pairs.get(i).copied().flatten()) {
                        (MarkupKind::Open, Some(close)) => {
                            let end_text = part_text(&parts[close])?;
                            let end = self.data_ref(end_text.clone());
                            let id = self.code_id(
                                vec!["pc".into(), r.clone(), end.clone()],
                                target,
                                used,
                            );
                            let children = self.range(parts, i + 1, close, pairs, target, used)?;
                            out.push(Inline::Pc {
                                code: code(id, r, text),
                                end: Some(end),
                                disp_end: Some(end_text),
                                children,
                            });
                            i = close;
                        }
                        (MarkupKind::Open, None) => {
                            let id = self.code_id(vec!["sc".into(), r.clone()], target, used);
                            out.push(Inline::Sc {
                                code: code(id, r, text),
                                isolated: true,
                            });
                        }
                        (MarkupKind::Close, _) => {
                            let id = self.code_id(vec!["ec".into(), r.clone()], target, used);
                            out.push(Inline::Ec {
                                code: code(id, r, text),
                                isolated: true,
                            });
                        }
                        (MarkupKind::Standalone, _) => {
                            let id = self.code_id(vec!["ph".into(), r.clone()], target, used);
                            out.push(Inline::Ph(code(id, r, text)));
                        }
                    }
                }
                _ => {
                    return Err(Error::Usage(
                        "a pattern part this version cannot write".into(),
                    ));
                }
            }
            i += 1;
        }
        Ok(out)
    }
}

/// For each open markup, the index of the close that pairs with it: the
/// next close of the same name while it is the innermost open one. What does
/// not pair so is isolated (`<sc>` / `<ec>`), which keeps every `<pc>`
/// properly nested.
fn pair_markup(parts: &[PatternPart<'_>]) -> Vec<Option<usize>> {
    let mut pairs = vec![None; parts.len()];
    let mut open: Vec<(usize, &str)> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if let PatternPart::Markup(Markup { kind, name, .. }) = part {
            match kind {
                MarkupKind::Open => open.push((i, name)),
                MarkupKind::Close => {
                    if let Some(&(at, n)) = open.last()
                        && n == name.as_ref()
                    {
                        pairs[at] = Some(i);
                        open.pop();
                    }
                }
                MarkupKind::Standalone => {}
            }
        }
    }
    pairs
}

/// `mf2 export --format xliff`: the document for `target_tag`.
pub(crate) fn export(
    source_path: &Path,
    source_tag: &str,
    target_path: &Path,
    target_tag: &str,
) -> Result<(String, usize)> {
    let source = Locale::read(source_path, source_tag)?;
    let target = Locale::read(target_path, target_tag)?;
    let built = build(&source, &target)?;
    Ok((xml::write(&built.doc), built.messages.len()))
}

// ─────────────────────────────── import ─────────────────────────────────

/// A code of §6.3's import table. A code's meaning never changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Finding {
    CodeEdited,
    UnknownCode,
    UnknownUnit,
    DoNotTranslate,
    Incomplete,
    Malformed,
}

impl Finding {
    /// Every code.
    /// (`src/workspace_tests.rs` holds it against the plan.)
    #[cfg(all(test, mf2_workspace))]
    pub(crate) const ALL: [Finding; 6] = [
        Finding::CodeEdited,
        Finding::UnknownCode,
        Finding::UnknownUnit,
        Finding::DoNotTranslate,
        Finding::Incomplete,
        Finding::Malformed,
    ];

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Finding::CodeEdited => "xliff-code-edited",
            Finding::UnknownCode => "xliff-unknown-code",
            Finding::UnknownUnit => "xliff-unknown-unit",
            Finding::DoNotTranslate => "xliff-do-not-translate",
            Finding::Incomplete => "xliff-incomplete",
            Finding::Malformed => "xliff-malformed",
        }
    }
}

/// One refusal: the code, where (`file` / `unit` ids), and why.
struct Report {
    code: Finding,
    at: String,
    message: String,
}

/// What the document asks of one message.
#[derive(Default)]
struct Update {
    refused: bool,
    /// Each translated unit: its keys (`None`: the message's only unit) and
    /// the pattern read back.
    patterns: Vec<(Option<Vec<Key<'static>>>, Pattern<'static>)>,
    /// Where the first unit was, for a report about the whole message.
    at: String,
}

/// Switches that exist only so the tests can show a check is what refuses
/// (the negative controls).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Checks {
    /// Skip comparing each `<data>` with the export's.
    #[cfg(test)]
    pub(crate) skip_data: bool,
}

/// What an XLIFF document asks of the target locale: the files it would
/// rewrite, and the units it refused. Nothing is written here — `mf2
/// import` first checks the corpus those files make (Phase 10 E3).
pub(crate) struct Imported {
    /// The target's files the document changes, with their new text.
    pub(crate) rewrites: Vec<Rewrite>,
    /// Messages changed, not counting those added.
    pub(crate) changed: usize,
    /// Messages the target did not have.
    pub(crate) added: usize,
    /// One line per refused unit: its code, where it is, and why.
    pub(crate) refused: Vec<String>,
}

/// `mf2 import` of an XLIFF document into `target_tag`: what it would
/// write, less every unit it refuses; `Err(Error::Corpus)` when the
/// document itself cannot be read (`xliff-malformed`, already reported).
pub(crate) fn import(
    source_path: &Path,
    source_tag: &str,
    target_path: &Path,
    target_tag: &str,
    text: &str,
    checks: Checks,
) -> Result<Imported> {
    let doc = match xml::read(text) {
        Ok(doc) => doc,
        Err(xml::Malformed(why)) => {
            eprintln!("{}: {why}; nothing was written", Finding::Malformed.code());
            return Err(Error::Corpus);
        }
    };
    if doc.trg_lang != target_tag {
        return Err(Error::Usage(format!(
            "the document is a translation into {:?}, not {target_tag}; nothing was written",
            doc.trg_lang
        )));
    }
    let source = Locale::read(source_path, source_tag)?;
    let target = Locale::read(target_path, target_tag)?;
    let built = build(&source, &target)?;

    let mut reports: Vec<Report> = Vec::new();
    let mut updates: BTreeMap<String, Update> = BTreeMap::new();
    for file in &doc.files {
        if !built.files.contains(&file.id) {
            reports.push(Report {
                code: Finding::UnknownUnit,
                at: file.id.clone(),
                message: format!("no <file> {:?} in the export", file.id),
            });
            continue;
        }
        for item in &file.items {
            let unit = match item {
                Item::Group(g) => {
                    if !built.groups.contains(&(file.id.clone(), g.id.clone())) {
                        reports.push(Report {
                            code: Finding::UnknownUnit,
                            at: format!("{}/{}", file.id, g.id),
                            message: "no such <group> in the export".to_owned(),
                        });
                    }
                    continue;
                }
                Item::Unit(u) => u,
            };
            let at = format!("{}/{}", file.id, unit.id);
            let Some(info) = built.units.get(&(file.id.clone(), unit.id.clone())) else {
                reports.push(Report {
                    code: Finding::UnknownUnit,
                    at,
                    message: "no such <unit> in the export".to_owned(),
                });
                continue;
            };
            let update = updates.entry(info.message.clone()).or_default();
            if update.at.is_empty() {
                update.at.clone_from(&at);
            }
            if unit.name.as_ref().is_some_and(|n| *n != info.name) {
                reports.push(Report {
                    code: Finding::UnknownUnit,
                    at,
                    message: format!(
                        "the unit is named {:?}; the export names it {:?}",
                        unit.name.as_deref().unwrap_or_default(),
                        info.name
                    ),
                });
                update.refused = true;
                continue;
            }
            let Some(target) = unit.target.as_ref().filter(|t| !t.is_empty()) else {
                continue;
            };
            let pattern = match read_back(unit, target, info, checks) {
                Ok(p) => p,
                Err((code, message)) => {
                    reports.push(Report { code, at, message });
                    update.refused = true;
                    continue;
                }
            };
            if !info.translate {
                if pattern != info.source {
                    reports.push(Report {
                        code: Finding::DoNotTranslate,
                        at,
                        message: format!(
                            "{} is @do-not-translate and its target differs from its source",
                            info.message
                        ),
                    });
                    update.refused = true;
                }
                // Equal to its source: nothing to write.
                continue;
            }
            update.patterns.push((info.keys.clone(), pattern));
        }
    }

    // Rebuild each message the document translates.
    let mut changed: BTreeMap<String, String> = BTreeMap::new();
    let mut added: Vec<String> = Vec::new();
    for (id, update) in &updates {
        if update.refused || update.patterns.is_empty() {
            continue;
        }
        let Some(info) = built.messages.get(id) else {
            continue;
        };
        let Some(message) = rebuild(info, &update.patterns) else {
            reports.push(Report {
                code: Finding::Incomplete,
                at: update.at.clone(),
                message: format!(
                    "{id}: the catch-all (*) variant has no target, so the message cannot be \
                     written"
                ),
            });
            continue;
        };
        if info.target.as_ref() == Some(&message) {
            continue;
        }
        let text =
            mf2_syntax::serialize(&message).map_err(|e| Error::Usage(format!("{id}: {e}")))?;
        if info.target.is_none() {
            added.push(id.clone());
        }
        changed.insert(id.clone(), text);
    }
    // In source order, so that new messages go in the order the source has.
    added.sort_by_key(|id| built.order.iter().position(|o| o == id));

    Ok(Imported {
        rewrites: plan_target(&target, &built, &changed, &added)?,
        changed: changed.len() - added.len(),
        added: added.len(),
        refused: reports
            .iter()
            .map(|r| format!("{}: {}: {}", r.code.code(), r.at, r.message))
            .collect(),
    })
}

/// A unit's target as a pattern: text as text, each code replaced by the
/// expression its data holds.
fn read_back(
    unit: &Unit,
    target: &[Inline],
    info: &UnitInfo,
    checks: Checks,
) -> std::result::Result<Pattern<'static>, (Finding, String)> {
    // Every `<data>` the document has must be the export's.
    let _ = checks;
    #[cfg(test)]
    let compare = !checks.skip_data;
    #[cfg(not(test))]
    let compare = true;
    if compare {
        for (id, text) in &unit.data {
            let expected = info.data.iter().find(|(i, _)| i == id).map(|(_, t)| t);
            if expected != Some(text) {
                return Err((
                    Finding::CodeEdited,
                    match expected {
                        Some(e) => format!("<data id={id:?}> is {text:?}; the export has {e:?}"),
                        None => format!("<data id={id:?}> {text:?} is not in the export"),
                    },
                ));
            }
        }
    }
    let data = |r: &str| -> std::result::Result<String, (Finding, String)> {
        unit.data
            .iter()
            .find(|(i, _)| i == r)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| {
                (
                    Finding::UnknownCode,
                    format!("a code names <data id={r:?}>, which the unit does not have"),
                )
            })
    };
    // The data ids of a code: its own `dataRef`s, else its base's (`copyOf`).
    let refs = |code: &Code,
                end: Option<&String>|
     -> std::result::Result<Vec<String>, (Finding, String)> {
        if let Some(r) = &code.data_ref {
            let mut v = vec![r.clone()];
            v.extend(end.cloned());
            return Ok(v);
        }
        if let Some(base) = &code.copy_of
            && let Some(v) = info.codes.get(base)
        {
            return Ok(v.clone());
        }
        Err((
            Finding::UnknownCode,
            format!(
                "code {:?} has no data and copies no code of the unit: it has no MF2 meaning",
                code.id
            ),
        ))
    };
    let mut pattern = Pattern::new();
    walk(target, &mut pattern, &data, &refs)?;
    Ok(pattern)
}

type Found<T> = std::result::Result<T, (Finding, String)>;

/// A code's data ids: its own, else its base's.
type Refs<'a> = dyn Fn(&Code, Option<&String>) -> Found<Vec<String>> + 'a;

fn walk(
    inlines: &[Inline],
    pattern: &mut Pattern<'static>,
    data: &dyn Fn(&str) -> Found<String>,
    refs: &Refs<'_>,
) -> Found<()> {
    for inline in inlines {
        match inline {
            Inline::Text(t) => pattern.push(PatternPart::Text(Cow::Owned(t.clone()))),
            Inline::Ph(code) | Inline::Sc { code, .. } | Inline::Ec { code, .. } => {
                let r = refs(code, None)?;
                push_code(pattern, &data(first(&r))?)?;
            }
            Inline::Pc {
                code,
                end,
                children,
                ..
            } => {
                let r = refs(code, end.as_ref())?;
                push_code(pattern, &data(first(&r))?)?;
                walk(children, pattern, data, refs)?;
                match r.get(1) {
                    Some(e) => push_code(pattern, &data(e)?)?,
                    None => {
                        return Err((
                            Finding::UnknownCode,
                            format!("<pc id={:?}> has no end data", code.id),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn first(refs: &[String]) -> &str {
    refs.first().map_or("", String::as_str)
}

/// The one expression or markup a `<data>` holds, appended.
fn push_code(pattern: &mut Pattern<'static>, text: &str) -> Found<()> {
    let parsed = mf2_syntax::parse_model(text)
        .message
        .map(Message::into_owned);
    if let Some(Message::Pattern(m)) = parsed
        && m.declarations.is_empty()
        && let [part] = m.pattern.parts()
        && !matches!(part, PatternPart::Text(_))
    {
        pattern.push(part.clone());
        return Ok(());
    }
    Err((
        Finding::CodeEdited,
        format!("{text:?} is not one MF2 expression or markup"),
    ))
}

/// The target message with the document's patterns in it; `None` when a
/// message that selects would have no catch-all variant.
fn rebuild(
    info: &MessageInfo,
    patterns: &[(Option<Vec<Key<'static>>>, Pattern<'static>)],
) -> Option<Message<'static>> {
    // A target keeps its own declarations and selectors; a new one takes
    // the source's.
    let mut message = match &info.target {
        Some(t) => t.clone(),
        None => match &info.source {
            Message::Pattern(p) => Message::Pattern(p.clone()),
            Message::Select(s) => Message::Select(SelectMessage {
                declarations: s.declarations.clone(),
                selectors: s.selectors.clone(),
                variants: Vec::new(),
            }),
            other => other.clone(),
        },
    };
    for (keys, pattern) in patterns {
        match (&mut message, keys) {
            (Message::Pattern(m), None) => m.pattern = pattern.clone(),
            (Message::Pattern(m), Some(k)) if k.is_empty() => m.pattern = pattern.clone(),
            (Message::Select(m), Some(k)) if k.len() == m.selectors.len() => {
                if let Some(v) = m.variants.iter_mut().find(|v| same_keys(&v.keys, k)) {
                    v.value = pattern.clone();
                } else {
                    // Before the catch-all, where a person would write it.
                    let at = m
                        .variants
                        .iter()
                        .position(|v| v.keys.iter().all(|k| matches!(k, Key::CatchAll(_))))
                        .unwrap_or(m.variants.len());
                    m.variants.insert(
                        at,
                        Variant {
                            keys: k.clone(),
                            value: pattern.clone(),
                        },
                    );
                }
            }
            // Keys of another shape than the target's: the export never
            // writes them.
            _ => {}
        }
    }
    if let Message::Select(m) = &message
        && !m
            .variants
            .iter()
            .any(|v| v.keys.iter().all(|k| matches!(k, Key::CatchAll(_))))
    {
        return None;
    }
    Some(message)
}

/// The target locale's files with the changed and added messages in them:
/// each file whose text changes.
fn plan_target(
    target: &Locale,
    built: &Built,
    changed: &BTreeMap<String, String>,
    added: &[String],
) -> Result<Vec<Rewrite>> {
    if changed.is_empty() {
        return Ok(Vec::new());
    }
    let mut out: Vec<(PathBuf, String, Option<String>)> = Vec::new();
    if target.json {
        let Some(file) = target.files.first() else {
            return Ok(Vec::new());
        };
        let mut pairs: Vec<(&str, &str)> = target
            .entries()
            .map(|e| {
                let source = changed.get(&e.id).map_or(e.source.as_str(), String::as_str);
                (e.id.as_str(), source)
            })
            .collect();
        for id in added {
            if let Some(text) = changed.get(id) {
                pairs.push((id, text));
            }
        }
        let before = read(&file.path)?;
        out.push((file.path.clone(), json::write(pairs), Some(before)));
    } else {
        // Existing messages, where they stand.
        let dir = target
            .files
            .first()
            .and_then(|f| f.path.parent().map(Path::to_path_buf));
        let mut by_name: BTreeMap<String, (PathBuf, Option<String>)> = BTreeMap::new();
        for file in &target.files {
            by_name.insert(
                file.name.clone(),
                (file.path.clone(), Some(read(&file.path)?)),
            );
        }
        // A new message goes in the file of the source's name.
        for id in added {
            if let Some(info) = built.messages.get(id) {
                by_name.entry(info.file.clone()).or_insert_with(|| {
                    let path = dir.clone().unwrap_or_default().join(&info.file);
                    (path, None)
                });
            }
        }
        for (name, (path, text)) in &by_name {
            let fresh = format!("@locale {}\n---\n", target.tag);
            let text_ref = text.as_deref().unwrap_or(&fresh);
            let (resource, diagnostics) = mf2_resource::parse(text_ref);
            if !diagnostics.is_empty() {
                return Err(syntax_error(path));
            }
            let mut resource =
                resource.map_values(
                    |info, value| match changed.get(&info.full_id().to_string()) {
                        Some(new) => Cow::Owned(new.clone()),
                        None => value,
                    },
                );
            for id in added {
                let Some(info) = built.messages.get(id) else {
                    continue;
                };
                if info.file != *name {
                    continue;
                }
                if let Some(value) = changed.get(id) {
                    insert(&mut resource, info, value);
                }
            }
            let new = serialize_with(&resource, &resource::FMT_STYLE)
                .map_err(|e| Error::Usage(format!("{}: {e}", path.display())))?;
            out.push((path.clone(), new, text.clone()));
        }
    }
    Ok(out
        .into_iter()
        .filter(|(_, new, before)| before.as_deref() != Some(new.as_str()))
        .map(|(path, text, _)| Rewrite { path, text })
        .collect())
}

const NOWHERE: Span = Span { start: 0, end: 0 };

/// Adds a new message to `resource` where the source has it: in the section
/// of the same id (added at the end when the target lacks it), after the
/// nearest entry before it in the source that the target has.
fn insert<'a>(resource: &mut Resource<'a, Cow<'a, str>>, info: &MessageInfo, value: &str) {
    let head_matches = |s: &Section<'a, Cow<'a, str>>| match (&s.head, &info.head) {
        (None, None) => true,
        (Some(h), Some(parts)) => {
            h.id.parts()
                .iter()
                .map(AsRef::as_ref)
                .eq(parts.iter().map(String::as_str))
        }
        _ => false,
    };
    let at = if let Some(at) = resource.sections.iter().position(head_matches) {
        at
    } else {
        let section = Section {
            head: info.head.as_ref().map(|parts| Head {
                id: owned_id(parts),
                comment: None,
                meta: Vec::new(),
                span: NOWHERE,
            }),
            entries: Vec::new(),
            detached: Vec::new(),
        };
        if info.head.is_none() {
            resource.sections.insert(0, section);
            0
        } else {
            resource.sections.push(section);
            resource.sections.len() - 1
        }
    };
    let section = &mut resource.sections[at];
    let full = |e: &Entry<'a, Cow<'a, str>>| match &section.head {
        Some(h) => h.id.join(&e.id).to_string(),
        None => e.id.to_string(),
    };
    let position = info
        .before
        .iter()
        .rev()
        .find_map(|b| section.entries.iter().position(|e| full(e) == *b))
        .map_or(0, |p| p + 1);
    for d in &mut section.detached {
        if d.before > position {
            d.before += 1;
        }
    }
    section.entries.insert(
        position,
        Entry {
            id: owned_id(&info.own),
            value: Cow::Owned(value.to_owned()),
            comment: None,
            meta: Vec::new(),
            span: NOWHERE,
            id_span: NOWHERE,
            value_span: NOWHERE,
            map: ValueMap::Empty,
        },
    );
}

fn owned_id(parts: &[String]) -> Id<'static> {
    Id::new(parts.iter().map(|p| Cow::Owned(p.clone())).collect())
}

#[cfg(test)]
mod tests {
    use super::{Checks, export, import, pair_markup, xliff_id};

    /// Negative control for `xliff-code-edited`: with the comparison of
    /// each `<data>` switched off, the edited code is written into the
    /// translation — so the refusal is that comparison's doing.
    #[test]
    fn without_the_data_check_an_edited_code_lands() {
        let dir = std::env::temp_dir().join(format!("mf2-xliff-control-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (en, pl) = (dir.join("en"), dir.join("pl"));
        std::fs::create_dir_all(&en).expect("mkdir");
        std::fs::create_dir_all(&pl).expect("mkdir");
        std::fs::write(en.join("m.mf2"), "@locale en\n---\n\nhi = Hi, {$name}!\n").expect("write");
        std::fs::write(
            pl.join("m.mf2"),
            "@locale pl\n---\n\nhi = Cześć, {$name}!\n",
        )
        .expect("write");
        let (doc, _) = export(&en, "en", &pl, "pl").expect("export");
        let edited = doc.replacen(">{$name}</data>", ">{$name :string}</data>", 1);
        assert_ne!(edited, doc);
        let refused = import(&en, "en", &pl, "pl", &edited, Checks::default()).expect("read");
        assert_eq!(refused.refused.len(), 1);
        assert!(refused.rewrites.is_empty());
        let lands =
            import(&en, "en", &pl, "pl", &edited, Checks { skip_data: true }).expect("read");
        assert_eq!(lands.refused, Vec::<String>::new());
        assert_eq!(
            (lands.changed, lands.added, lands.rewrites.len()),
            (1, 0, 1)
        );
        let after = &lands.rewrites[0].text;
        assert!(after.contains("hi = Cześć, {$name :string}!"), "{after}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_are_nmtokens_that_cannot_meet() {
        assert_eq!(xliff_id("chat.send-now_2"), "chat.send-now_2");
        assert_eq!(xliff_id("a:b"), "x:613a62");
        assert_eq!(xliff_id("żółw"), "x:c5bcc3b3c58277");
        assert_ne!(xliff_id("x:61"), "x:61");
    }

    #[test]
    fn markup_pairs_only_when_properly_nested() {
        let m = mf2_syntax::parse_model("{#a}{#b}x{/a}{/b}{/c}{#d}")
            .message
            .expect("valid");
        let mf2_model::Message::Pattern(p) = m else {
            panic!("a pattern");
        };
        let pairs = pair_markup(p.pattern.parts());
        // `b` pairs; `a` crosses it and stays isolated, as do `/c` and `#d`.
        assert_eq!(pairs[0], None);
        assert_eq!(pairs[1], Some(4));
        assert_eq!(pairs[6], None);
    }
}
