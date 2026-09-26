//! The resource data model, generic over the message type.
//!
//! Written from the W3C Message Resource draft at
//! `third_party/w3c-message-resource/PIN` (the draft states no license, so
//! nothing is vendored; see `plans/05-tooling.md` §2). The draft's
//! `Resource<Message>` becomes [`Resource<'a, V>`]: `V` is whatever an entry's
//! value is — the MF2 source as written (`parse` gives
//! `Cow<'a, str>`), or a parsed `Message`, or anything else a caller maps to
//! with [`Resource::map_values`].
//!
//! # Equality ignores spans
//!
//! Every `PartialEq` here compares *content*: ids, values, comment text and
//! properties. Spans and [`ValueMap`]s are where the content was found, not
//! what it is, so `parse(&serialize(&r)) == r` holds even though the
//! serializer lays the file out its own way.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use mf2_model::Span;

use crate::error::{Error, Role};

/// A dotted id: `chat`, `chat.prompts.row`.
///
/// The parts are *unescaped*: a part may contain any character, and
/// [`Display`](fmt::Display) writes the escapes back. Section heads and
/// entries each carry their own id; an entry's full id is its section's parts
/// followed by its own ([`EntryRef::id`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id<'a> {
    parts: Vec<Cow<'a, str>>,
}

impl<'a> Id<'a> {
    /// An id from its parts, as they read (unescaped).
    pub fn new(parts: Vec<Cow<'a, str>>) -> Self {
        Id { parts }
    }

    /// The parts, outermost first.
    pub fn parts(&self) -> &[Cow<'a, str>] {
        &self.parts
    }

    /// Appends a part.
    pub fn push(&mut self, part: impl Into<Cow<'a, str>>) {
        self.parts.push(part.into());
    }

    /// Whether the id has no parts.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// How many parts.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// This id's parts followed by `other`'s.
    #[must_use]
    pub fn join(&self, other: &Id<'a>) -> Id<'a> {
        let mut parts = Vec::with_capacity(self.parts.len() + other.parts.len());
        parts.extend(self.parts.iter().cloned());
        parts.extend(other.parts.iter().cloned());
        Id { parts }
    }

    /// The id as written in a resource file, with the escapes a part needs.
    ///
    /// The inverse of [`Id::read`]. Fails on a part the syntax cannot write
    /// (a control character, a line terminator) and on an empty part.
    pub fn canonical(&self) -> Result<String, Error> {
        let mut out = String::new();
        self.write_canonical(&mut out)?;
        Ok(out)
    }

    /// Writes [`Id::canonical`] into `out`.
    pub fn write_canonical(&self, out: &mut String) -> Result<(), Error> {
        if self.parts.is_empty() {
            return Err(Error::Empty(Role::Id));
        }
        let start = out.len();
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                out.push('.');
            }
            if part.is_empty() {
                return Err(Error::Empty(Role::IdPart));
            }
            for c in part.chars() {
                if is_unwritable(c) {
                    return Err(Error::Unrepresentable(Role::Id, c));
                }
                if !is_id_char(c) {
                    out.push('\\');
                }
                out.push(c);
            }
        }
        // `---` is the frontmatter separator, never an id (plans/05 §2).
        if out[start..] == *"---" {
            out.insert(start, '\\');
        }
        Ok(())
    }

    /// The id a resource file's text spells, unescaping each part.
    ///
    /// Takes the text as it stands in a file (`chat.prompts.row`,
    /// `odd\.name`); the inverse of [`Id::canonical`].
    pub fn read(text: &'a str) -> Result<Id<'a>, Error> {
        if text.is_empty() {
            return Err(Error::Empty(Role::Id));
        }
        let mut parts = Vec::new();
        let mut part = Cow::Borrowed("");
        let mut run = 0usize; // start of the current borrowed run
        let mut chars = text.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => {
                    let owned = part.to_mut();
                    owned.push_str(&text[run..i]);
                    if let Some((j, escaped)) = chars.next() {
                        owned.push(escaped);
                        run = j + escaped.len_utf8();
                    } else {
                        run = text.len();
                    }
                }
                '.' => {
                    finish_part(&mut part, text, run, i, &mut parts);
                    part = Cow::Borrowed("");
                    run = i + 1;
                }
                _ => {}
            }
        }
        finish_part(&mut part, text, run, text.len(), &mut parts);
        if parts.is_empty() {
            return Err(Error::Empty(Role::Id));
        }
        if parts.iter().any(|p| p.is_empty()) {
            return Err(Error::Empty(Role::IdPart));
        }
        Ok(Id { parts })
    }
}

