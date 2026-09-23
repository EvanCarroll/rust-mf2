//! Renders one set's L7 pages and writes everything the browser needs into
//! a directory (`plans/15-phase-7-work-order.md` A4):
//!
//! ```text
//! l7-page <locale> <dir>
//! ```
//!
//! * `islands-<locale>.html` — the islands page, server-rendered by the same
//!   view the client hydrates, with Leptos' own island script;
//! * `csr-<locale>.html` — the client-only page: an empty body, the index
//!   preload and the boot script, as a static host would serve it;
//! * `i18n/` — the set's catalogs under their content-hashed names, and
//!   `index-<locale>.json`, the client-only page's catalog index;
//! * `<locale>.json` — what the check needs: every call site's id and the
//!   text the server renders for it in the locale and in the twin. That is
//!   what a client-only page, which has no server text to agree with, is
//!   compared against.
//!
//! One set per run, because `leptos_mf2::install` is once per process.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use leptos::config::LeptosOptions;
use leptos::hydration::HydrationScripts;
use leptos::prelude::*;
use leptos::serde_json::{Value, json};
use leptos_mf2::{IslandsGate, RequestI18n};
use mf2_l7_web::{Cases, SET_ATTR, SETS, TWIN, set_index};
use tachys::view::RenderHtml;

/// The name `wasm-bindgen` gives the client's files, in both page kinds.
const OUTPUT_NAME: &str = "mf2_l7_web";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(locale), Some(out)) = (args.next(), args.next()) else {
        return Err("usage: l7-page <locale> <dir>".into());
    };
    let out = PathBuf::from(out);
    let index = set_index(&locale).ok_or_else(|| format!("no set for {locale:?}"))?;
    let set = &SETS[index];
    std::fs::create_dir_all(out.join("i18n"))?;

    leptos_mf2::install((set.setup)());
    leptos_mf2::install_catalogs(set.catalogs)?;
    for entry in leptos_mf2::catalog_entries() {
        std::fs::write(out.join("i18n").join(entry.file), entry.bytes)?;
    }
    let page = leptos_mf2::catalog(&locale).ok_or("the set's catalog is not installed")?;
    let twin = leptos_mf2::catalog(TWIN).ok_or("the twin's catalog is not installed")?;

    // The islands page, rendered as a request in the set's locale would be.
    let (lang, dir, scripts, body) = Owner::new().with(|| {
        RequestI18n::new(Arc::clone(&page)).provide();
        let (lang, dir) = leptos_mf2::html_lang();
        let options = LeptosOptions::builder()
            .output_name(OUTPUT_NAME)
            .site_pkg_dir("pkg-islands")
            .build();
        let scripts = RenderHtml::to_html(view! { <HydrationScripts options islands=true /> });
        let body = RenderHtml::to_html(view! {
            // First, and outside every island: the walk waits here.
            <IslandsGate />
            <Cases set=index islands=true />
        });
        (lang, dir, scripts, body)
    });
    let islands = format!(
        "<!DOCTYPE html><html lang=\"{lang}\" dir=\"{dir}\" {SET_ATTR}=\"{locale}\"><head>\
         <meta charset=\"utf-8\"><title>mf2 conformance L7: islands, {locale}</title>\
         {scripts}{links}</head><body>{body}</body></html>",
        links = link_tags(&locale),
    );
    write(&out.join(format!("islands-{locale}.html")), &islands)?;

    // The client-only page: nothing rendered, and `lang` a placeholder the
    // boot replaces with the locale it chooses.
    let csr = format!(
        "<!DOCTYPE html><html lang=\"en\" dir=\"ltr\" {SET_ATTR}=\"{locale}\"><head>\
         <meta charset=\"utf-8\"><title>mf2 conformance L7: client-only, {locale}</title>\
         <link rel=\"preload\" as=\"fetch\" crossorigin=\"anonymous\" href=\"i18n/index-{locale}.json\" data-mf2-index>\
         <script type=\"module\">import init, {{ start }} from './pkg-csr/{OUTPUT_NAME}.js'; \
         await init(); start();</script></head><body></body></html>"
    );
    write(&out.join(format!("csr-{locale}.html")), &csr)?;

    let mut catalogs = leptos::serde_json::Map::new();
    for entry in leptos_mf2::catalog_entries() {
        catalogs.insert(entry.tag.to_owned(), Value::from(entry.file));
    }
    write(
        &out.join("i18n").join(format!("index-{locale}.json")),
        &Value::Object(catalogs).to_string(),
    )?;

    // Each call site as the server renders it, alone, in both locales.
    let text_in = |catalog: &Arc<mf2::Catalog>, i: usize| {
        Owner::new().with(|| {
            RequestI18n::new(Arc::clone(catalog)).provide();
            text_of(&RenderHtml::to_html((set.view)(i)))
        })
    };
    let cases: Vec<Value> = (0..(set.len)())
        .map(|i| {
            json!({
                "id": (set.id)(i).unwrap_or_default(),
                "text": text_in(&page, i),
                "twin": text_in(&twin, i),
            })
        })
        .collect();
    let manifest = json!({
        "locale": locale,
        "twin": TWIN,
        "dir": dir,
        "configuration": if cfg!(feature = "all-functions") { "all" } else { "default" },
        "cases": cases,
    });
    write(&out.join(format!("{locale}.json")), &manifest.to_string())?;

    println!(
        "l7-page: {locale}: {} cases, {} catalogs → {}",
        (set.len)(),
        leptos_mf2::catalog_entries().len(),
        out.display()
    );
    Ok(())
}

/// The preload link for the page's catalog, and the map the switch reads —
/// what the shell of a real application emits (04 §6).
fn link_tags(locale: &str) -> String {
    let mut out = String::new();
    for entry in leptos_mf2::catalog_entries() {
        let href = leptos_mf2::links::catalog_href(entry.file);
        if entry.tag == locale {
            let _ = write!(
                out,
                "<link rel=\"preload\" as=\"fetch\" crossorigin=\"anonymous\" href=\"{href}\" data-mf2>"
            );
        }
        let _ = write!(
            out,
            "<link rel=\"mf2-catalog\" data-mf2-locale=\"{}\" href=\"{href}\">",
            entry.tag
        );
    }
    out
}

/// The text of a fragment tachys wrote: its tags and comments (hydration
/// markers among them) dropped, and the three entities it escapes a text
/// node with undone. The input is HTML *we* wrote, so a `<` is always a
/// tag; one in the text is `&lt;`.
fn text_of(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        text.push_str(&rest[..at]);
        rest = rest[at..].find('>').map_or("", |end| &rest[at + end + 1..]);
    }
    text.push_str(rest);
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}
