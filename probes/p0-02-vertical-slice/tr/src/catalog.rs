//! A deliberately trivial, hand-built `.mf2b` stand-in: magic, manifest hash,
//! locale + direction, an offset index and one UTF-8 string pool. Only `simple`
//! messages exist; the real format is specified in `plans/02-catalog-format.md`.
//!
//! Layout (all integers little-endian, read unaligned with `from_le_bytes`):
//!
//! ```text
//! 0   magic "MF2B"
//! 4   format_version u16 (= 1)
//! 6   flags u16            bit 0: rtl
//! 8   manifest_hash u64
//! 16  message_count u32
//! 20  locale_len u8, locale bytes (ASCII)
//! ..  index: (message_count + 1) × u32 — 2-bit kind ‖ 30-bit byte offset into
//!     the pool; entry i+1's offset is entry i's end
//! ..  string pool (UTF-8), always last
//! ```
//!
//! The pool carries no per-string length prefix: `02` §2 says "varint length +
//! UTF-8" *and* (F4) "one `str::from_utf8` pass over the pool", which cannot both
//! hold once a length reaches 128 (varint bytes ≥ 0x80 are not UTF-8). Ends come
//! from the next index entry instead.

use crate::error::CatalogError;

pub const MAGIC: [u8; 4] = *b"MF2B";
pub const FORMAT_VERSION: u16 = 1;
const HEADER_FIXED: usize = 21;
const KIND_SHIFT: u32 = 30;
const OFFSET_MASK: u32 = (1 << KIND_SHIFT) - 1;
/// Kind bits of a `simple` message (the only kind this probe emits).
pub const KIND_SIMPLE: u32 = 0;

/// Text direction of a locale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Ltr,
    Rtl,
}

impl Dir {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        }
    }
}

