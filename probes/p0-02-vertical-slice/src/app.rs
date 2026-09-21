//! The probe application: every call-site position of plans/04 §2 that P0.2
//! needs (text child, attribute, reactive prop, `Signal<String>`, `if`/`else`,
//! non-view `String`, server function), one lazy route, four streaming modes
//! and the P0.10 hydration-tolerance pages.

use crate::msg;
use leptos::prelude::*;
use leptos_meta::{Title, provide_meta_context};
use leptos_router::{
    Lazy, LazyRoute, SsrMode,
    components::{A, Route, Router, Routes},
    lazy_route, path,
};

#[cfg(feature = "ssr")]
const CSS: &str = r"
:root { color-scheme: light; --fg: #1a1a1a; --bg: #ffffff; --accent: #0b57d0; }
* { box-sizing: border-box; }
body { margin: 0; font: 16px/1.5 system-ui, sans-serif; color: var(--fg); background: var(--bg); }
.bar { display: flex; flex-wrap: wrap; gap: 1rem; align-items: center; justify-content: space-between;
       padding: 0.75rem 1rem; border-block-end: 1px solid #767676; }
nav ul { display: flex; flex-wrap: wrap; gap: 0.75rem; list-style: none; margin: 0; padding: 0; }
a { color: var(--accent); }
a:focus-visible, button:focus-visible, select:focus-visible, input:focus-visible { outline: 3px solid var(--accent); outline-offset: 2px; }
.switcher, .field { display: flex; gap: 0.5rem; align-items: center; }
main { display: flex; flex-direction: column; gap: 0.75rem; padding: 1rem; max-inline-size: 60rem; }
button, select, input { font: inherit; min-block-size: 2.25rem; padding-inline: 0.5rem; }
";

/// The document shell — server only. `<html lang dir>` and the catalog
/// preload come from the per-request context (render time, D9).
#[cfg(feature = "ssr")]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    use leptos_meta::MetaTags;
    let i18n = tr::server::current();
    let lang = i18n.catalog.locale().to_owned();
    let dir = i18n.catalog.dir().as_str();
    let href = i18n.href.to_string();
    view! {
        <!DOCTYPE html>
        <html lang=lang dir=dir>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                // The boot data: first in <head> so the catalog request starts
                // before (and runs alongside) the JS/wasm requests.
                <link rel="preload" r#as="fetch" crossorigin="anonymous" href=href data-mf2=""/>
                <link rel="icon" href="data:,"/>
                <style>{CSS}</style>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        // `<Title>` sets document.title from a client effect reading a String:
        // one of the reasons hydration waits for the catalog (plans/04 §6).
        <Title text=msg::APP_TITLE/>
        <Router>
            <header class="bar">
                <nav aria-label=msg::MAIN_NAV_LABEL>
                    <ul>
                        <li><A href="/">{msg::NAV_HOME}</A></li>
                        <li><A href="/lazy">{msg::NAV_LAZY}</A></li>
                        <li><A href="/stream/ooo">{msg::NAV_STREAM_OOO}</A></li>
                        <li><A href="/stream/inorder">{msg::NAV_STREAM_INORDER}</A></li>
                        <li><A href="/stream/blocked">{msg::NAV_STREAM_BLOCKED}</A></li>
                        <li><A href="/stream/async">{msg::NAV_STREAM_ASYNC}</A></li>
                        <li><A href="/p010/text">{msg::NAV_P010_TEXT}</A></li>
                        <li><A href="/p010/struct">{msg::NAV_P010_STRUCT}</A></li>
                    </ul>
                </nav>
                <LocaleSwitcher/>
            </header>
            <main>
                <Routes fallback=|| view! { <h1 id="not-found">{msg::NOT_FOUND}</h1> }>
                    <Route path=path!("/") view=HomePage/>
                    <Route path=path!("/lazy") view={Lazy::<LazyPage>::new()}/>
                    <Route path=path!("/stream/ooo") view=StreamPage ssr=SsrMode::OutOfOrder/>
                    <Route path=path!("/stream/inorder") view=StreamPage ssr=SsrMode::InOrder/>
                    <Route path=path!("/stream/blocked") view=BlockedStreamPage ssr=SsrMode::PartiallyBlocked/>
                    <Route path=path!("/stream/async") view=StreamPage ssr=SsrMode::Async/>
                    <Route path=path!("/p010/text") view=crate::p010::TextMismatch/>
                    <Route path=path!("/p010/struct") view=crate::p010::StructMismatch/>
                    <Route path=path!("/p010/struct-tr") view=crate::p010::StructMismatchTr/>
                </Routes>
            </main>
        </Router>
    }
}

/// A labelled native control; each language is named in its own language with
/// its own `lang` (plans/04 §9). The autonyms are catalog messages too (the
/// same text in every catalog), so no locale text is compiled into the wasm.
#[component]
fn LocaleSwitcher() -> impl IntoView {
    let current = tr::current_locale();
    let on_change = move |ev: leptos::ev::Event| {
        let tag = event_target_value(&ev);
        #[cfg(feature = "hydrate")]
        leptos::task::spawn_local(async move {
            if let Err(e) = tr::client::set_locale(&tag).await {
                leptos::leptos_dom::logging::console_error(e.code());
            }
        });
        #[cfg(not(feature = "hydrate"))]
        let _ = tag;
    };
    view! {
        <div class="switcher">
            <label for="locale-select">{msg::LANGUAGE_LABEL}</label>
            <select id="locale-select" on:change=on_change>
                <option value="en" lang="en" dir="ltr" selected=current == "en">{msg::AUTONYM_EN}</option>
                <option value="ar" lang="ar" dir="rtl" selected=current == "ar">{msg::AUTONYM_AR}</option>
            </select>
        </div>
    }
}

/// Server function: a non-view `String` formatted on the server from the
/// request context provided through `handle_server_fns_with_context`.
#[server]
pub async fn server_greeting() -> Result<String, ServerFnError> {
    Ok(msg::SERVER_GREETING.to_string())
}

#[component]
fn Labelled(#[prop(into)] label: TextProp, id: &'static str) -> impl IntoView {
    view! { <p id=id>{move || label.get()}</p> }
}

#[component]
fn HomePage() -> impl IntoView {
    let (show_b, set_show_b) = signal(false);
    let (server_msg, set_server_msg) = signal(None::<String>);
    let (from_handler, set_from_handler) = signal(None::<String>);
    let signal_text: Signal<String> = msg::SIGNAL_DEMO.into();
    let on_toggle = move |_| {
        set_show_b.update(|b| *b = !*b);
        // Non-view String from an event handler: MUST NOT warn (no observer).
        set_from_handler.set(Some(msg::TOGGLE_BUTTON.to_string()));
    };
    let on_ask = move |_| {
        leptos::task::spawn_local(async move {
            if let Ok(s) = server_greeting().await {
                set_server_msg.set(Some(s));
            }
        });
    };
    view! {
        <h1 id="home-heading">{msg::NAV_HOME}</h1>
        <p id="welcome">{msg::WELCOME}</p>
        <p id="canary">{msg::CANARY}</p>
        <div class="field">
            <label for="search">{msg::SEARCH_LABEL}</label>
            <input id="search" type="search" placeholder=msg::SEARCH_PLACEHOLDER/>
        </div>
        <Labelled id="prop-demo" label=msg::PROP_DEMO/>
        <p id="signal-demo">{signal_text}</p>
        <div class="field">
            <button id="toggle" on:click=on_toggle>{msg::TOGGLE_BUTTON}</button>
            <button id="ask-server" on:click=on_ask>{msg::SERVER_FN_BUTTON}</button>
        </div>
        <p id="if-else">{move || if show_b.get() { msg::OPTION_B } else { msg::OPTION_A }}</p>
        <p id="from-handler">{move || from_handler.get()}</p>
        <p id="server-msg">{move || server_msg.get()}</p>
    }
}

// ------------------------------------------------------------------ lazy route

/// Lazy route: `view` is compiled into its own wasm chunk under `--split`.
#[derive(Debug)]
pub struct LazyPage;

#[lazy_route]
impl LazyRoute for LazyPage {
    fn data() -> Self {
        Self
    }

    fn view(this: Self) -> AnyView {
        let _ = this;
        view! {
            <h1 id="lazy-heading" title=msg::LAZY_TITLE_ATTR>{msg::LAZY_HEADING}</h1>
            <p id="lazy-body">{msg::LAZY_BODY}</p>
            // Reads the thread-local state that lives in the main module.
            <p id="lazy-locale">{move || tr::current_locale()}</p>
        }
        .into_any()
    }
}

// ------------------------------------------------------------------- streaming

async fn slow_value(ms: u64) -> u32 {
    #[cfg(feature = "ssr")]
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    #[cfg(not(feature = "ssr"))]
    let _ = ms;
    42
}

/// Two Suspense boundaries whose `Tr` values render only after a resource
/// resolves — i.e. in a later poll of the response stream (D9's question).
#[component]
fn StreamPage() -> impl IntoView {
    let first = Resource::new(|| (), |()| slow_value(300));
    let second = Resource::new(|| (), |()| slow_value(600));
    view! {
        <h1 id="stream-heading">{msg::STREAM_HEADING}</h1>
        <Suspense fallback=|| view! { <p id="loading-1">{msg::LOADING}</p> }>
            {move || Suspend::new(async move {
                let n = first.await;
                view! {
                    <section id="streamed-1" aria-label=msg::STREAM_HEADING>
                        <p id="streamed-body">{msg::STREAM_BODY}</p>
                        <p id="streamed-body-2">{msg::STREAM_BODY_2}</p>
                        <p id="streamed-value">{n}</p>
                    </section>
                }
            })}
        </Suspense>
        <Suspense fallback=|| view! { <p id="loading-2">{msg::LOADING}</p> }>
            // The other common shape: a closure reading the resource.
            {move || second.get().map(|n| view! {
                <section id="streamed-2">
                    <p id="streamed-body-b">{msg::STREAM_BODY}</p>
                    <p id="streamed-value-b">{n}</p>
                </section>
            })}
        </Suspense>
    }
}

/// `SsrMode::PartiallyBlocked` with a blocking resource: the server replaces
/// the fallback itself before sending.
#[component]
fn BlockedStreamPage() -> impl IntoView {
    let blocking = Resource::new_blocking(|| (), |()| slow_value(300));
    view! {
        <h1 id="stream-heading">{msg::STREAM_HEADING}</h1>
        <Suspense fallback=|| view! { <p id="loading-1">{msg::LOADING}</p> }>
            {move || Suspend::new(async move {
                let n = blocking.await;
                view! {
                    <section id="streamed-1" aria-label=msg::STREAM_HEADING>
                        <p id="streamed-body">{msg::STREAM_BODY}</p>
                        <p id="streamed-body-2">{msg::STREAM_BODY_2}</p>
                        <p id="streamed-value">{n}</p>
                    </section>
                }
            })}
        </Suspense>
    }
}
