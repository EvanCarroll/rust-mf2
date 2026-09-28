//! A Ratatui `Line` from a message, zero-copy: catalog text is borrowed as
//! `&'static str`, markup is looked up in the app-wide theme by the hash of
//! its name, and only placeholders allocate.

use mf2::{Catalog, MarkupKind, Part, PartSink, markup_key};
use ratatui_core::style::{Modifier, Style};
use ratatui_core::text::{Line, Span};

use crate::{Msg, PartsOf, context, line_parts};

/// How a text part becomes a `&'static str`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    /// Not recovered: every text part is copied (as 1.x does).
    Copy,
    /// R1: a pointer-range check against the catalog's `&'static` bytes,
    /// then `from_utf8` on the re-slice — O(length) per part.
    RangeUtf8,
    /// R2: a pointer-range check against the catalog's string pool,
    /// validated once at `install` as `&'static str`; `str::get`
    /// re-slices in O(1). No change to the runtime.
    RangePool,
    /// S: the runtime's hidden seam (`PartSink::part_catalog_text`) hands
    /// over the string reference, resolved against the `&'static` catalog
    /// after an identity check (feature `seam`).
    Seam,
}

/// The app-wide theme: a style per markup name, keyed by the name's hash.
#[derive(Clone, Debug, Default)]
pub struct Theme {
    entries: Vec<(u64, Style)>,
}

impl Theme {
    /// No styles.
    #[must_use]
    pub const fn empty() -> Theme {
        Theme {
            entries: Vec::new(),
        }
    }

    /// The defaults for common names (D18): `b` / `strong` bold, `i` / `em`
    /// italic, `u` underlined, `s` / `del` crossed out, `code` / `kbd`
    /// reversed.
    #[must_use]
    pub fn with_defaults() -> Theme {
        let bold = Style::new().add_modifier(Modifier::BOLD);
        let italic = Style::new().add_modifier(Modifier::ITALIC);
        let crossed = Style::new().add_modifier(Modifier::CROSSED_OUT);
        let reversed = Style::new().add_modifier(Modifier::REVERSED);
        Theme::empty()
            .style("b", bold)
            .style("strong", bold)
            .style("i", italic)
            .style("em", italic)
            .style("u", Style::new().add_modifier(Modifier::UNDERLINED))
            .style("s", crossed)
            .style("del", crossed)
            .style("code", reversed)
            .style("kbd", reversed)
    }

    /// Styles markup `name`, replacing an earlier style for it.
    #[must_use]
    pub fn style(mut self, name: &str, style: Style) -> Theme {
        let key = markup_key(name);
        match self.entries.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = style,
            None => self.entries.push((key, style)),
        }
        self
    }

    #[inline]
    fn get(&self, key: u64) -> Option<Style> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, s)| *s)
    }
}

fn default_theme() -> &'static Theme {
    static DEFAULT: std::sync::OnceLock<Theme> = std::sync::OnceLock::new();
    DEFAULT.get_or_init(Theme::with_defaults)
}

/// The message as one `Line` in the current locale, styled by the app-wide
/// theme; a line break becomes a space.
#[must_use]
#[inline]
pub fn line<const LOCK: bool>(m: &impl Msg, method: Method) -> Line<'static> {
    line_dyn::<LOCK>(m, method)
}

fn line_dyn<const LOCK: bool>(m: &dyn PartsOf, method: Method) -> Line<'static> {
    let (s, catalog, pool, set) = line_parts::<LOCK>();
    let theme = set.theme.unwrap_or_else(default_theme);
    let cx = context(&set);
    let f = mf2::Formatter::new(catalog, s.corpus.registry(), &cx);
    let mut sink = ZeroSink {
        theme,
        method,
        catalog,
        bytes: catalog.as_bytes(),
        pool,
        open: [(0, Style::new()); DEPTH],
        depth: 0,
        spans: Vec::new(),
    };
    m.parts_to(&f, &mut sink);
    Line::from(sink.spans)
}

/// Open elements kept inline; deeper ones keep the innermost kept style.
const DEPTH: usize = 8;

