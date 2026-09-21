//! What both reports share: build information, the machine's load, number
//! formatting.

use serde::Serialize;

/// How the binary was built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Build {
    /// `rustc -V` of the compiler that built the harness.
    pub rustc: &'static str,
    /// Cargo's profile (`release` for every committed report).
    pub profile: &'static str,
    /// Cargo's `OPT_LEVEL`.
    pub opt_level: &'static str,
    /// Whether debug assertions were on (a debug build: timings are meaningless).
    pub debug_assertions: bool,
    /// Resolved versions of the compressor crates (from `Cargo.lock`).
    pub crates: &'static str,
}

impl Build {
    /// This binary's build.
    pub fn current() -> Self {
        Self {
            rustc: env!("CATALOG_BENCH_RUSTC"),
            profile: env!("CATALOG_BENCH_PROFILE"),
            opt_level: env!("CATALOG_BENCH_OPT_LEVEL"),
            debug_assertions: cfg!(debug_assertions),
            crates: env!("CATALOG_BENCH_CRATES"),
        }
    }

    /// One line for a report header.
    pub fn line(&self) -> String {
        format!(
            "{}; profile `{}`, opt-level {}{}",
            self.rustc,
            self.profile,
            self.opt_level,
            if self.debug_assertions {
                " — **DEBUG BUILD, timings meaningless**"
            } else {
                ""
            }
        )
    }
}

/// The 1-, 5- and 15-minute load averages (`/proc/loadavg`), if readable.
pub fn load_average() -> Option<[f64; 3]> {
    let text = std::fs::read_to_string("/proc/loadavg").ok()?;
    let mut it = text.split_whitespace().map(str::parse::<f64>);
    Some([it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?])
}

/// The mean current clock of all CPUs in MHz (`/proc/cpuinfo`), if readable:
/// a laptop that throttles shows it here.
pub fn cpu_mhz() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let mhz: Vec<f64> = text
        .lines()
        .filter(|l| l.starts_with("cpu MHz"))
        .filter_map(|l| l.split(':').nth(1)?.trim().parse().ok())
        .collect();
    (!mhz.is_empty()).then(|| mhz.iter().sum::<f64>() / mhz.len() as f64)
}

/// Seconds since the Unix epoch.
pub fn unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `1234567` → `1,234,567`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A `usize` with thousands separators.
pub fn n(v: usize) -> String {
    thousands(v as u64)
}

/// A signed difference with thousands separators and an explicit sign.
pub fn signed(d: i64) -> String {
    match d {
        0 => "±0".to_owned(),
        d if d > 0 => format!("+{}", thousands(d.unsigned_abs())),
        d => format!("−{}", thousands(d.unsigned_abs())),
    }
}

/// `a − b` as `i64`.
pub fn delta(a: usize, b: usize) -> i64 {
    i64::try_from(a).unwrap_or(i64::MAX) - i64::try_from(b).unwrap_or(i64::MAX)
}

/// A float with `places` decimals.
pub fn fixed(x: f64, places: usize) -> String {
    format!("{x:.places$}")
}

#[cfg(test)]
mod tests {
    use super::{delta, signed, thousands};

    #[test]
    fn formatting() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(25_600), "25,600");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(signed(0), "±0");
        assert_eq!(signed(1_234), "+1,234");
        assert_eq!(signed(-56), "−56");
        assert_eq!(delta(3, 5), -2);
    }
}
