use mf2::{
    BidiStrategy, CatalogFile, Compiled, Corpus, Dir, ErrorSink, Formatter, PartSink, Registry,
    Sink,
};
use mf2_native::{Message, NativeI18n};
use mf2_ratatui::{MarkupStyles, line, text};
use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::text::{Line, Span};

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
