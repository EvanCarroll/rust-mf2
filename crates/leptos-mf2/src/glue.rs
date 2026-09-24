//! Everything that names tachys, in one module for every supported tachys
//! line (`plans/04-leptos-integration.md` §3).
//!
//! The module is named for what it holds, not for a tachys line, because it
//! holds more than one. Leptos 0.9's only rendering-trait break is
//! `to_html_with_buf` taking a `RenderFlags` where 0.2 takes `escape` and
//! `mark_branches` (§1), and this crate implements that method twice — for
//! the plain descriptions and for `TrRich`. The `tachys-0-3` feature switches
//! those two methods, and nothing else, to 0.3's form. A second copy of the
//! module per line would make every other change to the glue twice (owner,
//! 2026-09-24). If a later tachys line changes more than a few methods, its
//! part moves into a module of its own beside this one.

pub mod view;
