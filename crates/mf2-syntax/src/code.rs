//! The stable diagnostic codes of `mf2-syntax` ([`mf2_model::Diagnostic::code`]).
//!
//! A code identifies *what* went wrong more precisely than the
//! [`mf2_model::ErrorKind`]. Codes are stable: a code's meaning never
//! changes, and a retired code is never reused. `0` means "no detail".
//!
//! | Code | Kind | Meaning |
//! |---:|---|---|
//! | 1 | syntax | U+0000 is not allowed anywhere in a message |
//! | 2 | syntax | `}` in text must be escaped as `\}` |
//! | 3 | syntax | `\` must be followed by `\`, `{`, `\|` or `}` |
//! | 4 | syntax | a placeholder is missing its closing `}` |
//! | 5 | syntax | a placeholder is empty (`{}`) |
//! | 6 | syntax | a character that cannot appear here |
//! | 7 | syntax | whitespace is required here |
//! | 8 | syntax | a name must start here |
//! | 9 | syntax | `=` expected (option; `.local` declaration) |
//! | 10 | syntax | an option needs a value: a literal or a variable |
//! | 11 | syntax | an attribute's value must be a literal |
//! | 12 | syntax | a quoted literal is missing its closing `\|` |
//! | 13 | syntax | not a keyword (`.input`, `.local`, `.match`) |
//! | 14 | syntax | an expression `{…}` expected |
//! | 15 | syntax | `.input` needs a variable expression `{$name …}` |
//! | 16 | syntax | `.local` needs a variable `$name` |
//! | 17 | syntax | a complex message needs a body: `{{…}}` or `.match` |
//! | 18 | syntax | nothing may follow the body of a complex message |
//! | 19 | syntax | a quoted pattern is missing its closing `}}` |
//! | 20 | syntax | `.match` needs at least one selector `$name` |
//! | 21 | syntax | `.match` needs at least one variant |
//! | 22 | syntax | a variant needs at least one key |
//! | 23 | syntax | a variant needs a quoted pattern `{{…}}` |
//! | 24 | syntax | markup is not allowed in a declaration |
//! | 25 | syntax | an expression needs an operand or a function before attributes |
//! | 26 | syntax | the source is longer than `u32::MAX` bytes |
//! | 101 | variant-key-mismatch | a variant's key count differs from the selector count |
//! | 102 | missing-fallback-variant | no variant has only catch-all keys |
//! | 103 | missing-selector-annotation | the selector does not (directly or indirectly) reference a declaration with a function |
//! | 104 | duplicate-declaration | the variable was declared, or used, in a previous declaration |
//! | 105 | duplicate-declaration | the declaration's own expression uses the variable it binds |
//! | 106 | duplicate-option-name | two options of one function or markup have the same name (under NFC) |
//! | 107 | duplicate-variant | two variants have the same keys (under NFC) |

/// U+0000 is not allowed anywhere in a message.
pub const NUL_CHARACTER: u16 = 1;
/// `}` in text must be escaped as `\}`.
pub const UNESCAPED_CLOSE_BRACE: u16 = 2;
/// `\` must be followed by `\`, `{`, `|` or `}`.
pub const INVALID_ESCAPE: u16 = 3;
/// A placeholder is missing its closing `}`.
pub const UNTERMINATED_PLACEHOLDER: u16 = 4;
/// A placeholder is empty (`{}`).
pub const EMPTY_PLACEHOLDER: u16 = 5;
/// A character that cannot appear here.
pub const UNEXPECTED_CHARACTER: u16 = 6;
/// Whitespace is required here.
pub const MISSING_WHITESPACE: u16 = 7;
/// A name must start here.
pub const EXPECTED_NAME: u16 = 8;
/// `=` expected (option; `.local` declaration).
pub const EXPECTED_EQUALS: u16 = 9;
/// An option needs a value: a literal or a variable.
pub const EXPECTED_OPTION_VALUE: u16 = 10;
/// An attribute's value must be a literal.
pub const EXPECTED_LITERAL: u16 = 11;
/// A quoted literal is missing its closing `|`.
pub const UNTERMINATED_QUOTED_LITERAL: u16 = 12;
/// Not a keyword (`.input`, `.local`, `.match`).
pub const UNKNOWN_KEYWORD: u16 = 13;
/// An expression `{…}` expected.
pub const EXPECTED_EXPRESSION: u16 = 14;
/// `.input` needs a variable expression `{$name …}`.
pub const EXPECTED_VARIABLE_EXPRESSION: u16 = 15;
/// `.local` needs a variable `$name`.
pub const EXPECTED_VARIABLE: u16 = 16;
/// A complex message needs a body: `{{…}}` or `.match`.
pub const MISSING_BODY: u16 = 17;
/// Nothing may follow the body of a complex message.
pub const CONTENT_AFTER_BODY: u16 = 18;
/// A quoted pattern is missing its closing `}}`.
pub const UNTERMINATED_QUOTED_PATTERN: u16 = 19;
/// `.match` needs at least one selector `$name`.
pub const EXPECTED_SELECTOR: u16 = 20;
/// `.match` needs at least one variant.
pub const EXPECTED_VARIANT: u16 = 21;
/// A variant needs at least one key.
pub const EXPECTED_KEY: u16 = 22;
/// A variant needs a quoted pattern `{{…}}`.
pub const EXPECTED_QUOTED_PATTERN: u16 = 23;
/// Markup is not allowed in a declaration.
pub const MARKUP_NOT_ALLOWED: u16 = 24;
/// An expression needs an operand or a function before attributes.
pub const EXPECTED_OPERAND: u16 = 25;
/// The source is longer than `u32::MAX` bytes.
pub const SOURCE_TOO_LONG: u16 = 26;

