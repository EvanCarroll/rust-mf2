//! The generated module, and nothing else: this crate exists so that what
//! `mf2-build` writes is compiled the way an application compiles it — and,
//! since Phase 5b, so that the `tr!` it exports is expanded the way an
//! application expands it.

mf2::include_generated!();

/// The fixture's own check that the generated items are usable.
#[cfg(test)]
mod tests {
    #[test]
    fn the_generated_module_says_what_it_should() {
        assert_eq!(super::SOURCE_LOCALE, "en");
        assert_ne!(super::MANIFEST_HASH, 0);
        assert!(super::has_locale("pl"));
        assert!(!super::has_locale("de"));
        let dirs: Vec<_> = super::LOCALES.iter().map(|(t, d)| (*t, *d)).collect();
        assert_eq!(
            dirs,
            [
                ("ar", mf2::Dir::Rtl),
                ("en", mf2::Dir::Ltr),
                ("pl", mf2::Dir::Ltr)
            ]
        );
        // The registry is closed over what the corpus uses.
        assert!(super::registry().get("integer").is_some());
        assert!(super::registry().get("percent").is_none());
    }

    /// What `tr!` expands to, formatted from the embedded catalog: the whole
    /// chain — `mf2-build`'s manifest, the generated wrapper, the
    /// proc-macro's checks, the facade's call-site core — in the shape an
    /// application uses it.
    #[cfg(all(feature = "ssr", not(feature = "split-catalogs")))]
    #[test]
    fn a_call_site_formats_from_the_embedded_catalog() {
        static CX: mf2::FormatContext = mf2::FormatContext::new(&mf2::host_std::HOST);
        // A description is `const`-constructible, so a call site can sit in
        // a static table and be formatted when it is shown.
        //
        // Inside the i18n crate itself the macro is called unqualified: a
        // `macro_export` macro that is itself macro-expanded — and this one
        // arrives through `include_generated!` — cannot be reached by an
        // absolute path in its own crate (rustc #52234). Every other crate
        // writes `mf2_i18n_fixture::tr!`, which `src/main.rs` does.
        const SAVE: mf2::Tr = tr!("plain");

        let bytes = super::catalog("en").expect("en is embedded");
        let catalog = mf2::Catalog::new(bytes.to_vec(), super::MANIFEST_HASH)
            .expect("the embedded catalog loads");
        let f = mf2::Formatter::new(&catalog, super::registry(), &CX);

        assert_eq!(SAVE.format(&f), "Save");
        assert_eq!(tr!("account.name").format(&f), "Name");
        // A string argument is isolated by the Default Bidi Strategy; a
        // number carries its own direction and is not.
        assert_eq!(
            tr!("greeting", name = "Ada").format(&f),
            "Hello, \u{2068}Ada\u{2069}!"
        );
        assert_eq!(tr!("items", count = 1).format(&f), "1 item");
        assert_eq!(tr!("items", count = 3).format(&f), "3 items");
        assert_eq!(
            tr!("received", count = 12).format(&f),
            "You have 12 of them"
        );
        // Markup with no handler formats to its text, which is what the
        // suite's markup tests assert at L5.
        assert_eq!(tr!("help").format(&f), "Press Esc to close");
    }

    /// Under `ssr` the catalogs are embedded, content-hashed and loadable —
    /// unless they were emitted apart, in which case this crate names none.
    #[cfg(all(feature = "ssr", not(feature = "split-catalogs")))]
    #[test]
    fn the_server_carries_every_catalog() {
        for (tag, name, bytes) in super::CATALOGS {
            assert!(name.starts_with(tag), "{name} is not {tag}'s");
            assert!(
                std::path::Path::new(name)
                    .extension()
                    .is_some_and(|e| e == "mf2b"),
                "{name}"
            );
            let catalog = mf2::Catalog::new(bytes.to_vec(), super::MANIFEST_HASH)
                .expect("the embedded catalog loads");
            assert_eq!(catalog.locale(), *tag);
        }
        assert!(super::catalog("en").is_some());
        assert!(super::catalog("de").is_none());
    }
}