/// A validated catalog: the structure bytes and the string pool, split once.
#[derive(Debug)]
pub struct Catalog {
    structure: Box<[u8]>,
    pool: Box<str>,
    count: u32,
    index_at: usize,
    locale_end: usize,
    dir: Dir,
    manifest_hash: u64,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes(s.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(s.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    let s = b.get(at..at.checked_add(8)?)?;
    Some(u64::from_le_bytes(s.try_into().ok()?))
}

impl Catalog {
    /// Validates once (magic, version, manifest hash, bounds, index
    /// monotonicity, one UTF-8 pass over the pool). Accessors never panic.
    pub fn new(mut bytes: Vec<u8>, expect_manifest: u64) -> Result<Self, CatalogError> {
        use CatalogError as E;
        if bytes.get(0..4) != Some(&MAGIC[..]) {
            return Err(if bytes.len() < 4 { E::Truncated } else { E::BadMagic });
        }
        if u16_at(&bytes, 4).ok_or(E::Truncated)? != FORMAT_VERSION {
            return Err(E::UnsupportedVersion);
        }
        let flags = u16_at(&bytes, 6).ok_or(E::Truncated)?;
        let manifest_hash = u64_at(&bytes, 8).ok_or(E::Truncated)?;
        if manifest_hash != expect_manifest {
            return Err(E::ManifestMismatch);
        }
        let count = u32_at(&bytes, 16).ok_or(E::Truncated)?;
        let locale_len = usize::from(*bytes.get(20).ok_or(E::Truncated)?);
        let locale_end = HEADER_FIXED + locale_len;
        let locale = bytes.get(HEADER_FIXED..locale_end).ok_or(E::Truncated)?;
        if !locale.is_ascii() || locale.is_empty() {
            return Err(E::BadLocale);
        }
        let index_at = locale_end;
        let entries = usize::try_from(count).map_err(|_| E::Truncated)? + 1;
        let pool_at = entries
            .checked_mul(4)
            .and_then(|n| n.checked_add(index_at))
            .ok_or(E::Truncated)?;
        if pool_at > bytes.len() {
            return Err(E::Truncated);
        }
        let pool_len = bytes.len() - pool_at;
        let mut prev = 0u32;
        for i in 0..entries {
            let e = u32_at(&bytes, index_at + i * 4).ok_or(E::Truncated)?;
            let off = e & OFFSET_MASK;
            if off < prev || off as usize > pool_len {
                return Err(E::BadIndex);
            }
            prev = off;
        }
        let pool_bytes = bytes.split_off(pool_at);
        let pool = String::from_utf8(pool_bytes).map_err(|_| E::BadUtf8)?;
        let dir = if flags & 1 == 1 { Dir::Rtl } else { Dir::Ltr };
        Ok(Self {
            structure: bytes.into_boxed_slice(),
            pool: pool.into_boxed_str(),
            count,
            index_at,
            locale_end,
            dir,
            manifest_hash,
        })
    }

    /// BCP-47 tag of this catalog.
    pub fn locale(&self) -> &str {
        self.structure
            .get(HEADER_FIXED..self.locale_end)
            .and_then(|b| core::str::from_utf8(b).ok())
            .unwrap_or("und")
    }

    pub fn dir(&self) -> Dir {
        self.dir
    }

    pub fn manifest_hash(&self) -> u64 {
        self.manifest_hash
    }

    pub fn len(&self) -> u32 {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// O(1) lookup. Out-of-range ids and bad offsets yield `""`, never a panic.
    pub fn get(&self, id: u32) -> &str {
        if id >= self.count {
            return "";
        }
        let at = self.index_at + id as usize * 4;
        let (Some(a), Some(b)) = (u32_at(&self.structure, at), u32_at(&self.structure, at + 4))
        else {
            return "";
        };
        self.pool
            .get((a & OFFSET_MASK) as usize..(b & OFFSET_MASK) as usize)
            .unwrap_or("")
    }
}

/// FNV-1a 64 — `const`, so the manifest hash of an id list is computed at
/// compile time and the ids themselves never reach the wasm.
#[allow(clippy::indexing_slicing)] // bounded by the loop condition
pub const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

/// The writer: server / build side only.
#[cfg(feature = "ssr")]
#[allow(clippy::expect_used, clippy::panic)] // build side, not the client path
pub fn write(locale: &str, dir: Dir, manifest_hash: u64, messages: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    let flags: u16 = if dir == Dir::Rtl { 1 } else { 0 };
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&manifest_hash.to_le_bytes());
    let count = u32::try_from(messages.len()).expect("message count fits u32");
    out.extend_from_slice(&count.to_le_bytes());
    out.push(u8::try_from(locale.len()).expect("locale tag < 256 bytes"));
    out.extend_from_slice(locale.as_bytes());
    let mut pool = String::new();
    let mut offsets = Vec::with_capacity(messages.len() + 1);
    for m in messages {
        offsets.push(u32::try_from(pool.len()).expect("pool < 1 GiB"));
        pool.push_str(m);
    }
    offsets.push(u32::try_from(pool.len()).expect("pool < 1 GiB"));
    for off in offsets {
        assert!(off <= OFFSET_MASK, "pool offset exceeds 30 bits");
        out.extend_from_slice(&((KIND_SIMPLE << KIND_SHIFT) | off).to_le_bytes());
    }
    out.extend_from_slice(pool.as_bytes());
    out
}

#[cfg(all(test, feature = "ssr"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_rejections() {
        let bytes = write("ar", Dir::Rtl, 42, &["مرحبا", "", "x"]);
        let c = Catalog::new(bytes.clone(), 42).expect("valid");
        assert_eq!(c.locale(), "ar");
        assert_eq!(c.dir(), Dir::Rtl);
        assert_eq!(c.get(0), "مرحبا");
        assert_eq!(c.get(1), "");
        assert_eq!(c.get(2), "x");
        assert_eq!(c.get(3), "");
        assert_eq!(Catalog::new(bytes.clone(), 43).unwrap_err(), CatalogError::ManifestMismatch);
        let mut bad = bytes.clone();
        bad[0] = b'X';
        assert_eq!(Catalog::new(bad, 42).unwrap_err(), CatalogError::BadMagic);
        let short = bytes[..bytes.len() - 20].to_vec();
        assert!(Catalog::new(short, 42).is_err());
    }
}
