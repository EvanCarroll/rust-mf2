//! The manifest (`plans/05-tooling.md` §3): what the wasm and every catalog
//! agree on — ids, per-message slot names and markup names, the function set
//! — its hash (`plans/02-catalog-format.md` §3) and its file,
//! `manifest.mf2m` (§5).

use alloc::string::String;
use alloc::vec::Vec;

use mf2_model::MsgId;

use crate::bytes::{Cur, u16_at, u64_at};
use crate::error::ManifestError;

/// `"MF2M"`.
const MAGIC: [u8; 4] = *b"MF2M";
/// File version 1.0 (`major << 8 | minor`).
const VERSION: u16 = 0x0100;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The manifest: ids in `MsgId` order (bytewise ascending), and per message
/// its slot names (NFC, ascending — the slot order) and markup names (NFC,
/// ascending, unique); the function identifiers the corpus uses (NFC,
/// ascending, unique). `mf2_syntax::analyze` gives exactly these per
/// message.
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Manifest {
    pub ids: Vec<String>,
    pub slots: Vec<Vec<String>>,
    pub markup: Vec<Vec<String>>,
    pub functions: Vec<String>,
}

/// Where the canonical serialization goes: a buffer, or straight into the
/// hash.
trait Sink {
    fn bytes(&mut self, b: &[u8]);

    fn uint(&mut self, mut v: u64) {
        loop {
            let low = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.bytes(&[low]);
                return;
            }
            self.bytes(&[low | 0x80]);
        }
    }

    fn str(&mut self, s: &str) {
        self.uint(s.len() as u64);
        self.bytes(s.as_bytes());
    }
}

impl Sink for Vec<u8> {
    fn bytes(&mut self, b: &[u8]) {
        self.extend_from_slice(b);
    }
}

struct Fnv(u64);

impl Sink for Fnv {
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
}

impl Manifest {
    fn serialize(&self, out: &mut impl Sink) {
        out.uint(self.ids.len() as u64);
        let empty = Vec::new();
        for (i, id) in self.ids.iter().enumerate() {
            out.str(id);
            let slots = self.slots.get(i).unwrap_or(&empty);
            out.uint(slots.len() as u64);
            for s in slots {
                out.str(s);
            }
            let markup = self.markup.get(i).unwrap_or(&empty);
            out.uint(markup.len() as u64);
            for s in markup {
                out.str(s);
            }
        }
        out.uint(self.functions.len() as u64);
        for s in &self.functions {
            out.str(s);
        }
    }

    /// The canonical serialization (`plans/02-catalog-format.md` §3).
    pub fn canonical(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.serialize(&mut out);
        out
    }

    /// `manifest_hash`: FNV-1a 64 over the canonical serialization.
    pub fn hash(&self) -> u64 {
        let mut h = Fnv(FNV_OFFSET);
        self.serialize(&mut h);
        h.0
    }

    /// The `MsgId` of `id` (chunk 0), by binary search.
    pub fn msg_id(&self, id: &str) -> Option<MsgId> {
        let i = self
            .ids
            .binary_search_by(|x| x.as_bytes().cmp(id.as_bytes()))
            .ok()?;
        MsgId::new(0, u32::try_from(i).ok()?)
    }

    /// Checks the shape: one slot list and one markup list per id, and every
    /// list strictly ascending (bytewise).
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.slots.len() != self.ids.len() || self.markup.len() != self.ids.len() {
            return Err(ManifestError::Invalid(
                "one slot list and one markup list per id",
            ));
        }
        if !ascending(&self.ids) {
            return Err(ManifestError::Invalid("ids are not strictly ascending"));
        }
        if !self.slots.iter().all(|s| ascending(s)) {
            return Err(ManifestError::Invalid("slots are not strictly ascending"));
        }
        if !self.markup.iter().all(|s| ascending(s)) {
            return Err(ManifestError::Invalid(
                "markup names are not strictly ascending",
            ));
        }
        if !ascending(&self.functions) {
            return Err(ManifestError::Invalid(
                "functions are not strictly ascending",
            ));
        }
        Ok(())
    }

    /// The file `manifest.mf2m`: `"MF2M" · u16 version · u64 hash ·
    /// canonical serialization`.
    pub fn write(&self) -> Vec<u8> {
        let body = self.canonical();
        let mut out = Vec::with_capacity(14 + body.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.hash().to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Reads `manifest.mf2m`: checks the magic, the version, the hash, that
    /// nothing trails, and [`Manifest::validate`].
    pub fn read(bytes: &[u8]) -> Result<Manifest, ManifestError> {
        if bytes.get(..4) != Some(&MAGIC[..]) {
            return Err(ManifestError::Magic);
        }
        let version = u16_at(bytes, 4).ok_or(ManifestError::Truncated)?;
        if version >> 8 != VERSION >> 8 {
            return Err(ManifestError::Version);
        }
        let stored = u64_at(bytes, 6).ok_or(ManifestError::Truncated)?;
        let mut c = Cur::new(bytes, 14);

        let n = count(&mut c)?;
        let mut m = Manifest {
            ids: Vec::with_capacity(n.min(c.remaining())),
            slots: Vec::with_capacity(n.min(c.remaining())),
            markup: Vec::with_capacity(n.min(c.remaining())),
            functions: Vec::new(),
        };
        for _ in 0..n {
            m.ids.push(string(&mut c)?);
            m.slots.push(strings(&mut c)?);
            m.markup.push(strings(&mut c)?);
        }
        m.functions = strings(&mut c)?;
        if !c.at_end() {
            return Err(ManifestError::Trailing);
        }
        if m.hash() != stored {
            return Err(ManifestError::Hash);
        }
        m.validate()?;
        Ok(m)
    }
}

fn ascending(list: &[String]) -> bool {
    list.windows(2).all(|w| match w {
        [a, b] => a.as_bytes() < b.as_bytes(),
        _ => true,
    })
}

fn count(c: &mut Cur<'_>) -> Result<usize, ManifestError> {
    c.len().ok_or(ManifestError::Truncated)
}

fn string(c: &mut Cur<'_>) -> Result<String, ManifestError> {
    let len = count(c)?;
    let b = c.take(len).ok_or(ManifestError::Truncated)?;
    core::str::from_utf8(b)
        .map(String::from)
        .map_err(|_| ManifestError::Utf8)
}

fn strings(c: &mut Cur<'_>) -> Result<Vec<String>, ManifestError> {
    let n = count(c)?;
    let mut out = Vec::with_capacity(n.min(c.remaining()));
    for _ in 0..n {
        out.push(string(c)?);
    }
    Ok(out)
}
