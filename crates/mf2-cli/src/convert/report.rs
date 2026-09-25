//! What a conversion says: one finding per construct it could not map, or
//! mapped but wants a person to look at, each under a stable code
//! (`plans/05-tooling.md` §6.1, "The codes").

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use mf2_build::Level;

/// A code of §6.1. A code's meaning never changes; a retired one is not
/// reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Code {
    Junk,
    MissingReference,
    CyclicReference,
    NumberOperand,
    CurrencyMissing,
    DatetimeOption,
    DateSelector,
    UnknownFunction,
    MixedKeys,
    VariantLimit,
    FileCollision,
    DuplicateId,
    Locale,
    UnboundTermVariable,
    TermPositional,
    NumberOption,
    UnreachableVariant,
    DatetimeApproximate,
    // `--from leptos-fluent`'s call sites (§6.2): every one an error.
    LfInitializer,
    LfContext,
    LfImport,
    LfDynamicId,
    LfIfForm,
    LfCfg,
    LfArgumentName,
    LfCall,
    LfParse,
    LfDependency,
    LfUnknownId,
    LfArguments,
}

impl Code {
    /// Every code, errors first.
    #[cfg(test)]
    pub(crate) const ALL: [Code; 30] = [
        Code::Junk,
        Code::MissingReference,
        Code::CyclicReference,
        Code::NumberOperand,
        Code::CurrencyMissing,
        Code::DatetimeOption,
        Code::DateSelector,
        Code::UnknownFunction,
        Code::MixedKeys,
        Code::VariantLimit,
        Code::FileCollision,
        Code::DuplicateId,
        Code::Locale,
        Code::UnboundTermVariable,
        Code::TermPositional,
        Code::NumberOption,
        Code::UnreachableVariant,
        Code::DatetimeApproximate,
        Code::LfInitializer,
        Code::LfContext,
        Code::LfImport,
        Code::LfDynamicId,
        Code::LfIfForm,
        Code::LfCfg,
        Code::LfArgumentName,
        Code::LfCall,
        Code::LfParse,
        Code::LfDependency,
        Code::LfUnknownId,
        Code::LfArguments,
    ];

    /// The code as the report writes it.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Code::Junk => "fluent-junk",
            Code::MissingReference => "fluent-missing-reference",
            Code::CyclicReference => "fluent-cyclic-reference",
            Code::NumberOperand => "fluent-number-operand",
            Code::CurrencyMissing => "fluent-currency-missing",
            Code::DatetimeOption => "fluent-datetime-option",
            Code::DateSelector => "fluent-date-selector",
            Code::UnknownFunction => "fluent-unknown-function",
            Code::MixedKeys => "fluent-mixed-keys",
            Code::VariantLimit => "fluent-variant-limit",
            Code::FileCollision => "fluent-file-collision",
            Code::DuplicateId => "fluent-duplicate-id",
            Code::Locale => "fluent-locale",
            Code::UnboundTermVariable => "fluent-unbound-term-variable",
            Code::TermPositional => "fluent-term-positional",
            Code::NumberOption => "fluent-number-option",
            Code::UnreachableVariant => "fluent-unreachable-variant",
            Code::DatetimeApproximate => "fluent-datetime-approximate",
            Code::LfInitializer => "leptos-fluent-initializer",
            Code::LfContext => "leptos-fluent-context",
            Code::LfImport => "leptos-fluent-import",
            Code::LfDynamicId => "leptos-fluent-dynamic-id",
            Code::LfIfForm => "leptos-fluent-if-form",
            Code::LfCfg => "leptos-fluent-cfg",
            Code::LfArgumentName => "leptos-fluent-argument-name",
            Code::LfCall => "leptos-fluent-call",
            Code::LfParse => "leptos-fluent-parse",
            Code::LfDependency => "leptos-fluent-dependency",
            Code::LfUnknownId => "leptos-fluent-unknown-id",
            Code::LfArguments => "leptos-fluent-arguments",
        }
    }

    /// An error leaves the construct unconverted and fails the command; a
    /// warning is converted faithfully but wants a person to look.
    pub(crate) const fn level(self) -> Level {
        match self {
            Code::UnboundTermVariable
            | Code::TermPositional
            | Code::NumberOption
            | Code::UnreachableVariant
            | Code::DatetimeApproximate => Level::Warn,
            _ => Level::Error,
        }
    }
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) code: Code,
    pub(crate) locale: String,
    pub(crate) file: PathBuf,
    /// One-based.
    pub(crate) line: u32,
    /// One-based, in characters.
    pub(crate) column: u32,
    /// The entry being converted, where there is one.
    pub(crate) id: Option<String>,
    pub(crate) message: String,
}

