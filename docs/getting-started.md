# Getting started

This page builds a small server-rendered Leptos application, `hello`,
translated into English and French. The server renders each page in the
reader's language. The browser then hydrates it and switches language
**live, without a reload**. That is the default way to use this library,
and most Leptos applications are built this way. The other delivery modes
(islands, client-only, lazy routes) are on [their own page](delivery-modes.md).
Each of them starts from what you build here.

Every file below is complete. `cargo xtask docs` puts these same blocks
together into this application and compiles it for the server and for the
browser, so what you copy here is what CI builds.

## What you need

* Rust (the 2024 edition), and the browser target:

  ```sh
  rustup target add wasm32-unknown-unknown
  ```

* [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos), which builds
  the server and the client together and serves them:

  ```sh
  cargo install cargo-leptos
  ```

* The `mf2` command, which makes the translation crate and checks
  translations:

  ```sh
  cargo install --path crates/mf2-cli   # from a checkout of this repository
  ```

> **1.1.0 is not published yet.** Five crates are available at 1.0.0, and
> the remaining crates will be published together at 1.1.0. The manifests
> on these pages name them as that release will:
> `mf2 = "1"`, `leptos-mf2 = "1"`, `mf2-axum = "1"`,
> `mf2-build = "1"`. Until then, replace each `"1"` with a path into a
> checkout, for example `mf2 = { path = "../rust-mf2/crates/mf2" }`. This
> is what `cargo xtask docs` does when it compiles these pages.

## The shape of an application

```text
hello/
├── Cargo.toml        the application: a workspace of its own
├── src/
│   ├── lib.rs        the page, and the browser's entry point
│   └── main.rs       the server
└── i18n/             the translation crate: made by `mf2 init`
    ├── Cargo.toml
    ├── build.rs
    ├── mf2.toml
    ├── src/lib.rs
    └── locales/
        ├── en/main.mf2
        └── fr/main.mf2
```

The translations live in a crate of their own, `i18n/`. Its build script
reads `locales/`, checks every message, and writes three things: one binary
catalog per language, a manifest describing all the messages, and a small
Rust module that includes the `tr!` macro. The browser downloads the catalog for
one language when it needs it. **No message text, message id, argument name
or plural rule is compiled into the wasm.** So changing a translation
leaves the wasm byte-for-byte the same, and every reader's cached copy
stays valid.

## The translation crate: `mf2 init`

In `hello/`:

```sh run=hello
mf2 -C i18n init --name hello-i18n --locale fr
```

This writes `i18n/` and prints what to do next (this page covers each
step). The crate's manifest:

```toml file=hello/i18n/Cargo.toml generated
[package]
name = "hello-i18n"
version = "0.1.0"
edition = "2024"

# The functions a message may use are this crate's features, declared
# once: the application's server and client builds both get them, so
# the two always format alike. Turn on what the corpus needs.
[features]
default = []
# The application forwards exactly one of these from its own build.
ssr = ["mf2/host-std", "mf2/ssr"]
hydrate = ["mf2/host-web", "mf2/hydrate"]
csr = ["mf2/host-web", "mf2/csr"]
fn-number = ["mf2/fn-number"]
fn-datetime = ["mf2/fn-datetime"]
datetime-icu = ["fn-datetime", "mf2/datetime-icu", "mf2-build/icu-blob"]
datetime-intl = ["fn-datetime", "mf2/datetime-intl"]
intl = ["mf2/intl"]

[dependencies]
mf2 = "1"

[build-dependencies]
mf2-build = "1"
```

The features decide **which formatting functions exist**. A message can
never add code to the wasm by itself: if a French translation uses
`:datetime` and the crate was not built with `fn-datetime`, the build fails
and names the message. With no features, a message can still use `:string`,
`:number`, `:integer` and plural selection. Numbers then use neutral
symbols (`1234.5`). Turn on `fn-number` for each language's own symbols and
grouping (`1 234,5` in French, grouped with a narrow no-break space,
U+202F), and `fn-datetime` with one date backend for
dates:

| Feature | Adds |
|---|---|
| `fn-number` | numbers in each locale's own symbols, `:percent`, `:currency`, `:unit` |
| `fn-datetime` + `datetime-icu` | `:datetime`, `:date`, `:time` through ICU4X, on the server and in the browser alike |
| `fn-datetime` + `datetime-intl` | the same, through the browser's own `Intl.DateTimeFormat`: a smaller wasm, and the browser's formatting |
| `intl` | numbers and plural rules through the browser's `Intl` too |

Its `src/lib.rs` includes what the build generates. The `setup()` function
at the end is what the application installs, once on each side:

```rust file=hello/i18n/src/lib.rs generated
//! The application's messages. Everything in here is generated: edit
//! `locales/` instead.
//!
//! This brings in `tr!` and `msg_id!` as well. Every *other* crate calls them
//! as `<this crate>::tr!("id", name = value)`; inside this one they are
//! called unqualified, because a `macro_export` macro that arrives through a
//! macro expansion cannot be named by an absolute path in its own crate
//! (rust-lang/rust#52234).

mf2::include_generated!();

/// What the application installs once on each side: the registry, the host,
/// the manifest hash and the locale table the build generated.
#[cfg(any(feature = "ssr", feature = "hydrate", feature = "csr"))]
#[must_use]
pub fn setup() -> mf2::leptos_mf2::Setup {
    mf2::leptos_mf2::Setup::new(
        registry(),
        &host::HOST,
        MANIFEST_HASH,
        SOURCE_LOCALE,
        LOCALES,
    )
}
```

`mf2.toml` configures the build. `source_locale` is the language the
messages are written in first. Every other language's catalog is checked
against it:

```toml file=hello/i18n/mf2.toml generated
source_locale = "en"

[catalog]
strip = [
    "cold",
    "ids",
]
missing = "fallback"

[locale_data]
currencies = "used"
units = "used"
```

`missing = "fallback"` means a message that has not been translated yet
shows in the source language (and `mf2 check` counts how many there are).
`strip` keeps message ids, attributes and comments out of the catalogs the
browser downloads, and `"used"` under `[locale_data]` leaves out the
currency and unit data no message uses.

## The messages

