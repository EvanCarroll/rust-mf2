//! The variants, each as a frame and as the three single-message cases.
//!
//! * `a` — 1.x: an app-owned `NativeI18n`, passed by reference;
//!   `format` → `String`; `mf2_ratatui::line` with `MarkupStyles`.
//! * `b` — the design: the ambient store with the per-thread settings copy;
//!   a simple message borrowed (`to_cow`), others one exact `String`; markup
//!   through the zero-copy sink and the app-wide theme (method R2).
//! * `b-copy` — the ambient store with 1.x's outputs (a `String` per text,
//!   1.x's sink): the store's own cost, everything else equal to `a`.
//! * `b-utf8`, `b-seam` — `b` with text parts recovered by method R1, or
//!   through the runtime's hidden seam (S, feature `seam`).
//! * `c` — `b`, reading the settings lock on every format.
//! * `d` — the trippy port's shape: `thread_local! { RefCell<NativeI18n> }`
//!   and a `t!`-like wrapper, `String` per message, 1.x's `line`.

use std::cell::RefCell;

use ambient::Method;
use mf2::{MarkupKind, Message, NoErrors, Part, PartSink};
use mf2_native::NativeI18n;
use mf2_ratatui::MarkupStyles;
use ratatui_core::text::{Line, Span};

use crate::frame::State;

// ------------------------------------------------------------------ (d)

std::thread_local! {
    /// The port's handle: one per thread, borrowed per message.
    pub static PORT: RefCell<Option<NativeI18n>> = const { RefCell::new(None) };
}

/// The port's `t!`: the thread's handle, borrowed, formatted to a `String`.
pub fn t(m: &impl Message) -> String {
    PORT.with(|p| p.borrow().as_ref().map(|i| i.format(m)).unwrap_or_default())
}

/// The port's markup line: the thread's handle, borrowed, 1.x's `line`.
pub fn t_line(m: &impl Message, styles: &MarkupStyles) -> Line<'static> {
    PORT.with(|p| {
        p.borrow()
            .as_ref()
            .map(|i| mf2_ratatui::line(i, m, styles))
            .unwrap_or_default()
    })
}

// ------------------------------------------------------------- frames

pub fn frame_a(i18n: &NativeI18n, styles: &MarkupStyles, st: &State, out: &mut Vec<Line<'static>>) {
    macro_rules! emit {
        (plain, $m:expr) => {
            out.push(Line::from(Span::raw(i18n.format(&$m))))
        };
        (rich, $m:expr) => {
            out.push(mf2_ratatui::line(i18n, &$m, styles))
        };
    }
    frame!(st);
}

pub fn frame_d(styles: &MarkupStyles, st: &State, out: &mut Vec<Line<'static>>) {
    macro_rules! emit {
        (plain, $m:expr) => {
            out.push(Line::from(Span::raw(t(&$m))))
        };
        (rich, $m:expr) => {
            out.push(t_line(&$m, styles))
        };
    }
    frame!(st);
}

pub fn frame_b_copy(styles: &MarkupStyles, st: &State, out: &mut Vec<Line<'static>>) {
    macro_rules! emit {
        (plain, $m:expr) => {
            out.push(Line::from(Span::raw(ambient::to_string::<false>(&$m))))
        };
        (rich, $m:expr) => {
            out.push(line_copy::<false>(&$m, styles))
        };
    }
    frame!(st);
}

pub fn frame_b<const LOCK: bool>(method: Method, st: &State, out: &mut Vec<Line<'static>>) {
    macro_rules! emit {
        (plain, $m:expr) => {
            out.push(Line::from(Span::raw(ambient::to_cow::<LOCK>(&$m))))
        };
        (rich, $m:expr) => {
            out.push(ambient::line::<LOCK>(&$m, method))
        };
    }
    frame!(st);
}

