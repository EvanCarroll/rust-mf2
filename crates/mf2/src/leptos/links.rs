//! What the page carries so that the client needs no inline script, no JSON
//! and no second negotiation.
//!
//! Three names, shared by the shell that writes them (the server) and the
//! boot that reads them (the client), so the two cannot drift.

/// The `data-` attribute that marks the preload link of the locale the page
/// was rendered in. The link is the boot data: the client reads the URL from
/// it and the locale from `<html lang>`.
pub const PRELOAD_ATTR: &str = "data-mf2";

/// The `rel` of the optional per-locale catalog links — the in-page tag → URL
/// map, which trades a few bytes in every page for the switch-time round trip
/// (the in-page tag → URL map).
pub const CATALOG_LINK_REL: &str = "mf2-catalog";

/// The attribute that carries a catalog link's locale tag.
pub const CATALOG_LINK_LOCALE_ATTR: &str = "data-mf2-locale";

/// The island name of [`IslandsGate`](crate::leptos::IslandsGate) — the export
/// [`islands_gate!`](crate::leptos::islands_gate) defines. The macro spells it as a
/// literal (an attribute argument cannot name a constant); if the two drift,
/// Leptos warns that it cannot find the island's function, and the islands
/// browser check fails on that warning.
pub const ISLANDS_GATE: &str = "mf2_islands_gate";

pub use crate::links::LOCALE_COOKIE;

/// The cookie that carries the reader's time zone, an IANA name
/// The client writes it when a page was
/// rendered in another zone than the reader's; `mf2-axum` reads it and
/// renders the next page in that zone. Its attributes are
/// [`LOCALE_COOKIE`]'s.
pub const TIME_ZONE_COOKIE: &str = "mf2_tz";

/// The attribute of the preload link ([`PRELOAD_ATTR`]) that states the
/// time zone the page was rendered in, when it was a reader's. Absent, the
/// page was rendered in `Setup`'s zone.
pub const ZONE_ATTR: &str = "data-mf2-zone";

pub use crate::links::LOCALE_QUERY;

/// The `data-` attribute that marks a client-only page's preload of its
/// catalog index: `index.html` writes
/// `<link rel="preload" as="fetch" crossorigin="anonymous" href="i18n/index.json" data-mf2-index>`,
/// and the boot reads the index's URL from it. A catalog's file name in the
/// index is relative to the index.
pub const CSR_INDEX_ATTR: &str = "data-mf2-index";

/// Where a client-only page's boot looks for the index when the page has no
/// [`CSR_INDEX_ATTR`] link — relative to the page, as `mf2 compile --site`
/// lays a site out.
pub const CSR_INDEX_URL: &str = "i18n/index.json";

/// The `localStorage` key a client-only application remembers its locale
/// under: a switch writes it, and the next boot reads it before
/// `navigator.languages`.
pub const LOCALE_STORAGE_KEY: &str = "mf2_locale";

pub use crate::links::CATALOG_ROUTE;

/// The URL a catalog published as `file` is served at.
#[must_use]
pub fn catalog_href(file: &str) -> alloc::string::String {
    [CATALOG_ROUTE, file].concat()
}
