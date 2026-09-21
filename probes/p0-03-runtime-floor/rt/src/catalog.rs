//! The catalog reader: `Catalog::new` validates once (F4), then every accessor
//! is a bounds-checked O(1) read that never panics (a bad offset yields `None`
//! and the caller's fallback).

use alloc::vec::Vec;

use mf2b_format::{self as f, flags, header, kind, locale_key, section};

use crate::error::CatalogError;

/// `MsgId`: low 24 bits index, high 8 bits chunk (plans/02 §3).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MsgId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Ltr,
    Rtl,
}

/// Functions this runtime knows (the closed-world registry of the skeleton).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FnId {
    String,
    Integer,
    Number,
    Unknown,
}

impl FnId {
    fn from_name(name: &str) -> Self {
        match name {
            "string" => FnId::String,
            "integer" => FnId::Integer,
            "number" => FnId::Number,
            _ => FnId::Unknown,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Span {
    pub off: usize,
    pub len: usize,
}

/// A validated catalog: the structure bytes and the string pool (F2). Share
/// it with `Rc`/`Arc<Catalog>`; the two buffers are never copied again.
pub struct Catalog {
    /// Eager UTF-8: the structure bytes (header + sections before the pool).
    /// Per-access UTF-8: the whole fetched buffer, untouched (zero-copy load).
    head: Vec<u8>,
    #[cfg(not(feature = "utf8-per-access"))]
    pool: alloc::string::String,
    #[cfg(feature = "utf8-per-access")]
    pool_start: usize,
    count: u32,
    dir: Dir,
    locale: Span,
    index: Span,
    pub(crate) messages: Span,
    pub(crate) names: Span,
    plural_cardinal: Span,
    plural_ordinal: Span,
    /// FUNCS section, first entry offset, entry count. Names are resolved
    /// against the registry when a function is called (a few short string
    /// compares; no table to allocate).
    funcs: Span,
    funcs_at: usize,
    nfuncs: usize,
}

/// One INDEX lookup.
pub enum Entry<'a> {
    Simple(&'a str),
    /// Offset of the message in MESSAGES.
    Pattern(usize),
    Select(usize),
    Absent,
}

pub(crate) fn u16le(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes(s.try_into().ok()?))
}

pub(crate) fn u32le(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(s.try_into().ok()?))
}

/// Byte cursor with LEB128 varints (≤ 5 bytes, value ≤ u32::MAX).
pub(crate) struct Cur<'a> {
    pub b: &'a [u8],
    pub i: usize,
}

impl<'a> Cur<'a> {
    pub fn new(b: &'a [u8], i: usize) -> Self {
        Cur { b, i }
    }
    pub fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.i)?;
        self.i = self.i.checked_add(1)?;
        Some(v)
    }
    pub fn var(&mut self) -> Option<u32> {
        let mut v: u32 = 0;
        let mut shift: u32 = 0;
        loop {
            let b = self.u8()?;
            v |= u32::from(b & 0x7f).checked_shl(shift)?;
            if b & 0x80 == 0 {
                return Some(v);
            }
            shift = shift.checked_add(7)?;
            if shift > 28 {
                return None;
            }
        }
    }
    pub fn usize(&mut self) -> Option<usize> {
        self.var().map(|v| v as usize)
    }
    pub fn skip(&mut self, n: usize) -> Option<()> {
        let j = self.i.checked_add(n)?;
        if j > self.b.len() {
            return None;
        }
        self.i = j;
        Some(())
    }
}

