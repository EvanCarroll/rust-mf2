//! The Phase 6 example (`plans/14-phase-6-work-order.md` A10), with the
//! Phase 7 lazy route (`plans/15-phase-7-work-order.md` A3).
//!
//! One page that uses every position the integration supports, so that what
//! breaks is visible rather than theoretical:
//!
//! | On the page | What it exercises |
//! |---|---|
//! | the `<title>` | D9 — leptos_meta reads it *after* rendering, outside the request owner (04 §5) |
//! | the heading and the tagline | a `simple` message: tachys writes the catalog's borrowed `&str` |
//! | the search field's `placeholder` | a description as an attribute value |
//! | "N people are here" | a **signal-valued** argument: one library effect, no closure per call site (04 §4) |
//! | the hotkey line | **markup as elements** — and the `<kbd>` lands in a different place in French, which is the point (04 §7) |
//! | the published line | a date, through `:datetime`: ICU4X on the server, `Intl` in the browser |
//! | the note | a sentence left untranslated in Arabic on purpose: borrowed from English, and inside `<span lang="en" dir="ltr">` there (`mark-fallback-lang`, WCAG 3.1.2) |
//! | the echo line | a plain `String` from an event handler — no bidi isolation in it (04 §9) |
//! | the switcher | `<LocaleSwitcher>`: a labelled native control whose option text never reaches the wasm, applied by a button — live once hydrated, a plain `GET ?lang=` before |
//!
//! and a second route, `/lazy`, whose code is a wasm chunk of its own under
//! `cargo leptos --split`: text, an attribute and a markup message rendered
//! from the catalog the main module installed, switching live, and freeing
//! its registry slots when the reader leaves it. A third, `/<tag>/about`,
//! has its language in its URL: its switcher goes to the other language's
//! URL instead of switching in place.
//!
//! Layout is flexbox, the SVG is an external file, and the page carries
//! schema.org `inLanguage` so that the locale is machine-readable as well as
//! rendered.

// The whole app is one view type, and `hydrate_lazy`'s future holds it: in a
// release build its layout query nests past rustc's default depth of 128
// ("queries overflow the depth limit"), so `cargo leptos build --split
// --release` did not compile. Debug builds were unaffected.
#![recursion_limit = "256"]

use leptos::prelude::*;
use leptos_meta::{MetaTags, Title, provide_meta_context};
use demo_i18n::Locale;
use mf2::leptos::{CatalogLinks, CatalogPreload, LocaleOption, LocaleSwitcher, html_lang};
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::{Lazy, LazyRoute, lazy_route, path};
use mf2::DateTimeValue;

/// The document. `<html lang dir>` comes from the catalog this request is
/// being rendered with, so the page always says what language it is in
/// (WCAG 3.1.1) and an RTL locale lays out right-to-left without any other
/// change.
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
                <link rel="stylesheet" href="/pkg/demo_ssr.css" />
                <link rel="icon" href="/globe.svg" type="image/svg+xml" />
                // The boot data: the catalog's immutable URL, preloaded so
                // that it downloads in parallel with the wasm (§6).
                <CatalogPreload />
                // The other locales' URLs, so a switch costs no round trip.
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
        // leptos_meta evaluates this *after* rendering, outside the request
        // owner — the case P0.2 found, and what the `ssr` capture fixes.
        <Title text=demo_i18n::tr!("app-title") />

        <Router>
            // The landmarks are siblings — banner, navigation, main,
            // contentinfo — so that "skip to main" lands on the content
            // (WCAG 1.3.1, 2.4.1).
            <div class="page" itemscope itemtype="https://schema.org/WebPage">
                // schema.org: the page states its language as data as well as
                // rendering it, so a crawler and a screen reader agree.
                <meta itemprop="inLanguage" content=current_locale />
                <header class="row">
                    <h1 itemprop="name">{demo_i18n::tr!("app-title")}</h1>
                    // A choice applies on the button, never on the select's
                    // `change`, which the keyboard fires per arrow key
                    // (WCAG 3.2.2). Before the wasm loads, the form's own
                    // `GET ?lang=` switches. With no children it offers
                    // every language, each named by its `language.<tag>`.
                    <LocaleSwitcher
                        label=demo_i18n::tr!("language.label")
                        button=demo_i18n::tr!("language.apply")
                    />
                </header>
                <nav aria-label=demo_i18n::tr!("nav.label")>
                    <ul class="row nav">
                        <li><A href="/">{demo_i18n::tr!("nav.home")}</A></li>
                        <li><A href="/lazy">{demo_i18n::tr!("nav.lazy")}</A></li>
                    </ul>
                </nav>

                <main class="content">
                    <Routes fallback=|| view! { <p id="not-found">{demo_i18n::tr!("not-found")}</p> }>
                        <Route path=path!("/") view=HomePage />
                        // Under `--split`, this route's view is its own wasm
                        // chunk, fetched when the route is first matched.
                        <Route path=path!("/lazy") view={Lazy::<LazyPage>::new()} />
                        <Route path=path!("/:lang/about") view=AboutPage />
                    </Routes>
                </main>

                <footer class="row">
                    <img src="/globe.svg" alt="" width="16" height="16" />
                    <small>{demo_i18n::tr!("tagline")}</small>
                </footer>
            </div>
        </Router>
    }
}

