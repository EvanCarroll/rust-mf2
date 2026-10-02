//! The client feature set (`plans/05-tooling.md` §3.1).
//!
//! Features belong to the **application**, declared once on its i18n crate
//! and applied to its server and its wasm build alike, so SSR output always
//! matches what the client would produce. `build.rs` therefore reads them
//! from that crate's own cargo features (`CARGO_FEATURE_FN_NUMBER`, …) and
//! `mf2-cli` from `--features`; neither reads `mf2.toml`, which would be a
//! second place for them to disagree.

use std::collections::BTreeSet;

/// Which functions the built catalogs may use, and which locale data they
/// need.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features {
    names: BTreeSet<String>,
}

/// A built-in function and the feature it needs.
///
/// `plans/00-master-plan.md` §5: a function that exists only behind a feature
/// and is used while that feature is off is a **build error** naming the file
/// and the line.
pub const BUILTINS: [(&str, Option<&str>); 10] = [
    ("string", None),
    ("number", None),
    ("integer", None),
    ("offset", None),
    ("percent", Some("fn-number")),
    ("currency", Some("fn-number")),
    ("unit", Some("fn-number")),
    ("datetime", Some("fn-datetime")),
    ("date", Some("fn-datetime")),
    ("time", Some("fn-datetime")),
];

/// The options each built-in function defines, for the `unknown-option`
/// lint: MF2 ignores an option a function does not have, so a misspelled or
/// borrowed name (`dateStyle` on `:datetime`) changes nothing and says
/// nothing at run time. Options in a namespace (`u:dir`, `ns:name`) are not
/// listed here: every function accepts `u:`'s, and another namespace's are
/// an implementation's own.
///
/// These are the names the runtime and the function crates read; the
/// conformance crate's `options_lint` test holds each list against what a
/// formatter actually reads.
pub const OPTIONS: [(&str, &[&str]); 10] = [
    ("string", &[]),
    (
        "number",
        &[
            "select",
            "signDisplay",
            "useGrouping",
            "minimumIntegerDigits",
            "minimumFractionDigits",
            "maximumFractionDigits",
            "minimumSignificantDigits",
            "maximumSignificantDigits",
            "trailingZeroDisplay",
            "roundingPriority",
            "roundingIncrement",
            "roundingMode",
        ],
    ),
    (
        "integer",
        &[
            "select",
            "signDisplay",
            "useGrouping",
            "minimumIntegerDigits",
            "maximumSignificantDigits",
        ],
    ),
    ("offset", &["add", "subtract"]),
    (
        "percent",
        &[
            "signDisplay",
            "useGrouping",
            "minimumFractionDigits",
            "maximumFractionDigits",
            "minimumSignificantDigits",
            "maximumSignificantDigits",
            "trailingZeroDisplay",
            "roundingPriority",
            "roundingMode",
        ],
    ),
    (
        "currency",
        &[
            "currency",
            "currencyDisplay",
            "currencySign",
            "fractionDigits",
            "useGrouping",
            "minimumIntegerDigits",
            "minimumSignificantDigits",
            "maximumSignificantDigits",
            "trailingZeroDisplay",
            "roundingPriority",
            "roundingIncrement",
            "roundingMode",
        ],
    ),
    (
        "unit",
        &[
            "unit",
            "unitDisplay",
            "usage",
            "signDisplay",
            "useGrouping",
            "minimumIntegerDigits",
            "minimumFractionDigits",
            "maximumFractionDigits",
            "minimumSignificantDigits",
            "maximumSignificantDigits",
            "roundingPriority",
            "roundingIncrement",
            "roundingMode",
        ],
    ),
    (
        "datetime",
        &[
            "dateFields",
            "dateLength",
            "timePrecision",
            "timeZoneStyle",
            "timeZone",
            "hour12",
            "calendar",
        ],
    ),
    ("date", &["fields", "length", "timeZone", "calendar"]),
    (
        "time",
        &[
            "precision",
            "timeZoneStyle",
            "timeZone",
            "hour12",
            "calendar",
        ],
    ),
];

/// Whether the built-in function `function` defines the option `option`;
/// `None` if `function` is not a built-in or `option` is in a namespace.
pub fn defines_option(function: &str, option: &str) -> Option<bool> {
    if option.contains(':') {
        return None;
    }
    let (_, names) = OPTIONS.iter().find(|(name, _)| *name == function)?;
    Some(names.contains(&option))
}

/// The features that decide what a catalog may hold: which functions a
/// message may call, and which locale data the catalog carries for them.
/// The wasm and the catalogs must agree on these; the others (`intl`,
/// `datetime-intl`, `tzdb-bundled`, the host features) change only the code
/// a build compiles — `tzdb-bundled` only which IANA database a named time
/// zone is looked up in.
pub const CATALOG_FEATURES: [&str; 3] = ["fn-number", "fn-datetime", "datetime-icu"];

impl Features {
    /// The set cargo passed this `build.rs`: every `CARGO_FEATURE_*` in the
    /// environment, back in its `kebab-case` spelling.
    ///
    /// Cargo spells a feature `some-feature` as `CARGO_FEATURE_SOME_FEATURE`,
    /// so a feature whose name already holds `_` comes back with `-`. None of
    /// the facade's features does (`plans/00-master-plan.md` §5).
    pub fn from_env() -> Features {
        Features::from_vars(std::env::vars())
    }

