//! The canonical-equivalence map a catalog carries (`plan/01` §4.3): the
//! code points whose full canonical decomposition uses only the characters
//! of this catalog's variant keys and argument names, with those
//! decompositions, and the combining classes of the characters involved.
//! The runtime decides canonical equivalence with a key from this map, so no
//! client — and no native binary that does not compile messages — links the
//! full normalization tables.
//!
//! This is the reader side: client-path code, with no allocation and no
//! panic. The builder is `writer::nfc_map` (feature `writer`).
//!
//! The bytes, little-endian, every field fixed width so that a lookup is a
//! binary search and nothing has to be parsed:
//!
//! ```text
//! u32 n_decomp | u32 n_ccc | u32 n_pool
//! n_decomp × 8 B: code point (3 B) | length (1 B) | pool offset (4 B)
//! n_ccc    × 4 B: code point (3 B) | combining class (1 B)
//! n_pool   × 3 B: code point
//! ```
//!
//! A length of 0 means the code point decomposes to itself; the pool holds
//! the longer decompositions, which are already in canonical order. Both
//! tables are sorted by code point, and a code point missing from the
//! decomposition table is one the map does not reach: a string containing it
//! matches no key. Combining class 0 is the default and is not stored.
//!
//! Hangul syllables are not listed — there are 11,172 of them and they
//! decompose by arithmetic (UAX #15), which [`NfcMap::decomposition`] does.

use crate::bytes::u32_at;

/// The three counts.
const HEADER: usize = 12;
/// `code point | length | pool offset`.
const DECOMP_ENTRY: usize = 8;
/// `code point | combining class`.
const CCC_ENTRY: usize = 4;
/// One code point.
const POOL_ENTRY: usize = 3;

/// The first Hangul syllable.
pub(crate) const HANGUL_S_BASE: u32 = 0xAC00;
/// The number of Hangul syllables.
pub(crate) const HANGUL_S_COUNT: u32 = 19 * HANGUL_N_COUNT;
/// The first leading jamo.
const HANGUL_L_BASE: u32 = 0x1100;
/// The first vowel jamo.
const HANGUL_V_BASE: u32 = 0x1161;
/// One below the first trailing jamo (index 0 means "no trailing jamo").
const HANGUL_T_BASE: u32 = 0x11A7;
/// Trailing jamo indices, including 0.
const HANGUL_T_COUNT: u32 = 28;
/// Syllables per leading jamo.
const HANGUL_N_COUNT: u32 = 21 * HANGUL_T_COUNT;

/// A catalog's canonical-equivalence map, read in place.
#[derive(Clone, Copy, Debug)]
pub struct NfcMap<'a> {
    decomp: &'a [u8],
    ccc: &'a [u8],
    pool: &'a [u8],
}

impl NfcMap<'_> {
    /// The map of a catalog whose keys and names no other code point
    /// decomposes into: only an identical string can match a key.
    pub const EMPTY: NfcMap<'static> = NfcMap {
        decomp: &[],
        ccc: &[],
        pool: &[],
    };
}

