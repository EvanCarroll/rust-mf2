//! One test per production of the working grammar,
//! and the examples that document it.

use mf2_resource::{Resource, code, parse, serialize};
use std::borrow::Cow;

/// Parses and asserts that nothing was reported.
fn ok(src: &str) -> Resource<'_, Cow<'_, str>> {
    let (resource, diags) = parse(src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diags
            .iter()
            .map(|d| (d.code, d.message(), d.span))
            .collect::<Vec<_>>()
    );
    resource
}

/// The entries of a resource as `(full id, value)`, in file order.
fn entries(resource: &Resource<'_, Cow<'_, str>>) -> Vec<(String, String)> {
    resource
        .iter()
        .map(|e| (e.id().to_string(), e.entry.value.to_string()))
        .collect()
}

fn codes(src: &str) -> Vec<u16> {
    parse(src).1.iter().map(|d| d.code).collect()
}

// ───────────────────────────── frontmatter ─────────────────────────────

#[test]
fn frontmatter_holds_the_locale_and_its_comment() {
    let r = ok("# About this file.\n@locale en-US\n---\n\nsend = Send\n");
    assert_eq!(r.locale(), Some("en-US"));
    assert_eq!(
        r.comment.as_ref().map(|c| c.text.as_ref()),
        Some("About this file.")
    );
    assert_eq!(entries(&r), [("send".into(), "Send".into())]);
}

#[test]
fn a_file_may_have_no_frontmatter() {
    let r = ok("send = Send\n");
    assert_eq!(r.locale(), None);
    assert_eq!(entries(&r), [("send".into(), "Send".into())]);
}

#[test]
fn a_second_separator_is_an_error() {
    assert_eq!(
        codes("@locale en\n---\nsend = Send\n---\n"),
        [code::UNEXPECTED_FRONTMATTER]
    );
}

#[test]
fn the_separator_may_not_follow_an_entry() {
    // The body has begun, so `---` is no longer frontmatter.
    assert_eq!(codes("send = Send\n---\n"), [code::UNEXPECTED_FRONTMATTER]);
}

// ──────────────────────────────── entries ───────────────────────────────

#[test]
fn spaces_around_the_equals_are_not_part_of_the_value() {
    let r = ok("a=x\nb   =   y\nc =\n");
    assert_eq!(
        entries(&r),
        [
            ("a".into(), "x".into()),
            ("b".into(), "y".into()),
            ("c".into(), String::new()),
        ]
    );
}

#[test]
fn an_entry_without_equals_is_reported_and_skipped() {
    let r = parse("send Send\nok = Yes\n");
    assert_eq!(
        r.1.iter().map(|d| d.code).collect::<Vec<_>>(),
        [code::EXPECTED_EQUALS]
    );
    assert_eq!(entries(&r.0), [("ok".into(), "Yes".into())]);
}

#[test]
fn the_first_unescaped_equals_ends_the_id() {
    let r = ok("a\\=b = x=y\n");
    assert_eq!(entries(&r), [("a\\=b".into(), "x=y".into())]);
    assert_eq!(r.sections[0].entries[0].id.parts(), ["a=b"]);
}

// ─────────────────────────── continuation lines ─────────────────────────

#[test]
fn continuation_lines_lose_their_indent_and_gain_one_lf() {
    let r = ok("key =\n  .input {$count :integer}\n  .match $count\n  * {{x}}\n");
    assert_eq!(
        entries(&r),
        [(
            "key".into(),
            ".input {$count :integer}\n.match $count\n* {{x}}".into()
        )]
    );
}

#[test]
fn an_empty_first_line_contributes_nothing() {
    // README of bench/workload-gen: the value starts with the first
    // continuation line, with no leading LF.
    let r = ok("key =\n  one\n  two\n");
    assert_eq!(entries(&r), [("key".into(), "one\ntwo".into())]);
}

#[test]
fn a_non_empty_first_line_is_joined_by_one_lf() {
    let r = ok("key = one\n  two\n");
    assert_eq!(entries(&r), [("key".into(), "one\ntwo".into())]);
}

#[test]
fn a_blank_line_ends_the_value() {
    let r = ok("key = one\n\n  \nother = two\n");
    assert_eq!(
        entries(&r),
        [("key".into(), "one".into()), ("other".into(), "two".into())]
    );
}

