//! The server (`plans/04-leptos-integration.md` §6).
//!
//! Three things are wired, and the third is the one that is easy to get
//! wrong: **the context goes to every `_with_context` entry point.** Route
//! list generation, the routes themselves (which also register server
//! functions), and the file/error handler. A missed one renders that path in
//! the default locale with no other symptom.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::Router;
    use demo_ssr::{App, shell};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler_with_context, generate_route_list};
    use mf2_axum::{AcceptLanguage, CookieLocale, Negotiator, QueryParam};
    use std::sync::Arc;

    // The generated module, installed once: the registry, the host, the
    // manifest hash, the locale table, and the catalogs — every one of which
    // is validated against `MANIFEST_HASH` here, so a deploy that mixes
    // builds fails at boot rather than in a request.
    mf2_axum::install(demo_i18n::setup(), demo_i18n::CATALOGS)?;

    // Negotiation: an ordered list. A query parameter first, so that a link
    // can force a locale for a screenshot or a test; then the cookie the
    // switcher wrote; then what the browser asked for.
    let negotiator = Arc::new(
        Negotiator::empty()
            .source(QueryParam::default())
            .source(CookieLocale::default())
            .source(AcceptLanguage)
            .sink(CookieLocale {
                // The demo is served over plain HTTP on localhost.
                secure: false,
                ..CookieLocale::default()
            }),
    );
    let context = {
        let negotiator = Arc::clone(&negotiator);
        move || {
            mf2_axum::provide_locale(&negotiator);
        }
    };

    let conf = get_configuration(None)?;
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let app = Router::new()
        // `/i18n/*`: the catalogs, immutable and precompressed.
        .merge(mf2_axum::catalog_routes())
        .leptos_routes_with_context(&leptos_options, routes, context.clone(), {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(file_and_error_handler_with_context(context, shell))
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
