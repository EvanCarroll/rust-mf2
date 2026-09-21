//! Order statistics of timing samples (the parser gate's `Spread`).

use serde::Serialize;

/// Order statistics of a row's samples.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Spread {
    /// Fastest sample.
    pub min: f64,
    /// First quartile.
    pub q1: f64,
    /// Median — the figure the gate compares.
    pub median: f64,
    /// Third quartile.
    pub q3: f64,
    /// Slowest sample.
    pub max: f64,
}

impl Spread {
    /// Order statistics of `samples` (sorted in place); `None` if empty.
    /// Quartiles interpolate linearly between closest ranks.
    #[allow(clippy::cast_precision_loss)] // ranks are tiny
    pub fn of(samples: &mut [f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        samples.sort_by(f64::total_cmp);
        let last = samples.len() - 1;
        let q = |quarters: usize| {
            let scaled = quarters * last;
            let lo = scaled / 4;
            let frac = (scaled % 4) as f64 / 4.0;
            let hi = (lo + 1).min(last);
            samples[lo] + (samples[hi] - samples[lo]) * frac
        };
        Some(Self {
            min: samples[0],
            q1: q(1),
            median: q(2),
            q3: q(3),
            max: samples[last],
        })
    }

    /// Interquartile range as a fraction of the median.
    pub fn relative_iqr(&self) -> f64 {
        if self.median > 0.0 {
            (self.q3 - self.q1) / self.median
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Spread;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn spread_of_odd_and_even_samples() {
        let mut odd = [5.0, 1.0, 3.0, 2.0, 4.0];
        let s = Spread::of(&mut odd).expect("non-empty");
        assert!(close(s.min, 1.0) && close(s.max, 5.0));
        assert!(close(s.median, 3.0) && close(s.q1, 2.0) && close(s.q3, 4.0));
        let mut even = [4.0, 1.0, 3.0, 2.0];
        let s = Spread::of(&mut even).expect("non-empty");
        assert!(close(s.median, 2.5));
        assert!(close(s.relative_iqr(), 0.6));
        assert!(Spread::of(&mut []).is_none());
    }
}
