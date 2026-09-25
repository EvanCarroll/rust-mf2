//! Support module of the `fluent-view` template: the `leptos-fluent`
//! initializer, as its documentation writes it (0.3.1).
//!
//! The messages are the workload's Fluent files, `../ftl/<tag>/*.ftl`
//! relative to the app's `Cargo.toml`; the language comes from a cookie,
//! else the request's `Accept-Language`, else `en`.

use leptos::prelude::*;
use leptos_fluent::leptos_fluent;

/// Provides the `leptos-fluent` context to everything under it.
#[component]
pub fn I18nProvider(children: Children) -> impl IntoView {
    leptos_fluent! {
        children: children(),
        locales: "../ftl",
        default_language: "en",
        sync_html_tag_lang: true,
        sync_html_tag_dir: true,
        cookie_name: "lang",
        initial_language_from_cookie: true,
        set_language_to_cookie: true,
        initial_language_from_accept_language_header: true,
    }
}
