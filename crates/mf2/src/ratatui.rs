//! Ratatui text from a native application's messages, their markup as
//! styles.
//!
//! A description — what `tr!` builds — converts to Ratatui's text in the
//! language in force: the native store's ([`native`](crate::native)), this
//! thread's language, else the app-wide one.
//!
//! ```ignore
//! use mf2::ratatui::{Theme, set_theme};
//! use my_i18n::{markup, tr};
//! use ratatui::style::{Color, Style, Stylize};
//! use ratatui::text::Line;
//! use ratatui::widgets::{Block, Paragraph};
//!
//! set_theme(Theme::default().style(markup::HOST, Style::new().fg(Color::Cyan)));
//! let status: Line = tr!("status", host = host, sent = n).into();
//! let help = Paragraph::new(tr!("help")).block(Block::bordered().title(tr!("title")));
//! let quit = tr!("quit").bold(); // a `Line`: the message's own styles over bold
//! frame.render_widget(tr!("welcome"), area);
//! ```
//!
//! # Conversions
//!
//! Each description — [`Tr`], [`TrArgs`], [`TrRich`], [`TrDyn`], or a
//! reference to one — converts into a [`Span`], a [`Line`] or a [`Text`],
//! so Ratatui's own conversions take it wherever they take text: a
//! `Paragraph`, a `Block`'s title, a `Cell`, a `Row`, a `ListItem`, a
//! `List`, `Tabs`. Each is also [`Styled`] as a `Line`, so Ratatui's
//! `Stylize` works — `tr!("quit").bold()` keeps the message's own markup
//! styles and patches the call's style under them — and a [`Widget`], drawn
//! as its `Text`.
//!
//! # Markup
//!
//! MF2 markup — `{#name}…{/name}` — marks up a stretch of a message without
//! saying what it looks like. A [`Theme`] maps each name to a [`Style`]:
//! nested elements patch their styles in order, a name the theme lacks
//! keeps the style around it, and standalone markup (`{#name/}`) writes
//! nothing. [`set_theme`] sets the app-wide theme and [`with_theme`] this
//! thread's for a scope; until then it is [`Theme::default`].
//!
//! # Line breaks
//!
//! A `Text` starts a new line at each line break in the message; a `Line`
//! and a `Span` join the lines with a space. **A `Span` flattens markup**:
//! a span has one style, so it keeps none of the message's.
//!
//! That has a trap. Ratatui collects anything that converts into a `Span`
//! into a `Line`, so `[tr!("a"), tr!("b")].into_iter().collect::<Line>()`
//! compiles, and loses the messages' markup styles. To keep them, write one
//! message for the styled line — the translation then places the pieces —
//! or extend a `Line` with each message's `Line::from(…).spans`.
//!
//! # What a conversion allocates
//!
//! The catalog's text is borrowed from the executable, never copied: a
//! constant message's `Span` allocates nothing, and its `Line` only the
//! `Line`'s `Vec` (a `Text`, its `Vec` of lines too). Each placeholder is
//! one `String`. Open markup elements are kept inline, by the name's hash,
//! so markup names never allocate.
//!
//! # Panics
//!
//! A conversion panics before [`install`](crate::native::install) (outside
//! [`with_locale`](crate::native::with_locale)), naming `install()`.
//!
//! The types are `ratatui-core`'s, which `ratatui` re-exports. 1.x's
//! [`line`], [`text`] and [`MarkupStyles`], which take a [`NativeI18n`] and
//! a map of styles on each call, are kept under their 1.x names.
//!
//! See the user guide's [native applications page](https://evancarroll.github.io/rust-mf2/native-apps.html).
//!
//! Native only, as [`native`](crate::native), which the `ratatui` feature
//! implies: beside `hydrate` or `csr`, which build the browser's client,
//! `ratatui` is a compile error when compiling for the browser (`wasm32`).
//! On the host the two compile together, so a workspace that holds a
//! browser client and a terminal UI checks as one (`cargo check
//! --workspace`, and rust-analyzer's check).

