//! The feature list `mf2 check` prints (`plan/01-size-and-features.md` §5):
//! the function and data features the corpus needs, those that are on, those
//! on and unused, and the features to write on the `mf2` dependency, with the
//! modes (`ssr`, `ratatui`, …) left as they are. There is no default set:
//! this list stands in for one.

use std::fmt::Write as _;

use mf2_build::check::Needs;
use mf2_build::{Backend, DateBackend, Features, Side, domain_features, kind_lines};
use serde_json::{Value, json};

/// The number family: the formatter, and the browser's `Intl` path for it.
const NUMBERS: [&str; 2] = ["fn-number", "number-intl"];

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

/// One side's date formatter (`plan/08` §3.2): the one in force, and the
/// features of the side that are on.
#[derive(Debug, PartialEq, Eq)]
struct SideDates {
    side: Side,
    /// The formatter the side's build formats with; `None` with none on.
    formatter: Option<DateBackend>,
    /// The side's date features as the crate writes them.
    features: Vec<String>,
}

/// The list, worked out.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FeatureList {
    /// The families the corpus needs: `fn-number`, and `dates` for a date
    /// formatter.
    needs: Vec<&'static str>,
    /// The function and data features that are on; `None` when unknown.
    on: Option<Vec<String>>,
    /// Those of `on` whose family no message uses.
    unused: Option<Vec<String>>,
    /// The features to write on the `mf2` dependency.
    line: Vec<String>,
    /// Per side, the formatter in force; `None` when unknown.
    dates: Option<Vec<SideDates>>,
    /// The corpus formats dates and the line names no formatter, because
    /// nothing says which builds the crate has: the lines to choose from,
    /// those of [`mf2_build::kind_lines`] whose framework is on, or all of
    /// them with none on. Empty otherwise.
    date_choices: Vec<(&'static str, Vec<String>)>,
    /// Where `on` came from.
    source: &'static str,
}

impl FeatureList {
    pub(crate) fn new(needs: Needs, features: &Features, source: &Source) -> FeatureList {
        // The number family, then the date functions and every family's
        // date formatters: the features this list answers for.
        let numbers: Vec<String> = NUMBERS.iter().map(|&name| name.to_owned()).collect();
        let dates = domain_features::<DateBackend>();
        let family = |names: &[String], used: bool| -> Vec<String> {
            names
                .iter()
                .filter(|name| features.has(name) && !used)
                .cloned()
                .collect()
        };
        let mut needed = Vec::new();
        if needs.numbers {
            needed.push("fn-number");
        }
        if needs.dates {
            needed.push("dates");
        }
        let known = !matches!(source, Source::Unknown);
        let on: Vec<String> = numbers
            .iter()
            .chain(&dates)
            .filter(|name| features.has(name))
            .cloned()
            .collect();
        let mut unused = family(&numbers, needs.numbers);
        unused.extend(family(&dates, needs.dates));

        // The modes as they are, then each needed family with what of it is on.
        let is_family = |name: &String| numbers.contains(name) || dates.contains(name);
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
        let mut date_choices = Vec::new();
        if needs.dates {
            // The formatters as the crate writes them, not every name they
            // imply; then, for each framework's side with none, its
            // recommended one (`plan/08` §3.5). `datetime` alone is never
            // written: it formats nothing.
            let mut formatters: Vec<String> = written
                .iter()
                .filter(|name| *name != DateBackend::DOMAIN && dates.contains(name))
                .cloned()
                .collect();
            if known {
                for feature in features.missing::<DateBackend>() {
                    if !formatters.contains(&feature) {
                        formatters.push(feature);
                    }
                }
            }
            if formatters.is_empty() {
                date_choices = if known {
                    features.lines::<DateBackend>()
                } else {
                    kind_lines::<DateBackend>()
                };
            }
            line.extend(formatters);
        }
        let dates = known.then(|| {
            Side::ALL
                .into_iter()
                .map(|side| SideDates {
                    side,
                    formatter: features.backend::<DateBackend>(side),
                    features: features.features_on::<DateBackend>(side),
                })
                .collect()
        });
        FeatureList {
            needs: needed,
            on: known.then_some(on),
            unused: known.then_some(unused),
            line,
            dates,
            date_choices,
            source: match source {
                Source::Given => "given",
                Source::Cargo { .. } => "cargo",
                Source::Unknown => "unknown",
            },
        }
    }

    /// The block of the text report.
    pub(crate) fn to_text(&self) -> String {
        fn list<S: std::borrow::Borrow<str>>(names: &[S]) -> String {
            if names.is_empty() {
                "none".to_owned()
            } else {
                names.join(", ")
            }
        }
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
        for side in self.dates.iter().flatten() {
            let label = format!("{}:", side.side.name());
            let what = match side.formatter {
                Some(formatter) => format!(
                    "`{}` formats dates ({}; {})",
                    formatter.name(),
                    side.features.join(", "),
                    formatter.cost(side.side)
                ),
                None => "no date formatter".to_owned(),
            };
            let _ = writeln!(out, "  {label:<17} {what}");
        }
        let quoted: Vec<String> = self.line.iter().map(|f| format!("\"{f}\"")).collect();
        let _ = writeln!(
            out,
            "  write:            mf2 = {{ ..., features = [{}] }}",
            quoted.join(", ")
        );
        if !self.date_choices.is_empty() {
            let _ = writeln!(out, "  and for dates, the line of the application's kind:");
            for (what, features) in &self.date_choices {
                let quoted: Vec<String> = features.iter().map(|f| format!("\"{f}\"")).collect();
                let _ = writeln!(out, "    {what}: {}", quoted.join(", "));
            }
        }
        out
    }

