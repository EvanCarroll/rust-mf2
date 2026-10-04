//! The measured shape of the reference workload, as integer tables.
//!
//! Percentages are parts per million so that quotas can be computed exactly
//! with integer arithmetic (largest-remainder apportionment), which keeps the
//! shape exact at every size and independent of the seed. Length
//! distributions are piecewise-linear quantile functions over integer knots —
//! no floating point, hence no platform-dependent rounding.

/// Share of messages with 0, 1, 2, 3, 4 variables (ppm).
pub const VAR_SHARES: [u64; 5] = [790_000, 147_000, 50_000, 11_000, 2_000];

/// Share of messages that are plural `.match` selects on one count (ppm);
/// they are part of the 1-variable bucket.
pub const SELECT_SHARE: u64 = 9_000;

/// Share of messages carrying inline markup (ppm); part of the 0-variable
/// bucket (the old stack faked these with private-use characters, so they were
/// counted as simple).
pub const MARKUP_SHARE: u64 = 5_000;

/// Message source length (bytes), as `(quantile ppm, length)` knots. Tuned so
/// that mean = 27, median = 19, p90 = 58, max = 259 (the maximum is also
/// forced on the longest message).
pub const LENGTH_KNOTS: &[(u64, u64)] = &[
    (0, 2),
    (50_000, 3),
    (100_000, 5),
    (200_000, 8),
    (300_000, 11),
    (400_000, 15),
    (500_000, 19),
    (600_000, 23),
    (700_000, 30),
    (800_000, 37),
    (900_000, 58),
    (950_000, 75),
    (990_000, 125),
    (1_000_000, 259),
];

/// Maximum message source length (bytes).
pub const MAX_LENGTH: usize = 259;

/// Full dotted id length (chars), as `(quantile ppm, length)` knots; mean 23.5.
pub const ID_KNOTS: &[(u64, u64)] = &[
    (0, 8),
    (100_000, 15),
    (250_000, 19),
    (500_000, 23),
    (750_000, 28),
    (900_000, 32),
    (1_000_000, 42),
];

/// Targets and tolerances of the reference workload's shape.
pub mod target {
    /// Messages per locale.
    pub const MESSAGES: usize = 1_600;
    /// Source files per locale.
    pub const FILES: usize = 18;
    /// Call sites.
    pub const SITES: usize = 1_860;
    /// Mean message length (bytes).
    pub const MEAN_LEN: f64 = 27.0;
    /// Median message length (bytes).
    pub const MEDIAN_LEN: f64 = 19.0;
    /// 90th percentile message length (bytes).
    pub const P90_LEN: usize = 58;
    /// Maximum message length (bytes).
    pub const MAX_LEN: usize = 259;
    /// Mean full id length (chars).
    pub const ID_MEAN: f64 = 23.5;
    /// Share of comment bytes in source files.
    pub const COMMENT_SHARE: f64 = 0.60;
    /// Tolerance on every percentage (percentage points).
    pub const PCT_TOLERANCE: f64 = 1.0;
    /// Tolerance on the mean text length (relative).
    pub const MEAN_TOLERANCE: f64 = 0.05;
}

/// Largest-remainder apportionment of `total` over integer `weights`.
/// Deterministic: ties go to the earlier weight.
pub fn apportion(total: usize, weights: &[u64]) -> Vec<usize> {
    let sum: u64 = weights.iter().sum();
    assert!(sum > 0, "apportion over zero weights");
    let total64 = total as u64;
    let mut counts: Vec<usize> = Vec::with_capacity(weights.len());
    let mut remainders: Vec<(u64, usize)> = Vec::with_capacity(weights.len());
    let mut given = 0usize;
    for (i, &w) in weights.iter().enumerate() {
        let exact = u128::from(total64) * u128::from(w);
        let floor = exact / u128::from(sum);
        let rem = exact % u128::from(sum);
        let floor = usize::try_from(floor).expect("quota fits in usize");
        counts.push(floor);
        given += floor;
        remainders.push((u64::try_from(rem).expect("remainder < sum"), i));
    }
    // Largest remainder first; equal remainders keep index order.
    remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for &(_, i) in remainders.iter().take(total - given) {
        counts[i] += 1;
    }
    counts
}

/// `round(total * ppm / 1e6)`, half up.
pub fn share(total: usize, ppm: u64) -> usize {
    let exact = u128::from(total as u64) * u128::from(ppm);
    usize::try_from((exact + 500_000) / 1_000_000).expect("share fits in usize")
}

/// `n` samples of the quantile function given by `knots`, at the mid-points
/// `(2i + 1) / 2n`, in ascending order, rounded half up.
pub fn quantile_pool(n: usize, knots: &[(u64, u64)]) -> Vec<usize> {
    let n2 = 2 * n as u128;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        // u = (2i + 1) / 2n, in ppm scaled by 2n: u_scaled = (2i+1) * 1e6.
        let u_scaled = (2 * i as u128 + 1) * 1_000_000;
        let seg = knots
            .windows(2)
            .find(|w| u_scaled <= u128::from(w[1].0) * n2)
            .unwrap_or(&knots[knots.len() - 2..]);
        let (a_u, a_len) = (u128::from(seg[0].0), u128::from(seg[0].1));
        let (b_u, b_len) = (u128::from(seg[1].0), u128::from(seg[1].1));
        let num = (u_scaled - a_u * n2) * (b_len - a_len);
        let den = (b_u - a_u) * n2;
        let len = a_len + (2 * num + den) / (2 * den);
        out.push(usize::try_from(len).expect("length fits in usize"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{LENGTH_KNOTS, apportion, quantile_pool};

    #[test]
    fn apportion_sums() {
        let c = apportion(1600, &super::VAR_SHARES);
        assert_eq!(c.iter().sum::<usize>(), 1600);
        assert_eq!(c, vec![1264, 235, 80, 18, 3]);
    }

    #[test]
    fn pool_shape() {
        let pool = quantile_pool(1600, LENGTH_KNOTS);
        assert_eq!(pool.len(), 1600);
        assert!(pool.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(pool[799], 19);
        assert_eq!(pool[800], 19);
        assert_eq!(pool[1439], 58);
    }
}
