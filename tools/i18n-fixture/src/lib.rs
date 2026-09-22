//! The generated module, and nothing else: this crate exists so that what
//! `mf2-build` writes is compiled the way an application compiles it.
//!
//! An application writes `mf2::include_generated!();` here instead; that
//! macro arrives with the `tr!` proc-macro in Phase 5b.

include!(concat!(env!("OUT_DIR"), "/mf2_generated.rs"));

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
