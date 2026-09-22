//! The stable diagnostic codes of `mf2-resource` ([`crate::Diagnostic::code`]).
//!
//! A code's meaning never changes, and a retired code is never reused, so a
//! `check` configuration or a CI filter may name one.
//!
//! | Code | Meaning |
//! |---:|---|
//! | 1 | the source is longer than `u32::MAX` bytes |
//! | 2 | an entry needs `=` between its id and its value |
//! | 3 | an id is empty |
//! | 4 | an id has an empty part (`a..b`, `.a`, `a.`) |
//! | 5 | a section head is missing its `]` |
//! | 6 | nothing may follow a section head's `]` |
//! | 7 | the frontmatter separator `---` may appear only once, before any entry |
//! | 8 | an indented line continues nothing |
//! | 9 | `\` is followed by a character that is not an escape |
//! | 10 | an escaped line break is not followed by a continuation line |
//! | 11 | a control character must be escaped |
//! | 12 | U+2028 and U+2029 must be escaped |
//! | 13 | a property needs a name after `@` |
//! | 14 | a property is cut off from what it describes by an empty line |
//! | 15 | an id may not look like the frontmatter separator `---` |
//! | 16 | a hex escape is not a Unicode scalar value |
//! | 17 | an id part needs `\` before this character |

/// The source is longer than `u32::MAX` bytes.
pub const SOURCE_TOO_LONG: u16 = 1;
/// An entry needs `=` between its id and its value.
pub const EXPECTED_EQUALS: u16 = 2;
/// An id is empty.
pub const EMPTY_ID: u16 = 3;
/// An id has an empty part (`a..b`, `.a`, `a.`).
pub const EMPTY_ID_PART: u16 = 4;
/// A section head is missing its `]`.
pub const UNTERMINATED_SECTION: u16 = 5;
/// Nothing may follow a section head's `]`.
pub const TRAILING_AFTER_SECTION: u16 = 6;
/// The frontmatter separator `---` may appear only once, before any entry.
pub const UNEXPECTED_FRONTMATTER: u16 = 7;
/// An indented line continues nothing.
pub const STRAY_CONTINUATION: u16 = 8;
/// `\` is followed by a character that is not an escape.
pub const INVALID_ESCAPE: u16 = 9;
/// An escaped line break is not followed by a continuation line.
pub const DANGLING_LINE_BREAK: u16 = 10;
/// A control character must be escaped.
pub const RAW_CONTROL: u16 = 11;
/// U+2028 and U+2029 must be escaped.
pub const RAW_LINE_SEPARATOR: u16 = 12;
/// A property needs a name after `@`.
pub const EXPECTED_PROPERTY_NAME: u16 = 13;
/// A property is cut off from what it describes by an empty line.
pub const DETACHED_PROPERTY: u16 = 14;
/// An id may not look like the frontmatter separator `---`.
pub const ID_LIKE_FRONTMATTER: u16 = 15;
/// A hex escape is not a Unicode scalar value.
pub const BAD_HEX_ESCAPE: u16 = 16;
/// An id part needs `\` before this character.
pub const UNESCAPED_ID_CHAR: u16 = 17;

/// What a code means, as one sentence without a trailing full stop.
///
/// An unknown code (a file written by a newer version) gives
/// `"a resource syntax error"`.
pub fn message(code: u16) -> &'static str {
    match code {
        SOURCE_TOO_LONG => "the source is longer than u32::MAX bytes",
        EXPECTED_EQUALS => "an entry needs `=` between its id and its value",
        EMPTY_ID => "an id is empty",
        EMPTY_ID_PART => "an id has an empty part",
        UNTERMINATED_SECTION => "a section head is missing its `]`",
        TRAILING_AFTER_SECTION => "nothing may follow a section head's `]`",
        UNEXPECTED_FRONTMATTER => {
            "the frontmatter separator `---` may appear only once, before any entry"
        }
        STRAY_CONTINUATION => "an indented line continues nothing",
        INVALID_ESCAPE => "`\\` is followed by a character that is not an escape",
        DANGLING_LINE_BREAK => "an escaped line break is not followed by a continuation line",
        RAW_CONTROL => "a control character must be escaped",
        RAW_LINE_SEPARATOR => "U+2028 and U+2029 must be escaped",
        EXPECTED_PROPERTY_NAME => "a property needs a name after `@`",
        DETACHED_PROPERTY => "a property is cut off from what it describes by an empty line",
        ID_LIKE_FRONTMATTER => "an id may not look like the frontmatter separator `---`",
        BAD_HEX_ESCAPE => "a hex escape is not a Unicode scalar value",
        UNESCAPED_ID_CHAR => "an id part needs `\\` before this character",
        _ => "a resource syntax error",
    }
}

/// Every code this version reports, in numeric order.
pub const ALL: [u16; 17] = [
    SOURCE_TOO_LONG,
    EXPECTED_EQUALS,
    EMPTY_ID,
    EMPTY_ID_PART,
    UNTERMINATED_SECTION,
    TRAILING_AFTER_SECTION,
    UNEXPECTED_FRONTMATTER,
    STRAY_CONTINUATION,
    INVALID_ESCAPE,
    DANGLING_LINE_BREAK,
    RAW_CONTROL,
    RAW_LINE_SEPARATOR,
    EXPECTED_PROPERTY_NAME,
    DETACHED_PROPERTY,
    ID_LIKE_FRONTMATTER,
    BAD_HEX_ESCAPE,
    UNESCAPED_ID_CHAR,
];
