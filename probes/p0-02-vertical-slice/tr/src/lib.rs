//! P0.2 probe: the concrete `Tr` description type, run end to end in a browser.
//!
//! Throwaway code. It answers, among other things, whether the client-path
//! crate can be `forbid(unsafe_code)` (it is) and panic-free.
//!
//! * `catalog` — hand-built two-section catalog (index + pool) and its reader.
//! * `glue` — tachys 0.2 impls for `Tr` (text child, attribute, conversions).
//! * `registry` — node-update strategy B: a library-owned slab of live nodes.
//! * `client` (feature `hydrate`) — thread-local catalog, boot gate, `set_locale`.
//! * `server` (feature `ssr`) — per-request context, default-locale fallback.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

pub mod catalog;
pub mod error;
mod glue;
#[cfg_attr(feature = "ssr", allow(dead_code))] // populated only in the browser
mod registry;

#[cfg(feature = "hydrate")]
pub mod client;
#[cfg(feature = "ssr")]
pub mod server;

pub use catalog::{Catalog, Dir, fnv1a64};
pub use error::{CatalogError, LoadError};
pub use glue::{AttrState, TrState};
pub use registry::live_nodes;

/// A message description: a 4-byte `Copy` value, identical on every target.
/// Nothing is looked up until it renders or is stringified (D9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tr {
    id: u32,
}

// The call-site value is 4 bytes on every target (D9's premise).
const _: () = assert!(core::mem::size_of::<Tr>() == 4);

/// What `tr!("…")` would expand to for a message without arguments.
#[inline(always)]
pub const fn tr(id: u32) -> Tr {
    Tr { id }
}

impl Tr {
    pub const fn id(self) -> u32 {
        self.id
    }

    /// The formatted message as an owned `String`. Tracks the locale only when
    /// a reactive observer exists, so calling it from an event handler does
    /// not warn (plans/04 §4).
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(self) -> String {
        track_locale();
        with_text(self.id, str::to_owned)
    }

    /// Same as [`Tr::to_string`] but never tracks (`tr_untracked!`).
    pub fn to_string_untracked(self) -> String {
        with_text(self.id, str::to_owned)
    }
}

/// Looks the message text up: per-request context on the server (render time,
/// D9), the thread-local catalog in the browser.
#[inline(never)]
pub(crate) fn with_text<R>(id: u32, f: impl FnOnce(&str) -> R) -> R {
    #[cfg(feature = "ssr")]
    {
        server::with_text(id, f)
    }
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    {
        client::with_text(id, f)
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        let _ = id;
        f("")
    }
}

/// Subscribes the current observer (if any) to locale changes.
#[inline]
pub(crate) fn track_locale() {
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    client::track_locale();
}

/// The active locale tag (server: this request's; client: the installed one).
pub fn current_locale() -> String {
    #[cfg(feature = "ssr")]
    {
        server::current().catalog.locale().to_owned()
    }
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    {
        client::current_locale()
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        String::new()
    }
}
