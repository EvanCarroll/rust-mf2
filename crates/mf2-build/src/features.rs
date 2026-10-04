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

/// The two builds that format dates (`plan/08` §3.1): a browser build, and
/// native code — a server, a command-line tool, a terminal UI. Each has its
/// own date formatter, chosen from the features of its own side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// `wasm32-unknown-unknown`: the families `host-web-datetime-` and
    /// `leptos-client-datetime-`.
    Browser,
    /// Everything else: the families `host-std-datetime-`,
    /// `leptos-server-datetime-`, `axum-datetime-` and `native-datetime-`.
    Native,
}

impl Side {
    /// Both sides, the browser first.
    pub const ALL: [Side; 2] = [Side::Browser, Side::Native];

    /// The feature families of this side, the framework-free one first:
    /// each family's features are its prefix and a formatter's name.
    pub fn families(self) -> &'static [&'static str] {
        match self {
            Side::Browser => &["host-web-datetime-", "leptos-client-datetime-"],
            Side::Native => &[
                "host-std-datetime-",
                "leptos-server-datetime-",
                "axum-datetime-",
                "native-datetime-",
            ],
        }
    }

    /// The formatters this side's families offer, the strongest first.
    pub fn formatters(self) -> &'static [DateFormatter] {
        match self {
            Side::Browser => &[DateFormatter::Icu, DateFormatter::Intl, DateFormatter::Iso],
            Side::Native => &[DateFormatter::Icu, DateFormatter::Iso],
        }
    }
}

/// A date formatter, the weakest first: the order is the rule a build with
/// more than one of its side's on applies, where the strongest formats
/// (`plan/08` §3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DateFormatter {
    /// The ISO stand-in: no locale data, no ICU4X.
    Iso,
    /// The browser's `Intl.DateTimeFormat`: no date data downloaded.
    Intl,
    /// ICU4X over the catalog's `icu.blob`, which the build cuts.
    Icu,
}