impl Catalog {
    /// Splits off and validates the buffer once: magic, version, layout,
    /// manifest hash (F6), section table, INDEX monotonicity, LOCALE and FUNCS
    /// tables, and one UTF-8 pass over the pool.
    #[cfg_attr(feature = "utf8-per-access", allow(unused_mut))]
    pub fn new(mut bytes: Vec<u8>, expect_manifest: u64) -> Result<Catalog, CatalogError> {
        let b = bytes.as_slice();
        if b.get(..4) != Some(&f::MAGIC[..]) {
            return Err(CatalogError::Magic);
        }
        if u16le(b, header::VERSION) != Some(f::VERSION) {
            return Err(CatalogError::Version);
        }
        let fl = u16le(b, header::FLAGS).ok_or(CatalogError::Header)?;
        if fl & flags::STR_MASK != flags::STR_NUL || fl & flags::INDEX_MASK != flags::INDEX_PLANES || fl & flags::REL_REFS != 0 {
            return Err(CatalogError::Layout);
        }
        let hash = b.get(header::MANIFEST_HASH..header::CHUNK).and_then(|s| s.try_into().ok()).map(u64::from_le_bytes);
        if hash != Some(expect_manifest) {
            return Err(CatalogError::ManifestMismatch);
        }
        let dir = match b.get(header::DIR) {
            Some(0) => Dir::Ltr,
            Some(1) => Dir::Rtl,
            _ => return Err(CatalogError::Header),
        };
        let count = u32le(b, header::MESSAGE_COUNT).ok_or(CatalogError::Header)?;
        if count > kind::OFFSET_MASK >> 6 {
            return Err(CatalogError::Header);
        }
        let locale_off = u32le(b, header::LOCALE_OFF).ok_or(CatalogError::Header)? as usize;
        let locale_len = usize::from(u16le(b, header::LOCALE_LEN).ok_or(CatalogError::Header)?);
        let nsec = usize::from(u16le(b, header::SECTION_COUNT).ok_or(CatalogError::Header)?);
        let table_end = nsec.checked_mul(header::SECTION_ENTRY).and_then(|n| n.checked_add(header::SECTIONS)).ok_or(CatalogError::Header)?;
        if table_end > b.len() {
            return Err(CatalogError::Header);
        }
        let mut sp = [None::<Span>; 16];
        let mut prev_end = table_end;
        let mut pool_start = None;
        for s in 0..nsec {
            let at = s.checked_mul(header::SECTION_ENTRY).and_then(|n| n.checked_add(header::SECTIONS)).ok_or(CatalogError::Header)?;
            let k = u16le(b, at).ok_or(CatalogError::Header)?;
            let off = u32le(b, at.checked_add(2).ok_or(CatalogError::Header)?).ok_or(CatalogError::Header)? as usize;
            let len = u32le(b, at.checked_add(6).ok_or(CatalogError::Header)?).ok_or(CatalogError::Header)? as usize;
            let end = off.checked_add(len).ok_or(CatalogError::Sections)?;
            if off < prev_end || end > b.len() || pool_start.is_some() {
                return Err(CatalogError::Sections);
            }
            prev_end = end;
            if k == section::STRINGS {
                if end != b.len() {
                    return Err(CatalogError::Sections);
                }
                pool_start = Some(off);
            }
            if let Some(slot) = sp.get_mut(usize::from(k)) {
                if slot.is_some() {
                    return Err(CatalogError::Sections);
                }
                *slot = Some(Span { off, len });
            } // unknown kinds are skipped
        }
        let get = |k: u16| sp.get(usize::from(k)).copied().flatten();
        let need = |k: u16| get(k).ok_or(CatalogError::MissingSection);
        let (index, messages, names, funcs_sec, locale_sec) =
            (need(section::INDEX)?, need(section::MESSAGES)?, need(section::NAMES)?, need(section::FUNCS)?, need(section::LOCALE)?);
        let pool_start = pool_start.ok_or(CatalogError::MissingSection)?;
        let pool_len = b.len().checked_sub(pool_start).ok_or(CatalogError::Sections)?;

        // INDEX: size, bounds, and strictly increasing MESSAGES offsets.
        if Some(index.len) != (count as usize).checked_mul(4) {
            return Err(CatalogError::Index);
        }
        let idx = b.get(index.off..index.off.checked_add(index.len).ok_or(CatalogError::Index)?).ok_or(CatalogError::Index)?;
        let mut prev: Option<u32> = None;
        for i in 0..count as usize {
            let e = plane_entry(idx, count as usize, i).ok_or(CatalogError::Index)?;
            let p = e & kind::OFFSET_MASK;
            match e >> kind::SHIFT {
                kind::SIMPLE => {
                    if p as usize >= pool_len {
                        return Err(CatalogError::Index);
                    }
                }
                kind::PATTERN | kind::SELECT => {
                    if p as usize >= messages.len || prev.is_some_and(|q| p <= q) {
                        return Err(CatalogError::Index);
                    }
                    prev = Some(p);
                }
                _ => {}
            }
        }

        // LOCALE container: find the plural entries.
        let (mut plural_cardinal, mut plural_ordinal) = (Span::default(), Span::default());
        let loc = b.get(locale_sec.off..locale_sec.off.checked_add(locale_sec.len).ok_or(CatalogError::Tables)?).ok_or(CatalogError::Tables)?;
        let mut c = Cur::new(loc, 0);
        let n = c.var().ok_or(CatalogError::Tables)?;
        for _ in 0..n {
            let key = c.var().ok_or(CatalogError::Tables)?;
            let len = c.usize().ok_or(CatalogError::Tables)?;
            let span = Span { off: locale_sec.off.checked_add(c.i).ok_or(CatalogError::Tables)?, len };
            c.skip(len).ok_or(CatalogError::Tables)?;
            match key {
                locale_key::PLURAL_CARDINAL => plural_cardinal = span,
                locale_key::PLURAL_ORDINAL => plural_ordinal = span,
                _ => {}
            }
        }
        // FUNCS: offsets now, names after the pool is validated.
        let fs = b.get(funcs_sec.off..funcs_sec.off.checked_add(funcs_sec.len).ok_or(CatalogError::Tables)?).ok_or(CatalogError::Tables)?;
        let mut fc = Cur::new(fs, 0);
        let nf = fc.usize().ok_or(CatalogError::Tables)?;
        if nf > fs.len() {
            return Err(CatalogError::Tables);
        }
        let funcs_at = fc.i;
        // Walk the table once for structure.
        for _ in 0..nf {
            fc.var().ok_or(CatalogError::Tables)?;
        }

        // Eager UTF-8: copy the structure (the small head) out, move the pool
        // to the front of the fetched buffer, and validate it once as UTF-8
        // (fallible allocation: failure is an error, not a panic path).
        #[cfg(not(feature = "utf8-per-access"))]
        let (head, pool) = {
            let mut head: Vec<u8> = Vec::new();
            head.try_reserve_exact(pool_start).map_err(|_| CatalogError::Alloc)?;
            head.extend_from_slice(b.get(..pool_start).ok_or(CatalogError::Sections)?);
            let total = bytes.len();
            if pool_start > total {
                return Err(CatalogError::Sections);
            }
            bytes.copy_within(pool_start..total, 0);
            bytes.truncate(pool_len);
            (head, alloc::string::String::from_utf8(bytes).map_err(|_| CatalogError::Utf8)?)
        };
        // Per-access UTF-8: keep the buffer as it is — no copy, no allocation.
        #[cfg(feature = "utf8-per-access")]
        let head = bytes;

        let cat = Catalog {
            head,
            #[cfg(not(feature = "utf8-per-access"))]
            pool,
            #[cfg(feature = "utf8-per-access")]
            pool_start,
            count,
            dir,
            locale: Span { off: locale_off, len: locale_len },
            index,
            messages,
            names,
            plural_cardinal,
            plural_ordinal,
            funcs: funcs_sec,
            funcs_at,
            nfuncs: nf,
        };
        if cat.pool_str(locale_off, locale_len).is_none() {
            return Err(CatalogError::Header);
        }
        let mut fc = Cur::new(cat.section(funcs_sec), funcs_at);
        for _ in 0..nf {
            let off = fc.usize().ok_or(CatalogError::Tables)?;
            cat.str_at(off).ok_or(CatalogError::Tables)?;
        }
        Ok(cat)
    }