/// A variant's key count differs from the selector count.
pub const VARIANT_KEY_MISMATCH: u16 = 101;
/// No variant has only catch-all keys.
pub const MISSING_FALLBACK_VARIANT: u16 = 102;
/// The selector does not (directly or indirectly) reference a declaration
/// with a function.
pub const MISSING_SELECTOR_ANNOTATION: u16 = 103;
/// The variable was declared, or used, in a previous declaration.
pub const DUPLICATE_DECLARATION: u16 = 104;
/// The declaration's own expression uses the variable it binds.
pub const SELF_REFERENCING_DECLARATION: u16 = 105;
/// Two options of one function or markup have the same name (under NFC).
pub const DUPLICATE_OPTION_NAME: u16 = 106;
/// Two variants have the same keys (under NFC).
pub const DUPLICATE_VARIANT: u16 = 107;

/// A one-line description of `code`, or `None` for an unknown code.
pub fn describe(code: u16) -> Option<&'static str> {
    Some(match code {
        NUL_CHARACTER => "U+0000 is not allowed anywhere in a message",
        UNESCAPED_CLOSE_BRACE => "`}` in text must be escaped as `\\}`",
        INVALID_ESCAPE => "`\\` must be followed by `\\`, `{`, `|` or `}`",
        UNTERMINATED_PLACEHOLDER => "a placeholder is missing its closing `}`",
        EMPTY_PLACEHOLDER => "a placeholder is empty (`{}`)",
        UNEXPECTED_CHARACTER => "a character that cannot appear here",
        MISSING_WHITESPACE => "whitespace is required here",
        EXPECTED_NAME => "a name must start here",
        EXPECTED_EQUALS => "`=` expected",
        EXPECTED_OPTION_VALUE => "an option needs a value: a literal or a variable",
        EXPECTED_LITERAL => "an attribute's value must be a literal",
        UNTERMINATED_QUOTED_LITERAL => "a quoted literal is missing its closing `|`",
        UNKNOWN_KEYWORD => "not a keyword (`.input`, `.local`, `.match`)",
        EXPECTED_EXPRESSION => "an expression `{…}` expected",
        EXPECTED_VARIABLE_EXPRESSION => "`.input` needs a variable expression `{$name …}`",
        EXPECTED_VARIABLE => "`.local` needs a variable `$name`",
        MISSING_BODY => "a complex message needs a body: `{{…}}` or `.match`",
        CONTENT_AFTER_BODY => "nothing may follow the body of a complex message",
        UNTERMINATED_QUOTED_PATTERN => "a quoted pattern is missing its closing `}}`",
        EXPECTED_SELECTOR => "`.match` needs at least one selector `$name`",
        EXPECTED_VARIANT => "`.match` needs at least one variant",
        EXPECTED_KEY => "a variant needs at least one key",
        EXPECTED_QUOTED_PATTERN => "a variant needs a quoted pattern `{{…}}`",
        MARKUP_NOT_ALLOWED => "markup is not allowed in a declaration",
        EXPECTED_OPERAND => "an expression needs an operand or a function before attributes",
        SOURCE_TOO_LONG => "the source is longer than u32::MAX bytes",
        VARIANT_KEY_MISMATCH => "a variant's key count differs from the selector count",
        MISSING_FALLBACK_VARIANT => "no variant has only catch-all keys",
        MISSING_SELECTOR_ANNOTATION => {
            "the selector does not reference a declaration with a function"
        }
        DUPLICATE_DECLARATION => "the variable was declared, or used, in a previous declaration",
        SELF_REFERENCING_DECLARATION => {
            "the declaration's own expression uses the variable it binds"
        }
        DUPLICATE_OPTION_NAME => "two options have the same name",
        DUPLICATE_VARIANT => "two variants have the same keys",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::describe;

    #[test]
    fn every_code_is_described_and_zero_is_not() {
        for code in (1..=26).chain(101..=107) {
            assert!(describe(code).is_some(), "{code}");
        }
        for code in [0, 27, 100, 108, u16::MAX] {
            assert!(describe(code).is_none(), "{code}");
        }
    }
}
