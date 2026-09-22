//! The parts of a BCP 47 tag the number lookup needs: language, script,
//! region, variants, and the Unicode extension's `nu` keyword. Lenient, like
//! the plural lookup: `_` is read as `-`, case is normalised (`zh-hant-tw`
//! → `zh-Hant-TW`), `root` is `und`, and anything not understood is ignored.

/// A parsed tag.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Tag {
    pub language: String,
    pub script: Option<String>,
    pub region: Option<String>,
    pub variants: Vec<String>,
    /// The `-u-nu-` value, lower case.
    pub nu: Option<String>,
}

fn alpha(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_alphabetic())
}

fn alnum(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        if i == 0 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

impl Tag {
    /// Parses `tag`.
    pub fn parse(tag: &str) -> Tag {
        let normalised = tag.replace('_', "-");
        let mut subtags = normalised.split('-').filter(|s| !s.is_empty()).peekable();
        let mut t = Tag::default();
        let first = subtags.next().unwrap_or("und").to_ascii_lowercase();
        t.language = if first == "root" || !alpha(&first) {
            "und".to_owned()
        } else {
            first
        };
        if let Some(s) = subtags.peek()
            && s.len() == 4
            && alpha(s)
        {
            t.script = Some(title(s));
            subtags.next();
        }
        if let Some(s) = subtags.peek() {
            let region = (s.len() == 2 && alpha(s))
                || (s.len() == 3 && s.bytes().all(|b| b.is_ascii_digit()));
            if region {
                t.region = Some(s.to_ascii_uppercase());
                subtags.next();
            }
        }
        while let Some(s) = subtags.peek() {
            let variant = alnum(s)
                && ((5..=8).contains(&s.len())
                    || (s.len() == 4 && s.as_bytes().first().is_some_and(u8::is_ascii_digit)));
            if !variant {
                break;
            }
            t.variants.push(s.to_ascii_lowercase());
            subtags.next();
        }
        // Extensions: `u` keywords until the next singleton; `x` ends the tag.
        let mut in_u = false;
        let mut key: Option<String> = None;
        for s in subtags {
            let s = s.to_ascii_lowercase();
            if s.len() == 1 {
                if s == "x" {
                    break;
                }
                in_u = s == "u";
                key = None;
                continue;
            }
            if !in_u {
                continue;
            }
            if s.len() == 2 {
                key = Some(s);
                continue;
            }
            if key.as_deref() == Some("nu") && t.nu.is_none() {
                t.nu = Some(s);
            }
        }
        t
    }

    /// `language[-Script][-REGION][-variant…]`, without extensions.
    pub fn base(&self) -> String {
        let mut out = self.language.clone();
        for part in self.script.iter().chain(&self.region).chain(&self.variants) {
            out.push('-');
            out.push_str(part);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::Tag;

    #[test]
    fn parts() {
        let t = Tag::parse("zh_hant_tw");
        assert_eq!(t.base(), "zh-Hant-TW");
        let t = Tag::parse("ar-EG-u-ca-islamic-nu-latn-x-foo-nu-arab");
        assert_eq!(t.base(), "ar-EG");
        assert_eq!(t.nu.as_deref(), Some("latn"));
        let t = Tag::parse("ca-ES-valencia-u-nu-arab");
        assert_eq!(t.variants, ["valencia"]);
        assert_eq!(t.nu.as_deref(), Some("arab"));
        assert_eq!(Tag::parse("root").base(), "und");
        assert_eq!(Tag::parse("es-419").region.as_deref(), Some("419"));
        assert_eq!(Tag::parse("de-1996").variants, ["1996"]);
        assert_eq!(Tag::parse("en-t-ja-u-nu-thai").nu.as_deref(), Some("thai"));
        assert_eq!(Tag::parse("").base(), "und");
    }
}
