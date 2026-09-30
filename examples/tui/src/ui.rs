//! The frame, translated with MF2 as the user guide's native page writes an
//! application: `tr!` wherever Ratatui takes text, in the language in force,
//! and the markup's styles set once, as a theme.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, Widget};

use crate::model::{Host, Model, ms, percent};
use crate::prelude::*;

/// What each markup name looks like. The messages say what a stretch is;
/// this says how it is drawn. `main` sets it once, with `set_theme`.
pub fn theme() -> mf2::ratatui::Theme {
    mf2::ratatui::Theme::default()
        .style(crate::markup::KEY, Style::new().yellow().bold())
        .style(crate::markup::HOST, Style::new().underlined())
        .style(crate::markup::OK, Style::new().green().bold())
        .style(crate::markup::WARN, Style::new().yellow())
        .style(crate::markup::ALERT, Style::new().red().bold())
}

/// Draws the whole frame into `buf`.
pub fn draw(model: &Model, area: Rect, buf: &mut Buffer) {
    let [header, middle, charts, bottom, footer] = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(16),
        Constraint::Length(6),
        Constraint::Length(17),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(model, header, buf);
    let [hops, details] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(46)]).areas(middle);
    draw_hops(model, hops, buf);
    draw_details(model, details, buf);
    draw_charts(model, charts, buf);
    let [settings, help, language, log] = Layout::horizontal([
        Constraint::Length(62),
        Constraint::Length(44),
        Constraint::Length(20),
        Constraint::Min(0),
    ])
    .areas(bottom);
    draw_settings(model, settings, buf);
    draw_help(help, buf);
    draw_language(language, buf);
    draw_log(model, log, buf);
    draw_footer(model, footer, buf);
}

fn draw_header(model: &Model,
    area: Rect,
    buf: &mut Buffer,
) {
    let block = Block::bordered().title(Line::from(tr!("app.title", version = model.version)));
    let inner = block.inner(area);
    block.render(area, buf);
    let [left, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(84)]).areas(inner);
    Paragraph::new(vec![
        Line::from(tr!(
                "header.target",
                source = model.source,
                destination = model.destination
            )),
        Line::from(tr!("header.protocol", protocol = model.protocol)),
        Line::from(tr!("header.status-running")),
        Line::from(tr!(
                "header.failures",
                failed = model.failed,
                total = model.sent,
                rate = model.failure_rate()
            )),
        Line::from(tr!(
                "header.discovered",
                hops = model.hops.len(),
                flows = model.flows.len()
            )),
    ])
    .render(left, buf);

    let mut hints: Vec<Span<'static>> = Vec::new();
    for hint in [
        Line::from(tr!("hint.help")),
        Line::from(tr!("hint.settings")),
        Line::from(tr!("hint.language")),
        Line::from(tr!("hint.freeze")),
        Line::from(tr!("hint.details")),
        Line::from(tr!("hint.chart")),
        Line::from(tr!("hint.quit")),
    ] {
        if !hints.is_empty() {
            hints.push(Span::raw("  "));
        }
        hints.extend(hint.spans);
    }
    Paragraph::new(vec![
        Line::from(hints),
        Line::from(tr!("header.privileged")),
    ])
    .alignment(Alignment::Right)
    .render(right, buf);
}

