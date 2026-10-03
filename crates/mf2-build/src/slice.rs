//! Locale data, sliced to what a corpus uses (`plans/02-catalog-format.md`
//! §4.4).
//!
//! `mf2::compile_str` does this for one message; here it is the union over a
//! locale's *flattened* message set — after fallback, because a translation
//! may use functions and literal options its source does not — narrowed by
//! the feature set (no `fn-number`, no number entries at all) and widened by
//! `mf2.toml` `[locale_data]`.

use mf2_locale_data::LocaleNeeds;
use mf2_locale_data::number::{NumberNeeds, Selection};
use mf2_model::{Declaration, Expression, FunctionRef, Message, OptionValue, SelectMessage};

use crate::config::LocaleDataConfig;
use crate::features::Features;

/// What one locale's catalog must carry, and what `check` noticed on the way.
// Each flag is an independent fact about the corpus, and each selects its own
// part of the entries; grouping them would only hide that.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default)]
pub struct Slice {
    /// What the date functions need of ICU4X, when `datetime-icu` is on.
    #[cfg(feature = "icu-blob")]
    pub dates: mf2_locale_data::icu_blob::DateNeeds,
    /// The entries to ask `mf2_locale_data::locale_entries` for.
    pub needs: LocaleNeeds,
    /// `:currency` was used with a non-literal `currency` option, so every
    /// currency CLDR has had to go in.
    pub dynamic_currency: bool,
    /// The same for `:unit`.
    pub dynamic_unit: bool,
    /// The corpus formats a number somewhere — a numeric function, or a
    /// placeholder that can receive one. Without `fn-number` that is the
    /// `neutral-numbers` warning; with it, the reason for `number.symbols`.
    pub formats_numbers: bool,
    /// The corpus has a placeholder with no function at all, which can
    /// therefore receive a number or a date at run time. Only then do the
    /// unannotated hooks belong in the registry (#90).
    pub unannotated: bool,
}

/// What `messages` need of `locale`'s data.
pub fn of(messages: &[&Message<'_>], config: &LocaleDataConfig, features: &Features) -> Slice {
    let mut slice = Slice::default();
    let mut numbers = NumberNeeds::default();
    for message in messages {
        numbers.add_message(message);
        let (cardinal, ordinal) = plural_kinds(message);
        slice.needs.cardinal |= cardinal;
        slice.needs.ordinal |= ordinal;
        scan_dynamic(message, &mut slice);
        #[cfg(feature = "icu-blob")]
        if features.datetime_icu() {
            slice.dates.add_message(message);
        }
    }
    slice.formats_numbers = numbers.symbols;
    if features.fn_number() {
        // The configured sets widen what the corpus was found to use.
        if let Some(currency) = &mut numbers.currency {
            currency.codes = config.currencies.with_used(&currency.codes);
        }
        if let Some(unit) = &mut numbers.unit {
            unit.ids = config.units.with_used(&unit.ids);
        }
        slice.needs.numbers = numbers;
    } else {
        // Without the feature the client formats digits with neutral
        // symbols, so none of the number entries would ever be read.
        slice.needs.numbers = NumberNeeds::default();
    }
    slice
}

/// Which plural rule sets a message's selectors need.
///
/// A selector takes its function from the declaration that binds it; the
/// `select` option says which rules apply (`plans/03-runtime.md` §4). An
/// option value that is not a literal could be either, so it is both.
pub(crate) fn plural_kinds(message: &Message<'_>) -> (bool, bool) {
    match message {
        Message::Select(select) => selector_kinds(select),
        _ => (false, false),
    }
}

/// `(cardinal, ordinal)`: which rule sets `select`'s selectors need.
pub(crate) fn selector_kinds(select: &SelectMessage<'_>) -> (bool, bool) {
    let mut cardinal = false;
    let mut ordinal = false;
    for selector in &select.selectors {
        match selector_function(select, &selector.name, 0) {
            // `:string` selects by exact text; nothing numeric about it.
            Some(f) if f.name == "string" => {}
            Some(f) => match f.options.get("select") {
                None => cardinal = true,
                Some(OptionValue::Literal(l)) => match l.value.as_ref() {
                    "exact" => {}
                    "ordinal" => ordinal = true,
                    // `plural`, and anything else: a value nobody knows makes
                    // the runtime report Bad Option and select by the
                    // default, which is the cardinal rules.
                    _ => cardinal = true,
                },
                // A variable: only the run knows which rules it asks for, so
                // carry both. `check`'s `dynamic-select` says why that costs.
                Some(_) => {
                    cardinal = true;
                    ordinal = true;
                }
            },
            // An unannotated selector is a Missing Selector Annotation, which
            // validation already refused; nothing to carry.
            None => {}
        }
    }
    (cardinal, ordinal)
}

/// The function bound to `$name` by the last declaration that declares it,
/// following a `.local` that only renames another variable.
///
/// `depth` bounds that chain: a cycle is a Duplicate Declaration, which
/// validation refuses, but nothing here should loop on a model built in code.
pub fn selector_function<'m>(
    select: &'m SelectMessage<'_>,
    name: &str,
    depth: u32,
) -> Option<&'m FunctionRef<'m>> {
    declared_function(&select.declarations, name, depth)
}