/// Everything a conversion found, and what it did that a person should
/// know about.
#[derive(Debug, Default)]
pub(crate) struct Report {
    pub(crate) findings: Vec<Finding>,
    /// Per converted id, the terms and messages copied into it (§6.1, "What
    /// conversion loses by design").
    pub(crate) inlined: BTreeMap<String, BTreeSet<String>>,
    /// The MF2 functions the output calls.
    pub(crate) functions: BTreeSet<&'static str>,
    /// How many entries were written, per locale.
    pub(crate) entries: BTreeMap<String, usize>,
}

impl Report {
    /// Adds findings that are already distinct (one per construct).
    pub(crate) fn extend(&mut self, findings: impl IntoIterator<Item = Finding>) {
        self.findings.extend(findings);
    }

    /// Adds a finding, unless the same one is already there: a construct
    /// inside a term is met once per message that uses the term.
    pub(crate) fn push(&mut self, finding: Finding) {
        let seen = self.findings.iter().any(|f| {
            f.code == finding.code
                && f.locale == finding.locale
                && f.file == finding.file
                && f.line == finding.line
                && f.column == finding.column
        });
        if !seen {
            self.findings.push(finding);
        }
    }

    pub(crate) fn errors(&self) -> usize {
        self.count(Level::Error)
    }

    pub(crate) fn warnings(&self) -> usize {
        self.count(Level::Warn)
    }

    fn count(&self, level: Level) -> usize {
        self.findings
            .iter()
            .filter(|f| f.code.level() == level)
            .count()
    }

    /// The client features the output needs (§5, *gated function*).
    pub(crate) fn features(&self) -> BTreeSet<&'static str> {
        self.functions
            .iter()
            .filter_map(|f| {
                mf2_build::features::BUILTINS
                    .iter()
                    .find(|(name, _)| name == f)
                    .and_then(|(_, feature)| *feature)
            })
            .collect()
    }

    fn sorted(&self) -> Vec<&Finding> {
        let mut sorted: Vec<&Finding> = self.findings.iter().collect();
        sorted.sort_by(|a, b| {
            (&a.file, a.line, a.column, a.code).cmp(&(&b.file, b.line, b.column, b.code))
        });
        sorted
    }

    /// One line per finding, as `mf2 check` writes them, then what was
    /// inlined and what the output needs.
    pub(crate) fn to_text(&self) -> String {
        let mut out = String::new();
        for f in self.sorted() {
            let _ = write!(
                out,
                "{}:{}:{}: {}: {}",
                f.file.display(),
                f.line,
                f.column,
                f.code.level(),
                f.message
            );
            // A Rust file's findings have no locale.
            match (&f.id, f.locale.is_empty()) {
                (Some(id), false) => {
                    let _ = write!(out, " (in {id}, locale {})", f.locale);
                }
                (None, false) => {
                    let _ = write!(out, " (locale {})", f.locale);
                }
                (Some(id), true) => {
                    let _ = write!(out, " (id {id})");
                }
                (None, true) => {}
            }
            let _ = writeln!(out, " [{}]", f.code.name());
        }
        let mut by_source: BTreeMap<&str, usize> = BTreeMap::new();
        for sources in self.inlined.values() {
            for s in sources {
                *by_source.entry(s.as_str()).or_default() += 1;
            }
        }
        for (source, count) in &by_source {
            let _ = writeln!(
                out,
                "note: {source} was copied into {count} message(s); a later edit to it is an edit to each"
            );
        }
        let features = self.features();
        if !features.is_empty() {
            let _ = writeln!(
                out,
                "note: the output needs the client feature(s) {}",
                features.into_iter().collect::<Vec<_>>().join(", ")
            );
        }
        out
    }

    /// `--format json`: `mf2 check`'s shape, plus what was inlined and the
    /// features the output needs.
    pub(crate) fn to_json(&self) -> String {
        let diagnostics: Vec<serde_json::Value> = self
            .sorted()
            .into_iter()
            .map(|f| {
                serde_json::json!({
                    "level": f.code.level(),
                    "locale": f.locale,
                    "file": f.file,
                    "line": f.line,
                    "column": f.column,
                    "id": f.id,
                    "code": f.code.name(),
                    "message": f.message,
                })
            })
            .collect();
        let value = serde_json::json!({
            "diagnostics": diagnostics,
            "inlined": self.inlined,
            "features": self.features(),
            "entries": self.entries,
        });
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_owned())
    }
}
