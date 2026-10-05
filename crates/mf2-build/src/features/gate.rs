//! What a built-in function asks of a build before a message may call it.
//!
//! A message never adds formatting code by itself: a function formats only
//! where every side the build formats on has named a formatter that can.

use super::backend::{Backend, NumberBackend};

/// What a built-in function needs of the build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Nothing: `:string`.
    None,
    /// A number formatter on every side the build formats on: `:number`,
    /// `:integer` and `:offset`, and so plural selection.
    Number,
    /// A number formatter that writes the language's own form — `builtin`,
    /// or the browser's `intl` — on every side: `:percent`, `:currency` and
    /// `:unit`, which plain digits cannot show.
    LocalizedNumber,
    /// A date formatter on every side: `:datetime`, `:date` and `:time`.
    Date,
}

/// A built-in function and what it needs of the build.
///
/// A function used where the build does not have what it needs is a **build
/// error** naming the file and the line (`gated-function`), with the
/// features to write.
pub const BUILTINS: [(&str, Gate); 10] = [
    ("string", Gate::None),
    ("number", Gate::Number),
    ("integer", Gate::Number),
    ("offset", Gate::Number),
    ("percent", Gate::LocalizedNumber),
    ("currency", Gate::LocalizedNumber),
    ("unit", Gate::LocalizedNumber),
    ("datetime", Gate::Date),
    ("date", Gate::Date),
    ("time", Gate::Date),
];

/// What a function asks of its domain's formatter, and how the refusal says
/// it.
#[derive(Clone, Copy)]
pub(crate) struct Ask<B: Backend> {
    /// What the function formats, in a message: `a date`.
    pub(crate) thing: &'static str,
    /// What a build that cannot format it is without: `date formatter`.
    pub(crate) lacks: &'static str,
    /// Whether `backend` can.
    pub(crate) able: fn(B) -> bool,
}

impl<B: Backend> Ask<B> {
    /// Any formatter of the domain.
    pub(crate) const ANY: Ask<B> = Ask {
        thing: B::THING,
        lacks: B::NOUN,
        able: |_| true,
    };
}

impl Ask<NumberBackend> {
    /// A number formatter that writes the language's own form.
    pub(crate) const LOCALIZED: Ask<NumberBackend> = Ask {
        thing: "a number in the language's own form",
        lacks: "number formatter that writes one",
        able: NumberBackend::localizes,
    };
}
