# Getting started

This page builds a small server-rendered Leptos application, `hello`,
translated into English and French. The server renders each page in the
reader's language. The browser then hydrates it and switches language
**live, without a reload**. That is the default way to use this library,
and most Leptos applications are built this way. The other delivery modes
(islands, client-only, lazy routes) are on [their own page](delivery-modes.md).
Each of them starts from what you build here.

Every file below is complete.

> **Checked by CI.** `cargo xtask docs` puts these same blocks together
> into this application and compiles it for the server and for the
> browser, so what you copy here is what CI builds.

## What you need

* Rust 1.88 or later (the 2024 edition), and the browser target:

  ```sh
  rustup target add wasm32-unknown-unknown
  ```

* [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos), which builds
  the server and the client together and serves them:

  ```sh
  cargo install cargo-leptos
  ```

* Optionally, the `mf2` command, which checks translations and makes
  starters ([The command line](command-line.md) has all of it). Nothing on
  this page needs it:

  ```sh
  cargo install mf2-cli
  ```

  `mf2 init --ssr` writes this whole application in one step
  ([Command line](command-line.md#init-a-starter)); this page writes it by
  hand, a file at a time, so that each line is explained.

* For a client-only application only, [Trunk](https://trunkrs.dev)
  (`cargo install trunk`), which builds it
  ([Delivery modes](delivery-modes.md#client-only)).

> **Which version these pages show.** The manifests on these pages name
> `mf2 = "2"` and `mf2-build = "2"`: 2.0.0, the next release, which these
> pages follow (1.0.0 is on crates.io; 1.1.0 was not published and will not
> be). Until 2.0.0 is published, build from a checkout of this repository:
> `cargo xtask docs` compiles these pages by replacing each `"2"` with a
> path into it, for example `mf2 = { path = "../rust-mf2/crates/mf2" }`,
> and `cargo install --path crates/mf2-cli` installs its `mf2` command.

## The shape of an application

```text
hello/
├── Cargo.toml        the application, and the `mf2` it builds with
├── build.rs          reads locales/, writes the catalogs and the module
├── locales/
│   ├── en/main.mf2
│   └── fr/main.mf2
└── src/
    ├── lib.rs        the page, and the browser's entry point
    └── main.rs       the server
```

One crate holds the application and its messages.

> **How it works.** The build script reads `locales/`, checks every
> message, and writes three things: one binary catalog per language, a
> manifest describing all the messages, and a small Rust module, which the
> crate includes, holding the `tr!` macro and the `Locale` type. The
> browser downloads the catalog for one language when it needs it. **No
> message text, message id, argument name or plural rule is compiled into
> the wasm.** So changing a translation leaves the wasm byte-for-byte the
> same, and every reader's cached copy stays valid.

## The messages

Messages are Unicode [MessageFormat 2](https://www.unicode.org/reports/tr35/tr35-messageFormat.html)
(MF2), in files under `locales/<language>/`. Each file starts by naming its
language. Then comes one message per `id = pattern`, and a `[section]`
prefixes the ids that follow it (`language.label` below).
[MF2 for developers](mf2-for-developers.md) teaches the language itself:

```mf2 file=hello/locales/en/main.mf2
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

```mf2 file=hello/locales/fr/main.mf2
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

`mf2 check` runs every check that the build runs, with the features cargo
resolves for the build (or the ones `--features` names). It also works
without cargo, so you can use it in a translator's editor: it then checks
as if every function were on, and says so in one line.

## The manifest

```toml file=hello/Cargo.toml
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

The application forwards its own `ssr` or `hydrate` to `mf2`, just as it
does to `leptos`, and the server's build turns on `mf2/axum` beside it.
`watch-additional-files` makes `cargo leptos watch` see an edit under
`locales/`: without it, a translation edit is not seen until something
else changes.

`mf2`'s other features decide **which formatting functions exist**, and
they are the same for both builds, so the server and the browser format
alike. A message can never add code to the wasm by itself: if a French
translation uses `:datetime` and the crate was not built with
`fn-datetime`, the build fails and names the message. With no features, a
message can still use `:string`, `:number`, `:integer` and plural
selection. Numbers then use neutral symbols (`1234.5`). `fn-number` gives
each language's own symbols and grouping (`1 234,5` in French, grouped with
a narrow no-break space, U+202F); dates take `fn-datetime` and one backend:

| Feature of `mf2` | Adds |
|---|---|
| `fn-number` | numbers in each locale's own symbols, `:percent`, `:currency`, `:unit` |
| `datetime-icu` (with `features = ["icu-blob"]` on `mf2-build`) | `:datetime`, `:date`, `:time` through ICU4X, on the server and in the browser alike; it turns on `fn-datetime` |
| `datetime-intl` | the same, through the browser's own `Intl.DateTimeFormat`: a smaller wasm, and the browser's formatting |
| `intl` | numbers and plural rules through the browser's `Intl` too |

A feature that is on but that no message uses adds nothing to the wasm.

The build script is three lines. It reads `locales/`, checks every message,
and writes the catalogs, the manifest and the module to cargo's `OUT_DIR`;
a message with an error fails the build and is named:

```rust file=hello/build.rs
fn main() {
    mf2_build::run();
}
```

An optional `mf2.toml` beside `Cargo.toml` configures the build: the
source language (`en` unless it says otherwise), what the catalogs leave
out, what a missing translation shows, and the lints. The defaults suit
this page: a message not translated yet shows in the source language, and
the catalogs the browser downloads carry no message ids and no comments.
[`mf2.toml`](configuration.md) lists every key.

**Leptos 0.9 or 0.8.** `mf2`'s `leptos` feature is for Leptos 0.9. A
requirement of `"0.9.0-beta"` takes every later `0.9.0-*` pre-release and
the 0.9 releases with an ordinary `cargo update`. An application that stays
on Leptos 0.8 names the 0.8 crates and `mf2`'s `leptos-0-8` in place of
`leptos`; the rest of the manifest, and every source file, is unchanged:

```toml file=hello-0-8/Cargo.toml merge
[dependencies]
leptos = { version = "0.8", default-features = false }
leptos_meta = "0.8"
leptos_router = "0.8"
leptos_axum = { version = "0.8", optional = true }
mf2 = { version = "2", features = ["leptos-0-8", "fn-number"] }
```

Asking for both lines at once — `leptos` and `leptos-0-8` — is a compile
error, the only one, that says what to write.

## The page

`src/lib.rs` includes what the build generated, and holds the document
shell, the page, and the browser's entry point. The shell does three things
for translation:

```rust file=hello/src/lib.rs
use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use mf2::leptos::{CatalogLinks, CatalogPreload, LocaleSwitcher, html_lang};

// What the build script generated: `tr!`, `Locale`, `install()` and the
// rest, at this crate's root.
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
                // This page's catalog, downloading in parallel with the wasm.
                <CatalogPreload />
                // Every language's catalog URL, so a switch needs no extra
                // round trip.
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
  catalog, so it downloads while the wasm does.
* `<CatalogLinks/>` lists every language's catalog URL (this page's
  included), so switching language fetches the catalog directly. Leave it
  out to keep pages a few bytes smaller. A switch then asks the server for
  the URL first, which costs one round trip.

> **How the client boots.** The preload link **is** the client's boot
> data: the client reads the catalog URL from it and the language from
> `<html lang>`. There is no inline script and no JSON.

The page itself uses `tr!` wherever it needs text:

```rust file=hello/src/lib.rs
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
        // A signal as an argument: the text follows the count and the
        // language, with no closure at the call site.
        <p role="status">{tr!("visits", count = count)}</p>
        <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
    }
}
```

`tr!("id", name = value)` is checked **at compile time** against the
messages. A misspelt id gets a suggestion, and a missing, unknown or
duplicated argument is an error. One macro works in a text node, an
attribute, a component prop and a `String`; [Call sites](call-sites.md)
covers every position.

> **What `tr!` returns.** A small description of the message (its number
> and its arguments), not text. The text is made where the description is
> rendered, in whichever language is current, which is why one macro works
> in every position.

The switcher is a form: a labelled `<select>` and a button. Choosing a
language does nothing until the button is pressed. Once the page has
hydrated, pressing it switches the page live. Before that, or if the wasm
never loads, the form submits `?lang=fr` and the server renders the page
in French. [Switching language](switching.md) explains why, and how to
build your own switcher.

The switcher lists every language, in the order of `Locale::ALL`, each
named by its `language.<tag>` message in its own language: adding a
language to `locales/` adds it to the switcher, with no code.

The browser's entry point installs what the build generated and hydrates:

```rust file=hello/src/lib.rs
/// The browser's entry point: install what the build generated, then
/// hydrate once this page's catalog has arrived.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    install();
    mf2::leptos::hydrate_body(App);
}
```

> **What `hydrate_body` does.** It fetches the catalog named by the
> preload link (reusing that download), checks that it came from the same
> build as the wasm, installs it, and only then hydrates. If the catalog
> cannot be loaded, the page stays as the server rendered it: readable, not
> interactive, with one `mf2:` line in the console. A catalog from a
> different deploy makes the page reload, rather than be read wrongly.

## The server

`src/main.rs` is an ordinary `leptos_axum` server with three additions:

```rust file=hello/src/main.rs
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::Router;
    use hello::{App, shell};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler, generate_route_list};
    use mf2::axum::{Negotiator, catalog_routes};

    // 1. The catalogs this build embeds, each checked against the build
    //    once, here: a deploy that mixes builds fails at start-up rather
    //    than in a request.
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
        // 2. Each request's language: `?lang=` (a link, or the switcher's
        //    form), then the cookie a choice leaves, then the browser's
        //    `Accept-Language`. The response gets `Content-Language`, `Vary`
        //    and the cookie.
        .layer(Negotiator::default())
        // 3. `/i18n/*`: the catalogs, served from the binary, each compressed
        //    once and kept, cached for a year (each file's name carries its
        //    content hash).
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

The routes and the error handler are Leptos's plain forms, with nothing
to pass to each.

> **How the language reaches the page.** The negotiator is a tower layer:
> every page the routes and the fallback render finds the request's
> language through the request itself. The language is chosen **once, on
> the server**: the client reads it from the page and never negotiates
> again, so hydration cannot disagree with the server.

## Run it

```sh
cargo leptos watch
```

Open <http://127.0.0.1:3000>. The page is in English or French, depending
on your browser's languages. Choose the other language and press the
button: the page switches without reloading, `<html lang>` changes with it,
and the count keeps its value. Reload, and the page is still in the
language you chose, because the cookie remembers it. Edit a message in
`locales/fr/main.mf2`: the watcher rebuilds, and the new text appears.
The wasm stays byte-for-byte the same.

For a release build, run `cargo leptos build --release`.

## Next

* [MF2 for developers](mf2-for-developers.md): the message language, from
  placeholders to plurals, gender and markup.
* [Call sites](call-sites.md): `tr!` in every position, arguments, markup
  as elements, and when a `String` needs to be plain.
* [Delivery modes](delivery-modes.md): lazy routes, islands (the smallest
  download), and client-only applications.
* [Switching language](switching.md): the switcher, negotiation, and your
  own controls.
* [Translating](translating.md): translators, reviews, pseudo-locales and
  the checks for CI.
* [Accessibility](accessibility.md): what the library does for WCAG 2.2 AA,
  and what the application still has to do.
* [Testing](testing.md): the text in each language, pinned per test.
* [Troubleshooting](troubleshooting.md): when `tr!` is not found, the text
  is empty, or the page stays in one language.
