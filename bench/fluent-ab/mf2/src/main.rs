//! The migrated application's server, finished by hand as
//! docs/migrating-from-leptos-fluent.md says: the generated server of the
//! `fluent-view` application, with the translation crate installed by
//! `mf2_axum::install` and the initializer's negotiation — a `lang` cookie,
//! else `Accept-Language`, else the source locale — as `mf2_axum`'s
//! `Negotiator`, given to every `_with_context` entry point.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler_with_context, generate_route_list};
    use mf2_axum::{AcceptLanguage, CookieLocale, Negotiator};
    use std::sync::Arc;
    use workload_app_mf2::app::{App, shell};

    mf2_axum::install(workload_i18n::setup(), workload_i18n::CATALOGS)?;
    let negotiator = Arc::new(
        Negotiator::empty()
            .source(CookieLocale::default())
            .source(AcceptLanguage)
            .sink(CookieLocale {
                // Served over plain HTTP on localhost.
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
fn main() {}