impl DateFormatter {
    /// The name a family's feature ends with.
    pub fn name(self) -> &'static str {
        match self {
            DateFormatter::Iso => "iso",
            DateFormatter::Intl => "intl",
            DateFormatter::Icu => "icu",
        }
    }
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
/// The wasm and the catalogs must agree on these (an `icu` formatter on
/// either side puts the date slice in); the others (`number-intl`, the
/// `intl` and `iso` formatters, `tzdb-bundled`, the host features) change only the code
/// a build compiles — `tzdb-bundled` only which IANA database a named time
/// zone is looked up in.
pub const CATALOG_FEATURES: [&str; 5] = [
    "fn-number",
    "fn-datetime",
    "datetime-icu",
    "host-std-datetime-icu",
    "host-web-datetime-icu",
];

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

    /// The date and time family: `datetime`, which every date formatter
    /// turns on (2.0's `fn-datetime`).
    pub fn fn_datetime(&self) -> bool {
        self.has("datetime") || self.has("fn-datetime")
    }

    /// Every date formatter on for `side`, the strongest first: a family's
    /// feature, or a 2.0 name (`datetime-icu` is `icu` on both sides,
    /// `datetime-intl` is `intl` in the browser and `icu` natively). More
    /// than one is the `several-date-formatters` case.
    pub fn date_formatters(&self, side: Side) -> Vec<DateFormatter> {
        side.formatters()
            .iter()
            .copied()
            .filter(|formatter| {
                side.families()
                    .iter()
                    .any(|family| self.has(&format!("{family}{}", formatter.name())))
                    || self.legacy_date_formatter(side, *formatter)
            })
            .collect()
    }

    /// The 2.0 names, until every user moves (`plan/08` §3.6).
    fn legacy_date_formatter(&self, side: Side, formatter: DateFormatter) -> bool {
        match (side, formatter) {
            (_, DateFormatter::Icu) => {
                self.has("datetime-icu") || (side == Side::Native && self.has("datetime-intl"))
            }
            (Side::Browser, DateFormatter::Intl) => self.has("datetime-intl"),
            _ => false,
        }
    }

    /// The formatter `side`'s build formats dates with: the strongest of its
    /// own side's on (ICU4X, then `Intl`, then ISO), whatever the other
    /// side's say; `None` with none.
    pub fn date_formatter(&self, side: Side) -> Option<DateFormatter> {
        self.date_formatters(side).into_iter().max()
    }

    /// Whether the catalogs carry ICU4X's date slice (`icu.blob`): when
    /// either side's formatter is `icu`. A build script cuts it for both
    /// builds, so it needs `mf2-build`'s `icu-blob` then.
    pub fn cuts_date_slice(&self) -> bool {
        Side::ALL
            .into_iter()
            .any(|side| self.date_formatter(side) == Some(DateFormatter::Icu))
    }

    /// Dates over ICU4X, from the catalog's `icu.blob`.
    pub fn datetime_icu(&self) -> bool {
        self.has("datetime-icu")
    }

    /// Dates over the browser's `Intl.DateTimeFormat`.
    pub fn datetime_intl(&self) -> bool {
        self.has("datetime-intl")
    }

    /// Numbers and plural selection through the browser's `Intl` on the
    /// client (`number-intl`, 2.0's `intl`). The server keeps the Rust path, so a catalog still
    /// carries the data unless the build writes a client variant.
    pub fn number_intl(&self) -> bool {
        self.has("number-intl")
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
    use super::{DateFormatter, Side};

    // `plan/08` §3.2, on each side: the formatter is chosen from the
    // features of its own side only; with more than one of a family on, the
    // strongest formats; with two frameworks that disagree, the strongest
    // formats; and a feature only adds.

    #[test]
    fn each_side_reads_only_its_own_features() {
        let split = Features::parse("leptos-client-datetime-intl,leptos-server-datetime-icu");
        assert_eq!(
            split.date_formatter(Side::Browser),
            Some(DateFormatter::Intl)
        );
        assert_eq!(split.date_formatter(Side::Native), Some(DateFormatter::Icu));
        assert_eq!(split.date_formatters(Side::Browser), [DateFormatter::Intl]);
        assert_eq!(split.date_formatters(Side::Native), [DateFormatter::Icu]);

        let browser_only = Features::parse("host-web-datetime-icu");
        assert_eq!(
            browser_only.date_formatter(Side::Browser),
            Some(DateFormatter::Icu)
        );
        assert_eq!(browser_only.date_formatter(Side::Native), None);

        let native_only = Features::parse("native-datetime-icu");
        assert_eq!(
            native_only.date_formatter(Side::Native),
            Some(DateFormatter::Icu)
        );
        assert_eq!(native_only.date_formatter(Side::Browser), None);

        // `intl` is no formatter of the native families.
        let no_native_intl = Features::parse("native-datetime-intl,host-std-datetime-intl");
        assert_eq!(no_native_intl.date_formatter(Side::Native), None);
        assert_eq!(no_native_intl.date_formatter(Side::Browser), None);
    }

    #[test]
    fn two_of_one_family_the_strongest_formats_in_the_browser() {
        let icu_intl = Features::parse("leptos-client-datetime-intl,leptos-client-datetime-icu");
        assert_eq!(
            icu_intl.date_formatter(Side::Browser),
            Some(DateFormatter::Icu)
        );
        assert_eq!(
            icu_intl.date_formatters(Side::Browser),
            [DateFormatter::Icu, DateFormatter::Intl]
        );
        let intl_iso = Features::parse("host-web-datetime-iso,host-web-datetime-intl");
        assert_eq!(
            intl_iso.date_formatter(Side::Browser),
            Some(DateFormatter::Intl)
        );
        let all =
            Features::parse("host-web-datetime-iso,host-web-datetime-intl,host-web-datetime-icu");
        assert_eq!(all.date_formatter(Side::Browser), Some(DateFormatter::Icu));
        assert_eq!(all.date_formatters(Side::Browser).len(), 3);
    }

    #[test]
    fn two_of_one_family_the_strongest_formats_natively() {
        let both = Features::parse("axum-datetime-iso,axum-datetime-icu");
        assert_eq!(both.date_formatter(Side::Native), Some(DateFormatter::Icu));
        assert_eq!(
            both.date_formatters(Side::Native),
            [DateFormatter::Icu, DateFormatter::Iso]
        );
        assert_eq!(both.date_formatter(Side::Browser), None);
    }

    #[test]
    fn two_frameworks_that_disagree_the_strongest_formats_natively() {
        // A command-line tool on ISO, with an optional web mode on ICU4X.
        let cli = Features::parse("native-datetime-iso");
        assert_eq!(cli.date_formatter(Side::Native), Some(DateFormatter::Iso));
        assert!(!cli.cuts_date_slice());
        let with_web = Features::parse("native-datetime-iso,axum-datetime-icu");
        assert_eq!(
            with_web.date_formatter(Side::Native),
            Some(DateFormatter::Icu)
        );
        assert!(with_web.cuts_date_slice());
        let leptos = Features::parse("leptos-server-datetime-iso,native-datetime-icu");
        assert_eq!(
            leptos.date_formatter(Side::Native),
            Some(DateFormatter::Icu)
        );
    }

    #[test]
    fn two_frameworks_that_disagree_the_strongest_formats_in_the_browser() {
        let mixed = Features::parse("host-web-datetime-intl,leptos-client-datetime-iso");
        assert_eq!(
            mixed.date_formatter(Side::Browser),
            Some(DateFormatter::Intl)
        );
        let icu = Features::parse("host-web-datetime-icu,leptos-client-datetime-intl");
        assert_eq!(icu.date_formatter(Side::Browser), Some(DateFormatter::Icu));
    }

    #[test]
    fn the_slice_is_cut_when_either_side_is_icu() {
        assert!(
            !Features::parse("leptos-client-datetime-intl,leptos-server-datetime-iso")
                .cuts_date_slice()
        );
        assert!(
            Features::parse("leptos-client-datetime-icu,leptos-server-datetime-iso")
                .cuts_date_slice()
        );
        assert!(
            Features::parse("leptos-client-datetime-intl,leptos-server-datetime-icu")
                .cuts_date_slice()
        );
        assert!(!Features::default().cuts_date_slice());
    }

    #[test]
    fn the_2_0_names_read_as_section_3_6_writes_them() {
        let icu = Features::parse("fn-datetime,datetime-icu");
        assert_eq!(icu.date_formatter(Side::Browser), Some(DateFormatter::Icu));
        assert_eq!(icu.date_formatter(Side::Native), Some(DateFormatter::Icu));
        let intl = Features::parse("fn-datetime,datetime-intl");
        assert_eq!(
            intl.date_formatter(Side::Browser),
            Some(DateFormatter::Intl)
        );
        assert_eq!(intl.date_formatter(Side::Native), Some(DateFormatter::Icu));
        assert!(intl.cuts_date_slice());
        let both = Features::parse("datetime-icu,datetime-intl");
        assert_eq!(both.date_formatter(Side::Browser), Some(DateFormatter::Icu));
        assert_eq!(
            Features::parse("fn-datetime").date_formatter(Side::Native),
            None
        );
        assert!(Features::parse("datetime").fn_datetime());
    }

    #[test]
    fn date_formatters_are_ordered_weakest_first() {
        assert!(DateFormatter::Icu > DateFormatter::Intl);
        assert!(DateFormatter::Intl > DateFormatter::Iso);
        for side in Side::ALL {
            let names: Vec<_> = side.formatters().iter().map(|f| f.name()).collect();
            assert_eq!(names.first(), Some(&"icu"));
            assert_eq!(names.last(), Some(&"iso"));
        }
    }

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
        let features = Features::parse(
            "default,ssr,number-intl,datetime-intl,fn-datetime,fn-number,host-web-datetime-intl",
        );
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
