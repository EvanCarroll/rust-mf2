//! The names the server and the browser's client share, so that the two
//! cannot drift: `mf2::axum` serves and negotiates with them, and the Leptos
//! layer's client writes and reads them (`mf2::leptos::links`, which
//! re-exports them).

#![allow(
    unreachable_pub,
    reason = "public as `mf2::leptos::links`'s with a Leptos mode, crate-internal without"
)]

/// The cookie a switch writes under `static-locale`, and that `mf2::axum`'s
/// `CookieLocale` reads by default: a switch there is this cookie and a
/// reload (D7 strategy C).
pub const LOCALE_COOKIE: &str = "mf2_locale";

/// The query parameter `mf2::axum`'s `QueryParam` reads by default. It ranks
/// above the cookie, so a `static-locale` switch removes it from the address
/// before reloading — otherwise the reload would negotiate the old locale.
pub const LOCALE_QUERY: &str = "lang";

/// Where the catalogs are served from, and what `/i18n/<tag>` redirects
/// within. `mf2::axum` mounts its routes here.
pub const CATALOG_ROUTE: &str = "/i18n/";
