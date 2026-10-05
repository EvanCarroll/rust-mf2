//! `mf2 check`: every lint, on the parsed corpus.
//!
//! The checks run on models, never on text, so a lint says what is wrong with
//! the *message* — and every one of them is placed in the file it came from,
//! through the record's value map.

use std::collections::{BTreeMap, BTreeSet};

use mf2_locale_data::number::NumberNeeds;
use mf2_locale_data::plural::{PluralKind, plural_rules};
use mf2_model::{
    Attributes, Declaration, Expression, FunctionRef, Key, Message, OptionValue, Options, Pattern,
    PatternPart,
};
use mf2_syntax::Analysis;

use crate::config::{Config, Missing};
use crate::corpus::LocaleSource;
use crate::features::{Backend, DateBackend, Features, NumberBackend, Side, defines_option};
use crate::lint::{Level, Lint};
use crate::loader::Record;
use crate::manifest::Built;
use crate::report::{Report, Sink};

/// Everything the checks need: every locale, parsed, and the manifest.
pub struct Corpus<'a> {
    /// The locales, in tag order.
    pub sources: &'a [LocaleSource],
    /// Their models, parallel to each locale's records.
    pub models: &'a [Vec<Option<Message<'a>>>],
    /// Their ids.
    pub indexes: &'a [BTreeMap<&'a str, usize>],
    /// Which locale is the source.
    pub source_index: usize,
    /// The manifest built from it.
    pub manifest: &'a Built,
}

/// Which gated function families the whole corpus can use: what `mf2 check`
/// writes the feature list from, and what `unused-feature` holds the
/// features that are on against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    /// A message calls `:datetime`, `:date` or `:time`: the date family.
    pub dates: bool,
    /// A message formats or selects on a number (a numeric function, a
    /// plural selection, or a plain placeholder that can receive one): the
    /// number family.
    pub numbers: bool,
}

/// Runs every lint over the corpus, and says what it needs.
pub fn corpus(
    corpus: &Corpus<'_>,
    config: &Config,
    features: &Features,
    report: &mut Report,
) -> Needs {
    let mut used = Used::default();
    for (locale, source) in corpus.sources.iter().enumerate() {
        let mut sink = Sink::new(report, &source.tag);
        for (record_index, record) in source.loaded.records.iter().enumerate() {
            let Some(model) = corpus.models[locale]
                .get(record_index)
                .and_then(Option::as_ref)
            else {
                continue;
            };
            let analysis = mf2_syntax::analyze(model);
            used.add(model, &analysis);
            let mut at = At {
                sink: &mut sink,
                source,
                record,
                config,
            };
            functions(&mut at, model, &analysis, features);
            options(&mut at, model);
            markup(&mut at, model, &analysis);
            bidi(&mut at, model);
            normalization(&mut at, model);
            names(&mut at, model, &analysis);
            plural_categories(&mut at, model, &source.tag);
            if locale != corpus.source_index {
                against_source(&mut at, corpus, model, &analysis);
            }
        }
        coverage(&mut sink, corpus, locale, config);
    }
    unused_features(corpus, config, features, &used, report);
    let mut several = several_formatters::<NumberBackend>(features);
    several.extend(several_formatters::<DateBackend>(features));
    report_once(
        corpus,
        report,
        config.level(Lint::SeveralFormatters),
        Lint::SeveralFormatters,
        several,
    );
    Needs {
        dates: used.dates,
        numbers: used.numbers(),
    }
}

/// One message being checked, with everything a diagnostic needs.
struct At<'r, 'c> {
    sink: &'r mut Sink<'c>,
    source: &'r LocaleSource,
    record: &'r Record,
    config: &'r Config,
}

impl At<'_, '_> {
    /// Reports at `offset` inside the message's source.
    fn say(&mut self, lint: Lint, offset: u32, message: impl Into<String>) {
        let level = self.config.level(lint);
        if level == Level::Allow {
            return;
        }
        let position = self.source.position(self.record, offset, offset);
        let path = self.source.file(self.record).path.clone();
        self.sink.add(
            level,
            Some(lint),
            &path,
            position,
            Some(&self.record.id),
            message,
        );
    }

    /// Where `part` of the message's source begins: its first occurrence.
    ///
    /// A name the analysis had to normalize is not a substring of the source
    /// at all, and a name used twice has two positions; both cases report the
    /// first match, or the start of the message. A lint says *what* is wrong
    /// with a message — the position takes a reader to the line.
    fn offset_of(&self, part: &str) -> u32 {
        self.record
            .source
            .find(part)
            .and_then(|at| u32::try_from(at).ok())
            .unwrap_or(0)
    }
}

