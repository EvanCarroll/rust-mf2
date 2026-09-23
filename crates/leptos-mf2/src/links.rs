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

/// Where the catalogs are served from, and what `/i18n/<tag>` redirects
/// within. `mf2-axum` mounts its routes here.
pub const CATALOG_ROUTE: &str = "/i18n/";

/// The URL a catalog published as `file` is served at.
#[must_use]
pub fn catalog_href(file: &str) -> alloc::string::String {
    [CATALOG_ROUTE, file].concat()
}
