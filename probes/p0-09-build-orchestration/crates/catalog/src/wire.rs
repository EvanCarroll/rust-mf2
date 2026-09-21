//! Little-endian, length-prefixed primitives shared by both file formats.

use crate::error::DecodeError;

/// Appends primitives to a byte buffer.
#[derive(Default)]
pub struct Writer {
    /// The bytes written so far.
    pub buf: Vec<u8>,
}

impl Writer {
    /// One byte.
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    /// A `u32`.
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    /// A `u64`.
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    /// A length-prefixed UTF-8 string.
    pub fn str(&mut self, s: &str) {
        self.u32(u32::try_from(s.len()).unwrap_or(u32::MAX));
        self.buf.extend_from_slice(s.as_bytes());
    }
    /// A count, as `u32`.
    pub fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }
}

/// Reads primitives from a byte slice; never panics.
pub struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    /// A reader over `bytes`.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.bytes.len() < n {
            return Err(DecodeError::Truncated);
        }
        let (head, tail) = self.bytes.split_at(n);
        self.bytes = tail;
        Ok(head)
    }
    /// Checks and skips magic bytes.
    pub fn magic(&mut self, expected: &'static str) -> Result<(), DecodeError> {
        if self.take(expected.len())? == expected.as_bytes() {
            Ok(())
        } else {
            Err(DecodeError::Magic { expected })
        }
    }
    /// One byte.
    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        self.take(1).map(|b| b.first().copied().unwrap_or(0))
    }
    /// A `u32`.
    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        let b = self.take(4)?;
        let mut a = [0u8; 4];
        a.copy_from_slice(b);
        Ok(u32::from_le_bytes(a))
    }
    /// A `u64`.
    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }
    /// A length-prefixed UTF-8 string, borrowed.
    pub fn str(&mut self) -> Result<&'a str, DecodeError> {
        let n = self.u32()? as usize;
        Ok(std::str::from_utf8(self.take(n)?)?)
    }
    /// A count.
    pub fn len(&mut self) -> Result<usize, DecodeError> {
        self.u32().map(|n| n as usize)
    }
}

/// FNV-1a 64 (plans/02-catalog-format.md §3).
pub struct Fnv64(u64);

impl Default for Fnv64 {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv64 {
    /// Feeds bytes.
    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    /// The hash.
    pub fn finish(&self) -> u64 {
        self.0
    }
}