use alloc::borrow::{Cow, ToOwned};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::Cell;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, PoisonError, RwLock};

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Modifier, Style, Styled};
use ratatui_core::text::{Line, Span, Text};
use ratatui_core::widgets::Widget;

use crate::native::NativeI18n;
use crate::native::store;
use crate::{
    Catalog, MarkupKind, Message, NoErrors, Part, PartSink, StrRef, Tr, TrArgs, TrDyn, TrRich,
    markup_key,
};

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
// ------------------------------------------------------------------ the theme

/// What markup looks like: a [`Style`] per markup name, found by the name's
/// hash.
///
/// ```ignore
/// use mf2::ratatui::{Theme, set_theme};
/// use ratatui::style::{Color, Style};
///
/// set_theme(
///     Theme::default()
///         .style(my_i18n::markup::OK, Style::new().fg(Color::Green))
///         .style(my_i18n::markup::HOST, Style::new().underlined()),
/// );
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Theme {
    entries: Vec<(Markup, Style)>,
}

impl Theme {
    /// No styles: markup changes nothing.
    #[must_use]
    pub const fn empty() -> Theme {
        Theme {
            entries: Vec::new(),
        }
    }

    /// Styles `markup` with `style`, replacing an earlier style for it.
    #[must_use]
    pub fn style(mut self, markup: Markup, style: Style) -> Theme {
        match self.entries.iter_mut().find(|(m, _)| m.key == markup.key) {
            Some(entry) => *entry = (markup, style),
            None => self.entries.push((markup, style)),
        }
        self
    }

    fn get(&self, key: u64) -> Option<Style> {
        self.entries
            .iter()
            .find(|(m, _)| m.key == key)
            .map(|(_, style)| *style)
    }
}

impl Default for Theme {
    /// The common names: `b` and `strong` bold, `i` and `em` italic, `u`
    /// underlined, `s` and `del` crossed out, `code` and `kbd` reversed.
    fn default() -> Theme {
        let named = |name: &'static str| Markup::new(markup_key(name), name);
        let bold = Style::new().add_modifier(Modifier::BOLD);
        let italic = Style::new().add_modifier(Modifier::ITALIC);
        let crossed = Style::new().add_modifier(Modifier::CROSSED_OUT);
        let reversed = Style::new().add_modifier(Modifier::REVERSED);
        Theme::empty()
            .style(named("b"), bold)
            .style(named("strong"), bold)
            .style(named("i"), italic)
            .style(named("em"), italic)
            .style(named("u"), Style::new().add_modifier(Modifier::UNDERLINED))
            .style(named("s"), crossed)
            .style(named("del"), crossed)
            .style(named("code"), reversed)
            .style(named("kbd"), reversed)
    }
}

/// The app-wide theme. Like the store's other settings, it sits behind an
/// `RwLock` with a generation counter, and each thread keeps a copy.
static THEME: LazyLock<RwLock<Arc<Theme>>> =
    LazyLock::new(|| RwLock::new(Arc::new(Theme::default())));
/// Bumped, under the write lock, by every `set_theme`.
static GENERATION: AtomicU64 = AtomicU64::new(1);

std::thread_local! {
    /// This thread's copy of the app-wide theme, and the generation it was
    /// taken at.
    static LOCAL: Cell<(u64, Option<Arc<Theme>>)> = const { Cell::new((0, None)) };
    /// This thread's theme for a scope (`with_theme`).
    static SCOPED: Cell<Option<Arc<Theme>>> = const { Cell::new(None) };
}

/// Sets the app-wide theme: every thread's next conversion uses it.
pub fn set_theme(theme: Theme) {
    let theme = Arc::new(theme);
    let mut slot = THEME.write().unwrap_or_else(PoisonError::into_inner);
    *slot = theme;
    GENERATION.fetch_add(1, Ordering::Release);
}

/// Runs `body` with `theme` as this thread's theme, then restores the one
/// before, also when `body` unwinds.
pub fn with_theme<R>(theme: &Theme, body: impl FnOnce() -> R) -> R {
    let outer = SCOPED.with(|scoped| scoped.replace(Some(Arc::new(theme.clone()))));
    let _restore = Restore(outer);
    body()
}

