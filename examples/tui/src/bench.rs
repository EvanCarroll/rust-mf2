//! The measurement both benchmark binaries make, so that they differ only
//! in the renderer. One run prints one JSON line:
//!
//! - `allocs` / `bytes`: the allocations one frame makes, and the bytes they
//!   ask for, in each locale of [`LOCALES`], after two warm-up frames (the
//!   first frame of a process parses the baseline's TOML and fills caches);
//! - `ns_per_frame`: the mean time of a frame over `frames` frames in each
//!   locale, switching language between them as a live switch does.
//!
//! `cargo xtask tui-gate` runs the binaries alternately and takes the median
//! of the runs' `ns_per_frame`; the allocation counts must be the same in
//! every run.

use std::hint::black_box;
use std::time::Instant;

use clap::Parser;
use ratatui::buffer::Buffer;

use crate::model::LOCALES;
use crate::{AREA, alloc};

/// One renderer, as the benchmark drives it.
pub trait Renderer {
    /// The name the report gives it.
    const NAME: &'static str;
    /// Switches the language of the next frame.
    fn set_locale(&mut self, tag: &str);
    /// Draws one frame.
    fn draw(&self, buf: &mut Buffer);
}

/// The benchmark binaries' arguments.
#[derive(Parser)]
pub struct Args {
    /// Frames timed in each locale.
    #[arg(long, default_value_t = 50)]
    pub frames: usize,
}

/// One measurement run, as a JSON line.
pub fn run<R: Renderer>(renderer: &mut R, frames: usize) -> String {
    let mut buf = Buffer::empty(AREA);
    let mut allocs = [0_u64; LOCALES.len()];
    let mut bytes = [0_u64; LOCALES.len()];
    for (i, tag) in LOCALES.iter().enumerate() {
        renderer.set_locale(tag);
        for _ in 0..2 {
            buf.reset();
            renderer.draw(&mut buf);
        }
        buf.reset();
        let (a0, b0) = alloc::snapshot();
        renderer.draw(&mut buf);
        let (a1, b1) = alloc::snapshot();
        allocs[i] = a1 - a0;
        bytes[i] = b1 - b0;
    }

    let start = Instant::now();
    for tag in LOCALES {
        renderer.set_locale(tag);
        for _ in 0..frames {
            buf.reset();
            renderer.draw(black_box(&mut buf));
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let ns_per_frame = start.elapsed().as_nanos() as f64 / (frames * LOCALES.len()) as f64;

    let list = |values: &[u64]| {
        values
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let tags = LOCALES
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"renderer\":\"{}\",\"locales\":[{tags}],\"allocs\":[{}],\"bytes\":[{}],\"frames\":{},\"ns_per_frame\":{ns_per_frame:.1}}}",
        R::NAME,
        list(&allocs),
        list(&bytes),
        frames * LOCALES.len(),
    )
}