// ───────────────────────────────── functions ─────────────────────────────

/// A function the client build does not have, and one nothing provides.
fn functions(
    at: &mut At<'_, '_>,
    model: &Message<'_>,
    analysis: &Analysis<'_>,
    features: &Features,
) {
    for name in &analysis.functions {
        let offset = at.offset_of(name.spelling);
        match features.provides(&name.nfc) {
            Some(true) => {}
            Some(false) => {
                // The message names the formatters to write (`plan/08`
                // §3.3): a domain's own feature alone formats nothing.
                let message = features.gated(&name.nfc).unwrap_or_default();
                at.say(Lint::GatedFunction, offset, message);
            }
            None => {
                if !at.config.functions.contains_key(name.nfc.as_ref()) {
                    at.say(
                        Lint::UnknownFunction,
                        offset,
                        format!(
                            "no crate provides :{}; a custom function has to be \
                             named under [functions] in mf2.toml",
                            name.nfc
                        ),
                    );
                }
            }
        }
    }
    let _ = model;
}

// ───────────────────────────── unused features ───────────────────────────

/// What the whole corpus uses of the gated function families.
#[derive(Default)]
struct Used {
    /// A message calls `:datetime`, `:date` or `:time`.
    dates: bool,
    /// A message formats a number (a numeric function, or a placeholder that
    /// can receive one) or selects by plural rules.
    numbers: NumberNeeds,
    plural: bool,
}

impl Used {
    fn add(&mut self, model: &Message<'_>, analysis: &Analysis<'_>) {
        self.dates |= analysis
            .functions
            .iter()
            .any(|name| matches!(name.nfc.as_ref(), "datetime" | "date" | "time"));
        self.numbers.add_message(model);
        let (cardinal, ordinal) = crate::slice::plural_kinds(model);
        self.plural |= cardinal || ordinal;
    }

    /// Whether anything formats or selects on a number.
    fn numbers(&self) -> bool {
        let n = &self.numbers;
        n.symbols || n.percent || n.currency.is_some() || n.unit.is_some() || self.plural
    }
}

/// A function family that is on for this build and that no message can use
/// (`unused-feature`): once per family, for the whole corpus.
fn unused_features(
    corpus: &Corpus<'_>,
    config: &Config,
    features: &Features,
    used: &Used,
    report: &mut Report,
) {
    let level = config.level(Lint::UnusedFeature);
    let mut found = Vec::new();
    if let Some(on) = family_on::<NumberBackend>(features)
        && !used.numbers()
    {
        found.push(format!(
            "{on} on for this build, and no message formats or selects on a number: \
             no numeric function, plural selection or plain placeholder that could \
             receive one, so the number code links for nothing; drop it (another \
             crate in the workspace may have turned it on), or set \
             `unused-feature = \"allow\"` in mf2.toml"
        ));
    }
    if let Some(on) = family_on::<DateBackend>(features)
        && !used.dates
    {
        found.push(format!(
            "{on} on for this build, and no message uses :datetime, :date or :time. \
             Only a date function formats a date: a date handed to a plain \
             placeholder is an error, so no message can use the formatter; drop \
             it (another crate in the workspace may have turned it on), or set \
             `unused-feature = \"allow\"` in mf2.toml"
        ));
    }
    // A family's feature with its framework off (`plan/08` §3.5): it
    // still turns its side's formatter on, under a name that says nothing
    // about this build.
    found.extend(without_framework::<NumberBackend>(features));
    found.extend(without_framework::<DateBackend>(features));
    if level == Level::Allow {
        return;
    }
    report_once(corpus, report, level, Lint::UnusedFeature, found);
}

/// The formatters of `B`'s domain that are on, as a crate writes them, or
/// the domain's own feature alone: `` `a` and `b` are ``. `None` with
/// nothing of the domain on.
fn family_on<B: Backend>(features: &Features) -> Option<String> {
    let mut on: Vec<String> = Side::ALL
        .into_iter()
        .flat_map(|side| features.features_on::<B>(side))
        .map(|name| format!("`{name}`"))
        .collect();
    if on.is_empty() && features.domain_on::<B>() {
        on.push(format!("`{}`", B::DOMAIN));
    }
    (!on.is_empty()).then(|| is_on(&on))
}