/// The theme in force on this thread: its [`with_theme`] scope's, else the
/// app-wide one.
#[must_use]
pub fn theme() -> Theme {
    Theme::clone(&in_force())
}

/// Puts back the theme around a [`with_theme`] scope.
struct Restore(Option<Arc<Theme>>);

impl Drop for Restore {
    fn drop(&mut self) {
        let outer = self.0.take();
        let _ = SCOPED.try_with(|scoped| scoped.set(outer));
    }
}

/// The theme in force: the scope's, else this thread's copy of the app-wide
/// one, taken again when the generation has moved.
fn in_force() -> Arc<Theme> {
    let scoped = SCOPED.with(|scoped| {
        let theme = scoped.take();
        let copy = theme.clone();
        scoped.set(theme);
        copy
    });
    if let Some(theme) = scoped {
        return theme;
    }
    let generation = GENERATION.load(Ordering::Acquire);
    LOCAL.with(|local| {
        let (taken, copy) = local.take();
        let theme = match copy {
            Some(theme) if taken == generation => theme,
            _ => Arc::clone(&THEME.read().unwrap_or_else(PoisonError::into_inner)),
        };
        local.set((generation, Some(Arc::clone(&theme))));
        theme
    })
}

// ------------------------------------------------------------ the conversions

macro_rules! conversions {
    ($($ty:ty),*) => {$(
        /// The message as one span: its lines joined with a space, its
        /// markup flattened (a span has one style).
        impl From<$ty> for Span<'static> {
            fn from(description: $ty) -> Span<'static> {
                span_of(&description)
            }
        }

        /// The message as one span, as by value.
        impl From<&$ty> for Span<'static> {
            fn from(description: &$ty) -> Span<'static> {
                span_of(description)
            }
        }

        /// The message as one line, its markup as the theme's styles, its
        /// lines joined with a space.
        impl From<$ty> for Line<'static> {
            fn from(description: $ty) -> Line<'static> {
                line_of(&description)
            }
        }

        /// The message as one line, as by value.
        impl From<&$ty> for Line<'static> {
            fn from(description: &$ty) -> Line<'static> {
                line_of(description)
            }
        }

        /// The message as text, its markup as the theme's styles: a new
        /// line at each line break.
        impl From<$ty> for Text<'static> {
            fn from(description: $ty) -> Text<'static> {
                text_of(&description)
            }
        }

        /// The message as text, as by value.
        impl From<&$ty> for Text<'static> {
            fn from(description: &$ty) -> Text<'static> {
                text_of(description)
            }
        }

        /// Styled as its [`Line`], so Ratatui's `Stylize` works: the line
        /// keeps the message's own markup styles, over the call's.
        impl Styled for $ty {
            type Item = Line<'static>;

            fn style(&self) -> Style {
                Style::new()
            }

            fn set_style<S: Into<Style>>(self, style: S) -> Line<'static> {
                line_of(&self).style(style)
            }
        }

        /// Drawn as its [`Text`].
        impl Widget for $ty {
            fn render(self, area: Rect, buf: &mut Buffer) {
                text_of(&self).render(area, buf);
            }
        }
    )*};
}

conversions!(Tr, TrArgs, TrRich, TrDyn);

/// The message as one span.
fn span_of(m: &dyn Message) -> Span<'static> {
    let content = store::with_formatter(|catalog, f| {
        store::with_scratch(|buf| {
            let mut flat = Flat {
                catalog,
                buf,
                only: None,
            };
            m.parts(f, &mut flat, &mut NoErrors);
            flat.finish()
        })
    });
    match content {
        Some(content) => Span::raw(content),
        None => store::not_installed(),
    }
}

/// The message as one line.
fn line_of(m: &dyn Message) -> Line<'static> {
    Line::from(lines_of(m, false).1)
}

/// The message as text.
fn text_of(m: &dyn Message) -> Text<'static> {
    let (mut lines, last) = lines_of(m, true);
    lines.push(Line::from(last));
    Text::from(lines)
}

