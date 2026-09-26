//! Pseudo-locales (`plans/05-tooling.md` §6): `en-XA` and `ar-XB`, as
//! `bench/workload-gen` defines them.
//!
//! * **`en-XA`** — `[`, the text accented character for character, padding
//!   words that make the message about 30 % longer, `]`. It shows at a glance
//!   which strings are translated, and whether the layout survives a language
//!   that needs more room.
//! * **`ar-XB`** — every text run that holds more than whitespace wrapped in
//!   U+202E RIGHT-TO-LEFT OVERRIDE … U+202C POP DIRECTIONAL FORMATTING, so a
//!   left-to-right build shows its text reversed wherever the base direction
//!   was not applied.
//!
//! Placeholders, selectors, variant keys and markup names are left alone:
//! what a pseudo-locale tests is the text and the layout, not the message's
//! structure. A message the corpus marks `@do-not-translate` is copied as it
//! stands.
//!
//! The tables are `bench/workload-gen`'s, and a test holds them to the
//! `en-XA` and `ar-XB` files the generator writes.

use std::borrow::Cow;

use mf2_model::{Message, Pattern, PatternMessage, PatternPart, SelectMessage};

/// RIGHT-TO-LEFT OVERRIDE, opening an `ar-XB` text run.
pub const RLO: char = '\u{202e}';
/// POP DIRECTIONAL FORMATTING, closing one.
pub const PDF: char = '\u{202c}';

/// Lower-case half of the `en-XA` accent map.
const ACCENTS_LOWER: [&str; 26] = [
    "å", "ƀ", "ç", "ð", "é", "ƒ", "ĝ", "ĥ", "î", "ĵ", "ķ", "ļ", "ɱ", "ñ", "ö", "þ", "ǫ", "ŕ", "š",
    "ţ", "û", "ṽ", "ŵ", "ẋ", "ý", "ž",
];

/// Upper-case half.
const ACCENTS_UPPER: [&str; 26] = [
    "Å", "Ɓ", "Ç", "Ð", "É", "Ƒ", "Ĝ", "Ĥ", "Î", "Ĵ", "Ķ", "Ļ", "Ṁ", "Ñ", "Ö", "Þ", "Ǫ", "Ŕ", "Š",
    "Ţ", "Û", "Ṽ", "Ŵ", "Ẋ", "Ý", "Ž",
];

/// The words `en-XA` pads with.
const PAD: [&str; 10] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];

/// Which pseudo-locale.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// `en-XA`: accented, expanded, bracketed.
    Accented,
    /// `ar-XB`: every text run overridden right-to-left.
    RightToLeft,
}

impl Kind {
    /// The tag it writes.
    pub fn tag(self) -> &'static str {
        match self {
            Kind::Accented => "en-XA",
            Kind::RightToLeft => "ar-XB",
        }
    }

    /// The kind `tag` names.
    pub fn from_tag(tag: &str) -> Option<Kind> {
        match tag {
            "en-XA" => Some(Kind::Accented),
            "ar-XB" => Some(Kind::RightToLeft),
            _ => None,
        }
    }

    /// Both, in the order `mf2 pseudo` writes them.
    pub const ALL: [Kind; 2] = [Kind::Accented, Kind::RightToLeft];
}

/// `message`, pseudo-localized.
pub fn message<'a>(message: &Message<'a>, kind: Kind) -> Message<'a> {
    match message {
        Message::Pattern(m) => Message::Pattern(PatternMessage {
            declarations: m.declarations.clone(),
            pattern: pattern(&m.pattern, kind),
        }),
        Message::Select(m) => Message::Select(SelectMessage {
            declarations: m.declarations.clone(),
            selectors: m.selectors.clone(),
            variants: m
                .variants
                .iter()
                .map(|v| mf2_model::Variant {
                    keys: v.keys.clone(),
                    value: pattern(&v.value, kind),
                })
                .collect(),
        }),
        // A kind this version does not know: left as it is.
        other => other.clone(),
    }
}

fn pattern<'a>(pattern: &Pattern<'a>, kind: Kind) -> Pattern<'a> {
    match kind {
        Kind::Accented => accented(pattern),
        Kind::RightToLeft => right_to_left(pattern),
    }
}

