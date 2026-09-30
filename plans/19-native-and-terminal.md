# 19 — Native and terminal applications, and the shape of 2.0

Part of the [master plan](00-master-plan.md) (D16–D22; §9, P10). RFC 2119 keywords
apply. Written on 2026-09-28 by Phase 10's A8
([18](18-phase-10-work-order.md)), and **approved by the owner the same day**
(18, question 17): every choice in §15 stands. Part C's API work (C1–C9) and
the web's typed API (D4) build it; Part B was free to start before the review.

This is the design of what an application writes against 2.0: the four samples,
exactly; what they cost by A1's counting rules; and the design under them — the
one crate's features, the call-site types per mode, the native store, `Display`
and `Debug`, arguments, Ratatui, the locale matcher, the generated module, the
build, and `tr!` in its own crate. The web side's own design is in
[04](04-leptos-integration.md) §12 and [05](05-tooling.md) §4.1, §6.4 and §9.1;
its sample and its targets are here (§1.4, §2), with the others. Where one of
18's task rows and this document differ, this document is the design, and the
row names the section.

**Inputs.** The owner's answers 1–16 and the records of A1–A7, A9 and both
halves of C3 ([18](18-phase-10-work-order.md)); D16–D24. **Evidence added
here:** one probe, `probes/p10-args/` (§7). Every other figure cites the record
that measured it.

## 1. The four samples

Every file is complete. The code is written against the API this document
designs, so none of it compiles yet. Each sample becomes a compiled book page as
its tasks land: C8 turns the three native ones into `docs/native-apps.md`'s
projects, and D6 turns `hello` into `docs/getting-started.md`. From then on
`cargo xtask docs` holds each page to the code below, as amended by the review.

What the four have in common:
- an application names **`mf2`**, with features, and **`mf2-build`** in its build
  script — nothing else from this library;
- the build script is **`mf2_build::run()`**;
- there is **no translation crate**: `build.rs` and `locales/` sit in the crate
  that uses the messages (D19; A6);
- there is **no `mf2.toml`**: its defaults (source locale `en`, catalogs
  stripped of ids and cold data, a missing message falls back) are what these
  applications want;
- nothing passes a handle, a style map or a language tag around.

### 1.1 A one-file CLI

`--lang`, a plural, an error message, `println!`. It counts the files in a
directory.

`count/Cargo.toml`:

```toml
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

`count/build.rs`:

```rust
fn main() {
    mf2_build::run();
}
```

`count/locales/en/main.mf2`:

```mf2
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

`count/locales/fr/main.mf2`:

```mf2
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

`count/src/main.rs`:

```rust
//! Counts the files in a directory, and says so in the reader's language.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

mf2::include_generated!();

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

What it shows:
- **`Locale`** is generated, one variant per language. clap parses `--lang`
  with its `FromStr`, which goes through the one matcher (§9): `--lang
  fr_CA.UTF-8` is `Locale::Fr`. `--lang de` is refused by clap with the
  matcher's reason, "no language of this application matches; it has en, fr".
- **`install()`** reads the catalogs the executable embeds, once, and chooses
  the first of the system's languages the application has (§5). It returns
  nothing: embedded catalogs from the same build cannot fail to load.
- **`dir = &args.dir`** is a path, one of `IntoArg`'s types. **`error =
  error`** is an `io::Error`, which has no typed form but has a `Display`: its
  text is the argument (§7).
- **`println!("{}", tr!(…))`** is `Display` (§6).

### 1.2 A trippy-shaped TUI

Bordered blocks, table headers, a key-hint bar with styled keys, a status line
with a plural and numbers, a language menu, and a live switch.

`hops/Cargo.toml`:

```toml
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
```

`hops/build.rs`: as the CLI's.

`hops/locales/en/main.mf2`:

```mf2
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

`hops/locales/fr/main.mf2`:

```mf2
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

`hops/src/main.rs`:

```rust
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

`hops/src/ui.rs`:

```rust
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

What it shows:
- **Every Ratatui position takes a description** (§8): a `Row` of headers, a
  `Cell`, a block title with markup, a `Line`, a `Span`. `.bold()` on a `Row`
  is Ratatui's own `Stylize`.
- **Markup is styled by the theme**, set once: the host in the title is
  underlined, the keys in the hint bar bold and yellow, "no reply" red. The
  French title puts the host where French wants it, with no code change.
- **`Locale::ALL` and `lang.name()`** make the language menu: `name()` is the
  `language.<tag>` message, generated when every language has one (§10).
- **`set_locale(Locale::Fr)`** is the live switch: the next frame is French, on
  every thread.
- **`mod ui` is declared before the include**; it imports `tr!` with the crate's
  prelude, which any module of the crate may do (§12).

**`examples/tui`, the measured TUI**, is A1's 1.x program, which the UX table's
1.x row counts; C8 rewrites it on 2.0 the same way:
- **its translation crate goes**, and `locales/` and a one-line `build.rs` join the example;
- **the 118 calls lose the handle** (`i18n`): 48 `line(i18n, &tr!(…),
  styles)` become `Line::from(tr!(…))` (or plain `tr!(…)` where a widget takes
  `Into<Line>`), and 70 `i18n.format(&tr!(…))` become `tr!(…)`;
- **the 10 draw functions lose `i18n`**, and the 6 that drew markup lose `styles`;
- **the style map is set once**, as a `Theme` of the same five names;
- **the language menu** becomes `Locale::ALL` and `name()`, its messages renamed
  to `language.<tag>`.

§2 counts it.

### 1.3 A two-crate workspace

A library and a TUI sharing messages. The library owns the corpus: `build.rs`
and `locales/` are the library's, and the TUI uses its `tr!`, its `Locale` and
its `install()`. There is no third, translation-only crate (1.x had one).

`trace/Cargo.toml`:

```toml
[workspace]
members = ["core", "tui"]
resolver = "3"
```

`trace/core/Cargo.toml`:

```toml
[package]
name = "trace-core"
version = "0.1.0"
edition = "2024"

[dependencies]
mf2 = { version = "2", features = ["native", "fn-number"] }

[build-dependencies]
mf2-build = "2"
```

`trace/core/build.rs`: as the CLI's.

`trace/core/locales/en/main.mf2`:

```mf2
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

`trace/core/locales/fr/main.mf2`:

```mf2
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

`trace/core/src/lib.rs`:

```rust
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

`trace/tui/Cargo.toml`:

```toml
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

`trace/tui/src/main.rs`:

```rust
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
    let trace = Trace { target: args.target, sent: 1204, lost: 25 };
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

What it shows:
- **The library returns descriptions** (`Trace::summary`) and formats its own
  errors (`Display for Error`). Neither takes a handle. The error prints in the
  language the TUI chose, because the process has one store (§5).
- **The TUI names `mf2` only for `ratatui`.** The library turns on `native`;
  cargo unifies the two in the workspace. The library's build script sees the
  unified set through `links` (A2, row 3), so `trace_core::markup` exists when
  the TUI is built.
- **Every module of either crate imports the library's prelude** (§12).
- **A library shared with a browser client must not turn on `native`**, which
  is refused in a browser build (§3). Such a library returns descriptions and
  lets each application choose its mode. This one serves a TUI only.

### 1.4 The Leptos `hello`

Getting started's application, in one crate (A6). Its design is in
[04](04-leptos-integration.md) §12; this is its exact code. The messages are
1.x's, unchanged.

`hello/Cargo.toml`:

```toml
[package]
name = "hello"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
leptos = { version = "0.9.0-beta", default-features = false }
leptos_meta = "0.9.0-beta"
leptos_router = "0.9.0-beta"
mf2 = { version = "2", features = ["leptos", "fn-number"] }

axum = { version = "0.8", optional = true }
console_error_panic_hook = { version = "0.1", optional = true }
leptos_axum = { version = "0.9.0-beta", optional = true }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"], optional = true }
wasm-bindgen = { version = "0.2", optional = true }

[build-dependencies]
mf2-build = "2"

[features]
hydrate = [
    "leptos/hydrate",
    "mf2/hydrate",
    "dep:console_error_panic_hook",
    "dep:wasm-bindgen",
]
ssr = [
    "leptos/ssr",
    "leptos_meta/ssr",
    "leptos_router/ssr",
    "mf2/ssr",
    "mf2/axum",
    "dep:axum",
    "dep:leptos_axum",
    "dep:tokio",
]

[package.metadata.leptos]
output-name = "hello"
site-root = "target/site"
site-pkg-dir = "pkg"
site-addr = "127.0.0.1:3000"
reload-port = 3001
bin-features = ["ssr"]
bin-default-features = false
lib-features = ["hydrate"]
lib-default-features = false
lib-profile-release = "wasm-release"
# `cargo leptos watch` watches the crate's sources only.
watch-additional-files = ["locales"]

[profile.wasm-release]
inherits = "release"
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

`hello/build.rs`: as the CLI's.

`hello/locales/en/main.mf2`:

```mf2
@locale en
---

app-title = Hello, MessageFormat 2
greeting = Hello, {$name}!

# A plural: the number picks the variant, by this language's rules.
visits =
  .input {$count :integer}
  .match $count
  one {{You have been here once.}}
  *   {{You have been here {$count} times.}}

