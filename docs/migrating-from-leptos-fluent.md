# Migrating from `leptos-fluent`

`mf2 convert --from leptos-fluent` moves a `leptos-fluent` application to
this library in one command. It converts the `.ftl` files to MF2 messages,
and it rewrites every `tr!` and `move_tr!` in the Rust sources to this
library's `tr!`, where the rewrite is mechanical. Everything it cannot
rewrite it reports, with the file, the line and the column. The rest of this
page is that report: what each item means and how to finish it by hand.

The example is the application of [Getting started](getting-started.md), as
it would have been written with `leptos-fluent` 0.3. That release is built
on Leptos 0.8, so an application moving from it either moves to Leptos 0.9
at the same time or keeps 0.8 through this library's opt-in
([Getting started](getting-started.md), "Leptos 0.9 or 0.8"). The *before* samples on
this page are `leptos-fluent` code. They are not compiled, since this
repository does not depend on `leptos-fluent`, but `cargo xtask docs` runs
the commands below on them, and checks that the rewritten file is the one
this page shows and that the finished application compiles.

## Before

The messages, in `leptos-fluent`'s layout, one directory per language:

```ftl file=migrate/locales/en/main.ftl before
app-title = Hello, Fluent
greeting = Hello, { $name }!
visits =
    { $count ->
        [one] You have been here once.
       *[other] You have been here { $count } times.
    }
visit-again = Visit again
not-found = There is nothing here.
search = Search
    .placeholder = Search the site
language-label = Language
language-apply = Apply
language-en = English
language-fr = Français
```

```ftl file=migrate/locales/fr/main.ftl before
app-title = Bonjour, Fluent
greeting = Bonjour, { $name } !
visits =
    { $count ->
        [one] Vous êtes venu une fois.
       *[other] Vous êtes venu { $count } fois.
    }
visit-again = Revenir
not-found = Il n’y a rien ici.
search = Rechercher
    .placeholder = Rechercher sur le site
language-label = Langue
language-apply = Appliquer
language-en = English
language-fr = Français
```

The page's components, with `tr!` where a `String` is wanted and `move_tr!`
where text must follow the language:

```rust file=migrate/src/components.rs before
use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};

#[component]
pub fn Home() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <h1>{move_tr!("greeting", { "name" => "Ada" })}</h1>
        <p role="status">{move_tr!("visits", { "count" => count.get() })}</p>
        <button on:click=move |_| *count.write() += 1>{move || tr!("visit-again")}</button>
        <input type="search" placeholder=move_tr!("search.placeholder") aria-label=move_tr!("search") />
    }
}

/// The text of a notice, for code that needs a `String`.
pub fn visits_notice(count: i32) -> String {
    tr!("visits", { "count" => count })
}
```

And the application: the `leptos_fluent!` initializer, the shell, and a
language selector reading the `I18n` context:

```rust file=migrate/src/lib.rs before
pub mod components;

use components::Home;
use leptos::prelude::*;
use leptos_fluent::{I18n, leptos_fluent, move_tr};
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

#[component]
fn I18nProvider(children: Children) -> impl IntoView {
    leptos_fluent! {
        children: children(),
        locales: "./locales",
        default_language: "en",
        sync_html_tag_lang: true,
        cookie_name: "lang",
        initial_language_from_cookie: true,
        set_language_to_cookie: true,
        initial_language_from_accept_language_header: true,
    }
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
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
        <I18nProvider>
            <Title text=move_tr!("app-title") />
            <Router>
                <header>
                    <LanguageSelector />
                </header>
                <main>
                    <Routes fallback=|| view! { <p>{move_tr!("not-found")}</p> }>
                        <Route path=path!("/") view=Home />
                    </Routes>
                </main>
            </Router>
        </I18nProvider>
    }
}

#[component]
fn LanguageSelector() -> impl IntoView {
    let i18n = expect_context::<I18n>();
    view! {
        <fieldset>
            <legend>{move_tr!("language-label")}</legend>
            {move || {
                i18n.languages
                    .iter()
                    .map(|lang| {
                        view! {
                            <label>
                                <input
                                    type="radio"
                                    name="language"
                                    value=lang
                                    checked=&i18n.language.get() == lang
                                    on:click=move |_| i18n.language.set(lang)
                                />
                                {lang.name}
                            </label>
                        }
                    })
                    .collect::<Vec<_>>()
            }}
        </fieldset>
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
```

## The conversion

