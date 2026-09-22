//! CLDR's message-like patterns (`{0} km`, `{0} {1}`, `{0}/{1}`) → the
//! template parts of `plans/02-catalog-format.md` §4.6. Each placeholder
//! occurs at most once; a pattern may have none (`ar` writes some dual
//! forms with the number in the word); any other brace, and a control
//! character, is refused, so a CLDR update that uses one is noticed.

use mf2_catalog::number::TemplatePart;

use crate::error::Error;

fn bad(pattern: &str, message: &'static str) -> Error {
    Error::Pattern {
        pattern: pattern.to_owned(),
        message,
    }
}

/// Parses `p`, allowing `{0}` … `{args − 1}`.
pub fn parse(p: &str, args: u8) -> Result<Vec<TemplatePart<'_>>, Error> {
    let mut out = Vec::new();
    let mut seen = [false; 2];
    let mut rest = p;
    while !rest.is_empty() {
        let at = rest.find(['{', '}']).unwrap_or(rest.len());
        if at > 0 {
            let text = &rest[..at];
            if text.chars().any(char::is_control) {
                return Err(bad(p, "a control character"));
            }
            out.push(TemplatePart::Text(text));
        }
        rest = &rest[at..];
        if rest.is_empty() {
            break;
        }
        let (part, i) = if rest.starts_with("{0}") {
            (TemplatePart::Arg0, 0usize)
        } else if rest.starts_with("{1}") {
            (TemplatePart::Arg1, 1usize)
        } else {
            return Err(bad(p, "a brace that is not `{0}` or `{1}`"));
        };
        if i >= usize::from(args) {
            return Err(bad(p, "a placeholder the pattern may not have"));
        }
        if seen[i] {
            return Err(bad(p, "a placeholder twice"));
        }
        seen[i] = true;
        out.push(part);
        rest = &rest[3..];
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::parse;
    use mf2_catalog::number::TemplatePart::{Arg0, Arg1, Text};

    #[test]
    fn shapes() {
        assert_eq!(parse("{0} km", 1).expect("km"), [Arg0, Text(" km")]);
        assert_eq!(parse("{0} {1}", 2).expect("name"), [Arg0, Text(" "), Arg1]);
        assert_eq!(parse("دورتان", 1).expect("dual"), [Text("دورتان")]);
        assert_eq!(parse("{1}/{0}", 2).expect("rtl"), [Arg1, Text("/"), Arg0]);
        for p in ["{2}", "{0}{0}", "{1}", "a}", "{x}", "\u{1}{0}"] {
            assert!(parse(p, 1).is_err(), "{p}");
        }
    }
}