    pub(crate) fn section(&self, s: Span) -> &[u8] {
        match s.off.checked_add(s.len) {
            Some(end) => self.head.get(s.off..end).unwrap_or(&[]),
            None => &[],
        }
    }

    #[cfg(not(feature = "utf8-per-access"))]
    fn pool_str(&self, off: usize, len: usize) -> Option<&str> {
        self.pool.get(off..off.checked_add(len)?)
    }

    #[cfg(feature = "utf8-per-access")]
    fn pool_bytes(&self) -> &[u8] {
        self.head.get(self.pool_start..).unwrap_or(&[])
    }

    #[cfg(feature = "utf8-per-access")]
    fn pool_str(&self, off: usize, len: usize) -> Option<&str> {
        core::str::from_utf8(self.pool_bytes().get(off..off.checked_add(len)?)?).ok()
    }

    /// The NUL-terminated pool string at `off` (a StrRef).
    #[cfg(not(feature = "utf8-per-access"))]
    #[inline]
    pub(crate) fn str_at(&self, off: usize) -> Option<&str> {
        let rest = self.pool.get(off..)?;
        let end = nul_pos(rest.as_bytes())?;
        rest.get(..end)
    }

    #[cfg(feature = "utf8-per-access")]
    #[inline]
    pub(crate) fn str_at(&self, off: usize) -> Option<&str> {
        let rest = self.pool_bytes().get(off..)?;
        let end = nul_pos(rest)?;
        core::str::from_utf8(rest.get(..end)?).ok()
    }