fn draw_hops(model: &Model, area: Rect, buf: &mut Buffer) {
    let header = Row::new(vec![
        Cell::from(tr!("column.hop")),
        Cell::from(tr!("column.host")),
        Cell::from(tr!("column.loss")),
        Cell::from(tr!("column.sent")),
        Cell::from(tr!("column.received")),
        Cell::from(tr!("column.last")),
        Cell::from(tr!("column.average")),
        Cell::from(tr!("column.best")),
        Cell::from(tr!("column.worst")),
        Cell::from(tr!("column.deviation")),
        Cell::from(tr!("column.jitter")),
        Cell::from(tr!("column.status")),
        Cell::from(tr!("column.asn")),
        Cell::from(tr!("column.location")),
    ])
    .bold();
    let rows = model.hops.iter().map(|hop| {
        let host = match hop.host {
            Host::Name(name) => Cell::from(name),
            Host::NoResponse => Cell::from(tr!("status.no-response")),
            Host::Resolving => Cell::from(tr!("details.dns-pending")),
            Host::LookupFailed => Cell::from(tr!("details.dns-failed")),
            Host::LookupTimedOut => Cell::from(tr!("details.dns-timeout")),
            Host::Hidden => Cell::from(tr!("header.hidden")),
        };
        let asn = match hop.asn {
            Some((number, _)) => Cell::from(format!("AS{number}")),
            None => Cell::from(tr!("details.asn-pending")),
        };
        let location = match hop.location {
            Some((city, country)) => Cell::from(format!("{city}, {country}")),
            None => Cell::from(tr!("header.unknown")),
        };
        let status = if hop.received > 0 { "✓" } else { "✗" };
        Row::new(vec![
            Cell::from(hop.ttl.to_string()),
            host,
            Cell::from(percent(hop.loss)),
            Cell::from(hop.sent.to_string()),
            Cell::from(hop.received.to_string()),
            Cell::from(ms(hop.last)),
            Cell::from(ms(hop.average)),
            Cell::from(ms(hop.best)),
            Cell::from(ms(hop.worst)),
            Cell::from(ms(hop.deviation)),
            Cell::from(ms(hop.jitter)),
            Cell::from(status),
            asn,
            location,
        ])
    });
    let widths = [
        Constraint::Length(3),
        Constraint::Min(18),
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Length(10),
        Constraint::Length(14),
    ];
    Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(tr!("hops.title")))
        .render(area, buf);
}

fn draw_details(model: &Model,
    area: Rect,
    buf: &mut Buffer,
) {
    let hop = model.selected_hop();
    let (asn, name) = hop.asn.unwrap_or((0, ""));
    let (city, country) = hop.location.unwrap_or(("", ""));
    Paragraph::new(vec![
        Line::from(tr!("details.host", host = model.selected_name())),
        Line::from(tr!("details.loss", loss = hop.loss)),
        Line::from(tr!("details.sent", sent = hop.sent)),
        Line::from(tr!("details.received", received = hop.received)),
        Line::from(tr!("details.last", ms = hop.last)),
        Line::from(tr!("details.average", ms = hop.average)),
        Line::from(tr!("details.best", ms = hop.best)),
        Line::from(tr!("details.worst", ms = hop.worst)),
        Line::from(tr!("details.deviation", ms = hop.deviation)),
        Line::from(tr!("details.jitter", ms = hop.jitter)),
        Line::from(tr!("details.asn", asn = asn, name = name)),
        Line::from(tr!("details.location", city = city, country = country)),
        Line::from(tr!("details.addresses", count = hop.addresses)),
        Line::from(tr!("details.nav")),
    ])
    .block(Block::bordered().title(tr!("details.title", ttl = hop.ttl)))
    .render(area, buf);
}

fn draw_charts(model: &Model, area: Rect, buf: &mut Buffer) {
    let ttl = model.selected_hop().ttl;
    let [chart, frequency, history, flows] = Layout::horizontal([
        Constraint::Length(40),
        Constraint::Length(34),
        Constraint::Length(34),
        Constraint::Min(0),
    ])
    .areas(area);
    Paragraph::new(vec![
        Line::from(tr!("chart.samples")),
        Line::from(tr!("chart.rtt")),
        Line::from(tr!("chart.hop", ttl = ttl)),
        Line::from(tr!("chart.zoom", factor = model.zoom)),
    ])
    .block(Block::bordered().title(tr!("chart.title")))
    .render(chart, buf);
    Block::bordered()
        .title(tr!("chart.frequency", ttl = ttl))
        .render(frequency, buf);
    Block::bordered()
        .title(tr!("chart.history", ttl = ttl))
        .render(history, buf);

    let rows = model.flows.iter().map(|flow| {
        let state = if flow.running {
            tr!("flows.running")
        } else {
            tr!("flows.frozen")
        };
        Row::new(vec![
            Cell::from(tr!("flows.selected", id = flow.id)),
            Cell::from(state),
        ])
    });
    Table::new(rows, [Constraint::Length(12), Constraint::Min(0)])
        .header(Row::new(vec![Cell::from(
            tr!("flows.count", count = model.flows.len()),
        )]))
        .block(Block::bordered().title(tr!("flows.title")))
        .render(flows, buf);
}

