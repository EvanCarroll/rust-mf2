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

    // Two real call sites, from a crate that only *depends* on the i18n
    // crate — which is the point of baking the manifest's path into the
    // exported wrapper (D8). Nothing here formats: a client call site is a
    // description until something renders it, and what reaches the artifact
    // is a `MsgId` and the argument, never the id or the argument's name
    // (B6, which `cargo xtask codegen-matrix` greps for).
    // `plain` and `items` on purpose: the edit scenarios add a variable to
    // `greeting` (S6), and a call site that passes the old argument set
    // *should* stop compiling then — which is the macro working, but not
    // something a byte-comparison run can build through.
    let save = std::hint::black_box(mf2_i18n_fixture::tr!("plain"));
    std::hint::black_box(save.id().raw());
    // The B1′ corpus has nothing a function crate could serve, so it has no
    // `items` either (`cargo xtask b12-generated`).
    #[cfg(not(feature = "corpus-plain"))]
    {
        let items = std::hint::black_box(mf2_i18n_fixture::tr!("items", count = 2));
        std::hint::black_box(items.id().raw());
    }
}