visit-again = Visit again
not-found = There is nothing here.

[language]
label = Language
apply = Apply
en = English
fr = Français
```

`hello/locales/fr/main.mf2`:

```mf2
@locale fr
---

app-title = Bonjour, MessageFormat 2
greeting = Bonjour, {$name} !

visits =
  .input {$count :integer}
  .match $count
  one  {{Vous êtes venu une fois.}}
  many {{Vous êtes venu {$count} fois.}}
  *    {{Vous êtes venu {$count} fois.}}

visit-again = Revenir
not-found = Il n’y a rien ici.

[language]
label = Langue
apply = Appliquer
en = English
fr = Français
```

`hello/src/lib.rs`:

```rust
use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use mf2::leptos::{CatalogLinks, CatalogPreload, LocaleSwitcher, html_lang};

mf2::include_generated!();

/// The document. `lang` and `dir` are those of the language this request
/// is rendered in, so an Arabic page is right-to-left from its first byte.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    let (lang, dir) = html_lang();
    view! {
        <!DOCTYPE html>
        <html lang=lang dir=dir>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
                <CatalogPreload />
                <CatalogLinks />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text=tr!("app-title") />
        <Router>
            <header>
                <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply") />
            </header>
            <main>
                <Routes fallback=|| view! { <p>{tr!("not-found")}</p> }>
                    <Route path=path!("/") view=Home />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Home() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <h1>{tr!("greeting", name = "Ada")}</h1>
        <p role="status">{tr!("visits", count = count)}</p>
        <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
    }
}

/// The browser's entry point.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    install();
    mf2::leptos::hydrate_body(App);
}
```

`hello/src/main.rs`:

```rust
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::Router;
    use hello::{App, shell};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler, generate_route_list};
    use mf2::axum::{Negotiator, catalog_routes};

    // The catalogs this build embeds, read once.
    hello::install();

    let conf = get_configuration(None)?;
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(file_and_error_handler(shell))
        // Each request's language: `?lang=`, then the cookie, then
        // `Accept-Language`; the response gets `Content-Language`, `Vary`
        // and the cookie.
        .layer(Negotiator::default())
        // `/i18n/*`: the catalogs, served from the binary.
        .merge(catalog_routes())
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
```

On Leptos 0.8 the application names the 0.8 crates and `mf2`'s other line.
Nothing else changes, and nothing needs `default-features = false`:

```toml
[dependencies]
leptos = { version = "0.8", default-features = false }
leptos_meta = "0.8"
leptos_router = "0.8"
leptos_axum = { version = "0.8", optional = true }
mf2 = { version = "2", features = ["leptos-0-8", "fn-number"] }
```

What it shows:
- **One crate**, with the mode forwarded to `mf2` alone, and `mf2/axum` beside
  `mf2/ssr` on the server's side.
- **`<LocaleSwitcher label button/>` with no children** lists every language,
  each named by its `language.<tag>` message, in its own `lang`. 1.x wrote one
  `<LocaleOption>` per language; adding a language now needs no code.
- **`install()` on each side**: on the server it reads the embedded catalogs;
  in the browser it records what the build generated for `hydrate_body`.
- **No `_with_context` wiring**: the negotiation is a tower layer, and the
  render finds its result through the request (D3, a probe first; §14).
- **No `#[cfg]` pair** anywhere in application code. A control of its own
  calls `set_locale(Locale::Fr)` on either side (04 §12.4).

## 2. The UX targets

Counted by A1's rules (18, "The UX table's 1.x column, by the rules") on §1's
code:
- **setup lines** — non-blank, non-comment lines that exist only for
  translation, or name one of our crates, or forward a feature to one;
- **crates named** — the family's crates in the manifests, plus the
  application's own translation crate;
- **concepts** — the distinct names of our API, generated items, features and
  `mf2.toml` keys the author writes before the first translated output;
- **commands** — what the book's page has the reader run, from an empty
  directory to the first translated output, with the files written by hand
  beside them.

| Sample | 1.x (A1) | **2.0 target** |
|---|---|---|
| one-file CLI | 35 lines; 3 + 1 crates; 18 concepts; 1 command, 4 translation files by hand | **13 lines; 2 + 0 crates; 8 concepts; 1 command, 1 file by hand** (`build.rs`) |
| trippy-shaped TUI (`examples/tui`) | 48; 4 + 1; 22; 1, 4 files | **24; 2 + 0; 14; 1, 1 file** |
| — the book's TUI (§1.2) | — | 23; 2 + 0; 17; 1, 1 file |
| two-crate workspace | 48; 4 + 1; 22; 1, 4 files | **19; 2 + 0; 16; 1, 1 file** |
| Leptos `hello` | 96 (+ 3 changed); 4 + 1 and the `mf2` tool; 28; 5 commands, 2 of them this library's | **22 (0 changed); 2 + 0, no tool; 18; 3 commands, none of them this library's** |

**What "falls" means.** A 2.0 row falls when three things hold:
- setup lines, crates and concepts are each below 1.x's;
- commands do not rise;
- fewer translation files are written by hand.

A count of one command cannot fall. The native pages' 1.x counts were already
one (`cargo run`). What fell there is the four files of a translation crate,
written by hand, which becomes `build.rs`.

The pages are counted along their hand-written path. `mf2 init` (C7, D5) is the
shortcut beside that path: two commands (installing `mf2-cli`, then `mf2 init`)
in place of every file. **The targets are ceilings.** A page that needs more
comes back to this document with the reason, rather than landing above them.

**The lines counted**, so that C8 and D6 recount alike:
- **CLI (13):**
  - `Cargo.toml` (3): the `mf2` dependency, `[build-dependencies]` and `mf2-build`;
  - `build.rs` (3);
  - `main.rs` (7): `include_generated!`, `#[arg(long)]`, `lang: Option<Locale>`,
    `install()`, and the three lines of `if let … set_locale`.