#[test]
fn an_escaped_line_break_removes_the_break_and_the_indent() {
    let r = ok("key = a long value \\\n  continued here\n");
    assert_eq!(
        entries(&r),
        [("key".into(), "a long value continued here".into())]
    );
}

#[test]
fn an_escaped_line_break_with_nothing_after_it_is_reported() {
    assert_eq!(codes("key = a \\\n"), [code::DANGLING_LINE_BREAK]);
    assert_eq!(
        codes("key = a \\\nother = b\n"),
        [code::DANGLING_LINE_BREAK]
    );
}

#[test]
fn an_indented_line_that_continues_nothing_is_reported() {
    assert_eq!(codes("  stray\n"), [code::STRAY_CONTINUATION]);
}

#[test]
fn crlf_line_endings_leave_no_cr_in_the_value() {
    let r = ok("@locale en\r\n---\r\n\r\nkey = one\r\n  two\r\n");
    assert_eq!(entries(&r), [("key".into(), "one\ntwo".into())]);
}

// ──────────────────────────────── escapes ───────────────────────────────

#[test]
fn mf2_escapes_pass_through_untouched() {
    let r = ok("key = a \\{ b \\} c \\\\ d \\|\n");
    assert_eq!(
        entries(&r),
        [("key".into(), "a \\{ b \\} c \\\\ d \\|".into())]
    );
    // Nothing had to be cooked, so the value still borrows the source.
    assert!(matches!(r.sections[0].entries[0].value, Cow::Borrowed(_)));
}

#[test]
fn container_escapes_become_their_characters() {
    let r = ok("key = a\\nb\\tc\\rd\\x41e\\u00E9f\\U01F600g\n");
    assert_eq!(
        entries(&r),
        [("key".into(), "a\nb\tc\rdAe\u{e9}f\u{1F600}g".into())]
    );
}

#[test]
fn an_escaped_space_keeps_leading_whitespace() {
    let r = ok("key = \\ leading\nother =\n  \\\tindented\n");
    assert_eq!(
        entries(&r),
        [
            ("key".into(), " leading".into()),
            ("other".into(), "\tindented".into()),
        ]
    );
}

#[test]
fn an_unknown_escape_is_reported_and_the_backslash_kept() {
    let (r, diags) = parse("key = a\\qb\n");
    assert_eq!(
        diags.iter().map(|d| d.code).collect::<Vec<_>>(),
        [code::INVALID_ESCAPE]
    );
    assert_eq!(entries(&r), [("key".into(), "a\\qb".into())]);
}

#[test]
fn a_bad_hex_escape_is_reported() {
    assert_eq!(codes("key = \\xZZ\n"), [code::BAD_HEX_ESCAPE]);
    assert_eq!(codes("key = \\u12\n"), [code::BAD_HEX_ESCAPE]);
    // A surrogate is not a Unicode scalar value.
    assert_eq!(codes("key = \\uD800\n"), [code::BAD_HEX_ESCAPE]);
}

#[test]
fn raw_control_characters_and_line_separators_are_reported() {
    assert_eq!(codes("key = a\u{7}b\n"), [code::RAW_CONTROL]);
    assert_eq!(codes("key = a\u{2028}b\n"), [code::RAW_LINE_SEPARATOR]);
    assert_eq!(codes("key = a\u{2029}b\n"), [code::RAW_LINE_SEPARATOR]);
}

// ──────────────────────────────── ids ───────────────────────────────────

#[test]
fn an_id_is_parts_joined_by_dots() {
    let r = ok("chat.input.send = Send\n");
    let entry = &r.sections[0].entries[0];
    assert_eq!(entry.id.parts(), ["chat", "input", "send"]);
    assert_eq!(entry.id.to_string(), "chat.input.send");
}

#[test]
fn a_symbol_in_an_id_part_must_be_escaped() {
    let r = ok("odd\\.name = x\n");
    assert_eq!(r.sections[0].entries[0].id.parts(), ["odd.name"]);
    assert_eq!(r.sections[0].entries[0].id.to_string(), "odd\\.name");
    assert_eq!(codes("odd:name = x\n"), [code::UNESCAPED_ID_CHAR]);
}

