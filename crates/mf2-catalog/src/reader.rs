//! The client reader: [`Catalog::new`] validates the structure once, in one
//! linear pass per section (F4); every accessor after that is a bounds-checked
//! O(1) read (O(log n) for [`Catalog::fallback_locale`] and
//! [`Catalog::lookup`]) that never panics. The fetched buffer *is* the
//! catalog: nothing is copied and nothing is allocated (F2).

use alloc::vec::Vec;

use mf2_model::{Dir, MsgId};

use crate::bytes::{Cur, nul_pos, plane_entry, u16_at, u32_at, u64_at};
use crate::error::CatalogError;
use crate::format::{
    HEADER_LEN, IDS_RESTART, MAGIC, MAX_FALLBACK_LOCALES, MAX_MESSAGES, SECTION_ENTRY_LEN,
    VERSION_MAJOR, flags, header, kind, locale_key, section,
};
use crate::plural;
use crate::view::{MsgView, Names};

/// An opaque reference to a catalog string; [`Catalog::text`] resolves it.
///
/// Nothing outside this crate may assume what it holds (a seam kept for
/// catalog text as JS strings, `plans/stretch_goals_after_v1`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StrRef(pub(crate) u32);

/// The CLDR version of a catalog's locale data.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CldrVersion {
    /// The release (`48` of CLDR 48.2.1).
    pub major: u16,
    /// The minor version (`2`).
    pub minor: u8,
    /// The patch (`1`).
    pub patch: u8,
}

impl CldrVersion {
    /// The header encoding: `major << 16 | minor << 8 | patch`.
    #[doc(hidden)]
    pub const fn to_u32(self) -> u32 {
        (self.major as u32) << 16 | (self.minor as u32) << 8 | self.patch as u32
    }

    /// From the header encoding; `None` for 0 (no CLDR data).
    #[allow(clippy::cast_possible_truncation)] // the fields are bit ranges of `v`
    #[doc(hidden)]
    pub const fn from_u32(v: u32) -> Option<Self> {
        if v == 0 {
            return None;
        }
        Some(CldrVersion {
            major: (v >> 16) as u16,
            minor: (v >> 8) as u8,
            patch: v as u8,
        })
    }
}

/// One INDEX lookup.
#[derive(Clone, Copy, Debug)]
pub enum Entry<'a> {
    /// A single text run: resolve with [`Catalog::text`]. The evaluator is
    /// not entered.
    Simple(StrRef),
    /// One pattern with placeholders or markup, maybe declarations.
    Pattern(MsgView<'a>),
    /// A `.match` message.
    Select(MsgView<'a>),
    /// The manifest has the id; this catalog (or chunk) does not carry it.
    Absent,
}

/// Offset and length of a section inside the buffer.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct Span {
    pub(crate) off: usize,
    pub(crate) len: usize,
}

impl Span {
    #[inline]
    pub(crate) fn of<'a>(&self, b: &'a [u8]) -> &'a [u8] {
        match self.off.checked_add(self.len) {
            Some(end) => b.get(self.off..end).unwrap_or(&[]),
            None => &[],
        }
    }
}

/// A validated `.mf2b` catalog (F2): the fetched buffer and the offsets
/// `new` found. Share it as `Rc<Catalog>` / `Arc<Catalog>`.
pub struct Catalog {
    bytes: Bytes,
    version: u16,
    flags: u16,
    hash: u64,
    count: u32,
    locale: u32,
    cldr: u32,
    chunk: u8,
    dir: Dir,
    pub(crate) index: Span,
    pub(crate) messages: Span,
    /// Read by the decoder only (the client never reads COLD).
    #[cfg_attr(not(feature = "decode"), allow(dead_code))]
    pub(crate) cold: Option<Span>,
    pub(crate) names: Span,
    fallback: Option<Fallback>,
    locale_sec: Span,
    plural: [Option<Span>; 2],
    funcs: Span,
    ids: Option<Span>,
    pub(crate) strings: Span,
}

/// A catalog's buffer. Without `static-bytes` it is the fetched `Vec`, as
/// it always was, so the client — which never turns the feature on — pays
/// nothing for it (`cargo xtask size`, 2026-09-27: the reference app's raw
/// wasm 6 bytes smaller; an unconditional owned-or-static enum cost it
/// 392).
#[cfg(not(feature = "static-bytes"))]
type Bytes = Vec<u8>;