/// Closes the part being read: the borrowed run `text[run..end]` plus anything
/// unescaping already had to own.
fn finish_part<'a>(
    part: &mut Cow<'a, str>,
    text: &'a str,
    run: usize,
    end: usize,
    parts: &mut Vec<Cow<'a, str>>,
) {
    let run = &text[run.min(end)..end];
    let done = match core::mem::replace(part, Cow::Borrowed("")) {
        Cow::Borrowed(_) => Cow::Borrowed(run),
        Cow::Owned(mut owned) => {
            owned.push_str(run);
            Cow::Owned(owned)
        }
    };
    parts.push(done);
}

/// Whether `c` may stand unescaped in an id part: letters, digits, `_`, `-`
/// and the non-ASCII name characters of `spec/message.abnf` (plans/05 §2).
pub fn is_id_char(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || c == '_' || c == '-';
    }
    mf2_model::is_name_char(c)
}

/// Whether `c` cannot appear in an id, a property name or a comment at all:
/// the syntax has no escape for it there.
fn is_unwritable(c: char) -> bool {
    c.is_control() || c == '\u{2028}' || c == '\u{2029}'
}

impl fmt::Display for Id<'_> {
    /// [`Id::canonical`], writing an unwritable character as it stands (only
    /// an id built in code can hold one).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.parts.len() == 1 && self.parts[0] == "---" {
            // The frontmatter separator is never an id (plans/05 §2).
            f.write_str("\\")?;
        }
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            for c in part.chars() {
                if !is_id_char(c) && !is_unwritable(c) {
                    f.write_str("\\")?;
                }
                write!(f, "{c}")?;
            }
        }
        Ok(())
    }
}

/// Where a cooked value's bytes came from in the file.
///
/// A value is cooked: continuation lines lose their indentation, line breaks
/// become one LF, escapes become their characters. A diagnostic inside a
/// message therefore has to be mapped back before it can name a column.
#[derive(Clone, Debug, Default)]
pub enum ValueMap {
    /// The value is one run of the source starting at this offset: cooked
    /// offset *n* is source offset `base + n`. The common case — a
    /// single-line value with no escapes.
    Linear {
        /// Source offset of the value's first byte.
        base: u32,
    },
    /// Runs of the source, in cooked order.
    Segments(Vec<Segment>),
    /// An empty value.
    #[default]
    Empty,
}

/// One run of [`ValueMap::Segments`]: `len` cooked bytes that came from
/// `src`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Offset of the run's first byte in the cooked value.
    pub cooked: u32,
    /// Offset of the run's first byte in the source.
    pub src: u32,
    /// Length of the run in cooked bytes (an escape's run is one character
    /// long however many source bytes wrote it).
    pub len: u32,
}

impl ValueMap {
    /// Where cooked offset `at` came from, clamped to the run that holds it.
    ///
    /// `None` only for an empty map.
    pub fn source_offset(&self, at: u32) -> Option<u32> {
        match self {
            ValueMap::Linear { base } => Some(base.saturating_add(at)),
            ValueMap::Empty => None,
            ValueMap::Segments(segs) => {
                let i = match segs.binary_search_by_key(&at, |s| s.cooked) {
                    Ok(i) => i,
                    Err(0) => return segs.first().map(|s| s.src),
                    Err(i) => i - 1,
                };
                let seg = segs.get(i)?;
                // Inside a run the offsets track one for one; an escape's run
                // is shorter in cooked bytes than in the source, so anything
                // past its start maps to where the escape began.
                let delta = at - seg.cooked;
                Some(if delta < seg.len {
                    seg.src + delta
                } else {
                    seg.src
                })
            }
        }
    }

    /// The span in the source of the cooked range `start..end`.
    pub fn source_span(&self, start: u32, end: u32) -> Option<Span> {
        let s = self.source_offset(start)?;
        let e = self.source_offset(end.max(start))?;
        Some(Span {
            start: s,
            end: e.max(s),
        })
    }
}

