//! `compile_str`: an ad-hoc message as a one-message catalog
//! (there is no "format from a model" path).

use alloc::vec::Vec;

use mf2_catalog::writer::{self, Options};
use mf2_catalog::{Catalog, Manifest, MsgId};
use mf2_locale_data::number::NumberNeeds;
use mf2_locale_data::{LocaleNeeds, direction, locale_entries};

use crate::error::CompileError;

/// A compiled message: its one-message catalog and the catalog's manifest.
#[derive(Debug)]
#[non_exhaustive]
pub struct Compiled {
    /// The catalog, loaded: format [`Compiled::ID`] from it.
    pub catalog: Catalog,
    /// Its manifest (the message's slots: `manifest.slots[0]`).
    #[doc(hidden)]
    pub manifest: Manifest,
}

impl Compiled {
    /// The id of the one message.
    pub const ID: MsgId = MsgId::from_raw(0);
}

/// Compiles MF2 `source` for `locale` into a one-message catalog: parse,
/// validate (syntax and data-model errors refuse the message, with their
/// kinds), analyze the variables (the slots), and write it with the
/// locale's direction, plural rules (both kinds) and number data (CLDR
/// 48.2.1): `number.symbols` always — any placeholder can receive a number,
/// which `fn-number` localizes — and what the message's numeric functions
/// need by the catalog's slicing rule: the
/// patterns, and the currencies and units its literal options name (a
/// variable option value: all of them). With `host-std-datetime-icu`, also the
/// `icu.blob` of what the message formats with the date functions, or can
/// receive as a date/time argument (a placeholder whose variable has no
/// function), by the same rule, for every variant of the ICU4X backend.
pub fn compile_str(source: &str, locale: &str) -> Result<Compiled, CompileError> {
    compile(source, locale, false)
}

/// [`compile_str`], with the catalog in its production form: COLD and IDS
/// stripped. It formats identically.
pub fn compile_str_stripped(source: &str, locale: &str) -> Result<Compiled, CompileError> {
    compile(source, locale, true)
}

fn compile(source: &str, locale: &str, strip: bool) -> Result<Compiled, CompileError> {
    let parsed = mf2_syntax::parse_model(source);
    let model = match parsed.message {
        Some(m) if parsed.diagnostics.is_empty() => m,
        _ => return Err(CompileError::Invalid(parsed.diagnostics.into_vec())),
    };
    let analysis = mf2_syntax::analyze(&model);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
    let mut options = Options::new(locale, direction(locale)?);
    options.cldr_version = Some(mf2_locale_data::CLDR_VERSION);
    let mut numbers = NumberNeeds::default();
    numbers.add_message(&model);
    numbers.symbols = true;
    let mut needs = LocaleNeeds::default();
    needs.cardinal = true;
    needs.ordinal = true;
    needs.numbers = numbers;
    options.locale_entries = locale_entries(locale, &needs)?;
    // `icu` natively (`host-std-datetime-icu`, which every native family's
    // `icu` turns on): the date data of what the message formats with the
    // date functions, or can receive as a date/time argument (02 §4.4), for
    // every backend variant.
    #[cfg(feature = "host-std-datetime-icu")]
    {
        use mf2_locale_data::icu_blob::{DateNeeds, IcuBlobSpec, icu_blob};
        let mut dates = DateNeeds::default();
        dates.add_message(&model);
        if !dates.is_empty() {
            options.locale_entries.push((
                mf2_catalog::format::locale_key::ICU_BLOB,
                icu_blob(locale, &IcuBlobSpec::every_variant(dates))?,
            ));
        }
    }
    if strip {
        options = options.stripped();
    }
    let (bytes, manifest) = writer::single(&model, &slots, &options)?;
    let catalog = Catalog::new(bytes, manifest.hash())?;
    Ok(Compiled { catalog, manifest })
}
