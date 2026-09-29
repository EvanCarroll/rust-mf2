//! Where the active language came from, and which of the corpus's languages
//! a tag or the system's list asks for: the one matcher
//! (plans/19-native-and-terminal.md §9), over the corpus's cut of CLDR's
//! table, which gives its locales the answers the whole table gives.

use mf2_catalog::Dir;

use crate::LanguageMatching;

/// Where the app-wide language came from, as
/// [`locale_source`](super::locale_source) says.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum LocaleSource {
    /// The application chose it with [`set_locale`](super::set_locale).
    Explicit,
    /// One of the system's preferred locales matched.
    System,
    /// Nothing matched, so the corpus's source locale is used.
    Source,
}

/// The locale of `locales` that best serves a reader of `desired`, their
/// languages in order of preference: what CLDR's language-matching data
/// (`matching`) finds closest, if close enough (`fr-CA` finds `fr`,
/// `zh-Hant-TW` finds `zh-TW`, `sr-Latn` finds `sr`; `zh-TW` does not find
/// `zh-CN`). Nothing is copied: a tag is read where it lies.
pub(crate) fn best<'a>(
    matching: &LanguageMatching,
    desired: impl IntoIterator<Item = &'a str>,
    locales: &[(&'static str, Dir)],
) -> Option<&'static str> {
    let at = matching.best_match(desired, locales)?;
    locales.get(at).map(|(tag, _)| *tag)
}
