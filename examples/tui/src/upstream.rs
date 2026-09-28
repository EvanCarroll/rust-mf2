//! The baseline: the frame translated the way upstream trippy translates its
//! terminal UI, re-implemented here from a description of that approach
//! (`plans/18-phase-10-work-order.md`, A1), not from its code.
//!
//! - One TOML table per message, one value per locale, parsed once.
//! - The current locale is a thread-local `String`, and every lookup clones
//!   it.
//! - Arguments are `%{name}` placeholders, replaced one `str::replace` at a
//!   time.
//! - A plural is an English rule (`n > 1`) choosing between two words.
//! - Word order is assembled in code with `format!` and spans.
//! - A key hint is bolded by slicing the translated word: the key when the
//!   word starts with it, else `[key]` before the word.
//!
//! It is what `cargo xtask tui-gate` measures the MF2 renderer against, not
//! a pattern to follow: each of the points above is a class of bug MF2
//! removes, and the plural and the word order are visibly wrong in some of
//! the four languages.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, Widget};
use serde::Deserialize;

use crate::model::{Host, LOCALES, Model, ms, percent};

/// The locale a missing translation falls back to.
const FALLBACK: &str = "en";

thread_local! {
    static CURRENT: RefCell<String> = RefCell::new(String::from(FALLBACK));
}

/// Message key → locale → text.
#[derive(Deserialize)]
struct Catalog(HashMap<String, HashMap<String, String>>);

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!("../upstream/locales.toml"))
            .expect("the baseline's catalog is valid TOML")
    })
}

/// Chooses the locale: `tag` itself, else its language, else the fallback.
pub fn set_locale(tag: &str) {
    let language = tag.split(['-', '_']).next().unwrap_or(tag);
    let chosen = LOCALES
        .iter()
        .find(|&&l| l == tag)
        .or_else(|| LOCALES.iter().find(|&&l| l == language))
        .copied()
        .unwrap_or(FALLBACK);
    CURRENT.with(|current| *current.borrow_mut() = chosen.to_owned());
}

/// The current locale.
pub fn locale() -> String {
    CURRENT.with(|current| current.borrow().clone())
}

/// The text of `key` in the current locale (the thread's `String`, cloned
/// for every lookup), else in the fallback locale, else the key itself.
pub fn lookup(key: &'static str) -> &'static str {
    let locale = locale();
    match catalog().0.get(key) {
        Some(values) => values
            .get(locale.as_str())
            .or_else(|| values.get(FALLBACK))
            .map_or(key, String::as_str),
        None => key,
    }
}

/// `t!("key")` borrows the text; `t!("key", name = value, …)` replaces each
/// `%{name}` with `value.to_string()`, one `replace` at a time.
macro_rules! t {
    ($key:literal) => {
        Cow::<'static, str>::Borrowed(lookup($key))
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let text = t!($key);
        $(
            let text = text.replace(concat!("%{", stringify!($name), "}"), &$value.to_string());
        )+
        text
    }};
}

/// A key hint, bolded by slicing the translated word.
fn hint(key: &'static str, word: &'static str, bold: Style) -> Vec<Span<'static>> {
    if let Some(rest) = word.strip_prefix(key) {
        vec![Span::styled(key, bold), Span::raw(rest)]
    } else {
        vec![
            Span::raw("["),
            Span::styled(key, bold),
            Span::raw("]"),
            Span::raw(word),
        ]
    }
}

