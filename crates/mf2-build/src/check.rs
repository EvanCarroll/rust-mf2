//! `mf2 check` (`plans/05-tooling.md` §5): every lint, on the parsed corpus.
//!
//! The checks run on models, never on text, so a lint says what is wrong with
//! the *message* — and every one of them is placed in the file it came from,
//! through the record's value map.

use std::collections::{BTreeMap, BTreeSet};

use mf2_locale_data::plural::{PluralKind, plural_rules};
use mf2_model::{
    Attributes, Declaration, FunctionRef, Key, Message, OptionValue, Options, Pattern, PatternPart,
};
use mf2_syntax::Analysis;

use crate::config::{Config, Missing};
use crate::corpus::LocaleSource;
use crate::features::{Features, defines_option};
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

/// Runs every lint over the corpus.
pub fn corpus(corpus: &Corpus<'_>, config: &Config, features: &Features, report: &mut Report) {
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
                let feature = features
                    .missing_feature(&name.nfc)
                    .unwrap_or("the right feature");
                at.say(
                    Lint::GatedFunction,
                    offset,
                    format!(
                        ":{} needs the `{feature}` feature, which this build does \
                         not have; a message may never add formatting code to the \
                         wasm by itself",
                        name.nfc
                    ),
                );
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

/// What a translation may and may not do with the source message's inputs
/// (`plans/05-tooling.md` §3).
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

// ───────────────────────────────── coverage ──────────────────────────────

/// How many of the corpus's messages a locale is missing, once per locale,
/// naming the first few (`plans/05-tooling.md` §5: "reported with counts
/// per locale").
fn coverage(sink: &mut Sink<'_>, corpus: &Corpus<'_>, locale: usize, config: &Config) {
    if locale == corpus.source_index {
        return;
    }
    let source = &corpus.sources[locale];
    let ids: Vec<&str> = corpus
        .manifest
        .manifest
        .ids
        .iter()
        .filter(|id| !corpus.indexes[locale].contains_key(id.as_str()))
        .map(String::as_str)
        .collect();
    let missing = ids.len();
    if missing == 0 {
        return;
    }
    let level = config.level(Lint::MissingTranslation);
    if level == Level::Allow {
        return;
    }
    let total = corpus.manifest.manifest.ids.len();
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
/// (`plans/05-tooling.md` §5).
///
/// A plain text scan, deliberately: a call site may build its id in a macro
/// of its own, so this errs towards saying nothing — an id that appears
/// anywhere in the sources counts as used.
pub fn unused_ids(
    ids: &[String],
    sources: &str,
    locale: &str,
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
    if unused.is_empty() {
        return;
    }
    let mut sink = Sink::new(report, locale);
    sink.add(
        level,
        Some(Lint::UnusedId),
        where_looked,
        mf2_resource::Position { line: 1, column: 1 },
        None,
        format!(
            "{} id(s) no source file names: {}",
            unused.len(),
            first_ids(&unused)
        ),
    );
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
