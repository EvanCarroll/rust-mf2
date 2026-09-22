//! Generated resources: every production of the working grammar, drawn at
//! random and put through the round trip.
//!
//! The generator builds *models* and lets the serializer write them, so it
//! covers each production the way `mf2 fmt` emits it; the properties are
//!
//! * `parse(serialize(r)) == r` — nothing is lost or invented;
//! * `serialize(parse(serialize(r))) == serialize(r)` — `mf2 fmt` is
//!   idempotent;
//! * no diagnostic on what the serializer wrote;
//! * every span and every [`ValueMap`](mf2_resource::ValueMap) entry lands in
//!   bounds and on a character boundary.
//!
//! Seeded and deterministic: a failure reports the seed that produced it.

use mf2_resource::{
    Comment, Detached, Entry, Head, Id, Meta, Resource, Section, Style, parse, serialize_with,
};
use std::borrow::Cow;

/// `SplitMix64`, so the generator needs no dependency and never changes.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next_u64() % n as u64).expect("below usize::MAX")
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }

    fn pick<'t, T>(&mut self, from: &'t [T]) -> &'t T {
        &from[self.below(from.len())]
    }
}

const WORDS: [&str; 8] = ["send", "Cancel", "más", "日本語", "a b", "OK", "…", "x"];

/// Pieces a value is built from: plain text, the four MF2 escapes the
/// container passes through, placeholders, line breaks and every character
/// the serializer has to escape.
const PIECES: [&str; 16] = [
    "text",
    " leading",
    "trailing ",
    "\\{",
    "\\}",
    "\\\\",
    "\\|",
    "{$count}",
    "{#kbd}Esc{/kbd}",
    "\n",
    "\t",
    "\r",
    "\u{7}",
    "\u{2028}",
    ".match $count",
    "",
];

/// Parts an id is built from: plain ones and ones every symbol of which has
/// to be escaped.
const PARTS: [&str; 12] = [
    "send", "chat", "a-b", "c_d", "ünï", "日本", "x.y", "p q", "r=s", "[t]", "#u", "---",
];

fn value(rng: &mut Rng) -> String {
    let n = rng.below(5);
    (0..n).map(|_| *rng.pick(&PIECES)).collect()
}

fn comment(rng: &mut Rng) -> Comment<'static> {
    let lines = 1 + rng.below(3);
    let text = (0..lines)
        .map(|_| {
            if rng.chance(15) {
                String::new()
            } else {
                let words = 1 + rng.below(4);
                (0..words)
                    .map(|_| *rng.pick(&WORDS))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Comment {
        text: Cow::Owned(text),
        span: mf2_model::Span { start: 0, end: 0 },
    }
}

fn meta(rng: &mut Rng) -> Vec<Meta<'static>> {
    let n = rng.below(3);
    (0..n)
        .map(|_| {
            let name = *rng.pick(&["param", "do-not-translate", "locale", "note"]);
            // A property with an empty value is written as one with none, so
            // the two are the same resource; never generate one.
            let value = if rng.chance(70) {
                let v = value(rng);
                if v.is_empty() {
                    None
                } else {
                    Some(Cow::Owned(v))
                }
            } else {
                None
            };
            Meta {
                name: Cow::Borrowed(name),
                value,
                span: mf2_model::Span { start: 0, end: 0 },
                value_span: None,
            }
        })
        .collect()
}

fn id(rng: &mut Rng) -> Id<'static> {
    let n = 1 + rng.below(3);
    Id::new(
        (0..n)
            .map(|_| Cow::Borrowed(*rng.pick(&PARTS)))
            .collect::<Vec<Cow<'static, str>>>(),
    )
}

fn entry(rng: &mut Rng) -> Entry<'static, String> {
    Entry {
        id: id(rng),
        value: value(rng),
        comment: rng.chance(50).then(|| comment(rng)),
        meta: meta(rng),
        span: mf2_model::Span { start: 0, end: 0 },
        id_span: mf2_model::Span { start: 0, end: 0 },
        value_span: mf2_model::Span { start: 0, end: 0 },
        map: mf2_resource::ValueMap::Empty,
    }
}

fn resource(rng: &mut Rng) -> Resource<'static, String> {
    let mut sections: Vec<Section<'static, String>> = (0..rng.below(4))
        .map(|i| {
            let entries: Vec<_> = (0..rng.below(4)).map(|_| entry(rng)).collect();
            // Only the first section may be headless: a file's entries before
            // its first `[section]`.
            let head = (i > 0 || rng.chance(60)).then(|| Head {
                id: id(rng),
                comment: rng.chance(40).then(|| comment(rng)),
                meta: meta(rng),
                span: mf2_model::Span { start: 0, end: 0 },
            });
            let mut detached: Vec<Detached<'static>> = (0..rng.below(3))
                .map(|_| Detached {
                    before: rng.below(entries.len() + 1),
                    comment: comment(rng),
                })
                .collect();
            // A file reads them back in file order.
            detached.sort_by_key(|d| d.before);
            Section {
                head,
                entries,
                detached,
            }
        })
        .collect();
    // A headless section with nothing in it is not a thing a file can say.
    sections.retain(|s| s.head.is_some() || !s.entries.is_empty() || !s.detached.is_empty());
    Resource {
        comment: rng.chance(40).then(|| comment(rng)),
        meta: meta(rng),
        sections,
    }
}