/// The same for any message: the function `$name` carries by the time a
/// pattern can use it, or `None` if it reaches the pattern unannotated.
pub fn declared_function<'m>(
    declarations: &'m [Declaration<'m>],
    name: &str,
    depth: u32,
) -> Option<&'m FunctionRef<'m>> {
    if depth > 16 {
        return None;
    }
    let mut found = None;
    for declaration in declarations {
        if declaration.name() == name {
            found = Some(declaration);
        }
    }
    match found? {
        Declaration::Input(input) => input.value.function.as_ref(),
        Declaration::Local(local) => match local.value.function() {
            Some(function) => Some(function),
            // `.local $x = {$y}` takes its function from `$y`.
            None => match &local.value {
                Expression::Variable(v) => {
                    declared_function(declarations, v.arg.name.as_ref(), depth + 1)
                }
                _ => None,
            },
        },
        _ => None,
    }
}

/// Notes a `:currency` or `:unit` whose set the build could not narrow, and
/// a placeholder with no function at all.
fn scan_dynamic(message: &Message<'_>, slice: &mut Slice) {
    let mut note = |function: Option<&FunctionRef<'_>>| {
        let Some(function) = function else { return };
        match function.name.as_ref() {
            "currency" => {
                if !matches!(
                    function.options.get("currency"),
                    Some(OptionValue::Literal(_))
                ) {
                    slice.dynamic_currency = true;
                }
            }
            "unit" if !matches!(function.options.get("unit"), Some(OptionValue::Literal(_))) => {
                slice.dynamic_unit = true;
            }
            _ => {}
        }
    };
    for declaration in message.declarations() {
        match declaration {
            Declaration::Input(input) => note(input.value.function.as_ref()),
            Declaration::Local(local) => note(local.value.function()),
            _ => {}
        }
    }
    for pattern in patterns(message) {
        for part in pattern {
            if let mf2_model::PatternPart::Expression(e) = part {
                note(e.function());
                // A bare `{$n}` only reaches the unannotated hooks when
                // nothing annotated `$n` on the way: `.input {$n :integer}`
                // resolves it first, and `{$n}` then formats that result.
                if let (None, Expression::Variable(v)) = (e.function(), e) {
                    let declared =
                        declared_function(message.declarations(), v.arg.name.as_ref(), 0);
                    if declared.is_none() {
                        slice.unannotated = true;
                    }
                }
            }
        }
    }
}

fn patterns<'m>(message: &'m Message<'_>) -> Vec<&'m mf2_model::Pattern<'m>> {
    match message {
        Message::Pattern(p) => vec![&p.pattern],
        Message::Select(s) => s.variants.iter().map(|v| &v.value).collect(),
        _ => Vec::new(),
    }
}

/// Whether a selection names every code (`"all"`), for reporting.
pub fn is_all(selection: &Selection) -> bool {
    matches!(selection, Selection::All)
}

/// The `icu.blob` entry a locale needs, when `datetime-icu` is on and the
/// corpus formats — or can receive — a date.
///
/// `mf2::compile_str` builds the same entry for one message; every variant of
/// the ICU4X backend is covered, since the build cannot know which the
/// application links.
#[cfg(feature = "icu-blob")]
pub fn icu_entry(
    locale: &str,
    slice: &Slice,
) -> Result<Option<(u32, Vec<u8>)>, mf2_locale_data::Error> {
    use mf2_locale_data::icu_blob::{IcuBlobSpec, icu_blob};
    if slice.dates.is_empty() {
        return Ok(None);
    }
    let blob = icu_blob(locale, &IcuBlobSpec::every_variant(slice.dates.clone()))?;
    Ok(Some((mf2_catalog::format::locale_key::ICU_BLOB, blob)))
}
