//! Support module of the `tr-view` template: the boot, and nothing else.
//!
//! Where the `tr` template needs a formatting helper because a description
//! could not render itself, this template has none: the library does the
//! rendering, so a call site is the description and the support module is
//! the install.

/// Client boot: install the generated setup, then the catalog, which arrives
/// hex in `<html data-catalog>` so that it is opaque to the optimiser
/// exactly as a fetched one would be.
pub fn boot() {
    #[cfg(feature = "hydrate")]
    {
        mf2::leptos::install(mf2::leptos::Setup::new(
            workload_i18n::registry(),
            &workload_i18n::host::HOST,
            workload_i18n::MANIFEST_HASH,
            workload_i18n::SOURCE_LOCALE,
            workload_i18n::LOCALES,
        ));
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
                match (
                    pair.first().copied().and_then(value),
                    pair.get(1).copied().and_then(value),
                ) {
                    (Some(hi), Some(lo)) => bytes.push(hi << 4 | lo),
                    _ => return,
                }
            }
            if let Ok(catalog) = mf2::leptos::read_catalog(bytes) {
                mf2::leptos::set_active(catalog);
            }
        }
    }
}