Messages are Unicode [MessageFormat 2](https://www.unicode.org/reports/tr35/tr35-messageFormat.html)
(MF2), in files under `locales/<language>/`. Each file starts by naming its
language. Then comes one message per `id = pattern`, and a `[section]`
prefixes the ids that follow it (`language.label` below):

```mf2 file=hello/i18n/locales/en/main.mf2
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

```mf2 file=hello/i18n/locales/fr/main.mf2
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

`{$name}` is an argument that the code passes in. `.match` chooses a
variant, and `*` is the one that matches anything else. Each language
writes the variants its grammar needs. French has a `many` category (for
numbers such as a million), and the build warns about a category that
no variant names, so the French message names it too. The language
names are messages too. `language.fr` is `Français` in every catalog, so
each language is named in its own language, and the names are catalog data
rather than text in the wasm.

`mf2 -C i18n check` runs every check that the build runs. It also works
without cargo, so you can use it in CI or in a translator's editor.

## The application's manifest

```toml file=hello/Cargo.toml
# A workspace of its own. An application is built twice, once with `ssr`
# and once with `hydrate`, and cargo unifies features across a workspace.
[workspace]
members = [".", "i18n"]
resolver = "3"

[package]
name = "hello"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
hello-i18n = { path = "i18n", features = ["fn-number", "fn-datetime", "datetime-icu"] }
leptos = { version = "0.9.0-beta", default-features = false }
leptos-mf2 = "1"
leptos_meta = "0.9.0-beta"
leptos_router = "0.9.0-beta"

axum = { version = "0.8", optional = true }
console_error_panic_hook = { version = "0.1", optional = true }
leptos_axum = { version = "0.9.0-beta", optional = true }
mf2-axum = { version = "1", optional = true }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"], optional = true }
wasm-bindgen = { version = "0.2", optional = true }

[features]
hydrate = [
    "leptos/hydrate",
    "leptos-mf2/hydrate",
    "hello-i18n/hydrate",
    "dep:console_error_panic_hook",
    "dep:wasm-bindgen",
]
ssr = [
    "leptos/ssr",
    "leptos_meta/ssr",
    "leptos_router/ssr",
    "leptos-mf2/ssr",
    "hello-i18n/ssr",
    "dep:axum",
    "dep:leptos_axum",
    "dep:mf2-axum",
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
# `cargo leptos watch` watches the crate's own sources only. Without this,
# a translation edit is not seen until something else changes.
watch-additional-files = ["i18n/locales"]

[profile.wasm-release]
inherits = "release"
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

The application forwards its own `ssr` or `hydrate` to the i18n crate and
to `leptos-mf2`, just as it does to `leptos`. The i18n crate's features are
the same for both builds, so the server and the browser format alike. Here
they give numbers in each language's own symbols and dates through ICU4X,
whose data travels in each language's catalog. A feature that is on but
that no message uses adds nothing to the wasm.

**Leptos 0.9 or 0.8.** `leptos-mf2` and `mf2-axum` are built for Leptos
0.9 by default. A requirement of `"0.9.0-beta"` takes every later
`0.9.0-*` pre-release and the 0.9 releases with an ordinary `cargo update`.
An application that stays on Leptos 0.8 names the 0.8 crates and turns
the default line off in both of ours; the rest of the manifest, and every
source file, is unchanged:

```toml file=hello-0-8/Cargo.toml merge
[dependencies]
leptos = { version = "0.8", default-features = false }
leptos-mf2 = { version = "1", default-features = false, features = ["leptos-0-8"] }
leptos_meta = "0.8"
leptos_router = "0.8"
leptos_axum = { version = "0.8", optional = true }
mf2-axum = { version = "1", default-features = false, features = ["leptos-0-8"], optional = true }
```

Asking for both lines at once — `leptos-0-8` with the default features
still on — is a compile error that says what to write.

## The page

`src/lib.rs` holds the document shell, the page, and the browser's entry
point. The shell does three things for translation:

```rust file=hello/src/lib.rs
use hello_i18n::tr;
use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_mf2::{CatalogLinks, CatalogPreload, LocaleOption, LocaleSwitcher, html_lang};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

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
                // This page's catalog, downloading in parallel with the wasm.
                <CatalogPreload />
                // The other languages' catalog URLs, so a switch needs no
                // extra round trip.
                <CatalogLinks />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}
