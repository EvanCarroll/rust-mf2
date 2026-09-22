//! What a build says about a corpus: diagnostics with a file, a line and a
//! column, in one place so that `build.rs`, `mf2 check` and `--format json`
//! all say the same thing.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::lint::{Level, Lint};

/// One thing a build has to say.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Whether it fails the build.
    pub level: Level,
    /// The locale it was found in.
    pub locale: String,
    /// The file.
    pub file: PathBuf,
    /// One-based line.
    pub line: u32,
    /// One-based column, in characters.
    pub column: u32,
    /// The message's id, where there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Which lint; `None` for a syntax or data-model error, which is not a
    /// lint but the message being wrong.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lint: Option<Lint>,
    /// What is wrong.
    pub message: String,
}

impl Diagnostic {
    /// The one-line form: `file:line:column: level: message [lint]`.
    pub fn write_text(&self, out: &mut String) {
        let _ = write!(
            out,
            "{}:{}:{}: {}: {}",
            self.file.display(),
            self.line,
            self.column,
            self.level,
            self.message
        );
        if let Some(id) = &self.id {
            let _ = write!(out, " (in {id}, locale {})", self.locale);
        } else {
            let _ = write!(out, " (locale {})", self.locale);
        }
        if let Some(lint) = self.lint {
            let _ = write!(out, " [{lint}]");
        }
        out.push('\n');
    }
}

/// Everything a build found, in the order found.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    /// The diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// An empty report.
    pub fn new() -> Report {
        Report::default()
    }

    /// Adds a diagnostic, unless its level is [`Level::Allow`].
    pub fn push(&mut self, diagnostic: Diagnostic) {
        if diagnostic.level != Level::Allow {
            self.diagnostics.push(diagnostic);
        }
    }

    /// Adds everything of `other`.
    pub fn extend(&mut self, other: Report) {
        self.diagnostics.extend(other.diagnostics);
    }

    /// How many diagnostics fail the build.
    pub fn errors(&self) -> usize {
        self.count(Level::Error)
    }

    /// How many are warnings.
    pub fn warnings(&self) -> usize {
        self.count(Level::Warn)
    }

    fn count(&self, level: Level) -> usize {
        self.diagnostics.iter().filter(|d| d.level == level).count()
    }

    /// Whether the build may go on.
    pub fn is_clean(&self) -> bool {
        self.errors() == 0
    }

    /// Nothing at all to say.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// The locales that have an error.
    pub fn failing_locales(&self) -> std::collections::BTreeSet<&str> {
        self.diagnostics
            .iter()
            .filter(|d| d.level == Level::Error)
            .map(|d| d.locale.as_str())
            .collect()
    }

    /// One line per diagnostic, sorted by file and position so that two runs
    /// of a build print the same thing.
    pub fn to_text(&self) -> String {
        let mut sorted: Vec<&Diagnostic> = self.diagnostics.iter().collect();
        sorted.sort_by(|a, b| {
            (&a.file, a.line, a.column, &a.message).cmp(&(&b.file, b.line, b.column, &b.message))
        });
        let mut out = String::new();
        for d in sorted {
            d.write_text(&mut out);
        }
        out
    }

    /// `--format json`: one object per diagnostic under `"diagnostics"`.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// The cargo instructions a `build.rs` prints for its warnings.
    pub fn to_cargo_warnings(&self) -> String {
        let mut out = String::new();
        for d in &self.diagnostics {
            let mut line = String::new();
            d.write_text(&mut line);
            let _ = write!(out, "cargo::warning={}", line.trim_end());
            out.push('\n');
        }
        out
    }
}

/// Builds diagnostics for one locale, so that the caller does not repeat the
/// tag and the file on every call.
pub struct Sink<'r> {
    report: &'r mut Report,
    locale: String,
}

impl<'r> Sink<'r> {
    /// A sink that tags everything with `locale`.
    pub fn new(report: &'r mut Report, locale: impl Into<String>) -> Sink<'r> {
        Sink {
            report,
            locale: locale.into(),
        }
    }

    /// Adds one diagnostic.
    pub fn add(
        &mut self,
        level: Level,
        lint: Option<Lint>,
        file: &Path,
        at: mf2_resource::Position,
        id: Option<&str>,
        message: impl Into<String>,
    ) {
        self.report.push(Diagnostic {
            level,
            locale: self.locale.clone(),
            file: file.to_path_buf(),
            line: at.line,
            column: at.column,
            id: id.map(ToOwned::to_owned),
            lint,
            message: message.into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, Report};
    use crate::lint::{Level, Lint};
    use std::path::PathBuf;

    fn diagnostic(level: Level, line: u32, message: &str) -> Diagnostic {
        Diagnostic {
            level,
            locale: "pl".to_owned(),
            file: PathBuf::from("locales/pl/chat.mf2"),
            line,
            column: 3,
            id: Some("chat.row".to_owned()),
            lint: Some(Lint::MissingTranslation),
            message: message.to_owned(),
        }
    }

    #[test]
    fn a_report_counts_and_sorts() {
        let mut report = Report::new();
        report.push(diagnostic(Level::Warn, 9, "later"));
        report.push(diagnostic(Level::Error, 2, "earlier"));
        // An allowed lint is not a diagnostic at all.
        report.push(diagnostic(Level::Allow, 1, "quiet"));
        assert_eq!(report.errors(), 1);
        assert_eq!(report.warnings(), 1);
        assert!(!report.is_clean());
        assert_eq!(report.failing_locales(), ["pl"].into_iter().collect());
        let text = report.to_text();
        assert!(
            text.starts_with("locales/pl/chat.mf2:2:3: error: earlier (in chat.row, locale pl) [missing-translation]\n"),
            "{text}"
        );
        assert!(!text.contains("quiet"), "{text}");
    }

    #[test]
    fn json_carries_the_fields_ci_needs() {
        let mut report = Report::new();
        report.push(diagnostic(Level::Error, 2, "broken"));
        let json: serde_json::Value = serde_json::from_str(&report.to_json()).expect("json");
        let first = &json["diagnostics"][0];
        assert_eq!(first["level"], "error");
        assert_eq!(first["lint"], "missing-translation");
        assert_eq!(first["file"], "locales/pl/chat.mf2");
        assert_eq!(first["line"], 2);
        assert_eq!(first["column"], 3);
        assert_eq!(first["id"], "chat.row");
        assert_eq!(first["locale"], "pl");
    }
}
