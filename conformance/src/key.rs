//! The ledger key of a suite test: (`file`, `hash`, `nth`) — plans/01-conformance.md §4.
//!
//! # The hash, byte for byte
//!
//! `hash` is the first 4 bytes of SHA-256, as 8 lowercase hex digits, over the
//! concatenation of four *fields*, taken **after** the file's
//! `defaultTestProperties` have been applied to the test (a test's own property
//! replaces the default of the same name wholesale):
//!
//! ```text
//! input = field(src) ‖ field(params) ‖ field(locale) ‖ field(bidiIsolation)
//!
//! field(absent) = 0x00
//! field(bytes)  = 0x01 ‖ len(bytes) as u64, big-endian ‖ bytes
//! ```
//!
//! * `src`, `locale`, `bidiIsolation`: the UTF-8 bytes of the JSON string value.
//!   An absent `bidiIsolation` is encoded as absent — it is *not* replaced by
//!   the spec's default strategy, so the key reflects the test data as written.
//! * `params`: the UTF-8 bytes of the **canonical JSON** of the whole `params`
//!   value (normally an array of `{name, value[, type]}` objects), defined by
//!   [`canonical_json`]:
//!   * no insignificant whitespace; array order preserved;
//!   * object members sorted by the UTF-8 bytes of their keys;
//!   * strings in double quotes; `"` → `\"`, `\` → `\\`, U+0008 → `\b`,
//!     U+000C → `\f`, U+000A → `\n`, U+000D → `\r`, U+0009 → `\t`, any other
//!     code point below U+0020 → `\u00xx` (lowercase hex); every other code
//!     point is written as itself in UTF-8 (so `"\u1E0c"` in the file hashes
//!     as the literal character `Ḍ`);
//!   * a number written without fraction or exponent that fits `i64`/`u64` →
//!     its decimal integer; any other number → the IEEE-754 double it denotes,
//!     in Rust's `{:?}` form (shortest round-trip digits, always containing
//!     `.` or `e`: `4.2`, `1.0`, `1e21`), so `1` and `1.0` stay distinct;
//!   * `true`, `false`, `null` as literals.
//!
//! `src` and `locale` are required after defaults, so they are always present.
//! The length prefixes and presence bytes make the concatenation unambiguous.
//!
//! `nth` is the 0-based occurrence ordinal of the test among the tests of the
//! same file with the same hash, in file order (`syntax.json` #78 ≡ #79 and
//! #95 ≡ #98 are byte-identical and get `nth` 0 and 1).

use std::fmt::{self, Write as _};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Identifies one suite test in the ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestKey {
    /// Path of the suite file relative to `test/tests/`, with `/` separators.
    pub file: String,
    /// 8 lowercase hex digits; see the module documentation.
    pub hash: String,
    /// Occurrence ordinal among tests of `file` with the same `hash`.
    pub nth: u32,
}

impl fmt::Display for TestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}/{}]", self.file, self.hash, self.nth)
    }
}

/// The exact byte string that is hashed; see the module documentation.
pub fn hash_input(
    src: &str,
    params: Option<&Value>,
    locale: &str,
    bidi_isolation: Option<&str>,
) -> Vec<u8> {
    let params = params.map(canonical_json);
    let mut buf = Vec::new();
    push_field(&mut buf, Some(src.as_bytes()));
    push_field(&mut buf, params.as_deref().map(str::as_bytes));
    push_field(&mut buf, Some(locale.as_bytes()));
    push_field(&mut buf, bidi_isolation.map(str::as_bytes));
    buf
}

/// `sha256(hash_input(..))[..4]` as 8 lowercase hex digits.
pub fn key_hash(
    src: &str,
    params: Option<&Value>,
    locale: &str,
    bidi_isolation: Option<&str>,
) -> String {
    let digest = Sha256::digest(hash_input(src, params, locale, bidi_isolation));
    let mut hex = String::with_capacity(8);
    for byte in digest.iter().take(4) {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn push_field(buf: &mut Vec<u8>, field: Option<&[u8]>) {
    match field {
        None => buf.push(0),
        Some(bytes) => {
            buf.push(1);
            let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend_from_slice(bytes);
        }
    }
}

/// Canonical JSON text of `value`; the rules are in the module documentation.
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out);
    out
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                let _ = write!(out, "{i}");
            } else if let Some(u) = n.as_u64() {
                let _ = write!(out, "{u}");
            } else if let Some(f) = n.as_f64() {
                let _ = write!(out, "{f:?}");
            } else {
                // Unreachable for numbers parsed from JSON text.
                out.push_str(&n.to_string());
            }
        }
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            // Sorted explicitly: serde_json's map order depends on its features.
            let mut members: Vec<(&String, &Value)> = map.iter().collect();
            members.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            out.push('{');
            for (i, (k, v)) in members.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_value(v, out);
            }
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::{canonical_json, hash_input, key_hash};
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_keys_and_keeps_number_forms() {
        let v: serde_json::Value = serde_json::from_str(
            r#"[ { "value": 1.0, "name": "a\u1E0c\n" }, {"value": 1, "name": "b"}, 4.2, 1e21, true, null ]"#,
        )
        .unwrap();
        assert_eq!(
            canonical_json(&v),
            "[{\"name\":\"a\u{1E0C}\\n\",\"value\":1.0},{\"name\":\"b\",\"value\":1},4.2,1e21,true,null]"
        );
    }

    #[test]
    fn hash_input_layout() {
        let params = json!([{"name": "x", "value": 1}]);
        let bytes = hash_input("a", Some(&params), "en-US", None);
        let p = br#"[{"name":"x","value":1}]"#;
        let mut expected = vec![1u8];
        expected.extend_from_slice(&1u64.to_be_bytes());
        expected.push(b'a');
        expected.push(1);
        expected.extend_from_slice(&(p.len() as u64).to_be_bytes());
        expected.extend_from_slice(p);
        expected.push(1);
        expected.extend_from_slice(&5u64.to_be_bytes());
        expected.extend_from_slice(b"en-US");
        expected.push(0);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn hash_golden() {
        // Cross-checked with an independent implementation (Python hashlib) of
        // the encoding in the module documentation.
        assert_eq!(key_hash("", None, "en-US", Some("none")), GOLDEN_EMPTY);
        let params = json!([{"name": "x", "value": 1}]);
        assert_eq!(
            key_hash("{$x}", Some(&params), "en-US", None),
            GOLDEN_PARAMS
        );
    }

    const GOLDEN_EMPTY: &str = "5ba88f35";
    const GOLDEN_PARAMS: &str = "01643909";
}
