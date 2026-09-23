//! The whole crate: what `mf2-build` generated, plus the one `Setup` value
//! the application installs.

mf2::include_generated!();

/// Everything `leptos_mf2::install` needs, from the generated module.
#[must_use]
pub fn setup() -> mf2::leptos_mf2::Setup {
    mf2::leptos_mf2::Setup::new(
        registry(),
        &host::HOST,
        MANIFEST_HASH,
        SOURCE_LOCALE,
        LOCALES,
    )
}
