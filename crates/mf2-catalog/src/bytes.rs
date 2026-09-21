//! Panic-free byte access for the reader: little-endian integers from
//! unaligned slices (F5), a cursor with minimal LEB128 varints, the NUL scan
//! and the byte-plane INDEX read. Every read returns `None` when out of
//! bounds or malformed; nothing here can panic (B12).

/// `u16` at `at`, little-endian.
#[inline]
pub(crate) fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    match b.get(at..at.checked_add(2)?)? {
        &[x0, x1] => Some(u16::from_le_bytes([x0, x1])),
        _ => None,
    }
}

/// `u32` at `at`, little-endian.
#[inline]
pub(crate) fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    match b.get(at..at.checked_add(4)?)? {
        &[x0, x1, x2, x3] => Some(u32::from_le_bytes([x0, x1, x2, x3])),
        _ => None,
    }
}

/// `u64` at `at`, little-endian.
#[inline]
pub(crate) fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    match b.get(at..at.checked_add(8)?)? {
        &[x0, x1, x2, x3, x4, x5, x6, x7] => {
            Some(u64::from_le_bytes([x0, x1, x2, x3, x4, x5, x6, x7]))
        }
        _ => None,
    }
}

/// A read position in a byte slice.
#[derive(Clone, Copy)]
pub(crate) struct Cur<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    #[inline]
    pub(crate) const fn new(b: &'a [u8], pos: usize) -> Self {
        Cur { b, pos }
    }

    #[inline]
    pub(crate) const fn pos(&self) -> usize {
        self.pos
    }

    #[inline]
    pub(crate) const fn bytes(&self) -> &'a [u8] {
        self.b
    }

    #[inline]
    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.b.len()
    }

    #[inline]
    pub(crate) fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.pos)
    }

    #[inline]
    pub(crate) fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.pos)?;
        self.pos = self.pos.checked_add(1)?;
        Some(v)
    }

    #[inline]
    pub(crate) fn u32(&mut self) -> Option<u32> {
        let v = u32_at(self.b, self.pos)?;
        self.pos = self.pos.checked_add(4)?;
        Some(v)
    }

    /// A minimal unsigned LEB128 of at most 5 bytes whose value fits `u32`.
    #[inline]
    pub(crate) fn varint(&mut self) -> Option<u32> {
        let first = self.u8()?;
        if first & 0x80 == 0 {
            return Some(u32::from(first));
        }
        let mut value = u32::from(first & 0x7f);
        let mut shift = 7u32;
        loop {
            let b = self.u8()?;
            let low = u32::from(b & 0x7f);
            if shift == 28 && (b & 0x80 != 0 || low > 0x0f) {
                return None;
            }
            value |= low << shift;
            if b & 0x80 == 0 {
                // Minimal: a multi-byte varint does not end in 0x00.
                return if b == 0 { None } else { Some(value) };
            }
            shift += 7;
        }
    }

    /// A varint as `usize`.
    #[inline]
    pub(crate) fn len(&mut self) -> Option<usize> {
        self.varint().map(|v| v as usize)
    }

    /// Skips `n` bytes (all present).
    #[inline]
    pub(crate) fn skip(&mut self, n: usize) -> Option<()> {
        let end = self.pos.checked_add(n)?;
        if end > self.b.len() {
            return None;
        }
        self.pos = end;
        Some(())
    }

    /// The next `n` bytes.
    #[inline]
    pub(crate) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.b.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }
}

/// A minimal unsigned LEB128 of at most 10 bytes whose value fits `u64`
/// (the plural entries of LOCALE, `plans/02-catalog-format.md` §4.1).
pub(crate) fn varint64(c: &mut Cur<'_>) -> Option<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let b = c.u8()?;
        let low = u64::from(b & 0x7f);
        if shift == 63 && (b & 0x80 != 0 || low > 1) {
            return None;
        }
        value |= low << shift;
        if b & 0x80 == 0 {
            return if b == 0 && shift > 0 {
                None
            } else {
                Some(value)
            };
        }
        shift += 7;
    }
}

/// Position of the first NUL byte, eight bytes at a time (SWAR). Hand-written
/// because `str::find` keeps a slice-index panic path alive (B12).
#[inline]
pub(crate) fn nul_pos(b: &[u8]) -> Option<usize> {
    const LO: u64 = 0x0101_0101_0101_0101;
    const HI: u64 = 0x8080_8080_8080_8080;
    let (chunks, rest) = b.as_chunks::<8>();
    let mut base = 0usize;
    for c in chunks {
        let v = u64::from_le_bytes(*c);
        // The lowest flagged byte is the first zero byte (little-endian).
        let z = v.wrapping_sub(LO) & !v & HI;
        if z != 0 {
            return base.checked_add((z.trailing_zeros() / 8) as usize);
        }
        base = base.wrapping_add(8);
    }
    let mut i = base;
    for &x in rest {
        if x == 0 {
            return Some(i);
        }
        i = i.wrapping_add(1);
    }
    None
}

/// Entry `i` of a byte-plane INDEX of `n` entries.
#[inline]
pub(crate) fn plane_entry(index: &[u8], n: usize, i: usize) -> Option<u32> {
    let b0 = *index.get(i)?;
    let b1 = *index.get(n.checked_add(i)?)?;
    let b2 = *index.get(n.checked_mul(2)?.checked_add(i)?)?;
    let b3 = *index.get(n.checked_mul(3)?.checked_add(i)?)?;
    Some(u32::from_le_bytes([b0, b1, b2, b3]))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use alloc::vec;

    use super::{Cur, nul_pos, varint64};

    fn v(b: &[u8]) -> Option<u32> {
        let mut c = Cur::new(b, 0);
        let r = c.varint()?;
        c.at_end().then_some(r)
    }

    #[test]
    fn varints() {
        assert_eq!(v(&[0]), Some(0));
        assert_eq!(v(&[0x7f]), Some(127));
        assert_eq!(v(&[0x80, 0x01]), Some(128));
        assert_eq!(v(&[0xff, 0xff, 0xff, 0xff, 0x0f]), Some(u32::MAX));
        // Non-minimal, too long, overflowing, truncated.
        assert_eq!(v(&[0x80, 0x00]), None);
        assert_eq!(v(&[0xff, 0xff, 0xff, 0xff, 0x1f]), None);
        assert_eq!(v(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01]), None);
        assert_eq!(v(&[0x80]), None);
        let mut c = Cur::new(&[0xff; 10], 0);
        assert_eq!(varint64(&mut c), None);
        let mut c = Cur::new(
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01],
            0,
        );
        assert_eq!(varint64(&mut c), Some(u64::MAX));
    }

    #[test]
    fn nul() {
        for len in 0..40 {
            let mut b = vec![b'a'; len];
            assert_eq!(nul_pos(&b), None);
            for at in 0..len {
                b[at] = 0;
                assert_eq!(nul_pos(&b), Some(at));
                b[at] = 0x80;
            }
        }
    }
}
