//! Prints one frame as plain text, in the system's language or `--lang`,
//! drawn with MF2 or, with `--upstream`, with the baseline.

use clap::Parser;
use demo_tui::{AREA, model::SAMPLE, to_text, ui, upstream};
use mf2_native::NativeI18n;
use ratatui::buffer::Buffer;

#[derive(Parser)]
struct Args {
    /// The language to draw in, instead of the system's.
    #[arg(long)]
    lang: Option<String>,
    /// Draw with the baseline renderer instead of MF2.
    #[arg(long)]
    upstream: bool,
}

fn main() -> Result<(), mf2_native::NativeError> {
    let args = Args::parse();
    let mut buf = Buffer::empty(AREA);
    if args.upstream {
        upstream::set_locale(args.lang.as_deref().unwrap_or("en"));
        upstream::draw(&SAMPLE, AREA, &mut buf);
    } else {
        let mut i18n = NativeI18n::embedded(&demo_tui_i18n::CORPUS)?;
        if let Some(lang) = args.lang.as_deref() {
            i18n.set_locale(lang)?;
        }
        ui::draw(&i18n, &SAMPLE, AREA, &mut buf);
    }
    print!("{}", to_text(&buf));
    Ok(())
}
