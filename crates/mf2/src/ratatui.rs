//! Ratatui text from a native application's messages, with the messages'
//! markup as styles.
//!
//! MF2 markup — `{#name}…{/name}` — marks up a stretch of a message
//! without saying what it looks like. The application maps each name to a
//! [`Style`]; the translation decides where the stretch goes:
//!
//! ```mf2
//! status = {#ok}Connected{/ok} to {#host}{$host}{/host}: {$sent :number} probes sent.
//! ```
//!
//! ```ignore
//! use mf2::ratatui::MarkupStyles;
//! use ratatui::style::{Color, Style};
//!
//! let styles = MarkupStyles::new()
//!     .with("ok", Style::new().fg(Color::Green).bold())
//!     .with("host", Style::new().underlined());
//! let status = mf2::ratatui::line(&i18n, &my_i18n::tr!("status", host = host, sent = n), &styles);
//! // "Connected" green and bold, " to ", the host underlined, ": 1,204 probes sent."
//! ```
//!
//! Nested elements combine their styles ([`Style::patch`]); a name with no
//! style changes nothing; standalone markup (`{#name/}`) writes nothing.
//! The types are `ratatui-core`'s, which `ratatui` re-exports, so the
//! result goes straight into a `Paragraph` or any other widget.
//!
//! See the user guide's [native applications page](https://evancarroll.github.io/rust-mf2/native-apps.html).
//!
//! Native only, as [`native`](crate::native), which the `ratatui` feature
//! implies: beside `hydrate` or `csr`, which build the browser's client,
//! `ratatui` is a compile error when compiling for the browser (`wasm32`).
//! On the host the two compile together, so a workspace that holds a
//! browser client and a terminal UI checks as one (`cargo check
//! --workspace`, and rust-analyzer's check).

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use ratatui_core::style::Style;
use ratatui_core::text::{Line, Span, Text};

use crate::native::NativeI18n;
use crate::{MarkupKind, Message, NoErrors, Part, PartSink};

/// A markup name the corpus uses, as the generated module's `markup::*`
/// names it: `markup::KEY` is `{#key}`. It holds the name and its hash, the
/// key a catalog's markup carries, so a misspelt name is a compile error
/// rather than a style that never applies.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Markup {
    key: u64,
    name: &'static str,
}

impl Markup {
    /// What the generated module writes: the name and its hash
    /// (`mf2::markup_key`).
    #[doc(hidden)]
    #[must_use]
    pub const fn new(key: u64, name: &'static str) -> Markup {
        Markup { key, name }
    }

    /// The markup name, as the messages write it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The name's hash: the key a catalog's markup carries.
    #[must_use]
    pub const fn key(self) -> u64 {
        self.key
    }
}

/// The style of each markup name.
#[derive(Clone, Debug, Default)]
pub struct MarkupStyles {
    entries: Vec<(String, Style)>,
}

impl MarkupStyles {
    /// No styles: markup changes nothing.
    #[must_use]
    pub const fn new() -> Self {
        MarkupStyles {
            entries: Vec::new(),
        }
    }

    /// Styles the markup element `name` with `style`, replacing an earlier
    /// style for the same name.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, style: Style) -> Self {
        let name = name.into();
        match self.entries.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = style,
            None => self.entries.push((name, style)),
        }
        self
    }

    /// The style of `name`, if it has one.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Style> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, style)| *style)
    }
}

/// Formats a message as owned Ratatui [`Text`] in the active locale: one
/// [`Line`] per line of the message, markup as styles.
#[must_use]
pub fn text(i18n: &NativeI18n, message: &impl Message, styles: &MarkupStyles) -> Text<'static> {
    let mut out = Styled::new(styles, true);
    if let Some(formatter) = i18n.formatter() {
        message.parts(&formatter, &mut out, &mut NoErrors);
    }
    Text::from(out.finish())
}

/// Formats a message as one owned Ratatui [`Line`] in the active locale,
/// markup as styles; a line break in the message becomes a space.
#[must_use]
pub fn line(i18n: &NativeI18n, message: &impl Message, styles: &MarkupStyles) -> Line<'static> {
    let mut out = Styled::new(styles, false);
    if let Some(formatter) = i18n.formatter() {
        message.parts(&formatter, &mut out, &mut NoErrors);
    }
    out.finish().into_iter().next().unwrap_or_default()
}

/// Builds lines of styled spans from a message's parts.
struct Styled<'s> {
    styles: &'s MarkupStyles,
    /// Whether `\n` starts a new line (else it is a space).
    split: bool,
    /// The open elements: each name and the style inside it.
    open: Vec<(String, Style)>,
    lines: Vec<Line<'static>>,
    spans: Vec<Span<'static>>,
    /// Text not yet in a span, and its style.
    run: String,
    run_style: Style,
}

impl<'s> Styled<'s> {
    fn new(styles: &'s MarkupStyles, split: bool) -> Self {
        Styled {
            styles,
            split,
            open: Vec::new(),
            lines: Vec::new(),
            spans: Vec::new(),
            run: String::new(),
            run_style: Style::new(),
        }
    }

    fn style(&self) -> Style {
        self.open
            .last()
            .map_or_else(Style::new, |(_, style)| *style)
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
            if self.split {
                self.end_line();
                self.run_style = style;
            } else {
                self.run.push(' ');
            }
            self.run.push_str(piece);
        }
    }

    fn end_span(&mut self) {
        if !self.run.is_empty() {
            let content = core::mem::take(&mut self.run);
            self.spans.push(Span::styled(content, self.run_style));
        }
    }

    fn end_line(&mut self) {
        self.end_span();
        self.lines
            .push(Line::from(core::mem::take(&mut self.spans)));
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.end_line();
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
                    // Back to the innermost open element of that name; a
                    // close with no open (the specification allows it) does
                    // nothing.
                    if let Some(at) = self.open.iter().rposition(|(n, _)| n == markup.name()) {
                        self.open.truncate(at);
                    }
                }
                MarkupKind::Standalone => {}
            },
            // Bidi isolation controls are dropped: Ratatui places every cell
            // itself, so a terminal never reorders what it draws, and the
            // controls would only be stray zero-width characters.
            _ => {}
        }
    }
}
