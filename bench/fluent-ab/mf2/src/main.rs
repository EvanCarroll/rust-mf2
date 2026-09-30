//! The migrated application's server, finished by hand as
//! docs/migrating-from-leptos-fluent.md says: the generated server of the
//! `fluent-view` application, with the generated `install()` and the
//! initializer's negotiation — a `lang` cookie, else `Accept-Language`, else
//! the source locale — as `mf2::axum`'s `Negotiator`, a tower layer.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler, generate_route_list};
    use mf2::axum::{AcceptLanguage, CookieLocale, Negotiator, catalog_routes};
    use workload_app_mf2::app::{App, shell};

    workload_app_mf2::install();
    let negotiator = Negotiator::empty()
        .source(CookieLocale::default())
        .source(AcceptLanguage)
        .sink(CookieLocale {
            // Served over plain HTTP on localhost.
            secure: false,
            ..CookieLocale::default()
        });

    let conf = get_configuration(None)?;
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);
    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(file_and_error_handler(shell))
        .layer(negotiator)
        .merge(catalog_routes())
        .with_state(leptos_options);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
