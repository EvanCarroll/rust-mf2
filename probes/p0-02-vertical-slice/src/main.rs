//! Axum server for the P0.2 probe. All four leptos_axum context entry points
//! receive the same `additional_context` (plans/04 §5): route-list generation,
//! `leptos_routes_with_context` (which also registers server functions through
//! `handle_server_fns_with_context`), and `file_and_error_handler_with_context`.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), p002::error::ServerError> {
    use axum::{Router, extract::Path, routing::get};
    use leptos::prelude::*;
    use leptos_axum::{
        LeptosRoutes, file_and_error_handler_with_context,
        generate_route_list_with_exclusions_and_ssg_and_context,
    };
    use p002::{
        app::{App, shell},
        i18n_server::{Catalogs, provide_i18n, serve_catalog},
    };
    use std::sync::{Arc, atomic::Ordering};

    let conf = get_configuration(None)?;
    let options = conf.leptos_options;
    let addr = options.site_addr;

    let catalogs = Arc::new(Catalogs::build());
    tr::server::install_default(catalogs.default_state());
    let ctx = {
        let catalogs = Arc::clone(&catalogs);
        move || provide_i18n(&catalogs)
    };

    let (routes, _static_routes) =
        generate_route_list_with_exclusions_and_ssg_and_context(App, None, ctx.clone());

    let app = Router::new()
        .route(
            "/i18n/{file}",
            get({
                let catalogs = Arc::clone(&catalogs);
                move |file: Path<String>| async move { serve_catalog(&catalogs, file) }
            }),
        )
        // Probe-only counters: render-time lookups with / without context.
        .route(
            "/__probe/ctx",
            get(|| async {
                format!(
                    "hits={} misses={}",
                    tr::server::CONTEXT_HITS.load(Ordering::Relaxed),
                    tr::server::CONTEXT_MISSES.load(Ordering::Relaxed)
                )
            }),
        )
        // Probe-only: a lookup with no reactive owner at all must fall back to
        // the default locale instead of panicking.
        .route("/__probe/no-ctx", get(|| async { p002::msg::WELCOME.to_string() }))
        .leptos_routes_with_context(&options, routes, ctx.clone(), {
            let options = options.clone();
            move || shell(options.clone())
        })
        .fallback(file_and_error_handler_with_context(ctx, shell))
        .with_state(options);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("p002 listening on http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