/// A catalog's buffer: fetched (owned), or part of the program itself (an
/// embedded catalog, never copied) — `static-bytes`, which only a native
/// application turns on.
#[cfg(feature = "static-bytes")]
enum Bytes {
    Owned(Vec<u8>),
    Static(&'static [u8]),
}

#[cfg(feature = "static-bytes")]
impl Bytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            Bytes::Owned(v) => v,
            Bytes::Static(b) => b,
        }
    }
}

#[cfg(feature = "static-bytes")]
impl core::ops::Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

/// FALLBACK: the locale table (`str32` × n) and the entries (`u32` each).
#[derive(Clone, Copy)]
struct Fallback {
    locales: Span,
    entries: Span,
}

impl core::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Catalog")
            .field("locale", &self.locale())
            .field("messages", &self.count)
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// The known sections found by the table walk.
#[derive(Default)]
struct Found {
    index: Option<Span>,
    messages: Option<Span>,
    cold: Option<Span>,
    names: Option<Span>,
    fallback: Option<Span>,
    locale: Option<Span>,
    funcs: Option<Span>,
    ids: Option<Span>,
    strings: Option<Span>,
}

impl Found {
    fn slot(&mut self, kind: u16) -> Option<&mut Option<Span>> {
        Some(match kind {
            section::INDEX => &mut self.index,
            section::MESSAGES => &mut self.messages,
            section::COLD => &mut self.cold,
            section::NAMES => &mut self.names,
            section::FALLBACK => &mut self.fallback,
            section::LOCALE => &mut self.locale,
            section::FUNCS => &mut self.funcs,
            section::IDS => &mut self.ids,
            section::STRINGS => &mut self.strings,
            _ => return None,
        })
    }
}

impl Catalog {
    /// Takes ownership of a fetched `.mf2b` buffer and validates its
    /// structure once: magic, version (F9), `manifest_hash` (F6), the section
    /// table, INDEX bounds and monotonicity, and the NAMES, FUNCS, FALLBACK,
    /// LOCALE (with its plural entries) and IDS tables. Strings are checked
    /// when read (F4). Linear in the buffer; allocates nothing; no copy (F2).
    pub fn new(bytes: Vec<u8>, expect_manifest: u64) -> Result<Catalog, CatalogError> {
        #[cfg(feature = "static-bytes")]
        let bytes = Bytes::Owned(bytes);
        Catalog::validate(bytes, expect_manifest)
    }

    /// As [`Catalog::new`], over bytes that live as long as the program —
    /// a catalog embedded with `include_bytes!` — without copying them
    /// (feature `static-bytes`).
    #[cfg(feature = "static-bytes")]
    pub fn from_static(
        bytes: &'static [u8],
        expect_manifest: u64,
    ) -> Result<Catalog, CatalogError> {
        Catalog::validate(Bytes::Static(bytes), expect_manifest)
    }

    // Always inlined: `new` is the client's only caller, and a separate
    // function costs its wasm (`cargo xtask size`).
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn validate(bytes: Bytes, expect_manifest: u64) -> Result<Catalog, CatalogError> {
        let b = bytes.as_slice();
        if b.get(..4) != Some(&MAGIC[..]) {
            return Err(CatalogError::Magic);
        }
        let version = u16_at(b, header::VERSION).ok_or(CatalogError::Truncated)?;
        if version >> 8 != VERSION_MAJOR {
            return Err(CatalogError::Version);
        }
        if b.len() < HEADER_LEN {
            return Err(CatalogError::Truncated);
        }
        let hash = u64_at(b, header::MANIFEST_HASH).ok_or(CatalogError::Truncated)?;
        if hash != expect_manifest {
            return Err(CatalogError::ManifestMismatch);
        }
        let get16 = |at| u16_at(b, at).ok_or(CatalogError::Truncated);
        let get32 = |at| u32_at(b, at).ok_or(CatalogError::Truncated);
        let flags = get16(header::FLAGS)?;
        let count = get32(header::MESSAGE_COUNT)?;
        let locale = get32(header::LOCALE)?;
        let cldr = get32(header::CLDR_VERSION)?;
        let chunk = *b.get(header::CHUNK).ok_or(CatalogError::Truncated)?;
        let dir = match b.get(header::DIR) {
            Some(0) => Dir::Ltr,
            Some(1) => Dir::Rtl,
            _ => return Err(CatalogError::Header),
        };
        if count > MAX_MESSAGES {
            return Err(CatalogError::Header);
        }
        let found = sections(b)?;
        let need = |s: Option<Span>| s.ok_or(CatalogError::MissingSection);
        let index = need(found.index)?;
        let messages = need(found.messages)?;
        let names = need(found.names)?;
        let locale_sec = need(found.locale)?;
        let funcs = need(found.funcs)?;
        let strings = need(found.strings)?;
        let pool = strings.of(b);
        if pool.last() != Some(&0) {
            return Err(CatalogError::Strings);
        }
        let in_pool = |r: u32| (r as usize) < pool.len();
        if !in_pool(locale) {
            return Err(CatalogError::Header);
        }
        check_index(index.of(b), count as usize, messages.len, pool.len())?;
        check_names(names.of(b), pool.len()).ok_or(CatalogError::Names)?;
        check_funcs(funcs.of(b), pool.len()).ok_or(CatalogError::Funcs)?;
        let fallback = match found.fallback {
            Some(s) => Some(check_fallback(b, s, count, pool.len()).ok_or(CatalogError::Fallback)?),
            None => None,
        };
        let plural = check_locale(b, locale_sec).ok_or(CatalogError::Locale)?;
        if let Some(ids) = found.ids {
            check_ids(ids.of(b), count as usize).ok_or(CatalogError::Ids)?;
        }
        Ok(Catalog {
            version,
            flags,
            hash,
            count,
            locale,
            cldr,
            chunk,
            dir,
            index,
            messages,
            cold: found.cold,
            names,
            fallback,
            locale_sec,
            plural,
            funcs,
            ids: found.ids,
            strings,
            bytes,
        })
    }