/// The features of `B`'s domain on whose framework is not, each as its
/// `unused-feature` message.
fn without_framework<B: Backend>(features: &Features) -> Vec<String> {
    features
        .without_framework::<B>()
        .into_iter()
        .map(|feature| {
            format!(
                "`{feature}` is on for this build and its framework is not: it is the \
                 {} of a framework this crate does not use; write the \
                 family of the framework that formats here (`mf2 check` names it), \
                 or set `unused-feature = \"allow\"` in mf2.toml",
                B::NOUN
            )
        })
        .collect()
}

/// Several formatters of `B`'s domain on one side (`several-formatters`):
/// once per side, for the whole corpus, naming the one that formats.
fn several_formatters<B: Backend>(features: &Features) -> Vec<String> {
    let mut found = Vec::new();
    for side in Side::ALL {
        let formatters = features.on::<B>(side);
        let Some(&strongest) = formatters.first() else {
            continue;
        };
        if formatters.len() < 2 {
            continue;
        }
        let on: Vec<String> = features
            .features_on::<B>(side)
            .iter()
            .map(|name| format!("`{name}`"))
            .collect();
        found.push(format!(
            "{} are on for this build, {} {}s for {}: a build formats \
             with one, the strongest, and here that is `{}` ({}); drop the others, \
             or set `several-formatters = \"allow\"` in mf2.toml",
            on.join(" and "),
            formatters.len(),
            B::NOUN,
            side.name(),
            strongest.name(),
            strongest.what()
        ));
    }
    found
}

/// Reports each of `found` once for the whole corpus, at the top of the
/// source locale's first file.
fn report_once(
    corpus: &Corpus<'_>,
    report: &mut Report,
    level: Level,
    lint: Lint,
    found: Vec<String>,
) {
    if found.is_empty() || level == Level::Allow {
        return;
    }
    let Some(source) = corpus.sources.get(corpus.source_index) else {
        return;
    };
    let file = source
        .loaded
        .files
        .first()
        .map_or_else(|| source.path.clone(), |f| f.path.clone());
    let at = mf2_resource::Position { line: 1, column: 1 };
    let mut sink = Sink::new(report, &source.tag);
    for message in found {
        sink.add(level, Some(lint), &file, at, None, message);
    }
}

/// "`a` is" or "`a` and `b` are".
fn is_on(names: &[String]) -> String {
    match names {
        [one] => format!("{one} is"),
        _ => format!("{} are", names.join(" and ")),
    }
}

// ───────────────────────────────── options ───────────────────────────────

/// The options the *build* reads: a wrong value there changes what the
/// catalog carries, silently. Everything else is the runtime's Bad Option.
fn options(at: &mut At<'_, '_>, model: &Message<'_>) {
    let mut check = |function: Option<&FunctionRef<'_>>| {
        let Some(function) = function else { return };
        for (name, value) in function.options.iter() {
            if defines_option(&function.name, name) == Some(false) {
                at.say(
                    Lint::UnknownOption,
                    at.offset_of(name),
                    format!(
                        ":{} has no option {name}; it is ignored, and the message \
                         formats as if it were not there",
                        function.name
                    ),
                );
            }
            let literal = match value {
                OptionValue::Literal(literal) => Some(literal.value.as_ref()),
                _ => None,
            };
            match (name, literal) {
                ("select", Some(v)) if !matches!(v, "plural" | "ordinal" | "exact") => at.say(
                    Lint::BadOptionValue,
                    at.offset_of(v),
                    format!("select={v}: expected plural, ordinal or exact"),
                ),
                ("select", None) => at.say(
                    Lint::DynamicSelect,
                    at.offset_of(name.as_ref()),
                    "select must be a literal: the build decides from it which \
                     plural rules a catalog carries",
                ),
                ("currency", Some(v)) if !is_currency_code(v) => at.say(
                    Lint::BadOptionValue,
                    at.offset_of(v),
                    format!("currency={v}: expected a three-letter ISO 4217 code"),
                ),
                ("unit", Some(v)) if !is_unit_id(v) => at.say(
                    Lint::BadOptionValue,
                    at.offset_of(v),
                    format!("unit={v}: not a CLDR unit identifier"),
                ),
                (
                    "minimumIntegerDigits"
                    | "minimumFractionDigits"
                    | "maximumFractionDigits"
                    | "minimumSignificantDigits"
                    | "maximumSignificantDigits",
                    Some(v),
                ) if v.parse::<u32>().is_err() => at.say(
                    Lint::BadOptionValue,
                    at.offset_of(v),
                    format!("{name}={v}: expected a whole number"),
                ),
                _ => {}
            }
        }
    };
    for declaration in model.declarations() {
        match declaration {
            Declaration::Input(input) => check(input.value.function.as_ref()),
            Declaration::Local(local) => check(local.value.function()),
            _ => {}
        }
    }
    for pattern in patterns(model) {
        for part in pattern {
            if let PatternPart::Expression(e) = part {
                check(e.function());
            }
        }
    }
}

