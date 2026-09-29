//! Renders the L6 page on the server and writes everything the browser needs
//! into `target/l6-web/` (`plans/14-phase-6-work-order.md` A6b).
//!
//! The page is written by the **same view** the client hydrates, in the same
//! build of the same crate, which is the only way the comparison means
//! anything.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use leptos::prelude::*;
use mf2::leptos::RequestI18n;
use mf2_l6_web::{PAGE_LOCALE, Page, TWIN_LOCALE, setup};
use tachys::view::RenderHtml;

/// The boot script and the hydration globals leptos expects on a page it
/// hydrates. Written by hand because there is no `cargo leptos` here: this is
/// a conformance harness, not an application.
const HEAD: &str = r#"<meta charset="utf-8">
<title>mf2 conformance L6</title>
<script>__RESOLVED_RESOURCES=[];__SERIALIZED_ERRORS=[];__PENDING_RESOURCES=[];__RESOURCE_RESOLVERS=[];__INCOMPLETE_CHUNKS=[];</script>
<script type="module">
import init, { hydrate } from './pkg/mf2_l6_web.js';
await init();
hydrate();
</script>"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("this crate sits two directories below the root")?
        .to_path_buf();
    let out = root.join("target/l6-web");
    std::fs::create_dir_all(out.join("i18n"))?;

    mf2::leptos::install(setup());
    mf2::leptos::install_catalogs(mf2_l6_web::CATALOGS)?;

    // Every catalog, under the content-hashed name the page will ask for.
    for entry in mf2::leptos::catalog_entries() {
        std::fs::write(out.join("i18n").join(entry.file), entry.bytes)?;
    }

    let catalog = mf2::leptos::catalog(PAGE_LOCALE).ok_or("the page's catalog is not installed")?;
    let body = Owner::new().with(|| {
        RequestI18n::new(Arc::clone(&catalog)).provide();
        RenderHtml::to_html(Page())
    });

    let links = link_tags();
    let html = format!(
        "<!DOCTYPE html><html lang=\"{PAGE_LOCALE}\" dir=\"ltr\"><head>{HEAD}{links}</head>\
         <body>{body}</body></html>"
    );
    write(&out.join("index.html"), html.as_bytes())?;

    // What the check needs to know about this run, so that it asserts on the
    // page it was given rather than on constants of its own.
    let manifest = format!(
        "{{\"locale\":\"{PAGE_LOCALE}\",\"twin\":\"{TWIN_LOCALE}\",\"cases\":{}}}\n",
        mf2_l6_web::CASES.len()
    );
    write(&out.join("page.json"), manifest.as_bytes())?;

    println!(
        "l6-page: {} cases, {} catalogs → {}",
        mf2_l6_web::CASES.len(),
        mf2::leptos::catalog_entries().len(),
        out.display()
    );
    Ok(())
}

/// The preload link for this page's catalog, and the map the switch reads —
/// the same two the shell of a real application emits (04 §6).
fn link_tags() -> String {
    let mut out = String::new();
    for entry in mf2::leptos::catalog_entries() {
        let href = mf2::leptos::links::catalog_href(entry.file);
        if entry.tag == PAGE_LOCALE {
            out.push_str(&format!(
                "<link rel=\"preload\" as=\"fetch\" crossorigin=\"anonymous\" href=\"{href}\" data-mf2>"
            ));
        }
        // Every locale, this one included: after a switch, the page's own
        // locale is no longer the one the preload link names, and coming
        // home must not need a round trip either.
        out.push_str(&format!(
            "<link rel=\"mf2-catalog\" data-mf2-locale=\"{}\" href=\"{href}\">",
            entry.tag
        ));
    }
    out
}

fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}
