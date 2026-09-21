//! P0.9 server (the probe's own, replacing workload-gen's `src/main.rs`):
//! installs the embedded catalogs, renders the app with a shell that carries
//! `<meta name="mf2-manifest">`, and serves `/i18n/<file>`, `/manifest`,
//! `/second` for the checks.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use axum::extract::Path;
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use workload_app_tr::app::App;

    p09_i18n::install();
    let conf = get_configuration(None).expect("leptos configuration");
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    async fn catalog(Path(file): Path<String>) -> axum::response::Response {
        match p09_i18n::__mf2::catalog_bytes(&file) {
            Some(b) => (
                [
                    (header::CONTENT_TYPE, "application/octet-stream"),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                b,
            )
                .into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }

    let app = Router::new()
        .route("/i18n/{file}", get(catalog))
        .route(
            "/manifest",
            get(|| async { std::format!("{:016x}\n", p09_i18n::MANIFEST_HASH) }),
        )
        .route(
            "/catalogs",
            get(|| async {
                p09_i18n::CATALOGS
                    .iter()
                    .map(|c| std::format!("{}\t{}\n", c.tag, c.file))
                    .collect::<String>()
            }),
        )
        .route("/second", get(|| async { p09_second::report(2, "Ada") }))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("serve");
}

/// The HTML shell (as workload-gen's, plus the manifest meta and `<html lang dir>`).
#[cfg(feature = "ssr")]
fn shell(options: leptos::prelude::LeptosOptions) -> impl leptos::prelude::IntoView {
    use leptos::prelude::*;
    use workload_app_tr::app::App;
    let lang = p09_i18n::__mf2::current_locale().unwrap_or(p09_i18n::SOURCE_LOCALE);
    let dir = if p09_i18n::LOCALES.iter().any(|l| l.tag == lang && l.rtl) {
        "rtl"
    } else {
        "ltr"
    };
    let manifest = std::format!("{:016x}", p09_i18n::MANIFEST_HASH);
    view! {
        <!DOCTYPE html>
        <html lang=lang dir=dir>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="mf2-manifest" content=manifest/>
                <title>"Reference workload (P0.9)"</title>
                <link rel="stylesheet" href="/pkg/workload-app-tr.css"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[cfg(not(feature = "ssr"))]
fn main() {}