/// The locale the page is in, following a switch: the generated
/// `current_locale()` is reactive in the browser, and the request's on the
/// server.
fn current_locale() -> &'static str {
    demo_i18n::current_locale().tag()
}

/// A page whose language is its URL's first segment, as on a site that
/// wants a crawlable URL per language (`mf2::axum::PathPrefix`). A `?lang=`
/// cannot change it, so its switcher takes each language's URL.
#[component]
fn AboutPage() -> impl IntoView {
    view! {
        <section id="about">
            <p>{demo_i18n::tr!("tagline")}</p>
            <LocaleSwitcher
                label=demo_i18n::tr!("language.label")
                button=demo_i18n::tr!("language.apply")
                href_of=about_href
            >
                <LocaleOption tag=Locale::En>{Locale::En.name()}</LocaleOption>
                <LocaleOption tag=Locale::Fr>{Locale::Fr.name()}</LocaleOption>
                <LocaleOption tag=Locale::Ar>{Locale::Ar.name()}</LocaleOption>
            </LocaleSwitcher>
        </section>
    }
}

/// This page's URL in `tag`.
fn about_href(tag: &str) -> String {
    format!("/{tag}/about")
}

#[component]
fn HomePage() -> impl IntoView {
    let count = RwSignal::new(3);
    let typed = RwSignal::new(String::new());
    // A fixed instant, so that the page is the same on every run and the
    // browser checks can compare it.
    let published = DateTimeValue::instant(1_767_225_600_000);

    view! {
        <p class="tagline">{demo_i18n::tr!("tagline")}</p>

        <section class="card">
            <label class="field">
                <span>{demo_i18n::tr!("search-label")}</span>
                // A description as an attribute value: one function in
                // the library, not one per call site.
                <input type="search" placeholder=demo_i18n::tr!("search-placeholder") />
            </label>
        </section>

        <section class="card">
            // A signal-valued argument. The count is reactive and the
            // locale is reactive, and neither costs this call site a
            // closure: `count` goes in as a `Signal`, and the node's
            // own argument effect is the library's (04 §4).
            // `role="status"`: the count changes with focus on the button,
            // so it is announced without moving focus (WCAG 4.1.3).
            <p id="people" role="status">{demo_i18n::tr!("people-online", count = count)}</p>
            <div class="row">
                <button id="add-one" on:click=move |_| *count.write() += 1>
                    {demo_i18n::tr!("add-one")}
                </button>
                <button id="reset" on:click=move |_| count.set(0)>
                    {demo_i18n::tr!("reset")}
                </button>
            </div>
        </section>

        <section class="card">
            // Markup as elements: `{#kbd}…{/kbd}` becomes a real <kbd>,
            // and in French it lands at the end of the sentence instead
            // of the middle — without the view knowing anything about
            // word order.
            <p id="hotkey">
                {demo_i18n::tr!("hotkey", kbd = |children| view! { <kbd>{children}</kbd> })}
            </p>
            <p id="published">
                {published
                    .clone()
                    .map(|when| demo_i18n::tr!("published", when = when))}
            </p>
        </section>

        <section class="card">
            // Left untranslated in Arabic on purpose, to show
            // `mark-fallback-lang`: the Arabic catalog borrows the sentence
            // from English, so there it renders inside
            // `<span lang="en" dir="ltr">` — a screen reader pronounces it
            // as English and it lays out left to right (WCAG 3.1.2). In
            // English and French it is a bare text node, as every other
            // message is. After a text child, it also carries the `<!>`
            // separator that hydration walks over.
            <p id="untranslated">
                {demo_i18n::tr!("note-label")} " " {demo_i18n::tr!("untranslated")}
            </p>
        </section>

        <section class="card">
            <label class="field">
                <span>{demo_i18n::tr!("echo-label")}</span>
                <input
                    id="echo-input"
                    type="text"
                    on:input=move |event| typed.set(event_target_value(&event))
                />
            </label>
            // A plain `String`, built in an event handler and shown as
            // data: no bidi isolation in it, because a program — and a
            // comparison — consumes it (04 §9).
            <p id="echo">{move || echo(typed.get())}</p>
        </section>

        <section class="card">
            // One sentence with a name in it, in three attributes. The
            // library decides by the attribute's name: `value=` is
            // submitted with a form and `data-*` is read by a script, so
            // they carry no bidi marks; `title=` is read by a person, so
            // the name in it is isolated (04 §9).
            <label class="field">
                <span>{demo_i18n::tr!("message-label")}</span>
                <input
                    id="message"
                    type="text"
                    name="message"
                    value=demo_i18n::tr!("greeting", name = "Ada")
                    title=demo_i18n::tr!("greeting", name = "Ada")
                    data-greeting=demo_i18n::tr!("greeting", name = "Ada")
                />
            </label>
        </section>
    }
}

