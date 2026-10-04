//! The manifest: what the wasm and every catalog
//! agree on.
//!
//! It is derived from the **source locale** — ids numbered densely in
//! bytewise order, each message's external variables in NFC slot order, its
//! markup names — plus the function identifiers the whole corpus uses, since
//! the generated registry is closed-world (B13). Its hash covers exactly
//! those four things, so editing a translation's *text* never changes it and
//! never rebuilds the wasm.
//!
//! The one thing a translation can change is that fourth input. A translator
//! who writes `{$n :number}` where the source wrote `{$n}` has added a
//! function the client must link, so the hash moves and the wasm is rebuilt
//! — correctly: a closed-world registry that did not would leave the page
//! with an Unknown Function at run time.

use std::collections::{BTreeMap, BTreeSet};

use mf2_catalog::Manifest;
use mf2_model::Message;

use crate::corpus::LocaleSource;

/// A manifest and the source records behind it.
#[derive(Debug)]
pub struct Built {
    /// The manifest.
    pub manifest: Manifest,
    /// For each `MsgId` index, the source locale's record index.
    pub source_records: Vec<usize>,
}

impl Built {
    /// The `MsgId` index of `id`.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.manifest
            .ids
            .binary_search_by(|i| i.as_str().cmp(id))
            .ok()
    }
}

/// Builds the manifest from the source locale's messages and the functions
/// the corpus uses.
pub fn build(
    source: &LocaleSource,
    models: &[Option<Message<'_>>],
    by_id: &BTreeMap<&str, usize>,
    functions: &BTreeSet<String>,
) -> Built {
    let mut manifest = Manifest {
        ids: Vec::with_capacity(by_id.len()),
        slots: Vec::with_capacity(by_id.len()),
        markup: Vec::with_capacity(by_id.len()),
        functions: functions.iter().cloned().collect(),
    };
    let mut source_records = Vec::with_capacity(by_id.len());
    // A `BTreeMap<&str, _>` is already in bytewise ascending order, which is
    // `MsgId` order.
    for (id, &record) in by_id {
        let (slots, markup) = match models.get(record).and_then(Option::as_ref) {
            Some(model) => {
                let analysis = mf2_syntax::analyze(model);
                (
                    analysis
                        .externals
                        .iter()
                        .map(|n| n.nfc.to_string())
                        .collect(),
                    analysis.markup.iter().map(|n| n.nfc.to_string()).collect(),
                )
            }
            // A message the source locale could not parse is already an
            // error; it keeps its id so that the rest of the report makes
            // sense.
            None => (Vec::new(), Vec::new()),
        };
        manifest.ids.push((*id).to_owned());
        manifest.slots.push(slots);
        manifest.markup.push(markup);
        source_records.push(record);
    }
    let _ = source;
    Built {
        manifest,
        source_records,
    }
}

/// The function identifiers a locale's messages use, in NFC.
pub fn functions_of(models: &[Option<Message<'_>>], into: &mut BTreeSet<String>) {
    for model in models.iter().flatten() {
        for function in &mf2_syntax::analyze(model).functions {
            into.insert(function.nfc.to_string());
        }
    }
}
