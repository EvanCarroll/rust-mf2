# Upgrading from 1.x

2.0 is not source-compatible with 1.x. Five crates became one, several
paths moved, and code that 1.x asked the application to write is now
generated. This page lists each change with its 1.x form and its 2.0 form,
so an upgrade is a list of edits. The
[changelog](https://github.com/EvanCarroll/rust-mf2/blob/main/CHANGELOG.md)
has every change in detail. The manifests name 3, the current release:
the forms marked 2.0 are what it accepts, and the changelog says what 3.0
changed since.

Blocks marked **1.x** show the old code for comparison and are not
compiled. The 2.0 forms are compiled: the command-line tool below here, and
each web file on the page this one links to, where it appears whole.

## One crate

An application names `mf2` and `mf2-build`, nothing else of this project.
Each 1.x crate is a feature of `mf2` now:

| 1.x crate | 2.0 |
|---|---|
| `leptos-mf2` | `mf2`'s `leptos` (Leptos 0.9) or `leptos-0-8`, and one mode: `ssr`, `hydrate` or `csr` |
| `mf2-axum` | `mf2`'s `axum` |
| `mf2-native` (1.1.0, never published) | `mf2`'s `native` |
| `mf2-ratatui` (1.1.0, never published) | `mf2`'s `ratatui` (which turns on `native`) |
| the translation crate (`hello-i18n`) | gone: the application's own crate holds `locales/` and the build script |

A mode turns the Leptos layer on; `leptos` alone chooses the line. Two
modes, both lines, or a mode with no line is a compile error that says what
to write. `native` or `ratatui` beside `hydrate` or `csr` is refused only in
a browser build (`wasm32`), so a workspace holding a browser client and a
terminal UI still checks as one.

**1.x**, a server-rendered application and its translation crate:

```toml excerpt
[workspace]
members = [".", "i18n"]

[dependencies]
hello-i18n = { path = "i18n", features = ["fn-number"] }
leptos-mf2 = "1"
mf2-axum = { version = "1", optional = true }

[features]
hydrate = ["leptos-mf2/hydrate", "hello-i18n/hydrate"]
ssr = ["leptos-mf2/ssr", "hello-i18n/ssr", "dep:mf2-axum"]
```

**2.0**, one crate. What formats numbers and dates is `mf2`'s features,
written once where both builds see them, each side naming its own; the mode
and `axum` are forwarded as `leptos`'s are:

```toml excerpt
[dependencies]
mf2 = { version = "3", features = ["leptos", "leptos-client-number-intl", "leptos-server-number-builtin"] }

[build-dependencies]
mf2-build = "3"

[features]
hydrate = ["leptos/hydrate", "mf2/hydrate"]
ssr = ["leptos/ssr", "mf2/ssr", "mf2/axum"]
```

Move `i18n/locales/` to `locales/`, `i18n/mf2.toml` (if any) beside
`Cargo.toml`, and delete the translation crate. `watch-additional-files`
becomes `["locales"]`. [Getting started](getting-started.md#the-manifest)
shows the whole manifest.

## The build script

**1.x**:

```rust excerpt
fn main() -> Result<(), Box<dyn std::error::Error>> {
    mf2_build::Build::new()?
        .emit(mf2_build::Emit::Native)
        .emit_cargo(true)
        .run()?
        .into_result()?;
    Ok(())
}
```

**2.0**: one call. What the generated module holds follows the features
`mf2` is built with, so a native application no longer asks for
`Emit::Native`:

```rust excerpt
fn main() {
    mf2_build::run();
}
```

`mf2_build::Build` remains for a build that needs more, such as
`Emit::NativeFiles` for catalogs shipped beside the executable.

## Paths

| 1.x | 2.0 |
|---|---|
| `leptos_mf2::…` | `mf2::leptos::…` |
| `mf2_axum::…` | `mf2::axum::…` |
| `mf2_native::NativeI18n` | the generated `install()` and the locale functions; `mf2::native::Catalogs` for the explicit form |
| `mf2_native::NativeError` | `mf2::native::Error` |
| `mf2_ratatui::…` | `mf2::ratatui::…` |
| `hello_i18n::tr!` | `tr!`, at the crate's root, where `mf2::include_generated!()` puts it |

## `setup()` and `install()`

A 1.x translation crate wrote `setup()` by hand, and the application
installed it once on each side. 2.0 generates both `setup()` and
`install()`. **Delete the hand-written `setup()`**: it would clash with the
generated one.

**1.x**:

```rust excerpt
// i18n/src/lib.rs
mf2::include_generated!();

pub fn setup() -> mf2::leptos_mf2::Setup {
    mf2::leptos_mf2::Setup::new(registry(), &host::HOST, MANIFEST_HASH, SOURCE_LOCALE, LOCALES)
}

// the browser's entry point
leptos_mf2::install(hello_i18n::setup());
leptos_mf2::hydrate_body(App);

// the server's main
mf2_axum::install(hello_i18n::setup(), hello_i18n::CATALOGS)?;
```

**2.0**:

```rust excerpt
// src/lib.rs
mf2::include_generated!();

// the browser's entry point
install();
mf2::leptos::hydrate_body(App);

// the server's main
hello::install();
```

A client-only application's generated `setup()` also carries the
language-matching data its browser needs (below); a `Setup` built by hand
with `Setup::new` has none.

## The server: a tower layer

1.x negotiated inside the render: a closure called `provide_locale`, and
every `_with_context` entry point had to be given it. A missed one rendered
a page in the default language with no other symptom. In 2.0 the
`Negotiator` is a tower layer, and the routes and the fallback are Leptos's
plain forms.

**1.x**:

```rust excerpt
let negotiator = Arc::new(
    Negotiator::empty()
        .source(QueryParam::default())
        .source(CookieLocale::default())
        .source(AcceptLanguage)
        .sink(CookieLocale { secure: !cfg!(debug_assertions), ..CookieLocale::default() }),
);
let context = {
    let negotiator = Arc::clone(&negotiator);
    move || {
        mf2_axum::provide_locale(&negotiator);
    }
};
let app = Router::new()
    .merge(mf2_axum::catalog_routes())
    .leptos_routes_with_context(&leptos_options, routes, context.clone(), {
        let leptos_options = leptos_options.clone();
        move || shell(leptos_options.clone())
    })
    .fallback(file_and_error_handler_with_context(context, shell));
```

**2.0**:

```rust excerpt
let app = Router::new()
    .leptos_routes(&leptos_options, routes, {
        let leptos_options = leptos_options.clone();
        move || shell(leptos_options.clone())
    })
    .fallback(file_and_error_handler(shell))
    .layer(Negotiator::default())
    .merge(catalog_routes());
```

[Getting started](getting-started.md#the-server) shows the whole server.

**The default order changed.** 1.x's `Negotiator::default()` read the
cookie, then `Accept-Language`. 2.0's reads `?lang=`, then the cookie, then
`Accept-Language`, and its cookie is `Secure` outside a debug build: the
chain 1.x's pages spelled out by hand. A negotiator built with
`Negotiator::empty()` and `.source(…)` works as before; give it to
`.layer(…)`.

`mf2::axum::path_prefix_redirect` keeps its signature
(`axum::middleware::from_fn`) and reads the query parameter's name from the
layer. The cookie is now written only for an explicit choice (`?lang=` or
the path), not for a guess from `Accept-Language`.

## The switcher

`<LocaleSwitcher/>` with no children lists every language, in the order of
`Locale::ALL`, each named by its `language.<tag>` message. 1.x needed a
`<LocaleOption>` per language; those children still work, for a list of
your own. The helpers that take a language take the generated `Locale`:
`tag=Locale::Fr`. [Switching language](switching.md) has the details.

## `tr!` arguments

An argument converts through `mf2::IntoArg`. Every type 1.x took still
converts, and more do: every integer type exactly (`u64` and `u128` past
`i64` as their exact decimal), `bool` as `true` or `false` for `.match`,
paths, `SystemTime`, jiff's instants and civil dates with `native`, and any
type with `Display`, as its text, which is not translated. A type that is
none of these is a compile error at the argument, naming `IntoArg`. The
table in [Call sites](call-sites.md#arguments) lists them.

An application's own type implemented `From<T> for ArgValue` in 1.x. That
still works; `IntoArg` is the 2.0 form. **1.x**:

```rust excerpt
impl From<Entries> for mf2::ArgValue {
    fn from(entries: Entries) -> mf2::ArgValue {
        mf2::ArgValue::from(entries.0 as i64)
    }
}
```

The 2.0 form is in the program below.

## Native applications

1.1.0's `mf2-native`, never published, kept the catalogs and the language
in a `NativeI18n` value that every format went through. 2.0 installs them once for the process: a description
shows its text wherever text is wanted, and the language is the generated
`Locale`.

**1.x**:

```rust excerpt
let mut i18n = mf2_native::NativeI18n::embedded(&count_i18n::CORPUS)?;
if let Some(lang) = args.lang.as_deref() {
    i18n.set_locale(lang)?;
}
println!("{}", i18n.format(&count_i18n::tr!("files", dir = dir, count = n as i64)));
```

**2.0**, the whole program, on the messages of
[`count`](native-apps.md#a-command-line-tool):

```rust file=upgrade/src/main.rs
//! Counts the files in a directory: `count`, as a 1.x tool reads after
//! the upgrade.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

mf2::include_generated!();

#[derive(Parser)]
struct Args {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Was `Option<String>`, checked by `set_locale`. `Locale` parses
    /// through the language matcher and refuses a language the tool lacks.
    #[arg(long)]
    lang: Option<Locale>,
}

/// An application's own type as an argument: 1.x's `From<Entries> for
/// ArgValue` becomes `IntoArg`.
struct Entries(usize);

impl mf2::IntoArg for Entries {
    fn into_arg(self) -> mf2::ArgValue {
        mf2::IntoArg::into_arg(self.0)
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    // Was `NativeI18n::embedded(&CORPUS)?`: it cannot fail now.
    install();
    if let Some(lang) = args.lang {
        set_locale(lang);
    }
    match std::fs::read_dir(&args.dir) {
        Ok(entries) => {
            // Was `i18n.format(&tr!(…))`: a description is its own text.
            let count = Entries(entries.count());
            println!("{}", tr!("files", dir = &args.dir, count = count));
            ExitCode::SUCCESS
        }
        Err(error) => {
            // An `io::Error` is an argument as its text.
            eprintln!("{}", tr!("unreadable", dir = &args.dir, error = error));
            ExitCode::FAILURE
        }
    }
}
```

The rest of the native API, 1.x to 2.0:

| 1.x | 2.0 |
|---|---|
| `NativeI18n::from_directory(&CORPUS, dir)?` | `install_from_directory(dir)?`, which accepts a partial set: only the source language's file is required |
| `i18n.set_locale("fr")?` | `set_locale(Locale::Fr)`, for every thread; `with_locale(Locale::Fr, …)` for one thread and one scope |
| `i18n.locale()`, `i18n.locale_source()` | `current_locale()`, `mf2::native::locale_source()` |
| `i18n.format(&message)` | `message.to_string()`, `println!("{}", message)`, `.to_plain_string()` (never isolated), `.to_cow()` (borrowed when the message is plain text) |
| `i18n.available_locales()` | `Locale::ALL` |
| several corpora, each with its own `NativeI18n` | one `install` per process (a second corpus panics); `mf2::native::Catalogs::embedded(&CORPUS)?` and `catalogs.format("fr", &message)` for another |

Formatting before `install()` panics in a build whose only mode is
`native`, and the message names `install()`. Beside a Leptos mode it never
panics: a message shows no text until the page's or the request's catalog
is there. Tests need no `install`: `with_locale` loads what it needs
([Testing](testing.md)).

## Ratatui

1.x's `mf2_ratatui::line`, `mf2_ratatui::text` and `MarkupStyles` are
gone. A description converts into a `Span`, `Line` or `Text` itself, and a
`Theme`, set once, styles its markup.

**1.x**:

```rust excerpt
let styles = MarkupStyles::new()
    .with("ok", Style::new().fg(Color::Green).bold())
    .with("host", Style::new().underlined());
let title = mf2_ratatui::line(i18n, &native_demo_i18n::tr!("title"), &styles);
let status = mf2_ratatui::text(i18n, &native_demo_i18n::tr!("status", host = host, sent = sent), &styles);
Paragraph::new(status).block(Block::bordered().title(title))
```

**2.0**: the theme once, at start-up, with a generated constant per markup
name; then each call site as Ratatui text:

```rust excerpt
set_theme(
    Theme::default()
        .style(markup::OK, Style::new().green().bold())
        .style(markup::HOST, Style::new().underlined()),
);

Paragraph::new(Text::from(tr!("status", host = host, sent = sent)))
    .block(Block::bordered().title(tr!("title")))
```

A description is `Styled` as a `Line` (`tr!("quit").bold()`) and a
`Widget`. Keep a styled line in one message: collecting several into one
`Line` flattens their markup. [Native CLI and Ratatui apps](native-apps.md#a-terminal-ui)
shows a whole terminal UI.

## Choosing a language

2.0 has one language matcher, built on CLDR's language-matching data, and
everything that chooses a language uses it: the server, a client-only
application's boot, `install`, `set_locale`, `with_locale` and parsing a
`Locale`. Most choices stay the same; these differ from 1.x:

* `zh-Hant-TW` and `zh-Hant` find an application's `zh-TW`.
* Traditional and Simplified Chinese no longer stand in for each other:
  CLDR has no rule between the two scripts, so a Traditional reader gets
  their next language, else the source language. An application that serves
  both needs a catalog for each ([Troubleshooting](troubleshooting.md)).
* Serbian's Latin and Cyrillic are served for each other.
* A language CLDR says a reader understands is served when theirs is
  missing: Breton readers get French, Catalan readers Spanish.
* Of several regions, the closest wins: an Australian reader gets `en-GB`
  before `en-US`.
* A reader's list is weighed as a whole: `de-AT` first finds `de` before an
  exact match of the second language.

A client-only application carries the part of CLDR's data its languages
need, in the generated `setup()`.

## `[locale_data]`

In 1.x a list of codes under `[locale_data]` changed nothing when a message
took its currency or unit from a variable: the catalog carried every code,
and `dynamic-currency` or `dynamic-unit` warned. In 2.0 the list says which
codes the variable can hold. The catalog carries those (and the ones the
messages name), with no warning; a code outside the list formats with the
code as its symbol. [`mf2.toml`](configuration.md#locale_data) has the
details.

## Checks and commands

* **`mf2 fmt`'s layout changed**: a blank line after the frontmatter's
  `---` and around a message laid out on lines of its own. Run `mf2 fmt`
  once; until then `mf2 fmt --check` reports the files.
* **`dropped-markup` is an error**: a translation that leaves out the
  source's markup fails the build. A corpus that drops markup on purpose
  sets it to `"warn"` under `[lints]` ([Lints](lints.md#dropped-markup)).
* **`@do-not-translate` messages are not missing**, in
  `missing-translation` and in `mf2 stats`'s counts.
* **`mf2 check` uses the crate's features** as cargo resolves them, so it
  reports what the build reports.
* **`mf2 import` checks what it would write**, and writes nothing if that
  brings an error. A JSON import that leaves out ids the language does not
  have yet exits 1: use XLIFF, which adds them.

## Behaviour that changed without an edit

* A language switch that meets another deploy's catalog now reloads into
  the new language, rather than refusing the switch.
* Under a Leptos mode, `Tr`, `TrArgs`, `TrRich` and `TrDyn` implement
  `Display`. `.to_string()` stays the leanest in a browser build; `{}` adds
  a few dozen bytes, and `{:?}` about 1 KB.
* A native application's dates follow the system's daylight-saving rules,
  even when the system zone has no IANA name.
