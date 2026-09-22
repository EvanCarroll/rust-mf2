//! Where formatted text and errors go: minimal traits, not `core::fmt::Write`
//! (B12). A client supplies its own sink (a DOM or SSR buffer adapter); the
//! implementations for `String` and `Vec` here grow with `try_reserve` and
//! drop what does not fit rather than abort.

use alloc::string::String;
use alloc::vec::Vec;

use mf2_catalog::{Catalog, StrRef};

use crate::error::FormatError;

/// A text sink.
pub trait Sink {
    /// Appends `s`.
    fn push_str(&mut self, s: &str);

    /// Appends the catalog string `r` — the seam for catalog text as JS
    /// strings (`plans/stretch_goals_after_v1/prob_builtin_strings.md` §7):
    /// the evaluator writes catalog text only through this method. Returns
    /// `false` (and writes nothing) when the string is not valid (F4).
    fn push_catalog_text(&mut self, catalog: &Catalog, r: StrRef) -> bool {
        match catalog.text(r) {
            Some(s) => {
                self.push_str(s);
                true
            }
            None => false,
        }
    }
}

impl Sink for String {
    fn push_str(&mut self, s: &str) {
        if self.try_reserve(s.len()).is_ok() {
            String::push_str(self, s);
        }
    }
}

/// An error sink.
pub trait ErrorSink {
    /// Reports `e`.
    fn error(&mut self, e: FormatError);
}

impl ErrorSink for Vec<FormatError> {
    fn error(&mut self, e: FormatError) {
        if self.try_reserve(1).is_ok() {
            self.push(e);
        }
    }
}

/// Discards errors: the release client's policy (`plans/03-runtime.md` §8).
#[derive(Clone, Copy, Default, Debug)]
pub struct NoErrors;

impl ErrorSink for NoErrors {
    #[inline]
    fn error(&mut self, _: FormatError) {}
}

/// Receives the sub-parts of a formatted expression (a number's
/// `minusSign`, `integer`, `decimal`, `fraction`, …).
pub trait SubPartSink {
    /// One sub-part: its kind and its text.
    fn sub_part(&mut self, kind: &str, text: &str);
}
