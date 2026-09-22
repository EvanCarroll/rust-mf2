//! A client binary, so that the edit scenarios have a real `.wasm` to
//! compare (`cargo xtask scenarios`).
//!
//! It does nothing but keep the generated items alive: what matters is that
//! the artifact carries whatever a client build of the generated module
//! carries, and that a translation-only edit leaves it byte for byte the
//! same.

fn main() {
    // A real client carries the manifest hash: it is what says a catalog it
    // fetched belongs to this wasm (F6). Reading it here is what makes the
    // scenarios' "the wasm did not change" mean something — a change to the
    // ids or the slots *does* reach the client, and a translation does not.
    let hash = std::hint::black_box(mf2_i18n_fixture::MANIFEST_HASH);
    let locales = std::hint::black_box(mf2_i18n_fixture::LOCALES).len();
    let known = usize::from(std::hint::black_box(mf2_i18n_fixture::has_locale("pl")));
    let functions = std::hint::black_box(mf2_i18n_fixture::registry());
    std::hint::black_box((hash, locales, known, functions.get("integer").is_some()));
}
