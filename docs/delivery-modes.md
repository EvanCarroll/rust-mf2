# Delivery modes

Leptos can deliver an application in four ways, and this library supports
all four. They differ in what runs in the browser and in how a language
switch happens:

| Mode | The browser gets | A language switch | Start from |
|---|---|---|---|
| **SSR + hydrate** (the default) | the whole application as wasm | live, no reload | [Getting started](getting-started.md) |
| …with **lazy routes** | the same, split into chunks fetched per route | live, no reload | [below](#lazy-routes) |
| **Islands** | only the interactive parts | the form's `?lang=` and a new page, remembered in a cookie | [below](#islands) |
| **Client-only** | the whole application, no server | live, remembered in the browser | [below](#client-only) |

In every mode, the wasm contains no message text, ids, argument names or
plural rules. A translation edit leaves it byte-for-byte the same, so
every reader's cached copy stays valid.

## SSR + hydrate

This is [Getting started](getting-started.md): the server negotiates the
language, renders the page in it, and embeds the catalogs. The browser
loads the page's catalog (preloaded by the page, in parallel with the
wasm), hydrates, and switches language live. Use it unless you have a
reason to use one of the others.

**One translation crate** is enough. The catalogs are embedded in the
server binary only: the generated module's list of catalog files is
compiled only with `ssr`. So a translation edit changes the server and
leaves the wasm alone, with no extra build step.

## Lazy routes

With `cargo leptos --split`, a `#[lazy_route]` becomes a wasm chunk of its
own, fetched the first time the route is matched. Descriptions inside the
chunk read the catalog the main module installed, join the same node
registry, and follow a switch like everything else. When the route
unmounts, they free their registry slots.

Two things change from Getting started: the route, and the entry point,
`hydrate_lazy`. The shell is unchanged:

```rust file=lazy/src/lib.rs
use hello_i18n::tr;
use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_mf2::{CatalogLinks, CatalogPreload, LocaleOption, LocaleSwitcher, html_lang};
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::{Lazy, LazyRoute, lazy_route, path};

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
```

The application has a second route, whose view is a `LazyRoute`:

```rust file=lazy/src/lib.rs
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
            <nav>
                <A href="/">{tr!("app-title")}</A>
                " "
                <A href="/visits">{tr!("visit-again")}</A>
            </nav>
            <main>
                <Routes fallback=|| view! { <p>{tr!("not-found")}</p> }>
                    <Route path=path!("/") view=|| view! { <h1>{tr!("greeting", name = "Ada")}</h1> } />
                    // Under `--split`, this view is a wasm chunk of its own.
                    <Route path=path!("/visits") view={Lazy::<Visits>::new()} />
                </Routes>
            </main>
        </Router>
    }
}

pub struct Visits;

#[lazy_route]
impl LazyRoute for Visits {
    fn data() -> Self {
        Visits
    }

    fn view(this: Self) -> AnyView {
        let Visits = this;
        let count = RwSignal::new(1);
        view! {
            <p role="status">{tr!("visits", count = count)}</p>
            <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
        }
        .into_any()
    }
}
```

`hydrate_lazy` replaces `hydrate_body`. It does the same thing, and in
addition, when the page being loaded **is** a lazy route, it loads that
route's chunk before hydration reaches it:

```rust file=lazy/src/lib.rs
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos_mf2::install(hello_i18n::setup());
    leptos_mf2::hydrate_lazy(App);
}
```

Leptos hydrates a lazy route only with its `lazy` feature, so the
client's feature list gains `leptos/lazy`. Without it, the page panics
when hydration reaches the route:

```toml file=lazy/Cargo.toml merge
[features]
hydrate = [
    "leptos/hydrate",
    "leptos/lazy",
    "leptos-mf2/hydrate",
    "hello-i18n/hydrate",
    "dep:console_error_panic_hook",
    "dep:wasm-bindgen",
]
```

Run it with `cargo leptos watch --split`, and build it with `cargo leptos
build --split`. The flag is not optional here: with `#[lazy_route]` and
`leptos/lazy`, a build without it leaves the JavaScript importing a
placeholder (`__wasm_split_placeholder__`) that the browser cannot
resolve, and nothing hydrates on any page. This is Leptos's behaviour, not
this library's.

## Islands

In an islands application, only the components marked `#[island]` are
compiled to wasm. Everything else renders on the server and ships no
code at all. **A server-only component costs the wasm nothing**, however
many messages it uses. `cargo xtask islands-zero` measured this on the
islands example (2026-09-28). It adds a server-only component with a call
site in every position (text, attribute, argument, markup) and compares the
client with and without it. The code section was 165,705 bytes with 864
functions both times, and the data section 23,446 bytes both times, so the
wasm shipped 85,726 bytes gzipped both times. The two files are not
identical byte for byte, but no section grew. (An earlier run, 2026-09-27,
saw the data section grow by 8 bytes: a few bytes of data at most, and no
code.)

Islands change the trade-off for switching. Most of the page has no client
code, so it cannot follow a live switch. The documented default is
therefore the `static-locale` feature: the switcher's form submits
`?lang=`, and the server renders the whole page in the new language,
server-only parts included, and remembers the choice in the cookie. Turn on `islands` in Leptos and `static-locale`
in `leptos-mf2`:

```toml file=islands/Cargo.toml merge
[dependencies]
leptos = { version = "0.9.0-beta", default-features = false, features = ["islands"] }
leptos-mf2 = { version = "1", features = ["static-locale"] }
```

`static-locale` applies to the server and the client alike. Nothing on
the client registers to follow the language, except a node with a
signal-valued argument (it still has to re-format when the signal changes).

**The islands gate.** A message with markup takes its element structure
from the catalog, so an island that contains one must not hydrate before
the catalog has arrived. Leptos hydrates islands one at a time, in page
order, and waits for any island that returns a promise. `<IslandsGate/>` is
an empty island, placed first in `<body>`, that waits for the catalog.
Every island after it then hydrates against the page's catalog. It costs
no extra request, and a few bytes of page:

```rust file=islands/src/lib.rs
use hello_i18n::tr;
use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_mf2::{CatalogPreload, IslandsGate, LocaleOption, LocaleSwitcher, html_lang};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    let (lang, dir) = html_lang();
    view! {
        <!DOCTYPE html>
        <html lang=lang dir=dir>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options islands=true />
                <MetaTags />
                // No `<CatalogLinks/>`: a switch reloads, and the server
                // writes the new page's preload.
                <CatalogPreload />
            </head>
            <body>
                // First, and outside every island.
                <IslandsGate />
                <App />
            </body>
        </html>
    }
}
```

The page is an ordinary component, so it renders on the server only. The
switcher is not an island either: under `static-locale` its form submits
`?lang=…`, and the server does the rest:

```rust file=islands/src/lib.rs
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text=tr!("app-title") />
        <header>
            <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply")>
                <LocaleOption tag="en">{tr!("language.en")}</LocaleOption>
                <LocaleOption tag="fr">{tr!("language.fr")}</LocaleOption>
            </LocaleSwitcher>
        </header>
        <main>
            <h1>{tr!("greeting", name = "Ada")}</h1>
            <Visits />
        </main>
    }
}

/// The one part that runs in the browser.
#[island]
fn Visits() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <p role="status">{tr!("visits", count = count)}</p>
        <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
    }
}
```

The client's entry point installs the crate and starts loading the catalog
(`hydrate_islands`). The application also has to export the gate's island
function itself, because `leptos-mf2` forbids the `unsafe` code that a
`#[wasm_bindgen]` export expands to. `islands_gate!()` writes it:

```rust file=islands/src/lib.rs
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos_mf2::install(hello_i18n::setup());
    leptos_mf2::hydrate_islands();
}

// The island `<IslandsGate/>` renders.
leptos_mf2::islands_gate!();
```

The server is the same as in Getting started. Islands change the client,
not the server.

**Without `static-locale`.** The islands then register their nodes to
follow a live switch, but the switch does not become live: the switcher
is not an island, so pressing it still submits `?lang=` and loads a new
page, and an island's state (a counter, say) starts again. The
registration costs wasm and memory for nothing, so keep `static-locale`
on an islands application.

## Client-only

A client-only application has no server. The browser does everything,
and any static file host can serve the site. Three things are different:

* **The language comes from the browser**: the one the reader chose last
  time (kept in `localStorage`), else the one that best serves
  `navigator.languages` (`fr-CA` finds `fr`), chosen as a server chooses
  from `Accept-Language`, else the source language. A switch is live, and
  is remembered. To choose as a server does, by CLDR's language-matching
  data, the page needs the part of that data its languages need, which the
  build generates, and a client-only build's generated `setup()` carries.
  A `Setup` built by hand without it
  (`.with_language_matching(&LANGUAGE_MATCHING)`) finds only a locale of
  the reader's own language (`fr-CA` still finds `fr`).
* **The catalogs are published beside the wasm**, by `mf2 compile --site`.
  There is no server to embed them in. The translation crate generates only
  the module (`Emit::Module`), so the wasm names no catalog file.
* **The page finds them through a small index**, `i18n/index.json`. The
  page preloads it, so it downloads in parallel with the wasm.

The application's manifest turns on `csr` everywhere:

```toml file=csr/Cargo.toml
[workspace]
members = [".", "i18n"]
resolver = "3"

[package]
name = "hello-csr"
version = "0.1.0"
edition = "2024"

[dependencies]
console_error_panic_hook = "0.1"
hello-i18n = { path = "i18n", features = ["csr", "fn-number"] }
leptos = { version = "0.9.0-beta", features = ["csr"] }
leptos-mf2 = { version = "1", features = ["csr"] }
leptos_meta = "0.9.0-beta"

[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

The translation crate's build script emits the module only:

```rust file=csr/i18n/build.rs
//! Parses locales/, writes the manifest, and generates the module
//! src/lib.rs includes. The catalogs are published by
//! `mf2 compile --site` (Trunk.toml), so this crate names none of them.

fn main() {
    let outcome = match mf2_build::Build::new()
        .and_then(|build| build.emit(mf2_build::Emit::Module).emit_cargo(true).run())
    {
        Ok(outcome) => outcome,
        Err(e) => {
            println!("cargo::error={e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = outcome.into_result() {
        println!("cargo::error={e}");
        std::process::exit(1);
    }
}
```

The application mounts through the same gate as hydration.
`mount_to_body` loads the index, chooses the language from those it
lists, loads that language's catalog, sets `<html lang dir>`, and only
then mounts. If the boot fails,
it logs one `mf2:` line and mounts nothing:

```rust file=csr/src/main.rs
use hello_i18n::tr;
use leptos::prelude::*;
use leptos_meta::{Title, provide_meta_context};
use leptos_mf2::{LocaleOption, LocaleSwitcher};

#[component]
fn App() -> impl IntoView {
    provide_meta_context();
    let count = RwSignal::new(1);
    view! {
        <Title text=tr!("app-title") />
        <header>
            <LocaleSwitcher label=tr!("language.label") button=tr!("language.apply")>
                <LocaleOption tag="en">{tr!("language.en")}</LocaleOption>
                <LocaleOption tag="fr">{tr!("language.fr")}</LocaleOption>
            </LocaleSwitcher>
        </header>
        <main>
            <h1>{tr!("greeting", name = "Ada")}</h1>
            <p role="status">{tr!("visits", count = count)}</p>
            <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
        </main>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos_mf2::install(hello_i18n::setup());
    leptos_mf2::mount_to_body(App);
}
```

With [Trunk](https://trunkrs.dev), `index.html` preloads the index:

```html file=csr/index.html
<!DOCTYPE html>
<!-- The boot sets `lang` and `dir` to the language it chooses. -->
<html lang="en" dir="ltr">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Hello, MessageFormat 2</title>
    <link rel="preload" as="fetch" crossorigin="anonymous" href="i18n/index.json" data-mf2-index />
    <link data-trunk rel="rust" data-bin="hello-csr" />
  </head>
  <body></body>
</html>
```

and a hook publishes the catalogs into the site after each build:

```toml file=csr/Trunk.toml
[build]
target = "index.html"
dist = "dist"

[[hooks]]
stage = "post_build"
command = "sh"
command_arguments = ["-c", "mf2 -C i18n compile --site \"$TRUNK_STAGING_DIR/i18n\""]
```

`mf2 compile --site` writes each catalog (with `.br` and `.gz` versions),
named by its content hash, and `index.json`. It builds the catalogs for the
translation crate's features **as cargo resolves them**, so the catalogs
and the wasm always agree on which functions exist. If you pass a
`--features` list that disagrees with cargo's, it fails and prints both
lists. Serve the catalogs with a long cache lifetime, since their names
change when their content does, and serve `index.json` with `no-cache`.

Booting costs two requests one after the other: the index, then one
catalog. SSR costs one. The index starts downloading with the wasm, not
after it.

`trunk serve` runs it locally; `trunk build --release` writes the site to
`dist/`.