    /// `format_version`: `major << 8 | minor`.
    #[doc(hidden)]
    pub fn format_version(&self) -> u16 {
        self.version
    }

    /// The manifest hash this catalog was compiled against (F6).
    pub fn manifest_hash(&self) -> u64 {
        self.hash
    }

    /// The BCP 47 tag (F7).
    pub fn locale(&self) -> &str {
        self.text(StrRef(self.locale)).unwrap_or("")
    }

    /// The locale's direction (F7): `Ltr` or `Rtl`.
    pub fn dir(&self) -> Dir {
        self.dir
    }

    /// The `MsgId` chunk this catalog holds (0 until chunking).
    #[doc(hidden)]
    pub fn chunk(&self) -> u8 {
        self.chunk
    }

    /// The CLDR version of the locale data, if any.
    pub fn cldr_version(&self) -> Option<CldrVersion> {
        CldrVersion::from_u32(self.cldr)
    }

    /// Whether COLD was stripped (production catalogs, §2.3).
    #[doc(hidden)]
    pub fn cold_stripped(&self) -> bool {
        self.flags & flags::COLD_STRIPPED != 0
    }

    /// Whether IDS was stripped (production catalogs, §2.3).
    #[doc(hidden)]
    pub fn ids_stripped(&self) -> bool {
        self.flags & flags::IDS_STRIPPED != 0
    }

    /// The number of ids in the manifest (INDEX entries).
    pub fn message_count(&self) -> u32 {
        self.count
    }

    /// The raw bytes (for serving, hashing, measuring).
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Gives the buffer back (a copy, for a catalog over static bytes).
    pub fn into_bytes(self) -> Vec<u8> {
        #[cfg(not(feature = "static-bytes"))]
        return self.bytes;
        #[cfg(feature = "static-bytes")]
        match self.bytes {
            Bytes::Owned(v) => v,
            Bytes::Static(b) => b.to_vec(),
        }
    }

