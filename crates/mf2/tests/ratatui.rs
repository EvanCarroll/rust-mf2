//! A message as Ratatui text, its markup as styles (`mf2::ratatui`): the
//! conversions through the native store, drawn on a `Buffer`, and what they
//! allocate; then 1.x's `line` / `text` (1.x's `mf2-ratatui`, whose tests
//! those were). The binary counts what each thread allocates.

#![allow(unsafe_code, reason = "a counting GlobalAlloc that forwards to System")]

#[path = "support/corpus.rs"]
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::borrow::Cow;
use std::cell::Cell;
use std::sync::OnceLock;

use mf2::native::{self, NativeI18n};
use mf2::ratatui::{Markup, MarkupStyles, Theme, line, set_theme, text, theme, with_theme};
use mf2::{
    BidiStrategy, CatalogFile, Compiled, Corpus, Dir, ErrorSink, Formatter, Message, MsgId,
    PartSink, Registry, Sink, TrDyn, markup_key, tr_rich,
};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier, Style, Stylize};
use ratatui_core::text::{Line, Span, Text};
use ratatui_core::widgets::Widget;

struct Counting;

std::thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
}

// SAFETY: each method forwards to `System` unchanged, with the caller's
// arguments, after counting; `GlobalAlloc`'s contract passes through.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged (see the impl).
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged (see the impl).
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged (see the impl).
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged (see the impl).
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// What `body` returns, and how many allocations this thread made in it.
fn allocations<R>(body: impl FnOnce() -> R) -> (R, usize) {
    let before = ALLOCATIONS.with(Cell::get);
    let out = body();
    (out, ALLOCATIONS.with(Cell::get) - before)
}

/// Three sets of the corpus's three messages (`welcome`, `hello` with
/// `name`, `files` with `n`), one per language, which a test picks with
/// [`in_set`].
const SETS: [(&str, Dir, [&str; 3]); 3] = [
    (
        "en",
        Dir::Ltr,
        ["Welcome", "Hello, {#b}{$name}{/b}!", "{$n :integer} files"],
    ),
    (
        "fr",
        Dir::Ltr,
        [
            "{#ok}all {#b}good{/b}{/ok} now",
            "{#em}a{/em}{#br/}b{/ok}c {$name}",
            "one\n{#b}two{/b}\nthree {$n :integer}",
        ],
    ),
    (
        "de",
        Dir::Ltr,
        ["Welcome", "Hello {$name :integer}!", "{$n :integer}"],
    ),
];

fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<&'static Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| support::corpus_of(&SETS, true).0)
}

/// `body`, formatting the set `tag` names on this thread.
fn in_set<R>(tag: &str, body: impl FnOnce() -> R) -> R {
    native::with_locale(corpus(), tag, body).expect("the set is the corpus's")
}

/// What the generated `markup::*` holds for `name`.
fn markup(name: &'static str) -> Markup {
    Markup::new(markup_key(name), name)
}

fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

#[test]
fn constant_text_is_borrowed_and_allocates_only_the_containers() {
    in_set("en", || {
        let _ = Text::from(support::welcome()); // this thread's first format
        let (span, n) = allocations(|| Span::from(support::welcome()));
        assert_eq!(span, Span::raw("Welcome"));
        assert!(matches!(span.content, Cow::Borrowed(_)));
        assert_eq!(n, 0, "a Span");
        let (line, n) = allocations(|| Line::from(support::welcome()));
        assert_eq!(line, Line::from("Welcome"));
        assert_eq!(n, 1, "a Line: its Vec");
        let (text, n) = allocations(|| Text::from(support::welcome()));
        assert_eq!(text, Text::from("Welcome"));
        assert_eq!(n, 2, "a Text: its Vec of lines and the line's");
    });
}