fn draw_settings(model: &Model,
    area: Rect,
    buf: &mut Buffer,
) {
    let block = Block::bordered().title(tr!("settings.title"));
    let inner = block.inner(area);
    block.render(area, buf);
    let [info, tabs, values] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Min(0),
    ])
    .areas(inner);
    Line::from(tr!("settings.info")).render(info, buf);

    let tab_rows = vec![
        Row::new(vec![
            Cell::from(tr!("settings.tab-interface")),
            Cell::from(tr!("settings.tab-interface-about")),
        ]),
        Row::new(vec![
            Cell::from(tr!("settings.tab-trace")),
            Cell::from(tr!("settings.tab-trace-about")),
        ]),
        Row::new(vec![
            Cell::from(tr!("settings.tab-dns")),
            Cell::from(tr!("settings.tab-dns-about")),
        ]),
        Row::new(vec![
            Cell::from(tr!("settings.tab-geoip")),
            Cell::from(tr!("settings.tab-geoip-about")),
        ]),
        Row::new(vec![
            Cell::from(tr!("settings.tab-keys")),
            Cell::from(tr!("settings.tab-keys-about")),
        ]),
        Row::new(vec![
            Cell::from(tr!("settings.tab-theme")),
            Cell::from(tr!("settings.tab-theme-about")),
        ]),
        Row::new(vec![
            Cell::from(tr!("settings.tab-columns")),
            Cell::from(tr!("settings.tab-columns-about")),
        ]),
    ];
    Table::new(tab_rows, [Constraint::Length(12), Constraint::Min(0)]).render(tabs, buf);

    let on = tr!("settings.on").to_string();
    let off = tr!("settings.off").to_string();
    let auto = tr!("settings.auto").to_string();
    let mode = tr!("header.unprivileged").to_string();
    Paragraph::new(vec![
        Line::from(tr!("settings.interval", ms = model.interval_ms)),
        Line::from(tr!("settings.max-ttl", hops = model.max_ttl)),
        Line::from(tr!("settings.timeout", ms = model.timeout_ms)),
        Line::from(tr!("settings.reverse-dns", state = on)),
        Line::from(tr!("settings.mode", mode = mode)),
        Line::from(tr!("settings.geoip", state = auto)),
        Line::from(tr!("settings.asn", state = off)),
    ])
    .render(values, buf);
}

fn draw_help(area: Rect, buf: &mut Buffer) {
    Paragraph::new(vec![
        Line::from(tr!("app.tagline")),
        Line::from(tr!("help.settings")),
        Line::from(tr!("help.keys")),
        Line::from(tr!("help.columns")),
        Line::from(tr!("help.license")),
        Line::from(tr!("help.copyright")),
        Line::from(tr!("help.close")),
    ])
    .block(Block::bordered().title(tr!("help.title")))
    .render(area, buf);
}

fn draw_language(area: Rect, buf: &mut Buffer) {
    // Each language named in itself, from its `language.<tag>` message.
    let current = current_locale();
    let lines: Vec<Line<'static>> = Locale::ALL
        .iter()
        .map(|&lang| {
            if lang == current {
                let name = lang.name().to_string();
                Line::from(tr!("languages.current", name = name)).bold()
            } else {
                Line::from(lang.name())
            }
        })
        .collect();
    Paragraph::new(lines)
        .block(Block::bordered().title(tr!("languages.title")))
        .render(area, buf);
}

fn draw_log(model: &Model, area: Rect, buf: &mut Buffer) {
    Paragraph::new(vec![
        Line::from(tr!("log.awaiting")),
        Line::from(tr!("log.started", host = model.destination)),
        Line::from(tr!("log.permission")),
        Line::from(tr!("log.resolve", host = model.unresolved)),
        Line::from(tr!("header.status-frozen", seconds = model.frozen_seconds)),
        Line::from(tr!("log.dns-pending", count = model.dns_pending)),
        Line::from(tr!("log.failed", error = model.error)),
        Line::from(tr!("log.quit")),
    ])
    .block(Block::bordered().title(tr!("log.title")))
    .render(area, buf);
}

fn draw_footer(model: &Model,
    area: Rect,
    buf: &mut Buffer,
) {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for part in [
        Line::from(tr!(
                "status.summary",
                hops = model.hops.len(),
                sent = model.sent,
                failed = model.failed
            )),
        Line::from(tr!("status.loss", rate = model.failure_rate())),
        Line::from(tr!("status.elapsed", minutes = model.elapsed_minutes)),
        Line::from(tr!("status.privacy", ttl = model.privacy_ttl)),
        Line::from(tr!("status.frozen")),
    ] {
        if !spans.is_empty() {
            spans.push(Span::raw(" │ "));
        }
        spans.extend(part.spans);
    }
    Line::from(spans).render(area, buf);
}
