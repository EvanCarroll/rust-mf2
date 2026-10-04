//! The feature list `mf2 check` prints (`plan/01-size-and-features.md` §5):
//! the function and data features the corpus needs, those that are on, those
//! on and unused, and the features to write on the `mf2` dependency, with the
//! modes (`ssr`, `ratatui`, …) left as they are. There is no default set:
//! this list stands in for one.

use std::fmt::Write as _;

use mf2_build::check::Needs;
use mf2_build::{DateFormatter, Features, Side};
use serde_json::{Value, json};

/// The number family: the formatter, and the browser's `Intl` path for it.
const NUMBERS: [&str; 2] = ["fn-number", "number-intl"];
/// The date functions, then the date formatters: each side's families
/// (`plan/08` §3.1), the framework-free one first.
const DATES: [&str; 15] = [
    "datetime",
    "host-std-datetime-iso",
    "host-std-datetime-icu",
    "host-web-datetime-iso",
    "host-web-datetime-intl",
    "host-web-datetime-icu",
    "leptos-client-datetime-iso",
    "leptos-client-datetime-intl",
    "leptos-client-datetime-icu",
    "leptos-server-datetime-iso",
    "leptos-server-datetime-icu",
    "axum-datetime-iso",
    "axum-datetime-icu",
    "native-datetime-iso",
    "native-datetime-icu",
];

/// Where the features that are on came from.
#[derive(Debug)]
pub(crate) enum Source {
    /// `--features`: modes are the other names it gives.
    Given,
    /// `cargo metadata`; the modes are those the crate writes on `mf2`.
    Cargo { written: Vec<String> },
    /// Nobody could say: the list is the corpus's needs only.
    Unknown,
}

/// The list, worked out.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FeatureList {
    /// The families the corpus needs, as their formatter features.
    needs: Vec<&'static str>,
    /// The function and data features that are on; `None` when unknown.
    on: Option<Vec<&'static str>>,
    /// Those of `on` whose family no message uses.
    unused: Option<Vec<&'static str>>,
    /// The features to write on the `mf2` dependency.
    line: Vec<String>,
    /// The browser formats dates with `Intl` and native code with ICU4X, so
    /// a server-rendered date can read differently once hydrated.
    both_backends: bool,
    /// Where `on` came from.
    source: &'static str,
}