fn key_style() -> Style {
    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

fn host_style() -> Style {
    Style::new().add_modifier(Modifier::UNDERLINED)
}

/// Draws the whole frame into `buf`, in the current locale.
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

fn draw_header(model: &Model, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered().title(format!(" {} v{} ", t!("app_title"), model.version));
    let inner = block.inner(area);
    block.render(area, buf);
    let [left, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(84)]).areas(inner);
    let plural_flows = if model.flows.len() > 1 {
        t!("flows")
    } else {
        t!("flow")
    };
    Paragraph::new(vec![
        Line::from(vec![
            Span::raw(format!("{}: ", t!("target"))),
            Span::styled(model.source, host_style()),
            Span::raw(" → "),
            Span::styled(model.destination, host_style()),
        ]),
        Line::raw(t!("protocol", protocol = model.protocol)),
        Line::from(vec![
            Span::raw(format!("{}: ", t!("status"))),
            Span::styled(
                t!("status_running"),
                Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(t!(
            "status_failures",
            failure_count = model.failed,
            total_probes = model.sent,
            failure_rate = format!("{:.1}", model.failure_rate() * 100.0)
        )),
        Line::raw(t!(
            "discovered_flows",
            hop_count = model.hops.len(),
            flow_count = model.flows.len(),
            plural_flows = plural_flows
        )),
    ])
    .render(left, buf);

    let bold = key_style();
    let mut hints: Vec<Span<'static>> = Vec::new();
    for (key, word) in [
        ("h", lookup("header_help")),
        ("s", lookup("header_settings")),
        ("l", lookup("header_language")),
        ("f", lookup("header_freeze")),
        ("d", lookup("header_details")),
        ("c", lookup("header_chart")),
        ("q", lookup("header_quit")),
    ] {
        if !hints.is_empty() {
            hints.push(Span::raw("  "));
        }
        hints.extend(hint(key, word, bold));
    }
    Paragraph::new(vec![Line::from(hints), Line::raw(t!("privileged"))])
        .alignment(Alignment::Right)
        .render(right, buf);
}

fn draw_hops(model: &Model, area: Rect, buf: &mut Buffer) {
    let header = Row::new(vec![
        Cell::from(t!("column_hop")),
        Cell::from(t!("column_host")),
        Cell::from(t!("column_loss")),
        Cell::from(t!("column_sent")),
        Cell::from(t!("column_received")),
        Cell::from(t!("column_last")),
        Cell::from(t!("column_average")),
        Cell::from(t!("column_best")),
        Cell::from(t!("column_worst")),
        Cell::from(t!("column_deviation")),
        Cell::from(t!("column_jitter")),
        Cell::from(t!("column_status")),
        Cell::from(t!("column_asn")),
        Cell::from(t!("column_location")),
    ])
    .style(Style::new().add_modifier(Modifier::BOLD));
    let rows = model.hops.iter().map(|hop| {
        let host = match hop.host {
            Host::Name(name) => Cow::Borrowed(name),
            Host::NoResponse => t!("no_response"),
            Host::Resolving => t!("dns_pending"),
            Host::LookupFailed => t!("dns_failed"),
            Host::LookupTimedOut => t!("dns_timeout"),
            Host::Hidden => t!("hidden"),
        };
        let asn = match hop.asn {
            Some((number, _)) => Cow::Owned(format!("AS{number}")),
            None => t!("asn_pending"),
        };
        let location = match hop.location {
            Some((city, country)) => Cow::Owned(format!("{city}, {country}")),
            None => t!("unknown"),
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
        .block(Block::bordered().title(t!("title_hops")))
        .render(area, buf);
}

fn draw_details(model: &Model, area: Rect, buf: &mut Buffer) {
    let hop = model.selected_hop();
    let (asn, name) = hop.asn.unwrap_or((0, ""));
    let (city, country) = hop.location.unwrap_or(("", ""));
    let addresses = if hop.addresses > 1 {
        t!("addresses")
    } else {
        t!("address")
    };
    let bold = key_style();
    Paragraph::new(vec![
        Line::from(vec![
            Span::raw(format!("{}: ", t!("host"))),
            Span::styled(model.selected_name(), host_style()),
        ]),
        Line::raw(format!("{}: {}", t!("loss"), percent(hop.loss))),
        Line::raw(format!("{}: {}", t!("sent"), hop.sent)),
        Line::raw(format!("{}: {}", t!("received"), hop.received)),
        Line::raw(format!("{}: {} ms", t!("last"), ms(hop.last))),
        Line::raw(format!("{}: {} ms", t!("average"), ms(hop.average))),
        Line::raw(format!("{}: {} ms", t!("best"), ms(hop.best))),
        Line::raw(format!("{}: {} ms", t!("worst"), ms(hop.worst))),
        Line::raw(format!("{}: {} ms", t!("deviation"), ms(hop.deviation))),
        Line::raw(format!("{}: {} ms", t!("jitter"), ms(hop.jitter))),
        Line::raw(format!("AS{asn} · {name}")),
        Line::raw(format!("{}: {city}, {country}", t!("location"))),
        Line::raw(format!("{} {addresses}", hop.addresses)),
        Line::from(vec![
            Span::styled("←", bold),
            Span::raw(format!(" {} · ", t!("previous_hop"))),
            Span::styled("→", bold),
            Span::raw(format!(" {}", t!("next_hop"))),
        ]),
    ])
    .block(Block::bordered().title(format!("{} {}", t!("hop"), hop.ttl)))
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
        Line::raw(t!("samples")),
        Line::raw(t!("rtt")),
        Line::raw(format!("{} {ttl}", t!("hop"))),
        Line::raw(format!("{} ×{}", t!("zoom"), model.zoom)),
    ])
    .block(Block::bordered().title(t!("chart_title")))
    .render(chart, buf);
    Block::bordered()
        .title(format!("{} #{ttl}", t!("frequency")))
        .render(frequency, buf);
    Block::bordered()
        .title(format!("{} #{ttl}", t!("samples")))
        .render(history, buf);

    let rows = model.flows.iter().map(|flow| {
        let state = if flow.running {
            t!("flow_running")
        } else {
            t!("flow_frozen")
        };
        Row::new(vec![
            Cell::from(format!("{} {}", t!("flow_label"), flow.id)),
            Cell::from(state),
        ])
    });
    let count = model.flows.len();
    let noun = if count > 1 { t!("flows") } else { t!("flow") };
    Table::new(rows, [Constraint::Length(12), Constraint::Min(0)])
        .header(Row::new(vec![Cell::from(format!("{count} {noun}"))]))
        .block(Block::bordered().title(t!("title_flows")))
        .render(flows, buf);
}

fn draw_settings(model: &Model, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered().title(format!(" {} ", t!("title_settings")));
    let inner = block.inner(area);
    block.render(area, buf);
    let [info, tabs, values] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Min(0),
    ])
    .areas(inner);
    Line::raw(t!("settings_info")).render(info, buf);

    let tab_rows = vec![
        Row::new(vec![
            Cell::from(t!("settings_tab_interface_title")),
            Cell::from(t!("settings_tab_interface_desc")),
        ]),
        Row::new(vec![
            Cell::from(t!("settings_tab_trace_title")),
            Cell::from(t!("settings_tab_trace_desc")),
        ]),
        Row::new(vec![
            Cell::from(t!("settings_tab_dns_title")),
            Cell::from(t!("settings_tab_dns_desc")),
        ]),
        Row::new(vec![
            Cell::from(t!("settings_tab_geoip_title")),
            Cell::from(t!("settings_tab_geoip_desc")),
        ]),
        Row::new(vec![
            Cell::from(t!("settings_tab_keys_title")),
            Cell::from(t!("settings_tab_keys_desc")),
        ]),
        Row::new(vec![
            Cell::from(t!("settings_tab_theme_title")),
            Cell::from(t!("settings_tab_theme_desc")),
        ]),
        Row::new(vec![
            Cell::from(t!("settings_tab_columns_title")),
            Cell::from(t!("settings_tab_columns_desc")),
        ]),
    ];
    Table::new(tab_rows, [Constraint::Length(12), Constraint::Min(0)]).render(tabs, buf);

    Paragraph::new(vec![
        Line::raw(format!(
            "{}: {} ms",
            t!("probe_interval"),
            model.interval_ms
        )),
        Line::raw(format!("{}: {}", t!("max_ttl"), model.max_ttl)),
        Line::raw(format!("{}: {} ms", t!("timeout"), model.timeout_ms)),
        Line::raw(format!("{}: {}", t!("reverse_dns"), t!("on"))),
        Line::raw(format!("{}: {}", t!("mode"), t!("unprivileged"))),
        Line::raw(format!("{}: {}", t!("geoip_lookups"), t!("auto"))),
        Line::raw(format!("{}: {}", t!("asn_lookups"), t!("off"))),
    ])
    .render(values, buf);
}

fn draw_help(area: Rect, buf: &mut Buffer) {
    Paragraph::new(vec![
        Line::raw(t!("tagline")),
        Line::raw(t!("help_show_settings", key = "s")),
        Line::raw(t!("help_show_bindings", key = "k")),
        Line::raw(t!("help_show_columns", key = "c")),
        Line::raw(t!("help_license")),
        Line::raw(t!("help_copyright")),
        Line::raw(t!("help_close")),
    ])
    .block(Block::bordered().title(format!(" {} ", t!("title_help"))))
    .render(area, buf);
}

fn draw_language(area: Rect, buf: &mut Buffer) {
    let current = locale();
    let lines: Vec<Line<'static>> = LOCALES
        .iter()
        .map(|&tag| {
            let name = match tag {
                "de" => t!("language_de"),
                "es" => t!("language_es"),
                "fr" => t!("language_fr"),
                _ => t!("language_en"),
            };
            if tag == current {
                Line::styled(
                    t!("language_current", name = name),
                    Style::new().add_modifier(Modifier::BOLD),
                )
            } else {
                Line::raw(name)
            }
        })
        .collect();
    Paragraph::new(lines)
        .block(Block::bordered().title(t!("title_language")))
        .render(area, buf);
}

fn draw_log(model: &Model, area: Rect, buf: &mut Buffer) {
    let alert = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
    Paragraph::new(vec![
        Line::raw(t!("awaiting_data")),
        Line::from(vec![
            Span::raw(format!("{} ", t!("trace_started"))),
            Span::styled(model.destination, host_style()),
        ]),
        Line::from(vec![
            Span::styled(t!("permission_denied"), alert),
            Span::raw(format!(": {}", t!("permission_hint"))),
        ]),
        Line::from(vec![
            Span::styled(t!("resolve_failed"), alert),
            Span::raw(" "),
            Span::styled(model.unresolved, host_style()),
        ]),
        Line::from(vec![
            Span::raw(format!("{}: ", t!("status"))),
            Span::styled(t!("status_frozen"), Style::new().fg(Color::Yellow)),
            Span::raw(format!(
                " {}",
                t!("frozen_for", seconds = model.frozen_seconds)
            )),
        ]),
        Line::raw(t!("dns_lookups_pending", count = model.dns_pending)),
        Line::raw(t!("trace_failed", error = model.error)),
        Line::raw(t!("press_to_quit", key = "q")),
    ])
    .block(Block::bordered().title(t!("title_events")))
    .render(area, buf);
}

fn draw_footer(model: &Model, area: Rect, buf: &mut Buffer) {
    let spans = vec![
        Span::raw(t!(
            "summary",
            hops = model.hops.len(),
            sent = model.sent,
            failed = model.failed
        )),
        Span::raw(" │ "),
        Span::raw(t!(
            "loss_rate",
            rate = format!("{:.1}", model.failure_rate() * 100.0)
        )),
        Span::raw(" │ "),
        Span::raw(t!("running_for", minutes = model.elapsed_minutes)),
        Span::raw(" │ "),
        Span::raw(t!("privacy", ttl = model.privacy_ttl)),
        Span::raw(" │ "),
        Span::styled(t!("frozen"), Style::new().fg(Color::Yellow)),
        Span::raw(": "),
        Span::raw(t!("press_to_resume", key = "f")),
    ];
    Line::from(spans).render(area, buf);
}