/// Every span and value map of `resource` lands inside `src`, on a character
/// boundary.
fn spans_are_sane(src: &str, resource: &Resource<'_, Cow<'_, str>>) {
    let ok = |span: mf2_model::Span| {
        let (s, e) = (span.start as usize, span.end as usize);
        assert!(s <= e && e <= src.len(), "span {span:?} out of bounds");
        assert!(
            src.is_char_boundary(s) && src.is_char_boundary(e),
            "span {span:?} splits a character"
        );
    };
    for section in &resource.sections {
        if let Some(head) = &section.head {
            ok(head.span);
            if let Some(c) = &head.comment {
                ok(c.span);
            }
            for m in &head.meta {
                ok(m.span);
            }
        }
        for entry in &section.entries {
            ok(entry.span);
            ok(entry.id_span);
            ok(entry.value_span);
            if let Some(c) = &entry.comment {
                ok(c.span);
            }
            for m in &entry.meta {
                ok(m.span);
            }
            for (i, _) in entry.value.char_indices() {
                let at = entry
                    .map
                    .source_offset(u32::try_from(i).expect("a short value"))
                    .expect("a mapped value");
                assert!(
                    (at as usize) < src.len() && src.is_char_boundary(at as usize),
                    "cooked offset {i} of {:?} maps to {at}",
                    entry.value
                );
            }
        }
    }
}

fn round_trip(seed: u64, style: &Style) {
    let mut rng = Rng(seed);
    let model = resource(&mut rng);
    let src = serialize_with(&model, style).expect("a generated model is writable");
    let (read, diags) = parse(&src);
    assert!(
        diags.is_empty(),
        "seed {seed}: {diags:?} on what the serializer wrote:\n{src}"
    );
    spans_are_sane(&src, &read);

    // The same resource, entry for entry.
    let model_as_read: Resource<'_, Cow<'_, str>> = Resource {
        comment: model.comment.clone(),
        meta: model.meta.clone(),
        sections: model
            .sections
            .iter()
            .map(|s| Section {
                head: s.head.clone(),
                entries: s
                    .entries
                    .iter()
                    .map(|e| Entry {
                        id: e.id.clone(),
                        value: Cow::Borrowed(e.value.as_str()),
                        comment: e.comment.clone(),
                        meta: e.meta.clone(),
                        span: e.span,
                        id_span: e.id_span,
                        value_span: e.value_span,
                        map: e.map.clone(),
                    })
                    .collect(),
                detached: s.detached.clone(),
            })
            .collect(),
    };
    assert_eq!(read, model_as_read, "seed {seed}: round trip via\n{src}");

    let again = serialize_with(&read, style).expect("what parse gave is writable");
    assert_eq!(again, src, "seed {seed}: fmt is not idempotent");
}

#[test]
fn generated_resources_round_trip() {
    for seed in 0..2_000 {
        round_trip(seed, &Style::default());
    }
}

#[test]
fn generated_resources_round_trip_wrapped() {
    for seed in 0..2_000 {
        round_trip(seed, &Style::wrapped(40));
    }
}

/// Every production reaches the generator: the counts say so, and a failure
/// here means a later run stopped covering something.
#[test]
fn the_generator_covers_every_production() {
    let mut rng = Rng(7);
    let (mut heads, mut detached, mut props, mut multiline, mut escapes, mut empty_values) =
        (0, 0, 0, 0, 0, 0);
    let mut frontmatter = 0;
    for _ in 0..500 {
        let r = resource(&mut rng);
        frontmatter += usize::from(r.comment.is_some() || !r.meta.is_empty());
        for s in &r.sections {
            heads += usize::from(s.head.is_some());
            detached += s.detached.len();
            for e in &s.entries {
                props += e.meta.len();
                multiline += usize::from(e.value.contains('\n'));
                escapes += usize::from(e.value.contains('\\'));
                empty_values += usize::from(e.value.is_empty());
            }
        }
    }
    for (what, n) in [
        ("frontmatter", frontmatter),
        ("section heads", heads),
        ("detached comments", detached),
        ("properties", props),
        ("multi-line values", multiline),
        ("escapes", escapes),
        ("empty values", empty_values),
    ] {
        assert!(n > 10, "only {n} {what} in 500 generated resources");
    }
}
