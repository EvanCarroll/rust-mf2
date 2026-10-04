//! `catalog-bench` — Phase 2's measurements of the `.mf2b` catalog
//! (tasks A8 and A9's native half):
//!
//! * **size** (A8, budget B7): every locale of the reference workload as a
//!   stripped (production) and an unstripped catalog — raw, gzip (GNU
//!   `gzip -9 -n`, flate2, zlib-rs), brotli 11; structure vs pool; section
//!   by section; the B7 checks; deltas against P0.7; the NAMES estimate.
//! * **bench** (A9, B9/B10 natively): `Catalog::new`, `get`, `text` and view
//!   walks, medians of interleaved samples, a counting allocator, the
//!   0-allocation / 0-copy load check, compared with P0.8.
//!
//! The binary runs them (`cargo run --release -p catalog-bench -- size|bench`);
//! the library's unit tests use tiny inputs only. See `README.md`.

// Byte counts and nanoseconds, all far below 2^52: `as f64` is exact enough.
#![allow(clippy::cast_precision_loss)]

pub mod alloc;
pub mod baseline;
pub mod bench;
pub mod compress;
pub mod corpus;
mod error;
pub mod names;
pub mod plural;
pub mod report;
pub mod size;
pub mod stats;

use std::path::{Path, PathBuf};

pub use error::{Error, Result};

/// Every allocation of every binary linking this crate goes through the
/// counting allocator (the bench binary and the unit tests).
#[global_allocator]
static GLOBAL: alloc::Counting = alloc::Counting;

/// The repository root (two levels above this crate's manifest).
pub fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| manifest.to_path_buf(), Path::to_path_buf)
}
