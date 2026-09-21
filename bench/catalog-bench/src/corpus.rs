//! The inputs: the four generated locales of the reference workload — `en`
//! (the source locale, byte-identical to the committed
//! `bench/corpora/workload-1600.json`), `pl`, `en-XA`, `ar-XB` — parsed with
//! `mf2-syntax`; the manifest built from the source locale; and one stripped
//! and one unstripped catalog per locale, loaded back and decoded.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mf2_catalog::format::locale_key;
use mf2_catalog::writer::{self, Options};
use mf2_catalog::{Catalog, Dir, Entry, Manifest, MsgId, decode};
use mf2_model::Message;
use serde::Serialize;
use workload_gen::{Knobs, Workload, locale};

use crate::error::{Error, Result};
use crate::plural;

/// The reference `manifest_hash` of the default workload (P0.7; plans/09
/// "State at the start").
pub const MANIFEST_HASH: u64 = 0x43e0_dc12_eeb0_5ef1;

/// The committed source-locale corpus, relative to the repository root.
pub const COMMITTED_CORPUS: &str = "bench/corpora/workload-1600.json";

/// One locale's messages as MF2 source.
#[derive(Debug, Clone)]
pub struct LocaleSource {
    /// BCP 47 tag.
    pub tag: String,
    /// Direction (`ar-XB` is right-to-left).
    pub dir: Dir,
    /// `(id, MF2 source)`, in `MsgId` (bytewise id) order.
    pub messages: Vec<(String, String)>,
}

impl LocaleSource {
    /// MF2 source bytes: the UTF-8 length of every message source, summed
    /// (plans/06 §2's "text length"; P0.7's "MF2 source bytes").
    pub fn source_bytes(&self) -> usize {
        self.messages.iter().map(|(_, s)| s.len()).sum()
    }
}

/// Generates the four locales in process (workload-gen with its default
/// knobs and seed) and checks that `en` is the committed corpus byte for
/// byte, so that every figure is reproducible from the committed file and
/// the generator.
pub fn generate(repo: &Path) -> Result<Vec<LocaleSource>> {
    let knobs = Knobs::default();
    let wl = Workload::generate(&knobs)?;
    let path = repo.join(COMMITTED_CORPUS);
    let committed = std::fs::read_to_string(&path).map_err(|source| Error::Read {
        path: path.clone(),
        source,
    })?;
    if workload_gen::corpus_json(&wl)? != committed {
        return Err(Error::Corpus(format!(
            "{COMMITTED_CORPUS} differs from the generator's default output \
             (regenerate it with `cargo xtask gen-workload corpora`, or fix the generator)"
        )));
    }
    Ok(locale::locales(&knobs)?
        .iter()
        .map(|loc| {
            let sources = locale::sources(&wl, loc);
            LocaleSource {
                tag: loc.tag.to_owned(),
                dir: if loc.is_rtl() { Dir::Rtl } else { Dir::Ltr },
                messages: wl
                    .order
                    .iter()
                    .map(|&j| (wl.messages[j].id.clone(), sources[j].clone()))
                    .collect(),
            }
        })
        .collect())
}

/// How many INDEX entries of each kind a catalog has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Kinds {
    /// Kind 0: one text run.
    pub simple: usize,
    /// Kind 1: a pattern with placeholders or markup.
    pub pattern: usize,
    /// Kind 2: `.match`.
    pub select: usize,
    /// Kind 3: not in this catalog.
    pub absent: usize,
}

impl Kinds {
    /// Counts the kinds of every INDEX entry of `cat`.
    pub fn of(cat: &Catalog) -> Self {
        let mut k = Kinds::default();
        for i in 0..cat.message_count() {
            match MsgId::new(0, i).map(|id| cat.get(id)) {
                Some(Entry::Simple(_)) => k.simple += 1,
                Some(Entry::Pattern(_)) => k.pattern += 1,
                Some(Entry::Select(_)) => k.select += 1,
                Some(Entry::Absent) | None => k.absent += 1,
            }
        }
        k
    }
}

/// One locale's catalogs and what was checked about them.
#[derive(Debug, Clone)]
pub struct Built {
    /// BCP 47 tag.
    pub tag: String,
    /// Direction.
    pub dir: Dir,
    /// MF2 source bytes.
    pub source_bytes: usize,
    /// Messages (manifest ids).
    pub messages: usize,
    /// INDEX kinds.
    pub kinds: Kinds,
    /// The CLDR locale of the plural rules, and the entry's length.
    pub plural: (&'static str, usize),
    /// Production catalog: COLD and IDS stripped.
    pub stripped: Vec<u8>,
    /// With IDS (COLD is empty for this workload, so absent either way).
    pub unstripped: Vec<u8>,
    /// Messages whose `decode` equals the parsed model: stripped, unstripped
    /// (both 0 when `build` was asked not to decode).
    pub lossless: (usize, usize),
}

/// Parses one message; the workload is valid MF2, so any diagnostic is an error.
fn parse<'s>(tag: &str, id: &str, src: &'s str) -> Result<Message<'s>> {
    let parsed = mf2_syntax::parse_model(src);
    match parsed.message {
        Some(m) if parsed.diagnostics.is_empty() => Ok(m),
        _ => Err(Error::Corpus(format!(
            "{tag} `{id}` does not parse cleanly: {:?}",
            parsed.diagnostics
        ))),
    }
}

