//! The STRINGS pool (`plans/02-catalog-format.md` §2.9): every distinct
//! string once, identifiers first, then text, each group sorted bytewise,
//! each string NUL-terminated.
//!
//! The writer encodes everything twice with the same code: pass 1 only
//! collects strings (every reference is 0), [`Pool::finish`] lays the pool
//! out, and pass 2 gets the final offsets. So the two passes cannot disagree
//! on which strings exist.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::WriteError;
use crate::format::MAX_OFFSET;

/// Which group of the pool a use puts a string in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// Names, keys, identifiers, tags, option values: sorted first.
    Ident,
    /// Message text, literal operands, attribute and catch-all values.
    Text,
}

pub(crate) struct Pool {
    /// Pass 2: offsets are final.
    fixed: bool,
    /// Every string: whether some use is an identifier, and its offset.
    strings: BTreeMap<String, (bool, u32)>,
    bytes: Vec<u8>,
}

impl Pool {
    pub(crate) fn new() -> Self {
        Pool {
            fixed: false,
            strings: BTreeMap::new(),
            bytes: Vec::new(),
        }
    }

    /// The `StrRef` of `s` (0 during pass 1).
    pub(crate) fn r(&mut self, s: &str, class: Class) -> Result<u32, WriteError> {
        let ident = class == Class::Ident;
        if self.fixed {
            return self
                .strings
                .get(s)
                .map(|e| e.1)
                .ok_or(WriteError::Internal("string missing from pass 1"));
        }
        if let Some(e) = self.strings.get_mut(s) {
            e.0 |= ident;
        } else {
            if s.as_bytes().contains(&0) {
                return Err(WriteError::Nul(String::from(s)));
            }
            self.strings.insert(String::from(s), (ident, 0));
        }
        Ok(0)
    }

    /// Lays the pool out: identifiers, then text, each sorted bytewise
    /// (`String`'s order is bytewise). Switches to pass 2.
    pub(crate) fn finish(&mut self) -> Result<(), WriteError> {
        let mut bytes = Vec::new();
        for group in [true, false] {
            for (s, e) in &mut self.strings {
                if e.0 == group {
                    e.1 = u32::try_from(bytes.len())
                        .ok()
                        .filter(|&o| o <= MAX_OFFSET)
                        .ok_or(WriteError::TooLarge("STRINGS"))?;
                    bytes.extend_from_slice(s.as_bytes());
                    bytes.push(0);
                }
            }
        }
        if bytes.len() > MAX_OFFSET as usize {
            return Err(WriteError::TooLarge("STRINGS"));
        }
        self.bytes = bytes;
        self.fixed = true;
        Ok(())
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
