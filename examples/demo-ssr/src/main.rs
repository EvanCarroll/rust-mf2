//! The server (`plans/04-leptos-integration.md` §12.5).
//!
//! The generated `install()`, the `Negotiator` as a layer, and the catalog
//! routes. The routes and the file/error handler are Leptos's plain forms:
//! the render finds the negotiated language in the request itself.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::Router;
    use demo_ssr::{App, shell};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler, generate_route_list};
    use mf2::axum::{AcceptLanguage, CookieLocale, Negotiator, PathPrefix, QueryParam};

    // The generated module, installed once: the setup and the catalogs,
    // each checked against the manifest hash.
    demo_i18n::install();

    // Negotiation: an ordered list. A path prefix first, for the pages whose
    // language is in their URL (`/fr/about`; any other path names no locale
    // and falls through); then a query parameter, so that a link can force a
    // locale for a screenshot or a test; then the cookie the switcher wrote;
    // then what the browser asked for.
    let negotiator = Negotiator::empty()
        .source(PathPrefix)
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
        // On a page whose language is in its URL, the switcher's form
        // (without the wasm) submits `?lang=`, which the path outranks: send
        // it to that language's URL instead. Under the negotiator, which
        // tells it the query's name.
        .layer(axum::middleware::from_fn(mf2::axum::path_prefix_redirect))
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
