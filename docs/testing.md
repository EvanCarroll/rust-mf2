# Testing

A test pins a language for the code it runs, checks the text in each
language the application ships, and draws a terminal UI into a buffer to
compare with what the reader should see. Pseudo-locales then show whether
the layout has room for a longer language. This page adds tests to `trace`,
the library and terminal UI from
[Native CLI and Ratatui apps](native-apps.md#a-library-and-its-terminal-ui);
the docs build runs every one of them.

## One language for one test: `with_locale`

The generated module has `with_locale(Locale, body)`: it runs `body` with
this thread formatting in that language, and puts the thread's language
back when `body` returns or panics. It needs no `install()`: the embedded
catalogs are loaded on first use. Other threads are not affected, so tests
pinned to different languages run in parallel, as `cargo test` runs them.
`set_locale` is the wrong tool in a test: it changes the language of every
thread, and the tests running beside it.

The library's tests go in `core/tests/`:

```rust file=testing/core/tests/messages.rs
//! The library's messages, as text, in each language it ships.

use trace_core::{Error, Locale, Trace, with_locale};

fn trace() -> Trace {
    Trace {
        target: String::from("example.org"),
        sent: 1204,
        lost: 25,
    }
}

#[test]
fn the_summary_in_english_and_in_french() {
    let trace = trace();
    let english = with_locale(Locale::En, || trace.summary().to_string());
    assert_eq!(english, "1,204 probes sent · loss 2.1%");
    let french = with_locale(Locale::Fr, || trace.summary().to_string());
    assert_eq!(french, "1\u{202f}204 sondes envoyées · perte 2,1\u{a0}%");
}

#[test]
fn one_probe_is_singular() {
    let trace = Trace {
        sent: 1,
        lost: 0,
        ..trace()
    };
    let english = with_locale(Locale::En, || trace.summary().to_string());
    assert_eq!(english, "1 probe sent · loss 0%");
}

#[test]
fn an_error_prints_in_the_chosen_language() {
    let error = Error::Resolve {
        host: String::from("example.invalid"),
    };
    let french = with_locale(Locale::Fr, || error.to_string());
    assert_eq!(french, "Impossible de résoudre example.invalid.");
}

#[test]
fn locale_format_is_one_message_in_one_language() {
    assert_eq!(
        Locale::Fr.format(&trace().title()),
        "Trace vers example.org"
    );
}
```

* **Compare text, not descriptions.** `tr!` returns a description of the
  message, which is formatted when it is shown; `.to_string()` inside
  `with_locale` formats it in the test's language. A description built
  inside `with_locale` and formatted after it is in the thread's own
  language again.
* **`Locale::format`** formats one message in one language, with no
  closure: the shortest way to check a single message.
* French numbers group with a narrow no-break space (`\u{202f}`) and put a
  no-break space (`\u{a0}`) before `%`, as CLDR says French does. Write
  them as escapes, so that the expected text shows what is in it.
* Asserting on a description itself, or calling `unwrap()` on a result
  that holds one, reaches its `Debug`. That costs nothing in a native test;
  in code a browser build ships it costs about 1 KB of wasm
  ([Features](features.md#what-a-browser-build-pays-for-text)).

## A frame in several languages: `TestBackend`

Ratatui's `TestBackend` draws into a buffer instead of a terminal, and
`assert_buffer_lines` compares the buffer with the lines the reader should
see: a snapshot of the frame, in each language. The terminal UI's tests
draw what its `main` draws; an application with a larger frame puts its
drawing in a function that both call, as `hops` does with `ui::draw`.

```rust file=testing/tui/tests/frame.rs
//! The terminal UI's frame, drawn into a buffer in each language.

use mf2::ratatui::{Theme, with_theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use trace_core::{Locale, Trace, with_locale};

fn trace() -> Trace {
    Trace {
        target: String::from("example.org"),
        sent: 1204,
        lost: 25,
    }
}

/// The frame `main` draws, in `locale`, on a terminal `width` columns wide.
fn draw(locale: Locale, width: u16) -> Terminal<TestBackend> {
    let trace = trace();
    let mut terminal = Terminal::new(TestBackend::new(width, 3)).expect("a test terminal");
    with_locale(locale, || {
        terminal.draw(|frame| {
            let block = Block::bordered().title(trace.title());
            frame.render_widget(Paragraph::new(trace.summary()).block(block), frame.area());
        })
    })
    .expect("a frame");
    terminal
}

#[test]
fn the_frame_in_english() {
    draw(Locale::En, 40).backend().assert_buffer_lines([
        "┌Trace to example.org──────────────────┐",
        "│1,204 probes sent · loss 2.1%         │",
        "└──────────────────────────────────────┘",
    ]);
}

#[test]
fn the_frame_in_french() {
    draw(Locale::Fr, 40).backend().assert_buffer_lines([
        "┌Trace vers example.org────────────────┐",
        "│1\u{202f}204 sondes envoyées · perte 2,1\u{a0}%   │",
        "└──────────────────────────────────────┘",
    ]);
}
```

A test sets no theme, so markup draws with the style around it and the
snapshot is plain text. To test the styles, give the test a theme with
`with_theme`, which holds for its body on this thread only, and look at
the cells:

```rust file=testing/tui/tests/frame.rs
#[test]
fn the_host_is_underlined_by_the_theme() {
    let theme = Theme::default().style(trace_core::markup::HOST, Style::new().underlined());
    let terminal = with_theme(&theme, || draw(Locale::Fr, 40));
    let buffer = terminal.backend().buffer();
    // "Trace vers " is 11 columns after the corner; the host follows.
    assert!(buffer[(12, 0)].modifier.contains(Modifier::UNDERLINED));
    assert!(!buffer[(11, 0)].modifier.contains(Modifier::UNDERLINED));
}
```

## Room for a longer language: pseudo-locales

A translation is often longer than the source, and a terminal does not
reflow. [Pseudo-locales](translating.md#pseudo-locales-room-for-longer-text)
make one before any translator starts: `en-XA` is the source accented, in
brackets and about a third longer, and `ar-XB` runs right to left. The
library's messages are in `core`, so the command runs there:

```sh run=testing output=pseudo.txt
mf2 -C core pseudo
```

```text file=testing/pseudo.txt generated
mf2 pseudo: en-XA — 4 message(s) in 1 file(s)
mf2 pseudo: ar-XB — 4 message(s) in 1 file(s)
```

With them written, `Locale::ALL` has `EnXa` and `ArXb`, and a test that
walks `Locale::ALL` checks them with the rest. This one holds every
language's summary to the width the frame gives it:

```rust file=testing/tui/tests/frame.rs
/// Every language's summary fits inside the border of the narrowest
/// terminal the application supports.
#[test]
fn every_summary_fits_sixty_columns() {
    let trace = trace();
    for locale in Locale::ALL {
        let width = with_locale(locale, || Line::from(trace.summary()).width());
        assert!(width <= 58, "{} needs {width} columns", locale.tag());
    }
}
```

At the snapshots' 40 columns this test would fail: `en-XA`'s summary
needs 39 columns and the border leaves 38, which is what a longer
translation would meet. A failure names the language and the columns it
needs. The pseudo-locales
stay out of the repository ([Translating](translating.md) says why), so CI
writes them before it runs the tests:

```sh
mf2 -C core pseudo
cargo test --workspace
```

Without them the test still runs, over the shipped languages; it names no
pseudo-locale, so it compiles either way.