#[test]
fn a_placeholder_is_one_string_and_pattern_text_is_borrowed() {
    in_set("en", || {
        with_theme(&Theme::default(), || {
            let hello = support::hello("Ada");
            let _ = (Line::from(&hello), Span::from(&hello)); // the first format
            let (line, n) = allocations(|| Line::from(&hello));
            assert_eq!(
                line,
                Line::from(vec![
                    Span::raw("Hello, "),
                    Span::styled("Ada", bold()),
                    Span::raw("!")
                ])
            );
            let borrowed: Vec<bool> = line
                .spans
                .iter()
                .map(|span| matches!(span.content, Cow::Borrowed(_)))
                .collect();
            assert_eq!(borrowed, [true, false, true]);
            assert_eq!(n, 2, "a Line: its Vec and the placeholder's String");
            let (span, n) = allocations(|| Span::from(&hello));
            assert_eq!(span, Span::raw("Hello, Ada!"));
            assert_eq!(n, 1, "a Span: its String");
        });
    });
}

#[test]
fn markup_takes_the_themes_styles() {
    let green = Style::new().fg(Color::Green);
    let theme = Theme::empty()
        .style(markup("ok"), green)
        .style(markup("b"), bold());
    in_set("fr", || {
        with_theme(&theme, || {
            // Nested elements patch their styles in order.
            assert_eq!(
                Line::from(support::welcome()),
                Line::from(vec![
                    Span::styled("all ", green),
                    Span::styled("good", green.patch(bold())),
                    Span::raw(" now"),
                ])
            );
            // A name the theme lacks, standalone markup and a close with no
            // open change nothing.
            assert_eq!(
                Line::from(support::hello("Ada")),
                Line::from(vec![
                    Span::raw("a"),
                    Span::raw("b"),
                    Span::raw("c "),
                    Span::raw("Ada")
                ])
            );
        });
    });
}

#[test]
fn the_default_theme_styles_the_common_names() {
    let names = [
        ("b", Modifier::BOLD),
        ("strong", Modifier::BOLD),
        ("i", Modifier::ITALIC),
        ("em", Modifier::ITALIC),
        ("u", Modifier::UNDERLINED),
        ("s", Modifier::CROSSED_OUT),
        ("del", Modifier::CROSSED_OUT),
        ("code", Modifier::REVERSED),
        ("kbd", Modifier::REVERSED),
    ];
    let expected = names
        .iter()
        .fold(Theme::empty(), |theme, (name, modifier)| {
            theme.style(markup(name), Style::new().add_modifier(*modifier))
        });
    assert_eq!(Theme::default(), expected);
}

#[test]
fn the_app_wide_theme_and_a_scoped_one() {
    // No other test sets the app-wide theme.
    assert_eq!(theme(), Theme::default());
    let red = Style::new().fg(Color::Red);
    let custom = Theme::default().style(markup("b"), red);
    set_theme(custom.clone());
    assert_eq!(theme(), custom);
    // Every thread's next conversion uses it.
    let drawn = std::thread::spawn(|| in_set("en", || Line::from(support::hello("Ada"))))
        .join()
        .expect("the thread draws");
    assert_eq!(drawn.spans.get(1), Some(&Span::styled("Ada", red)));
    // A scope's theme wins on its thread, until the scope ends.
    let plain = in_set("en", || {
        with_theme(&Theme::empty(), || {
            assert_eq!(theme(), Theme::empty());
            Line::from(support::hello("Ada"))
        })
    });
    assert_eq!(plain.spans.get(1), Some(&Span::raw("Ada")));
    assert_eq!(theme(), custom);
    set_theme(Theme::default());
}

#[test]
fn text_breaks_lines_and_a_line_or_a_span_joins_them() {
    in_set("fr", || {
        with_theme(&Theme::default(), || {
            let files = support::files(5);
            assert_eq!(
                Text::from(&files).lines,
                [
                    Line::from("one"),
                    Line::from(vec![Span::styled("two", bold())]),
                    Line::from(vec![Span::raw("three "), Span::raw("5")]),
                ]
            );
            assert_eq!(
                Line::from(&files),
                Line::from(vec![
                    Span::raw("one"),
                    Span::raw(" "),
                    Span::styled("two", bold()),
                    Span::raw(" "),
                    Span::raw("three "),
                    Span::raw("5"),
                ])
            );
            // A span has one style: the markup is flattened.
            assert_eq!(Span::from(&files), Span::raw("one two three 5"));
        });
    });
}

