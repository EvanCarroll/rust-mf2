//! The server (`plans/04-leptos-integration.md` §6) — the same wiring as
//! `examples/demo-ssr`'s. Islands change the client, not the server.
//!
//! The generated `install()`, the `Negotiator` as a layer, and the catalog
//! routes; the routes and the file/error handler are Leptos's plain forms.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::Router;
    use demo_islands::{App, shell};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler, generate_route_list};
    use mf2::axum::{AcceptLanguage, CookieLocale, Negotiator, QueryParam};

    // The generated module, installed once: the setup and the catalogs,
    // each checked against the manifest hash.
    demo_islands_i18n::install();

    // Negotiation: an ordered list. A query parameter first, so that a link
    // can force a locale for a screenshot or a test; then the cookie the
    // switcher wrote; then what the browser asked for.
    let negotiator = Negotiator::empty()
        .source(QueryParam::default())
        .source(CookieLocale::default())
        .source(AcceptLanguage)
        .sink(CookieLocale {
            // The demo is served over plain HTTP on localhost.
            secure: false,
            ..CookieLocale::default()
        });

    let conf = get_configuration(None)?;
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let app = Router::new()
        // `/i18n/*`: the catalogs, immutable and precompressed.
        .merge(mf2::axum::catalog_routes())
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(file_and_error_handler(shell))
        .layer(negotiator)
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {
    // The client is the `cdylib` half of this crate; `cargo leptos` builds
    // both from one manifest.
}
