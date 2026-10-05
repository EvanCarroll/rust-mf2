//! The feature list `mf2 check` prints (`plan/01-size-and-features.md` §5):
//! the function and data features the corpus needs, those that are on, those
//! on and unused, and the features to write on the `mf2` dependency, with the
//! modes (`ssr`, `ratatui`, …) left as they are. There is no default set:
//! this list stands in for one.

use std::fmt::Write as _;

use mf2_build::check::Needs;
use mf2_build::{Backend, DateBackend, Features, NumberBackend, Side, domain_features, kind_lines};
use serde_json::{Value, json};

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

/// One side's formatter of a domain (`plan/08` §3.2): the one in force, and
/// the features of the side that are on.
#[derive(Debug, PartialEq, Eq)]
struct SideFormatter {
    side: Side,
    /// The formatter the side's build formats with — its name, and what it
    /// costs there; `None` with none on.
    formatter: Option<(&'static str, String)>,
    /// The side's features of the domain, as the crate writes them.
    features: Vec<String>,
}

/// What the list says of one domain: numbers, or dates.
#[derive(Debug, PartialEq, Eq)]
struct Domain {
    /// What its formatters format, and so the family's name in `needs` and
    /// in the JSON: `numbers`.
    things: &'static str,
    /// The domain as an adjective: `number`.
    adjective: &'static str,
    /// What one of its formatters is called: `number formatter`.
    noun: &'static str,
    /// A message uses it.
    needed: bool,
    /// Its features that are on: its own, and every formatter's.
    on: Vec<String>,
    /// Per side, the formatter in force; `None` when unknown.
    sides: Option<Vec<SideFormatter>>,
    /// Its formatters to write on the `mf2` dependency.
    write: Vec<String>,
    /// The corpus uses the domain and `write` names no formatter, because
    /// nothing says which builds the crate has: the lines to choose from,
    /// those of [`mf2_build::kind_lines`] whose framework is on, or all of
    /// them with none on. Empty otherwise.
    choices: Vec<(&'static str, Vec<String>)>,
}

impl Domain {
    /// The domain of `B`, for a corpus that uses it (`needed`) or does not.
    /// `written` is what the crate writes on `mf2`; `known` whether anybody
    /// could say which features are on.
    fn new<B: Backend>(needed: bool, features: &Features, written: &[String], known: bool) -> Self {
        let all = domain_features::<B>();
        let on = all
            .iter()
            .filter(|name| features.has(name))
            .cloned()
            .collect();
        let mut write = Vec::new();
        let mut choices = Vec::new();
        if needed {
            // The formatters as the crate writes them, not every name they
            // imply; then, for each framework's side with none, its
            // recommended one (`plan/08` §3.5). The domain's own feature
            // alone is never written: it formats nothing.
            write = written
                .iter()
                .filter(|name| *name != B::DOMAIN && all.contains(name))
                .cloned()
                .collect();
            if known {
                for feature in features.missing::<B>() {
                    if !write.contains(&feature) {
                        write.push(feature);
                    }
                }
            }
            if write.is_empty() {
                choices = if known {
                    features.lines::<B>()
                } else {
                    kind_lines::<B>()
                };
            }
        }
        let sides = known.then(|| {
            Side::ALL
                .into_iter()
                .map(|side| SideFormatter {
                    side,
                    formatter: features
                        .backend::<B>(side)
                        .map(|backend| (backend.name(), backend.cost(side))),
                    features: features.features_on::<B>(side),
                })
                .collect()
        });
        Domain {
            things: B::THINGS,
            adjective: B::ADJECTIVE,
            noun: B::NOUN,
            needed,
            on,
            sides,
            write,
            choices,
        }
    }
}

/// The list, worked out.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FeatureList {
    /// The families the corpus needs: `numbers`, `dates`.
    needs: Vec<&'static str>,
    /// The number and date features that are on; `None` when unknown.
    on: Option<Vec<String>>,
    /// Those of `on` whose family no message uses.
    unused: Option<Vec<String>>,
    /// The features to write on the `mf2` dependency.
    line: Vec<String>,
    /// Numbers, then dates.
    domains: [Domain; 2],
    /// The sides the application has: those of the frameworks that are on,
    /// or both when nothing says which builds the crate has.
    sides: Vec<Side>,
    /// Where `on` came from.
    source: &'static str,
}

impl FeatureList {
    pub(crate) fn new(needs: Needs, features: &Features, source: &Source) -> FeatureList {
        let known = !matches!(source, Source::Unknown);
        let written: Vec<String> = match source {
            Source::Cargo { written } => written.clone(),
            Source::Given => features.names().map(str::to_owned).collect(),
            Source::Unknown => Vec::new(),
        };
        let domains = [
            Domain::new::<NumberBackend>(needs.numbers, features, &written, known),
            Domain::new::<DateBackend>(needs.dates, features, &written, known),
        ];
        let on: Vec<String> = domains
            .iter()
            .flat_map(|domain| domain.on.iter().cloned())
            .collect();
        let unused: Vec<String> = domains
            .iter()
            .filter(|domain| !domain.needed)
            .flat_map(|domain| domain.on.iter().cloned())
            .collect();
        // The modes as they are, then each needed family's formatters. What
        // a family's feature implies is not a mode: the names of every
        // domain's features go, on or not.
        let of_a_domain = |name: &String| {
            domain_features::<NumberBackend>().contains(name)
                || domain_features::<DateBackend>().contains(name)
        };
        let mut line: Vec<String> = written
            .iter()
            .filter(|name| !of_a_domain(name))
            .cloned()
            .collect();
        for domain in &domains {
            line.extend(domain.write.iter().cloned());
        }
        let families = features.families();
        let sides = Side::ALL
            .into_iter()
            .filter(|side| {
                families.is_empty() || families.iter().any(|active| active.family.side == *side)
            })
            .collect();
        FeatureList {
            needs: domains
                .iter()
                .filter(|domain| domain.needed)
                .map(|domain| domain.things)
                .collect(),
            on: known.then_some(on),
            unused: known.then_some(unused),
            line,
            domains,
            sides,
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
        // Each side's formatters: of a domain the corpus uses, or one that
        // has a feature on. A domain neither used nor on has nothing to say,
        // and neither has a side the application does not have, unless a
        // formatter of it is on all the same.
        for side in Side::ALL {
            for domain in &self.domains {
                if !domain.needed && domain.on.is_empty() {
                    continue;
                }
                let Some(formatter) = domain
                    .sides
                    .iter()
                    .flatten()
                    .find(|formatter| formatter.side == side)
                else {
                    continue;
                };
                if !self.sides.contains(&side) && formatter.formatter.is_none() {
                    continue;
                }
                let label = format!("{}:", side.name());
                let what = match &formatter.formatter {
                    Some((name, cost)) => format!(
                        "`{name}` formats {} ({}; {cost})",
                        domain.things,
                        formatter.features.join(", "),
                    ),
                    None => format!("no {}", domain.noun),
                };
                let _ = writeln!(out, "  {label:<17} {what}");
            }
        }
        let quoted: Vec<String> = self.line.iter().map(|f| format!("\"{f}\"")).collect();
        let _ = writeln!(
            out,
            "  write:            mf2 = {{ ..., features = [{}] }}",
            quoted.join(", ")
        );
        for domain in &self.domains {
            if domain.choices.is_empty() {
                continue;
            }
            let _ = writeln!(
                out,
                "  and for {}, the line of the application's kind:",
                domain.things
            );
            for (what, features) in &domain.choices {
                let quoted: Vec<String> = features.iter().map(|f| format!("\"{f}\"")).collect();
                let _ = writeln!(out, "    {what}: {}", quoted.join(", "));
            }
        }
        out
    }

    /// The `features` field of the JSON report.
    pub(crate) fn to_json(&self) -> Value {
        let mut value = json!({
            "source": self.source,
            "needs": self.needs,
            "on": self.on,
            "unused": self.unused,
            "line": self.line,
        });
        for domain in &self.domains {
            let sides = domain.sides.as_ref().map(|sides| {
                let mut by_side = serde_json::Map::new();
                for side in sides {
                    by_side.insert(
                        side.side.key().to_owned(),
                        json!({
                            "formatter": side.formatter.as_ref().map(|(name, _)| name),
                            "features": side.features,
                        }),
                    );
                }
                Value::Object(by_side)
            });
            let choices = (!domain.choices.is_empty()).then(|| {
                domain
                    .choices
                    .iter()
                    .map(|(what, features)| json!({ "for": what, "features": features }))
                    .collect::<Vec<_>>()
            });
            value[domain.things] = json!(sides);
            value[format!("{}_choices", domain.adjective)] = json!(choices);
        }
        value
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
            "number",
            "host-std-number-builtin",
            "native-number-builtin",
            "datetime",
            "host-std-datetime-iso",
            "native-datetime-iso",
            "ratatui",
            "native",
            "host-std",
        ];
        let written = cargo(&["ratatui", "native-number-builtin", "native-datetime-iso"]);
        let list = list((true, false), &on, &written);
        assert_eq!(list.needs, ["numbers"]);
        assert_eq!(
            list.on.as_deref().unwrap_or_default(),
            [
                "number",
                "host-std-number-builtin",
                "native-number-builtin",
                "datetime",
                "host-std-datetime-iso",
                "native-datetime-iso"
            ]
        );
        assert_eq!(
            list.unused.as_deref().unwrap_or_default(),
            ["datetime", "host-std-datetime-iso", "native-datetime-iso"]
        );
        assert_eq!(list.line, ["ratatui", "native-number-builtin"]);
        let text = list.to_text();
        assert!(
            text.contains(r#"features = ["ratatui", "native-number-builtin"]"#),
            "{text}"
        );
        assert!(
            text.contains("native code:      `builtin` formats numbers (native-number-builtin;"),
            "{text}"
        );
        // A terminal application has no browser side: nothing is said of one.
        assert!(!text.contains("the browser:"), "{text}");
        // The date formatter is on, so it is said, though no message uses it.
        assert!(
            text.contains("native code:      `iso` formats dates (native-datetime-iso;"),
            "{text}"
        );
    }

    #[test]
    fn a_needed_family_keeps_its_formatters_and_names_each_side() {
        let on = [
            "number",
            "leptos-client-number-intl",
            "leptos-server-number-builtin",
            "datetime",
            "leptos-client-datetime-intl",
            "leptos-server-datetime-icu",
        ];
        let written = cargo(&[
            "leptos-client-number-intl",
            "leptos-server-number-builtin",
            "leptos-client-datetime-intl",
            "leptos-server-datetime-icu",
        ]);
        let list = list((true, true), &on, &written);
        assert_eq!(list.needs, ["numbers", "dates"]);
        assert_eq!(list.unused, Some(Vec::new()));
        assert_eq!(
            list.line,
            [
                "leptos-client-number-intl",
                "leptos-server-number-builtin",
                "leptos-client-datetime-intl",
                "leptos-server-datetime-icu"
            ]
        );
        let text = list.to_text();
        for line in [
            "the browser:      `intl` formats numbers (leptos-client-number-intl;",
            "the browser:      `intl` formats dates (leptos-client-datetime-intl;",
            "native code:      `builtin` formats numbers (leptos-server-number-builtin;",
            "native code:      `icu` formats dates (leptos-server-datetime-icu;",
        ] {
            assert!(text.contains(line), "{line}: {text}");
        }
        // The "both backends" lines are gone.
        assert!(!text.contains("once hydrated"), "{text}");
        let json = list.to_json();
        assert!(json.get("date_backend").is_none(), "{json}");
        assert_eq!(json["numbers"]["browser"]["formatter"], "intl");
        assert_eq!(json["numbers"]["native"]["formatter"], "builtin");
        assert_eq!(json["dates"]["browser"]["formatter"], "intl");
        assert_eq!(json["dates"]["native"]["formatter"], "icu");
        assert_eq!(
            json["dates"]["native"]["features"],
            serde_json::json!(["leptos-server-datetime-icu"])
        );
        assert!(json["number_choices"].is_null());
        assert!(json["date_choices"].is_null());
    }

    #[test]
    fn a_missing_formatter_is_the_recommended_one() {
        let list = list((true, true), &["csr"], &Source::Given);
        assert_eq!(list.needs, ["numbers", "dates"]);
        assert_eq!(
            list.line,
            [
                "csr",
                "leptos-client-number-intl",
                "leptos-client-datetime-intl"
            ]
        );
        let text = list.to_text();
        assert!(
            text.contains("the browser:      no number formatter"),
            "{text}"
        );
        assert!(
            text.contains("the browser:      no date formatter"),
            "{text}"
        );
        // A client-only application has no native side.
        assert!(!text.contains("native code:"), "{text}");
        // With nothing that says which builds the crate has: both sides.
        let text = super::FeatureList::new(
            Needs {
                numbers: true,
                dates: false,
            },
            &Features::from_names(["host-std-number-builtin"]),
            &Source::Given,
        )
        .to_text();
        assert!(
            text.contains("the browser:      no number formatter"),
            "{text}"
        );
        assert!(
            text.contains("native code:      `builtin` formats numbers"),
            "{text}"
        );
    }

    #[test]
    fn the_cached_browser_formatter_is_named() {
        // As cargo resolves it: the feature turns `icu`'s on, and the host's.
        let on = [
            "csr",
            "datetime",
            "host-web-datetime-icu",
            "host-web-datetime-icu-cached",
            "leptos-client-datetime-icu-cached",
        ];
        let written = cargo(&["csr", "leptos-client-datetime-icu-cached"]);
        let list = list((false, true), &on, &written);
        assert_eq!(list.line, ["csr", "leptos-client-datetime-icu-cached"]);
        let text = list.to_text();
        assert!(
            text.contains(
                "the browser:      `icu-cached` formats dates \
                 (leptos-client-datetime-icu-cached;"
            ),
            "{text}"
        );
        assert_eq!(
            list.to_json()["dates"]["browser"]["formatter"],
            "icu-cached"
        );
    }

    #[test]
    fn a_domain_neither_used_nor_on_has_no_line() {
        let list = list(
            (true, false),
            &["native", "native-number-builtin"],
            &Source::Given,
        );
        let text = list.to_text();
        assert!(text.contains("formats numbers"), "{text}");
        assert!(!text.contains("date formatter"), "{text}");
        assert!(!text.contains("formats dates"), "{text}");
        // The JSON says each side's formatter of each domain all the same.
        assert!(list.to_json()["dates"]["native"]["formatter"].is_null());
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

        // Numbers, the same.
        let on = ["leptos", "ssr", "number", "leptos-client-number-intl"];
        let written = cargo(&["leptos", "leptos-client-number-intl"]);
        let list = super::FeatureList::new(
            Needs {
                numbers: true,
                dates: false,
            },
            &Features::from_names(on),
            &written,
        );
        assert_eq!(
            list.line,
            [
                "leptos",
                "leptos-client-number-intl",
                "leptos-server-number-builtin"
            ]
        );
    }

    #[test]
    fn a_domains_own_feature_alone_is_never_written() {
        // A command-line tool: its family's recommended formatter instead.
        let list = list(
            (true, true),
            &["native", "number", "datetime"],
            &cargo(&["native", "number", "datetime"]),
        );
        assert_eq!(
            list.line,
            ["native", "native-number-builtin", "native-datetime-icu"]
        );
        // No framework: the lines to choose from.
        let list = list_given((true, true), &["number", "datetime"]);
        assert!(list.line.is_empty(), "{:?}", list.line);
        let text = list.to_text();
        assert!(
            text.contains("and for numbers, the line of the application's kind:"),
            "{text}"
        );
        assert!(
            text.contains("and for dates, the line of the application's kind:"),
            "{text}"
        );
        let lines = mf2_build::kind_lines::<mf2_build::NumberBackend>()
            .into_iter()
            .chain(mf2_build::kind_lines::<mf2_build::DateBackend>());
        for (_, features) in lines {
            for feature in features {
                assert!(text.contains(&format!("\"{feature}\"")), "{text}");
            }
        }
        for choices in ["number_choices", "date_choices"] {
            assert_eq!(
                list.to_json()[choices].as_array().map(Vec::len),
                Some(mf2_build::KINDS.len())
            );
        }
    }

    fn list_given(needs: (bool, bool), on: &[&str]) -> FeatureList {
        list(needs, on, &Source::Given)
    }

    #[test]
    fn without_cargo_the_list_is_the_needs_only() {
        let on = ["host-std-number-builtin", "datetime"];
        let list = list((false, true), &on, &Source::Unknown);
        assert_eq!(list.on, None);
        assert!(list.domains.iter().all(|domain| domain.sides.is_none()));
        assert!(list.line.is_empty(), "{:?}", list.line);
        let text = list.to_text();
        assert!(text.contains("the corpus's needs only") && !text.contains("on and unused"));
        assert!(text.contains("\"native-datetime-icu\""), "{text}");
        assert!(list.to_json()["on"].is_null());
        assert!(list.to_json()["dates"].is_null());
        assert!(list.to_json()["numbers"].is_null());
    }
}