    /// The section table: `(kind, offset, length)` in file order, unknown
    /// kinds included (for tools: sizes, dumps).
    #[doc(hidden)]
    pub fn sections(&self) -> impl Iterator<Item = (u16, u32, u32)> + '_ {
        let n = u16_at(&self.bytes, header::SECTION_COUNT).unwrap_or(0);
        (0..usize::from(n)).filter_map(move |i| {
            let at = HEADER_LEN.checked_add(i.checked_mul(SECTION_ENTRY_LEN)?)?;
            Some((
                u16_at(&self.bytes, at)?,
                u32_at(&self.bytes, at.checked_add(2)?)?,
                u32_at(&self.bytes, at.checked_add(6)?)?,
            ))
        })
    }

    /// O(1) lookup by id (F3). Never fails: an id outside this catalog (or
    /// chunk) is `Absent`.
    #[inline]
    #[doc(hidden)]
    pub fn get(&self, id: MsgId) -> Entry<'_> {
        if id.chunk() != self.chunk || id.index() >= self.count {
            return Entry::Absent;
        }
        let Some(e) = plane_entry(
            self.index.of(&self.bytes),
            self.count as usize,
            id.index() as usize,
        ) else {
            return Entry::Absent;
        };
        let off = e & kind::OFFSET_MASK;
        match e >> kind::SHIFT {
            kind::SIMPLE => Entry::Simple(StrRef(off)),
            kind::PATTERN => Entry::Pattern(MsgView::new(self, off as usize, false)),
            kind::SELECT => Entry::Select(MsgView::new(self, off as usize, true)),
            _ => Entry::Absent,
        }
    }

    /// The string `r` refers to; `None` if it is out of bounds or not valid
    /// UTF-8 (F4: checked on access, so a corrupt string costs only the
    /// message that uses it).
    #[inline]
    #[doc(hidden)]
    pub fn text(&self, r: StrRef) -> Option<&str> {
        let rest = self.strings.of(&self.bytes).get(r.0 as usize..)?;
        let end = nul_pos(rest)?;
        core::str::from_utf8(rest.get(..end)?).ok()
    }

    /// The locale a message's text came from, when it is not this catalog's
    /// own (F7). O(log n).
    #[doc(hidden)]
    pub fn fallback_locale(&self, id: MsgId) -> Option<&str> {
        let fb = self.fallback?;
        if id.chunk() != self.chunk {
            return None;
        }
        let entries = fb.entries.of(&self.bytes);
        let target = id.index();
        let (mut lo, mut hi) = (0usize, entries.len() / 4);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let e = u32_at(entries, mid.checked_mul(4)?)?;
            match (e & 0x00ff_ffff).cmp(&target) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => {
                    let loc = u32_at(fb.locales.of(&self.bytes), ((e >> 24) as usize) * 4)?;
                    return self.text(StrRef(loc));
                }
            }
        }
        None
    }

    /// Entry `index` of FUNCS: a function identifier (`ns:name`, NFC).
    #[doc(hidden)]
    pub fn function(&self, index: u32) -> Option<&str> {
        let at = (index as usize).checked_mul(4)?;
        self.text(StrRef(u32_at(self.funcs.of(&self.bytes), at)?))
    }

    /// The number of FUNCS entries.
    #[doc(hidden)]
    pub fn function_count(&self) -> u32 {
        u32::try_from(self.funcs.len / 4).unwrap_or(u32::MAX)
    }

    /// The payload of the LOCALE entry with `key` (opaque; §2.7, §4).
    #[doc(hidden)]
    pub fn locale_entry(&self, key: u32) -> Option<&[u8]> {
        match key {
            locale_key::PLURAL_CARDINAL => return self.plural[0].map(|s| s.of(&self.bytes)),
            locale_key::PLURAL_ORDINAL => return self.plural[1].map(|s| s.of(&self.bytes)),
            _ => {}
        }
        let mut c = Cur::new(self.locale_sec.of(&self.bytes), 0);
        let n = c.varint()?;
        for _ in 0..n {
            let k = c.varint()?;
            let len = c.len()?;
            let payload = c.take(len)?;
            if k == key {
                return Some(payload);
            }
            if k > key {
                return None;
            }
        }
        None
    }

    /// A message's variable names (NAMES): its slots and its locals. Empty
    /// for simple and absent messages.
    #[doc(hidden)]
    pub fn names(&self, id: MsgId) -> Names<'_> {
        match self.get(id) {
            Entry::Pattern(m) | Entry::Select(m) => m.names(),
            Entry::Simple(_) | Entry::Absent => Names::EMPTY,
        }
    }

    /// The name of message `id` (IDS); `None` when IDS is stripped or the
    /// index is past the last message.
    ///
    /// Build side (feature `decode`): a client formats by `MsgId` and never
    /// needs an id back, so this is not on its path. `mf2 dump` and the
    /// tooling that reports on a catalog do.
    #[cfg(feature = "decode")]
    #[doc(hidden)]
    pub fn id_of(&self, id: MsgId) -> Option<alloc::string::String> {
        let index = id.index() as usize;
        let count = self.count as usize;
        if index >= count {
            return None;
        }
        let ids = self.ids?.of(&self.bytes);
        let blocks = count.div_ceil(IDS_RESTART);
        let table_len = blocks.checked_mul(4)?;
        let entries = ids.get(table_len..)?;
        let block = index / IDS_RESTART;
        let at = u32_at(ids, block.checked_mul(4)?)? as usize;
        let mut c = Cur::new(entries, at);
        // Ids are prefix-compressed against the one before them, so the id
        // is rebuilt from the start of its restart block.
        let mut current: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        for i in (block * IDS_RESTART)..=index {
            let shared = c.len()?;
            let len = c.len()?;
            let suffix = c.take(len)?;
            if shared > current.len() {
                return None;
            }
            current.truncate(shared);
            current.extend_from_slice(suffix);
            if i == index {
                return alloc::string::String::from_utf8(current).ok();
            }
        }
        None
    }

    /// Looks a message id up by name (IDS); `None` when IDS is stripped or
    /// the id is unknown. O(log n).
    pub fn lookup(&self, id: &str) -> Option<MsgId> {
        let ids = self.ids?.of(&self.bytes);
        let count = self.count as usize;
        let blocks = count.div_ceil(IDS_RESTART);
        let table_len = blocks.checked_mul(4)?;
        let entries = ids.get(table_len..)?;
        let key = id.as_bytes();
        // The last restart whose id is ≤ key.
        let (mut lo, mut hi) = (0usize, blocks);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let at = u32_at(ids, mid.checked_mul(4)?)? as usize;
            let mut c = Cur::new(entries, at);
            let _shared = c.varint()?;
            let len = c.len()?;
            if c.take(len)? <= key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let block = lo.checked_sub(1)?;
        let at = u32_at(ids, block.checked_mul(4)?)? as usize;
        let mut c = Cur::new(entries, at);
        // Scan the block, rebuilding each id only as far as it matches `key`:
        // `matched` is the length of the common prefix of the previous id
        // and `key`.
        let mut matched = 0usize;
        let first = block.checked_mul(IDS_RESTART)?;
        for i in first..count.min(first.checked_add(IDS_RESTART)?) {
            let shared = c.len()?;
            let len = c.len()?;
            let suffix = c.take(len)?;
            if shared > matched {
                // Shares more with the previous id than that id shared with
                // `key`: it differs from `key` where the previous one did.
                continue;
            }
            // `shared ≤ matched`: the id agrees with `key` on its first
            // `shared` bytes. Fewer than `matched` does not prove a miss:
            // IDS does not require `shared` to be maximal (02 §2.8), so the
            // suffix may repeat bytes of the previous id and still match.
            let rest = key.get(shared..)?;
            let common = rest.iter().zip(suffix).take_while(|(a, b)| a == b).count();
            if common == rest.len() && common == suffix.len() {
                return MsgId::new(self.chunk, u32::try_from(i).ok()?);
            }
            matched = shared.checked_add(common)?;
        }
        None
    }
}

