//! Locales: the source locale `en`, synthetic real locales with their own
//! plural categories, and the pseudo-locales `en-XA` (accented, expanded,
//! bracketed) and `ar-XB` (every text run wrapped in RLO … PDF).

use crate::error::Error;
use crate::knobs::Knobs;
use crate::model::{Body, Message, Part, Pattern, Workload, canary, render_body};
use crate::rng::Rng;
use crate::text::{accent, synthetic};
use crate::vocab::{self, RealLocale};

/// RIGHT-TO-LEFT OVERRIDE, opening an `ar-XB` text run.
pub const RLO: char = '\u{202E}';
/// POP DIRECTIONAL FORMATTING, closing an `ar-XB` text run.
pub const PDF: char = '\u{202C}';

/// How a locale's text is produced.
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// The source locale (`en`).
    Source,
    /// A real locale with synthetic text.
    Real(RealLocale),
    /// `en-XA`.
    Accented,
    /// `ar-XB`.
    Rtl,
}

/// One output locale.
#[derive(Debug, Clone)]
pub struct Locale {
    /// BCP 47 tag.
    pub tag: &'static str,
    /// Text production.
    pub kind: Kind,
}

impl Locale {
    /// Text direction.
    pub fn is_rtl(&self) -> bool {
        match self.kind {
            Kind::Rtl => true,
            Kind::Real(r) => r.tag == "ar",
            _ => false,
        }
    }
}

/// The locales `knobs` asks for, source first, pseudo-locales last.
pub fn locales(knobs: &Knobs) -> Result<Vec<Locale>, Error> {
    let extra = knobs.locales.checked_sub(1).ok_or_else(|| {
        Error::Knobs("--locales counts the source locale and must be at least 1".into())
    })?;
    let mut out = vec![Locale {
        tag: "en",
        kind: Kind::Source,
    }];
    for real in vocab::REAL_LOCALES.iter().take(extra) {
        out.push(Locale {
            tag: real.tag,
            kind: Kind::Real(*real),
        });
    }
    if !knobs.no_pseudo {
        out.push(Locale {
            tag: "en-XA",
            kind: Kind::Accented,
        });
        out.push(Locale {
            tag: "ar-XB",
            kind: Kind::Rtl,
        });
    }
    Ok(out)
}

/// MF2 source of every message in `locale`, indexed like `wl.messages`.
pub fn sources(wl: &Workload, locale: &Locale) -> Vec<String> {
    bodies(wl, locale)
        .iter()
        .zip(&wl.messages)
        .map(|(body, m)| render_body(body, &m.vars))
        .collect()
}

/// The body of every message in `locale`, indexed like `wl.messages`: what
/// [`sources`] renders as MF2 and `crate::fluent` as Fluent.
pub fn bodies(wl: &Workload, locale: &Locale) -> Vec<Body> {
    wl.messages
        .iter()
        .enumerate()
        .map(|(j, m)| body(wl.knobs.seed, j, m, locale))
        .collect()
}

fn body(seed: u64, j: usize, message: &Message, locale: &Locale) -> Body {
    if message.canary {
        // Never pseudo-localised: CI greps for the literal text.
        return Body::Pattern(vec![
            Part::Text(format!("{} ", canary::text(locale.tag))),
            Part::Var(0),
        ]);
    }
    match locale.kind {
        Kind::Source => message.source.clone(),
        Kind::Real(real) => {
            let mut rng = Rng::stream(seed, real.tag, j as u64);
            translate(&mut rng, &real, &message.source)
        }
        Kind::Accented => map_patterns(&message.source, pseudo_accent),
        Kind::Rtl => map_patterns(&message.source, rtl_wrap),
    }
}

fn map_patterns(body: &Body, f: fn(&[Part]) -> Pattern) -> Body {
    match body {
        Body::Pattern(p) => Body::Pattern(f(p)),
        Body::Select { selector, variants } => Body::Select {
            selector: *selector,
            variants: variants.iter().map(|(k, p)| (k.clone(), f(p))).collect(),
        },
    }
}

fn translate(rng: &mut Rng, real: &RealLocale, body: &Body) -> Body {
    match body {
        Body::Pattern(p) => Body::Pattern(translate_pattern(rng, real, p)),
        Body::Select { selector, variants } => {
            let one = variants.first().map(|v| &v.1);
            let other = variants.last().map(|v| &v.1);
            let variants = real
                .plural
                .iter()
                .map(|&category| {
                    let from = if category == "one" { one } else { other };
                    let pattern = from.map_or_else(Vec::new, |p| translate_pattern(rng, real, p));
                    let key = if category == "other" { "*" } else { category };
                    (key.to_owned(), pattern)
                })
                .collect();
            Body::Select {
                selector: *selector,
                variants,
            }
        }
    }
}

fn translate_pattern(rng: &mut Rng, real: &RealLocale, pattern: &[Part]) -> Pattern {
    pattern
        .iter()
        .map(|part| match part {
            Part::Text(text) => {
                let core = text.trim();
                if core.is_empty() {
                    return Part::Text(if real.spaced {
                        text.clone()
                    } else {
                        String::new()
                    });
                }
                if !core.chars().any(char::is_alphanumeric) {
                    // Punctuation only (a final period after a placeholder).
                    return Part::Text(text.clone());
                }
                let period = core.ends_with('.');
                let target = core.len() * real.expansion / 100;
                let mut out = String::new();
                if real.spaced && text.starts_with(' ') {
                    out.push(' ');
                }
                out.push_str(&synthetic(rng, real, target));
                if period {
                    out.push('.');
                }
                if real.spaced && text.ends_with(' ') {
                    out.push(' ');
                }
                Part::Text(out)
            }
            other => other.clone(),
        })
        .filter(|p| !matches!(p, Part::Text(t) if t.is_empty()))
        .collect()
}

fn merge_text(parts: Vec<Part>) -> Pattern {
    let mut out: Pattern = Vec::with_capacity(parts.len());
    for part in parts {
        match (out.last_mut(), part) {
            (Some(Part::Text(prev)), Part::Text(t)) => prev.push_str(&t),
            (_, part) => out.push(part),
        }
    }
    out
}

/// `en-XA`: `[` + accented text + padding words + `]`, about 30–40 % longer.
fn pseudo_accent(pattern: &[Part]) -> Pattern {
    let text_bytes: usize = pattern
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.len(),
            Part::Markup { inner, .. } => inner.len(),
            Part::Var(_) => 0,
        })
        .sum();
    let mut parts = vec![Part::Text("[".to_owned())];
    for part in pattern {
        parts.push(match part {
            Part::Text(t) => Part::Text(accent(t)),
            Part::Markup { name, inner } => Part::Markup {
                name,
                inner: accent(inner),
            },
            Part::Var(i) => Part::Var(*i),
        });
    }
    let mut pad = String::new();
    let mut k = 0;
    while pad.len() * 10 < text_bytes * 3 || pad.is_empty() {
        pad.push(' ');
        pad.push_str(vocab::PSEUDO_PAD[k % vocab::PSEUDO_PAD.len()]);
        k += 1;
    }
    pad.push(']');
    parts.push(Part::Text(pad));
    merge_text(parts)
}

/// `ar-XB`: every text run that holds more than whitespace becomes
/// RLO + text + PDF.
fn rtl_wrap(pattern: &[Part]) -> Pattern {
    pattern
        .iter()
        .map(|part| match part {
            Part::Text(t) if !t.trim().is_empty() => Part::Text(format!("{RLO}{t}{PDF}")),
            other => other.clone(),
        })
        .collect()
}