/// The lazy route. Its view is compiled into a wasm chunk of its own under
/// `cargo leptos --split`, and everything in it reads state the **main**
/// module owns: the catalog `hydrate_lazy` installed, the registry its
/// nodes join (and leave, when the reader navigates away), and the trigger
/// a switch fires. Chunks share linear memory, statics and the reactive
/// owner, so there is nothing to hand across (P0.2).
#[derive(Debug)]
pub struct LazyPage;

#[lazy_route]
impl LazyRoute for LazyPage {
    fn data() -> Self {
        Self
    }

    fn view(this: Self) -> AnyView {
        let _ = this;
        let presses = RwSignal::new(0);
        view! {
            // Its own `<title>`, which a client navigation here sets and
            // leaving restores (WCAG 2.4.2).
            <Title text=demo_i18n::tr!("lazy.page-title") />
            <section class="card">
                // Text and an attribute: the two positions every page has.
                <h2 id="lazy-heading" title=demo_i18n::tr!("lazy.title")>
                    {demo_i18n::tr!("lazy.heading")}
                </h2>
                // Markup, whose node structure comes from the catalog: the
                // chunk has to see the installed one to hydrate at all.
                <p id="lazy-body">
                    {demo_i18n::tr!(
                        "lazy.body",
                        strong = |children| view! { <strong>{children}</strong> }
                    )}
                </p>
                <p>
                    {demo_i18n::tr!("lazy.locale-label")} " "
                    <code id="lazy-locale">{current_locale}</code>
                </p>
            </section>
            <section class="card">
                // The chunk's own handler and reactive text: a plain
                // closure, and a message with a signal-valued argument.
                // Both must work whether the route was loaded directly or
                // reached by a link.
                <p id="lazy-presses" role="status">
                    {demo_i18n::tr!("lazy.presses", count = presses)}
                </p>
                <div class="row">
                    <button id="lazy-add" on:click=move |_| *presses.write() += 1>
                        {demo_i18n::tr!("lazy.press")}
                    </button>
                    <output id="lazy-count">{move || presses.get()}</output>
                </div>
            </section>
        }
        .into_any()
    }
}

/// The `String` path — about half of real call sites (04 §2) — here in an
/// event-driven position: no view, no signal, just text.
fn echo(typed: String) -> String {
    if typed.is_empty() {
        demo_i18n::tr!("typed-nothing").to_string()
    } else {
        demo_i18n::tr!("typed", typed = typed).to_string()
    }
}

/// How many nodes follow the locale (D7's registry). The browser checks poll
/// it to know that hydration has finished, and read it again after a route
/// change to see that dropped nodes freed their slots.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mf2_live_nodes() -> usize {
    mf2::leptos::live_nodes()
}

/// The client's entry point: install the generated setup, then boot.
///
/// `hydrate_lazy` fetches the catalog the page was served with — reusing
/// the preload, so no extra request — validates it against `MANIFEST_HASH`,
/// installs it, and only then hydrates (§6). It is `hydrate_body` for an
/// application with lazy routes: on a page that *is* a lazy route, the
/// chunk is loaded before hydration walks it.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    demo_i18n::install();
    mf2::leptos::hydrate_lazy(App);
}
