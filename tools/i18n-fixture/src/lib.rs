//! The generated module, and nothing else: this crate exists so that what
//! `mf2-build` writes is compiled the way an application compiles it — and,
//! since Phase 5b, so that the `tr!` it exports is expanded the way an
//! application expands it.

mf2::include_generated!();

/// The fixture's own check that the generated items are usable.
#[cfg(test)]
mod tests {
    // A module of the crate that includes the module names `tr!` by path,
    // as every other crate does (`mf2_i18n_fixture::tr!`, `src/main.rs`).
    #[allow(unused_imports)]
    use crate::tr;

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

    /// The generated `Locale`: a variant per locale, and a tag parsed
    /// through the one matcher, whose refusal lists the languages.
    #[test]
    fn the_generated_locale_parses_through_the_matcher() {
        use super::Locale;

        assert_eq!(Locale::ALL, [Locale::Ar, Locale::En, Locale::Pl]);
        assert_eq!(Locale::SOURCE, Locale::En);
        assert_eq!(Locale::Ar.dir(), mf2::Dir::Rtl);
        assert_eq!(Locale::Pl.to_string(), "pl");
        assert_eq!("pl_PL.UTF-8".parse::<Locale>(), Ok(Locale::Pl));
        assert_eq!(Locale::best_match(["de", "pl"]), Some(Locale::Pl));
        let refused = "de".parse::<Locale>().map(Locale::tag);
        assert_eq!(
            refused.map_err(|e| e.to_string()),
            Err("no language of this application matches; it has ar, en, pl".to_owned())
        );
    }

    /// With `mf2/clap`, `--lang` parses through the matcher and `--help`
    /// lists the tags.
    #[test]
    fn clap_parses_a_lang_through_the_matcher() {
        use super::Locale;

        let command = clap::Command::new("app").arg(
            clap::Arg::new("lang")
                .long("lang")
                .value_parser(clap::value_parser!(Locale)),
        );
        let matches = command
            .clone()
            .try_get_matches_from(["app", "--lang", "pl_PL.UTF-8"])
            .expect("pl_PL.UTF-8 is Polish");
        assert_eq!(matches.get_one::<Locale>("lang"), Some(&Locale::Pl));
        let refused = command
            .clone()
            .try_get_matches_from(["app", "--lang", "de"])
            .map(|_| ())
            .map_err(|e| e.to_string());
        assert!(
            refused
                .as_ref()
                .is_err_and(|e| e.contains("it has ar, en, pl")),
            "{refused:?}"
        );
        let help = command.clone().render_help().to_string();
        assert!(help.contains("[possible values: ar, en, pl]"), "{help}");
    }

    /// A native module's typed forms: a named language without `install()`.
    #[cfg(feature = "native")]
    #[test]
    fn the_typed_forms_format_in_a_named_language() {
        use super::{Locale, current_locale, with_locale};

        let save = tr!("plain");
        assert_eq!(Locale::En.format(&save), "Save");
        assert_eq!(
            with_locale(Locale::Pl, || (current_locale(), save.to_string())),
            (Locale::Pl, Locale::Pl.format(&save))
        );
    }

    /// What `tr!` expands to, formatted from the embedded catalog: the whole
    /// chain — `mf2-build`'s manifest, the generated wrapper, the
    /// proc-macro's checks, the facade's call-site core — in the shape an
    /// application uses it.
    #[cfg(all(
        feature = "ssr",
        not(feature = "split-catalogs"),
        not(feature = "native")
    ))]
    #[test]
    fn a_call_site_formats_from_the_embedded_catalog() {
        static CX: mf2::FormatContext = mf2::FormatContext::new(&mf2::host_std::HOST);
        // A description is `const`-constructible, so a call site can sit in
        // a static table and be formatted when it is shown.
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

    /// Each argument through the step of the macro's dispatch its type allows:
    /// `IntoArg` (a `u64` past
    /// `i64`, a `bool`, a path, a variable's `&str`), 1.x's `From` (`&String`),
    /// and any other type's `Display` text.
    #[cfg(all(
        feature = "ssr",
        not(feature = "split-catalogs"),
        not(feature = "native")
    ))]
    #[test]
    fn a_call_site_converts_each_argument_by_its_type() {
        static CX: mf2::FormatContext = mf2::FormatContext::new(&mf2::host_std::HOST);
        let bytes = super::catalog("en").expect("en is embedded");
        let catalog = mf2::Catalog::new(bytes.to_vec(), super::MANIFEST_HASH)
            .expect("the embedded catalog loads");
        let f = mf2::Formatter::new(&catalog, super::registry(), &CX);
        let hello = |name: &str| format!("Hello, \u{2068}{name}\u{2069}!");

        assert_eq!(
            tr!("received", count = u64::MAX).format(&f),
            "You have 18446744073709551615 of them"
        );
        assert_eq!(tr!("items", count = 1_u64).format(&f), "1 item");
        assert_eq!(tr!("items", count = &2_u8).format(&f), "2 items");
        let who = String::from("Ada");
        assert_eq!(
            tr!("greeting", name = who.as_str()).format(&f),
            hello("Ada")
        );
        assert_eq!(tr!("greeting", name = &who).format(&f), hello("Ada"));
        assert_eq!(tr!("greeting", name = true).format(&f), hello("true"));
        let dir = std::path::Path::new("locales");
        assert_eq!(tr!("greeting", name = dir).format(&f), hello("locales"));
        let error = std::io::Error::other("no such file");
        assert_eq!(
            tr!("greeting", name = error).format(&f),
            hello("no such file")
        );
        assert_eq!(
            tr!("greeting", name = std::net::Ipv4Addr::LOCALHOST).format(&f),
            hello("127.0.0.1")
        );
    }

    /// The rich expansion: handlers for every markup name of the message,
    /// found again by the name the catalog gives at render time — which is
    /// what a renderer does, without the name ever being in the wasm.
    #[cfg(all(
        feature = "ssr",
        not(feature = "split-catalogs"),
        not(feature = "native")
    ))]
    #[test]
    fn a_rich_call_site_carries_a_handler_per_markup_name() {
        struct Element(&'static str);

        impl mf2::MarkupHandler for Element {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        // A handler the caller wrote goes through `Handler`, in every build:
        // whether something else in the workspace turned the `leptos` feature
        // on must not change what a call site compiles to (04 §2.1).
        let rich = tr!(
            "help",
            kbd = mf2::Handler(Element("kbd")),
            b = mf2::Handler(Element("b"))
        );
        for name in ["kbd", "b"] {
            let handler = rich.handler(name).expect("every markup name is handled");
            let handler = handler
                .as_any()
                .downcast_ref::<Element>()
                .expect("the renderer knows its own type");
            assert_eq!(handler.0, name);
        }
        assert!(rich.handler("i").is_none());
    }

    /// Under `ssr` the catalogs are embedded, content-hashed and loadable —
    /// unless they were emitted apart, in which case this crate names none.
    #[cfg(all(
        feature = "ssr",
        not(feature = "split-catalogs"),
        not(feature = "native")
    ))]
    #[test]
    fn the_server_carries_every_catalog() {
        for (tag, name, bytes, _server) in super::CATALOGS {
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