fn is_currency_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_unit_id(id: &str) -> bool {
    mf2_locale_data::unit_ids().is_ok_and(|ids| ids.contains(&id))
        || mf2_locale_data::composition(id).is_ok_and(|c| c.is_some())
}

// ───────────────────────────────── markup ────────────────────────────────

/// Markup opened and not closed, or closed and not opened, in one pattern.
fn markup(at: &mut At<'_, '_>, model: &Message<'_>, analysis: &Analysis<'_>) {
    for pattern in patterns(model) {
        let mut open: Vec<&str> = Vec::new();
        let mut unmatched_close: Vec<&str> = Vec::new();
        for part in pattern {
            let PatternPart::Markup(m) = part else {
                continue;
            };
            match m.kind {
                mf2_model::MarkupKind::Open => open.push(m.name.as_ref()),
                mf2_model::MarkupKind::Close => {
                    if let Some(i) = open.iter().rposition(|n| *n == m.name.as_ref()) {
                        open.remove(i);
                    } else {
                        unmatched_close.push(m.name.as_ref());
                    }
                }
                // Standalone markup pairs with nothing by design.
                mf2_model::MarkupKind::Standalone => {}
            }
        }
        for name in open {
            at.say(
                Lint::UnpairedMarkup,
                at.offset_of(name),
                format!("{{#{name}}} is opened and never closed"),
            );
        }
        for name in unmatched_close {
            at.say(
                Lint::UnpairedMarkup,
                at.offset_of(name),
                format!("{{/{name}}} closes markup that was never opened"),
            );
        }
    }
    let _ = analysis;
}

// ────────────────────────────────── bidi ─────────────────────────────────

/// Unpaired isolates in literal text: a stray U+2066..U+2068 without its
/// U+2069 leaks its direction into whatever follows the message.
fn bidi(at: &mut At<'_, '_>, model: &Message<'_>) {
    for pattern in patterns(model) {
        let mut depth = 0i32;
        let mut text = None;
        for part in pattern {
            let PatternPart::Text(run) = part else {
                continue;
            };
            text.get_or_insert(run.as_ref());
            for c in run.chars() {
                match c {
                    '\u{2066}'..='\u{2068}' => depth += 1,
                    '\u{2069}' => depth -= 1,
                    _ => {}
                }
            }
        }
        if depth != 0 {
            let offset = text.map_or(0, |t| at.offset_of(t));
            at.say(
                Lint::SuspiciousBidi,
                offset,
                if depth > 0 {
                    "an isolate is opened and never closed (U+2069 missing)"
                } else {
                    "an isolate is closed that was never opened"
                },
            );
        }
    }
}

/// Text that is not in Unicode Normalization Form C. Selection compares keys
/// under NFC and the catalog encodes names under NFC, so unnormalized text
/// formats as itself but never matches what a translator typed.
fn normalization(at: &mut At<'_, '_>, model: &Message<'_>) {
    for pattern in patterns(model) {
        for part in pattern {
            let PatternPart::Text(text) = part else {
                continue;
            };
            if non_nfc(text) {
                at.say(
                    Lint::NonNfcSource,
                    at.offset_of(text),
                    "this text is not in Unicode Normalization Form C",
                );
                return;
            }
        }
    }
}

// ────────────────────────────────── names ────────────────────────────────

