//! `mf2-ratatui` — 1.x's Ratatui support of Rust MF2, kept as a shim: every
//! item it named, under the name and path it named it, re-exported from
//! [`mf2`](https://docs.rs/mf2), where the code now lives as
//! `mf2::ratatui`, behind `mf2`'s `ratatui` feature (which implies
//! `native`).
//!
//! A 1.x application keeps compiling unchanged, in a workspace beside a
//! browser client too: `mf2` refuses `ratatui` beside `hydrate` or `csr`
//! only when compiling for the browser (`wasm32`), so `cargo check
//! --workspace`, which unifies `ratatui` with the client's mode, compiles
//! as in 1.x. A new application names `mf2` alone:
//!
//! ```toml
//! mf2 = { version = "2", features = ["ratatui"] }
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

/// A message as Ratatui text, its markup as styles: the styles of the
/// markup names, and the two conversions, through `mf2::native`'s
/// `NativeI18n`.
pub use mf2::ratatui::{MarkupStyles, line, text};