/// Walks the section table: bounds, order, duplicates, STRINGS last.
fn sections(b: &[u8]) -> Result<Found, CatalogError> {
    let n = usize::from(u16_at(b, header::SECTION_COUNT).ok_or(CatalogError::Truncated)?);
    let table_end = n
        .checked_mul(SECTION_ENTRY_LEN)
        .and_then(|t| t.checked_add(HEADER_LEN))
        .ok_or(CatalogError::Truncated)?;
    if table_end > b.len() {
        return Err(CatalogError::Truncated);
    }
    let mut found = Found::default();
    let mut prev_end = table_end;
    let mut last = None;
    for i in 0..n {
        let at = HEADER_LEN + i * SECTION_ENTRY_LEN;
        let (Some(kind), Some(off), Some(len)) =
            (u16_at(b, at), u32_at(b, at + 2), u32_at(b, at + 6))
        else {
            return Err(CatalogError::Truncated);
        };
        let (off, len) = (off as usize, len as usize);
        let end = off.checked_add(len).ok_or(CatalogError::SectionTable)?;
        if off < prev_end || end > b.len() || found.strings.is_some() {
            return Err(CatalogError::SectionTable);
        }
        prev_end = end;
        last = Some(end);
        if let Some(slot) = found.slot(kind) {
            if slot.is_some() {
                return Err(CatalogError::SectionTable);
            }
            *slot = Some(Span { off, len });
        }
    }
    if found.strings.is_some() && last != Some(b.len()) {
        return Err(CatalogError::SectionTable);
    }
    Ok(found)
}