/// The message's finished lines and the spans of its last line; `split`:
/// a line break starts a new line, else it is a space.
fn lines_of(m: &dyn Message, split: bool) -> (Vec<Line<'static>>, Vec<Span<'static>>) {
    let out = store::with_formatter(|catalog, f| {
        let mut sink = Draft {
            catalog,
            theme: None,
            split,
            open: [(0, Style::new()); DEPTH],
            depth: 0,
            spans: Vec::new(),
            lines: Vec::new(),
        };
        m.parts(f, &mut sink, &mut NoErrors);
        (sink.lines, sink.spans)
    });
    match out {
        Some(out) => out,
        None => store::not_installed(),
    }
}

/// Each line break in `text` as a space, in place.
fn spaces(text: &mut String) {
    while let Some(at) = text.find('\n') {
        text.replace_range(at..=at, " ");
    }
}

/// A placeholder's text as its own `String`: written into the reused buffer
/// and copied out once, so it is one allocation whatever its length.
fn placeholder(write: impl FnOnce(&mut String)) -> String {
    store::with_scratch(|buf| {
        write(buf);
        String::from(buf.as_str())
    })
}

/// Open markup elements kept inline; deeper ones keep the style around them.
const DEPTH: usize = 8;

/// Builds a message's spans: the store's catalog text borrowed, a
/// placeholder one `String`, markup as the theme's styles.
struct Draft {
    catalog: &'static Catalog,
    /// The theme in force, read at the first markup element.
    theme: Option<Arc<Theme>>,
    /// Whether a line break starts a new line (`Text`), else a space.
    split: bool,
    /// The open elements: each name's hash and the style inside it.
    open: [(u64, Style); DEPTH],
    depth: usize,
    spans: Vec<Span<'static>>,
    lines: Vec<Line<'static>>,
}

impl Draft {
    fn style(&self) -> Style {
        self.depth
            .min(DEPTH)
            .checked_sub(1)
            .and_then(|at| self.open.get(at))
            .map_or_else(Style::new, |(_, style)| *style)
    }

    fn line_break(&mut self, style: Style) {
        if self.split {
            let spans = core::mem::take(&mut self.spans);
            self.lines.push(Line::from(spans));
        } else {
            self.spans.push(Span::styled(" ", style));
        }
    }

    /// The catalog's text, borrowed.
    fn borrowed(&mut self, text: &'static str) {
        let style = self.style();
        for (at, piece) in text.split('\n').enumerate() {
            if at > 0 {
                self.line_break(style);
            }
            if !piece.is_empty() {
                self.spans.push(Span::styled(piece, style));
            }
        }
    }

    /// A placeholder's text, or text that is not the store's.
    fn owned(&mut self, mut text: String) {
        let style = self.style();
        if !text.contains('\n') {
            if !text.is_empty() {
                self.spans.push(Span::styled(text, style));
            }
        } else if !self.split {
            spaces(&mut text);
            self.spans.push(Span::styled(text, style));
        } else {
            for (at, piece) in text.split('\n').enumerate() {
                if at > 0 {
                    self.line_break(style);
                }
                if !piece.is_empty() {
                    self.spans.push(Span::styled(piece.to_owned(), style));
                }
            }
        }
    }

    fn open(&mut self, key: u64) {
        let outer = self.style();
        let own = self.theme.get_or_insert_with(in_force).get(key);
        if let Some(slot) = self.open.get_mut(self.depth) {
            *slot = (key, own.map_or(outer, |own| outer.patch(own)));
        }
        self.depth = self.depth.saturating_add(1);
    }

    fn close(&mut self, key: u64) {
        if self.depth > DEPTH {
            // The innermost element, deeper than those kept.
            self.depth -= 1;
        } else if let Some(at) = self
            .open
            .get(..self.depth)
            .and_then(|open| open.iter().rposition(|(k, _)| *k == key))
        {
            // Back to the innermost open element of that name; a close with
            // no open (the specification allows it) does nothing.
            self.depth = at;
        }
    }
}

