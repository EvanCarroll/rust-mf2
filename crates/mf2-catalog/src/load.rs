//! The load number: what a catalog is given when it is loaded, so that a cache
//! can tell one catalog from another without comparing their bytes.
//!
//! It exists only where this build's side asked for it — `std-load-id` off
//! the browser, `web-load-id` on `wasm32-unknown-unknown` — and is nothing at
//! all otherwise: [`Load`] is then zero-sized and [`Load::next`] does nothing,
//! so the reader names it without a `cfg` of its own.

/// Whether this build numbers its catalogs: its own side's feature is on.
#[doc(hidden)]
pub const LOAD_ID: bool = cfg!(any(
    all(
        feature = "std-load-id",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ),
    all(
        feature = "web-load-id",
        target_arch = "wasm32",
        target_os = "unknown"
    )
));

#[cfg(any(
    all(
        feature = "std-load-id",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ),
    all(feature = "web-load-id", target_arch = "wasm32", target_os = "unknown")
))]
mod numbered {
    use core::sync::atomic::{AtomicU64, Ordering};

    /// The next load number: process-wide, from 1, never handed out twice (a
    /// `u64` does not wrap in a process's life).
    static LOADS: AtomicU64 = AtomicU64::new(1);

    /// A catalog's load number.
    #[derive(Clone, Copy)]
    pub(crate) struct Load(u64);

    impl Load {
        /// A number not given before.
        pub(crate) fn next() -> Load {
            Load(LOADS.fetch_add(1, Ordering::Relaxed))
        }

        /// The number.
        pub(crate) fn id(self) -> u64 {
            self.0
        }
    }
}

#[cfg(not(any(
    all(
        feature = "std-load-id",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ),
    all(feature = "web-load-id", target_arch = "wasm32", target_os = "unknown")
)))]
mod numbered {
    /// No load number in this build.
    #[derive(Clone, Copy)]
    pub(crate) struct Load;

    impl Load {
        /// Nothing.
        pub(crate) fn next() -> Load {
            Load
        }
    }
}

pub(crate) use numbered::Load;
