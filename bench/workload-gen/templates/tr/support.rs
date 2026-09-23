//! Support module of the `tr` template: the one place a description is
//! formatted, so that a call site is the description and nothing else.
//!
//! The catalog is installed at boot from `<html data-catalog>` — opaque to
//! the optimiser, as a fetched catalog is — and every site goes through the
//! same two functions, which are `#[inline(never)]` so that what a site
//! costs is a call and its arguments.

use std::cell::RefCell;

use mf2::{Catalog, FormatContext, Formatter};

thread_local! {
    static CATALOG: RefCell<Option<Catalog>> = const { RefCell::new(None) };
}

static CX: FormatContext = FormatContext::new(&HOST);

#[cfg(feature = "ssr")]
use mf2::host_std::HOST;
#[cfg(not(feature = "ssr"))]
use mf2::host_web::HOST;

/// What a call site formats to.
pub trait Fmt {
    /// Its text, against `f`.
    fn text(&self, f: &Formatter<'_>) -> String;
}

impl Fmt for mf2::Tr {
    fn text(&self, f: &Formatter<'_>) -> String {
        self.format(f)
    }
}

impl Fmt for mf2::TrArgs {
    fn text(&self, f: &Formatter<'_>) -> String {
        self.format(f)
    }
}

/// Formats a description with the active catalog. One function for every
/// call site of its shape — two instances in the whole application, not one
/// per site.
#[inline(never)]
pub fn s<T: Fmt>(description: T) -> String {
    CATALOG.with(|c| match &*c.borrow() {
        Some(catalog) => {
            let f = Formatter::new(catalog, workload_i18n::registry(), &CX);
            description.text(&f)
        }
        None => String::new(),
    })
}

/// Installs a catalog.
pub fn install(bytes: Vec<u8>) {
    if let Ok(catalog) = Catalog::new(bytes, workload_i18n::MANIFEST_HASH) {
        CATALOG.with(|c| *c.borrow_mut() = Some(catalog));
    }
}

/// Client boot: the catalog arrives base64 in `<html data-catalog>`, so that
/// it is opaque to the optimiser exactly as a fetched one would be.
pub fn boot() {
    #[cfg(feature = "hydrate")]
    {
        let data = leptos::prelude::document()
            .document_element()
            .and_then(|e| e.get_attribute("data-catalog"));
        if let Some(data) = data {
            let mut bytes = Vec::with_capacity(data.len() / 2);
            let digits = data.as_bytes();
            let value = |b: u8| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                _ => None,
            };
            for pair in digits.chunks(2) {
                match (pair.first().copied().and_then(value), pair.get(1).copied().and_then(value)) {
                    (Some(hi), Some(lo)) => bytes.push(hi << 4 | lo),
                    _ => return,
                }
            }
            install(bytes);
        }
    }
}
