//! What the parser reports beside its best-effort result.

use mf2_model::Span;

use crate::code;

/// One resource syntax error.
///
/// The parser keeps going after every one of them, so a `check` run reports
/// all of a file's errors at once.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Diagnostic {
    /// The stable detail code: see [`crate::code`].
    pub code: u16,
    /// Where, in the file.
    pub span: Span,
}

impl Diagnostic {
    /// A diagnostic at `start..end`.
    pub fn new(code: u16, start: u32, end: u32) -> Self {
        Diagnostic {
            code,
            span: Span { start, end },
        }
    }

    /// What went wrong, as one sentence without a trailing full stop.
    pub fn message(&self) -> &'static str {
        code::message(self.code)
    }
}
