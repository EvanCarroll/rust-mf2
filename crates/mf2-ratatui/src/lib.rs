//! `mf2-ratatui` — 1.x's Ratatui support of Rust MF2, now a pointer: it
//! re-exports [`mf2`](https://docs.rs/mf2)'s `mf2::ratatui`, where the code
//! lives, behind `mf2`'s `ratatui` feature (which implies `native`), and
//! turns that feature on.
//!
//! 1.x's `line`, `text` and `MarkupStyles`, which took a handle and a map
//! of styles on every call, are gone in 2.0: a description converts into
//! Ratatui's text itself, and a `Theme`, set once, styles its markup. An
//! application names `mf2` alone:
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

/// A message as Ratatui text, its markup as styles: `mf2::ratatui`.
pub use mf2::ratatui::*;