impl FeatureList {
    pub(crate) fn new(needs: Needs, features: &Features, source: &Source) -> FeatureList {
        let family = |names: &[&'static str], used: bool| -> Vec<&'static str> {
            names
                .iter()
                .copied()
                .filter(|name| features.has(name) && !used)
                .collect()
        };
        let mut needed = Vec::new();
        if needs.numbers {
            needed.push("fn-number");
        }
        if needs.dates {
            needed.push("datetime");
        }
        let known = !matches!(source, Source::Unknown);
        let on: Vec<&'static str> = NUMBERS
            .iter()
            .chain(DATES.iter())
            .copied()
            .filter(|name| features.has(name))
            .collect();
        let mut unused = family(&NUMBERS, needs.numbers);
        unused.extend(family(&DATES, needs.dates));

        // The modes as they are, then each needed family with what of it is on.
        let is_family = |name: &str| NUMBERS.contains(&name) || DATES.contains(&name);
        let written: Vec<String> = match source {
            Source::Cargo { written } => written.clone(),
            Source::Given => features.names().map(str::to_owned).collect(),
            Source::Unknown => Vec::new(),
        };
        let mut line = written.clone();
        line.retain(|name| !is_family(name));
        if needs.numbers {
            line.push("fn-number".to_owned());
            if known && features.number_intl() {
                line.push("number-intl".to_owned());
            }
        }
        if needs.dates {
            // The formatters as the crate writes them, not every name they
            // imply.
            let formatters: Vec<String> = written
                .iter()
                .filter(|name| DATES[1..].contains(&name.as_str()))
                .cloned()
                .collect();
            if formatters.is_empty() {
                line.push("datetime".to_owned());
            }
            line.extend(formatters);
        }
        FeatureList {
            needs: needed,
            on: known.then_some(on),
            unused: known.then_some(unused),
            line,
            both_backends: known
                && features.date_formatter(Side::Browser) == Some(DateFormatter::Intl)
                && features.date_formatter(Side::Native) == Some(DateFormatter::Icu),
            source: match source {
                Source::Given => "given",
                Source::Cargo { .. } => "cargo",
                Source::Unknown => "unknown",
            },
        }
    }

    /// The block of the text report.
    pub(crate) fn to_text(&self) -> String {
        let list = |names: &[&str]| {
            if names.is_empty() {
                "none".to_owned()
            } else {
                names.join(", ")
            }
        };
        let mut out = String::new();
        let from = match self.source {
            "given" => "as --features names them",
            "cargo" => "as cargo resolves them for this crate",
            _ => "the corpus's needs only: cargo could not say which are on",
        };
        let _ = writeln!(out, "mf2 features ({from}):");
        let _ = writeln!(out, "  the corpus needs: {}", list(&self.needs));
        if let (Some(on), Some(unused)) = (&self.on, &self.unused) {
            let _ = writeln!(out, "  on:               {}", list(on));
            let _ = writeln!(out, "  on and unused:    {}", list(unused));
        }
        let quoted: Vec<String> = self.line.iter().map(|f| format!("\"{f}\"")).collect();
        let _ = writeln!(
            out,
            "  write:            mf2 = {{ ..., features = [{}] }}",
            quoted.join(", ")
        );
        if self.both_backends {
            let _ = writeln!(
                out,
                "  dates:            in the browser `intl` formats \
                 (Intl.DateTimeFormat, no ICU4X date code or data in the client), \
                 and on the server and natively `icu` does (ICU4X over the \
                 catalog's icu.blob), so a server-rendered date can read \
                 differently once hydrated"
            );
        }
        out
    }

    /// The `features` field of the JSON report.
    pub(crate) fn to_json(&self) -> Value {
        let backends = self
            .both_backends
            .then(|| json!({ "wasm32-unknown-unknown": "intl", "other": "icu" }));
        json!({
            "source": self.source,
            "needs": self.needs,
            "on": self.on,
            "unused": self.unused,
            "line": self.line,
            "date_backend": backends,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FeatureList, Needs, Source};
    use mf2_build::{DateFormatter, Features, Side};

    fn list(needs: (bool, bool), on: &[&str], source: &Source) -> FeatureList {
        let needs = Needs {
            numbers: needs.0,
            dates: needs.1,
        };
        FeatureList::new(needs, &Features::from_names(on.iter().copied()), source)
    }

    fn cargo(written: &[&str]) -> Source {
        Source::Cargo {
            written: written.iter().map(|&w| w.to_owned()).collect(),
        }
    }

    #[test]
    fn a_terminal_application_keeps_its_mode_and_drops_what_it_does_not_use() {
        let on = [
            "fn-number",
            "datetime",
            "native-datetime-iso",
            "ratatui",
            "native",
            "host-std",
        ];
        let written = cargo(&["ratatui", "fn-number", "native-datetime-iso"]);
        let list = list((true, false), &on, &written);
        assert_eq!(list.needs, ["fn-number"]);
        assert_eq!(
            list.on.as_deref(),
            Some(&["fn-number", "datetime", "native-datetime-iso"][..])
        );
        assert_eq!(
            list.unused.as_deref(),
            Some(&["datetime", "native-datetime-iso"][..])
        );
        assert_eq!(list.line, ["ratatui", "fn-number"]);
        assert!(
            list.to_text()
                .contains(r#"features = ["ratatui", "fn-number"]"#)
        );
        assert!(!list.to_text().contains("dates:"));
    }

    #[test]
    fn a_needed_family_keeps_its_backend_and_names_the_one_that_wins() {
        let on = [
            "fn-number",
            "datetime",
            "leptos-client-datetime-intl",
            "leptos-server-datetime-icu",
        ];
        let written = cargo(&[
            "fn-number",
            "leptos-client-datetime-intl",
            "leptos-server-datetime-icu",
        ]);
        let list = list((true, true), &on, &written);
        assert_eq!(list.unused.as_deref(), Some(&[][..]));
        assert_eq!(
            list.line,
            [
                "fn-number",
                "leptos-client-datetime-intl",
                "leptos-server-datetime-icu"
            ]
        );
        assert!(list.to_text().contains("in the browser `intl` formats"));
        assert_eq!(
            list.to_json()["date_backend"]["wasm32-unknown-unknown"],
            "intl"
        );
        assert_eq!(list.to_json()["date_backend"]["other"], "icu");
    }

    #[test]
    fn a_missing_family_is_added_and_number_intl_stays_with_numbers() {
        let list = list((true, true), &["number-intl", "csr"], &Source::Given);
        assert_eq!(list.needs, ["fn-number", "datetime"]);
        assert_eq!(list.line, ["csr", "fn-number", "number-intl", "datetime"]);
    }

    #[test]
    fn without_cargo_the_list_is_the_needs_only() {
        let on = ["fn-number", "datetime"];
        let list = list((false, true), &on, &Source::Unknown);
        assert_eq!(list.on, None);
        assert_eq!(list.line, ["datetime"]);
        let text = list.to_text();
        assert!(text.contains("the corpus's needs only") && !text.contains("on and unused"));
        assert!(list.to_json()["on"].is_null());
    }
}
