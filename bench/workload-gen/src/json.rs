//! A tiny JSON writer. Output format is fixed here (not delegated to a
//! serializer) so that committed corpora are byte-stable across dependency
//! versions.

use std::fmt::Write as _;

/// Appends `s` as a JSON string literal: `"`, `\` and control characters are
/// escaped, everything else (including non-ASCII) is written raw.
pub fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A flat JSON object, one `"key": "value"` pair per line, in the given order.
pub fn flat_object<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut out = String::from("{\n");
    let mut first = true;
    for (key, value) in pairs {
        if !first {
            out.push_str(",\n");
        }
        first = false;
        out.push_str("  ");
        string(&mut out, key);
        out.push_str(": ");
        string(&mut out, value);
    }
    out.push_str(if first { "}\n" } else { "\n}\n" });
    out
}

#[cfg(test)]
mod tests {
    use super::flat_object;

    #[test]
    fn escapes_and_parses_back() {
        let s = flat_object([("a.b", "x \"q\" \\ \n\u{1}é")]);
        assert_eq!(s, "{\n  \"a.b\": \"x \\\"q\\\" \\\\ \\n\\u0001é\"\n}\n");
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["a.b"], "x \"q\" \\ \n\u{1}é");
    }
}
