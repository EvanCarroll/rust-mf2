//! Which of the corpus's locales a tag asks for, and where the active one
//! came from.

use mf2_catalog::Dir;

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

/// Languages written in more than one script, where a tag with another
/// region (`zh-TW` for `zh-CN`) may mean another script: for these, only an
/// exact or truncated match counts, never "same language, other region".
const MULTI_SCRIPT: [&str; 14] = [
    "az", "bs", "ff", "ha", "ks", "mn", "pa", "sd", "shi", "sr", "uz", "vai", "yue", "zh",
];

/// The first candidate, in order, that one of `locales` matches.
pub(crate) fn negotiate<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    locales: &[(&'static str, Dir)],
) -> Option<&'static str> {
    candidates
        .into_iter()
        .find_map(|candidate| match_locale(candidate, locales))
}

/// The supported locale `candidate` asks for: case-insensitive, `_` read as
/// `-`, a POSIX `.charset` or `@modifier` ignored; then with subtags dropped
/// from the end (`fr-CA-x-private` → `fr-CA` → `fr`); then any locale of
/// the same language, unless the language is written in more than one
/// script or either tag names a script.
pub(crate) fn match_locale(
    candidate: &str,
    locales: &[(&'static str, Dir)],
) -> Option<&'static str> {
    let bare = candidate
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-");
    if bare.is_empty() || bare == "*" || bare == "C" || bare == "POSIX" {
        return None;
    }
    let mut range = bare.as_str();
    loop {
        if let Some((tag, _)) = locales
            .iter()
            .find(|(tag, _)| tag.eq_ignore_ascii_case(range))
        {
            return Some(tag);
        }
        match range.rfind('-') {
            Some(at) => range = range.get(..at)?,
            None => break,
        }
    }
    let language = bare.split('-').next()?;
    if MULTI_SCRIPT
        .iter()
        .any(|l| l.eq_ignore_ascii_case(language))
        || has_script(&bare)
    {
        return None;
    }
    locales
        .iter()
        .find(|(tag, _)| {
            !has_script(tag)
                && tag
                    .split('-')
                    .next()
                    .is_some_and(|part| part.eq_ignore_ascii_case(language))
        })
        .map(|(tag, _)| *tag)
}

/// Whether a tag has a script subtag (four letters, after the language).
fn has_script(tag: &str) -> bool {
    tag.split('-')
        .skip(1)
        .take_while(|s| s.len() != 1)
        .any(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_alphabetic()))
}

#[cfg(test)]
mod tests {
    use super::{match_locale, negotiate};
    use mf2_catalog::Dir;

    static LOCALES: &[(&str, Dir)] = &[
        ("en", Dir::Ltr),
        ("fr-FR", Dir::Ltr),
        ("fr-CA", Dir::Ltr),
        ("ar", Dir::Rtl),
        ("zh-CN", Dir::Ltr),
        ("sr-Latn", Dir::Ltr),
    ];

    #[test]
    fn matches_case_insensitively_and_reads_posix_names() {
        assert_eq!(match_locale("FR_ca", LOCALES), Some("fr-CA"));
        assert_eq!(match_locale("fr_CA.UTF-8", LOCALES), Some("fr-CA"));
        assert_eq!(match_locale("ar_EG.UTF-8@latin", LOCALES), Some("ar"));
    }

    #[test]
    fn truncates_and_then_matches_by_language() {
        assert_eq!(match_locale("fr-CA-x-private", LOCALES), Some("fr-CA"));
        assert_eq!(match_locale("en-GB", LOCALES), Some("en"));
        assert_eq!(match_locale("fr-BE", LOCALES), Some("fr-FR"));
    }

    #[test]
    fn never_crosses_scripts() {
        assert_eq!(match_locale("zh-TW", LOCALES), None);
        assert_eq!(match_locale("zh-CN", LOCALES), Some("zh-CN"));
        assert_eq!(match_locale("sr-Cyrl", LOCALES), None);
        assert_eq!(match_locale("sr-Latn-RS", LOCALES), Some("sr-Latn"));
    }

    #[test]
    fn rejects_empty_wildcard_and_posix_default() {
        for tag in ["", "*", "C", "POSIX", "C.UTF-8"] {
            assert_eq!(match_locale(tag, LOCALES), None, "{tag}");
        }
    }

    #[test]
    fn takes_the_first_candidate_that_matches() {
        assert_eq!(negotiate(["de-DE", "fr-CA", "en"], LOCALES), Some("fr-CA"));
        assert_eq!(negotiate(["de", "it"], LOCALES), None);
    }
}
