//! The per-input work budget every target asserts, measured in **CPU time**.
//!
//! What these targets claim is a property of the code: the work per input
//! byte is bounded, so no input makes a parser or a writer go quadratic.
//! Wall-clock time is the wrong instrument for that. It measures the machine
//! as much as the code: a stall, a busy desktop, or a laptop lid reads as a
//! blown budget on an input that replays in a millisecond. Both of Phase 5a's
//! A12 failures were that, and neither was a defect:
//!
//! * 83.8 ms against an 83.6 ms budget, with wasm builds running alongside —
//!   1 ms of actual work;
//! * a libFuzzer timeout "after 7803 seconds", which is what a **suspended
//!   laptop** looks like to a wall clock (the owner had closed the lid). The
//!   saved input replays instantly, and the run was at 2,345 exec/s until the
//!   moment it stopped.
//!
//! A suspend is the clearest case for the change: no time passed for the
//! code, and all of it passed for the clock.
//!
//! Thread CPU time counts only the cycles this thread was actually given, so
//! it measures the algorithm and nothing else. A genuine loop still trips it,
//! because a loop burns CPU; libFuzzer's own `-timeout` remains the guard
//! against a thread that blocks rather than spins.

use std::time::Duration;

/// A CPU-time clock for the calling thread.
pub struct Cpu(Duration);

impl Cpu {
    /// Starts measuring.
    pub fn start() -> Cpu {
        Cpu(now())
    }

    /// The CPU time used since [`Cpu::start`].
    pub fn elapsed(&self) -> Duration {
        now().saturating_sub(self.0)
    }
}

/// `CLOCK_THREAD_CPUTIME_ID`, or zero if the platform will not say.
fn now() -> Duration {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `clock_gettime` writes a `timespec` through the pointer and
    // reads nothing else; `ts` is a live, correctly typed local.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &raw mut ts) };
    if rc != 0 {
        return Duration::ZERO;
    }
    Duration::new(
        ts.tv_sec.try_into().unwrap_or(0),
        ts.tv_nsec.try_into().unwrap_or(0),
    )
}

/// The budget for an input of `bytes` bytes, plus `extra` for the work a
/// target does beyond reading its input (formatting output, for instance).
///
/// The constant term absorbs one-off costs — a locale's data built on first
/// use, a page faulted in — and the per-byte term is what catches growth
/// that is worse than linear.
pub fn budget(bytes: usize, extra: Duration) -> Duration {
    Duration::from_millis(50)
        + Duration::from_micros(50) * u32::try_from(bytes).unwrap_or(u32::MAX)
        + extra
}

/// Fails if `cpu` overran the budget for `bytes`, naming both.
pub fn check(cpu: &Cpu, bytes: usize, extra: Duration) {
    let budget = budget(bytes, extra);
    let used = cpu.elapsed();
    assert!(
        used <= budget,
        "{used:?} of CPU for {bytes} bytes (budget {budget:?})"
    );
}
