//! `mf2-native` — 1.x's native application support of Rust MF2, kept as a
//! shim: every item it named, under the name and path it named it,
//! re-exported from [`mf2`](https://docs.rs/mf2), where the code now lives
//! as `mf2::native`, behind `mf2`'s `native` feature.
//!
//! A 1.x application keeps compiling unchanged, in a workspace beside a
//! browser client too: `mf2` refuses `native` beside `hydrate` or `csr`
//! only when compiling for the browser (`wasm32`), so `cargo check
//! --workspace`, which unifies `native` with the client's mode, compiles
//! as in 1.x. A new application names `mf2` alone:
//!
//! ```toml
//! mf2 = { version = "2", features = ["native"] }
//! ```
//!
//! # The user guide
//!
//! The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user
//! guide; its [native applications page](https://evancarroll.github.io/rust-mf2/native-apps.html)
//! covers CLI and terminal applications. An application starts at
//! [`mf2`](https://docs.rs/mf2).

#![no_std]
#![forbid(unsafe_code)]

/// The native application support: the catalogs of one generated corpus,
/// the active locale, and where it came from.
pub use mf2::native::{LocaleSource, NativeError, NativeI18n};

/// What 1.x re-exported from `mf2` beside them.
pub use mf2::{BidiStrategy, Corpus, Dir, Message, TimeZone};