/// INDEX: size, bounds, and strictly increasing MESSAGES offsets.
fn check_index(
    index: &[u8],
    count: usize,
    messages_len: usize,
    pool_len: usize,
) -> Result<(), CatalogError> {
    if Some(index.len()) != count.checked_mul(4) {
        return Err(CatalogError::Index);
    }
    let mut prev: Option<u32> = None;
    for i in 0..count {
        let e = plane_entry(index, count, i).ok_or(CatalogError::Index)?;
        let off = e & kind::OFFSET_MASK;
        match e >> kind::SHIFT {
            kind::SIMPLE => {
                if off as usize >= pool_len {
                    return Err(CatalogError::Index);
                }
            }
            kind::PATTERN | kind::SELECT => {
                if off as usize >= messages_len || prev.is_some_and(|p| off <= p) {
                    return Err(CatalogError::Index);
                }
                prev = Some(off);
            }
            _ => {}
        }
    }
    Ok(())
}

/// NAMES: entries back to back, every `str32` inside the pool.
fn check_names(names: &[u8], pool_len: usize) -> Option<()> {
    let mut c = Cur::new(names, 0);
    while !c.at_end() {
        let n = c.len()?.checked_add(c.len()?)?;
        if n > c.remaining() / 4 {
            return None;
        }
        for _ in 0..n {
            if c.u32()? as usize >= pool_len {
                return None;
            }
        }
    }
    Some(())
}

/// FUNCS: `str32`s inside the pool.
fn check_funcs(funcs: &[u8], pool_len: usize) -> Option<()> {
    if !funcs.len().is_multiple_of(4) {
        return None;
    }
    let mut c = Cur::new(funcs, 0);
    while !c.at_end() {
        if c.u32()? as usize >= pool_len {
            return None;
        }
    }
    Some(())
}

/// FALLBACK: the locale table, then entries strictly increasing by message.
fn check_fallback(buf: &[u8], sec: Span, count: u32, pool_len: usize) -> Option<Fallback> {
    let mut c = Cur::new(sec.of(buf), 0);
    let n_locales = c.len()?;
    if n_locales > MAX_FALLBACK_LOCALES || n_locales > c.remaining() / 4 {
        return None;
    }
    let locales = Span {
        off: sec.off.checked_add(c.pos())?,
        len: n_locales * 4,
    };
    for _ in 0..n_locales {
        if c.u32()? as usize >= pool_len {
            return None;
        }
    }
    let entries = Span {
        off: sec.off.checked_add(c.pos())?,
        len: c.remaining(),
    };
    if !entries.len.is_multiple_of(4) {
        return None;
    }
    let mut prev: Option<u32> = None;
    while !c.at_end() {
        let e = c.u32()?;
        let (msg, loc) = (e & 0x00ff_ffff, (e >> 24) as usize);
        if msg >= count || loc >= n_locales || prev.is_some_and(|p| msg <= p) {
            return None;
        }
        prev = Some(msg);
    }
    Some(Fallback { locales, entries })
}

/// LOCALE: the container, keys strictly increasing; the plural entries
/// walked for structure. Returns the plural entries' spans.
fn check_locale(b: &[u8], s: Span) -> Option<[Option<Span>; 2]> {
    let mut c = Cur::new(s.of(b), 0);
    let n = c.varint()?;
    let mut plural = [None, None];
    let mut prev: Option<u32> = None;
    for _ in 0..n {
        let key = c.varint()?;
        if prev.is_some_and(|p| key <= p) {
            return None;
        }
        prev = Some(key);
        let len = c.len()?;
        let at = c.pos();
        let payload = c.take(len)?;
        let span = Span {
            off: s.off.checked_add(at)?,
            len,
        };
        match key {
            locale_key::PLURAL_CARDINAL | locale_key::PLURAL_ORDINAL => {
                if !plural::valid(payload) {
                    return None;
                }
                if let Some(slot) = plural.get_mut(key as usize - 1) {
                    *slot = Some(span);
                }
            }
            _ => {}
        }
    }
    c.at_end().then_some(plural)
}

/// IDS: restart table, then `count` front-coded ids with a restart every
/// [`IDS_RESTART`].
fn check_ids(ids: &[u8], count: usize) -> Option<()> {
    let blocks = count.div_ceil(IDS_RESTART);
    let table_len = blocks.checked_mul(4)?;
    let table = ids.get(..table_len)?;
    let mut c = Cur::new(ids.get(table_len..)?, 0);
    let mut prev_len = 0usize;
    for i in 0..count {
        let at = c.pos();
        let shared = c.len()?;
        let len = c.len()?;
        if i % IDS_RESTART == 0 {
            if shared != 0 || u32_at(table, (i / IDS_RESTART) * 4)? as usize != at {
                return None;
            }
        } else if shared > prev_len {
            return None;
        }
        c.skip(len)?;
        prev_len = shared.checked_add(len)?;
    }
    c.at_end().then_some(())
}