/// Names chosen by users that `syntax.md` asks a linter to warn on: not an
/// identifier under the Unicode Default Identifier Syntax (UAX #31), or not
/// allowed by the General Security Profile (UTS #39) — a character outside
/// `Identifier_Status=Allowed`, or scripts mixed in one name.
///
/// MF2's own names are the profile: `-` and `.` continue a name and `_`
/// starts one (UAX #31 permits both kinds of addition), and a namespace's
/// `:` separates two names, each checked alone. Names compare under NFC, so
/// the NFC form is what is checked; a name is reported once per message.
fn names(at: &mut At<'_, '_>, model: &Message<'_>, analysis: &Analysis<'_>) {
    let mut found: Vec<(&'static str, &str)> = Vec::new();
    for name in analysis.externals.iter().chain(&analysis.locals) {
        found.push(("variable", &name.nfc));
    }
    for name in &analysis.functions {
        found.push(("function", &name.nfc));
    }
    for name in &analysis.markup {
        found.push(("markup", &name.nfc));
    }
    for declaration in model.declarations() {
        match declaration {
            Declaration::Input(input) => add(
                &mut found,
                input.value.function.as_ref().map(|f| &f.options),
                &input.value.attributes,
            ),
            Declaration::Local(local) => add(
                &mut found,
                local.value.function().map(|f| &f.options),
                local.value.attributes(),
            ),
            _ => {}
        }
    }
    for pattern in patterns(model) {
        for part in pattern {
            match part {
                PatternPart::Expression(e) => {
                    add(&mut found, e.function().map(|f| &f.options), e.attributes());
                }
                PatternPart::Markup(m) => add(&mut found, Some(&m.options), &m.attributes),
                _ => {}
            }
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (kind, name) in found {
        let name = nfc(name);
        let Some(why) = name.split(':').find_map(nonstandard) else {
            continue;
        };
        if seen.insert(name.to_string()) {
            at.say(
                Lint::NonstandardName,
                at.offset_of(&name),
                format!("the {kind} name `{name}` {why}"),
            );
        }
    }
}

/// Adds the option and attribute names of one expression or markup.
fn add<'m>(
    found: &mut Vec<(&'static str, &'m str)>,
    options: Option<&'m Options<'_>>,
    attributes: &'m Attributes<'_>,
) {
    for (name, _) in options.into_iter().flat_map(Options::iter) {
        found.push(("option", name));
    }
    for (name, _) in attributes.iter() {
        found.push(("attribute", name));
    }
}

/// Why `name` (one side of a namespace) is not a standard identifier, or
/// `None` when it is one.
fn nonstandard(name: &str) -> Option<&'static str> {
    use unicode_security::{GeneralSecurityProfile, MixedScript};
    let mut chars = name.chars();
    let start = chars.next()?;
    let identifier = (unicode_ident::is_xid_start(start) || start == '_')
        && chars.all(|c| unicode_ident::is_xid_continue(c) || c == '-' || c == '.');
    if !identifier {
        Some("is not a Unicode identifier (UAX #31)")
    } else if !name.chars().all(GeneralSecurityProfile::identifier_allowed) {
        Some("uses a character the General Security Profile does not allow (UTS #39)")
    } else if !name.is_single_script() {
        Some("mixes scripts (UTS #39)")
    } else {
        None
    }
}

/// `name` in NFC, borrowed when it already is.
fn nfc(name: &str) -> std::borrow::Cow<'_, str> {
    use unicode_normalization::UnicodeNormalization;
    if non_nfc(name) {
        std::borrow::Cow::Owned(name.nfc().collect())
    } else {
        std::borrow::Cow::Borrowed(name)
    }
}

// ────────────────────────────── plural categories ────────────────────────

/// A plural `.match` that does not mention every category the target locale
/// has: those numbers all land on the catch-all.
fn plural_categories(at: &mut At<'_, '_>, model: &Message<'_>, locale: &str) {
    let Message::Select(select) = model else {
        return;
    };
    let kinds = plural_kinds_used(select);
    if kinds.is_empty() {
        return;
    }
    let mut keys: BTreeSet<&str> = BTreeSet::new();
    for variant in &select.variants {
        for key in &variant.keys {
            if let Key::Literal(literal) = key {
                keys.insert(literal.value.as_ref());
            }
        }
    }
    for kind in kinds {
        let Ok(rules) = plural_rules(kind, locale) else {
            continue;
        };
        let missing: Vec<&str> = rules
            .rules
            .iter()
            .map(|r| r.category.as_str())
            .filter(|c| *c != "other" && !keys.contains(c))
            .collect();
        if !missing.is_empty() {
            at.say(
                Lint::MissingPluralCategory,
                0,
                format!(
                    "{locale} has the {} categor{} {}, which no variant names; \
                     they all fall to the catch-all",
                    if kind == PluralKind::Ordinal {
                        "ordinal"
                    } else {
                        "plural"
                    },
                    if missing.len() == 1 { "y" } else { "ies" },
                    missing.join(", ")
                ),
            );
        }
    }
}

/// Which plural rule sets a select message's selectors use, as the slicer
/// works them out — one implementation, so a lint and a catalog can never
/// disagree about which rules a locale needs.
fn plural_kinds_used(select: &mf2_model::SelectMessage<'_>) -> Vec<PluralKind> {
    let (cardinal, ordinal) = crate::slice::selector_kinds(select);
    let mut kinds = Vec::new();
    if cardinal {
        kinds.push(PluralKind::Cardinal);
    }
    if ordinal {
        kinds.push(PluralKind::Ordinal);
    }
    kinds
}

// ────────────────────────── a translation and its source ─────────────────

