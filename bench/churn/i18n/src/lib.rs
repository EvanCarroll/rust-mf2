//! The whole crate: what `mf2-build` generated, plus the one `Setup` value
//! the harness installs.

mf2::include_generated!();

/// Everything `mf2::leptos::install` needs, from the generated module.
#[must_use]
pub fn setup() -> mf2::leptos::Setup {
    mf2::leptos::Setup::new(
        registry(),
        &host::HOST,
        MANIFEST_HASH,
        SOURCE_LOCALE,
        LOCALES,
    )
}
