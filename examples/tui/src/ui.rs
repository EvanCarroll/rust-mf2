//! The frame, translated with MF2 through the 1.x API, as the user guide's
//! native page writes an application: the `NativeI18n` handle passed to
//! everything that makes text, `mf2_ratatui::line` for a message with markup
//! and `NativeI18n::format` for plain text, and the markup's styles built
//! for each draw.

use demo_tui_i18n::tr;
use mf2_native::NativeI18n;
use mf2_ratatui::{MarkupStyles, line};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, Widget};

use crate::model::{Host, LOCALES, Model, ms, percent};

/// What each markup name looks like. The messages say what a stretch is;
/// this says how it is drawn.
fn styles() -> MarkupStyles {
    MarkupStyles::new()
        .with(
            "key",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
        .with("host", Style::new().add_modifier(Modifier::UNDERLINED))
        .with(
            "ok",
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
        .with("warn", Style::new().fg(Color::Yellow))
        .with(
            "alert",
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
}

/// Draws the whole frame into `buf`.
pub fn draw(i18n: &NativeI18n, model: &Model, area: Rect, buf: &mut Buffer) {
    let styles = styles();
    let [header, middle, charts, bottom, footer] = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(16),
        Constraint::Length(6),
        Constraint::Length(17),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(i18n, &styles, model, header, buf);
    let [hops, details] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(46)]).areas(middle);
    draw_hops(i18n, model, hops, buf);
    draw_details(i18n, &styles, model, details, buf);
    draw_charts(i18n, model, charts, buf);
    let [settings, help, language, log] = Layout::horizontal([
        Constraint::Length(62),
        Constraint::Length(44),
        Constraint::Length(20),
        Constraint::Min(0),
    ])
    .areas(bottom);
    draw_settings(i18n, &styles, model, settings, buf);
    draw_help(i18n, &styles, help, buf);
    draw_language(i18n, language, buf);
    draw_log(i18n, &styles, model, log, buf);
    draw_footer(i18n, &styles, model, footer, buf);
}

fn draw_header(
    i18n: &NativeI18n,
    styles: &MarkupStyles,
    model: &Model,
    area: Rect,
    buf: &mut Buffer,
) {
    let block = Block::bordered().title(line(
        i18n,
        &tr!("app.title", version = model.version),
        styles,
    ));
    let inner = block.inner(area);
    block.render(area, buf);
    let [left, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(84)]).areas(inner);
    Paragraph::new(vec![
        line(
            i18n,
            &tr!(
                "header.target",
                source = model.source,
                destination = model.destination
            ),
            styles,
        ),
        line(
            i18n,
            &tr!("header.protocol", protocol = model.protocol),
            styles,
        ),
        line(i18n, &tr!("header.status-running"), styles),
        line(
            i18n,
            &tr!(
                "header.failures",
                failed = model.failed,
                total = model.sent,
                rate = model.failure_rate()
            ),
            styles,
        ),
        line(
            i18n,
            &tr!(
                "header.discovered",
                hops = model.hops.len(),
                flows = model.flows.len()
            ),
            styles,
        ),
    ])
    .render(left, buf);

    let mut hints: Vec<Span<'static>> = Vec::new();
    for hint in [
        line(i18n, &tr!("hint.help"), styles),
        line(i18n, &tr!("hint.settings"), styles),
        line(i18n, &tr!("hint.language"), styles),
        line(i18n, &tr!("hint.freeze"), styles),
        line(i18n, &tr!("hint.details"), styles),
        line(i18n, &tr!("hint.chart"), styles),
        line(i18n, &tr!("hint.quit"), styles),
    ] {
        if !hints.is_empty() {
            hints.push(Span::raw("  "));
        }
        hints.extend(hint.spans);
    }
    Paragraph::new(vec![
        Line::from(hints),
        Line::raw(i18n.format(&tr!("header.privileged"))),
    ])
    .alignment(Alignment::Right)
    .render(right, buf);
}

