//! Everything that names tachys, one module per supported tachys line
//! (`plans/04-leptos-integration.md` §3).
//!
//! Leptos 0.9's only rendering-trait break is `to_html_with_buf` taking a
//! `RenderFlags`, with the text-separator rules moving from `escape` to
//! `flags.hydrate` (§1). When 0.9 is released, `tachys_0_3.rs` goes beside
//! `tachys_0_2.rs` and a feature picks one; nothing outside this directory
//! changes.

pub mod tachys_0_2;