#[test]
fn a_description_draws_as_its_text() {
    in_set("fr", || {
        with_theme(&Theme::default(), || {
            let mut drawn = Buffer::empty(Rect::new(0, 0, 8, 3));
            support::files(5).render(drawn.area, &mut drawn);
            let mut expected = Buffer::with_lines(["one     ", "two     ", "three 5 "]);
            expected.set_style(Rect::new(0, 1, 3, 1), bold());
            assert_eq!(drawn, expected);
        });
    });
}

#[test]
fn stylize_keeps_the_messages_own_styles_over_the_calls() {
    in_set("en", || {
        with_theme(&Theme::default(), || {
            let call = Style::new().italic().fg(Color::Yellow);
            let line: Line<'static> = support::hello("Ada").italic().fg(Color::Yellow);
            assert_eq!(line.style, call);
            assert_eq!(
                line.spans,
                [
                    Span::raw("Hello, "),
                    Span::styled("Ada", bold()),
                    Span::raw("!")
                ]
            );
            let mut drawn = Buffer::empty(Rect::new(0, 0, 12, 1));
            line.render(drawn.area, &mut drawn);
            let mut expected = Buffer::with_lines(["Hello, Ada! "]);
            expected.set_style(expected.area, call);
            expected.set_style(Rect::new(7, 0, 3, 1), bold());
            assert_eq!(drawn, expected);
            let quit: Line<'static> = support::welcome().bold();
            assert_eq!(quit, Line::from("Welcome").style(bold()));
        });
    });
}

#[test]
fn every_description_converts_by_value_and_by_reference() {
    in_set("en", || {
        with_theme(&Theme::default(), || {
            let args = support::hello("Ada");
            let expected = Line::from(&args);
            assert_eq!(Line::from(args.clone()), expected);
            let dynamic = TrDyn::new(MsgId::from_raw(1), [("name", "Ada")]);
            assert_eq!(Line::from(&dynamic), expected);
            assert_eq!(Line::from(dynamic), expected);
            let rich = tr_rich(args.clone(), Vec::new().into());
            assert_eq!(Line::from(&rich), expected);
            assert_eq!(Text::from(rich), Text::from(expected.clone()));
            assert_eq!(Text::from(&args), Text::from(expected));
            assert_eq!(Span::from(args), Span::raw("Hello, Ada!"));
            assert_eq!(Span::from(&support::welcome()), Span::raw("Welcome"));
        });
    });
}

#[test]
fn collecting_descriptions_into_a_line_flattens_their_markup() {
    in_set("en", || {
        with_theme(&Theme::default(), || {
            let collected: Line<'static> = [support::hello("Ada"), support::hello("Bo")]
                .into_iter()
                .collect();
            assert_eq!(
                collected,
                Line::from(vec![Span::raw("Hello, Ada!"), Span::raw("Hello, Bo!")])
            );
            // Extending a line with each message's spans keeps them.
            let mut kept = Line::default();
            for hello in [support::hello("Ada"), support::hello("Bo")] {
                kept.spans.extend(Line::from(hello).spans);
            }
            assert_eq!(kept.spans.get(4), Some(&Span::styled("Bo", bold())));
        });
    });
}

#[test]
fn a_fallback_shows_its_source_and_isolation_never_reaches_the_terminal() {
    in_set("de", || {
        // `:integer` of "Ada" fails: the placeholder falls back.
        assert_eq!(
            Line::from(support::hello("Ada")),
            Line::from(vec![
                Span::raw("Hello "),
                Span::raw("{$name}"),
                Span::raw("!")
            ])
        );
        assert_eq!(
            Span::from(support::hello("Ada")),
            Span::raw("Hello {$name}!")
        );
        native::set_bidi(BidiStrategy::Default);
        let line = Line::from(support::files(5));
        native::set_bidi(BidiStrategy::None);
        assert_eq!(line, Line::from("5"));
    });
}

// ---------------------------------------------------------------- 1.x's names

static REGISTRY: Registry = Registry::EMPTY;

/// The one message of a `compile_str` catalog.
struct Only;