fn draw_hops(i18n: &NativeI18n, model: &Model, area: Rect, buf: &mut Buffer) {
    let header = Row::new(vec![
        Cell::from(i18n.format(&tr!("column.hop"))),
        Cell::from(i18n.format(&tr!("column.host"))),
        Cell::from(i18n.format(&tr!("column.loss"))),
        Cell::from(i18n.format(&tr!("column.sent"))),
        Cell::from(i18n.format(&tr!("column.received"))),
        Cell::from(i18n.format(&tr!("column.last"))),
        Cell::from(i18n.format(&tr!("column.average"))),
        Cell::from(i18n.format(&tr!("column.best"))),
        Cell::from(i18n.format(&tr!("column.worst"))),
        Cell::from(i18n.format(&tr!("column.deviation"))),
        Cell::from(i18n.format(&tr!("column.jitter"))),
        Cell::from(i18n.format(&tr!("column.status"))),
        Cell::from(i18n.format(&tr!("column.asn"))),
        Cell::from(i18n.format(&tr!("column.location"))),
    ])
    .style(Style::new().add_modifier(Modifier::BOLD));
    let rows = model.hops.iter().map(|hop| {
        let host = match hop.host {
            Host::Name(name) => name.to_owned(),
            Host::NoResponse => i18n.format(&tr!("status.no-response")),
            Host::Resolving => i18n.format(&tr!("details.dns-pending")),
            Host::LookupFailed => i18n.format(&tr!("details.dns-failed")),
            Host::LookupTimedOut => i18n.format(&tr!("details.dns-timeout")),
            Host::Hidden => i18n.format(&tr!("header.hidden")),
        };
        let asn = match hop.asn {
            Some((number, _)) => format!("AS{number}"),
            None => i18n.format(&tr!("details.asn-pending")),
        };
        let location = match hop.location {
            Some((city, country)) => format!("{city}, {country}"),
            None => i18n.format(&tr!("header.unknown")),
        };
        let status = if hop.received > 0 { "✓" } else { "✗" };
        Row::new(vec![
            Cell::from(hop.ttl.to_string()),
            Cell::from(host),
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
            Cell::from(asn),
            Cell::from(location),
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
        .block(Block::bordered().title(i18n.format(&tr!("hops.title"))))
        .render(area, buf);
}

fn draw_details(
    i18n: &NativeI18n,
    styles: &MarkupStyles,
    model: &Model,
    area: Rect,
    buf: &mut Buffer,
) {
    let hop = model.selected_hop();
    let (asn, name) = hop.asn.unwrap_or((0, ""));
    let (city, country) = hop.location.unwrap_or(("", ""));
    Paragraph::new(vec![
        line(
            i18n,
            &tr!("details.host", host = model.selected_name()),
            styles,
        ),
        line(i18n, &tr!("details.loss", loss = hop.loss), styles),
        line(i18n, &tr!("details.sent", sent = hop.sent), styles),
        line(
            i18n,
            &tr!("details.received", received = hop.received),
            styles,
        ),
        line(i18n, &tr!("details.last", ms = hop.last), styles),
        line(i18n, &tr!("details.average", ms = hop.average), styles),
        line(i18n, &tr!("details.best", ms = hop.best), styles),
        line(i18n, &tr!("details.worst", ms = hop.worst), styles),
        line(i18n, &tr!("details.deviation", ms = hop.deviation), styles),
        line(i18n, &tr!("details.jitter", ms = hop.jitter), styles),
        line(i18n, &tr!("details.asn", asn = asn, name = name), styles),
        line(
            i18n,
            &tr!("details.location", city = city, country = country),
            styles,
        ),
        line(
            i18n,
            &tr!("details.addresses", count = hop.addresses),
            styles,
        ),
        line(i18n, &tr!("details.nav"), styles),
    ])
    .block(Block::bordered().title(i18n.format(&tr!("details.title", ttl = hop.ttl))))
    .render(area, buf);
}

fn draw_charts(i18n: &NativeI18n, model: &Model, area: Rect, buf: &mut Buffer) {
    let ttl = model.selected_hop().ttl;
    let [chart, frequency, history, flows] = Layout::horizontal([
        Constraint::Length(40),
        Constraint::Length(34),
        Constraint::Length(34),
        Constraint::Min(0),
    ])
    .areas(area);
    Paragraph::new(vec![
        Line::raw(i18n.format(&tr!("chart.samples"))),
        Line::raw(i18n.format(&tr!("chart.rtt"))),
        Line::raw(i18n.format(&tr!("chart.hop", ttl = ttl))),
        Line::raw(i18n.format(&tr!("chart.zoom", factor = model.zoom))),
    ])
    .block(Block::bordered().title(i18n.format(&tr!("chart.title"))))
    .render(chart, buf);
    Block::bordered()
        .title(i18n.format(&tr!("chart.frequency", ttl = ttl)))
        .render(frequency, buf);
    Block::bordered()
        .title(i18n.format(&tr!("chart.history", ttl = ttl)))
        .render(history, buf);

    let rows = model.flows.iter().map(|flow| {
        let state = if flow.running {
            i18n.format(&tr!("flows.running"))
        } else {
            i18n.format(&tr!("flows.frozen"))
        };
        Row::new(vec![
            Cell::from(i18n.format(&tr!("flows.selected", id = flow.id))),
            Cell::from(state),
        ])
    });
    Table::new(rows, [Constraint::Length(12), Constraint::Min(0)])
        .header(Row::new(vec![Cell::from(
            i18n.format(&tr!("flows.count", count = model.flows.len())),
        )]))
        .block(Block::bordered().title(i18n.format(&tr!("flows.title"))))
        .render(flows, buf);
}

fn draw_settings(
    i18n: &NativeI18n,
    styles: &MarkupStyles,
    model: &Model,
    area: Rect,
    buf: &mut Buffer,
) {
    let block = Block::bordered().title(i18n.format(&tr!("settings.title")));
    let inner = block.inner(area);
    block.render(area, buf);
    let [info, tabs, values] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Min(0),
    ])
    .areas(inner);
    line(i18n, &tr!("settings.info"), styles).render(info, buf);

    let tab_rows = vec![
        Row::new(vec![
            Cell::from(i18n.format(&tr!("settings.tab-interface"))),
            Cell::from(i18n.format(&tr!("settings.tab-interface-about"))),
        ]),
        Row::new(vec![
            Cell::from(i18n.format(&tr!("settings.tab-trace"))),
            Cell::from(i18n.format(&tr!("settings.tab-trace-about"))),
        ]),
        Row::new(vec![
            Cell::from(i18n.format(&tr!("settings.tab-dns"))),
            Cell::from(i18n.format(&tr!("settings.tab-dns-about"))),
        ]),
        Row::new(vec![
            Cell::from(i18n.format(&tr!("settings.tab-geoip"))),
            Cell::from(i18n.format(&tr!("settings.tab-geoip-about"))),
        ]),
        Row::new(vec![
            Cell::from(i18n.format(&tr!("settings.tab-keys"))),
            Cell::from(i18n.format(&tr!("settings.tab-keys-about"))),
        ]),
        Row::new(vec![
            Cell::from(i18n.format(&tr!("settings.tab-theme"))),
            Cell::from(i18n.format(&tr!("settings.tab-theme-about"))),
        ]),
        Row::new(vec![
            Cell::from(i18n.format(&tr!("settings.tab-columns"))),
            Cell::from(i18n.format(&tr!("settings.tab-columns-about"))),
        ]),
    ];
    Table::new(tab_rows, [Constraint::Length(12), Constraint::Min(0)]).render(tabs, buf);

    let on = i18n.format(&tr!("settings.on"));
    let off = i18n.format(&tr!("settings.off"));
    let auto = i18n.format(&tr!("settings.auto"));
    let mode = i18n.format(&tr!("header.unprivileged"));
    Paragraph::new(vec![
        Line::raw(i18n.format(&tr!("settings.interval", ms = model.interval_ms))),
        Line::raw(i18n.format(&tr!("settings.max-ttl", hops = model.max_ttl))),
        Line::raw(i18n.format(&tr!("settings.timeout", ms = model.timeout_ms))),
        Line::raw(i18n.format(&tr!("settings.reverse-dns", state = on))),
        Line::raw(i18n.format(&tr!("settings.mode", mode = mode))),
        Line::raw(i18n.format(&tr!("settings.geoip", state = auto))),
        Line::raw(i18n.format(&tr!("settings.asn", state = off))),
    ])
    .render(values, buf);
}

fn draw_help(i18n: &NativeI18n, styles: &MarkupStyles, area: Rect, buf: &mut Buffer) {
    Paragraph::new(vec![
        line(i18n, &tr!("app.tagline"), styles),
        line(i18n, &tr!("help.settings"), styles),
        line(i18n, &tr!("help.keys"), styles),
        line(i18n, &tr!("help.columns"), styles),
        line(i18n, &tr!("help.license"), styles),
        line(i18n, &tr!("help.copyright"), styles),
        line(i18n, &tr!("help.close"), styles),
    ])
    .block(Block::bordered().title(i18n.format(&tr!("help.title"))))
    .render(area, buf);
}

fn draw_language(i18n: &NativeI18n, area: Rect, buf: &mut Buffer) {
    let current = i18n.locale();
    let lines: Vec<Line<'static>> = LOCALES
        .iter()
        .map(|&tag| {
            let name = match tag {
                "de" => i18n.format(&tr!("language-name.de")),
                "es" => i18n.format(&tr!("language-name.es")),
                "fr" => i18n.format(&tr!("language-name.fr")),
                _ => i18n.format(&tr!("language-name.en")),
            };
            if tag == current {
                Line::styled(
                    i18n.format(&tr!("language.current", name = name)),
                    Style::new().add_modifier(Modifier::BOLD),
                )
            } else {
                Line::raw(name)
            }
        })
        .collect();
    Paragraph::new(lines)
        .block(Block::bordered().title(i18n.format(&tr!("language.title"))))
        .render(area, buf);
}

fn draw_log(i18n: &NativeI18n, styles: &MarkupStyles, model: &Model, area: Rect, buf: &mut Buffer) {
    Paragraph::new(vec![
        line(i18n, &tr!("log.awaiting"), styles),
        line(i18n, &tr!("log.started", host = model.destination), styles),
        line(i18n, &tr!("log.permission"), styles),
        line(i18n, &tr!("log.resolve", host = model.unresolved), styles),
        line(
            i18n,
            &tr!("header.status-frozen", seconds = model.frozen_seconds),
            styles,
        ),
        line(
            i18n,
            &tr!("log.dns-pending", count = model.dns_pending),
            styles,
        ),
        line(i18n, &tr!("log.failed", error = model.error), styles),
        line(i18n, &tr!("log.quit"), styles),
    ])
    .block(Block::bordered().title(i18n.format(&tr!("log.title"))))
    .render(area, buf);
}

fn draw_footer(
    i18n: &NativeI18n,
    styles: &MarkupStyles,
    model: &Model,
    area: Rect,
    buf: &mut Buffer,
) {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for part in [
        line(
            i18n,
            &tr!(
                "status.summary",
                hops = model.hops.len(),
                sent = model.sent,
                failed = model.failed
            ),
            styles,
        ),
        line(
            i18n,
            &tr!("status.loss", rate = model.failure_rate()),
            styles,
        ),
        line(
            i18n,
            &tr!("status.elapsed", minutes = model.elapsed_minutes),
            styles,
        ),
        line(
            i18n,
            &tr!("status.privacy", ttl = model.privacy_ttl),
            styles,
        ),
        line(i18n, &tr!("status.frozen"), styles),
    ] {
        if !spans.is_empty() {
            spans.push(Span::raw(" │ "));
        }
        spans.extend(part.spans);
    }
    Line::from(spans).render(area, buf);
}