impl PartSink for Draft {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(text) => self.owned(text.to_owned()),
            Part::Expression(expression) => self.owned(placeholder(|buf| expression.write(buf))),
            Part::Fallback(source) => self.owned(placeholder(|buf| {
                buf.push('{');
                source.write(buf);
                buf.push('}');
            })),
            Part::Markup(markup) => match markup.kind() {
                MarkupKind::Open => self.open(markup_key(markup.name())),
                MarkupKind::Close => self.close(markup_key(markup.name())),
                MarkupKind::Standalone => {}
            },
            // Bidi isolation controls are dropped: Ratatui places every cell
            // itself, so a terminal never reorders what it draws, and the
            // controls would only be stray zero-width characters.
            _ => {}
        }
    }

    fn part_catalog_text(&mut self, catalog: &Catalog, r: StrRef) -> bool {
        let own: &'static Catalog = self.catalog;
        if core::ptr::eq(catalog, own) {
            let Some(text) = own.text(r) else {
                return false;
            };
            self.borrowed(text);
        } else {
            let Some(text) = catalog.text(r) else {
                return false;
            };
            self.owned(text.to_owned());
        }
        true
    }
}

/// Builds a message's one span in the reused buffer, or borrows it when it
/// is one catalog string with no line break.
struct Flat<'b> {
    catalog: &'static Catalog,
    buf: &'b mut String,
    /// The text so far, when it is one catalog string (`buf` is then empty).
    only: Option<&'static str>,
}

impl Flat<'_> {
    /// Moves a borrowed string into the buffer, before more text follows.
    fn spill(&mut self) {
        if let Some(only) = self.only.take() {
            self.buf.push_str(only);
        }
    }

    fn borrowed(&mut self, text: &'static str) {
        if text.is_empty() {
            return;
        }
        if self.only.is_none() && self.buf.is_empty() && !text.contains('\n') {
            self.only = Some(text);
        } else {
            self.spill();
            self.buf.push_str(text);
        }
    }

    fn finish(self) -> Cow<'static, str> {
        match self.only {
            Some(only) => Cow::Borrowed(only),
            None if self.buf.is_empty() => Cow::Borrowed(""),
            None => {
                spaces(self.buf);
                Cow::Owned(String::from(self.buf.as_str()))
            }
        }
    }
}

impl PartSink for Flat<'_> {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(text) => {
                self.spill();
                self.buf.push_str(text);
            }
            Part::Expression(expression) => {
                self.spill();
                expression.write(&mut *self.buf);
            }
            Part::Fallback(source) => {
                self.spill();
                self.buf.push('{');
                source.write(&mut *self.buf);
                self.buf.push('}');
            }
            // A span has one style, so markup writes nothing here; bidi
            // isolation controls are dropped, as in a line.
            _ => {}
        }
    }

    fn part_catalog_text(&mut self, catalog: &Catalog, r: StrRef) -> bool {
        let own: &'static Catalog = self.catalog;
        if core::ptr::eq(catalog, own) {
            let Some(text) = own.text(r) else {
                return false;
            };
            self.borrowed(text);
        } else {
            let Some(text) = catalog.text(r) else {
                return false;
            };
            self.part(Part::Text(text));
        }
        true
    }
}

// ------------------------------------------------------------- 1.x's names

/// The style of each markup name, by name: 1.x's map, which [`line`] and
/// [`text`] take on each call. [`Theme`] is 2.0's.
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
    let mut out = ByName::new(styles, true);
    if let Some(formatter) = i18n.formatter() {
        message.parts(&formatter, &mut out, &mut NoErrors);
    }
    Text::from(out.finish())
}

/// Formats a message as one owned Ratatui [`Line`] in the active locale,
/// markup as styles; a line break in the message becomes a space.
#[must_use]
pub fn line(i18n: &NativeI18n, message: &impl Message, styles: &MarkupStyles) -> Line<'static> {
    let mut out = ByName::new(styles, false);
    if let Some(formatter) = i18n.formatter() {
        message.parts(&formatter, &mut out, &mut NoErrors);
    }
    out.finish().into_iter().next().unwrap_or_default()
}

/// Builds lines of styled spans from a message's parts, styled by name.
struct ByName<'s> {
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

impl<'s> ByName<'s> {
    fn new(styles: &'s MarkupStyles, split: bool) -> Self {
        ByName {
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

impl PartSink for ByName<'_> {
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