    /// The `features` field of the JSON report.
    pub(crate) fn to_json(&self) -> Value {
        let dates = self.dates.as_ref().map(|sides| {
            let mut by_side = serde_json::Map::new();
            for side in sides {
                by_side.insert(
                    side.side.key().to_owned(),
                    json!({
                        "formatter": side.formatter.map(DateBackend::name),
                        "features": side.features,
                    }),
                );
            }
            Value::Object(by_side)
        });
        let choices = (!self.date_choices.is_empty()).then(|| {
            self.date_choices
                .iter()
                .map(|(what, features)| json!({ "for": what, "features": features }))
                .collect::<Vec<_>>()
        });
        json!({
            "source": self.source,
            "needs": self.needs,
            "on": self.on,
            "unused": self.unused,
            "line": self.line,
            "dates": dates,
            "date_choices": choices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FeatureList, Needs, Source};
    use mf2_build::Features;

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
            list.on.as_deref().unwrap_or_default(),
            ["fn-number", "datetime", "native-datetime-iso"]
        );
        assert_eq!(
            list.unused.as_deref().unwrap_or_default(),
            ["datetime", "native-datetime-iso"]
        );
        assert_eq!(list.line, ["ratatui", "fn-number"]);
        assert!(
            list.to_text()
                .contains(r#"features = ["ratatui", "fn-number"]"#)
        );
        assert!(!list.to_text().contains("dates:"));
    }

    #[test]
    fn a_needed_family_keeps_its_formatters_and_names_each_side() {
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
        assert_eq!(list.unused, Some(Vec::new()));
        assert_eq!(
            list.line,
            [
                "fn-number",
                "leptos-client-datetime-intl",
                "leptos-server-datetime-icu"
            ]
        );
        let text = list.to_text();
        assert!(
            text.contains("the browser:      `intl` formats dates (leptos-client-datetime-intl;"),
            "{text}"
        );
        assert!(
            text.contains("native code:      `icu` formats dates (leptos-server-datetime-icu;"),
            "{text}"
        );
        // The "both backends" lines are gone.
        assert!(!text.contains("once hydrated"), "{text}");
        let json = list.to_json();
        assert!(json.get("date_backend").is_none(), "{json}");
        assert_eq!(json["dates"]["browser"]["formatter"], "intl");
        assert_eq!(json["dates"]["native"]["formatter"], "icu");
        assert_eq!(
            json["dates"]["native"]["features"],
            serde_json::json!(["leptos-server-datetime-icu"])
        );
        assert!(json["date_choices"].is_null());
    }

    #[test]
    fn a_missing_formatter_is_the_recommended_one_and_number_intl_stays_with_numbers() {
        let list = list((true, true), &["number-intl", "csr"], &Source::Given);
        assert_eq!(list.needs, ["fn-number", "dates"]);
        assert_eq!(
            list.line,
            [
                "csr",
                "fn-number",
                "number-intl",
                "leptos-client-datetime-intl"
            ]
        );
        assert!(
            list.to_text()
                .contains("the browser:      no date formatter")
        );
    }

    #[test]
    fn a_server_rendered_crate_with_only_a_client_formatter_is_told_the_server_one() {
        let on = ["leptos", "ssr", "datetime", "leptos-client-datetime-intl"];
        let written = cargo(&["leptos", "leptos-client-datetime-intl"]);
        let list = list((false, true), &on, &written);
        assert_eq!(
            list.line,
            [
                "leptos",
                "leptos-client-datetime-intl",
                "leptos-server-datetime-icu"
            ]
        );
        assert!(
            list.to_text()
                .contains("native code:      no date formatter"),
            "{}",
            list.to_text()
        );
        assert!(list.to_json()["dates"]["native"]["formatter"].is_null());
    }

    #[test]
    fn datetime_alone_is_never_written() {
        // A command-line tool: its family's recommended formatter instead.
        let list = list(
            (false, true),
            &["native", "datetime"],
            &cargo(&["native", "datetime"]),
        );
        assert_eq!(list.line, ["native", "native-datetime-icu"]);
        // No framework: the lines to choose from.
        let list = list_given((false, true), &["datetime"]);
        assert!(list.line.is_empty(), "{:?}", list.line);
        let text = list.to_text();
        for (_, features) in mf2_build::kind_lines::<mf2_build::DateBackend>() {
            for feature in features {
                assert!(text.contains(&format!("\"{feature}\"")), "{text}");
            }
        }
        assert_eq!(
            list.to_json()["date_choices"].as_array().map(Vec::len),
            Some(mf2_build::KINDS.len())
        );
    }

    fn list_given(needs: (bool, bool), on: &[&str]) -> FeatureList {
        list(needs, on, &Source::Given)
    }

    #[test]
    fn without_cargo_the_list_is_the_needs_only() {
        let on = ["fn-number", "datetime"];
        let list = list((false, true), &on, &Source::Unknown);
        assert_eq!(list.on, None);
        assert_eq!(list.dates, None);
        assert!(list.line.is_empty(), "{:?}", list.line);
        let text = list.to_text();
        assert!(text.contains("the corpus's needs only") && !text.contains("on and unused"));
        assert!(text.contains("\"native-datetime-icu\""), "{text}");
        assert!(list.to_json()["on"].is_null());
        assert!(list.to_json()["dates"].is_null());
    }
}