#[test]
fn an_empty_id_part_is_reported() {
    assert_eq!(codes("a..b = x\n"), [code::EMPTY_ID_PART]);
    assert_eq!(codes(".a = x\n"), [code::EMPTY_ID_PART]);
    assert_eq!(codes(" = x\n"), [code::STRAY_CONTINUATION]);
    assert_eq!(codes("= x\n"), [code::EMPTY_ID]);
}

#[test]
fn an_id_may_not_look_like_the_frontmatter_separator() {
    assert_eq!(codes("--- = x\n"), [code::ID_LIKE_FRONTMATTER]);
    // Escaping its first character keeps it out of the way.
    let r = ok("\\--- = x\n");
    assert_eq!(r.sections[0].entries[0].id.parts(), ["---"]);
}

#[test]
fn non_ascii_name_characters_need_no_escape() {
    let r = ok("Ünïcode.日本語 = x\n");
    assert_eq!(r.sections[0].entries[0].id.parts(), ["Ünïcode", "日本語"]);
}

// ─────────────────────────────── sections ───────────────────────────────

#[test]
fn a_section_prefixes_the_ids_that_follow() {
    let r = ok("bare = x\n\n[hotkeys]\nrelease = Esc\n\n[chat.prompts]\nrow = y\n");
    assert_eq!(
        entries(&r),
        [
            ("bare".into(), "x".into()),
            ("hotkeys.release".into(), "Esc".into()),
            ("chat.prompts.row".into(), "y".into()),
        ]
    );
    assert_eq!(r.sections.len(), 3);
    assert!(r.sections[0].head.is_none());
}

#[test]
fn sections_do_not_nest() {
    // The second head states its full path; it does not continue the first.
    let r = ok("[a]\nx = 1\n[b]\ny = 2\n");
    assert_eq!(
        entries(&r),
        [("a.x".into(), "1".into()), ("b.y".into(), "2".into())]
    );
}

#[test]
fn a_section_head_must_be_closed_and_alone() {
    assert_eq!(codes("[chat\nx = 1\n"), [code::UNTERMINATED_SECTION]);
    assert_eq!(codes("[chat] oops\n"), [code::TRAILING_AFTER_SECTION]);
}

// ────────────────────────── comments and properties ─────────────────────

#[test]
fn adjacent_comment_lines_are_one_comment() {
    let r = ok("# one\n# two\nkey = x\n");
    assert_eq!(
        r.sections[0].entries[0]
            .comment
            .as_ref()
            .map(|c| c.text.as_ref()),
        Some("one\ntwo")
    );
}

#[test]
fn one_space_after_the_hash_is_not_part_of_the_comment() {
    let r = ok("#no space\n#  two spaces\n#\nkey = x\n");
    assert_eq!(
        r.sections[0].entries[0]
            .comment
            .as_ref()
            .map(|c| c.text.as_ref()),
        Some("no space\n two spaces\n")
    );
}

#[test]
fn an_empty_line_detaches_a_comment() {
    let r = ok("# floating\n\nkey = x\n");
    assert!(r.sections[0].entries[0].comment.is_none());
    assert_eq!(r.sections[0].detached.len(), 1);
    assert_eq!(r.sections[0].detached[0].before, 0);
    assert_eq!(r.sections[0].detached[0].comment.text, "floating");
}

#[test]
fn properties_may_sit_between_a_comment_and_its_entry() {
    let r = ok("# why\n@param $count - How many.\n@do-not-translate\nkey = x\n");
    let entry = &r.sections[0].entries[0];
    assert_eq!(entry.comment.as_ref().map(|c| c.text.as_ref()), Some("why"));
    assert_eq!(entry.meta.len(), 2);
    assert_eq!(entry.meta[0].name, "param");
    assert_eq!(entry.meta[0].value.as_deref(), Some("$count - How many."));
    assert_eq!(entry.meta[1].name, "do-not-translate");
    assert_eq!(entry.meta[1].value, None);
}

#[test]
fn an_empty_line_cuts_a_property_off() {
    assert_eq!(
        codes("@param $x - a\n\nkey = 1\n"),
        [code::DETACHED_PROPERTY]
    );
}

