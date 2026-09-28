# Switching language

This page covers how a request's language is chosen, the switcher the
library provides, what a switch does, and how to build your own control.
Its samples join the [call sites](call-sites.md) library, with these
messages:

```mf2 file=calls/i18n/locales/en/main.mf2
[language]
label = Language
apply = Apply
en = English
fr = Français
```

## How the server chooses

On a server, `mf2-axum`'s `Negotiator` chooses each request's language. It
tries an ordered list of **sources**, and the first one that names a
language this build has wins (`fr-CA` finds `fr`). **Sinks** record the
result in the response:

| Source | Reads |
|---|---|
| `QueryParam` | `?lang=fr` (the name can be changed): how a link, a test, or the switcher's form chooses |
| `CookieLocale` | the `mf2_locale` cookie: a choice the reader made earlier |
| `AcceptLanguage` | the browser's languages, in quality order |
| `PathPrefix` | `/fr/…`, for a site whose languages have their own URLs |

If no source matches, the source language is used, unless
`.default_locale(…)` names another. The response carries
`Content-Language` and a `Vary` that names each source's header. The
server is the **only** place a language is negotiated. The client reads it
from the page (`<html lang>` and the preload link), so hydration cannot
choose differently.

Getting started lists the sources as query, cookie, then
`Accept-Language`. A site with a language in its URLs puts the path first:

```rust file=calls/src/lib.rs
/// A site whose pages live under `/en/…` and `/fr/…`.
#[cfg(feature = "ssr")]
pub fn path_negotiator() -> mf2_axum::Negotiator {
    use mf2_axum::{AcceptLanguage, CookieLocale, Negotiator, PathPrefix};
    Negotiator::empty()
        .source(PathPrefix)
        .source(CookieLocale::default())
        .source(AcceptLanguage)
        .sink(CookieLocale::default())
}
```

Such a site should also tell search engines about the other languages'
URLs. `<AlternateLinks/>` in the `<head>` writes one
`<link rel="alternate" hreflang>` per language, plus an `x-default`:

```rust file=calls/src/lib.rs
#[component]
pub fn Alternates() -> impl IntoView {
    view! {
        <leptos_mf2::AlternateLinks href_of=|tag| format!("https://example.com/{tag}/") />
    }
}
```

## The switcher

`<LocaleSwitcher>` is the switcher the library provides. It is a
`<form method="get">`. Inside it are a `<select name="lang">` in its own
`<label>`, and a submit button whose text you supply:

```rust file=calls/src/lib.rs
#[component]
pub fn Header() -> impl IntoView {
    view! {
        <header>
            <leptos_mf2::LocaleSwitcher label=tr!("language.label") button=tr!("language.apply")>
                <leptos_mf2::LocaleOption tag="en">{tr!("language.en")}</leptos_mf2::LocaleOption>
                <leptos_mf2::LocaleOption tag="fr">{tr!("language.fr")}</leptos_mf2::LocaleOption>
            </leptos_mf2::LocaleSwitcher>
        </header>
    }
}
```

**Nothing happens until the button is pressed.** A `<select>` fires
`change` on every arrow key, so a switcher that switched on `change` would
change the page's language at each keypress while a keyboard user moved
through the list (WCAG 3.2.2). What pressing the button does depends on
what the page runs:

| The page | Pressing the button |
|---|---|
| hydrated (`hydrate`) or client-only (`csr`) | switches live, with focus left on the button |
| `static-locale` (the islands default) | writes the cookie and reloads in the new language |
| not yet hydrated, hydration failed, or no wasm | submits `?lang=fr`, which the server negotiates |

The last row is why it is a form. The switcher works before the wasm
arrives and when it never does, and on an islands page it needs no island.

The option for the page's language is `selected` in the server's HTML.
Each option carries its own `lang`, so a screen reader reads "Français"
with a French voice. Each option's text is a message: `language.fr` is
`Français` in every catalog, so each language is named in its own
language, and none of the names is compiled into the wasm. The switcher
has no fixed `id`, so a page can have two (a header and a footer).

