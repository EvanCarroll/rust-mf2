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
result in the response — `CookieLocale` as a sink writes the cookie only
for an explicit choice, a language that came from `?lang=` or the path,
never for a guess from `Accept-Language` or the default:

| Source | Reads |
|---|---|
| `QueryParam` | `?lang=fr` (the name can be changed): how a link, a test, or the switcher's form chooses |
| `CookieLocale` | the `mf2_locale` cookie: a choice the reader made earlier |
| `AcceptLanguage` | the browser's languages, in quality order |
| `PathPrefix` | `/fr/…`, for a site whose languages have their own URLs |

If no source matches, the source language is used, unless
`.default_locale(…)` names another. The response carries
`Content-Language` and a `Vary` that names each source's header, and
`Cookie` as well, because the server also reads the reader's time zone
from the `mf2_tz` cookie. The
server is the **only** place a language is negotiated. The client reads it
from the page (`<html lang>` and the preload link), so hydration cannot
choose differently.

Getting started lists the sources as query, cookie, then
`Accept-Language`. A site with a language in its URLs puts the path first.
A `?lang=` then cannot change the language of `/en/…`, so the switcher's
form, which submits one, needs the server to send it on to the other
language's URL: `path_prefix_redirect` answers `/en/page?lang=fr` with a
redirect to `/fr/page`.

```rust file=calls/src/lib.rs
/// A site whose pages live under `/en/…` and `/fr/…`.
#[cfg(feature = "ssr")]
pub fn path_negotiator() -> mf2_axum::Negotiator {
    use mf2_axum::{AcceptLanguage, CookieLocale, Negotiator, PathPrefix, QueryParam};
    Negotiator::empty()
        .source(PathPrefix)
        .source(QueryParam::default())
        .source(CookieLocale::default())
        .source(AcceptLanguage)
        .sink(CookieLocale::default())
}

/// The redirect, as a layer on the application's router.
#[cfg(feature = "ssr")]
pub fn with_path_redirect(router: axum::Router) -> axum::Router {
    router.layer(axum::middleware::from_fn(mf2_axum::path_prefix_redirect))
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

## The server's options

`Negotiator::default()` is what a site gets when it says nothing more: the
cookie, then `Accept-Language`, with the cookie as its sink. A site that
wants `?lang=` too, or another order, starts from `Negotiator::empty()`
and lists its sources, as Getting started does. Each part can be changed:

* **`QueryParam("hl")`** reads another parameter name than `lang`.
  `QueryParam::default()` is `lang`, the name `<LocaleSwitcher>`'s form
  submits.
* **`CookieLocale`'s fields**: `name` (`mf2_locale`), `max_age` (a year,
  in seconds), `path` (`/`), `same_site` (`Lax`) and `secure` (`true`).
  The client writes the cookie under `mf2_locale` on every switch, so a
  different `name` is for an **extra** source that reads a cookie another
  system wrote, listed after the default one. A `Secure` cookie is not
  stored from a page served over plain HTTP, so a development server
  without TLS sets `secure: false` (Getting started ties it to
  `debug_assertions`).
* **`.default_locale("fr")`** answers a request no source matched in
  French instead of the source language, if the build has French.
* **`Negotiator::over(locales, default)`** starts from an explicit table
  of tags and directions instead of the build's, for a site that offers
  fewer languages than it built, or a test. `Negotiator::locales()` (and,
  anywhere on the server or the client, `leptos_mf2::locales()`) returns
  the table in use: the tags and their directions, in build order — for a
  sitemap or a list of `hreflang` links.

```rust file=calls/src/lib.rs
/// `?hl=` first, a cookie an older version of the site wrote after this
/// library's own, and French for a request nothing matches.
#[cfg(feature = "ssr")]
pub fn custom_negotiator() -> mf2_axum::Negotiator {
    use mf2_axum::{AcceptLanguage, CookieLocale, Negotiator, QueryParam};
    Negotiator::empty()
        .source(QueryParam("hl"))
        .source(CookieLocale::default())
        .source(CookieLocale {
            name: "site_lang",
            ..CookieLocale::default()
        })
        .source(AcceptLanguage)
        .sink(CookieLocale {
            secure: !cfg!(debug_assertions),
            ..CookieLocale::default()
        })
        .default_locale("fr")
}
```

**What was negotiated.** `provide_locale` returns it, and
`mf2_axum::negotiated()` gives it to any component that renders in the
request: the tag, its direction, `from` (the source that matched, such as
`"query"` or `"cookie"`, or `"default"`) and `matched`.

```rust file=calls/src/lib.rs
/// Says where the page's language came from, on the server.
#[cfg(feature = "ssr")]
#[component]
pub fn LanguageOrigin() -> impl IntoView {
    let from = mf2_axum::negotiated().map_or("default", |n| n.from);
    view! { <meta name="language-origin" content=from /> }
}
```

**Your own sources and sinks.** A source implements `LocaleSource`: a
name, the request header it reads (for `Vary`), and the tags a request
offers, best first. A sink implements `LocaleSink`: a name, and the header
it adds to the response for what was negotiated. A subdomain as a source:

```rust file=calls/src/lib.rs
/// `fr.example.com` → `fr`.
#[cfg(feature = "ssr")]
#[derive(Debug)]
pub struct Subdomain;

#[cfg(feature = "ssr")]
impl mf2_axum::LocaleSource for Subdomain {
    fn name(&self) -> &'static str {
        "subdomain"
    }

    fn vary(&self) -> Option<axum::http::HeaderName> {
        Some(axum::http::header::HOST)
    }

    fn candidates<'r>(
        &self,
        parts: &'r axum::http::request::Parts,
        out: &mut Vec<std::borrow::Cow<'r, str>>,
    ) {
        let host = parts.headers.get(axum::http::header::HOST).and_then(|h| h.to_str().ok());
        if let Some((first, _)) = host.and_then(|h| h.split_once('.')) {
            out.push(std::borrow::Cow::Borrowed(first));
        }
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
| hydrated with `static-locale` | writes the cookie and reloads in the new language |
| not yet hydrated, hydration failed, or no wasm | submits `?lang=fr`, which the server negotiates |

The last row is why it is a form. The switcher works before the wasm
arrives and when it never does, and on an islands page it needs no island:
there the switcher is not an island, so no client code runs, and the form's
`?lang=` and the server's cookie are the switch.

**On a site whose languages live in its URLs**, give the switcher
`href_of`, the URL of the current page in a language — the shape
`<AlternateLinks/>` takes. Each option then carries its language's URL,
and pressing the button goes there instead of switching in place. Without
the wasm, the form's `?lang=` goes to the server, and
`path_prefix_redirect` (above) sends it on to the same URL:

```rust file=calls/src/lib.rs
/// This page's URL in `tag`, on a site under `/en/…` and `/fr/…`.
fn account_href(tag: &str) -> String {
    format!("/{tag}/account")
}

#[component]
pub fn AccountHeader() -> impl IntoView {
    view! {
        <leptos_mf2::LocaleSwitcher
            label=tr!("language.label")
            button=tr!("language.apply")
            href_of=account_href
        >
            <leptos_mf2::LocaleOption tag="en">{tr!("language.en")}</leptos_mf2::LocaleOption>
            <leptos_mf2::LocaleOption tag="fr">{tr!("language.fr")}</leptos_mf2::LocaleOption>
        </leptos_mf2::LocaleSwitcher>
    }
}
```

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
   redirects to it (one extra round trip, only at switch time). A
   client-only application finds it in the site's `index.json`, and has no
   `/i18n/<tag>` to ask;
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
