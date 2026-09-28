#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;

    use axum::Router;
    use hello::{App, shell};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, file_and_error_handler_with_context, generate_route_list};
    use mf2_axum::{AcceptLanguage, CookieLocale, Negotiator, QueryParam};

    // 1. Install the translations and their catalogs, from this crate's own
    //    lib (another crate of the package). Every catalog is checked
    //    against the build here, so a deploy that mixes builds fails at
    //    start-up rather than in a request.
    mf2_axum::install(hello::setup(), hello::CATALOGS)?;

    // 2. How a request's language is chosen: the first source, in order,
    //    that names a language this build has. `?lang=` first, so a link
    //    (and the switcher's form) can choose; then the cookie a choice
    //    leaves; then the browser's `Accept-Language`.
    let negotiator = Arc::new(
        Negotiator::empty()
            .source(QueryParam::default())
            .source(CookieLocale::default())
            .source(AcceptLanguage)
            .sink(CookieLocale {
                // `Secure` except in a debug build served over plain HTTP.
                secure: !cfg!(debug_assertions),
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
        // 3. `/i18n/*`: the catalogs, served from the binary, each compressed
        //    once and kept, cached for a year (each file's name carries its
        //    content hash).
        .merge(mf2_axum::catalog_routes())
        .leptos_routes_with_context(&leptos_options, routes, context.clone(), {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(file_and_error_handler_with_context(context, shell))
        .with_state(leptos_options);

    // The lib's `tr!` from another crate (this binary), by `use`: the
    // consumer's spelling.
    {
        use hello::tr;
        let _title: mf2::Tr = tr!("app-title");
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
