//! One frame: the hops in a bordered table, a language menu, a key-hint bar
//! and a status line. Nothing is passed for translation: the text is in the
//! current language, and the theme says how markup looks.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, List, Row, Table};

use crate::prelude::*;

/// A trace in progress.
pub struct Trace {
    pub target: &'static str,
    pub sent: u64,
    pub hops: Vec<Hop>,
}

/// One hop: the host that answered, if one did; the share of probes lost;
/// the average round trip in milliseconds.
pub struct Hop {
    pub host: Option<&'static str>,
    pub loss: f64,
    pub average: f64,
}

impl Trace {
    /// A trace to draw.
    pub fn sample() -> Trace {
        let hop = |host, loss, average| Hop { host, loss, average };
        Trace {
            target: "example.org",
            sent: 1204,
            hops: vec![
                hop(Some("router.lan"), 0.0, 1.1),
                hop(None, 1.0, 0.0),
                hop(Some("example.org"), 0.021, 45.6),
            ],
        }
    }

    /// The share of probes lost over the whole path.
    fn loss(&self) -> f64 {
        self.hops.iter().map(|hop| hop.loss).sum::<f64>() / self.hops.len() as f64
    }
}

/// Draws the frame.
pub fn draw(frame: &mut Frame, trace: &Trace) {
    let [main, hints, status] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let [hops, menu] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(16)]).areas(main);

    // A bordered table: a title with markup, headers, and cells in the
    // reader's number format.
    let header = Row::new([
        tr!("column.hop"),
        tr!("column.host"),
        tr!("column.loss"),
        tr!("column.average"),
    ])
    .bold();
    let rows = trace.hops.iter().enumerate().map(|(i, hop)| {
        let host = match hop.host {
            Some(name) => Cell::from(name),
            None => Cell::from(tr!("cell.no-reply")),
        };
        Row::new([
            Cell::from((i + 1).to_string()),
            host,
            Cell::from(tr!("cell.loss", share = hop.loss)),
            Cell::from(tr!("cell.ms", ms = hop.average)),
        ])
    });
    let widths = [
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(8),
        Constraint::Length(10),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(tr!("title", target = trace.target)));
    frame.render_widget(table, hops);

    // The language menu: each language named in itself, the current one bold.
    let items = Locale::ALL.iter().enumerate().map(|(i, &lang)| {
        let item = Line::from(vec![Span::raw(format!("{} ", i + 1)), Span::from(lang.name())]);
        if lang == current_locale() {
            item.bold()
        } else {
            item
        }
    });
    let list = List::new(items).block(Block::bordered().title(tr!("languages")));
    frame.render_widget(list, menu);

    // The key-hint bar is one message, so a translation places the keys.
    frame.render_widget(Line::from(tr!("hints")).right_aligned(), hints);

    // The status line: a plural, a count and a percentage.
    let summary = tr!(
        "status",
        hops = trace.hops.len(),
        sent = trace.sent,
        loss = trace.loss(),
    );
    frame.render_widget(Line::from(summary), status);
}
