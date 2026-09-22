//! `compile_str`: an ad-hoc message as a one-message catalog
//! (`plans/03-runtime.md` §1: there is no "format from a model" path).

use alloc::vec::Vec;

use mf2_catalog::writer::{self, Options};
use mf2_catalog::{Catalog, Manifest, MsgId};
use mf2_locale_data::{PluralKind, direction, plural_locale_entries};

use crate::error::CompileError;

/// A compiled message: its one-message catalog and the catalog's manifest.
#[derive(Debug)]
pub struct Compiled {
    /// The catalog, loaded: format [`Compiled::ID`] from it.
    pub catalog: Catalog,
    /// Its manifest (the message's slots: `manifest.slots[0]`).
    pub manifest: Manifest,
}

impl Compiled {
    /// The id of the one message.
    pub const ID: MsgId = MsgId::from_raw(0);
}

/// Compiles MF2 `source` for `locale` into a one-message catalog: parse,
/// validate (syntax and data-model errors refuse the message, with their
/// kinds), analyze the variables (the slots), and write it with the
/// locale's direction and plural rules (both kinds, CLDR 48.2.1).
pub fn compile_str(source: &str, locale: &str) -> Result<Compiled, CompileError> {
    compile(source, locale, false)
}

/// [`compile_str`], with the catalog in its production form: COLD and IDS
/// stripped (`plans/02-catalog-format.md` §2.3). It formats identically.
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
    options.locale_entries =
        plural_locale_entries(locale, &[PluralKind::Cardinal, PluralKind::Ordinal])?;
    if strip {
        options = options.stripped();
    }
    let (bytes, manifest) = writer::single(&model, &slots, &options)?;
    let catalog = Catalog::new(bytes, manifest.hash())?;
    Ok(Compiled { catalog, manifest })
}