/// How many bytes of text a pattern shows, markup's own text included: what
/// the padding is measured against.
fn text_bytes(pattern: &Pattern<'_>) -> usize {
    pattern
        .into_iter()
        .map(|part| match part {
            PatternPart::Text(t) => t.len(),
            _ => 0,
        })
        .sum()
}

fn accented<'a>(source: &Pattern<'a>) -> Pattern<'a> {
    let bytes = text_bytes(source);
    let mut parts: Vec<PatternPart<'a>> = vec![PatternPart::Text(Cow::Borrowed("["))];
    for part in source {
        parts.push(match part {
            PatternPart::Text(t) => PatternPart::Text(Cow::Owned(accent(t))),
            other => other.clone(),
        });
    }
    let mut pad = String::new();
    let mut k = 0usize;
    while pad.len() * 10 < bytes * 3 || pad.is_empty() {
        pad.push(' ');
        pad.push_str(PAD[k % PAD.len()]);
        k += 1;
    }
    pad.push(']');
    parts.push(PatternPart::Text(Cow::Owned(pad)));
    merge_text(parts)
}

fn right_to_left<'a>(source: &Pattern<'a>) -> Pattern<'a> {
    source
        .into_iter()
        .map(|part| match part {
            PatternPart::Text(t) if !t.trim().is_empty() => {
                PatternPart::Text(Cow::Owned(format!("{RLO}{t}{PDF}")))
            }
            other => other.clone(),
        })
        .collect()
}

/// One accented character per ASCII letter; everything else as it stands.
pub fn accent(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            'a'..='z' => out.push_str(ACCENTS_LOWER[(c as usize) - ('a' as usize)]),
            'A'..='Z' => out.push_str(ACCENTS_UPPER[(c as usize) - ('A' as usize)]),
            _ => out.push(c),
        }
    }
    out
}

/// Adjacent text runs are one run, as [`Pattern`] keeps them.
fn merge_text<'a>(parts: Vec<PatternPart<'a>>) -> Pattern<'a> {
    let mut out: Vec<PatternPart<'a>> = Vec::with_capacity(parts.len());
    for part in parts {
        match (out.last_mut(), part) {
            (Some(PatternPart::Text(previous)), PatternPart::Text(text)) => {
                previous.to_mut().push_str(&text);
            }
            (_, part) => out.push(part),
        }
    }
    Pattern::from(out)
}

#[cfg(test)]
mod tests {
    use super::{Kind, accent, message};

    fn round(source: &str, kind: Kind) -> String {
        let model = mf2_syntax::parse_model(source)
            .message
            .expect("the source parses");
        mf2_syntax::serialize(&message(&model, kind)).expect("it serializes")
    }

    #[test]
    fn accented_text_is_bracketed_and_padded() {
        let out = round("Save", Kind::Accented);
        assert!(out.starts_with("[Šåṽé"), "{out}");
        assert!(out.ends_with(']'), "{out}");
        assert!(out.len() > "Save".len() * 2, "{out}");
        assert_eq!(accent("Az"), "Åž");
        assert_eq!(accent("{$x} 1!"), "{$ẋ} 1!");
    }

    #[test]
    fn a_placeholder_is_never_touched() {
        let out = round("Hello, {$name}!", Kind::Accented);
        assert!(out.contains("{$name}"), "{out}");
        let out = round("Hello, {$name}!", Kind::RightToLeft);
        assert!(out.contains("{$name}"), "{out}");
    }

    #[test]
    fn right_to_left_wraps_every_text_run() {
        let out = round("Hello, {$name}!", Kind::RightToLeft);
        assert_eq!(out, "\u{202e}Hello, \u{202c}{$name}\u{202e}!\u{202c}");
        // A run of nothing but spaces is left alone: wrapping it would add
        // two invisible characters and show nothing.
        let out = round("{$a} {$b}", Kind::RightToLeft);
        assert_eq!(out, "{$a} {$b}");
    }

    #[test]
    fn every_variant_of_a_select_is_localized() {
        let out = round(
            ".input {$n :integer}\n.match $n\none {{one item}}\n* {{{$n} items}}",
            Kind::RightToLeft,
        );
        assert!(out.contains("\u{202e}one item\u{202c}"), "{out}");
        assert!(out.contains("\u{202e} items\u{202c}"), "{out}");
        // The selector and the keys are structure, not text.
        assert!(out.contains(".input {$n :integer}"), "{out}");
        assert!(out.contains("one {{"), "{out}");
    }
}