impl Message for Only {
    fn write(&self, f: &Formatter<'_>, out: &mut dyn Sink, errs: &mut dyn ErrorSink) {
        f.write(Compiled::ID, &[], out, errs);
    }

    fn parts(&self, f: &Formatter<'_>, out: &mut dyn PartSink, errs: &mut dyn ErrorSink) {
        f.parts(Compiled::ID, &[], out, errs);
    }
}

fn i18n(source: &str) -> NativeI18n {
    let compiled = mf2::compile_str(source, "en").expect("the test message compiles");
    let bytes: &'static [u8] = Box::leak(compiled.catalog.as_bytes().to_vec().into());
    let files: &'static [CatalogFile] =
        Box::leak(Box::new([CatalogFile::new("en", "en.mf2b", Some(bytes))]));
    let corpus = Box::leak(Box::new(Corpus::new(
        "en",
        compiled.catalog.manifest_hash(),
        &[("en", Dir::Ltr)],
        &REGISTRY,
        files,
    )));
    NativeI18n::embedded(corpus).expect("the catalog loads")
}

fn styles() -> MarkupStyles {
    MarkupStyles::new()
        .with("ok", Style::new().fg(Color::Green))
        .with("b", Style::new().add_modifier(Modifier::BOLD))
        .with("host", Style::new().add_modifier(Modifier::UNDERLINED))
}

#[test]
fn a_message_without_markup_is_one_plain_span() {
    let i18n = i18n("Network status");
    assert_eq!(
        line(&i18n, &Only, &styles()),
        Line::from(vec![Span::raw("Network status")])
    );
}

#[test]
fn markup_styles_its_stretch_and_its_placeholders() {
    let i18n =
        i18n(".local $host = {|example.org|} {{{#ok}Connected{/ok} to {#host}{$host}{/host}.}}");
    assert_eq!(
        line(&i18n, &Only, &styles()),
        Line::from(vec![
            Span::styled("Connected", Style::new().fg(Color::Green)),
            Span::raw(" to "),
            Span::styled(
                "example.org",
                Style::new().add_modifier(Modifier::UNDERLINED)
            ),
            Span::raw("."),
        ])
    );
}

#[test]
fn nested_markup_combines_styles() {
    let i18n = i18n("{#ok}all {#b}good{/b}{/ok} now");
    assert_eq!(
        line(&i18n, &Only, &styles()),
        Line::from(vec![
            Span::styled("all ", Style::new().fg(Color::Green)),
            Span::styled(
                "good",
                Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)
            ),
            Span::raw(" now"),
        ])
    );
}

#[test]
fn unknown_standalone_and_unmatched_markup_change_nothing() {
    let i18n = i18n("{#em}a{/em}{#br/}b{/ok}c");
    assert_eq!(
        line(&i18n, &Only, &styles()),
        Line::from(vec![Span::raw("abc")])
    );
}

#[test]
fn line_breaks_start_lines_in_text_and_are_spaces_in_a_line() {
    let i18n = i18n("{#ok}one\ntwo{/ok}");
    let green = Style::new().fg(Color::Green);
    let lines = text(&i18n, &Only, &styles()).lines;
    assert_eq!(
        lines,
        [
            Line::from(vec![Span::styled("one", green)]),
            Line::from(vec![Span::styled("two", green)]),
        ]
    );
    assert_eq!(
        line(&i18n, &Only, &styles()),
        Line::from(vec![Span::styled("one two", green)])
    );
}

#[test]
fn a_fallback_shows_its_source() {
    let i18n = i18n("Hello {$missing}!");
    assert_eq!(
        line(&i18n, &Only, &styles()),
        Line::from(vec![Span::raw("Hello {$missing}!")])
    );
}

#[test]
fn bidi_isolation_never_reaches_the_terminal() {
    let mut i18n = i18n(".local $x = {|Bob|} {{Hi {$x}}}");
    i18n.set_bidi(BidiStrategy::Default);
    assert_eq!(i18n.format(&Only), "Hi \u{2068}Bob\u{2069}");
    assert_eq!(
        line(&i18n, &Only, &styles()),
        Line::from(vec![Span::raw("Hi Bob")])
    );
}