impl<'a> NfcMap<'a> {
    /// The map in `b`, or `None` if the bytes are not a whole map. Empty
    /// bytes are the empty map: an absent section means there is nothing to
    /// reach.
    pub fn from_bytes(b: &'a [u8]) -> Option<NfcMap<'a>> {
        if b.is_empty() {
            return Some(NfcMap::EMPTY);
        }
        let n_decomp = usize::try_from(u32_at(b, 0)?).ok()?;
        let n_ccc = usize::try_from(u32_at(b, 4)?).ok()?;
        let n_pool = usize::try_from(u32_at(b, 8)?).ok()?;
        let decomp_len = n_decomp.checked_mul(DECOMP_ENTRY)?;
        let ccc_len = n_ccc.checked_mul(CCC_ENTRY)?;
        let pool_len = n_pool.checked_mul(POOL_ENTRY)?;
        let at_ccc = HEADER.checked_add(decomp_len)?;
        let at_pool = at_ccc.checked_add(ccc_len)?;
        let end = at_pool.checked_add(pool_len)?;
        if b.len() != end {
            return None;
        }
        Some(NfcMap {
            decomp: b.get(HEADER..at_ccc)?,
            ccc: b.get(at_ccc..at_pool)?,
            pool: b.get(at_pool..end)?,
        })
    }

    /// Whether the map reaches no code point at all.
    pub fn is_empty(&self) -> bool {
        self.decomp.is_empty()
    }

    /// The map's bytes, as [`NfcMap::from_bytes`] took them (the writer's
    /// section payload).
    pub fn len_bytes(&self) -> usize {
        if self.is_empty() && self.ccc.is_empty() && self.pool.is_empty() {
            0
        } else {
            HEADER + self.decomp.len() + self.ccc.len() + self.pool.len()
        }
    }

    /// `ch`'s full canonical decomposition, in canonical order, or `None`
    /// when the map does not reach `ch` — then no string containing `ch` is
    /// canonically equivalent to any of this catalog's keys or names.
    pub fn decomposition(&self, ch: char) -> Option<DecompChars<'a>> {
        let cp = u32::from(ch);
        if let Some(s) = cp
            .checked_sub(HANGUL_S_BASE)
            .filter(|s| *s < HANGUL_S_COUNT)
        {
            // UAX #15's arithmetic: a syllable is its leading, vowel and
            // optional trailing jamo. Each `from_u32` is a code point in the
            // jamo blocks, so none of them can fail.
            let l = char::from_u32(HANGUL_L_BASE + s / HANGUL_N_COUNT)?;
            let v = char::from_u32(HANGUL_V_BASE + (s % HANGUL_N_COUNT) / HANGUL_T_COUNT)?;
            let t = s % HANGUL_T_COUNT;
            let (third, len) = if t == 0 {
                (l, 2)
            } else {
                (char::from_u32(HANGUL_T_BASE + t)?, 3)
            };
            return Some(DecompChars {
                inner: Inner::Inline {
                    chs: [l, v, third],
                    at: 0,
                    len,
                },
            });
        }
        let at = find(self.decomp, DECOMP_ENTRY, cp)?;
        let len = *self.decomp.get(at.checked_add(3)?)?;
        if len == 0 {
            return Some(DecompChars {
                inner: Inner::Inline {
                    chs: [ch, ch, ch],
                    at: 0,
                    len: 1,
                },
            });
        }
        let off = usize::try_from(u32_at(self.decomp, at.checked_add(4)?)?).ok()?;
        let start = off.checked_mul(POOL_ENTRY)?;
        Some(DecompChars {
            inner: Inner::Pool {
                pool: self.pool,
                at: start,
                left: len,
            },
        })
    }

    /// `ch`'s canonical combining class, 0 when the map does not list it.
    /// Only characters the map reaches are asked for, and every character of
    /// a stored decomposition whose class is not 0 is listed.
    pub fn ccc(&self, ch: char) -> u8 {
        let Some(at) = find(self.ccc, CCC_ENTRY, u32::from(ch)) else {
            return 0;
        };
        at.checked_add(3)
            .and_then(|i| self.ccc.get(i))
            .copied()
            .unwrap_or(0)
    }
}

/// The characters of one code point's decomposition. Cloning one restarts
/// it where it stands, which is how the runtime walks a run of combining
/// marks more than once.
#[derive(Clone, Debug)]
pub struct DecompChars<'a> {
    inner: Inner<'a>,
}

#[derive(Clone, Copy, Debug)]
enum Inner<'a> {
    /// A run of the pool.
    Pool { pool: &'a [u8], at: usize, left: u8 },
    /// A code point that decomposes to itself, or a Hangul syllable.
    Inline { chs: [char; 3], at: u8, len: u8 },
}

impl Iterator for DecompChars<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match &mut self.inner {
            Inner::Pool { pool, at, left } => {
                if *left == 0 {
                    return None;
                }
                let ch = code_point(pool, *at).and_then(char::from_u32)?;
                *at = at.checked_add(POOL_ENTRY)?;
                *left -= 1;
                Some(ch)
            }
            Inner::Inline { chs, at, len } => {
                if at >= len {
                    return None;
                }
                let ch = *chs.get(usize::from(*at))?;
                *at += 1;
                Some(ch)
            }
        }
    }
}

/// The three-byte code point at `at`.
fn code_point(b: &[u8], at: usize) -> Option<u32> {
    match b.get(at..at.checked_add(POOL_ENTRY)?)? {
        &[x0, x1, x2] => Some(u32::from_le_bytes([x0, x1, x2, 0])),
        _ => None,
    }
}

/// The offset of the record for `cp` in a table of `stride`-byte records
/// sorted by their leading three-byte code point.
fn find(b: &[u8], stride: usize, cp: u32) -> Option<usize> {
    let mut lo = 0usize;
    let mut hi = b.len() / stride;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let at = mid.checked_mul(stride)?;
        let key = code_point(b, at)?;
        match key.cmp(&cp) {
            core::cmp::Ordering::Less => lo = mid.checked_add(1)?,
            core::cmp::Ordering::Greater => hi = mid,
            core::cmp::Ordering::Equal => return Some(at),
        }
    }
    None
}
