//! A trippy-shaped terminal UI in the reader's language. `1` and `2` switch
//! the language live; `q` quits.

mod ui;

use clap::Parser;
use mf2::ratatui::{Theme, set_theme};
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::style::Style;

mf2::include_generated!();

#[derive(Parser)]
struct Args {
    /// The language to draw in, instead of the system's.
    #[arg(long)]
    lang: Option<Locale>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    install();
    if let Some(lang) = args.lang {
        set_locale(lang);
    }
    // What each markup name looks like: a message says what a stretch is,
    // the theme how it is drawn. `markup::…` has a constant for every name
    // the messages use, so a misspelt name does not compile.
    set_theme(
        Theme::default()
            .style(markup::KEY, Style::new().bold().yellow())
            .style(markup::HOST, Style::new().underlined())
            .style(markup::WARN, Style::new().red()),
    );
    let trace = ui::Trace::sample();
    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| ui::draw(frame, &trace))?;
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // Every thread's next format is in the new language.
                    KeyCode::Char('1') => set_locale(Locale::En),
                    KeyCode::Char('2') => set_locale(Locale::Fr),
                    KeyCode::Char('q') => return Ok(()),
                    _ => {}
                }
            }
        }
    })
}
