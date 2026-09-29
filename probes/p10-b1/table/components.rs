//! B1's measurement variant: A7's function table (v3), in the merged
//! crate's layout. Not committed; the static-dispatch form is.

use alloc::string::String;
#[cfg(feature = "ssr")]
use alloc::vec::Vec;

use mf2_catalog::Dir;

use crate::line::leptos::IntoView;
use crate::line::ui;

pub use ui::{
    AlternateLinksProps, CatalogLinksProps, CatalogPreloadProps, IslandsGateProps,
    LocaleOptionProps, LocaleSwitcherProps,
};

/// The preload link (the table form).
#[must_use]
#[allow(non_snake_case)]
pub fn CatalogPreload() -> impl IntoView {
    install_table();
    ui::CatalogPreload()
}

/// The catalog links (the table form).
#[must_use]
#[allow(non_snake_case)]
pub fn CatalogLinks() -> impl IntoView {
    install_table();
    ui::CatalogLinks()
}

/// The islands gate (the table form).
#[must_use]
#[allow(non_snake_case)]
pub fn IslandsGate() -> impl IntoView {
    install_table();
    ui::IslandsGate()
}

/// The hreflang links (the table form).
#[must_use]
#[allow(non_snake_case)]
pub fn AlternateLinks(props: AlternateLinksProps) -> impl IntoView {
    install_table();
    ui::AlternateLinks(props)
}

/// One option (the table form).
#[must_use]
#[allow(non_snake_case)]
pub fn LocaleOption(props: LocaleOptionProps) -> impl IntoView {
    install_table();
    ui::LocaleOption(props)
}

/// The switcher (the table form), with the live switch installed.
#[must_use]
#[allow(non_snake_case)]
pub fn LocaleSwitcher(props: LocaleSwitcherProps) -> impl IntoView {
    install_table();
    #[cfg(any(feature = "hydrate", feature = "csr"))]
    ui::install_switch(switch);
    ui::LocaleSwitcher(props)
}

/// The `lang` and `dir` the shell should put on `<html>`.
#[must_use]
pub fn html_lang() -> (String, &'static str) {
    match crate::leptos::catalog::active() {
        Some(catalog) => (
            catalog.locale().into(),
            if catalog.dir() == Dir::Rtl {
                "rtl"
            } else {
                "ltr"
            },
        ),
        None => (crate::leptos::state::source_locale().into(), "ltr"),
    }
}

static TABLE: ui::Table = ui::Table {
    locales: crate::leptos::state::locales,
    source_locale: crate::leptos::state::source_locale,
    is_current,
    locale_query: crate::leptos::links::LOCALE_QUERY,
    islands_gate: crate::leptos::links::ISLANDS_GATE,
    #[cfg(feature = "ssr")]
    preload,
    #[cfg(feature = "ssr")]
    catalog_links,
    #[cfg(feature = "ssr")]
    catalog_link_rel: crate::leptos::links::CATALOG_LINK_REL,
};

fn install_table() {
    ui::install(&TABLE);
}

fn is_current(tag: &str) -> bool {
    match crate::leptos::catalog::active() {
        Some(catalog) => catalog.locale() == tag,
        None => crate::leptos::state::source_locale() == tag,
    }
}

#[cfg(feature = "ssr")]
fn preload() -> Option<(String, Option<String>)> {
    let href = crate::leptos::catalog::active()
        .and_then(|catalog| crate::leptos::catalog::catalog_name(catalog.locale()))
        .map(crate::leptos::links::catalog_href)?;
    let zone = crate::leptos::catalog::request_time_zone()
        .and_then(|zone| crate::leptos::zone::zone_name(&zone).map(String::from));
    Some((href, zone))
}

#[cfg(feature = "ssr")]
fn catalog_links() -> Vec<(&'static str, String)> {
    crate::leptos::catalog::catalog_entries()
        .iter()
        .map(|entry| (entry.tag, crate::leptos::links::catalog_href(entry.file)))
        .collect()
}

#[cfg(any(feature = "hydrate", feature = "csr"))]
fn switch(tag: String) {
    crate::line::leptos::task::spawn_local(async move {
        if let Err(error) = crate::leptos::set_locale(&tag).await {
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(match error {
                crate::error::LoadError::UnknownLocale => "mf2: no such locale",
                _ => "mf2: the locale could not be switched; the page is unchanged",
            }));
        }
    });
}
