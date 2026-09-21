//! Identities shared with the catalog and the runtime.

/// A message id: an index into one catalog chunk's message table.
///
/// Low 24 bits: the index; high 8 bits: the chunk (plans/02-catalog-format.md
/// §3). Until per-route chunking exists every message is in chunk 0.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct MsgId(u32);

impl MsgId {
    /// Bits of the index part.
    pub const INDEX_BITS: u32 = 24;

    const INDEX_MASK: u32 = (1 << Self::INDEX_BITS) - 1;

    /// The id of message `index` in `chunk`; `None` if `index ≥ 2^24`.
    pub const fn new(chunk: u8, index: u32) -> Option<MsgId> {
        if index > Self::INDEX_MASK {
            return None;
        }
        Some(MsgId(((chunk as u32) << Self::INDEX_BITS) | index))
    }

    /// The id whose raw value is `raw` (every `u32` is a valid id).
    pub const fn from_raw(raw: u32) -> MsgId {
        MsgId(raw)
    }

    /// The raw value: `chunk << 24 | index`.
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// The chunk (high 8 bits).
    pub const fn chunk(self) -> u8 {
        // Lossless: a u32 shifted right by 24 fits in 8 bits.
        #[allow(clippy::cast_possible_truncation)]
        let chunk = (self.0 >> Self::INDEX_BITS) as u8;
        chunk
    }

    /// The index within the chunk (low 24 bits).
    pub const fn index(self) -> u32 {
        self.0 & Self::INDEX_MASK
    }
}

/// Text direction.
///
/// The catalog header uses `Ltr`/`Rtl` for a locale; resolved values (e.g.
/// through `u:dir`) may be `Auto`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Dir {
    /// Left to right.
    Ltr = 0,
    /// Right to left.
    Rtl = 1,
    /// Determined from the content (first strong character).
    Auto = 2,
}

#[cfg(test)]
mod tests {
    use super::{Dir, MsgId};

    #[test]
    fn bit_layout() {
        let id = MsgId::new(0x12, 0x34_5678).expect("index fits");
        assert_eq!(id.raw(), 0x1234_5678);
        assert_eq!(id.chunk(), 0x12);
        assert_eq!(id.index(), 0x34_5678);
        assert_eq!(MsgId::from_raw(0x1234_5678), id);
        assert_eq!(MsgId::INDEX_BITS, 24);
    }

    #[test]
    fn index_bounds() {
        assert!(MsgId::new(0, (1 << 24) - 1).is_some());
        assert!(MsgId::new(0, 1 << 24).is_none());
        assert!(MsgId::new(255, u32::MAX).is_none());
        let max = MsgId::new(255, (1 << 24) - 1).expect("fits");
        assert_eq!(max.raw(), u32::MAX);
        assert_eq!((max.chunk(), max.index()), (255, 0xFF_FFFF));
        let zero = MsgId::new(0, 0).expect("fits");
        assert_eq!(zero.raw(), 0);
    }

    #[test]
    fn order_is_chunk_then_index() {
        let a = MsgId::new(0, 5).expect("fits");
        let b = MsgId::new(0, 6).expect("fits");
        let c = MsgId::new(1, 0).expect("fits");
        assert!(a < b && b < c);
    }

    #[test]
    fn dir_discriminants() {
        assert_eq!(Dir::Ltr as u8, 0);
        assert_eq!(Dir::Rtl as u8, 1);
        assert_eq!(Dir::Auto as u8, 2);
    }
}
