//! A trippy-shaped terminal UI in four languages, drawn into a Ratatui
//! buffer, twice: [`ui`] with MF2, [`upstream`] the way trippy itself
//! translates. `cargo xtask tui-gate` measures both (allocations and time
//! per frame, executable size); `cargo run` prints one frame.
//!
//! The messages are in `locales/`; `build.rs` compiles them, and the include
//! below brings in what it generates: `tr!`, `Locale`, `install()` and the
//! rest, which the modules import with `use crate::prelude::*`.

#![deny(unsafe_code)]

pub mod alloc;
pub mod bench;
pub mod model;
pub mod ui;
pub mod upstream;

mf2::include_generated!();

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// The area every frame is drawn into.
pub const AREA: Rect = Rect::new(0, 0, 160, 47);

/// The buffer as plain text, one line per row, trailing spaces trimmed.
#[must_use]
pub fn to_text(buf: &Buffer) -> String {
    let area = buf.area;
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        let mut row = String::new();
        for x in area.left()..area.right() {
            row.push_str(buf[(x, y)].symbol());
        }
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out
}
