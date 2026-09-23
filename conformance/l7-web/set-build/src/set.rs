// What a conformance L7 page needs from one set, beyond its generated module
// and its call sites (`plans/15-phase-7-work-order.md` A4). Included by each
// of the four set crates; see `mf2-l7-set-build`.

/// Everything `leptos_mf2::install` needs, from the generated module.
#[must_use]
pub fn setup() -> ::leptos_mf2::Setup {
    ::leptos_mf2::Setup::new(
        registry(),
        &host::HOST,
        MANIFEST_HASH,
        SOURCE_LOCALE,
        LOCALES,
    )
}

/// How many call sites the page holds.
#[must_use]
pub fn len() -> usize {
    CASES.len()
}

/// The message id of call site `index`.
#[must_use]
pub fn id(index: usize) -> Option<&'static str> {
    CASES.get(index).map(|(id, _)| *id)
}

/// Call site `index`, as a view — nothing for an index the set has not.
#[must_use]
pub fn view(index: usize) -> ::leptos::prelude::AnyView {
    use ::leptos::prelude::IntoAny;
    match CASES.get(index).map(|(_, build)| build()) {
        Some(Case::Tr(t)) => t.into_any(),
        Some(Case::Args(t)) => t.into_any(),
        Some(Case::Dyn(t)) => t.into_any(),
        None => ().into_any(),
    }
}
