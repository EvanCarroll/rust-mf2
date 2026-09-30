# Native CLI and Ratatui apps

The call sites `tr!` builds are plain values: a command-line tool or a
terminal UI formats them as well as a web page does. A native application
names two crates of this library, and no more:

* **`mf2`**, with the `native` feature — or `ratatui`, which implies it —
  and the features of the functions its messages call (`fn-number` here);
* **`mf2-build`**, in its build script, whose whole body is
  `mf2_build::run()`.

The messages sit in `locales/` beside the code that uses them: there is no
separate translation crate, and no `mf2.toml` — its defaults (the source
language is `en`; a message missing from a translation falls back to it) are
what these applications want. Nothing passes a handle, a style map or a
language tag around.

This page builds three applications, and `cargo xtask docs` compiles each:

* [`count`](#a-command-line-tool), a one-file command-line tool;
* [`hops`](#a-terminal-ui), a Ratatui terminal UI with a language menu and a
  live switch;
* [`trace`](#a-library-and-its-terminal-ui), a workspace in which a library
  owns the messages and a terminal UI draws them.

The first two are what `mf2 init --cli` and `mf2 init --tui` write (see
[the command line](command-line.md#init-a-starter)): the files below are
theirs, and the book checks that they match. Write them by hand, or let
`init` write them.

## A command-line tool

`count` counts the files in a directory and says so in the reader's
language. Its manifest names `mf2` with `native` and `fn-number`, and
`mf2-build` for the build script:

```toml file=count/Cargo.toml generated
[package]
name = "count"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
mf2 = { version = "2", features = ["native", "fn-number"] }

[build-dependencies]
mf2-build = "2"
```

The rest of the manifest only makes rebuilding quicker: the build script
compiles the messages again after every edit to them, and built optimized it
does so faster. An application may leave it out.

```toml file=count/Cargo.toml generated
# The build script compiles the messages again after every edit to
# them: built optimized, it does so faster.
[profile.dev.build-override]
opt-level = 2
```

The build script reads `locales/`, checks every message, and generates a
module with the catalogs embedded in the executable:

```rust file=count/build.rs generated
fn main() {
    mf2_build::run();
}
```

One file per language. A plural's variants follow each language's own
rules: French has a `many` form that English has not.

```mf2 file=count/locales/en/main.mf2 generated
@locale en
---

files =
  .input {$count :integer}
  .match $count
  0   {{{$dir} is empty.}}
  one {{{$dir} holds one file.}}
  *   {{{$dir} holds {$count} files.}}

unreadable = Cannot read {$dir}: {$error}
```

```mf2 file=count/locales/fr/main.mf2 generated
@locale fr
---

files =
  .input {$count :integer}
  .match $count
  0    {{{$dir} est vide.}}
  one  {{{$dir} contient {$count} fichier.}}
  many {{{$dir} contient {$count} de fichiers.}}
  *    {{{$dir} contient {$count} fichiers.}}

unreadable = Impossible de lire {$dir} : {$error}
```

`mf2::include_generated!()` brings in what the build script generated:
`tr!`, the `Locale` enum with one variant per language, `install()` and
`set_locale`.

```rust file=count/src/main.rs generated
//! Counts the files in a directory, and says so in the reader's language.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

mf2::include_generated!();
```

`Locale` parses from a string, so clap takes `--lang` as one. The parse
goes through the same language matcher as the system's languages do:
`--lang fr_CA.UTF-8` is French, and `--lang de` is refused with the
languages the application has.

```rust file=count/src/main.rs generated
#[derive(Parser)]
struct Args {
    /// The directory to count.
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// The language to answer in, instead of the system's.
    #[arg(long)]
    lang: Option<Locale>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    install();
    if let Some(lang) = args.lang {
        set_locale(lang);
    }
    match std::fs::read_dir(&args.dir) {
        Ok(entries) => {
            println!("{}", tr!("files", dir = &args.dir, count = entries.count()));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", tr!("unreadable", dir = &args.dir, error = error));
            ExitCode::FAILURE
        }
    }
}
```

* **`install()`** loads the catalogs the executable embeds, once, and
  chooses the first of the system's languages the application has. It
  cannot fail: catalogs from the same build always load. A process holds
  one message set: installing a second corpus panics, and
  `mf2::native::Catalogs` formats another (`catalogs.format("fr", &message)`).
* **`set_locale`** overrides that choice, for every thread.
* **`println!("{}", tr!(…))`** formats the message in the language in
  force. `tr!(…).to_string()` gives the text as a `String`.
* **The arguments** are Rust values: `dir` is a path and `count` a number,
  which `:integer` formats in the reader's way. `error` is an `io::Error`,
  which has no typed form; its text is the argument.

`cargo run -- --lang fr` prints the French.

### Two settings differ from the web

* **Bidi isolation is off.** MF2 can wrap a placeholder whose direction
  could differ from the message's in invisible isolation characters, which
  terminals and logs tend to show as stray characters. An application that
  shows right-to-left text in a terminal that honours them turns them on
  with `mf2::native::set_bidi`.
* **Dates use the system's time zone** — its IANA name when it has one,
  else a zone that follows the system's daylight-saving rules, else UTC.
  `mf2::native::set_time_zone` changes it.

### Catalogs outside the executable

A build script that asks `mf2_build::Build` for `Emit::NativeFiles`, in
place of `run()`, embeds no catalog. The generated
`install_from_directory(dir)` then loads the files, named
`<locale>.<hash>.mf2b`, from a directory the application chooses. Each file
is checked as it loads, its bytes against the hash in its name, so a catalog
from another build is an error rather than wrong text. `mf2 compile --out
DIR`, given the features the application builds with, writes the same files
for a package to ship.

## A terminal UI

`hops` draws a trace in the shape of a network-diagnostic tool: a bordered
table, a language menu, a key-hint bar with styled keys, a status line with
a plural and numbers. `1` and `2` switch the language while it runs. Its
manifest turns on `ratatui` in place of `native`:

```toml file=hops/Cargo.toml generated
[package]
name = "hops"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
mf2 = { version = "2", features = ["ratatui", "fn-number"] }
ratatui = "0.30"

[build-dependencies]
mf2-build = "2"

# The build script compiles the messages again after every edit to
# them: built optimized, it does so faster.
[profile.dev.build-override]
opt-level = 2
```

Its build script is the command-line tool's:

```rust file=hops/build.rs generated
fn main() {
    mf2_build::run();
}
```

The messages mark up what a stretch of text is — a key, a host, a warning —
and say nothing of how it looks. The translation decides where it goes:

```mf2 file=hops/locales/en/main.mf2 generated
@locale en
---

title = Trace to {#host}{$target}{/host}
languages = Language
hints = {#key}1{/key} {#key}2{/key} language · {#key}q{/key} quit

status =
  .input {$hops :integer}
  .match $hops
  one {{{$hops} hop · probes: {$sent :integer} · loss: {$loss :percent maximumFractionDigits=1}}}
  *   {{{$hops} hops · probes: {$sent :integer} · loss: {$loss :percent maximumFractionDigits=1}}}

[column]
hop = #
host = Host
loss = Loss
average = Avg (ms)

[cell]
no-reply = {#warn}no reply{/warn}
loss = {$share :percent maximumFractionDigits=1}
ms = {$ms :number minimumFractionDigits=1 maximumFractionDigits=1}

# Each language's name, in that language: the same in every file.
@do-not-translate
[language]
en = English
fr = Français
```

```mf2 file=hops/locales/fr/main.mf2 generated
@locale fr
---

title = Trace vers {#host}{$target}{/host}
languages = Langue
hints = {#key}1{/key} {#key}2{/key} langue · {#key}q{/key} quitter

status =
  .input {$hops :integer}
  .match $hops
  one  {{{$hops} saut · sondes : {$sent :integer} · perte : {$loss :percent maximumFractionDigits=1}}}
  many {{{$hops} de sauts · sondes : {$sent :integer} · perte : {$loss :percent maximumFractionDigits=1}}}
  *    {{{$hops} sauts · sondes : {$sent :integer} · perte : {$loss :percent maximumFractionDigits=1}}}

[column]
hop = #
host = Hôte
loss = Perte
average = Moy. (ms)

[cell]
no-reply = {#warn}pas de réponse{/warn}
loss = {$share :percent maximumFractionDigits=1}
ms = {$ms :number minimumFractionDigits=1 maximumFractionDigits=1}

@do-not-translate
[language]
en = English
fr = Français
```

The `language` section names each language in itself. When every language
has its `language.<tag>` message, the generated `Locale` has a `name()`
that returns it, for a language menu.

`main` chooses the language as the command-line tool does, then sets the
**theme**: how each markup name is drawn. `markup::…` holds a constant for
every name the messages use, so a misspelt name does not compile.

```rust file=hops/src/main.rs generated
//! A trippy-shaped terminal UI in the reader's language. `1` and `2` switch
//! the language live; `q` quits.

mod ui;

use clap::Parser;
use mf2::ratatui::{Theme, set_theme};
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::style::Style;

mf2::include_generated!();

#[derive(Parser)]
struct Args {
    /// The language to draw in, instead of the system's.
    #[arg(long)]
    lang: Option<Locale>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    install();
    if let Some(lang) = args.lang {
        set_locale(lang);
    }
    // What each markup name looks like: a message says what a stretch is,
    // the theme how it is drawn. `markup::…` has a constant for every name
    // the messages use, so a misspelt name does not compile.
    set_theme(
        Theme::default()
            .style(markup::KEY, Style::new().bold().yellow())
            .style(markup::HOST, Style::new().underlined())
            .style(markup::WARN, Style::new().red()),
    );
    let trace = ui::Trace::sample();
    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| ui::draw(frame, &trace))?;
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // Every thread's next format is in the new language.
                    KeyCode::Char('1') => set_locale(Locale::En),
                    KeyCode::Char('2') => set_locale(Locale::Fr),
                    KeyCode::Char('q') => return Ok(()),
                    _ => {}
                }
            }
        }
    })
}
```

* **`set_theme`** sets the app-wide theme, once; every thread's next draw
  uses it. `Theme::default()` already draws `b` and `strong` bold, `i` and
  `em` italic, `u` underlined, `s` and `del` crossed out, and `code` and
  `kbd` reversed; `.style(…)` adds or replaces one name's style.
  `mf2::ratatui::with_theme` sets one for a scope on this thread.
* **`set_locale(Locale::Fr)`** is the live switch: the next frame is
  French, on every thread.
* **`mod ui` is declared before the include.** It imports `tr!` with the
  crate's prelude, `use crate::prelude::*`, which any module may do.

```rust file=hops/src/ui.rs generated
//! One frame: the hops in a bordered table, a language menu, a key-hint bar
//! and a status line. Nothing is passed for translation: the text is in the
//! current language, and the theme says how markup looks.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, List, Row, Table};

use crate::prelude::*;

/// A trace in progress.
pub struct Trace {
    pub target: &'static str,
    pub sent: u64,
    pub hops: Vec<Hop>,
}

/// One hop: the host that answered, if one did; the share of probes lost;
/// the average round trip in milliseconds.
pub struct Hop {
    pub host: Option<&'static str>,
    pub loss: f64,
    pub average: f64,
}

impl Trace {
    /// A trace to draw.
    pub fn sample() -> Trace {
        let hop = |host, loss, average| Hop { host, loss, average };
        Trace {
            target: "example.org",
            sent: 1204,
            hops: vec![
                hop(Some("router.lan"), 0.0, 1.1),
                hop(None, 1.0, 0.0),
                hop(Some("example.org"), 0.021, 45.6),
            ],
        }
    }

    /// The share of probes lost over the whole path.
    fn loss(&self) -> f64 {
        self.hops.iter().map(|hop| hop.loss).sum::<f64>() / self.hops.len() as f64
    }
}
```

A description converts to Ratatui's `Span`, `Line` and `Text`, so Ratatui
takes it wherever it takes text: a `Row` of headers, a `Cell`, a block's
title, a `Paragraph`, a `List`.

```rust file=hops/src/ui.rs generated
/// Draws the frame.
pub fn draw(frame: &mut Frame, trace: &Trace) {
    let [main, hints, status] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let [hops, menu] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(16)]).areas(main);
```

```rust file=hops/src/ui.rs generated
    // A bordered table: a title with markup, headers, and cells in the
    // reader's number format.
    let header = Row::new([
        tr!("column.hop"),
        tr!("column.host"),
        tr!("column.loss"),
        tr!("column.average"),
    ])
    .bold();
    let rows = trace.hops.iter().enumerate().map(|(i, hop)| {
        let host = match hop.host {
            Some(name) => Cell::from(name),
            None => Cell::from(tr!("cell.no-reply")),
        };
        Row::new([
            Cell::from((i + 1).to_string()),
            host,
            Cell::from(tr!("cell.loss", share = hop.loss)),
            Cell::from(tr!("cell.ms", ms = hop.average)),
        ])
    });
    let widths = [
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(8),
        Constraint::Length(10),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(tr!("title", target = trace.target)));
    frame.render_widget(table, hops);
```

**Markup is drawn by the theme**: the host in the title is underlined, and
"no reply" is red. The French title puts the host where French wants it,
with no change to the code. Nested elements patch their styles in order; a
name the theme lacks keeps the style around it; standalone markup
(`{#name/}`) draws nothing. Ratatui's own `Stylize` works on a description
too: `.bold()` gives a `Line` that keeps the message's styles over bold.

`Locale::ALL`, `name()` and `current_locale()` make the language menu:

```rust file=hops/src/ui.rs generated
    // The language menu: each language named in itself, the current one bold.
    let items = Locale::ALL.iter().enumerate().map(|(i, &lang)| {
        let item = Line::from(vec![Span::raw(format!("{} ", i + 1)), Span::from(lang.name())]);
        if lang == current_locale() {
            item.bold()
        } else {
            item
        }
    });
    let list = List::new(items).block(Block::bordered().title(tr!("languages")));
    frame.render_widget(list, menu);
```

The key-hint bar is **one message**, so that the translation places the
keys:

```rust file=hops/src/ui.rs generated
    // The key-hint bar is one message, so a translation places the keys.
    frame.render_widget(Line::from(tr!("hints")).right_aligned(), hints);

    // The status line: a plural, a count and a percentage.
    let summary = tr!(
        "status",
        hops = trace.hops.len(),
        sent = trace.sent,
        loss = trace.loss(),
    );
    frame.render_widget(Line::from(summary), status);
}
```

### Lines, spans and markup

* A `Text` starts a new line at each line break in the message; a `Line`
  and a `Span` join the lines with a space.
* **A `Span` has one style, so it keeps none of the message's markup.**
  That has a trap: Ratatui collects anything that converts into a `Span`
  into a `Line`, so `[tr!("a"), tr!("b")].into_iter().collect::<Line>()`
  compiles, and loses both messages' styles. To keep them, write one
  message for the styled line, as `hints` is — the translation then places
  the pieces — or extend a `Line` with each message's
  `Line::from(tr!(…)).spans`.

A conversion borrows the catalog's text from the executable: a message with
no placeholders makes a `Span` without allocating, and each placeholder is
one `String`.

## A library and its terminal UI

`trace` is a workspace of two crates. The library, `trace-core`, owns the
messages: `build.rs` and `locales/` are its. The terminal UI uses the
library's `tr!`, `Locale` and `install()`; there is no third crate for the
translations.

```toml file=trace/Cargo.toml
[workspace]
members = ["core", "tui"]
resolver = "3"
```

The library turns on `native` and the functions its messages call:

```toml file=trace/core/Cargo.toml
[package]
name = "trace-core"
version = "0.1.0"
edition = "2024"

[dependencies]
mf2 = { version = "2", features = ["native", "fn-number"] }

[build-dependencies]
mf2-build = "2"
```

```rust file=trace/core/build.rs
fn main() {
    mf2_build::run();
}
```

```mf2 file=trace/core/locales/en/main.mf2
@locale en
---

title = Trace to {#host}{$target}{/host}

summary =
  .input {$sent :integer}
  .match $sent
  one {{{$sent} probe sent · loss {$loss :percent maximumFractionDigits=1}}}
  *   {{{$sent} probes sent · loss {$loss :percent maximumFractionDigits=1}}}

[error]
resolve = Could not resolve {$host}.
permission = Tracing needs privileges: run as root, or use unprivileged mode.
```

```mf2 file=trace/core/locales/fr/main.mf2
@locale fr
---

title = Trace vers {#host}{$target}{/host}

summary =
  .input {$sent :integer}
  .match $sent
  one  {{{$sent} sonde envoyée · perte {$loss :percent maximumFractionDigits=1}}}
  many {{{$sent} de sondes envoyées · perte {$loss :percent maximumFractionDigits=1}}}
  *    {{{$sent} sondes envoyées · perte {$loss :percent maximumFractionDigits=1}}}

[error]
resolve = Impossible de résoudre {$host}.
permission = Le traçage demande des privilèges : lancez-le en root, ou en mode non privilégié.
```

The library returns **descriptions** — `mf2::TrArgs`, what `tr!` builds —
rather than text, and formats its own errors. Neither takes a handle: an
error prints in the language the application chose, because the process has
one store of catalogs and one language.

```rust file=trace/core/src/lib.rs
//! What a trace knows, and every message the program shows. The terminal UI
//! uses this crate's `tr!`, `Locale` and `install()`.

use std::fmt;

mf2::include_generated!();

/// A trace in progress.
pub struct Trace {
    pub target: String,
    pub sent: u64,
    pub lost: u64,
}

impl Trace {
    /// The trace's title: its target, marked as a host.
    pub fn title(&self) -> mf2::TrArgs {
        tr!("title", target = &self.target)
    }

    /// One line about the trace so far.
    pub fn summary(&self) -> mf2::TrArgs {
        let loss = self.lost as f64 / self.sent.max(1) as f64;
        tr!("summary", sent = self.sent, loss = loss)
    }
}

/// Why a trace cannot start, in the reader's language.
#[derive(Debug)]
pub enum Error {
    Resolve { host: String },
    Permission,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Resolve { host } => write!(f, "{}", tr!("error.resolve", host = host)),
            Error::Permission => write!(f, "{}", tr!("error.permission")),
        }
    }
}

impl std::error::Error for Error {}
```

The terminal UI names `mf2` only for `ratatui`. Cargo unifies the two
crates' features in the workspace, and the library's build script sees the
unified set, so `trace_core::markup` exists when the terminal UI is built.

```toml file=trace/tui/Cargo.toml
[package]
name = "trace-tui"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
mf2 = { version = "2", features = ["ratatui"] }
ratatui = "0.30"
trace-core = { path = "../core" }
```

```rust file=trace/tui/src/main.rs
//! The terminal UI: the library's trace and messages, drawn with Ratatui.

use clap::Parser;
use mf2::ratatui::{Theme, set_theme};
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use trace_core::Trace;
use trace_core::prelude::*;

#[derive(Parser)]
struct Args {
    /// The host to trace.
    target: String,
    /// The language to draw in, instead of the system's.
    #[arg(long)]
    lang: Option<Locale>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    trace_core::install();
    if let Some(lang) = args.lang {
        set_locale(lang);
    }
    set_theme(Theme::default().style(trace_core::markup::HOST, Style::new().underlined()));
    let trace = Trace {
        target: args.target,
        sent: 1204,
        lost: 25,
    };
    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| {
                let block = Block::bordered().title(trace.title());
                frame.render_widget(Paragraph::new(trace.summary()).block(block), frame.area());
            })?;
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('1') => set_locale(Locale::En),
                    KeyCode::Char('2') => set_locale(Locale::Fr),
                    KeyCode::Char('q') => return Ok(()),
                    _ => {}
                }
            }
        }
    })
}
```

Every module of either crate imports the library's prelude, as `ui.rs` does
in `hops`.

**A library shared with a browser client must not turn on `native`**: a
browser build refuses it. Such a library returns descriptions and leaves
each application to choose its mode. This one serves a terminal UI only.