First the crate gains what [Getting started](getting-started.md)'s has
for its messages, and nothing else: its manifest names `mf2` (with the
`leptos` feature) and `mf2-build`, and forwards `ssr` and `hydrate` to
`mf2`, and it has the three-line build script. Here that is Getting
started's `Cargo.toml` and `build.rs`, unchanged. (`mf2 init --ssr
--no-messages`, in the crate, adds the same.) Then the conversion. Without `--write` the command changes nothing: it
prints a diff of every Rust file it would rewrite, the `.mf2` files it would
write, and its report. Read the diff, then run it again with `--write`:

```sh run=migrate status=1
mf2 convert --from leptos-fluent .
```

```sh run=migrate status=1 output=convert-report.txt
mf2 convert --from leptos-fluent . --write
```

`.` is the application's crate. The `.ftl` directory is the initializer's
`locales:` (or `--locales DIR`, else `locales/`), and each `.ftl` file
becomes a `.mf2` file beside it. The messages are in the application's own
crate, so the rewritten `use` lines import `crate::tr`. (Messages kept in a
crate of their own are written there with `-C DIR`, and the `use` lines
name that crate, read from its `Cargo.toml`, or `--i18n-crate`.)

Both runs exit with status 1, because the report is not empty: **a
migration is finished when the report is empty.** Everything else was
written.

## What the command rewrote

`src/components.rs`, rewritten:

```rust file=migrate/src/components.rs generated
use leptos::prelude::*;
use crate::tr;

#[component]
pub fn Home() -> impl IntoView {
    let count = RwSignal::new(1);
    view! {
        <h1>{tr!("greeting", name = "Ada")}</h1>
        <p role="status">{Signal::derive(move || tr!("visits", count = count.get()).to_string())}</p>
        <button on:click=move |_| *count.write() += 1>{tr!("visit-again")}</button>
        <input type="search" placeholder=tr!("search.placeholder") aria-label=tr!("search") />
    }
}

/// The text of a notice, for code that needs a `String`.
pub fn visits_notice(count: i32) -> String {
    tr!("visits", count = count).to_string()
}
```

Each call became `tr!`, with its arguments written `name = value`. What it
became depends on where it stands:

| Where the call stands | `leptos-fluent` | After |
|---|---|---|
| where Leptos renders it: a child, an attribute or prop value, with arguments that cannot change (literals, or none) | `tr!(…)`, `move_tr!(…)`, `move \|\| tr!(…)` | `tr!(…)`: the description renders itself and follows a language switch |
| the same, with an argument that may change, in a closure | `move \|\| tr!(…)` | the closure kept, returning `tr!(…).to_string()` |
| anywhere else: a function, a closure, a `match` arm, an `if` branch | `tr!(…)` | `tr!(…).to_string()`, the same `String` as before |
| `move_tr!` outside a view, or with an argument that may change | `move_tr!(…)` | `Signal::derive(move \|\| tr!(…).to_string())`, `move_tr!`'s own expansion: the same `Signal<String>` |

The command keeps every type and every evaluation of an argument as it was.
It cannot know that `count.get()` reads a signal, so the visit counter kept
a closure. The idiomatic form passes the signal itself, and the text follows
it without a closure at the call site ([Call sites](call-sites.md),
"Arguments that change"). Change it by hand where you like:

```text
// before
<p role="status">{Signal::derive(move || tr!("visits", count = count.get()).to_string())}</p>
// after
<p role="status">{tr!("visits", count = count)}</p>
```

The other rewrites:

* `tr!(i18n, "id", …)`: the context is dropped.
* `{ "name" => value }` becomes `name = value`. A name that is not a Rust
  identifier, such as `$user-name`, is written `"user-name" = value`.
* A Fluent attribute keeps its id: `search.placeholder` is the message the
  conversion made from `search`'s `.placeholder`.
* `use leptos_fluent::{move_tr, tr};` becomes `use crate::tr;`.

Every call is also checked against the converted messages. An id that is
not a message, or arguments that are not exactly the message's variables,
are reported, because `tr!` checks both when it compiles. `leptos-fluent`
ignored an extra argument and printed `{$name}` for a missing one.

A file that does not name `leptos_fluent` is not touched. A second run
therefore changes nothing: it exits as the first did, and reports the
`.mf2` files it would write as unchanged.

## What is left to do by hand

For the application above, the report says:

```text file=migrate/convert-report.txt generated
./src/lib.rs:5:1: error: this `use leptos_fluent` is not rewritten: it names I18n, leptos_fluent, move_tr [leptos-fluent-import]
./src/lib.rs:12:5: error: `leptos_fluent!` initializes leptos-fluent: replace it with the generated `install()`, on the server and in the browser; its `lang` cookie is not read: the client always writes `mf2_locale`, so to keep the language readers chose before, add `CookieLocale { name: "lang", ..Default::default() }` to the server's `Negotiator` as an extra source, after `CookieLocale::default()` and before `AcceptLanguage` [leptos-fluent-initializer]
./src/lib.rs:64:33: error: the `leptos-fluent` context: its language and languages become `<LocaleSwitcher>` or the locale API, its `tr` / `tr_with_args` `msg_id!` and `TrDyn` [leptos-fluent-context]
mf2 convert: 22 entries in 2 locale(s), 2 .mf2 file(s) written; 2 Rust file(s) rewritten; 3 error(s), 0 warning(s)
```

The sample's manifest does not name `leptos-fluent`; a real application's
does, and its report also has a `leptos-fluent-dependency` line for it.
Each code, and how to finish it:

| Code | What | Instead |
|---|---|---|
| `leptos-fluent-initializer` | `leptos_fluent! { … }` or `static_loader! { … }` | the generated `install()`, called in the browser's entry point and at the server's start. The language negotiation options become `mf2::axum`'s `Negotiator` ([Switching language](switching.md)) |
| `leptos-fluent-context` | the `I18n` context: `i18n.language`, `i18n.languages`, `i18n.tr(…)` | a language selector becomes `<LocaleSwitcher>`; a lookup by an id known only at run time becomes `msg_id!` and `TrDyn` ([Call sites](call-sites.md)) |
| `leptos-fluent-import` | a `use leptos_fluent::…` naming more than `tr` and `move_tr` | the generated `tr`, at the crate's root |
| `leptos-fluent-dependency` | `leptos-fluent` or `fluent-templates` in a `Cargo.toml` | `mf2` (with `leptos`) and `mf2-build`, with `ssr` and `hydrate` forwarded to `mf2` ([Getting started](getting-started.md)) |
| `leptos-fluent-dynamic-id` | an id that is not a string literal | a literal id, or `msg_id!` and `TrDyn` |
| `leptos-fluent-if-form` | `tr!(if c { "a" } else { "b" })` | `if c { tr!("a") } else { tr!("b") }`, each branch the same type |
| `leptos-fluent-cfg` | `#[cfg]` inside a call | a `#[cfg]` on a statement around it |
| `leptos-fluent-argument-name`, `leptos-fluent-call` | a call in none of the documented forms | by hand |
| `leptos-fluent-unknown-id`, `leptos-fluent-arguments` | the call does not match the converted message | the message or the call (rewritten already) |
| `leptos-fluent-parse` | a file that does not tokenize | fix it and run again |

