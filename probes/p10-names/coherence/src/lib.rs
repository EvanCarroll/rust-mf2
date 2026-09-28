//! What a Ratatui application writes with the call-site types (Phase 10 A7,
//! item 4), compiled with a Leptos mode on as well — cargo unifies features
//! across a workspace, so a web crate and a TUI crate meet in one build.

use leptos::prelude::{ElementChild, GlobalAttributes, IntoView, view};
use leptos_mf2::{Tr, TrArgs, TrDyn, TrRich};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Cell, List, ListItem, Paragraph, Row, Table, Tabs, Widget};

/// Every position of a trippy-shaped frame, fed a description directly.
pub fn positions(t: Tr, a: TrArgs, r: TrRich, d: TrDyn) {
    // Titles and paragraphs: `Into<Line>`, `Into<Text>`.
    let _ = Paragraph::new(t).block(Block::bordered().title(t));
    let _ = Paragraph::new(a.clone());
    let _ = Paragraph::new(r.clone());
    let _ = Paragraph::new(d.clone());
    // Table headers and rows: `Into<Cell>` through Ratatui's blanket over
    // `Into<Text>`, with no impl of ours.
    let header = Row::new([t, t]);
    let rows = [Row::new(vec![Cell::from(t), Cell::from(a.clone())])];
    let _ = Table::new(rows, [Constraint::Length(8), Constraint::Fill(1)]).header(header);
    // Lists and tabs: `Into<ListItem>` / `Into<Line>` through the blankets.
    let _ = ListItem::new(t);
    let _ = List::new([t, t]);
    let _ = Tabs::new([t, t]);
    // `Stylize` through `Styled<Item = Line>`: a line that keeps the
    // message's own styles and adds the call's.
    let bold: Line<'static> = t.bold();
    let _: Line<'static> = t.fg(Color::Yellow).italic();
    let _ = bold;
    // `Line` collects descriptions through its blanket over `Into<Span>`.
    let _: Line<'static> = [t, t].into_iter().collect();
    let _: Span<'static> = t.into();
    let _: Text<'static> = r.into();
    let _ = Line::from(vec![Span::from(t), Span::raw(" "), Span::from(d)]);
    // `{}` and `to_string()`: the inherent method wins over `ToString`.
    let _ = std::format!("{t} {a}");
    let _: String = t.to_string();
}

/// A description drawn as a widget.
pub fn draw(t: Tr, area: Rect, buf: &mut Buffer) {
    t.render(area, buf);
}

/// The Leptos glue in the same scope as the Ratatui impls.
pub fn view(t: Tr) -> impl IntoView {
    view! { <p title=t>{t}</p> }
}
