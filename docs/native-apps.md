# Native CLI and TUI applications

The call sites `tr!` builds are Leptos-free values: a command-line tool or a
terminal UI formats them as well as a web page does. Two crates make that
comfortable:

* `mf2-native` loads the catalogs, picks the reader's language from the
  system, keeps the active locale in a value the application owns, and
  formats messages.
* `mf2-ratatui` turns a message into [Ratatui](https://ratatui.rs) text,
  with the message's markup as styles. It is optional.

This page builds one small application that does both. `cargo xtask docs`
compiles it, with and without its `tui` feature.

## The translation crate

As on the web, the messages live in a translation crate whose build script
runs `mf2-build`. A native application asks for `Emit::Native`: the
catalogs are embedded in the executable, the module formats through the
native host, and everything the application needs is one generated value,
`CORPUS`.

```toml file=native/i18n/Cargo.toml
[package]
name = "native-demo-i18n"
version = "0.1.0"
edition = "2024"
build = "build.rs"

# As on the web, the functions a message may use are this crate's
# features; a native application simply turns them on by default.
[features]
default = ["fn-number"]
fn-number = ["mf2/fn-number"]

[dependencies]
mf2 = { version = "1", features = ["host-std"] }

[build-dependencies]
mf2-build = "1"
```

```rust file=native/i18n/build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    mf2_build::Build::new()?
        .emit(mf2_build::Emit::Native)
        .emit_cargo(true)
        .run()?
        .into_result()?;
    Ok(())
}
```

```toml file=native/i18n/mf2.toml
source_locale = "en"

[catalog]
missing = "fallback"
```

```rust file=native/i18n/src/lib.rs
mf2::include_generated!();
```

`CORPUS` holds the source locale, the manifest hash, the locale table, the
function registry and the catalogs. The crate also exports `tr!`, as it
does for a Leptos application.

The messages use markup — `{#ok}…{/ok}` — to mark a stretch of text
without saying what it looks like. The terminal UI decides that below.

```mf2 file=native/i18n/locales/en/main.mf2
@locale en
---

welcome = Welcome to the CLI and TUI example.
title = Network status
status = {#ok}Connected{/ok} to {#host}{$host}{/host}: {$sent :number} probes sent.
```

```mf2 file=native/i18n/locales/fr/main.mf2
@locale fr
---

welcome = Bienvenue dans l'exemple CLI et TUI.
title = État du réseau
status = {#ok}Connecté{/ok} à {#host}{$host}{/host} : {$sent :number} sondes envoyées.
```

## The application

The application depends on `mf2-native` and the translation crate, and on
Ratatui only when its `tui` feature is on, so a command-line-only build
does not compile it.

```toml file=native/Cargo.toml
[package]
name = "native-demo"
version = "0.1.0"
edition = "2024"

[workspace]
members = [".", "i18n"]
resolver = "3"

[features]
default = []
tui = ["dep:mf2-ratatui", "dep:ratatui"]

[dependencies]
clap = { version = "4", features = ["derive"] }
mf2-native = "1"
native-demo-i18n = { path = "i18n" }
mf2-ratatui = { version = "1", optional = true }
ratatui = { version = "0.30", default-features = false, optional = true }
```

### Choosing the language

`NativeI18n::embedded` loads every catalog of the corpus and checks each
one against the build. It selects the first of the system's preferred
languages that the corpus supports — `fr_CA.UTF-8` finds `fr` — and the
source locale when none is. `locale_source()` says which happened.

A language the user names explicitly goes through `set_locale`, which
returns an error for a language the corpus does not have, so a mistyped
`--lang` is reported rather than ignored.

```rust file=native/src/main.rs
use clap::Parser;

#[derive(Parser)]
struct Args {
    /// The language to use instead of the system's.
    #[arg(long)]
    lang: Option<String>,
    /// Read the catalogs from this directory instead of the executable.
    #[arg(long)]
    catalogs: Option<std::path::PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let corpus = &native_demo_i18n::CORPUS;
    let mut i18n = match &args.catalogs {
        Some(dir) => mf2_native::NativeI18n::from_directory(corpus, dir)?,
        None => mf2_native::NativeI18n::embedded(corpus)?,
    };
    if let Some(lang) = args.lang.as_deref() {
        i18n.set_locale(lang)?;
    }
```

`available_locales()` lists the supported tags in build order, for a
`--list-locales`, and `set_locale` can be called again at any time: the
locale is a field of `NativeI18n`, not a process or thread global.

### Formatting

`format` returns the message as a `String` in the active locale.
`format_with_errors` also returns the MF2 errors, for a tool that wants to
log them.

```rust file=native/src/main.rs
    println!("{}", i18n.format(&native_demo_i18n::tr!("welcome")));
    Ok(())
}
```

Two settings differ from the web:

* **Bidi isolation is off.** MF2's default wraps each placeholder in
  invisible isolation characters (U+2068 … U+2069), which terminals and
  logs tend to show as stray characters. An application that shows
  right-to-left text in a terminal that applies them can turn them on with
  `set_bidi(BidiStrategy::Default)`; `dir()` gives the active locale's
  direction.
* **Dates use the system's time zone** — its IANA name when it has one,
  else its current offset, else UTC. `set_time_zone` changes it.

### Catalogs outside the executable

With `Emit::NativeFiles` instead, the build writes the catalogs to its
output directory under content-hashed names and embeds none of them; the
application ships the `.mf2b` files and loads them with
`NativeI18n::from_directory`, as `--catalogs` does above. The corpus
records each file name, and each file is checked against the build when it
is loaded, so a catalog from another build is an error, not wrong text.

## Ratatui

`mf2_ratatui::line` and `mf2_ratatui::text` format a message into
Ratatui's own `Line` and `Text`, which any widget takes. The application
maps each markup name to a style; the translation decides where the styled
stretch goes, so the French message above can move it without any change
to the code. Nested elements combine their styles, and a name with no style
is plain text.

```rust file=native/src/lib.rs
#[cfg(feature = "tui")]
pub mod tui {
    use mf2_native::NativeI18n;
    use mf2_ratatui::MarkupStyles;
    use ratatui::style::{Color, Style};
    use ratatui::widgets::{Block, Paragraph};

    pub fn status(i18n: &NativeI18n, host: &str, sent: u32) -> Paragraph<'static> {
        let styles = MarkupStyles::new()
            .with("ok", Style::new().fg(Color::Green).bold())
            .with("host", Style::new().underlined());
        let title = mf2_ratatui::line(i18n, &native_demo_i18n::tr!("title"), &styles);
        let status = mf2_ratatui::text(
            i18n,
            &native_demo_i18n::tr!("status", host = host, sent = sent),
            &styles,
        );
        Paragraph::new(status).block(Block::bordered().title(title))
    }
}
```

In English the paragraph reads "Connected to example.org: 1,204 probes
sent.", with "Connected" green and bold and the host underlined. An event
handler that calls `i18n.set_locale("fr")` changes the next draw.

`text` starts a new line at each line break in the message; `line` keeps
the message on one line. The adapter never writes bidi isolation
characters: Ratatui places every character itself, so a terminal has
nothing to reorder.