The form has the class `mf2-locale-switcher`. A flexbox layout that works
in both directions:

```css
.mf2-locale-switcher {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
}

.mf2-locale-switcher label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
```

## What a switch does

A live switch (`leptos_mf2::set_locale("fr")`) does this, in order:

1. it finds the French catalog's URL: from the page's `<CatalogLinks/>`
   if the shell renders them, or else by asking `GET /i18n/fr`, which
   redirects to it (one extra round trip, only at switch time);
2. it fetches the catalog and checks it against the build. A catalog from
   another deploy (the server was redeployed since the page loaded) is
   never read: the choice is remembered as in step 6, and the page reloads
   into the new language, which the new deploy's server renders;
3. it installs the catalog and rewrites every registered text node,
   attribute and markup fragment directly, with no effect per call site;
4. it notifies derived props (`TextProp`, `Signal<String>`) and closures
   that read a string;
5. it sets `<html lang dir>`, so an Arabic page turns right-to-left from the
   `dir` attribute alone;
6. it remembers the choice for the next visit: the cookie on a
   server-rendered page, `localStorage` in a client-only application.

If any other step before the install fails, the page stays as it was and
`set_locale` returns the error.

## Your own control

The simplest control that works everywhere is a link per language. It
navigates, so the page reloads in the new language, and it needs no client
code:

```rust file=calls/src/lib.rs
#[component]
pub fn LanguageLinks() -> impl IntoView {
    view! {
        <nav aria-label=tr!("language.label")>
            <ul class="languages">
                <li><a href="?lang=en" hreflang="en" lang="en">{tr!("language.en")}</a></li>
                <li><a href="?lang=fr" hreflang="fr" lang="fr">{tr!("language.fr")}</a></li>
            </ul>
        </nav>
    }
}
```

To switch live from your own control, call `set_locale` where there is
client code: under this application's `hydrate` feature (a client-only
application's is `csr`). `preload_locale` fetches and checks a catalog without
switching to it. Call it when the pointer or keyboard focus reaches a
control, so that the switch itself is instant:

```rust file=calls/src/lib.rs
/// A button that switches live. It needs client code, so it does nothing
/// before hydration: prefer `LocaleSwitcher` unless the page cannot work
/// without the wasm anyway.
#[component]
pub fn SwitchButton(tag: &'static str, children: Children) -> impl IntoView {
    view! {
        <button
            type="button"
            lang=tag
            on:click=move |_| switch_to(tag)
            on:pointerenter=move |_| warm(tag)
            on:focus=move |_| warm(tag)
        >
            {children()}
        </button>
    }
}

#[cfg(feature = "hydrate")]
fn switch_to(tag: &'static str) {
    leptos::task::spawn_local(async move {
        // On failure the page is left as it was.
        let _ = leptos_mf2::set_locale(tag).await;
    });
}

#[cfg(feature = "hydrate")]
fn warm(tag: &'static str) {
    leptos::task::spawn_local(async move {
        let _ = leptos_mf2::preload_locale(tag).await;
    });
}

// The server renders the button; nothing clicks it there.
#[cfg(feature = "ssr")]
fn switch_to(_: &'static str) {}

#[cfg(feature = "ssr")]
fn warm(_: &'static str) {}
```

## The current language

`html_lang()` returns the current language and its direction
(`("fr", "ltr")`). It reads the catalog in force, which is not a signal. In
the browser, `track_locale()` subscribes the code that calls it to the
next switch. Together they make a value that follows the language:

```rust file=calls/src/lib.rs
/// The page's language, following a switch.
pub fn current_language() -> String {
    // A request's language never changes on the server.
    #[cfg(not(feature = "ssr"))]
    leptos_mf2::track_locale();
    leptos_mf2::html_lang().0
}
```

Use it as a closure in a view, `content=current_language`. The
[accessibility](accessibility.md) page uses it for schema.org's
`inLanguage`.
