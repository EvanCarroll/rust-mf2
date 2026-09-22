//! Byte offsets to lines and columns, for the messages `mf2 check` prints.

use alloc::vec::Vec;

/// Where every line of a file starts, so that a [`Span`](mf2_model::Span) can
/// name a line and a column.
///
/// Built once per file and shared by the resource's own diagnostics and the
/// message ones a value's [`ValueMap`](crate::ValueMap) maps back into it.
#[derive(Clone, Debug)]
pub struct LineIndex {
    starts: Vec<u32>,
    len: u32,
}

/// A one-based position in a file.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Position {
    /// The line, counting from 1.
    pub line: u32,
    /// The column in characters, counting from 1 — what an editor shows.
    pub column: u32,
}

impl LineIndex {
    /// Indexes `src`.
    pub fn new(src: &str) -> Self {
        let mut starts = alloc::vec![0u32];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                starts.push(u32::try_from(i + 1).unwrap_or(u32::MAX));
            }
        }
        LineIndex {
            starts,
            len: u32::try_from(src.len()).unwrap_or(u32::MAX),
        }
    }

    /// The line and column of byte `offset`, clamped to the file.
    ///
    /// `src` must be the string the index was built from.
    pub fn position(&self, src: &str, offset: u32) -> Position {
        let offset = offset.min(self.len);
        let line = match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let start = self.starts.get(line).copied().unwrap_or(0) as usize;
        let column = src
            .get(start..offset as usize)
            .map_or(0, |s| s.chars().count());
        Position {
            line: u32::try_from(line + 1).unwrap_or(u32::MAX),
            column: u32::try_from(column + 1).unwrap_or(u32::MAX),
        }
    }

    /// How many lines the file has (a file that ends in a line break has no
    /// empty line after it).
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }
}
