//! What the page carries so that the client needs no inline script, no JSON
//! and no second negotiation (`plans/04-leptos-integration.md` §6).
//!
//! Three names, shared by the shell that writes them (the server) and the
//! boot that reads them (the client), so the two cannot drift.

/// The `data-` attribute that marks the preload link of the locale the page
/// was rendered in. The link is the boot data: the client reads the URL from
/// it and the locale from `<html lang>`.
pub const PRELOAD_ATTR: &str = "data-mf2";

/// The `rel` of the optional per-locale catalog links — the in-page tag → URL
/// map, which trades a few bytes in every page for the switch-time round trip
/// (owner question 1).
pub const CATALOG_LINK_REL: &str = "mf2-catalog";

/// The attribute that carries a catalog link's locale tag.
pub const CATALOG_LINK_LOCALE_ATTR: &str = "data-mf2-locale";

/// The island name of [`IslandsGate`](crate::IslandsGate) — the export
/// [`islands_gate!`](crate::islands_gate) defines. The macro spells it as a
/// literal (an attribute argument cannot name a constant); if the two drift,
/// Leptos warns that it cannot find the island's function, and the islands
/// browser check fails on that warning.
pub const ISLANDS_GATE: &str = "mf2_islands_gate";

/// The cookie a switch writes under `static-locale`, and that `mf2-axum`'s
/// `CookieLocale` reads by default: a switch there is this cookie and a
/// reload (D7 strategy C).
pub const LOCALE_COOKIE: &str = "mf2_locale";

/// The query parameter `mf2-axum`'s `QueryParam` reads by default. It ranks
/// above the cookie, so a `static-locale` switch removes it from the address
/// before reloading — otherwise the reload would negotiate the old locale.
pub const LOCALE_QUERY: &str = "lang";

/// Where the catalogs are served from, and what `/i18n/<tag>` redirects
/// within. `mf2-axum` mounts its routes here.
pub const CATALOG_ROUTE: &str = "/i18n/";

/// The URL a catalog published as `file` is served at.
#[must_use]
pub fn catalog_href(file: &str) -> alloc::string::String {
    [CATALOG_ROUTE, file].concat()
}