/// A `#` comment: one or more adjacent lines, joined by LF, without their
/// `#` and the space that may follow it.
#[derive(Clone, Debug)]
pub struct Comment<'a> {
    /// The text.
    pub text: Cow<'a, str>,
    /// Where the comment's lines are.
    pub span: Span,
}

impl PartialEq for Comment<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}
impl Eq for Comment<'_> {}

/// An `@name value` property.
#[derive(Clone, Debug)]
pub struct Meta<'a> {
    /// The name, without the `@`.
    pub name: Cow<'a, str>,
    /// The value, cooked like an entry's; `None` for a bare `@name`.
    pub value: Option<Cow<'a, str>>,
    /// Where the property is, from `@` to the end of its last line.
    pub span: Span,
    /// Where the value is in the source, if it has one.
    pub value_span: Option<Span>,
}

impl PartialEq for Meta<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value
    }
}
impl Eq for Meta<'_> {}

/// One `id = value` entry.
#[derive(Clone, Debug)]
pub struct Entry<'a, V> {
    /// The entry's own id, without its section's prefix.
    pub id: Id<'a>,
    /// The value: the MF2 source as written, or whatever it was mapped to.
    pub value: V,
    /// The comment above it, if it attaches (`parse` documents the
    /// attachment rules).
    pub comment: Option<Comment<'a>>,
    /// The properties above it, in the order written.
    pub meta: Vec<Meta<'a>>,
    /// The whole entry, from its id to the end of its last value line.
    pub span: Span,
    /// The id alone.
    pub id_span: Span,
    /// The value alone, from its first byte to the end of its last line.
    pub value_span: Span,
    /// Where the cooked value's bytes came from.
    pub map: ValueMap,
}

impl<V: PartialEq> PartialEq for Entry<'_, V> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.value == other.value
            && self.comment == other.comment
            && self.meta == other.meta
    }
}
impl<V: Eq> Eq for Entry<'_, V> {}

/// A `[section]` head.
#[derive(Clone, Debug)]
pub struct Head<'a> {
    /// The section's id; every entry under it is prefixed with these parts.
    pub id: Id<'a>,
    /// The comment above the head, if it attaches.
    pub comment: Option<Comment<'a>>,
    /// The properties above the head.
    pub meta: Vec<Meta<'a>>,
    /// The `[id]` line.
    pub span: Span,
}

impl PartialEq for Head<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.comment == other.comment && self.meta == other.meta
    }
}
impl Eq for Head<'_> {}

/// A comment an empty line cut off from whatever follows it: it describes
/// nothing, and is kept so that `mf2 fmt` does not delete it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detached<'a> {
    /// The index in [`Section::entries`] this comment sat before
    /// (`entries.len()` if it came after the last one).
    pub before: usize,
    /// The comment.
    pub comment: Comment<'a>,
}

/// A run of entries under one head. A file's entries before its first
/// `[section]` are a section with no head.
#[derive(Clone, Debug)]
pub struct Section<'a, V> {
    /// The head, or `None` for the entries before the first one.
    pub head: Option<Head<'a>>,
    /// The entries, in the order written.
    pub entries: Vec<Entry<'a, V>>,
    /// Comments that attach to nothing, by the entry they precede.
    pub detached: Vec<Detached<'a>>,
}

impl<V: PartialEq> PartialEq for Section<'_, V> {
    fn eq(&self, other: &Self) -> bool {
        self.head == other.head && self.entries == other.entries && self.detached == other.detached
    }
}
impl<V: Eq> Eq for Section<'_, V> {}

impl<V> Default for Section<'_, V> {
    fn default() -> Self {
        Section {
            head: None,
            entries: Vec::new(),
            detached: Vec::new(),
        }
    }
}

/// One resource file: its frontmatter and its sections.
#[derive(Clone, Debug)]
pub struct Resource<'a, V> {
    /// The comment above the frontmatter separator.
    pub comment: Option<Comment<'a>>,
    /// The frontmatter properties — `@locale` among them.
    pub meta: Vec<Meta<'a>>,
    /// The sections, in the order written.
    pub sections: Vec<Section<'a, V>>,
}

