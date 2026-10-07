//! One measurement run of the MF2 renderer (`demo_tui::bench`), for
//! `cargo xtask tui-allocs-vs-trippy`.

use clap::Parser;
use demo_tui::bench::{self, Args, Renderer};
use demo_tui::prelude::*;
use demo_tui::{AREA, alloc::Counting, model::SAMPLE, ui};
use ratatui::buffer::Buffer;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

struct Mf2;

impl Renderer for Mf2 {
    const NAME: &'static str = "mf2";

    fn set_locale(&mut self, tag: &str) {
        set_locale(tag.parse().expect("every benchmark locale is in the corpus"));
    }

    fn draw(&self, buf: &mut Buffer) {
        ui::draw(&SAMPLE, AREA, buf);
    }
}

fn main() {
    let args = Args::parse();
    demo_tui::install();
    mf2::ratatui::set_theme(ui::theme());
    println!("{}", bench::run(&mut Mf2, args.frames));
}
