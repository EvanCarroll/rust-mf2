//! Feature `serde`: the resource as JSON, in the shape of the draft's data
//! model.
//!
//! ```json
//! {
//!   "comment": "A comment about the file.",
//!   "meta": [{ "name": "locale", "value": "en-US" }],
//!   "sections": [
//!     { "id": [], "entries": [{ "id": ["chat-send"], "value": "Send" }] },
//!     { "id": ["hotkeys"],
//!       "entries": [{ "id": ["release"], "value": "Release {#kbd}?{/kbd}" }] }
//!   ]
//! }
//! ```
//!
//! An id is its parts, unescaped, so a section's and an entry's compose
//! without re-reading dots; an entry's `value` is whatever the resource's
//! message type serializes as (`Cow<str>` from [`crate::parse`],
//! `mf2_model::Message` after [`Resource::map_values`], …). Fields that are
//! empty are left out.
//!
//! A resource read from JSON has no spans and no
//! [`ValueMap`](crate::ValueMap): nothing wrote its values in a file.
//!
//! The shape is written from the draft at
//! `third_party/w3c-message-resource/PIN`, which is not vendored; where the
//! two disagree, the draft wins (see the crate documentation).

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use mf2_model::Span;
use serde::{Deserialize, Serialize, Serializer};

use crate::model::{Comment, Detached, Entry, Head, Id, Meta, Resource, Section, ValueMap};

const NO_SPAN: Span = Span { start: 0, end: 0 };

// ───────────────────────────── serialization ─────────────────────────────

#[derive(Serialize)]
struct SerMeta<'r> {
    name: &'r str,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<&'r str>,
}

#[derive(Serialize)]
struct SerDetached<'r> {
    before: usize,
    comment: &'r str,
}

#[derive(Serialize)]
struct SerEntry<'r, V> {
    id: Vec<&'r str>,
    value: &'r V,
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<&'r str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    meta: Vec<SerMeta<'r>>,
}

#[derive(Serialize)]
struct SerSection<'r, V> {
    id: Vec<&'r str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<&'r str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    meta: Vec<SerMeta<'r>>,
    entries: Vec<SerEntry<'r, V>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    detached: Vec<SerDetached<'r>>,
}

#[derive(Serialize)]
struct SerResource<'r, V> {
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<&'r str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    meta: Vec<SerMeta<'r>>,
    sections: Vec<SerSection<'r, V>>,
}

fn ser_meta<'r>(meta: &'r [Meta<'_>]) -> Vec<SerMeta<'r>> {
    meta.iter()
        .map(|m| SerMeta {
            name: &m.name,
            value: m.value.as_deref(),
        })
        .collect()
}

fn ser_id<'r>(id: &'r Id<'_>) -> Vec<&'r str> {
    id.parts().iter().map(AsRef::as_ref).collect()
}

impl<V: Serialize> Serialize for Resource<'_, V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        SerResource {
            comment: self.comment.as_ref().map(|c| c.text.as_ref()),
            meta: ser_meta(&self.meta),
            sections: self
                .sections
                .iter()
                .map(|section| SerSection {
                    id: section
                        .head
                        .as_ref()
                        .map(|h| ser_id(&h.id))
                        .unwrap_or_default(),
                    comment: section
                        .head
                        .as_ref()
                        .and_then(|h| h.comment.as_ref())
                        .map(|c| c.text.as_ref()),
                    meta: section
                        .head
                        .as_ref()
                        .map_or_else(Vec::new, |h| ser_meta(&h.meta)),
                    entries: section
                        .entries
                        .iter()
                        .map(|entry| SerEntry {
                            id: ser_id(&entry.id),
                            value: &entry.value,
                            comment: entry.comment.as_ref().map(|c| c.text.as_ref()),
                            meta: ser_meta(&entry.meta),
                        })
                        .collect(),
                    detached: section
                        .detached
                        .iter()
                        .map(|d| SerDetached {
                            before: d.before,
                            comment: &d.comment.text,
                        })
                        .collect(),
                })
                .collect(),
        }
        .serialize(s)
    }
}

// ──────────────────────────── deserialization ────────────────────────────

#[derive(Deserialize)]
struct DeMeta {
    name: String,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Deserialize)]
struct DeDetached {
    before: usize,
    comment: String,
}

#[derive(Deserialize)]
struct DeEntry<V> {
    id: Vec<String>,
    value: V,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    meta: Vec<DeMeta>,
}

#[derive(Deserialize)]
struct DeSection<V> {
    #[serde(default)]
    id: Vec<String>,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    meta: Vec<DeMeta>,
    #[serde(default = "Vec::new")]
    entries: Vec<DeEntry<V>>,
    #[serde(default)]
    detached: Vec<DeDetached>,
}

#[derive(Deserialize)]
struct DeResource<V> {
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    meta: Vec<DeMeta>,
    #[serde(default = "Vec::new")]
    sections: Vec<DeSection<V>>,
}

fn de_comment<'a>(text: Option<String>) -> Option<Comment<'a>> {
    text.map(|text| Comment {
        text: Cow::Owned(text),
        span: NO_SPAN,
    })
}

fn de_meta<'a>(meta: Vec<DeMeta>) -> Vec<Meta<'a>> {
    meta.into_iter()
        .map(|m| Meta {
            name: Cow::Owned(m.name),
            value: m.value.map(Cow::Owned),
            span: NO_SPAN,
            value_span: None,
        })
        .collect()
}

fn de_id<'a>(parts: Vec<String>) -> Id<'a> {
    Id::new(parts.into_iter().map(Cow::Owned).collect())
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for Resource<'_, V> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let de = DeResource::<V>::deserialize(d)?;
        Ok(Resource {
            comment: de_comment(de.comment),
            meta: de_meta(de.meta),
            sections: de
                .sections
                .into_iter()
                .map(|s| Section {
                    head: if s.id.is_empty() {
                        None
                    } else {
                        Some(Head {
                            id: de_id(s.id),
                            comment: de_comment(s.comment),
                            meta: de_meta(s.meta),
                            span: NO_SPAN,
                        })
                    },
                    entries: s
                        .entries
                        .into_iter()
                        .map(|e| Entry {
                            id: de_id(e.id),
                            value: e.value,
                            comment: de_comment(e.comment),
                            meta: de_meta(e.meta),
                            span: NO_SPAN,
                            id_span: NO_SPAN,
                            value_span: NO_SPAN,
                            map: ValueMap::Empty,
                        })
                        .collect(),
                    detached: s
                        .detached
                        .into_iter()
                        .map(|d| Detached {
                            before: d.before,
                            comment: Comment {
                                text: Cow::Owned(d.comment),
                                span: NO_SPAN,
                            },
                        })
                        .collect(),
                })
                .collect(),
        })
    }
}
