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
#[cfg(feature = "icu-blob")]
use crate::config::{DateCalendars, DatesConfig, ZoneNames};
use crate::features::Features;

/// What one locale's catalog must carry, and what `check` noticed on the way.
// Each flag is an independent fact about the corpus, and each selects its own
// part of the entries; grouping them would only hide that.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default)]
pub struct Slice {
    /// What the date functions need of ICU4X, when either side's date
    /// formatter is `icu`.
    #[cfg(feature = "icu-blob")]
    pub dates: mf2_locale_data::icu_blob::DateNeeds,
    /// The corpus's ICU4X form, which the slice is cut for alone: the same
    /// for every locale, so set once they are all sliced ([`date_form`]).
    #[cfg(feature = "icu-blob")]
    pub form: DateForm,
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

/// The form of the ICU4X date formatter a corpus links (`plan/08` §5.1):
/// Gregorian only or every calendar, with or without zone names. The
/// generated module states it, `mf2`'s `__date_statics!` turns it into the
/// date handlers of that type, and the date slice is cut for it alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DateForm {
    /// Every calendar (`Icu<AnyCalendar, _>`), not only the Gregorian one.
    pub any_calendar: bool,
    /// Zone names (`Icu<_, WithZones>`).
    pub zone_names: bool,
    /// Why the calendars are what they are, for `mf2 check`.
    pub calendar_reason: String,
    /// Why the zone names are what they are, for `mf2 check`.
    pub zone_reason: String,
}

impl Default for DateForm {
    /// The widest form, which every build had before the form was chosen.
    fn default() -> DateForm {
        DateForm {
            any_calendar: true,
            zone_names: true,
            calendar_reason: "the widest form".to_owned(),
            zone_reason: "the widest form".to_owned(),
        }
    }
}

impl DateForm {
    /// `__date_statics!`'s first word: `any` or `gregorian`.
    pub fn calendars_word(&self) -> &'static str {
        if self.any_calendar {
            "any"
        } else {
            "gregorian"
        }
    }

    /// `__date_statics!`'s second word: `zones` or `no_zones`.
    pub fn zones_word(&self) -> &'static str {
        if self.zone_names { "zones" } else { "no_zones" }
    }

    /// The form in words, with the reasons: what `mf2 check` prints.
    pub fn describe(&self) -> String {
        format!(
            "ICU4X dates: {} ({}); {} ({})",
            if self.any_calendar {
                "every calendar"
            } else {
                "the Gregorian calendar only"
            },
            self.calendar_reason,
            if self.zone_names {
                "zone names"
            } else {
                "no zone names"
            },
            self.zone_reason,
        )
    }
}

/// The corpus's form, from every locale's slice (`(tag, slice)`) and
/// `mf2.toml`'s `[dates]`: the calendars are Gregorian only unless a
/// language prefers another calendar or a message names one (or takes
/// `calendar` from a variable); zone names only if a message has
/// `timeZoneStyle`. A calendar only an argument carries is not seen here
/// (`mf2::DateTimeValue::with_calendar` is a run-time value): formatting it
/// with the Gregorian form is an *Unsupported Operation*, reported, with a
/// fallback value.
#[cfg(feature = "icu-blob")]
pub fn date_form(
    slices: &[(&str, &Slice)],
    config: &DatesConfig,
) -> Result<DateForm, mf2_locale_data::Error> {
    let (any_calendar, calendar_reason) = match config.calendars {
        DateCalendars::Gregorian => (false, "set by mf2.toml".to_owned()),
        DateCalendars::All => (true, "set by mf2.toml".to_owned()),
        DateCalendars::Auto => {
            let mut found = None;
            for (tag, _) in slices {
                if !mf2_locale_data::icu_blob::prefers_gregorian(tag)? {
                    found = Some(format!("`{tag}` prefers another calendar"));
                    break;
                }
            }
            if found.is_none() && slices.iter().any(|(_, s)| s.dates.other_calendars()) {
                found = Some(
                    "a message names another calendar, or takes `calendar` from a variable"
                        .to_owned(),
                );
            }
            match found {
                Some(reason) => (true, reason),
                None => (
                    false,
                    "every language prefers it and no message names another".to_owned(),
                ),
            }
        }
    };
    let (zone_names, zone_reason) = match config.zone_names {
        ZoneNames::Yes => (true, "set by mf2.toml".to_owned()),
        ZoneNames::No => (false, "set by mf2.toml".to_owned()),
        ZoneNames::Auto if slices.iter().any(|(_, s)| s.dates.zone_names()) => {
            (true, "a message has `timeZoneStyle`".to_owned())
        }
        ZoneNames::Auto => (false, "no message has `timeZoneStyle`".to_owned()),
    };
    Ok(DateForm {
        any_calendar,
        zone_names,
        calendar_reason,
        zone_reason,
    })
}

/// What `messages` need of `locale`'s data.
pub fn of(messages: &[&Message<'_>], config: &LocaleDataConfig, features: &Features) -> Slice {
    let mut slice = Slice::default();
    let mut numbers = NumberNeeds::default();
    // Cut when either side formats with ICU4X: the catalogs are the same
    // files for the server and the browser.
    #[cfg(feature = "icu-blob")]
    let cuts_date_slice = features.cuts_date_slice();
    for message in messages {
        numbers.add_message(message);
        let (cardinal, ordinal) = plural_kinds(message);
        slice.needs.cardinal |= cardinal;
        slice.needs.ordinal |= ordinal;
        scan_dynamic(message, &mut slice);
        #[cfg(feature = "icu-blob")]
        if cuts_date_slice {
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

/// The `icu.blob` entry a locale needs, when a side's formatter is `icu` and the
/// corpus formats — or can receive — a date.
///
/// It is cut for the corpus's form alone (`slice.form`, `plan/08` §5.1): the
/// generated module links that variant of the ICU4X backend and no other.
/// (`mf2::compile_str`, whose registry is the widest, covers every variant.)
#[cfg(feature = "icu-blob")]
pub fn icu_entry(
    locale: &str,
    slice: &Slice,
) -> Result<Option<(u32, Vec<u8>)>, mf2_locale_data::Error> {
    use mf2_locale_data::icu_blob::{IcuBlobSpec, icu_blob};
    if slice.dates.is_empty() {
        return Ok(None);
    }
    let spec = IcuBlobSpec::new(
        slice.form.any_calendar,
        slice.form.zone_names,
        slice.dates.clone(),
    );
    let blob = icu_blob(locale, &spec)?;
    Ok(Some((mf2_catalog::format::locale_key::ICU_BLOB, blob)))
}