/// The manifest `mf2-build` will write: the source locale's ids (sorted),
/// per message the NFC slot and markup names of `mf2_syntax::analyze`, and the
/// functions of every locale (as `crates/mf2-catalog/tests/manifest.rs`).
pub fn manifest(source: &[(String, Message<'_>)], all: &[Vec<(String, Message<'_>)>]) -> Manifest {
    let mut m = Manifest::default();
    for (id, msg) in source {
        let a = mf2_syntax::analyze(msg);
        m.ids.push(id.clone());
        m.slots
            .push(a.externals.iter().map(|n| n.nfc.to_string()).collect());
        m.markup
            .push(a.markup.iter().map(|n| n.nfc.to_string()).collect());
    }
    let functions: BTreeSet<String> = all
        .iter()
        .flatten()
        .flat_map(|(_, msg)| {
            mf2_syntax::analyze(msg)
                .functions
                .iter()
                .map(|n| n.nfc.to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    m.functions = functions.into_iter().collect();
    m
}

/// Builds and loads every locale's catalogs and, with `round_trip`, decodes
/// every message back (otherwise `Built::lossless` is `(0, 0)`). The first
/// locale is the source locale; its manifest must hash to [`MANIFEST_HASH`].
pub fn build(locales: &[LocaleSource], round_trip: bool) -> Result<(Manifest, Vec<Built>)> {
    let parsed: Vec<Vec<(String, Message<'_>)>> = locales
        .iter()
        .map(|l| {
            l.messages
                .iter()
                .map(|(id, src)| Ok((id.clone(), parse(&l.tag, id, src)?)))
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<_>>()?;
    let source = parsed
        .first()
        .ok_or_else(|| Error::Corpus("no locales".to_owned()))?;
    let manifest = manifest(source, &parsed);
    manifest
        .validate()
        .map_err(|e| Error::Corpus(format!("manifest: {e}")))?;
    let hash = manifest.hash();
    if hash != MANIFEST_HASH {
        return Err(Error::Corpus(format!(
            "manifest_hash {hash:016x}, expected {MANIFEST_HASH:016x} (P0.7's reference)"
        )));
    }
    let slot_of: BTreeMap<&str, usize> = manifest
        .ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    let mut built = Vec::with_capacity(locales.len());
    for (l, msgs) in locales.iter().zip(&parsed) {
        let mut by_id: Vec<Option<&Message<'_>>> = vec![None; manifest.ids.len()];
        for (id, m) in msgs {
            let i = slot_of
                .get(id.as_str())
                .ok_or_else(|| Error::Corpus(format!("{}: id `{id}` is not in en", l.tag)))?;
            by_id[*i] = Some(m);
        }
        let (plural_locale, entry) = plural::cardinal(&l.tag)
            .ok_or_else(|| Error::Corpus(format!("no plural rules for `{}`", l.tag)))?;
        let mut options = Options::new(l.tag.clone(), l.dir);
        options.cldr_version = Some(plural::CLDR);
        options.locale_entries = vec![(locale_key::PLURAL_CARDINAL, entry.to_vec())];
        let unstripped = writer::catalog(&manifest, &by_id, &options)?;
        let stripped = writer::catalog(&manifest, &by_id, &options.stripped())?;

        let load = |bytes: &[u8]| {
            Catalog::new(bytes.to_vec(), hash).map_err(|source| Error::Load {
                tag: l.tag.clone(),
                source,
            })
        };
        let equal = |cat: &Catalog| {
            if !round_trip {
                return 0;
            }
            by_id
                .iter()
                .enumerate()
                .filter(|(i, want)| {
                    let id = u32::try_from(*i).ok().and_then(|i| MsgId::new(0, i));
                    match (id, want) {
                        (Some(id), Some(want)) => decode(cat, id).is_ok_and(|got| got == **want),
                        _ => false,
                    }
                })
                .count()
        };
        let cat_s = load(&stripped)?;
        let cat_u = load(&unstripped)?;
        built.push(Built {
            tag: l.tag.clone(),
            dir: l.dir,
            source_bytes: l.source_bytes(),
            messages: manifest.ids.len(),
            kinds: Kinds::of(&cat_s),
            plural: (plural_locale, entry.len()),
            lossless: (equal(&cat_s), equal(&cat_u)),
            stripped,
            unstripped,
        });
    }
    Ok((manifest, built))
}