- **`examples/tui` on 2.0 (24):**
  - `Cargo.toml` (3), `build.rs` (3), `lib.rs` (1: the include);
  - `main.rs` (16): two `use` lines (the library's prelude, `mf2::ratatui`),
    `--lang` (2), `install()`, `set_locale` (3), and the 8-line theme of five
    names;
  - `ui.rs` (1: the prelude).
  1.x's 48 was the CLI's 35, `mf2-ratatui`, three `use` lines, and an 8-line
  style map built for each draw (+1).
- **The book's TUI (23):**
  - as the CLI's Cargo.toml and build.rs (6);
  - `main.rs` (16): the include, `use mf2::ratatui`, `--lang` (2), `install()`,
    `set_locale` (3), the 6-line theme, and the two switch keys (1.x's counted
    program had no live switch);
  - `ui.rs` (1).
  The language menu is drawing code with translated text, as 1.x's
  `draw_language` was, and is not counted.
- **Two-crate (19):**
  - `core/Cargo.toml` (3), `core/build.rs` (3), `core/lib.rs` (1);
  - `tui/Cargo.toml` (1: `mf2` with `ratatui`);
  - `tui/main.rs` (11): two `use` lines, `--lang` (2), `install()`,
    `set_locale` (3), the theme (1) and the two switch keys.
- **`hello` (22):**
  - `Cargo.toml` (7): the `mf2` dependency, `[build-dependencies]`,
    `mf2-build`, `"mf2/hydrate"`, `"mf2/ssr"`, `"mf2/axum"`,
    `watch-additional-files`;
  - `build.rs` (3);
  - `lib.rs` (8): the include, one `use`, `html_lang()`, the two head
    components, the switcher, `install()`, `hydrate_body`;
  - `main.rs` (4): one `use`, `install()`, `.layer(…)`, `.merge(…)`.
  `<html lang=lang dir=dir>` is a changed line, as in 1.x. The three
  `_with_context` forms become Leptos's plain ones, which a Leptos application
  without translations has, so they are no longer changed lines.

**The concepts counted:**
- **CLI (8):** `native`, `fn-number`, `mf2_build::run`, `include_generated!`,
  `install`, `Locale`, `set_locale`, `tr!`.
- **`examples/tui` (14):** `ratatui`, `fn-number`, `run`, `include_generated!`,
  `install`, `Locale`, `set_locale`, `tr!`, `prelude`, `Theme`,
  `Theme::default`, `style`, `set_theme`, and `markup`. The five constants
  count as one concept: 1.x's markup names were strings, not counted.
- **The book's TUI (17):** the above, and `Locale::ALL`, `name`,
  `current_locale`.
- **Two-crate (16):** `native`, `fn-number`, `ratatui`, `run`,
  `include_generated!`, `tr!`, `mf2::TrArgs`, `install`, `Locale`,
  `set_locale`, `prelude`, `Theme`, `Theme::default`, `style`, `set_theme`,
  `markup`.
- **`hello` (18):**
  - `run`, `include_generated!`, `tr!`;
  - the features: `leptos`, `fn-number`, the mode forwarded to `mf2` (one, as
    A1 counted it), `axum`;
  - `watch-additional-files`;
  - `html_lang`, `CatalogPreload`, `CatalogLinks`, `LocaleSwitcher`, the
    `language.<tag>` messages, `install`, `hydrate_body`;
  - `Negotiator`, `Negotiator::default`, `catalog_routes`.

**One difference of content, not of API.** 1.x's `hello` also turned on
`fn-datetime` and `datetime-icu`, which its messages do not use. With them the
2.0 page writes `datetime-icu` in `mf2`'s list (it implies `fn-datetime`) and
`features = ["icu-blob"]` on `mf2-build`, whose line stays one line (§11): +2
concepts, +0 lines, 20 concepts against 28.

## 3. One crate: features and modules

`mf2` is the crate an application names (D16). `leptos-mf2`, `mf2-native`,
`mf2-ratatui` and `mf2-axum` fold into it as the modules `mf2::leptos`,
`mf2::native`, `mf2::ratatui` and `mf2::axum`, each behind its feature. The six
Leptos components live in `mf2-leptos-ui-0-9` and `mf2-leptos-ui-0-8`, which
`mf2::leptos` wraps with its `Layer` chosen (question 13; B1). 18 published crates become 16.

| Feature | Implies | Gives |
|---|---|---|
| *(none)* | — | the call-site types (§4), `IntoArg`, `Message`, the runtime's formatter and extension points, `Corpus`, `include_generated!`. Text only through a `Formatter` the caller builds |
| `native` | `host-std`; `sys-locale`, `jiff` (`tz-system`) | `mf2::native`: the app-wide store (§5), and the ambient forms (§6) |
| `ratatui` | `native`; `ratatui-core` 0.1 | `mf2::ratatui`: the conversions and the theme (§8) |
| `leptos` | the Leptos 0.9 crates | the line `mf2::leptos` renders with — the default line |
| `leptos-0-8` | the Leptos 0.8 crates | the 0.8 line |
| `ssr` | `host-std`; the Leptos layer | server rendering; the ambient forms |
| `hydrate`, `csr` | `host-web`; the Leptos layer | the browser client; the ambient forms |
| `axum` | `host-std`; `axum`, `tower` | `mf2::axum`: the negotiation layer, the catalog routes, `Locale` as an extractor; with `ssr`, the render reads what the layer negotiated |
| `static-locale`, `mark-fallback-lang` | a Leptos mode | as in 1.x |
| `fn-number`, `fn-datetime`, `datetime-icu`, `datetime-intl`, `intl` | as in 1.x | as in 1.x |
| `compile` | `host-std` | `compile_str` |
| `clap` | `clap` | the generated `Locale` as a clap value: parsed by the matcher, its values listed in `--help` (§10) |
| `host-std`, `host-web` | — | kept; implied by the above, and named by hand only for a formatter of one's own |

**`leptos` is not a default feature, and a mode needs a line.** An application
writes the line once, on its `mf2` dependency (`features = ["leptos"]`), and
the mode where it writes Leptos's (`ssr = ["mf2/ssr", …]`). An application on
0.8 writes `leptos-0-8` instead. No manifest turns default features off.
Refused with a `compile_error!` that says what to write:
- two modes, as in 1.x;
- both lines;
- a mode with no line: "mf2: `ssr` needs a Leptos line: turn on `leptos`
  (Leptos 0.9) or `leptos-0-8` beside it";
- `native`, `ratatui` or `axum` together with `hydrate` or `csr` **when
  compiling for the browser** (`wasm32`), which would put them in a browser
  build (master plan §4.1). On the host the combination compiles, so
  `cargo check --workspace` over a browser client and a native application
  works, as in 1.x (owner, [18](18-phase-10-work-order.md) question 24).

Every other combination compiles, `ssr` with `native` included: cargo unifies
features across a workspace. The matrix that proves it is B1's and C4's
(`codegen-matrix`).

**Inside `mf2`** (A7's rules):
- the Leptos lines are reached through a private alias module;
- `::axum` is written wherever the module `axum` is in scope at the crate root;
- there is no `extern crate … as leptos`.

**In applications** (A7, N5): import items from `mf2::leptos`, or write full
paths, and never `use mf2::leptos;` in a module that uses the `leptos` crate.
The book says so.

## 4. The call-site types, and the API per mode

`Tr`, `TrArgs`, `TrRich`, `TrDyn`, `ArgValue`, `ArgList`, `ArgSource`,
`DateTimeValue`, `Text`, and the markup traits are **defined once, in `mf2`,
with only additive impls behind features**, the arrangement that survives
feature unification (`plans/phase-6-results.md`, "A hazard found the hard
way"). `Tr` stays a 4-byte `Copy` value, `const`-constructible (04 §2).

| On the four descriptions | core | `native` | `ssr` | `hydrate` / `csr` | `ratatui` |
|---|---|---|---|---|---|
| `id()`, `write` / `parts` / `format` against a `Formatter` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `Debug` (§6) | ✓ | ✓ | ✓ | ✓ | ✓ |
| `to_string()`, `to_plain_string()`, `to_cow()`, `Display`, `From<_> for String` (§6) | — | ✓ | ✓ | ✓ | ✓ |
| the Leptos glue (`Render`, `AttributeValue`, `IntoProperty`, `From` into `TextProp`, `Signal<String>`, `Oco`) | — | — | ✓ | ✓ | — |
| `From` into `Span`, `Line`, `Text`; `Widget`; `Styled` (§8) | — | — | — | — | ✓ |

**A build with no mode has no ambient text** (A5). Its descriptions have no
`to_string()` and no `Display`, since nothing says which catalog to read. A
library that only returns descriptions needs no mode. One that formats turns on
`native`, or is built only as part of an application that has a mode.

| Module | What an application names | Mode |
|---|---|---|
| `mf2` (root) | the types above; `IntoArg`; `Message`; `UnknownLocale` (the error of `Locale::from_str`); `Dir`; `LanguageMatching` (the type of the generated `LANGUAGE_MATCHING`; C3); `include_generated!`; `compile_str` (`compile`); the runtime's formatter, sinks and function traits, as in 1.x | always |
| `mf2::native` | `Catalogs` (1.x's `NativeI18n`, explicit, with no globals); `Error`; `set_bidi` / `bidi`; `set_time_zone` / `time_zone`; `locale_source` and what it returns, `LocaleSource` (1.x's name, beside `mf2::axum`'s trait; §16) | `native` |
| `mf2::ratatui` | `Theme`, `Markup`; `set_theme`, `with_theme`, `theme` | `ratatui` |
| `mf2::leptos` | the six components and their props; `html_lang`; `islands_gate!`; `Setup`, `LoadError`, `track_locale`; the boots `hydrate_body`, `hydrate_lazy`, `hydrate_islands` (`hydrate`) and `mount_to_body` (`csr`); `RequestI18n` (`ssr`) | a Leptos mode |
| `mf2::axum` | `Negotiator` (a tower layer), `Negotiated`, the source and sink traits and their four built-in sources, `catalog_routes`, `path_prefix_redirect`, `negotiated` | `axum` |

The generated module adds the application's own typed items: `Locale`,
`install`, `set_locale`, `current_locale` and others (§10). B5 lists `mf2`'s
API per mode (`crates/mf2/api/{core,ssr,hydrate,csr,native,ratatui}.txt`;
`axum.txt` with D1), from the table `[package.metadata.api]` in its manifest,
and `cargo xtask release` compares each mode with the last release. The
generated items are each application's own code, so no listing of `mf2`
holds them; what 2.x promises of them, the version policy says (G1).

## 5. The native store, and the one lookup

A4's variant (b), measured at 0.86–0.88 × 1.x's time for a 112-message frame,
227 allocations against 413, and a stripped CLI 5,248 B smaller (A4):
- **the store** is a `OnceLock` holding the `&'static Corpus` and one `Catalog`
  per language. Each `Catalog` reads the executable's bytes in place and is
  leaked once, so each is `&'static`;
- **the app-wide language** is one `AtomicUsize`, an index into the corpus's
  languages. `set_locale` stores it, and every thread's next format reads it;
- **a thread's override** is one thread-local `Cell`. `with_locale` sets it,
  and a guard restores it, also when the body unwinds, so tests pinned to
  different languages run in parallel;
- **the settings** (bidi, time zone, theme) sit behind an `RwLock` with a
  generation counter. Each thread keeps a copy and takes the lock only when the
  generation has moved: about 27–31 ns a format saved against reading the lock
  (A4);
- **text is borrowed**, not copied: a simple message is a `&'static str` from
  the catalog, and pattern text parts come through the runtime's hidden seam
  (`PartSink::part_catalog_text`, A4's method S; fallback R2). Only a
  placeholder's text is allocated.

**The one lookup** (D17), used by `Display`, every `to_string()` and every
Ratatui conversion, in this order:
1. the request's catalog (`ssr`) or the page's (`hydrate` / `csr`);
2. the native thread's override;
3. the native app-wide language.

Each step exists only with its feature. A native-only build has steps 2 and 3;
C2 times step 1 in a build that unifies `ssr` and `native` (A4).

**Before `install()`:**
- **in a build whose only mode is `native`**, the ambient forms panic with
  "mf2: no catalogs are installed: call install() at start-up" (decided in 18,
  "Decided without asking"; A4's test);
- **in a build with a web mode**, the web's rule holds: empty text, never a
  panic (04 §5), and E4's one-time warning;
- **`Locale::format` and `with_locale`** do not need `install()`. They load the
  embedded catalogs on first use, so a test or a server can format in a named
  language without choosing an app-wide one.

**What `install()` does.** It loads every embedded catalog and chooses the
first of the system's preferred languages that the matcher accepts (§9), else
the source language; `locale_source()` says which happened. It returns nothing:
- a failure would mean a corrupt executable, or catalogs from two builds, which
  the build cannot produce. It panics, naming the catalog;
- a second call with the same corpus does nothing;
- another corpus in the same process is refused (§13).

`install_from_directory(dir) -> Result<(), mf2::native::Error>` is the form
that can really fail. It loads catalogs shipped beside the executable, each
checked against its content-hashed name (1.x's rules).

**The settings natively:**
- **bidi isolation is off by default**, since terminals show the isolation
  marks as stray characters (1.x); `set_bidi(BidiStrategy::Default)` turns it
  on for a terminal that applies them;
- **the time zone is the system's**, by its IANA name. Without one it is a zone
  that follows the system's daylight-saving rules (a POSIX `TZ` such as
  `EST5EDT,M3.2.0,M11.1.0`), never the offset in force at start-up, as 1.x
  froze it. UTC is the last resort. C2 chooses how the runtime's `TimeZone`
  carries rules and tests it across a transition.

**`mf2::native::Catalogs`** is the explicit form, with no globals. It holds a
corpus's catalogs, and `format(locale, &message)` formats with them. The store
is built on it. It serves a server, a tool, and later several message sets in
one process (§13).

**As built (C2;** [18](18-phase-10-work-order.md), C2's record**).** Where the
design above did not settle a detail:
- **`with_locale(&CORPUS, locale, body)` takes the corpus**, so that it works
  before `install()`: it loads that corpus's embedded catalogs into the store
  if the store is empty. C4's typed `with_locale(Locale, body)` passes its own
  `CORPUS`; a corpus other than the store's is `Error::AnotherCorpus`.
- **`set_locale` needs `install()`**, as `locale()` (outside `with_locale`) and
  `locale_source()` do: `install` chooses the app-wide language once, and a
  later `install` keeps an explicit one.
- **The store is a `Catalogs` in a `OnceLock`**, loaded by the first of
  `install`, `install_from_directory` and `with_locale`; with the store
  loaded, `install_from_directory` reads nothing. The app-wide language and
  where it came from are one atomic, read together. The time zone setting
  starts as "the system's", read at its first use, so an application that
  sets its own first never reads the system's.
- **The rule-following zone** is `mf2_runtime::TimeZone::rules("EST5EDT,…")`,
  a POSIX TZ rule the zone carries as it carries a name. The date functions
  see it where they see a named zone (`ZoneOption::Named`) and ask the host
  for its offsets; `mf2-host-std` evaluates it with jiff after its database.
  The system's rule is `TZ`'s when `TZ` holds one, else the one a TZif file
  (version 2 and later) ends with. The browser's host evaluates no rule (a
  *Bad Option*, as for a zone it does not know); the web never makes one.
- **Names:** `Catalogs`, and `Error` (1.x's `NativeError`, renamed). 1.x's
  `NativeI18n` stays in `mf2::native`, hidden and built on `Catalogs`, for the
  `mf2-native` shim and `mf2::ratatui`'s 1.x `line` / `text` (C8). Its
  `from_directory` keeps 1.x's rule, every file required; the 2.0 forms take
  a partial set.
- **`Catalogs::format` in a locale it has no catalog for** formats in the
  source language, as the web's `provide_locale` does.
- **The one lookup beside a Leptos mode:** step 1 is the request's context
  alone (not the server's fallback to the source locale's catalog), then the
  store, then the web's rule. A build with a Leptos mode and no `native`
  compiles the web's text path as before.
- **The ambient forms natively** (§6) are `to_string()`, `to_plain_string()`,
  `to_cow()`, `Display` and `From<_> for String`; 1.x's web synonym
  `to_display_string()` stays with the Leptos modes. Each goes through one
  function taking a trait object of the two methods the text needs, as A4
  found cheapest.
- **The parts seam** (`PartSink::part_catalog_text`, A4's S) is C5's: the
  Ratatui conversions are its only user.

## 6. `Display`, `to_string`, and `Debug`

As the owner decided in question 14:
- **`to_string()`** (inherent, fmt-free) is the text in the current language.
  It is isolated as the mode's default says: the web isolates, since the
  specification makes that the default for a single string (04 §9, Phase 7
  A12). Natively it follows the store's bidi setting, which is off unless set;
- **`to_plain_string()`** is never isolated;
- **`to_cow() -> Cow<'static, str>`** borrows when the message is simple and its
  catalog is `&'static` (native), and owns the text otherwise;
- **`Display`** pads the text `to_cow()` gives: `f.pad(&self.to_cow())`. That is
  A9's S3: one text path per description type, shared by `.to_string()`, `{}`
  and a wrapper's `.to_string()`, and `{:<12}` pads;
- **`From<Tr…> for String`** stays.

What it costs:
- **in a browser build**, `{}` costs 25–70 B gz over `.to_string()` in an
  application that already calls `format!`. A wrapper's `.to_string()` (a
  signal's read guard, an `Arc`, a `RefCell` borrow, `&&`) costs 60–100 B gz
  (A9);
- **natively**, a simple message's `{}` allocates nothing. A message with
  arguments allocates the one `String` the formatter pads. A4's streaming
  `Display` allocated none, and was set aside for the single text path the
  owner chose. 1.x allocated that `String` too (`i18n.format`), so the native
  gate (allocations ≤ 1.x) holds.

**`Debug` is on every public type**:
- **on the call-site types** it is written through `write_str` (A9's S2), with
  no builder, no float formatter and no string escaping. `{:?}` on a `TrArgs`
  costs 0.9–1.2 KB gz in a browser build, against 11.8–16.5 KB for the derived
  form (A9);
- **on the runtime's and the catalog's 31 public types** that lack it (A5), it
  is derived, or hand-written where a field has no `Debug`, as the Rust API
  guidelines ask (C-DEBUG). `bash bench/b12/check.sh` proves none of it is
  reachable from the client's paths;
- **`missing_debug_implementations` warns** in `mf2`, the runtime and the
  catalog (CI denies warnings).

**S2's fidelity**, which the book states:
- a float shows at most six fraction digits (`1e-7` prints `0.0`);
- a quote or a newline in a text is not escaped;
- a date prints as `DateTimeValue(2026-09-28T14:05:09.007)`;
- `{:#?}` prints what `{:?}` prints;
- integers, `2.5`, `0.1`, `123456.789`, `Unset` and `Custom(..)` print as a
  derived `Debug` would;
- a description shows its `MsgId` number, not the message's id, which is not
  in the build (B6).

**The book's rule (F):**
- `.to_string()` is the leanest in browser code;
- `{}` costs a few dozen bytes;
- `{:?}` costs about 1 KB;
- `unwrap()` and `assert_eq!` on a description reach `{:?}`.

**The demos keep a check.** D6 adds it to the nightly job: a debug-profile
client build of each demo, then `fmt-check.sh` (A9), which fails on any
`Display` or `Debug` impl of ours. The demos are what readers copy, so they
follow the book's rule. The check costs about 2 minutes a demo from cold, and
3 seconds incremental (A9). A release build with names kept misses the wrapper
traps, which is why the build is a debug one (A9).

## 7. Arguments

**`IntoArg`** replaces `From<…> for ArgValue` as the macro's conversion (C1). It
is a trait of ours, implemented per type, with a
`#[diagnostic::on_unimplemented]` message that names what is accepted:
- **integers:** `i8`…`i128`, `u8`…`u128`, `isize`, `usize`, and the `NonZero`
  forms. They are exact: past `i64`, an exact decimal written without
  `core::fmt`, and `usize` without saturation;
- **other numbers and values:** `f32`, `f64`, `bool` (as the string `true` or
  `false`, which `.match` selects on), `char`;
- **text:** `&str` (copied, unless a literal, which the macro emits as
  `ArgValue::str_static`), `String`, `&String`, `Cow<'static, str>` (borrowed
  stays static), `Arc<str>`, `Text`;
- **under std:** `Path`, `PathBuf`, `OsStr`, `OsString` and their references
  (their text, lossy where it is not UTF-8), and `SystemTime` (an instant);
- **under `native`:** jiff's `Timestamp`, `Zoned` and civil date and time types;
- **dates and extensions:** `DateTimeValue` and the runtime's `DateTime`,
  `ArgValue` itself, and `Arc<C: CustomValue>`;
- **under a Leptos mode:** the signals of any `T: IntoArg`;
- **`&T` for `T: IntoArg + Copy`.**

**Any other type with a `Display` is an argument too, as its text.** That
covers trippy's `KeyBinding`, an `io::Error`, an `Ipv4Addr`, or a type with an
English `Display` of its own. `tr!` emits, for each argument, a method
dispatch spanned at the argument. The design had three steps on a wrapper
holding the value; C1 built four, the second keeping 1.x's conversion, and
chooses the step from the type alone, on a zero-sized `Probe<T>`, in a
closure, while the value goes straight into a function as it went into 1.x's
`ArgValue::from(e)` (below, "As built"): `convert(e, |p| (&&&p).__mf2_kind())`.
1. `&&&Probe<T>`, where `T: IntoArg` — the typed conversion;
2. `&&Probe<T>`, where `T: Into<ArgValue>` — 1.x's conversion, for an
   application's own `From<T> for ArgValue` and generic code bounded on it;
3. `&Probe<T>`, where `T: Display` — the value's text, formatted when the
   description is built;
4. `Probe<T>`, which always applies, and whose method requires
   `T: IntoArg` — so a type with none of these gets `IntoArg`'s own message.

**Observed** (`probes/p10-args/`, `./run.sh`, rustc 1.98.1, 2026-09-28):
- 13 of 13 accepted cases take the step expected:
  - `i64`, `&i64`, `usize`, `String`, `&str` and `&Path` by `IntoArg`;
  - `io::Error`, `Ipv4Addr`, a `KeyBinding` and `&KeyBinding` by `Display`;
  - a type with both by `IntoArg`;
  - generic code bounded on `Display` alone by `Display`;
  - generic code bounded on both by `IntoArg`.
- The three refused cases give E0277 with our text:
  - a type with neither: "`Opaque` is not a message argument", with
    rustc's list of the types that implement `IntoArg`;
  - an unbounded generic `T`: rustc's help, "consider restricting type
    parameter `T` with trait `IntoArg`";
  - an `Option<i64>`.
- Each refusal also carries a note naming the hidden method
  (`ViaNeither::__mf2_arg`), which adds noise. The first line and the label
  are ours.

**What the `Display` step costs:**
- **nothing where it is not taken.** The typed step compiles to what 1.x's
  `ArgValue::from` does. C1's gate is `b5 --view`, unchanged;
- **one `String` where it is taken**, the text made once when the description
  is built.

In a browser build that step links the type's own `Display`. 1.x's alternative
was the application writing `.to_string()` itself, which costs the same.

A number type with no `IntoArg` (a `rust_decimal::Decimal`) becomes its text.
`:number` and `:integer` parse a text operand as a number literal
(`mf2-runtime`'s `numeric_operand`), so it still formats and selects, exactly.

**The risk it leaves.** A type whose `Display` is English is shown in English,
inside a translated sentence. 1.x's compile error did not prevent that either:
its fix was `.to_string()`. The book shows the MF2 way, a string selector:
`state = state.as_str()`, and `.match $state` in the message.

**The `&str` copy.** A `&str` from a variable is copied into an `Arc<str>`, one
allocation per argument (A4). C1 measures an inline short string against B5 and
keeps it only if it pays.

**As built (C1;** [18](18-phase-10-work-order.md), C1's record**).** Where the
design above did not settle a detail, or could not be built as written:
- **A second step, 1.x's `From`.** 1.x's `tr!` wrote `ArgValue::from(e)`, so
  an application's own `impl From<Meters> for ArgValue`, and generic code
  bounded `where ArgValue: From<T>`, were arguments. With three steps they
  would stop compiling, or turn silently into the type's `Display` text: the
  promise that a 1.x application compiles unchanged, and §14's gate "every
  1.x argument type still accepted", need them. The step comes after `IntoArg`,
  so a built-in type converts by `IntoArg` (a `usize` exactly, where
  `ArgValue::from` saturates), and before `Display`, so a type with both
  keeps 1.x's conversion.
- **The step from the type, the value untouched.** Four steps need four
  receiver types, and the probe's wrapper has three (by value, `&`, `&mut`).
  A first build put the value in an `Option` behind a `Deref`, for step 2 to
  move it out of `&mut`: each `String` argument kept the `Option`'s check
  (`b5 --view` +1.0 B gz a site). A second chose the kind on a zero-sized
  probe of `&value`, the value bound by a `match`: the code around the
  arguments moved by a few bytes in 14 of 60 components (`tr-view` +196 B
  raw at 1,860 sites, which gzip read as +530 B and brotli as −125). The
  kind is now chosen in a closure given the probe, which rustc checks after
  `e`, and the value goes straight into `mf2::__arg::convert`, as into
  `ArgValue::from`: the size workloads compile to the code HEAD's did (C1's
  record).
- **References to types that are not `Copy`.** `&T` for `T: IntoArg + Copy`
  and an `IntoArg` for `&String` cannot both exist: rustc refuses the pair
  (E0119), since `String` could become `Copy` upstream. `&String` keeps 1.x's
  `From<&String>`, and `From<&PathBuf>`, `From<&OsString>` and (with
  `native`) `From<&Zoned>` are added, all taken by step 2. `&Zoned` would
  otherwise have been its `Display` text, not a date.
- **`Cow<'static, str>`.** Trait selection ignores lifetimes, so a `Cow` that
  borrows for less than `'static` takes step 1 and is refused by the borrow
  checker ("argument requires that `name` is borrowed for `'static`"), as
  1.x refused every `Cow`. The rustdoc says to pass `&*cow`.
- **jiff's civil types:** `civil::Date` (a floating date at 00:00, as a date
  literal is) and `civil::DateTime` (floating). `civil::Time` is not an
  `IntoArg`: the runtime's date/time value always has a date, and the
  date/time literal grammar has no time-only form, so a date would have to be
  invented. It takes step 3, its ISO text.
- **Instants** (`SystemTime`, `Timestamp`, `Zoned`) are floored to the
  millisecond, the runtime's resolution, also before the epoch. A
  `SystemTime` past the years a `Date` holds (±999,999) is
  `ArgValue::Unset`, an Unresolved Variable. A `Zoned` keeps its offset, and
  its zone when jiff has its IANA name.
- **"Under std"** is wherever `mf2` links `std`: `host-std`, `native` or a
  Leptos mode (`extern crate std` now also under `host-std`, which links it
  through `mf2-host-std` anyway).
- **The span.** On stable a span is one token, so the error points at the
  argument's first token (`Some` of `Some(3)`); the trybuild case shows it.
- **Integers past `i64`** are written by one shared function, and the five
  wide conversions (`u64`, `i128`, `u128`, `usize`, `isize`) are out of line:
  on a 64-bit target a `usize` argument may take that path, so a terminal UI
  pays for it (+1,760 B stripped `tui-mf2`, its `usize` arguments; C1's
  record). On `wasm32` every `usize` fits `i64`, and nothing of it is linked.
- **The `&str` copy stays.** The inline short string was measured and not
  kept: 10 of a frame's 1,816 allocations, no time measurably saved, about
  500 B more in every web client, and a `Text` variant that 1.x code matching
  `Text` exhaustively would not compile against (C1's record).

## 8. Ratatui

**Conversions** (D18; A7 proved they are coherent with Leptos's impls and with
Ratatui's blanket impls):
- **`From<Tr | TrArgs | TrRich | TrDyn>`** for `Span<'static>`,
  `Line<'static>` and `Text<'static>`. `Cell`, `Row`, `ListItem`, `List`,
  `Tabs`, `Paragraph` and `Block::title` take them through Ratatui's own
  blankets;
- **`Styled<Item = Line<'static>>`**, so Ratatui's `Stylize` works:
  `tr!("quit").bold()` is a `Line` that keeps the message's own markup styles
  and patches the call's style under them;
- **`Widget`** for each description, drawn as its `Text`;
- **by-reference forms** (`From<&TrArgs>`) are C5's to add, each checked
  against A7's coherence set.

**Never** (A7, each with its E0119): `From<Tr> for Cow<str>`, and
`FromIterator<Tr> for Line`. `Cell` and `ListItem` come through their blankets.

**What a conversion allocates** (A4's zero-copy sink):
- constant text is borrowed;
- pattern text parts are borrowed through the seam;
- a placeholder is one `String`;
- the open-element stack is inline, keyed by the markup name's hash, so markup
  names never allocate.

A markup line fell from 10 allocations to its `Line`'s `Vec` plus one per
placeholder, and A4's frame from 413 to 227.

**Line breaks:**
- `Text` starts a new line at each line break in the message;
- `Line` and `Span` join the lines with a space;
- **a `Span` flattens markup:** a span has one style.

The last has a trap, which the book names. `Line`'s blanket collects anything
`Into<Span>`, so collecting descriptions into a `Line`
(`[tr!("a"), tr!("b")].into_iter().collect::<Line>()`) compiles and loses their
markup styles. The book shows two ways to keep them:
- **one message for a styled line** (§1.2's `hints`), which is also the MF2
  way: the translation then places the keys;
- **extending a `Line`** with each message's `Line::from(…).spans`, as
  `examples/tui`'s seven hints will.

**The theme** (question 5):
- **`Theme`** maps a markup name's hash (FNV-1a 64, the key `TrRich` already
  uses) to a `Style`;
- **`Theme::default()`** styles the common names:
  - `b` and `strong` bold;
  - `i` and `em` italic;
  - `u` underlined;
  - `s` and `del` crossed out;
  - `code` and `kbd` reversed.
- **`Theme::empty()`** styles none;
- **`.style(markup::KEY, style)`** sets or replaces one name's style;
- **`set_theme(theme)`** sets the app-wide theme. Every thread's next draw uses
  it (it is one of the store's settings);
- **`with_theme(&theme, || …)`** sets this thread's theme for a scope;
- **`theme()`** reads the theme in force.

**How markup is drawn:**
- nested elements patch their styles in order;
- a name the theme lacks keeps the style around it;
- standalone markup (`{#name/}`) writes nothing, as in 1.x.

**`markup::*`** is generated with `ratatui`: one constant per markup name the
corpus uses. `markup::KEY` is a `mf2::ratatui::Markup`, holding the hash and
the name, so a misspelt name is a compile error. The build refuses two names
whose hashes collide, as the macro already does within one message.

**What goes.** The old `line` / `text` / `MarkupStyles` go with C8's page.
`MarkupStyles` was a `Vec<(String, Style)>` built per draw, and `line` / `text`
took the handle and the map on every call. Until then they are
`mf2::ratatui`'s, under 1.x's names (B3), and the `mf2-ratatui` shim
re-exports them; what the shim keeps when they go is C8's to settle
([18](18-phase-10-work-order.md), B3's record).

**The gate** (C5, against A1's figures on `examples/tui`, per frame in
en/de/es/fr):
- allocations ≤ both 1.x (1,816 / 1,815 / 1,816 / 1,817) and the in-house
  upstream re-implementation (1,517 / 1,519 / 1,518 / 1,526);
- time ≤ 1.x, by alternating binaries.

The upstream figure is the harder one: the MF2 side must drop at least 299
allocations a frame. Inference, from A4: its frame fell by 45 %, and the frame
also holds Ratatui's own allocations, which C5 measures apart. Fallback: a
reusable-buffer API.

## 9. The locale matcher

One matcher for everything that matches a language (D21):
- `install()`, over the system's list;
- `Locale::from_str`, one tag;
- `Locale::best_match`, a list;
- the web negotiator, over `Accept-Language`;
- the client-only boot, over `navigator.languages`.

The rules follow CLDR 48's `languageMatching` data and UTS #35 Part 1 §4.3–§4.4,
as C3's text half read them. No project rule sits on top (question 15).

**Stated here** (C3's text half, "For A8 and C3"):
- **the threshold:** a match only when the weighted distance is **below 50**.
  That is the top of the text's range; for whole-number distances any threshold
  between 49 and 50 behaves the same;
- **the demotion:** **5 for each later entry** in the reader's list. It is
  counted against the threshold, so an exact match can come from at most the
  10th entry (45); the 11th (50) is refused and the reader gets the next
  choice, else the source language. **This design states the demotion and does
  not bound it**, since it follows the text's arithmetic and the case is rare.
  C3 tests it;
- **ties** go to the earlier pair. Among one reader entry's candidates, a
  paradigm locale wins (§4.4.1);
- **a macroregion is inside a match variable when all its contents are** (the
  reading that reproduces the text's three `es-419` examples);
- **a desired `und` is not maximized**;
- **a tag likely subtags cannot fill keeps its empty fields**;
- **POSIX names are read as tags:** `fr_CA.UTF-8` is `fr-CA`. `C`, `POSIX`, `*`
  and an empty value never match.

**What readers get** (C3's evidence):
- Serbian Latin and Cyrillic are served for each other (5);
- Punjabi's two scripts are refused (54);
- Spanish regions fall back as the owner described: `es-MX` gets `es` (5), and
  `es-419` (4) when the application has it;
- `zh-Hant-TW` finds `zh-TW` (0), and `zh-HK` finds `zh-TW` (5);
- Traditional and Simplified Chinese are refused both ways (54), as is `zh-TW` ↔
  `zh-CN`.

**What changes from 1.x:** `zh-Hant-TW` and `zh-TW` no longer reach an
application's `zh` by truncation, and the web's "any locale of the same
language" step goes (question 15).

**Left to C3, each with a test:**
- `$!X` for a macroregion that straddles a variable (`en-001` against `$enUS`);
- the text's worked examples, all but its opening illustration (C3's text
  half).

**The client carries only the corpus's languages.** The rules whose supported
side is one of them, or `*`, and the likely subtags they need. C3 measures its
cut against B1. Server builds carry the whole table; a native build carries
its corpus's cut too, which gives it the same answers (C3, below).

**As built (C3;** [18](18-phase-10-work-order.md), C3's entry**).** Where the
design above did not settle a detail, or C3 measured a better way to meet it:
- **The data.** `cargo xtask locale-data` extracts the three vendored files
  into `mf2-locale-data`'s `data/matching.txt` (the likely subtags without
  `und`, the paradigm locales, the variables with the regions inside each,
  the 378 rules in the data's order) and packs the whole of it into `mf2`'s
  `src/matching/cldr.rs`; the drift test holds both to `third_party/`. The
  packing turns the first two levels' rules into ordered pairs (the first
  rule in the data's order decides a pair, a two-way rule gives both) and
  checks what that rests on: only each level's default has a `*` there.
- **`mf2::LanguageMatching`** is the table's type, a build fact like
  `Corpus`: its constructor and its matching methods are hidden, for the
  generated code and the tests. **`mf2-build` generates the corpus's cut,
  `LANGUAGE_MATCHING`**, beside `LOCALES`, from the tags alone, so a
  translation edit leaves it as it is. A cut gives the corpus's locales, for
  any reader, exactly what the whole table gives (a test checks eight
  corpora against every tag the table fills).
- **Who carries what.** A server (`host-std`) matches with the whole table,
  since `Negotiator::over` and `lookup_locale` take any locales. **A native
  application matches with its corpus's cut, not the whole table:** the
  generated `CORPUS` carries it, and a native application only ever matches
  against its own corpus, for which the answers are the same. The whole
  table cost `tui-mf2` +38,008 B stripped, the cut +4,568 B (C3's entry).
  A browser's client carries the cut when its setup does:
  `Setup::with_language_matching(&LANGUAGE_MATCHING)`, for a client-only
  application (routed to C4: the generated `setup()` passes it under `csr`).
  A hydrated page's client never matches, since it takes the server's
  choice, and carries none.
- **Without a table** (a setup written by hand without it, a corpus built by
  hand), the matcher runs with no data: nothing is filled in and only the
  three default distances apply, so a tag finds its own language's locales
  (`fr-CA` finds `fr`), the first in the build's order among several, and
  no other script or language.
- **Small readings, each with a test:** a value that is no tag (`*`, `C`,
  `POSIX`, empty) takes no place in the reader's list; `Zzzz` and `ZZ` are no
  script and no region (§4.3's canonicalization); an extended language
  subtag is skipped (`zh-yue-HK` reads as `zh-HK`); what follows the region
  (variants, extensions, private use) and a POSIX `@modifier` are ignored;
  `und` is filled in on neither side — a reader's by the text's rule, an
  application's because none offers text in no language, so the table
  leaves `und`'s likely subtags out.
- **The text's worked examples** are tests, each with its section; the
  opening illustration is a test too, of the default it gets (54).
- **Native:** choosing a language allocates nothing (a test counts); 1.x
  copied the tag into a `String` on every `set_locale` and `with_locale`.
- **The web's negotiation** matches each source's candidates as one list
  (`Accept-Language` in quality order), with the demotion, rather than one
  candidate at a time; the client-only boot matches `navigator.languages`
  so too.

## 10. The generated module

`mf2::include_generated!()` at the crate root includes one file. Its text is
the same in every build except for what the build script decides; its
compile-time choices go through `mf2`'s cfg-forwarding macros, at item level
(A2).

| Item | always | `native` | `ssr` | `hydrate` / `csr` | `axum` | `ratatui` |
|---|---|---|---|---|---|---|
| `tr!`, `msg_id!` (§12), `prelude` | ✓ | | | | | |
| `enum Locale`: `ALL`, `SOURCE`, `tag()`, `dir()`, `FromStr`, `Display`, `best_match()` | ✓ | | | | | |
| `Locale::name()` — the `language.<tag>` message, when every language has one | ✓ | | | | | |
| `Locale::format(&message) -> String`, in that language | | ✓ | ✓ | — | ✓ | |
| `install()` | | ✓ | ✓ | ✓ | ✓ | |
| `install_from_directory(dir) -> Result<(), mf2::native::Error>` | | ✓ | | | | |
| `set_locale(Locale)` | | ✓ app-wide | ✓ does nothing | ✓ switches, spawned | | |
| `preload_locale(Locale)` | | | ✓ does nothing | ✓ | | |
| `current_locale() -> Locale` | | ✓ this thread's | ✓ the request's | ✓ reactive | | |
| `with_locale(Locale, body) -> R` | | ✓ | | | | |
| `setup()` | | | ✓ | ✓ | | |
| `impl FromRequestParts for Locale` | | | | | ✓ | |
| `markup::*` | | | | | | ✓ |
| the build's facts: `MANIFEST_HASH`, `SOURCE_LOCALE`, `LOCALES`, `LANGUAGE_MATCHING` (C3, §9), `registry()`, `host`, `CORPUS` / `CATALOGS` | ✓, documented as such | | | | | |

**`Locale`:**
- **variants** are the tags in UpperCamelCase (`pt-BR` is `PtBr`). The build
  refuses two tags that give one name;
- **`ALL`** is in tag order, as `LOCALES` is;
- **`FromStr`** goes through the matcher, and it is client-path code: the
  browser's boot reaches it. Its error, `mf2::UnknownLocale`, lists the
  supported tags and allocates nothing;
- **`Display`** writes the tag;
- **with `clap`**, `Locale` gets a value parser that parses through the matcher
  and lists the tags in `--help`. clap's `ValueEnum` would have been the other
  way; its exact match refuses `--lang fr_CA.UTF-8`;
- **with `axum`**, `Locale` is an extractor: `async fn hello(locale: Locale) ->
  String { locale.format(&tr!("hello")) }` gets the language the `Negotiator`
  layer chose, or negotiates with the defaults when there is no layer.

**One item per combination.** Under `ssr` with `native`, `install()` does both
sides' work from one embedded byte table, shared by `CATALOGS` and `CORPUS`.
`set_locale` and `current_locale` follow the lookup's order (§5).

**The prelude** is the generated one; `mf2` has none. It holds `tr`, `msg_id`,
`Locale`, `set_locale`, `current_locale`, `with_locale` and `preload_locale`
where they exist, and the description types (`Tr`, `TrArgs`, `TrRich`,
`TrDyn`) for signatures. `install` and `markup` stay out: they are named once.
A3's rules hold: `tr` in `mf2`'s own namespace would be ambiguous with the
generated one.

**Doc comments fit the mode**: a native module never says "wasm".

**The generated names are fixed.** An application whose root already defines
`install`, `setup`, `Locale` or `markup` gets E0428 at the include. A way to rename them
(`mf2.toml`) is left for later, when an application needs one (§16).

**As built (C4;** [18](18-phase-10-work-order.md), C4's entry**).** Where the
design above did not settle a detail, or could not be built yet:
- **the macros** are `mf2::__generated`'s; `CATALOGS` follows `mf2`'s
  `host-std`, not `ssr`, since a 1.x crate's `ssr` may forward only
  `mf2/host-std` (L5's do). A `Both` module gains `CORPUS` with `native`;
- **`Locale::best_match`** returns `Option<Locale>`; `from_str` in a hydrated
  page takes an exact tag (ASCII case aside) and links none of the matcher;
- **`markup::*`** holds a constant for each name whose ASCII letters and digits
  give one no other name gives; the rest (MF2 allows `+:_¡`, which L5 uses)
  get none rather than a refusal;
- **`setup()`** came after C4 ([18](18-phase-10-work-order.md) questions 30
  and 31): `install()` installs it, and every hand-written one in the tree
  went in the same change, since it would clash with the generated one;
- **not built:** `Locale::format` without `native`, and the `axum` column,
  with D1; `tr` and `msg_id` in the prelude, which need C6's wrapper
  (rust-lang/rust#52234).

## 11. The build

D19, as A2 adopted it:
- **`mf2` has a build script and `links = "mf2-v2"`**. Its script prints
  `cargo::metadata=features=…` (sorted, comma-separated, defaults left out),
  `target=…` and `version=…`;
- **`mf2_build::run()` is the whole build script.** It reads
  `DEP_MF2_V2_FEATURES`. If the variable is absent, the crate does not name
  `mf2` as a normal dependency, which the include needs: `run()` fails and says
  so. There is no fallback to the crate's own `CARGO_FEATURE_*`, since a crate
  that includes the module always names `mf2`. It refuses an `mf2` whose
  `version` is not its own;
- **`run()` prints `cargo::warning=` and `cargo::error=`**, and exits non-zero on
  an error, so a build script needs no `Result`.

**What it emits**, from `mf2`'s features:

| `mf2` has | The build writes |
|---|---|
| `native` | the module; the catalogs, embedded, uncompressed (`CORPUS`) |
| `ssr` (a web server) | the module; the catalogs, embedded, with `.br` and `.gz` (`CATALOGS`) |
| `ssr` and `native` | as `ssr`, with one byte table shared by `CATALOGS` and `CORPUS` |
| `axum` without a Leptos mode | as `ssr` |
| `hydrate` | the module only; the server's build embeds the catalogs |
| `csr` | the module only; `mf2 compile --site` publishes the catalogs (04 §8) |
| no mode | the module only (a library that returns descriptions) |

**Compression** is only for a web server. A debug build compresses at a fast
level, which C6 picks by measurement (review #18: maximum-quality brotli in a
debug build cost seconds per edit). A release build compresses at the maximum.
As built (C6): brotli 5 and gzip 1 in a debug build (the brotli CLI on a 197 KB catalog: quality 5
in 3 % of 11's time, for 12 % more bytes), from `Build::new()`, so in any build script.

**`mf2.toml` is optional**, and its defaults are 1.x's `Config::default()`. The
build prints `rerun-if-changed` only for a file that exists (A3 found a missing
`mf2.toml` rebuilt the crate every time).

**Checks** C6 adds:
- **`datetime-icu` needs `icu-blob` on the build side.** When `mf2` has
  `datetime-icu` and `mf2-build` lacks `icu-blob`, `run()` fails with "mf2-build:
  `mf2` has `datetime-icu`, whose catalogs carry ICU4X's date data: add
  `features = ["icu-blob"]` to this crate's `mf2-build` build-dependency". A
  build-dependency's features cannot travel through `links` (A2). `icu-blob`
  stays opt-in, because it compiles ICU4X's data crates into the build script;
- **`mf2 check` and `mf2 compile --site` read `mf2`'s node** in the cargo
  resolve (A2, row 11), plus `--features`. A failed `cargo metadata` says so,
  and turns no function into a false `gated-function` error (A1's finding).
  As built (C6): `--features`, given, still names the whole set; without cargo's answer, `check`
  checks as if every function were on.

**One crate is the default** for native and web applications (A6). Two crates
are for a corpus several crates share (§1.3).

**The edit loop:**
- `mf2 init` writes `[profile.dev.build-override] opt-level = 2` into a new
  application, and prints it for an existing one (review #18, C7);
- a cargo-leptos application keeps `watch-additional-files = ["locales"]`
  (A6), which `mf2 init --ssr` writes (D5).

## 12. `tr!` in its own crate

A3's shape (3c): the generated module exports the wrapper under a hidden name,
and names it `tr` with a `use`; the same for `msg_id`. Then:

```rust
#[doc(hidden)] #[macro_export] macro_rules! __mf2_tr { … }   // path and hash baked in, as before
pub use __mf2_tr as tr;
pub mod prelude { pub use super::tr; /* … §10 */ }
```

- **In the crate that includes the module** — a one-crate application, or a
  library with messages of its own — any module, declared before or after the
  include, writes `use crate::prelude::*;` (or `use crate::tr;`, or
  `crate::tr!(…)`). At the root, after the include, `tr!` needs no import. A
  module that forgets gets rustc's own suggestion, `use crate::tr;`, which
  compiles.
- **In a crate that depends on it**, `my_lib::tr!(…)`, `use my_lib::tr;`, or
  `use my_lib::prelude::*;`, as in 1.x.
- **Three rules keep the name unambiguous** (A3, variants 3e and 5, and 2):
  - the module defines no textual `macro_rules! tr` beside the re-export;
  - `mf2` has no prelude with a `tr` in it;
  - `macro_expanded_macro_exports_accessed_by_absolute_paths` is never allowed.

**Upgrading from 1.x:** a module declared after the include that called `tr!`
unqualified now imports it.

## 13. What the design keeps open

Method §6's list:
- **Hot reload.** The store reads its catalog table through one pointer. A
  later `mf2 watch` push can swap in new `&'static` catalogs, leaking the old
  ones, in development only. Nothing in 2.0's API takes a catalog's lifetime
  from the application.
- **Editor tooling.** The manifest's path and hash are still baked into `tr!`,
  so rust-analyzer expands it (A3, A6). `cargo metadata` carries `mf2`'s
  features (A2).
- **Servers without Leptos.** `axum` alone gives the negotiation layer, the
  catalog routes, `Locale` as an extractor, and `Locale::format`. A
  per-request ambient language for plain Axum (so `{}` works in a handler)
  is later: a task-local the layer could set.
- **Per-route catalogs.** The catalog format and the `MsgId` chunk bits are
  untouched (D6).
- **Custom functions.** `mf2.toml`'s `[functions]` is unchanged. The `Function`
  trait's ergonomics are later.
- **Several message sets in one process.** 2.0 refuses a second corpus in the
  store (`install()` names the reason). The store is keyed by
  `&'static Corpus`, so a later version can hold several. `Catalogs` serves
  several today, explicitly.

The master plan's "Kept open" items stay open:
- **RTL alignment in terminals:** `Locale::dir()` is there for it;
- **column widths across locales;**
- **embedded pseudo-locales.**

## 14. The gates

Method §3's table, made concrete. Every size A/B is taken within one tree and
one `Cargo.lock` (A4). A move outside ±64 B gz is read beside the raw bytes at
both scales, and a function-level diff, before any fallback (A5).

| Change | Baseline | Gate | Fallback |
|---|---|---|---|
| The merge (B1–B4, D1) | A1's figures, re-measured in the same tree and lock | B1 within ±64 B gz; B5 within ±0.2 B a site; B7 catalogs byte-identical; B12 clean | revert the offending impl |
| Helper crates and the function table (B1) | A7 | the above, and the demos' shipped wasm (demo-ssr, demo-csr, demo-islands) within ±64 B gz; e2e green on both Leptos lines | static dispatch; then back to the owner with question 13's other options |
| `Display` (S3) and `Debug` (S2) on the moved types (B1, C2) | A9's `a5` library | no `Display`, `Debug` or `core::fmt` of ours in a client that formats nothing (A5's twiggy check); `{}` on a `Tr` over `.to_string()` ≤ 70 B gz in demo-ssr; `{:?}` on a `TrArgs` ≤ 1.3 KB gz in every client A9 measured and demo-islands, over a base that already formats text (a client that formats nothing else is measured over a `format!` control; question 18) | back to the owner (question 14 decided both) |
| Arguments (C1) | 1.x's `ArgValue::from` | `b5 --view` unchanged; every 1.x argument type still accepted; the trybuild message at the argument; each `IntoArg` type exact (tests through `compile_str`) | per-type `IntoArg` only, the message naming `.to_string()` |
| The native store (C2) | 1.x `NativeI18n`; the port's `RefCell`; A1's kept 1.x binaries | time per frame ≤ 1.x (alternating binaries, `tui-gate --baseline`); stripped `tui-mf2` ≤ 1,965,320 B; B10 allocations ≤ 1.x | the explicit `Catalogs` path; drop the thread override |
| The matcher (C3) | both matchers' tests | every case passes, except those questions 11 and 15 change, each listed; the text's examples; the client's table against B1 within ±64 B gz | the client uses the server's choice |
| The generated module (C4) | 1.x's codegen | `codegen-matrix` with the native combinations; L5 unchanged; `scenarios`: the wasm byte-identical across a translation edit | — (a design error goes back to §10) |
| Ratatui (C5) | 1.x `mf2-ratatui`; `upstream.rs` | allocations per frame ≤ both (§8); time ≤ 1.x | a reusable-buffer API |
| `links` (C6) | today's forwarding | every A2 scenario, on the real crates; the wasm byte-identical across a translation edit | function features stay on a translation crate |
| In-crate `tr!` (C6) | textual scope | A3 passes under cargo and rust-analyzer | textual scope, documented, with a clear error |
| The server layer (D3) | the `_with_context` e2e results | e2e green without the context (`demo`, `lazy`, `csr`, `islands`, `a11y`) | keep the context wiring: §1.4's `main.rs` then keeps 1.x's context closure, +7 setup lines and 3 changed ones |
| The UX table (C8, D6) | A1's 1.x column | each row at or below §2's target | back to this document |

## 15. Choices this design made that no answer settled

**Reviewed by the owner (2026-09-28, 18 question 17): all seventeen stand.**
The first two were put to the owner by name and confirmed; the rest were
approved as written. The ones to look at first came first.

1. **Any `Display` type is an argument, as its text** (§7). The owner's rule
   says to keep the ergonomic API when it can be made cheap, and this costs
   nothing where it is not used; the probe shows the mechanism and the error.
   The risk is an English `Display` inside a translated sentence, which 1.x's
   compile error did not prevent either.
2. **`install()` returns nothing** (§5). 18's sketch had `install()?`. Embedded
   catalogs from the same build cannot fail to load; a failure means a corrupt
   executable, and it panics naming the catalog.
   `install_from_directory` returns a `Result`.
3. **`<LocaleSwitcher/>` with no children lists every language** by its
   `language.<tag>` message (§1.4; 04 §12.3). This folds D4's "options
   component" into the switcher, and `Locale::name()` gives a TUI the same
   names.
4. **A Leptos mode needs a line.** `features = ["leptos"]` on the dependency,
   `"mf2/ssr"` in the mode; `leptos` is not a default feature, and `ssr` alone
   is a `compile_error!` naming both lines (§3). That keeps 0.8 applications
   free of `default-features = false`.
5. **`Negotiator` is itself the tower layer**: `.layer(Negotiator::default())`,
   the tower-http idiom, where the configuration is the layer (04 §12.5). D3
   probes whether the render can read its result.
6. **What "every row falls" means for commands** (§2). A count of one cannot
   fall. A row falls when setup lines, crates and concepts fall, commands do
   not rise, and fewer translation files are written by hand. The pages are
   counted along their hand-written path, with `mf2 init` as the shortcut.
7. **`hello` turns on `fn-number` only** (§2). 1.x's page also turned on two
   date features its messages do not use; with them the target is +2
   concepts, +0 lines.
8. **The `clap` feature parses through the matcher** and lists the values in
   `--help`, rather than clap's `ValueEnum`, which would refuse `--lang
   fr_CA.UTF-8` (§10).
9. **`Locale` is an axum extractor** with `axum` (§10), for plain Axum
   handlers.
10. **Only the generated prelude** (§10): `tr`, `msg_id`, `Locale`, the language
    functions, and the description types. `mf2` has none.
11. **`Display` pads `to_cow()`** (§6): natively a simple message's `{}`
    allocates nothing. A message with arguments allocates one `String`, where
    A4's streaming `Display` allocated none. That is the single text path
    question 14 chose, applied natively.
12. **`Debug` on every public type**, the runtime's and the catalog's
    included (§6), with `missing_debug_implementations` on.
13. **The demos keep a nightly `fmt-check`** (§6), failing on any `Display` or
    `Debug` of ours. The demos model the book's rule.
14. **The theme's defaults** (§8): `code` and `kbd` reversed. Bold would have
    been the other choice, but `b` is already bold.
15. **The matcher's demotion is stated, not bounded** (§9). An exact match
    11th or later in a reader's list is refused, as the text's arithmetic
    gives.
16. **`with_locale` and `Locale::format` work before `install()`** (§5). They
    load the embedded catalogs themselves, so tests and servers need no global
    set-up; only the app-wide forms panic before `install()`.
17. **`run()` without `links` metadata fails** (§11), rather than falling back
    to the crate's own features.

## 16. Routed to later tasks

- **B1:** `Debug` on the moved types, written as S2 (§6); the per-mode
  listings' shape for the helpers' `*Props` (A7). *Done*
  ([18](18-phase-10-work-order.md), B1's record): `Debug` as S2 and `Display`
  as S3 on the moved types; the `*Props` listed, as aliases in `mf2`'s
  listing.
- **B2 / D1: no clash** (owner, [18](18-phase-10-work-order.md) question 23).
  `mf2::native::LocaleSource` (an enum: system, source, explicit) and
  `mf2::axum::LocaleSource` (a trait) were in two crates and now share one, but
  items in different modules of one crate keep their names: they never share a
  scope. They meet only where one scope glob-imports both modules and writes the
  bare name (rustc asks for the path), or in a prelude that carries both, which
  neither needs. B2's rename to `LocaleOrigin` is undone. **The rule for every
  merge:** rename only for a clash in one scope, such as A7's root module named
  like a dependency crate (N1–N5).
- **C1:** §7 as designed, the probe as its starting point. *Done*
  ([18](18-phase-10-work-order.md), C1's record): §7, with four steps where
  it had three, and what §7's "As built" lists.
- **C2:** §5, §6. It times the lookup's first step with `ssr` and `native`
  unified (A4); it chooses how a rule-following time zone is carried; it adds
  `Debug` to the runtime's and the catalog's public types. *Done*
  ([18](18-phase-10-work-order.md), C2's record): §5's "As built"; the first
  step costs a native format about 6–7 ns (under load); a POSIX TZ rule in
  the runtime's `TimeZone`, evaluated by the host; `Debug` on the 31 types.
- **C3:** §9. `$!X` for a straddling macroregion, and a test for the stated
  demotion. *Done* ([18](18-phase-10-work-order.md), C3's entry): §9's "As
  built"; `en-001` is in `$!enUS` (3 to `en-GB`); an exact match 11th in a
  reader's list is refused; native builds carry the corpus's cut, not the
  whole table.
- **C4:** §10. The generated names' collisions, and a way to rename them if an
  application needs one.
- **C5:** §8. Collecting descriptions into a `Line` flattens their markup: the
  rustdoc and the book say so.
- **C6:** §11. The fast debug compression level, measured.
- **C8:** the three native pages from §1.1–§1.3; `examples/tui` as §1.2
  describes; the UX recount.
- **D2–D4, D6:** 04 §12; the demos' nightly `fmt-check` (§6).
- **F:** the book's `Display` / `Debug` rule (§6); a `Display` argument's text
  is not translated, and a string selector is the MF2 way (§7); a styled line
  as one message (§8).
- **Unchanged, still open:** an argument shorthand, `tr!("id", error)` for
  `error = error`, is not in any task; it would be a macro change after 2.0.

## Prior art: trippy

*C9 writes this section: call sites and keys, lines added and removed against
upstream, stripped size and build time against upstream, each upstream bug
class with the compile error that now catches it, and what did not fit
(18, C9).*