    pub fn locale(&self) -> &str {
        self.pool_str(self.locale.off, self.locale.len).unwrap_or("")
    }

    pub fn dir(&self) -> Dir {
        self.dir
    }

    pub fn len(&self) -> u32 {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// O(1) lookup (F3). Never fails; a bad offset reads as `Absent`.
    #[inline]
    pub fn get(&self, id: MsgId) -> Entry<'_> {
        if id.0 >> 24 != 0 || id.0 >= self.count {
            return Entry::Absent;
        }
        let Some(e) = plane_entry(self.section(self.index), self.count as usize, id.0 as usize) else {
            return Entry::Absent;
        };
        let p = (e & kind::OFFSET_MASK) as usize;
        match e >> kind::SHIFT {
            kind::SIMPLE => self.str_at(p).map_or(Entry::Absent, Entry::Simple),
            kind::PATTERN => Entry::Pattern(p),
            kind::SELECT => Entry::Select(p),
            _ => Entry::Absent,
        }
    }

    /// Registry id and name of function-table entry `idx`.
    pub(crate) fn func(&self, idx: usize) -> (FnId, &str) {
        if idx >= self.nfuncs {
            return (FnId::Unknown, "");
        }
        let mut c = Cur::new(self.section(self.funcs), self.funcs_at);
        for _ in 0..idx {
            if c.var().is_none() {
                return (FnId::Unknown, "");
            }
        }
        match c.usize().and_then(|o| self.str_at(o)) {
            Some(name) => (FnId::from_name(name), name),
            None => (FnId::Unknown, ""),
        }
    }

    pub(crate) fn plural(&self, ordinal: bool) -> &[u8] {
        self.section(if ordinal { self.plural_ordinal } else { self.plural_cardinal })
    }

    /// Name of external slot / local `idx` from a NAMES entry (`names_ref` ≥ 1).
    pub(crate) fn name(&self, names_ref: usize, local: bool, idx: usize) -> Option<&str> {
        let mut c = Cur::new(self.section(self.names), names_ref.checked_sub(1)?);
        let n_ext = c.usize()?;
        let target = if local {
            for _ in 0..n_ext {
                c.var()?;
            }
            let n_loc = c.usize()?;
            if idx >= n_loc {
                return None;
            }
            idx
        } else {
            if idx >= n_ext {
                return None;
            }
            idx
        };
        for _ in 0..target {
            c.var()?;
        }
        self.str_at(c.usize()?)
    }
}

/// Position of the first NUL byte, eight bytes at a time (SWAR). Hand-written
/// because `str::find` keeps a slice-index panic path alive (B12).
#[inline]
fn nul_pos(b: &[u8]) -> Option<usize> {
    const LO: u64 = 0x0101_0101_0101_0101;
    const HI: u64 = 0x8080_8080_8080_8080;
    let mut chunks = b.chunks_exact(8);
    let mut base = 0usize;
    for c in &mut chunks {
        let v = u64::from_le_bytes(c.try_into().ok()?);
        // Lowest flagged byte is the first zero byte (little-endian).
        let z = v.wrapping_sub(LO) & !v & HI;
        if z != 0 {
            return base.checked_add((z.trailing_zeros() / 8) as usize);
        }
        base = base.wrapping_add(8);
    }
    chunks.remainder().iter().position(|&x| x == 0).and_then(|p| base.checked_add(p))
}

/// Entry `i` of a byte-plane INDEX with `n` entries.
#[inline]
fn plane_entry(idx: &[u8], n: usize, i: usize) -> Option<u32> {
    let b0 = *idx.get(i)?;
    let b1 = *idx.get(n.checked_add(i)?)?;
    let b2 = *idx.get(n.checked_mul(2)?.checked_add(i)?)?;
    let b3 = *idx.get(n.checked_mul(3)?.checked_add(i)?)?;
    Some(u32::from_le_bytes([b0, b1, b2, b3]))
}
