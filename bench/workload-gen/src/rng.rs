//! A small, value-stable PRNG (`SplitMix64`).
//!
//! Written here rather than taken from a crate so that generated output can
//! never change because a dependency was bumped. Every consumer derives its own
//! stream from `(seed, label, index)`, so changing one part of the generator
//! does not shift the random choices of the others.

/// `SplitMix64` (Steele, Lea, Flood 2014). Fast, tiny, and good enough for
/// synthetic data; not for cryptography.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator seeded directly.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// An independent stream for `(seed, label, index)`.
    pub fn stream(seed: u64, label: &str, index: u64) -> Self {
        // FNV-1a over the label keeps streams apart without any table.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in label.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        let mut mixer = Self::new(seed ^ hash);
        let first = mixer.next_u64();
        let mut mixer = Self::new(first ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        Self::new(mixer.next_u64())
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (Lemire's multiply-shift; `n` must be non-zero).
    #[allow(clippy::cast_possible_truncation)] // the product's high half is < n
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "Rng::below(0)");
        ((u128::from(self.next_u64()) * n as u128) >> 64) as usize
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: usize, hi: usize) -> usize {
        assert!(lo <= hi, "Rng::range with lo > hi");
        lo + self.below(hi - lo + 1)
    }

    /// `true` with probability `percent / 100`.
    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    /// One element of a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// Fisher–Yates shuffle.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn value_stable() {
        // Reference values of SplitMix64 seeded with 0: if these change, every
        // generated corpus changes.
        let mut rng = Rng::new(0);
        assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
    }

    #[test]
    fn below_in_range() {
        let mut rng = Rng::stream(1, "t", 0);
        for n in 1..50 {
            assert!(rng.below(n) < n);
        }
    }
}