    /// [`Features::from_env`] over a given environment, for tests.
    pub fn from_vars(vars: impl IntoIterator<Item = (String, String)>) -> Features {
        let names = vars
            .into_iter()
            .filter_map(|(key, _)| {
                key.strip_prefix("CARGO_FEATURE_")
                    .map(|f| f.to_ascii_lowercase().replace('_', "-"))
            })
            .collect();
        Features { names }
    }

    /// The set a `--features` argument names: comma- or space-separated, with
    /// `crate/feature` reduced to `feature` as cargo reads it.
    pub fn parse(list: &str) -> Features {
        let names = list
            .split([',', ' '])
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .map(|f| f.rsplit_once('/').map_or(f, |(_, name)| name).to_owned())
            .collect();
        Features { names }
    }

    /// A set from names, for tests and `mf2 compile`.
    pub fn from_names<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Features {
        Features {
            names: names.into_iter().map(Into::into).collect(),
        }
    }

    /// Only the [`CATALOG_FEATURES`] of this set: what `mf2 compile --site`
    /// compares with the i18n crate's.
    #[must_use]
    pub fn for_catalogs(&self) -> Features {
        Features::from_names(CATALOG_FEATURES.into_iter().filter(|f| self.has(f)))
    }

    /// Whether `name` is on.
    pub fn has(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// The names, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// Number localization: symbols, grouping, numbering systems, and
    /// `:percent` / `:currency` / `:unit`.
    pub fn fn_number(&self) -> bool {
        self.has("fn-number")
    }

    /// The date and time family.
    pub fn fn_datetime(&self) -> bool {
        self.has("fn-datetime")
    }

    /// Dates over ICU4X, from the catalog's `icu.blob`.
    pub fn datetime_icu(&self) -> bool {
        self.has("datetime-icu")
    }

    /// Dates over the browser's `Intl.DateTimeFormat`.
    pub fn datetime_intl(&self) -> bool {
        self.has("datetime-intl")
    }

    /// Numbers, plural selection and dates through the browser's `Intl` on
    /// the client (D4). The server keeps the Rust path, so a catalog still
    /// carries the data unless the build writes a client variant.
    pub fn intl(&self) -> bool {
        self.has("intl")
    }

    /// Whether this build provides the built-in function `identifier` (an
    /// MF2 identifier without its `:`), or `None` if it is not a built-in —
    /// a custom function, which `[functions]` in `mf2.toml` answers for.
    pub fn provides(&self, identifier: &str) -> Option<bool> {
        let (_, gate) = BUILTINS.iter().find(|(name, _)| *name == identifier)?;
        Some(match gate {
            None => true,
            Some(feature) => self.has(feature),
        })
    }

    /// The feature `identifier` needs and this build does not have.
    pub fn missing_feature(&self, identifier: &str) -> Option<&'static str> {
        let (_, gate) = BUILTINS.iter().find(|(name, _)| *name == identifier)?;
        gate.filter(|feature| !self.has(feature))
    }
}

#[cfg(test)]
mod tests {
    use super::Features;

    #[test]
    fn cargo_feature_variables_become_feature_names() {
        let features = Features::from_vars([
            ("CARGO_FEATURE_FN_NUMBER".to_owned(), "1".to_owned()),
            ("CARGO_FEATURE_SSR".to_owned(), "1".to_owned()),
            ("PATH".to_owned(), "/usr/bin".to_owned()),
        ]);
        assert!(features.fn_number());
        assert!(features.has("ssr"));
        assert!(!features.fn_datetime());
        assert_eq!(features.names().collect::<Vec<_>>(), ["fn-number", "ssr"]);
    }

    #[test]
    fn a_features_argument_reads_like_cargos() {
        let features = Features::parse("fn-number, mf2/fn-datetime datetime-icu");
        assert!(features.fn_number());
        assert!(features.fn_datetime());
        assert!(features.datetime_icu());
    }

    #[test]
    fn only_some_features_change_a_catalog() {
        let features = Features::parse("default,ssr,intl,datetime-intl,fn-datetime,fn-number");
        assert_eq!(
            features.for_catalogs().names().collect::<Vec<_>>(),
            ["fn-datetime", "fn-number"]
        );
    }

    #[test]
    fn gated_functions_need_their_feature() {
        let none = Features::default();
        assert_eq!(none.provides("number"), Some(true));
        assert_eq!(none.provides("percent"), Some(false));
        assert_eq!(none.provides("date"), Some(false));
        assert_eq!(none.provides("app:emoji"), None);
        assert_eq!(none.missing_feature("currency"), Some("fn-number"));
        assert_eq!(none.missing_feature("number"), None);

        let both = Features::parse("fn-number,fn-datetime");
        assert_eq!(both.provides("percent"), Some(true));
        assert_eq!(both.provides("time"), Some(true));
        assert_eq!(both.missing_feature("unit"), None);
    }
}
