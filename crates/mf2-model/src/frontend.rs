//! The parser boundary: what `mf2-syntax` implements, and what an
//! `ox_mf2_parser` adapter would implement if the D1 gate were not met.

use crate::diagnostic::Diagnostics;
use crate::message::Message;

/// The result of parsing one message.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Parsed<'src> {
    /// `Some` whenever the source has no syntax error — also when it has
    /// data-model errors, which are then in `diagnostics`.
    pub message: Option<Message<'src>>,
    /// Every syntax error the frontend could recover to (with spans); when
    /// there is none, every data-model error.
    pub diagnostics: Diagnostics,
}

/// A parser from MF2 source to the data model.
pub trait Frontend {
    /// `&mut self` lets an implementation keep scratch state across calls (the
    /// gate's "reused" rows); the returned model borrows only the source.
    fn parse<'src>(&mut self, source: &'src str) -> Parsed<'src>;
}