The finished `src/lib.rs`: the initializer and the selector are replaced,
and the shell gains what [Getting started](getting-started.md) explains
(`<html lang dir>`, the catalog preload, the switcher):

```rust file=migrate/src/lib.rs
pub mod components;

use components::Home;
use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use mf2::leptos::{CatalogLinks, CatalogPreload, LocaleOption, LocaleSwitcher, html_lang};

mf2::include_generated!();

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
                <LocaleSwitcher label=tr!("language-label") button=tr!("language-apply")>
                    <LocaleOption tag="en">{tr!("language-en")}</LocaleOption>
                    <LocaleOption tag="fr">{tr!("language-fr")}</LocaleOption>
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

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    install();
    mf2::leptos::hydrate_body(App);
}
```

The server is [Getting started](getting-started.md)'s `src/main.rs`: it
calls the generated `install()` and negotiates each request's language. It reads this library's cookie, `mf2_locale`, which the
client writes on every switch — not the initializer's `lang`. So that a
reader's earlier choice is kept after the migration, add a source for the
old cookie after the default one and before `AcceptLanguage`, as the
report says: `.source(CookieLocale { name: "lang",
..CookieLocale::default() })`. The manifest is Getting started's too:
`leptos-fluent` gives way to `mf2` and `mf2-build`, and `ssr` and
`hydrate` forward to `mf2`.

## What changes for the reader

* **Text follows a language switch everywhere.** A `tr!` in a view used to
  be a `String` fixed when the view was built. It is now a description
  that the library re-renders on a switch, the fix `leptos-fluent` asked
  for with `move_tr!` or a closure.
* **The browser downloads one language.** `leptos-fluent` compiled every
  language's `.ftl` into the wasm. Now each language is a catalog fetched
  when it is needed, and the wasm holds no message text.
* **Numbers are localized.** `fluent-bundle` printed a number as Rust's
  `f64` does. `:number` groups digits and uses each language's symbols
  (with the `fn-number` feature).
* **Bidi isolation follows the MF2 specification.** A placeholder whose
  direction could differ from the message's (a string) is isolated; a
  formatted number, in the locale's own direction, is not. The characters are
  invisible, and attributes a program reads get none
  ([Call sites](call-sites.md)).
* **Terms are copied.** MF2 has no Fluent terms (`-brand`). The conversion
  copies a term into every message that uses it, and its report counts
  where. A later change to the brand name is a change to each message.

A decimal with more than three places now chooses its plural variant by
the rounded number it shows (1.0004 is "one"), and plural rules are
current CLDR's. The conversion keeps everything else: for the same
arguments a converted message chooses the variant Fluent chose.
