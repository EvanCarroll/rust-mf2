//! The manifest file (`manifest.mf2m`, plans/02-catalog-format.md §5): what
//! the wasm and the catalogs agree on — ids → `MsgId`, per-message variable
//! slots and markup names, the function set — and `manifest_hash`.

use crate::error::DecodeError;
use crate::wire::{Fnv64, Reader, Writer};

const MAGIC: &str = "P09M";
const VERSION: u8 = 1;

/// One message of the source locale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// Dotted message id.
    pub id: String,
    /// External variables in slot order (ascending bytewise).
    pub vars: Vec<String>,
    /// Markup names, sorted.
    pub markup: Vec<String>,
}

/// The manifest. `entries[i]` is `MsgId(i)`; entries are sorted by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// `manifest_hash`.
    pub hash: u64,
    /// Source locale tag.
    pub source_locale: String,
    /// Messages, sorted by id (bytewise) — the index is the `MsgId`.
    pub entries: Vec<ManifestEntry>,
    /// Function names used by any locale, sorted.
    pub functions: Vec<String>,
}

impl Manifest {
    /// Builds a manifest; sorts and computes the hash.
    pub fn new(
        source_locale: &str,
        mut entries: Vec<ManifestEntry>,
        mut functions: Vec<String>,
    ) -> Self {
        entries.sort_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));
        functions.sort();
        functions.dedup();
        let hash = Self::compute_hash(&entries, &functions);
        Self {
            hash,
            source_locale: source_locale.to_owned(),
            entries,
            functions,
        }
    }

    /// FNV-1a 64 over the canonical serialization of the ordered
    /// (id, slots, markup) list and the function set (02 §3).
    pub fn compute_hash(entries: &[ManifestEntry], functions: &[String]) -> u64 {
        let mut h = Fnv64::default();
        for e in entries {
            h.write(e.id.as_bytes());
            h.write(&[0]);
            for v in &e.vars {
                h.write(v.as_bytes());
                h.write(&[0]);
            }
            h.write(&[1]);
            for m in &e.markup {
                h.write(m.as_bytes());
                h.write(&[0]);
            }
            h.write(&[2]);
        }
        h.write(&[3]);
        for f in functions {
            h.write(f.as_bytes());
            h.write(&[0]);
        }
        h.finish()
    }

    /// The `MsgId` and entry of `id`.
    pub fn lookup(&self, id: &str) -> Option<(u32, &ManifestEntry)> {
        let i = self
            .entries
            .binary_search_by(|e| e.id.as_bytes().cmp(id.as_bytes()))
            .ok()?;
        Some((u32::try_from(i).ok()?, self.entries.get(i)?))
    }

    /// Serializes.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.buf.extend_from_slice(MAGIC.as_bytes());
        w.u8(VERSION);
        w.u64(self.hash);
        w.str(&self.source_locale);
        w.len(self.entries.len());
        for e in &self.entries {
            w.str(&e.id);
            w.len(e.vars.len());
            for v in &e.vars {
                w.str(v);
            }
            w.len(e.markup.len());
            for m in &e.markup {
                w.str(m);
            }
        }
        w.len(self.functions.len());
        for f in &self.functions {
            w.str(f);
        }
        w.buf
    }

    /// Deserializes (the hash is read, not recomputed; see [`Self::verify`]).
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        r.magic(MAGIC)?;
        let v = r.u8()?;
        if v != VERSION {
            return Err(DecodeError::Version(v));
        }
        let hash = r.u64()?;
        let source_locale = r.str()?.to_owned();
        let n = r.len()?;
        let mut entries = Vec::with_capacity(n.min(1 << 20));
        for _ in 0..n {
            let id = r.str()?.to_owned();
            let nv = r.len()?;
            let mut vars = Vec::with_capacity(nv.min(64));
            for _ in 0..nv {
                vars.push(r.str()?.to_owned());
            }
            let nm = r.len()?;
            let mut markup = Vec::with_capacity(nm.min(64));
            for _ in 0..nm {
                markup.push(r.str()?.to_owned());
            }
            entries.push(ManifestEntry { id, vars, markup });
        }
        let nf = r.len()?;
        let mut functions = Vec::with_capacity(nf.min(1024));
        for _ in 0..nf {
            functions.push(r.str()?.to_owned());
        }
        Ok(Self {
            hash,
            source_locale,
            entries,
            functions,
        })
    }

    /// Whether the stored hash matches the content.
    pub fn verify(&self) -> bool {
        self.hash == Self::compute_hash(&self.entries, &self.functions)
    }
}