```

* `html_lang()` gives `<html lang dir>` for the language the server
  negotiated. This is how a screen reader picks its voice (WCAG 3.1.1), and
  how a right-to-left language lays out right-to-left.
* `<CatalogPreload/>` writes a `<link rel="preload">` for this page's
  catalog. This link **is** the client's boot data: the client reads the
  catalog URL from it and the language from `<html lang>`. There is no
  inline script and no JSON, and the catalog downloads while the wasm does.
* `<CatalogLinks/>` lists the other languages' catalog URLs, so switching
  language fetches the catalog directly. Leave it out to keep pages a few
  bytes smaller. A switch then asks the server for the URL first, which
  costs one round trip.

The page itself uses `tr!` wherever it needs text:

```rust file=hello/src/lib.rs
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text=tr!("app-title") />
        <Router>
            <header>
                <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply")>
                    <LocaleOption tag="en">{tr!("language.en")}</LocaleOption>
                    <LocaleOption tag="fr">{tr!("language.fr")}</LocaleOption>
                </LocaleSwitcher>
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
        // A signal as an argument: the text follows the count and the
        // language, with no closure at the call site.
        <p role="status">{tr!("visits", count = count)}</p>
        <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
    }
}
```

`tr!("id", name = value)` is checked **at compile time** against the
messages. A misspelt id gets a suggestion, and a missing, unknown or
duplicated argument is an error. It returns a small description of the
message (its number and its arguments), not text. The text is made where
the description is rendered, in whichever language is current, which is
why one macro works in a text node, an attribute, a component prop and a
`String`. [Call sites](call-sites.md) covers every position.

The switcher is a form: a labelled `<select>` and a button. Choosing a
language does nothing until the button is pressed. Once the page has
hydrated, pressing it switches the page live. Before that, or if the wasm
never loads, the form submits `?lang=fr` and the server renders the page
in French. [Switching language](switching.md) explains why, and how to
build your own switcher.

The browser's entry point installs the i18n crate and hydrates:

```rust file=hello/src/lib.rs
/// The browser's entry point: install what the i18n crate generated, then
/// hydrate once this page's catalog has arrived.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos_mf2::install(hello_i18n::setup());
    leptos_mf2::hydrate_body(App);
}
```

`hydrate_body` fetches the catalog named by the preload link (reusing that
download), checks that it came from the same build as the wasm, installs
it, and only then hydrates. If the catalog cannot be loaded, the page stays
as the server rendered it: readable, not interactive, with one `mf2:` line
in the console. A catalog from a different deploy makes the page reload,
rather than be read wrongly.

## The server

`src/main.rs` is an ordinary `leptos_axum` server with three additions:

```rust file=hello/src/main.rs
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;

    use axum::Router;
    use hello::{App, shell};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler_with_context, generate_route_list};
    use mf2_axum::{AcceptLanguage, CookieLocale, Negotiator, QueryParam};

    // 1. Install the i18n crate and its catalogs. Every catalog is checked
    //    against the build here, so a deploy that mixes builds fails at
    //    start-up rather than in a request.
    mf2_axum::install(hello_i18n::setup(), hello_i18n::CATALOGS)?;

    // 2. How a request's language is chosen: the first source, in order,
    //    that names a language this build has. `?lang=` first, so a link
    //    (and the switcher's form) can choose; then the cookie a choice
    //    leaves; then the browser's `Accept-Language`.
    let negotiator = Arc::new(
        Negotiator::empty()
            .source(QueryParam::default())
            .source(CookieLocale::default())
            .source(AcceptLanguage)
            .sink(CookieLocale {
                // `Secure` except in a debug build served over plain HTTP.
                secure: !cfg!(debug_assertions),
                ..CookieLocale::default()
            }),
    );
    let context = {
        let negotiator = Arc::clone(&negotiator);
        move || {
            mf2_axum::provide_locale(&negotiator);
        }
    };

    let conf = get_configuration(None)?;
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let app = Router::new()
        // 3. `/i18n/*`: the catalogs, served from the binary, each compressed
        //    once and kept, cached for a year (each file's name carries its
        //    content hash).
        .merge(mf2_axum::catalog_routes())
        .leptos_routes_with_context(&leptos_options, routes, context.clone(), {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(file_and_error_handler_with_context(context, shell))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
```

**Pass the context to every `_with_context` entry point**: the routes and
the fallback handler. If one is missed, the pages it serves render in the
default language, and nothing else shows that anything is wrong.

`provide_locale` negotiates the language for each request. It installs
that language's catalog for everything the request renders, and adds
`Content-Language`, `Vary` and the cookie to the response. The language is
chosen **once, on the server**: the client reads it from the page and never
negotiates again, so hydration cannot disagree with the server.

## Run it

```sh
cargo leptos watch
```

Open <http://127.0.0.1:3000>. The page is in English or French, depending
on your browser's languages. Choose the other language and press the
button: the page switches without reloading, `<html lang>` changes with it,
and the count keeps its value. Reload, and the page is still in the
language you chose, because the cookie remembers it. Edit a message in
`i18n/locales/fr/main.mf2`: the watcher rebuilds, and the new text appears.
The wasm stays byte-for-byte the same.

For a release build, run `cargo leptos build --release`.

## Next

* [Call sites](call-sites.md): `tr!` in every position, arguments, markup
  as elements, and when a `String` needs to be plain.
* [Delivery modes](delivery-modes.md): lazy routes, islands (the smallest
  download), and client-only applications.
* [Switching language](switching.md): the switcher, negotiation, and your
  own controls.
* [Accessibility](accessibility.md): what the library does for WCAG 2.2 AA,
  and what the application still has to do.
