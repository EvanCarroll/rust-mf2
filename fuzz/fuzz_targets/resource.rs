//! Fuzz target `resource`: `mf2-resource` on arbitrary input.
//!
//! For every UTF-8 input: no panic, and
//!
//! * every span the parser reports — diagnostics, entries, ids, values,
//!   comments, properties — is in bounds and on character boundaries, and
//!   every cooked offset of every value maps back into the file the same way;
//! * a file the parser read **without a diagnostic** serializes, and that
//!   text parses to the same resource and writes itself again byte for byte
//!   (`mf2 fmt` is idempotent);
//! * **linear time**: the whole check takes at most a fixed budget per input
//!   byte.
//!
//! Non-UTF-8 input is fed through `from_utf8_lossy`, so every run exercises
//! the parser.

#![no_main]

use std::time::{Duration, Instant};

use libfuzzer_sys::fuzz_target;
use mf2_model::Span;
use mf2_resource::{Resource, Style, parse, serialize_with};

fn span_ok(src: &str, span: Span) -> bool {
    let (s, e) = (span.start as usize, span.end as usize);
    s <= e && e <= src.len() && src.is_char_boundary(s) && src.is_char_boundary(e)
}

fn check_spans(src: &str, resource: &Resource<'_, std::borrow::Cow<'_, str>>) {
    for section in &resource.sections {
        if let Some(head) = &section.head {
            assert!(span_ok(src, head.span), "head span {:?}", head.span);
            if let Some(c) = &head.comment {
                assert!(span_ok(src, c.span), "comment span {:?}", c.span);
            }
            for m in &head.meta {
                assert!(span_ok(src, m.span), "property span {:?}", m.span);
                if let Some(s) = m.value_span {
                    assert!(span_ok(src, s), "property value span {s:?}");
                }
            }
        }
        for d in &section.detached {
            assert!(span_ok(src, d.comment.span), "detached span");
        }
        for entry in &section.entries {
            assert!(span_ok(src, entry.span), "entry span {:?}", entry.span);
            assert!(span_ok(src, entry.id_span), "id span {:?}", entry.id_span);
            assert!(span_ok(src, entry.value_span), "value span");
            if let Some(c) = &entry.comment {
                assert!(span_ok(src, c.span), "comment span {:?}", c.span);
            }
            for m in &entry.meta {
                assert!(span_ok(src, m.span), "property span {:?}", m.span);
            }
            for (i, _) in entry.value.char_indices() {
                let at = entry
                    .map
                    .source_offset(i as u32)
                    .expect("a non-empty value is mapped");
                assert!(
                    (at as usize) < src.len() && src.is_char_boundary(at as usize),
                    "cooked offset {i} maps to {at}, outside the file"
                );
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let src: &str = &src;
    let start = Instant::now();

    let (resource, diags) = parse(src);
    for d in &diags {
        assert!(span_ok(src, d.span), "diagnostic span {:?}", d.span);
    }
    check_spans(src, &resource);

    if diags.is_empty() {
        for style in [Style::default(), Style::wrapped(40)] {
            let text = serialize_with(&resource, &style).expect("a clean parse writes back");
            let (again, d2) = parse(&text);
            assert!(d2.is_empty(), "what the serializer wrote does not parse");
            check_spans(&text, &again);
            assert_eq!(again, resource, "round trip via {text:?}");
            let twice = serialize_with(&again, &style).expect("writes back again");
            assert_eq!(twice, text, "fmt is not idempotent");
        }
    }

    let budget = Duration::from_millis(50) + Duration::from_micros(50) * src.len() as u32;
    let elapsed = start.elapsed();
    assert!(
        elapsed <= budget,
        "{elapsed:?} for {} bytes (budget {budget:?})",
        src.len()
    );
});
