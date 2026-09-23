//! What the macro costs, when something is measuring it (P0.9's method,
//! `plans/06-size-and-perf.md` §5).
//!
//! Off unless `MF2_MACRO_STATS` names a file. Each rustc process then writes
//! `<file>.<pid>`:
//!
//! ```text
//! pid 12345 expansions 2000 nanos 137482910 reads 1
//! ```
//!
//! which is how A2's "a cache hit does not re-read the manifest" and A7's
//! macro time are *measured* rather than asserted: expansion order inside one
//! crate is rustc's business, so the count has to be read after the compile,
//! not from inside it.

use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Instant;

#[derive(Default)]
struct Stats {
    expansions: u64,
    nanos: u128,
    reads: u64,
}

/// The stats of this process, or `None` when nothing is measuring.
fn stats() -> Option<&'static Mutex<Stats>> {
    static STATS: OnceLock<Option<Mutex<Stats>>> = OnceLock::new();
    STATS
        .get_or_init(|| std::env::var_os("MF2_MACRO_STATS").map(|_| Mutex::new(Stats::default())))
        .as_ref()
}

fn with(f: impl FnOnce(&mut Stats)) {
    if let Some(stats) = stats() {
        let mut stats = stats.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut stats);
    }
}

/// When this expansion began, if anything is measuring.
pub(crate) fn start() -> Option<Instant> {
    stats().map(|_| Instant::now())
}

/// One expansion, and what it took.
pub(crate) fn expansion(started: Option<Instant>) {
    let Some(started) = started else {
        return;
    };
    let nanos = started.elapsed().as_nanos();
    with(|s| {
        s.expansions += 1;
        s.nanos += nanos;
    });
    flush();
}

/// One manifest read from disk — what a cache hit does not do.
pub(crate) fn read() {
    with(|s| s.reads += 1);
}

/// Writes the running totals. Every expansion rewrites the one short line,
/// because a proc-macro server has no exit hook to write it at.
fn flush() {
    let Some(path) = std::env::var_os("MF2_MACRO_STATS") else {
        return;
    };
    with(|s| {
        let line = format!(
            "pid {} expansions {} nanos {} reads {}\n",
            std::process::id(),
            s.expansions,
            s.nanos,
            s.reads
        );
        let mut path = std::path::PathBuf::from(&path);
        let name = format!(
            "{}.{}",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            std::process::id()
        );
        path.set_file_name(name);
        let _ = std::fs::write(path, line);
    });
}
