//! Format to parts (`plans/03-runtime.md` §2.2): the shape the suite's
//! `expParts` asserts, and what the Leptos layer builds elements from.

use mf2_model::{Dir, MarkupKind};

use crate::function::{FnContext, Function, OptionEntries};
use crate::sink::{Sink, SubPartSink};
use crate::text::write_escaped_literal;
use crate::unannotated;
use crate::value::Value;

/// Receives the parts of a formatted message, in order.
pub trait PartSink {
    /// One part.
    fn part(&mut self, part: Part<'_>);
}

/// A part of a formatted message. Concatenated, the parts are the string
/// output.
#[non_exhaustive]
pub enum Part<'p> {
    /// Literal text.
    Text(&'p str),
    /// A bidi isolation control (the Default Bidi Strategy).
    BidiIsolation(Isolation),
    /// A formatted placeholder.
    Expression(ExpressionPart<'p>),
    /// Markup (it writes no text).
    Markup(MarkupPart<'p>),
    /// A placeholder that resolved to a fallback value: `{source}`.
    Fallback(FallbackSource<'p>),
}

/// A bidi isolation control.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Isolation {
    /// U+2066 LEFT-TO-RIGHT ISOLATE.
    Lri,
    /// U+2067 RIGHT-TO-LEFT ISOLATE.
    Rli,
    /// U+2068 FIRST STRONG ISOLATE.
    Fsi,
    /// U+2069 POP DIRECTIONAL ISOLATE.
    Pdi,
}

impl Isolation {
    /// The control character, as text.
    pub const fn as_str(self) -> &'static str {
        match self {
            Isolation::Lri => "\u{2066}",
            Isolation::Rli => "\u{2067}",
            Isolation::Fsi => "\u{2068}",
            Isolation::Pdi => "\u{2069}",
        }
    }
}

/// A formatted placeholder: its resolved value and how it formats.
pub struct ExpressionPart<'p> {
    pub(crate) value: &'p Value<'p>,
    pub(crate) handler: Option<&'static dyn Function>,
    pub(crate) cx: FnContext<'p>,
    pub(crate) dir: Dir,
    pub(crate) id: Option<&'p str>,
}

impl<'p> ExpressionPart<'p> {
    /// The part's kind: the handler's `part_kind` (`"string"`, `"number"`,
    /// …); for an unannotated value `"string"` or `"number"`.
    pub fn kind(&self) -> &'p str {
        match self.handler {
            Some(h) => h.part_kind(),
            None => unannotated::kind(self.value),
        }
    }

    /// The catalog's locale.
    pub fn locale(&self) -> &'p str {
        self.cx.locale()
    }

    /// The value's direction after `u:dir`; `Auto` = unknown.
    pub fn dir(&self) -> Dir {
        self.dir
    }

    /// The `u:id` option.
    pub fn id(&self) -> Option<&'p str> {
        self.id
    }

    /// The resolved value.
    pub fn value(&self) -> &'p Value<'p> {
        self.value
    }

    /// Writes the formatted text (what string output shows).
    pub fn write(&self, out: &mut dyn Sink) {
        match self.handler {
            Some(h) => h.format(&self.cx, self.value, out),
            None => unannotated::format(self.value, self.cx.host(), out),
        }
    }

    /// The sub-parts of the formatted text (a number's `minusSign`,
    /// `integer`, `decimal`, `fraction`, …).
    pub fn sub_parts(&self, out: &mut dyn SubPartSink) {
        match self.handler {
            Some(h) => h.format_parts(&self.cx, self.value, out),
            None => unannotated::format_parts(self.value, self.cx.host(), out),
        }
    }
}

/// A markup placeholder.
pub struct MarkupPart<'p> {
    pub(crate) kind: MarkupKind,
    pub(crate) name: &'p str,
    pub(crate) id: Option<&'p str>,
    pub(crate) options: OptionEntries<'p, 'p>,
}

impl<'p> MarkupPart<'p> {
    /// Open, standalone or close.
    pub fn kind(&self) -> MarkupKind {
        self.kind
    }

    /// The markup name (NFC).
    pub fn name(&self) -> &'p str {
        self.name
    }

    /// The `u:id` option.
    pub fn id(&self) -> Option<&'p str> {
        self.id
    }

    /// The resolved options, `u:` options removed.
    pub fn options(&self) -> MarkupOptions<'p> {
        MarkupOptions {
            entries: self.options,
            at: 0,
        }
    }
}

/// The options of a [`MarkupPart`]: `(name, value)` in source order.
pub struct MarkupOptions<'p> {
    entries: OptionEntries<'p, 'p>,
    at: usize,
}

impl<'p> Iterator for MarkupOptions<'p> {
    type Item = (&'p str, &'p Value<'p>);

    fn next(&mut self) -> Option<Self::Item> {
        let e = self.entries.get(self.at)?;
        self.at += 1;
        Some((e.0, e.1.value))
    }
}

/// What a fallback value shows (formatting.md, "Fallback Resolution").
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum FallbackSource<'p> {
    /// `$name`.
    Variable(&'p str),
    /// `|value|`, with `\` and `|` escaped.
    Literal(&'p str),
    /// `:ns:name`.
    Function(&'p str),
    /// `�` (a malformed record, or a name that could not be read).
    Unknown,
}

impl FallbackSource<'_> {
    /// Writes the representation, without the braces.
    pub fn write(&self, out: &mut dyn Sink) {
        match *self {
            FallbackSource::Variable(n) => {
                out.push_str("$");
                out.push_str(n);
            }
            FallbackSource::Literal(v) => {
                out.push_str("|");
                write_escaped_literal(v, out);
                out.push_str("|");
            }
            FallbackSource::Function(n) => {
                out.push_str(":");
                out.push_str(n);
            }
            FallbackSource::Unknown => out.push_str("\u{FFFD}"),
        }
    }
}
