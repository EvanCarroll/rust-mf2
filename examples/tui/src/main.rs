//! Prints one frame as plain text, in the system's language or `--lang`,
//! drawn with MF2 or, with `--upstream`, with the baseline.

use clap::Parser;
use demo_tui::prelude::*;
use demo_tui::{AREA, model::SAMPLE, to_text, ui, upstream};
use ratatui::buffer::Buffer;

#[derive(Parser)]
struct Args {
    /// The language to draw in, instead of the system's.
    #[arg(long)]
    lang: Option<Locale>,
    /// Draw with the baseline renderer instead of MF2.
    #[arg(long)]
    upstream: bool,
}

fn main() {
    let args = Args::parse();
    let mut buf = Buffer::empty(AREA);
    if args.upstream {
        upstream::set_locale(args.lang.map_or("en", Locale::tag));
        upstream::draw(&SAMPLE, AREA, &mut buf);
    } else {
        demo_tui::install();
        if let Some(lang) = args.lang {
            set_locale(lang);
        }
        mf2::ratatui::set_theme(ui::theme());
        ui::draw(&SAMPLE, AREA, &mut buf);
    }
    print!("{}", to_text(&buf));
}