/// What a translation may and may not do with the source message's inputs.
fn against_source(
    at: &mut At<'_, '_>,
    corpus: &Corpus<'_>,
    model: &Message<'_>,
    analysis: &Analysis<'_>,
) {
    let id = at.record.id.as_str();
    let source_locale = &corpus.sources[corpus.source_index];
    let Some(&record) = corpus.indexes[corpus.source_index].get(id) else {
        // An id the source lacks is reported once, where the manifest is
        // built; nothing to compare against here.
        return;
    };
    let Some(source_model) = corpus.models[corpus.source_index]
        .get(record)
        .and_then(Option::as_ref)
    else {
        return;
    };
    let source_analysis = mf2_syntax::analyze(source_model);
    let source_vars: BTreeSet<&str> = source_analysis
        .externals
        .iter()
        .map(|n| n.nfc.as_ref())
        .collect();
    let source_markup: BTreeSet<&str> = source_analysis
        .markup
        .iter()
        .map(|n| n.nfc.as_ref())
        .collect();

    for name in &analysis.externals {
        if !source_vars.contains(name.nfc.as_ref()) {
            at.say(
                Lint::UndeclaredVariable,
                at.offset_of(name.spelling),
                format!(
                    "${} is not an input of the source message; if this language \
                     needs it, the source has to declare it with .input",
                    name.nfc
                ),
            );
        }
    }
    for name in &analysis.markup {
        if !source_markup.contains(name.nfc.as_ref()) {
            at.say(
                Lint::UndeclaredMarkup,
                at.offset_of(name.spelling),
                format!("{{#{}}} is not in the source message", name.nfc),
            );
        }
    }
    let here: BTreeSet<&str> = analysis.externals.iter().map(|n| n.nfc.as_ref()).collect();
    let dropped: Vec<&str> = source_vars.difference(&here).copied().collect();
    if !dropped.is_empty() {
        at.say(
            Lint::DroppedPlaceholder,
            0,
            format!(
                "the source message shows {}, which this translation does not",
                dropped
                    .iter()
                    .map(|v| format!("${v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    }
    // By name, over the whole message: a variant may leave markup out as
    // long as the translation keeps it somewhere, as a placeholder may be.
    let here: BTreeSet<&str> = analysis.markup.iter().map(|n| n.nfc.as_ref()).collect();
    let dropped: Vec<&str> = source_markup.difference(&here).copied().collect();
    if !dropped.is_empty() {
        at.say(
            Lint::DroppedMarkup,
            0,
            format!(
                "the source message has {}, which this translation leaves out, so this \
                 language loses what it marks (a link, a style)",
                dropped
                    .iter()
                    .map(|m| format!("{{#{m}}}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    }
    dates_against_source(at, &source_locale.tag, model, source_model);
    let source_record = &source_locale.loaded.records[record];
    if source_record.do_not_translate() && source_record.source != at.record.source {
        at.say(
            Lint::DoNotTranslate,
            0,
            format!(
                "this message is marked @do-not-translate in {}, but differs here",
                source_locale.tag
            ),
        );
    }
    let _ = model;
}

/// `date-mismatch`: a variable this translation formats with a date
/// function and the source shows bare, or the other way round (`plan/08`
/// §4.3).
fn dates_against_source(
    at: &mut At<'_, '_>,
    source_tag: &str,
    model: &Message<'_>,
    source_model: &Message<'_>,
) {
    let here = DateUse::of(model);
    let source = DateUse::of(source_model);
    for name in here.dated.intersection(&source.bare) {
        at.say(
            Lint::DateMismatch,
            0,
            format!(
                "${name} is formatted as a date here, but the source message ({source_tag}) \
                 shows it bare, where a date is a Bad Operand and shows {{${name}}}: write \
                 {{${name} :datetime}} there too"
            ),
        );
    }
    for name in here.bare.intersection(&source.dated) {
        at.say(
            Lint::DateMismatch,
            0,
            format!(
                "${name} is formatted as a date in the source message ({source_tag}), but \
                 shown bare here, where a date is a Bad Operand and shows {{${name}}}: write \
                 {{${name} :datetime}}"
            ),
        );
    }
}

/// The built-in date functions: what `date-mismatch` counts as formatting a
/// date.
fn is_date_function(name: &str) -> bool {
    matches!(name, "datetime" | "date" | "time")
}

/// How one message shows its variables as dates, by NFC name: those an
/// expression formats with a date function (in a placeholder or a
/// declaration), and those a placeholder shows bare — with no function, and
/// none declared on the way, so that a date there is a Bad Operand.
#[derive(Debug, Default)]
struct DateUse {
    dated: BTreeSet<String>,
    bare: BTreeSet<String>,
}

impl DateUse {
    fn of(message: &Message<'_>) -> DateUse {
        let declarations = message.declarations();
        let mut out = DateUse::default();
        // The root variable a date-function expression formats, if any.
        let date_call = |e: &Expression<'_>| -> Option<String> {
            if let (Some(f), Expression::Variable(v)) = (e.function(), e)
                && is_date_function(f.name.as_ref())
                && let Some(root) = root_variable(declarations, v.arg.name.as_ref(), 0)
            {
                return Some(nfc(root).into_owned());
            }
            None
        };
        for d in declarations {
            match d {
                Declaration::Input(i) => {
                    if i.value
                        .function
                        .as_ref()
                        .is_some_and(|f| is_date_function(f.name.as_ref()))
                    {
                        out.dated
                            .insert(nfc(i.value.arg.name.as_ref()).into_owned());
                    }
                }
                Declaration::Local(l) => out.dated.extend(date_call(&l.value)),
                _ => {}
            }
        }
        let mut bare = Vec::new();
        for pattern in patterns(message) {
            for part in pattern {
                if let PatternPart::Expression(e) = part {
                    out.dated.extend(date_call(e));
                    if let (None, Expression::Variable(v)) = (e.function(), e) {
                        let name = v.arg.name.as_ref();
                        if crate::slice::declared_function(declarations, name, 0).is_none()
                            && let Some(root) = root_variable(declarations, name, 0)
                        {
                            bare.push(nfc(root).into_owned());
                        }
                    }
                }
            }
        }
        out.bare.extend(bare);
        out
    }
}

/// The variable `$name` stands for: itself when it is an input or an
/// argument, the variable a `.local $name = {$other}` binds (followed), or
/// `None` when a `.local` binds a literal or a function's result.
fn root_variable<'a>(
    declarations: &'a [Declaration<'_>],
    name: &'a str,
    depth: u32,
) -> Option<&'a str> {
    if depth > 16 {
        return None;
    }
    let local = declarations.iter().rev().find_map(|d| match d {
        Declaration::Local(l) if l.name == name => Some(l),
        _ => None,
    });
    match local {
        None => Some(name),
        Some(l) => match &l.value {
            Expression::Variable(v) if v.function.is_none() => {
                root_variable(declarations, v.arg.name.as_ref(), depth + 1)
            }
            _ => None,
        },
    }
}

// ───────────────────────────────── coverage ──────────────────────────────

/// What a locale has of the messages that need translating: the source's
/// messages but those marked `@do-not-translate` (a brand, a language's own
/// name), which need none — a locale without them is not missing anything,
/// and a copy of one is not a translation (Phase 10 E2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    /// The locale.
    pub tag: String,
    /// How many of the corpus's messages need translating.
    pub translatable: usize,
    /// Those this locale does not have, in the manifest's order.
    pub missing: Vec<String>,
}

impl Coverage {
    /// How many of the messages that need translating the locale has.
    pub fn translated(&self) -> usize {
        self.translatable - self.missing.len()
    }
}

/// The coverage of `locale`: of the source locale, every message it needs.
pub fn coverage_of(corpus: &Corpus<'_>, locale: usize) -> Coverage {
    let source = &corpus.sources[corpus.source_index];
    let needs_translating = |id: &&String| {
        corpus.indexes[corpus.source_index]
            .get(id.as_str())
            .is_none_or(|&record| !source.loaded.records[record].do_not_translate())
    };
    let ids: Vec<&String> = corpus
        .manifest
        .manifest
        .ids
        .iter()
        .filter(needs_translating)
        .collect();
    Coverage {
        tag: corpus.sources[locale].tag.clone(),
        translatable: ids.len(),
        missing: ids
            .into_iter()
            .filter(|id| !corpus.indexes[locale].contains_key(id.as_str()))
            .cloned()
            .collect(),
    }
}

/// How many of the corpus's messages a locale is missing, once per locale,
/// naming the first few (reported with counts
/// per locale).
fn coverage(sink: &mut Sink<'_>, corpus: &Corpus<'_>, locale: usize, config: &Config) {
    if locale == corpus.source_index {
        return;
    }
    let source = &corpus.sources[locale];
    let coverage = coverage_of(corpus, locale);
    let ids: Vec<&str> = coverage.missing.iter().map(String::as_str).collect();
    let missing = ids.len();
    if missing == 0 {
        return;
    }
    let level = config.level(Lint::MissingTranslation);
    if level == Level::Allow {
        return;
    }
    let total = coverage.translatable;
    let path = source
        .loaded
        .files
        .first()
        .map_or_else(|| source.path.clone(), |f| f.path.clone());
    sink.add(
        level,
        Some(Lint::MissingTranslation),
        &path,
        mf2_resource::Position { line: 1, column: 1 },
        None,
        match config.catalog.missing {
            Missing::Fallback => format!(
                "{missing} of {total} messages are missing here and fall back to {}: {}",
                config.chain(&source.tag).join(", "),
                first_ids(&ids)
            ),
            Missing::Empty => format!(
                "{missing} of {total} messages are missing here and ship empty \
                 (`[catalog] missing = \"empty\"`): {}",
                first_ids(&ids)
            ),
            Missing::Id => format!(
                "{missing} of {total} messages are missing here and ship as their id \
                 (`[catalog] missing = \"id\"`): {}",
                first_ids(&ids)
            ),
        },
    );
}

/// How many ids a report names before it counts the rest.
const IDS_SHOWN: usize = 10;

/// The first [`IDS_SHOWN`] of `ids`, then how many more there are.
fn first_ids(ids: &[&str]) -> String {
    let shown = ids.len().min(IDS_SHOWN);
    let list = ids[..shown].join(", ");
    match ids.len() - shown {
        0 => list,
        more => format!("{list}, and {more} more"),
    }
}

/// An id no `tr!` in the application's sources names
/// reported where `defined` says the source
/// locale defines it, else at `where_looked`.
///
/// A plain text scan, deliberately: a call site may build its id in a macro
/// of its own, so this errs towards saying nothing — an id that appears
/// anywhere in the sources counts as used. The ids the generated module
/// names itself (the languages' names) are the caller's to leave out.
pub fn unused_ids(
    ids: &[String],
    sources: &str,
    locale: &str,
    defined: &BTreeMap<String, (std::path::PathBuf, mf2_resource::Position)>,
    where_looked: &std::path::Path,
    config: &Config,
    report: &mut Report,
) {
    let level = config.level(Lint::UnusedId);
    if level == Level::Allow {
        return;
    }
    let mut unused: Vec<&str> = ids
        .iter()
        .filter(|id| !sources.contains(id.as_str()))
        .map(String::as_str)
        .collect();
    unused.sort_unstable();
    let mut sink = Sink::new(report, locale);
    for id in unused {
        let (file, at) = defined.get(id).map_or(
            (where_looked, mf2_resource::Position { line: 1, column: 1 }),
            |(file, at)| (file.as_path(), *at),
        );
        sink.add(
            level,
            Some(Lint::UnusedId),
            file,
            at,
            Some(id),
            "no source file names this id".to_owned(),
        );
    }
}

fn patterns<'m>(message: &'m Message<'_>) -> Vec<&'m Pattern<'m>> {
    match message {
        Message::Pattern(p) => vec![&p.pattern],
        Message::Select(s) => s.variants.iter().map(|v| &v.value).collect(),
        _ => Vec::new(),
    }
}

/// Whether `source` is not in Unicode Normalization Form C.
pub fn non_nfc(source: &str) -> bool {
    use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};
    match is_nfc_quick(source.chars()) {
        IsNormalized::Yes => false,
        IsNormalized::No => true,
        IsNormalized::Maybe => source.nfc().ne(source.chars()),
    }
}

#[cfg(test)]
mod tests {
    use super::nonstandard;

    #[test]
    fn mf2_names_are_standard_under_the_profile() {
        for name in [
            "name",
            "_x",
            "a-b.c",
            "x1",
            "número",
            "名前",
            "ひらがな漢字",
            "имя",
        ] {
            assert_eq!(nonstandard(name), None, "{name}");
        }
    }

    #[test]
    fn each_reason_is_reported() {
        // Valid MF2 name-start, not XID_Start: U+2140 DOUBLE-STRUCK N-ARY
        // SUMMATION.
        assert_eq!(
            nonstandard("\u{2140}x"),
            Some("is not a Unicode identifier (UAX #31)")
        );
        // XID_Start, but Identifier_Status=Restricted: U+01C5 LATIN CAPITAL
        // LETTER D WITH SMALL LETTER Z WITH CARON (not NFKC-stable).
        assert_eq!(
            nonstandard("\u{1c5}x"),
            Some("uses a character the General Security Profile does not allow (UTS #39)")
        );
        // Latin with U+0430 CYRILLIC SMALL LETTER A.
        assert_eq!(nonstandard("n\u{430}me"), Some("mixes scripts (UTS #39)"));
    }
}
