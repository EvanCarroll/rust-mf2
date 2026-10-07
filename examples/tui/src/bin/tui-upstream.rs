//! One measurement run of the baseline renderer (`demo_tui::bench`), for
//! `cargo xtask tui-allocs-vs-pseudotrippy`.

use clap::Parser;
use demo_tui::bench::{self, Args, Renderer};
use demo_tui::{AREA, alloc::Counting, model::SAMPLE, upstream};
use ratatui::buffer::Buffer;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

struct Upstream;

impl Renderer for Upstream {
    const NAME: &'static str = "upstream";

    fn set_locale(&mut self, tag: &str) {
        upstream::set_locale(tag);
    }

    fn draw(&self, buf: &mut Buffer) {
        upstream::draw(&SAMPLE, AREA, buf);
    }
}

fn main() {
    let args = Args::parse();
    println!("{}", bench::run(&mut Upstream, args.frames));
}
