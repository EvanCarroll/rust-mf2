//! The generated-input tests (`generated_l3`, `generated_l4`) spread their
//! cases over threads: each case is independent and seeded by its number, so
//! splitting the range changes nothing but the time. A run of 3,000 cases on
//! one thread took up to ten minutes of `cargo xtask ci` (`plan/09`, 16.0).

use std::ops::Range;
use std::panic::resume_unwind;
use std::thread;

/// Splits the cases `0..n` into one contiguous range per available thread
/// (`std::thread::available_parallelism`), runs `each` on every range on a
/// scoped thread, and returns the results in range order, for the caller to
/// merge before its closing assertions.
///
/// A panic in a thread (a failing case, named with its number and message by
/// `each`) is printed by that thread and raised again here with its own
/// payload; the scope waits for the other threads first.
pub fn cases<T: Send>(n: u64, each: impl Fn(Range<u64>) -> T + Sync) -> Vec<T> {
    let threads = thread::available_parallelism()
        .ok()
        .and_then(|t| u64::try_from(t.get()).ok())
        .unwrap_or(1)
        .clamp(1, n.max(1));
    let chunk = n.div_ceil(threads);
    let each = &each;
    thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let start = i.saturating_mul(chunk).min(n);
                let end = start.saturating_add(chunk).min(n);
                scope.spawn(move || each(start..end))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|payload| resume_unwind(payload))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::cases;

    #[test]
    fn every_case_runs_once_in_order() {
        for n in [0u64, 1, 7, 100] {
            let ranges = cases(n, |range| range.collect::<Vec<_>>());
            let all: Vec<u64> = ranges.into_iter().flatten().collect();
            assert_eq!(all, (0..n).collect::<Vec<_>>(), "n = {n}");
        }
    }

    #[test]
    #[should_panic(expected = "case 5 fails")]
    fn a_failing_case_keeps_its_message() {
        cases(10, |range| {
            for case in range {
                assert_ne!(case, 5, "case {case} fails");
            }
        });
    }
}