struct ZeroSink<'t> {
    theme: &'t Theme,
    method: Method,
    #[cfg_attr(not(feature = "seam"), allow(dead_code))]
    catalog: &'static Catalog,
    bytes: &'static [u8],
    pool: Option<&'static str>,
    open: [(u64, Style); DEPTH],
    depth: usize,
    spans: Vec<Span<'static>>,
}

impl ZeroSink<'_> {
    #[inline]
    fn style(&self) -> Style {
        match self.depth.min(DEPTH).checked_sub(1) {
            Some(i) => self.open.get(i).map_or_else(Style::new, |(_, s)| *s),
            None => Style::new(),
        }
    }

    /// `s` as the `&'static str` it is a part of, when the method can prove
    /// it.
    #[inline]
    fn recover(&self, s: &str) -> Option<&'static str> {
        match self.method {
            Method::RangePool => {
                let pool = self.pool?;
                let off = (s.as_ptr() as usize).checked_sub(pool.as_ptr() as usize)?;
                pool.get(off..off.checked_add(s.len())?)
            }
            Method::RangeUtf8 => {
                let off = (s.as_ptr() as usize).checked_sub(self.bytes.as_ptr() as usize)?;
                std::str::from_utf8(self.bytes.get(off..off.checked_add(s.len())?)?).ok()
            }
            Method::Copy | Method::Seam => None,
        }
    }

    fn text_static(&mut self, s: &'static str) {
        let style = self.style();
        let mut pieces = s.split('\n');
        if let Some(first) = pieces.next()
            && !first.is_empty()
        {
            self.spans.push(Span::styled(first, style));
        }
        for piece in pieces {
            self.spans.push(Span::styled(" ", style));
            if !piece.is_empty() {
                self.spans.push(Span::styled(piece, style));
            }
        }
    }

    fn text_owned(&mut self, s: &str) {
        self.owned(s.to_owned());
    }

    /// A placeholder's text: its own `String`, moved into the span.
    fn owned(&mut self, mut text: String) {
        if text.contains('\n') {
            text = text.replace('\n', " ");
        }
        if !text.is_empty() {
            let style = self.style();
            self.spans.push(Span::styled(text, style));
        }
    }

    fn open(&mut self, key: u64) {
        let outer = self.style();
        let style = self.theme.get(key).map_or(outer, |own| outer.patch(own));
        if let Some(slot) = self.open.get_mut(self.depth) {
            *slot = (key, style);
        }
        self.depth += 1;
    }

    fn close(&mut self, key: u64) {
        // Back to the innermost open element of that name; a close with no
        // open does nothing (as 1.x).
        let kept = self.depth.min(DEPTH);
        if let Some(at) = self
            .open
            .get(..kept)
            .and_then(|open| open.iter().rposition(|(k, _)| *k == key))
        {
            self.depth = at;
        } else if self.depth > DEPTH {
            self.depth -= 1;
        }
    }
}

impl PartSink for ZeroSink<'_> {
    fn part(&mut self, part: Part<'_>) {
        match part {
            Part::Text(s) => match self.recover(s) {
                Some(text) => self.text_static(text),
                None => self.text_owned(s),
            },
            Part::Expression(expression) => {
                let mut text = String::new();
                expression.write(&mut text);
                self.owned(text);
            }
            Part::Fallback(source) => {
                let mut text = String::from("{");
                source.write(&mut text);
                text.push('}');
                self.owned(text);
            }
            Part::Markup(markup) => match markup.kind() {
                MarkupKind::Open => self.open(markup_key(markup.name())),
                MarkupKind::Close => self.close(markup_key(markup.name())),
                MarkupKind::Standalone => {}
            },
            // Bidi isolation controls are dropped, as 1.x does.
            _ => {}
        }
    }

    #[cfg(feature = "seam")]
    fn part_catalog_text(&mut self, catalog: &Catalog, r: mf2::StrRef) -> bool {
        if self.method == Method::Seam && std::ptr::eq(catalog, self.catalog) {
            return match self.catalog.text(r) {
                Some(s) => {
                    if !s.is_empty() {
                        self.text_static(s);
                    }
                    true
                }
                None => false,
            };
        }
        match catalog.text(r) {
            Some(s) => {
                if !s.is_empty() {
                    self.part(Part::Text(s));
                }
                true
            }
            None => false,
        }
    }
}
