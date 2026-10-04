//! The date goldens under the form each language gets (plan/08 §5.1).
//!
//! A build links the narrowest ICU4X form its corpus needs — Gregorian only
//! unless a language prefers another calendar or a message names one, zone
//! names only for a zone style — and cuts the date slice for that form
//! alone. Each golden of the `dates` family, formatted from a catalog whose
//! slice is cut for its message's form in its language, through the date
//! handlers `mf2::__date_statics!` makes for that form, must give the
//! committed output (which `tests/goldens.rs` holds the widest form to).

use mf2_catalog::Catalog;
use mf2_catalog::format::locale_key::ICU_BLOB;
use mf2_catalog::writer::{self, Options as WriterOptions};
use mf2_conformance::goldens::{FAMILIES, cases};
use mf2_locale_data::direction;
use mf2_locale_data::icu_blob::{DateNeeds, IcuBlobSpec, icu_blob, prefers_gregorian};
use mf2_runtime::{BidiStrategy, FormatContext, Formatter, MsgId, Registry};

/// A registry over the date handlers of one form, as a generated module
/// makes them.
macro_rules! form {
    ($name:ident, $calendars:ident $zones:ident) => {
        #[allow(dead_code, unused_imports)]
        mod $name {
            mf2::__date_statics!($calendars $zones);
            static FUNCTIONS: [(&str, &dyn mf2_runtime::Function); 3] =
                [("date", &DATE), ("datetime", &DATETIME), ("time", &TIME)];
            pub(crate) static REGISTRY: mf2_runtime::Registry = mf2_runtime::Registry::new(&FUNCTIONS);
        }
    };
}

form!(gregorian_no_zones, gregorian no_zones);
form!(gregorian_zones, gregorian zones);
form!(any_no_zones, any no_zones);
form!(any_zones, any zones);

fn registry(any_calendar: bool, zones: bool) -> &'static Registry {
    match (any_calendar, zones) {
        (false, false) => &gregorian_no_zones::REGISTRY,
        (false, true) => &gregorian_zones::REGISTRY,
        (true, false) => &any_no_zones::REGISTRY,
        (true, true) => &any_zones::REGISTRY,
    }
}

#[test]
fn the_date_goldens_hold_under_the_form_each_language_gets() {
    let family = FAMILIES
        .iter()
        .find(|f| f.name == "dates")
        .expect("the dates family");
    let mut cx = FormatContext::new(&mf2::host_std::ZONES_HOST);
    cx.bidi = BidiStrategy::None;
    let mut forms = [[0usize; 2]; 2];
    for g in cases(family).expect("the golden cases") {
        let model = mf2_syntax::parse_model(&g.message)
            .message
            .expect("a golden message parses");
        let mut needs = DateNeeds::default();
        needs.add_message(&model);
        let any_calendar =
            !prefers_gregorian(g.locale).expect("a locale tag") || needs.other_calendars();
        let zones = needs.zone_names();
        forms[usize::from(any_calendar)][usize::from(zones)] += 1;

        let analysis = mf2_syntax::analyze(&model);
        let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
        let mut options = WriterOptions::new(g.locale, direction(g.locale).expect("a direction"));
        options.cldr_version = Some(mf2_locale_data::CLDR_VERSION);
        let blob = icu_blob(g.locale, &IcuBlobSpec::new(any_calendar, zones, needs))
            .expect("the slice builds");
        options.locale_entries.push((ICU_BLOB, blob));
        let (bytes, manifest) = writer::single(&model, &slots, &options).expect("the catalog");
        let catalog = Catalog::new(bytes, manifest.hash()).expect("the catalog loads");

        let f = Formatter::new(&catalog, registry(any_calendar, zones), &cx);
        let named: Vec<(&str, mf2_runtime::Arg<'_>)> = g
            .case
            .args
            .iter()
            .map(|(n, a)| (n.as_str(), a.arg()))
            .collect();
        let mut text = String::new();
        let mut errors = Vec::new();
        f.write_named(MsgId::from_raw(0), &named, &mut text, &mut errors);

        let widest = mf2_l4_runner::run(&g.case).expect("the widest form formats");
        assert_eq!(
            (text.as_str(), mf2_l4_runner::error_names(&errors)),
            (widest.text.as_str(), widest.errors.clone()),
            "{} {} {:?}: calendars {}, zones {}",
            g.locale,
            g.message,
            g.arg,
            if any_calendar { "any" } else { "gregorian" },
            zones,
        );
    }
    // The panel's languages all prefer the Gregorian calendar: three of the
    // four forms are reached (the fourth, a language like `th`, is the
    // build's test, `mf2-build`'s `tests/slicing.rs`).
    assert!(forms[0][0] > 0, "{forms:?}");
    assert!(forms[0][1] > 0, "{forms:?}");
    assert!(forms[1][0] > 0, "{forms:?}");
}