#[test]
fn a_property_attaches_to_a_section_head() {
    let r = ok("@do-not-translate\n[brand]\nname = Example\n");
    let head = r.sections[0].head.as_ref().expect("a head");
    assert_eq!(head.meta[0].name, "do-not-translate");
}

#[test]
fn a_property_value_continues_like_an_entry() {
    let r = ok("@param $count - How many people are\n  in the room.\nkey = x\n");
    assert_eq!(
        r.sections[0].entries[0].meta[0].value.as_deref(),
        Some("$count - How many people are\nin the room.")
    );
}

#[test]
fn a_property_needs_a_name() {
    assert_eq!(codes("@ oops\nkey = 1\n"), [code::EXPECTED_PROPERTY_NAME]);
}

// ─────────────────────────── the documented example ─────────────────────

/// The grammar's worked example file, whole.
#[test]
fn the_plan_example_reads_as_documented() {
    let src = "\
# The resource-level locale is the only required property.
@locale en-US
---

chat-send = Send

@param $count - How many people are in the room; a whole number.
users-online =
  .input {$count :integer}
  .match $count
  one {{{$count} user online}}
  *   {{{$count} users online}}

[hotkeys]
# \u{2192} id \"hotkeys.release\"
release = Release {#kbd}?{/kbd} to close

@do-not-translate
[brand]
name = Example
";
    let r = ok(src);
    assert_eq!(r.locale(), Some("en-US"));
    assert_eq!(
        entries(&r),
        [
            ("chat-send".into(), "Send".into()),
            (
                "users-online".into(),
                ".input {$count :integer}\n.match $count\none {{{$count} user online}}\n*   {{{$count} users online}}".into()
            ),
            ("hotkeys.release".into(), "Release {#kbd}?{/kbd} to close".into()),
            ("brand.name".into(), "Example".into()),
        ]
    );
    let brand = r.sections.last().expect("a section");
    assert_eq!(
        brand.head.as_ref().expect("a head").meta[0].name,
        "do-not-translate"
    );
    // And it reads back as itself.
    let written = serialize(&r).expect("writable");
    assert_eq!(ok(&written), r);
}

// ─────────────────────────── the canonical layout ───────────────────────

/// `mf2 fmt`'s blank lines (owner, 2026-09-27):
/// one after the frontmatter's `---`, one before a section head and before
/// a commented entry, and one on each side of a message whose value starts
/// on its own line. Blank lines anywhere else come out; an entry that only
/// carries properties follows straight on.
#[test]
fn the_canonical_layout_sets_off_the_frontmatter_and_block_messages() {
    let messy = "\
@locale en
---
a = A


b = B
multi =
  .input {$n :integer}
  .match $n
  one {{one}}
  * {{other}}
c = C
@param $n - A count.
d = D {$n}
other =
  line one
  line two
# A comment.
e = E

[s]
f = F
g =
  x
  y
";
    let canonical = "\
@locale en
---

a = A
b = B

multi =
  .input {$n :integer}
  .match $n
  one {{one}}
  * {{other}}

c = C
@param $n - A count.
d = D {$n}

other =
  line one
  line two

# A comment.
e = E

[s]
f = F

g =
  x
  y
";
    let r = ok(messy);
    let written = serialize(&r).expect("writable");
    assert_eq!(written, canonical);
    // The same model, and a fixed point: formatting the output changes
    // nothing.
    assert_eq!(ok(&written), r);
    assert_eq!(serialize(&ok(&written)).expect("writable"), written);

    // A block right after the frontmatter gets one blank line before it,
    // not two; right after a head it is set off too, as a commented entry
    // there is.
    let first = "@locale en\n---\nm =\n  a\n  b\nn = N\n[s]\no =\n  a\n  b\n";
    let written = serialize(&ok(first)).expect("writable");
    assert_eq!(
        written,
        "@locale en\n---\n\nm =\n  a\n  b\n\nn = N\n\n[s]\n\no =\n  a\n  b\n"
    );
    assert_eq!(serialize(&ok(&written)).expect("writable"), written);

    // Without frontmatter nothing is set off at the top.
    let bare = serialize(&ok("m =\n  a\n  b\n")).expect("writable");
    assert_eq!(bare, "m =\n  a\n  b\n");
}

// ───────────────────────────── the value map ────────────────────────────

#[test]
fn a_cooked_offset_maps_back_to_the_file() {
    let src = "key =\n  one \\{x\\}\n  two\n";
    let r = ok(src);
    let entry = &r.sections[0].entries[0];
    assert_eq!(entry.value, "one \\{x\\}\ntwo");
    // Every cooked offset lands on the byte the file wrote it at.
    for (i, _) in entry.value.char_indices() {
        let at = entry
            .map
            .source_offset(u32::try_from(i).expect("a short value"))
            .expect("mapped");
        assert!(
            src.is_char_boundary(at as usize),
            "offset {i} mapped into the middle of a character"
        );
    }
    let two = u32::try_from(entry.value.find("two").expect("found")).expect("a short value");
    assert_eq!(
        &src[entry.map.source_offset(two).expect("mapped") as usize..][..3],
        "two"
    );
}

#[test]
fn an_escaped_value_maps_through_its_escapes() {
    let src = "key = a\\u0062c\n";
    let r = ok(src);
    let entry = &r.sections[0].entries[0];
    assert_eq!(entry.value, "abc");
    let at = |i: u32| entry.map.source_offset(i).expect("mapped") as usize;
    assert_eq!(&src[at(0)..=at(0)], "a");
    assert_eq!(&src[at(1)..at(1) + 2], "\\u");
    assert_eq!(&src[at(2)..=at(2)], "c");
}

/// The blanks before `=` are layout and come off; an **escaped** one is part
/// of the id and stays. Trimming it read the id back a character short and
/// wrote a line that no longer parsed — found by the `resource` fuzz target
/// on `#h\n#h\n\ \ =7` (Phase 5a, A12).
#[test]
fn an_escaped_blank_before_the_equals_belongs_to_the_id() {
    for (src, id) in [
        ("\\ \\ =7\n", "  "),
        ("\\ =7\n", " "),
        ("a\\ =7\n", "a "),
        // In an id `\X` is literally X — there is no `\t`-means-tab rule
        // there, because an id cannot hold a control character at all.
        ("a\\t=7\n", "at"),
        // A real blank still comes off, and so does one after an escaped one.
        ("a   =7\n", "a"),
        ("a\\   =7\n", "a "),
        // `\\` is an escaped backslash, so the blank after it is layout.
        ("a\\\\ =7\n", "a\\"),
    ] {
        let r = ok(src);
        let entry = &r.sections[0].entries[0];
        assert_eq!(
            entry.id.parts(),
            &[id],
            "id of {src:?} (got {:?})",
            entry.id.parts()
        );
        assert_eq!(entry.value, "7", "value of {src:?}");
        // And what it writes back reads the same — the round trip the fuzz
        // target checks.
        let text = mf2_resource::serialize(&r).expect("writes back");
        let (again, diags) = mf2_resource::parse(&text);
        assert!(
            diags.is_empty(),
            "{src:?} wrote {text:?}, which does not parse"
        );
        assert_eq!(again, r, "{src:?} wrote {text:?}");
    }
}

/// A parse that reports nothing must produce a resource the serializer can
/// write: "clean parse ⇒ writes back" is what the `resource` fuzz target
/// asserts. An escape passes its character through as itself, so `\` cannot
/// smuggle an unwritable one into an id — the serializer refuses U+2028 and
/// U+2029 there just as it refuses a control (Phase 5a, A12).
#[test]
fn an_escape_does_not_make_an_unwritable_character_writable_in_an_id() {
    // Not `\n`: a raw line break ends the line before any escape applies, so
    // it can never reach an id in the first place.
    for bad in ['\u{2028}', '\u{2029}', '\u{0007}'] {
        let src = format!("a\\{bad}b = 1\n");
        let (resource, diags) = mf2_resource::parse(&src);
        assert!(
            !diags.is_empty(),
            "an id holding an escaped U+{:04X} parsed clean",
            bad as u32
        );
        // And the two agree: what the parser refuses, the writer refuses.
        assert!(
            mf2_resource::serialize(&resource).is_err() || resource.sections[0].entries.is_empty(),
            "U+{:04X}: the writer accepted what the parser rejected",
            bad as u32
        );
    }
}