impl<V: PartialEq> PartialEq for Resource<'_, V> {
    fn eq(&self, other: &Self) -> bool {
        self.comment == other.comment && self.meta == other.meta && self.sections == other.sections
    }
}
impl<V: Eq> Eq for Resource<'_, V> {}

impl<V> Default for Resource<'_, V> {
    fn default() -> Self {
        Resource {
            comment: None,
            meta: Vec::new(),
            sections: Vec::new(),
        }
    }
}

impl<'a, V> Resource<'a, V> {
    /// The value of the frontmatter property `name`, if it has one.
    pub fn property(&self, name: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|m| m.name == name)
            .and_then(|m| m.value.as_deref())
    }

    /// The resource's `@locale`, the draft's one required property.
    pub fn locale(&self) -> Option<&str> {
        self.property("locale")
    }

    /// Every entry with the head it sits under, in file order.
    pub fn iter(&self) -> impl Iterator<Item = EntryRef<'_, 'a, V>> {
        self.sections.iter().flat_map(|s| {
            s.entries.iter().map(move |entry| EntryRef {
                section: s.head.as_ref(),
                entry,
            })
        })
    }

    /// How many entries the resource has.
    pub fn entry_count(&self) -> usize {
        self.sections.iter().map(|s| s.entries.len()).sum()
    }

    /// The same resource with every value replaced.
    ///
    /// This is the draft's `Resource<Message>`: a loader parses to
    /// `Resource<Cow<str>>` and maps the sources to messages, keeping every
    /// id, comment, property and span.
    pub fn map_values<W>(self, mut f: impl FnMut(&EntryInfo<'_, 'a>, V) -> W) -> Resource<'a, W> {
        Resource {
            comment: self.comment,
            meta: self.meta,
            sections: self
                .sections
                .into_iter()
                .map(|s| {
                    let head = s.head;
                    let entries = s
                        .entries
                        .into_iter()
                        .map(|e| {
                            let Entry {
                                id,
                                value,
                                comment,
                                meta,
                                span,
                                id_span,
                                value_span,
                                map,
                            } = e;
                            let value = {
                                let info = EntryInfo {
                                    section: head.as_ref(),
                                    id: &id,
                                    meta: &meta,
                                    span,
                                    id_span,
                                    value_span,
                                    map: &map,
                                };
                                f(&info, value)
                            };
                            Entry {
                                id,
                                value,
                                comment,
                                meta,
                                span,
                                id_span,
                                value_span,
                                map,
                            }
                        })
                        .collect();
                    Section {
                        head,
                        entries,
                        detached: s.detached,
                    }
                })
                .collect(),
        }
    }
}

/// An entry and the head it sits under.
#[derive(Clone, Copy, Debug)]
pub struct EntryRef<'r, 'a, V> {
    /// The section's head, or `None` for the entries before the first one.
    pub section: Option<&'r Head<'a>>,
    /// The entry.
    pub entry: &'r Entry<'a, V>,
}

impl<'a, V> EntryRef<'_, 'a, V> {
    /// The entry's full id: its section's parts followed by its own.
    pub fn id(&self) -> Id<'a> {
        match self.section {
            Some(head) => head.id.join(&self.entry.id),
            None => self.entry.id.clone(),
        }
    }
}

/// What [`Resource::map_values`] tells its closure about the entry it is
/// mapping. The value itself is passed separately, by value.
#[derive(Clone, Copy, Debug)]
pub struct EntryInfo<'r, 'a> {
    /// The section's head, or `None`.
    pub section: Option<&'r Head<'a>>,
    /// The entry's own id.
    pub id: &'r Id<'a>,
    /// The entry's properties.
    pub meta: &'r [Meta<'a>],
    /// The whole entry.
    pub span: Span,
    /// The id alone.
    pub id_span: Span,
    /// The value alone.
    pub value_span: Span,
    /// Where the cooked value's bytes came from.
    pub map: &'r ValueMap,
}

impl<'a> EntryInfo<'_, 'a> {
    /// The entry's full id.
    pub fn full_id(&self) -> Id<'a> {
        match self.section {
            Some(head) => head.id.join(self.id),
            None => self.id.clone(),
        }
    }
}
