//! One measurement run of the MF2 renderer (`demo_tui::bench`), for
//! `cargo xtask tui-gate`.

use clap::Parser;
use demo_tui::bench::{self, Args, Renderer};
use demo_tui::{AREA, alloc::Counting, model::SAMPLE, ui};
use mf2_native::NativeI18n;
use ratatui::buffer::Buffer;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

struct Mf2(NativeI18n);

impl Renderer for Mf2 {
    const NAME: &'static str = "mf2";

    fn set_locale(&mut self, tag: &str) {
        self.0
            .set_locale(tag)
            .expect("every benchmark locale is in the corpus");
    }

    fn draw(&self, buf: &mut Buffer) {
        ui::draw(&self.0, &SAMPLE, AREA, buf);
    }
}

fn main() -> Result<(), mf2_native::NativeError> {
    let args = Args::parse();
    let i18n = NativeI18n::embedded(&demo_tui_i18n::CORPUS)?;
    println!("{}", bench::run(&mut Mf2(i18n), args.frames));
    Ok(())
}
