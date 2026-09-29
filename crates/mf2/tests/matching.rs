//! The one locale matcher (plans/19-native-and-terminal.md §9;
//! plans/18-phase-10-work-order.md, C3), over CLDR's whole table as a server
//! and a native application carry it. Each case says why it holds:
//!
//! * the owner's answers (18, questions 11 and 15) and C3's table;
//! * the rules 19 §9 states: the threshold, the demotion (unbounded), the
//!   ties, a macroregion in a match variable, a reader's `und`, a tag the
//!   data cannot fill, POSIX names;
//! * the worked examples of UTS #35 Part 1 (§4.4, §4.4.1, §4.2's table),
//!   paraphrased, each with its section — all but the opening illustration,
//!   which the steps do not reproduce (below);
//! * every case the two 1.x matchers' tests passed (`mf2-native`'s
//!   `locale.rs`, `mf2-axum`'s `negotiate.rs`), except those questions 11
//!   and 15 change, each listed with what 1.x gave.
//!
//! The figures are the matching distances of C3's text half ("the evidence
//! re-run", its tables), which these reproduce.

use mf2::{Dir, LanguageMatching};

fn cldr() -> &'static LanguageMatching {
    LanguageMatching::cldr()
}

/// The locale of `supported` that best serves a reader of `desired`.
fn best(desired: &[&str], supported: &[&'static str]) -> Option<&'static str> {
    let table: Vec<(&str, Dir)> = supported.iter().map(|t| (*t, Dir::Ltr)).collect();
    let at = cldr().best_match(desired.iter().copied(), &table)?;
    supported.get(at).copied()
}

/// The matching distance of two tags.
fn distance(desired: &str, supported: &str) -> u32 {
    cldr()
        .distance_of(desired, supported)
        .unwrap_or_else(|| panic!("{desired} and {supported} are tags"))
}

// ------------------------------------------------ C3's table, questions 11, 15

#[test]
fn traditional_chinese_finds_taiwan_through_likely_subtags() {
    // `zh-TW` and `zh-Hant` both fill to zh-Hant-TW: an exact match, where
    // 1.x truncated `zh-Hant-TW` to `zh` before `zh-TW` was tried.
    assert_eq!(distance("zh-Hant-TW", "zh-TW"), 0);
    assert_eq!(best(&["zh-Hant-TW"], &["zh", "zh-TW"]), Some("zh-TW"));
    assert_eq!(best(&["zh-Hant"], &["zh", "zh-TW"]), Some("zh-TW"));
    // Hong Kong is served Taiwan's Traditional when the application has no
    // `zh-HK`: 5 (`zh-Hant-*`; HK is in `$cnsar`, TW is not).
    assert_eq!(distance("zh-HK", "zh-TW"), 5);
    assert_eq!(best(&["zh-HK"], &["zh", "zh-TW"]), Some("zh-TW"));
    // …and Macao's before Taiwan's: 4 (`zh-Hant-$cnsar`).
    assert_eq!(best(&["zh-HK"], &["zh-TW", "zh-MO"]), Some("zh-MO"));
}

#[test]
fn traditional_and_simplified_are_not_served_for_each_other() {
    // Question 15: CLDR has no rule between the two scripts, so the pair
    // takes the default script distance, 50, and the regions add 4.
    assert_eq!(distance("zh-Hant", "zh-Hans"), 54);
    assert_eq!(distance("zh-Hans", "zh-Hant"), 54);
    assert_eq!(best(&["zh-Hant"], &["zh-Hans"]), None);
    assert_eq!(best(&["zh-Hans"], &["zh-Hant"]), None);
    // A region implies the script: `zh-TW` is Traditional, `zh-CN` and a
    // bare `zh` Simplified.
    assert_eq!(best(&["zh-TW"], &["zh-CN"]), None);
    assert_eq!(best(&["zh"], &["zh-TW"]), None);
    assert_eq!(best(&["zh-TW"], &["zh"]), None);
    // The reader's next listed language, else the source (`None`).
    assert_eq!(best(&["zh-Hant", "en"], &["zh", "en"]), Some("en"));
    assert_eq!(best(&["zh-Hant"], &["zh", "en"]), None);
    // A list that also names plain `zh` gets Simplified through it: 5.
    assert_eq!(best(&["zh-TW", "zh"], &["zh", "en"]), Some("zh"));
}

#[test]
fn serbian_scripts_are_served_for_each_other_and_punjabi_scripts_are_not() {
    // Question 11: `sr-Latn` ↔ `sr-Cyrl` is 5 both ways.
    assert_eq!(distance("sr-Latn", "sr-Cyrl"), 5);
    assert_eq!(distance("sr-Cyrl", "sr-Latn"), 5);
    assert_eq!(best(&["sr-Latn"], &["sr", "en"]), Some("sr"));
    assert_eq!(best(&["sr"], &["sr-Latn", "en"]), Some("sr-Latn"));
    // Montenegro's Serbian is Latin: 5 for the script, 4 for the region.
    assert_eq!(distance("sr-ME", "sr"), 9);
    // Punjabi's Shahmukhi and Gurmukhi have no rule: 50 and 4.
    assert_eq!(distance("pa-Arab", "pa-Guru"), 54);
    assert_eq!(distance("pa-Guru", "pa-Arab"), 54);
    assert_eq!(best(&["pa-Arab"], &["pa-Guru"]), None);
    assert_eq!(best(&["pa-Arab", "en"], &["pa-Guru", "en"]), Some("en"));
}

#[test]
fn spanish_regions_fall_back_as_the_owner_described() {
    // Question 11: a Mexican reader gets `es` (5)…
    assert_eq!(distance("es-MX", "es"), 5);
    assert_eq!(best(&["es-MX"], &["es", "en"]), Some("es"));
    // …and `es-419` before it when the application has it (4: both in the
    // Americas); a reader of `es` gets `es-MX` when that is all there is.
    assert_eq!(distance("es-MX", "es-419"), 4);
    assert_eq!(best(&["es-MX"], &["es", "es-419"]), Some("es-419"));
    assert_eq!(distance("es", "es-MX"), 5);
    assert_eq!(best(&["es"], &["es-MX", "en"]), Some("es-MX"));
    // And the same served exactly when the application has it.
    assert_eq!(best(&["es-MX"], &["es", "es-MX"]), Some("es-MX"));
}

#[test]
fn other_languages_where_cldr_accepts_them() {
    // One-way rules: a reader of Catalan accepts Spanish (20), not the
    // reverse (80); Cantonese accepts Traditional Chinese (10 and 4).
    assert_eq!(distance("ca", "es"), 20);
    assert_eq!(best(&["ca"], &["es"]), Some("es"));
    assert_eq!(best(&["es"], &["ca"]), None);
    assert_eq!(distance("yue", "zh-TW"), 14);
    assert_eq!(best(&["yue"], &["zh"]), None);
    // A reader of Punjabi in Gurmukhi accepts English (30 + 10 + 4), in
    // Shahmukhi not (30 + 50 + 4).
    assert_eq!(distance("pa", "en"), 44);
    assert_eq!(distance("pa-PK", "en"), 84);
    // Norwegian Bokmål and Norwegian: 1.
    assert_eq!(best(&["nb"], &["no"]), Some("no"));
}

// ------------------------------------------------------ the rules of 19 §9

#[test]
fn the_threshold_is_below_fifty() {
    // A match only when the weighted distance is below 50: 44 is served,
    // 54 and 84 are not.
    assert_eq!(best(&["pa"], &["en"]), Some("en"));
    assert_eq!(best(&["zh-Hant"], &["zh-Hans"]), None);
    assert_eq!(best(&["pa-PK"], &["en"]), None);
}

#[test]
fn demotion_is_unbounded_so_an_eleventh_exact_match_is_refused() {
    // 19 §9 states the demotion and does not bound it: 5 for each later
    // entry, counted against the threshold. An exact match tenth in the
    // list weighs 45 and is served; eleventh, 50, and it is not.
    let others = ["fr", "de", "it", "es", "pt", "nl", "sv", "da", "nb", "pl"];
    let supported = ["zh-Hant", "en"];
    let mut list: Vec<&str> = others.iter().take(9).copied().collect();
    list.push("zh-Hant");
    assert_eq!(list.len(), 10);
    assert_eq!(best(&list, &supported), Some("zh-Hant"));
    let mut list: Vec<&str> = others.to_vec();
    list.push("zh-Hant");
    assert_eq!(list.len(), 11);
    assert_eq!(best(&list, &supported), None);
    // What takes no place in the list (`*`, a POSIX `C`) is not counted.
    let mut list: Vec<&str> = vec!["*", "C"];
    list.extend(others.iter().take(9));
    list.push("zh-Hant");
    assert_eq!(best(&list, &supported), Some("zh-Hant"));
}

#[test]
fn a_tie_goes_to_the_earlier_pair_and_a_paradigm_wins_within_an_entry() {
    // Two candidates at 4 for `fr-BE`: the application's first.
    assert_eq!(best(&["fr-BE"], &["fr-FR", "fr-CA"]), Some("fr-FR"));
    assert_eq!(best(&["fr-BE"], &["fr-CA", "fr-FR"]), Some("fr-CA"));
    // A region difference of 5 on the first entry ties an exact match on
    // the second (demoted 5): the earlier entry wins.
    assert_eq!(
        best(&["en-US", "en-GB"], &["en-GB", "en-AU"]),
        Some("en-GB")
    );
    assert_eq!(best(&["en-AU", "fr"], &["en-US", "fr"]), Some("en-US"));
    // §4.4.1: among one entry's candidates at the same distance, a paradigm
    // locale wins whatever the order (`es-419` and `es-CR` are both 4 from
    // `es-MX`).
    assert_eq!(best(&["es-MX"], &["es-CR", "es-419"]), Some("es-419"));
    assert_eq!(best(&["es-MX"], &["es-419", "es-CR"]), Some("es-419"));
}

#[test]
fn a_macroregion_is_inside_a_variable_when_all_its_contents_are() {
    // `419` lies inside `$americas` (019), since all its contents do:
    // `es-419` ↔ `es-MX` is 4, and `es-419` ↔ `es` 5.
    assert_eq!(distance("es-419", "es-MX"), 4);
    assert_eq!(distance("es-419", "es"), 5);
    // `001` straddles `$enUS`: some of it is inside, some not. It is not
    // inside, so it is in `$!enUS` — every region not in `$enUS` — and the
    // rule that sends such an English to `en-GB` (3) applies; to `en-US` it
    // is the general English distance, 5.
    assert_eq!(distance("en-001", "en-GB"), 3);
    assert_eq!(distance("en-001", "en-US"), 5);
    assert_eq!(best(&["en-001"], &["en", "en-GB"]), Some("en-GB"));
    // Europe likewise.
    assert_eq!(distance("en-150", "en-GB"), 3);
}

#[test]
fn a_readers_und_is_not_maximized() {
    // Filled in, `und` would be en-Latn-US and match `en` exactly,
    // outranking the reader's real languages (§4.4's example, below).
    assert_eq!(best(&["und"], &["en"]), None);
    assert_ne!(distance("und", "en"), 0);
    assert_eq!(best(&["und", "it"], &["en", "it"]), Some("it"));
    assert_eq!(best(&["und-US"], &["en-US"]), None);
}

#[test]
fn a_tag_the_data_cannot_fill_keeps_its_empty_fields() {
    // `qaa` (private use) has no likely subtags: it still matches itself,
    // and a script on one side only is a script difference.
    assert_eq!(best(&["qaa"], &["en", "qaa"]), Some("qaa"));
    assert_eq!(distance("qaa", "qaa-Latn"), 50);
    assert_eq!(best(&["qaa-Latn"], &["qaa"]), None);
    // A language of five to eight letters is a language too.
    assert_eq!(best(&["abcdef"], &["abcdef", "en"]), Some("abcdef"));
}

#[test]
fn posix_names_are_read_as_tags() {
    // `_` read as `-`, any case, a `.codeset` or `@modifier` left out.
    let locales = ["en", "fr-CA", "ar"];
    assert_eq!(best(&["fr_CA.UTF-8"], &locales), Some("fr-CA"));
    assert_eq!(best(&["FR_ca"], &locales), Some("fr-CA"));
    assert_eq!(best(&["ar_EG.UTF-8@latin"], &locales), Some("ar"));
    assert_eq!(best(&["en_US.ISO-8859-1"], &locales), Some("en"));
    // `C`, `POSIX`, `*` and an empty value never match.
    for tag in ["", "*", "C", "POSIX", "C.UTF-8", "posix"] {
        assert_eq!(best(&[tag], &locales), None, "{tag:?}");
    }
}

#[test]
fn what_follows_the_region_is_ignored() {
    let locales = ["en", "fr-CA", "de"];
    assert_eq!(best(&["fr-CA-x-private"], &locales), Some("fr-CA"));
    assert_eq!(best(&["en-US-u-ca-gregory"], &locales), Some("en"));
    assert_eq!(best(&["de-DE-1996"], &locales), Some("de"));
    // An extended language subtag is skipped.
    assert_eq!(best(&["zh-yue-HK"], &["zh-TW", "en"]), Some("zh-TW"));
}

// --------------------------------------- UTS #35 Part 1's worked examples

#[test]
fn section_4_4_a_reader_of_several_languages() {
    // A reader of American English, German, French, Swiss German and
    // Italian, and an application with Japanese, German and Taiwanese
    // Chinese: German, 5 (the second entry, exact).
    assert_eq!(
        best(
            &["en-US", "de", "fr", "gsw", "it"],
            &["ja-JP", "de", "zh-TW"]
        ),
        Some("de")
    );
}

#[test]
fn section_4_4_the_opening_illustration_is_not_reproduced() {
    // The same section's illustration serves a reader of plain Chinese the
    // application's Taiwanese Chinese. It is the closest of the three, but
    // 54 away (script 50, region 4), above any threshold the section
    // allows, so its own steps give the default: the illustration ranks the
    // candidates and leaves the threshold out (18, C3's text half).
    assert_eq!(distance("zh", "zh-TW"), 54);
    assert_eq!(best(&["zh"], &["ja-JP", "de", "zh-TW"]), None);
}

#[test]
fn section_4_4_demotion_of_a_later_entry() {
    // A reader of Austrian German then French, and an application with
    // German, French and Japanese: German (4), not the exact French (5).
    assert_eq!(best(&["de-AT", "fr"], &["de", "fr", "ja"]), Some("de"));
}

#[test]
fn section_4_4_und_is_not_filled_in() {
    // `und` then Italian, against English and Italian: Italian.
    assert_eq!(best(&["und", "it"], &["en", "it"]), Some("it"));
}

#[test]
fn section_4_4_1_paradigm_and_cluster() {
    // English of Saudi Arabia, against Guam's, plain, India's and the
    // UK's: the UK's. CLDR 48 decides it by rule (`$!enUS` → GB, 3, against
    // 4 for India), before the paradigm tie-break would.
    assert_eq!(
        best(&["en-SA"], &["en-GU", "en", "en-IN", "en-GB"]),
        Some("en-GB")
    );
    assert_eq!(distance("en-SA", "en-GB"), 3);
    assert_eq!(distance("en-SA", "en-IN"), 4);
}

#[test]
fn section_4_4_1_macroregions() {
    // Latin American Spanish is closer to Mexico's than to Spain's; Mexico's
    // to Latin America's than to Spain's; and to Latin America's than to one
    // of its members (the paradigm wins the tie).
    assert_eq!(best(&["es-419"], &["es-MX", "es"]), Some("es-MX"));
    assert_eq!(best(&["es-MX"], &["es-419", "es"]), Some("es-419"));
    assert_eq!(best(&["es-MX"], &["es-CR", "es-419"]), Some("es-419"));
}

#[test]
fn section_4_2_a_fallback_from_a_list() {
    // §4.2's table: a reader of English then French, against Canadian
    // French and Russian: Canadian French (demotion 5, region 4).
    assert_eq!(best(&["en", "fr"], &["fr-CA", "ru"]), Some("fr-CA"));
}

// --------------------------------------------- what the 1.x matchers passed

/// `mf2-native` 1.x's `locale.rs` table.
const NATIVE_1X: [&str; 6] = ["en", "fr-FR", "fr-CA", "ar", "zh-CN", "sr-Latn"];

#[test]
fn native_1x_cases_still_pass() {
    // matches_case_insensitively_and_reads_posix_names
    assert_eq!(best(&["FR_ca"], &NATIVE_1X), Some("fr-CA"));
    assert_eq!(best(&["fr_CA.UTF-8"], &NATIVE_1X), Some("fr-CA"));
    assert_eq!(best(&["ar_EG.UTF-8@latin"], &NATIVE_1X), Some("ar"));
    // truncates_and_then_matches_by_language
    assert_eq!(best(&["fr-CA-x-private"], &NATIVE_1X), Some("fr-CA"));
    assert_eq!(best(&["en-GB"], &NATIVE_1X), Some("en"));
    assert_eq!(best(&["fr-BE"], &NATIVE_1X), Some("fr-FR"));
    // never_crosses_scripts, as far as CLDR's data refuses a script
    assert_eq!(best(&["zh-TW"], &NATIVE_1X), None);
    assert_eq!(best(&["zh-CN"], &NATIVE_1X), Some("zh-CN"));
    assert_eq!(best(&["sr-Latn-RS"], &NATIVE_1X), Some("sr-Latn"));
    // rejects_empty_wildcard_and_posix_default
    for tag in ["", "*", "C", "POSIX", "C.UTF-8"] {
        assert_eq!(best(&[tag], &NATIVE_1X), None, "{tag}");
    }
    // takes_the_first_candidate_that_matches
    assert_eq!(best(&["de-DE", "fr-CA", "en"], &NATIVE_1X), Some("fr-CA"));
    assert_eq!(best(&["de", "it"], &NATIVE_1X), None);
}

#[test]
fn web_1x_cases_still_pass() {
    // `mf2-axum`'s lookup_truncates_then_falls_back_to_the_language.
    let locales = ["en", "fr-CA", "ar"];
    assert_eq!(best(&["en"], &locales), Some("en"));
    assert_eq!(best(&["EN-GB"], &locales), Some("en"));
    assert_eq!(best(&["fr"], &locales), Some("fr-CA"));
    assert_eq!(best(&["de"], &locales), None);
    assert_eq!(best(&["*"], &locales), None);
    // The csr check's readers (tools/e2e/checks/csr.mjs).
    let demo = ["ar", "en", "fr"];
    assert_eq!(best(&["fr-CA"], &demo), Some("fr"));
    assert_eq!(best(&["ar-EG"], &demo), Some("ar"));
    assert_eq!(best(&["de-DE"], &demo), None);
}

#[test]
fn what_questions_11_and_15_change_from_1x() {
    // Each case, with what 1.x gave.
    //
    // Serbian's scripts are served for each other (question 11): 1.x's
    // native matcher refused another script (`None`), and a Serbian region
    // the table lacked.
    assert_eq!(best(&["sr-Cyrl"], &NATIVE_1X), Some("sr-Latn"));
    assert_eq!(best(&["sr-RS"], &["sr-Latn"]), Some("sr-Latn"));
    // Traditional Chinese no longer reaches `zh` by truncation (question
    // 15): 1.x served `zh` to `zh-Hant-TW`, `zh-Hant` and `zh-TW`, natively
    // and on the web.
    for tag in ["zh-Hant-TW", "zh-Hant", "zh-TW"] {
        assert_eq!(best(&[tag], &["zh", "en"]), None, "{tag}");
    }
    // The web's "any locale of the same language" step goes (question 15):
    // it served Simplified to a reader of Traditional, and the other
    // script of a language CLDR does not serve across.
    assert_eq!(best(&["zh-TW"], &["zh-CN"]), None);
    assert_eq!(best(&["pa-PK"], &["pa"]), None);
    assert_eq!(best(&["uz-AF"], &["uz"]), None);
    // And it is replaced by CLDR's distances: another region is served
    // (`fr` finds `fr-CA`, as 1.x's step did), the closest one first.
    assert_eq!(best(&["en-AU"], &["en-US", "en-GB"]), Some("en-GB"));
}

// --------------------------------------------------------- a corpus's cut

/// `mf2-build`'s cut of the shipped table for `locales`, as the generated
/// module's `LANGUAGE_MATCHING` holds it.
fn cut(locales: &[&str]) -> &'static LanguageMatching {
    let e = mf2_locale_data::matching::Matching::shipped()
        .expect("the shipped table")
        .cut(locales)
        .encode()
        .expect("encodes");
    let variables: Vec<&'static [u16]> = e.variables.into_iter().map(|v| &*v.leak()).collect();
    Box::leak(Box::new(LanguageMatching::new(
        e.scripts.leak(),
        e.regions.leak(),
        e.languages.leak(),
        e.language_values.leak(),
        e.tags.leak(),
        e.tag_values.leak(),
        e.language_pairs.leak(),
        e.language_distances.leak(),
        e.script_pairs.leak(),
        e.region_rules.leak(),
        variables.leak(),
        e.paradigms.leak(),
        e.defaults,
    )))
}

/// Readers to try: every tag the shipped table fills, those of this file,
/// and a few regions and scripts of each language the corpora name.
fn readers() -> Vec<String> {
    let table = mf2_locale_data::matching::Matching::shipped().expect("the shipped table");
    let mut out: Vec<String> = table.likely.keys().cloned().collect();
    for tag in [
        "en-001",
        "en-150",
        "en-SA",
        "en-GU",
        "es-419",
        "es-CR",
        "fr-BE",
        "zh-Hant",
        "zh-Hans",
        "zh-HK",
        "sr-Latn",
        "sr-Cyrl",
        "pa-Arab",
        "und",
        "und-US",
        "qaa",
        "de-AT",
        "gsw",
        "ar-Latn",
        "hi-Latn",
        "ja-Hira",
        "ko-Hang",
        "fr_CA.UTF-8",
        "C",
        "*",
        "",
    ] {
        out.push(tag.to_owned());
    }
    for language in [
        "en", "es", "pt", "fr", "ar", "zh", "sr", "de", "pl", "cy", "ja",
    ] {
        for region in [
            "US", "GB", "MX", "419", "BR", "PT", "FR", "CA", "EG", "MA", "TW", "CN",
        ] {
            out.push(format!("{language}-{region}"));
        }
        for script in ["Latn", "Cyrl", "Arab", "Hant", "Hans"] {
            out.push(format!("{language}-{script}"));
        }
    }
    out
}

#[test]
fn a_corpus_cut_gives_every_reader_what_the_whole_table_gives() {
    let corpora: [&[&str]; 8] = [
        &["ar", "en", "fr"],
        &["de", "en", "es", "fr"],
        &[
            "en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy",
        ],
        &["en", "pl", "en-XA", "ar-XB"],
        &["en", "zh", "zh-TW", "zh-HK", "yue"],
        &["sr", "sr-Latn", "hr", "bs"],
        &["en", "en-GB", "es", "es-419", "es-MX", "pt-BR", "pt-PT"],
        &["pa", "pa-Arab", "ur", "hi"],
    ];
    let readers = readers();
    let whole = cldr();
    for corpus in corpora {
        let part = cut(corpus);
        let table: Vec<(&str, Dir)> = corpus.iter().map(|t| (*t, Dir::Ltr)).collect();
        for reader in &readers {
            assert_eq!(
                whole.best_match([reader.as_str()], &table),
                part.best_match([reader.as_str()], &table),
                "{reader} against {corpus:?}"
            );
            for own in corpus {
                // The same distance, or both beyond any match.
                let (w, p) = (
                    whole.distance_of(reader, own),
                    part.distance_of(reader, own),
                );
                assert!(
                    w == p || w.zip(p).is_some_and(|(w, p)| w >= 50 && p >= 50),
                    "{reader} → {own} in {corpus:?}: {w:?} whole, {p:?} cut"
                );
            }
        }
        // Lists: windows of the readers, each against the corpus.
        for window in readers.windows(4).step_by(7) {
            let list = window.iter().map(String::as_str);
            assert_eq!(
                whole.best_match(list.clone(), &table),
                part.best_match(list, &table),
                "{window:?} against {corpus:?}"
            );
        }
    }
}

#[test]
fn a_cut_is_small() {
    // What a browser's client carries for the demos' corpus: the rules that
    // can serve Arabic, English or French, and the likely subtags of the
    // languages whose readers they accept.
    let e = mf2_locale_data::matching::Matching::shipped()
        .expect("the shipped table")
        .cut(&["ar", "en", "fr"])
        .encode()
        .expect("encodes");
    assert!(
        e.languages.len() + e.tags.len() < 150,
        "{}",
        e.languages.len()
    );
    assert!(e.language_pairs.len() < 150, "{}", e.language_pairs.len());
}