/// Per message of a frame, in frame order: `(kind, allocations)` for `a`
/// and for `b` — where each variant's allocations go.
pub fn breakdown(i18n: &NativeI18n, styles: &MarkupStyles, st: &State) -> Vec<(&'static str, u64, u64)> {
    let mut rows = Vec::with_capacity(crate::frame::MESSAGES);
    let mut out: Vec<Line<'static>> = Vec::with_capacity(2 * crate::frame::MESSAGES);
    macro_rules! emit {
        (plain, $m:expr) => {{
            let _ = crate::alloc::take();
            out.push(Line::from(Span::raw(i18n.format(&$m))));
            let a = crate::alloc::take().0;
            out.push(Line::from(Span::raw(ambient::to_cow::<false>(&$m))));
            let b = crate::alloc::take().0;
            rows.push(("plain", a, b));
        }};
        (rich, $m:expr) => {{
            let _ = crate::alloc::take();
            out.push(mf2_ratatui::line(i18n, &$m, styles));
            let a = crate::alloc::take().0;
            out.push(ambient::line::<false>(&$m, Method::RangePool));
            let b = crate::alloc::take().0;
            rows.push(("rich", a, b));
        }};
    }
    frame!(st);
    rows
}

// ------------------------------------------------ 1.x's sink, ambient

/// 1.x's `mf2_ratatui::line`, over the ambient formatter instead of a
/// `NativeI18n`: the sink is `mf2-ratatui`'s `Styled`, copied.
pub fn line_copy<const LOCK: bool>(m: &impl Message, styles: &MarkupStyles) -> Line<'static> {
    let mut out = Styled::new(styles);
    ambient::with_formatter::<LOCK, _>(|f| m.parts(f, &mut out, &mut NoErrors));
    out.finish().into_iter().next().unwrap_or_default()
}

struct Styled<'s> {
    styles: &'s MarkupStyles,
    open: Vec<(String, ratatui_core::style::Style)>,
    lines: Vec<Line<'static>>,
    spans: Vec<Span<'static>>,
    run: String,
    run_style: ratatui_core::style::Style,
}

impl<'s> Styled<'s> {
    fn new(styles: &'s MarkupStyles) -> Self {
        Styled {
            styles,
            open: Vec::new(),
            lines: Vec::new(),
            spans: Vec::new(),
            run: String::new(),
            run_style: ratatui_core::style::Style::new(),
        }
    }

    fn style(&self) -> ratatui_core::style::Style {
        self.open
            .last()
            .map_or_else(ratatui_core::style::Style::new, |(_, style)| *style)
    }

    fn push(&mut self, text: &str) {
        let style = self.style();
        if style != self.run_style {
            self.end_span();
            self.run_style = style;
        }
        let mut pieces = text.split('\n');
        if let Some(first) = pieces.next() {
            self.run.push_str(first);
        }
        for piece in pieces {
            self.run.push(' ');
            self.run.push_str(piece);
        }
    }

    fn end_span(&mut self) {
        if !self.run.is_empty() {
            let content = std::mem::take(&mut self.run);
            self.spans.push(Span::styled(content, self.run_style));
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.end_span();
        self.lines.push(Line::from(std::mem::take(&mut self.spans)));
        self.lines
    }
}

impl PartSink for Styled<'_> {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(text) => self.push(text),
            Part::Expression(expression) => {
                let mut text = String::new();
                expression.write(&mut text);
                self.push(&text);
            }
            Part::Fallback(source) => {
                let mut text = String::from("{");
                source.write(&mut text);
                text.push('}');
                self.push(&text);
            }
            Part::Markup(markup) => match markup.kind() {
                MarkupKind::Open => {
                    let outer = self.style();
                    let style = self
                        .styles
                        .get(markup.name())
                        .map_or(outer, |own| outer.patch(own));
                    self.open.push((markup.name().to_owned(), style));
                }
                MarkupKind::Close => {
                    if let Some(at) = self.open.iter().rposition(|(n, _)| n == markup.name()) {
                        self.open.truncate(at);
                    }
                }
                MarkupKind::Standalone => {}
            },
            _ => {}
        }
    }
}
