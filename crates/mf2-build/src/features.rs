//! The client feature set.
//!
//! Features belong to the **application**, declared once on its i18n crate
//! and applied to its server and its wasm build alike, so SSR output always
//! matches what the client would produce. `build.rs` therefore reads them
//! from that crate's own cargo features (`CARGO_FEATURE_FN_NUMBER`, …) and
//! `mf2-cli` from `--features`; neither reads `mf2.toml`, which would be a
//! second place for them to disagree.

use std::collections::BTreeSet;
use std::fmt::Write as _;

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
    #[doc(hidden)]
    pub const ALL: [Side; 2] = [Side::Browser, Side::Native];

    /// The feature families of this side, the framework-free one first:
    /// each family's features are its prefix and a formatter's name.
    #[doc(hidden)]
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
    #[doc(hidden)]
    pub fn formatters(self) -> &'static [DateFormatter] {
        match self {
            Side::Browser => &[DateFormatter::Icu, DateFormatter::Intl, DateFormatter::Iso],
            Side::Native => &[DateFormatter::Icu, DateFormatter::Iso],
        }
    }

    /// The formatter the tools recommend for this side (`plan/08` §3.1):
    /// `Intl` in a browser, ICU4X in native code.
    #[doc(hidden)]
    pub fn recommended(self) -> DateFormatter {
        match self {
            Side::Browser => DateFormatter::Intl,
            Side::Native => DateFormatter::Icu,
        }
    }

    /// The side as a message names it.
    #[doc(hidden)]
    pub fn name(self) -> &'static str {
        match self {
            Side::Browser => "the browser",
            Side::Native => "native code",
        }
    }

    /// The side as `mf2 check --format json` names it.
    #[doc(hidden)]
    pub fn key(self) -> &'static str {
        match self {
            Side::Browser => "browser",
            Side::Native => "native",
        }
    }
}

/// The date family of a framework that is on (`plan/08` §3.3, §3.5): the
/// features a crate writes for it are its prefix and a formatter's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateFamily {
    /// The family, without its formatter: `leptos-server-datetime-`.
    pub prefix: &'static str,
    /// The side its builds run on.
    pub side: Side,
    /// Whether this build is one of them, and so needs a formatter of its
    /// side. A Leptos server's build script also reads the browser's
    /// family, which changes no code in it.
    #[doc(hidden)]
    pub builds_here: bool,
}

impl DateFamily {
    /// The family's feature for `formatter`.
    #[doc(hidden)]
    pub fn feature(self, formatter: DateFormatter) -> String {
        format!("{}{}", self.prefix, formatter.name())
    }

    /// The family's feature for its side's recommended formatter.
    #[doc(hidden)]
    pub fn recommended(self) -> String {
        self.feature(self.side.recommended())
    }
}

/// What the tools write for dates when no framework says which build this
/// is (`plan/08` §3.5): a kind of application and its features.
pub const DATE_LINES: [(&str, &[&str]); 4] = [
    (
        "a server-rendered Leptos application",
        &["leptos-client-datetime-intl", "leptos-server-datetime-icu"],
    ),
    (
        "a command-line tool or a terminal UI",
        &["native-datetime-icu"],
    ),
    ("an Axum server", &["axum-datetime-icu"]),
    (
        "no framework",
        &["host-std-datetime-icu", "host-web-datetime-intl"],
    ),
];

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
    #[doc(hidden)]
    pub fn name(self) -> &'static str {
        match self {
            DateFormatter::Iso => "iso",
            DateFormatter::Intl => "intl",
            DateFormatter::Icu => "icu",
        }
    }

    /// What it is, as a message names it.
    #[doc(hidden)]
    pub fn what(self) -> &'static str {
        match self {
            DateFormatter::Iso => "the ISO stand-in",
            DateFormatter::Intl => "the browser's Intl.DateTimeFormat",
            DateFormatter::Icu => "ICU4X",
        }
    }

    /// What it costs on `side`, as `plan/08` §1.2 measured it (§3.1).
    #[doc(hidden)]
    pub fn cost(self, side: Side) -> &'static str {
        match (self, side) {
            (DateFormatter::Iso, _) => "no locale data and no ICU4X",
            (DateFormatter::Intl, _) => "+239 B gzipped, and no date data downloaded",
            (DateFormatter::Icu, Side::Browser) => {
                "+43 to +100 KB gzipped, and the date slice in each catalog"
            }
            (DateFormatter::Icu, Side::Native) => "+298 KB, and the date slices",
        }
    }
}

/// A built-in function and the feature it needs.
///
/// A function that exists only behind a feature
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
    ("datetime", Some("datetime")),
    ("date", Some("datetime")),
    ("time", Some("datetime")),
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

/// What decides what a catalog — the file a browser downloads — may hold,
/// as [`Features::for_catalogs`] names it: which functions a message may
/// call (`fn-number`, `datetime`), and whether the catalog carries ICU4X's
/// date slice (`icu-blob`, [`Features::date_slice_place`] being
/// [`Place::Catalog`]), and whether the browser formats numbers through
/// `Intl` and so reads none of the number entries (`number-intl`,
/// [`Features::number_place`] being [`Place::Server`]), or only the
/// currency and unit names (`intl-names`, the number split of `plan/08` §6,
/// [`Features::names_place`] being [`Place::Server`]; a feature of
/// `mf2-fn-number` and `mf2-host-web`, which the i18n crate or
/// `mf2 compile --features` names). These are the
/// browser-side choices, in the one list both builds see, so that a
/// server's build script knows what the browser reads (`plan/08` §4.1). The
/// wasm and the catalogs must agree on them; the other features (the `intl`
/// and `iso` formatters, `tzdb-bundled`, the host features) change only the
/// code a build compiles — `tzdb-bundled` only which IANA database a named
/// time zone is looked up in.
pub const CATALOG_FEATURES: [&str; 5] = [
    "fn-number",
    "datetime",
    "icu-blob",
    "number-intl",
    "intl-names",
];

/// Where the build writes a LOCALE entry (`plan/08` §4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Into the catalog: a browser reads it, or no browser downloads the
    /// catalog (a native application: one reader, one file).
    Catalog,
    /// Into the server-only table beside the catalog: a browser downloads
    /// the catalog and only native code reads the entry.
    Server,
    /// Nowhere: no side reads it.
    Nowhere,
}

impl Features {
    /// The set cargo passed this `build.rs`: every `CARGO_FEATURE_*` in the
    /// environment, back in its `kebab-case` spelling.
    ///
    /// Cargo spells a feature `some-feature` as `CARGO_FEATURE_SOME_FEATURE`,
    /// so a feature whose name already holds `_` comes back with `-`. None of
    /// the facade's features does.
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

    /// What of this set changes a catalog, in the [`CATALOG_FEATURES`]
    /// names: `fn-number`, `datetime` when the date functions are on under
    /// any of their names, `icu-blob` when the catalog carries the date
    /// slice (not when only the server-only table does), `number-intl` when
    /// the number entries go to the server-only table, `intl-names` when
    /// only the currency and unit entries do. What
    /// `mf2 compile --site` compares with the i18n crate's, so that two
    /// spellings of one build (a framework's family or the host's, with or
    /// without the `datetime` they imply) compare equal.
    #[must_use]
    pub fn for_catalogs(&self) -> Features {
        let [numbers, dates, slice, intl, names] = CATALOG_FEATURES;
        let on = [
            (numbers, self.fn_number()),
            (dates, self.fn_datetime()),
            (slice, self.date_slice_place() == Place::Catalog),
            (intl, self.number_place() == Place::Server),
            (
                names,
                self.number_place() == Place::Catalog && self.names_place() == Place::Server,
            ),
        ];
        Features::from_names(on.into_iter().filter(|(_, on)| *on).map(|(name, _)| name))
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

    /// The date and time functions: `datetime`, which every date formatter
    /// turns on. A formatter's feature counts too, for a list written by
    /// hand (`mf2 check --features native-datetime-icu`), which does not
    /// spell out what the feature implies.
    pub fn fn_datetime(&self) -> bool {
        self.has("datetime")
            || Side::ALL
                .into_iter()
                .any(|side| !self.date_formatters(side).is_empty())
    }

    /// Every date formatter on for `side`, the strongest first: one of its
    /// families' features. More than one is the `several-date-formatters`
    /// case.
    #[doc(hidden)]
    pub fn date_formatters(&self, side: Side) -> Vec<DateFormatter> {
        side.formatters()
            .iter()
            .copied()
            .filter(|formatter| {
                side.families()
                    .iter()
                    .any(|family| self.has(&format!("{family}{}", formatter.name())))
            })
            .collect()
    }

    /// The formatter `side`'s build formats dates with: the strongest of its
    /// own side's on (ICU4X, then `Intl`, then ISO), whatever the other
    /// side's say; `None` with none.
    pub fn date_formatter(&self, side: Side) -> Option<DateFormatter> {
        self.date_formatters(side).into_iter().max()
    }

    /// Whether the build cuts ICU4X's date slice (`icu.blob`): when either
    /// side's formatter is `icu`. A build script cuts it for both builds, so
    /// it needs `mf2-build`'s `icu-blob` then; [`Features::date_slice_place`]
    /// says whether it goes into the catalog or the server-only table.
    pub fn cuts_date_slice(&self) -> bool {
        Side::ALL
            .into_iter()
            .any(|side| self.date_formatter(side) == Some(DateFormatter::Icu))
    }

    /// Whether a browser downloads this build's catalogs: a framework or
    /// host of the browser's side is on, or a browser date formatter is.
    /// Without one the catalogs have native readers alone, and keep every
    /// entry (`plan/08` §4.1).
    #[doc(hidden)]
    pub fn has_browser_side(&self) -> bool {
        self.date_families()
            .iter()
            .any(|family| family.side == Side::Browser)
            || self.date_formatter(Side::Browser).is_some()
    }

    /// Where ICU4X's date slice (`icu.blob`) goes (`plan/08` §4.1): into
    /// the catalog when the browser's formatter is `icu` or no browser
    /// downloads the catalog and native code's is; into the server-only
    /// table when native code's is `icu` and the browser's is not; nowhere
    /// when neither side's is. A native application's build keeps it in
    /// its catalog whatever this says (`Build`).
    pub fn date_slice_place(&self) -> Place {
        let icu = |side| self.date_formatter(side) == Some(DateFormatter::Icu);
        if icu(Side::Browser) || (icu(Side::Native) && !self.has_browser_side()) {
            Place::Catalog
        } else if icu(Side::Native) {
            Place::Server
        } else {
            Place::Nowhere
        }
    }

    /// Where the number entries go (`plan/08` §4.1): `plural.cardinal`,
    /// `plural.ordinal`, `number.symbols`, `number.patterns`,
    /// `currency.data` and `unit.data`. With `number-intl` the browser
    /// formats and selects through `Intl` and reads none of them, so when a
    /// browser downloads the catalog they go to the server-only table;
    /// otherwise, and when no browser does, into the catalog. Native code
    /// reads them on every build that needs them, so never nowhere. A
    /// native application's build keeps them in its catalog whatever this
    /// says (`Build`).
    pub fn number_place(&self) -> Place {
        if self.number_intl() && self.has_browser_side() {
            Place::Server
        } else {
            Place::Catalog
        }
    }

    /// Where `currency.data` and `unit.data` go (`plan/08` §6): with the
    /// number split (`intl-names`) the browser takes the currency and unit
    /// names from `Intl` and reads neither, so when a browser downloads the
    /// catalog they go to the server-only table; otherwise where the other
    /// number entries go ([`Features::number_place`]).
    pub fn names_place(&self) -> Place {
        if self.intl_names() && self.has_browser_side() {
            Place::Server
        } else {
            self.number_place()
        }
    }

    /// The date families of the frameworks that are on (`plan/08` §3.5),
    /// each with its side and whether this build is one of its builds:
    ///
    /// * `ssr`: the Leptos server's family, built here, and the browser's,
    ///   which the server's build script reads; `hydrate`: the same two, the
    ///   browser's built here; `csr`: the browser's alone;
    /// * `axum` (without `ssr`, whose family a Leptos server writes) and
    ///   `native` (or `ratatui`): their own, built here;
    /// * with none of these, `host-std` and `host-web`: the families of no
    ///   framework.
    ///
    /// Empty when nothing says which build this is (`mf2 check --features
    /// fn-number`, a library with no host).
    pub fn date_families(&self) -> Vec<DateFamily> {
        let family = |prefix, side, builds_here| DateFamily {
            prefix,
            side,
            builds_here,
        };
        let (ssr, hydrate, csr) = (self.has("ssr"), self.has("hydrate"), self.has("csr"));
        let mut families = Vec::new();
        if ssr || hydrate {
            families.push(family("leptos-server-datetime-", Side::Native, ssr));
        }
        if ssr || hydrate || csr {
            families.push(family(
                "leptos-client-datetime-",
                Side::Browser,
                hydrate || csr,
            ));
        }
        if self.has("axum") && !ssr {
            families.push(family("axum-datetime-", Side::Native, true));
        }
        if self.has("native") || self.has("ratatui") {
            families.push(family("native-datetime-", Side::Native, true));
        }
        if families.is_empty() {
            if self.has("host-std") {
                families.push(family("host-std-datetime-", Side::Native, true));
            }
            if self.has("host-web") {
                families.push(family("host-web-datetime-", Side::Browser, true));
            }
        }
        families
    }

    /// The sides this build formats dates on and has no formatter for, in
    /// [`Side::ALL`]'s order. Empty when every side it builds has one, and
    /// when no framework or host says which sides it builds.
    #[doc(hidden)]
    pub fn sides_without_formatter(&self) -> Vec<Side> {
        let families = self.date_families();
        Side::ALL
            .into_iter()
            .filter(|&side| {
                families
                    .iter()
                    .any(|family| family.builds_here && family.side == side)
                    && self.date_formatter(side).is_none()
            })
            .collect()
    }

    /// Whether a date function can format in this build (`plan/08` §3.3):
    /// every side it builds has a formatter; with no framework or host on,
    /// some side has one. `datetime` alone has none.
    pub fn formats_dates(&self) -> bool {
        if self.date_families().iter().any(|family| family.builds_here) {
            self.sides_without_formatter().is_empty()
        } else {
            Side::ALL
                .into_iter()
                .any(|side| self.date_formatter(side).is_some())
        }
    }

    /// The date features on for `side`, as a crate writes them: the
    /// frameworks' features, and a host family's only for a formatter no
    /// framework's feature names (the frameworks' families are written as
    /// the host's, so both are on).
    #[doc(hidden)]
    pub fn date_features_on(&self, side: Side) -> Vec<String> {
        let Some((host, frameworks)) = side.families().split_first() else {
            return Vec::new();
        };
        let mut on = Vec::new();
        for formatter in side.formatters() {
            let named: Vec<String> = frameworks
                .iter()
                .map(|prefix| format!("{prefix}{}", formatter.name()))
                .filter(|feature| self.has(feature))
                .collect();
            if named.is_empty() {
                let feature = format!("{host}{}", formatter.name());
                if self.has(&feature) {
                    on.push(feature);
                }
            } else {
                on.extend(named);
            }
        }
        on
    }

    /// The date features to add (`plan/08` §3.5): for each family of the
    /// frameworks that are on whose side has no formatter, the side's
    /// recommended one.
    #[doc(hidden)]
    pub fn missing_date_features(&self) -> Vec<String> {
        let mut missing: Vec<String> = Vec::new();
        for family in self.date_families() {
            let feature = family.recommended();
            if self.date_formatter(family.side).is_none() && !missing.contains(&feature) {
                missing.push(feature);
            }
        }
        missing
    }

    /// The date features on whose framework is not (`plan/08` §3.5): a
    /// Leptos family without Leptos, `axum-datetime-` without `axum`,
    /// `native-datetime-` without `native`. The host families need none.
    #[doc(hidden)]
    pub fn date_features_without_framework(&self) -> Vec<String> {
        let leptos = self.leptos_on();
        let frameworks = [
            ("leptos-client-datetime-", Side::Browser, leptos),
            ("leptos-server-datetime-", Side::Native, leptos),
            ("axum-datetime-", Side::Native, self.has("axum")),
            (
                "native-datetime-",
                Side::Native,
                self.has("native") || self.has("ratatui"),
            ),
        ];
        let mut off = Vec::new();
        for (prefix, side, on) in frameworks {
            if on {
                continue;
            }
            for formatter in side.formatters() {
                let feature = format!("{prefix}{}", formatter.name());
                if self.has(&feature) {
                    off.push(feature);
                }
            }
        }
        off
    }

    /// The build's refusal when it cuts ICU4X's date slice and `mf2-build`
    /// lacks `icu-blob` (`plan/08` §3.5): [`Error::IcuBlob`], naming the
    /// `icu` features that are on. The build script asks it with its own
    /// features; `mf2 check` with those the application's manifest gives
    /// its `mf2-build` build-dependency, so both say the same.
    ///
    /// [`Error::IcuBlob`]: crate::Error::IcuBlob
    #[doc(hidden)]
    pub fn check_icu_blob(&self, icu_blob: bool) -> crate::Result<()> {
        if !self.cuts_date_slice() || icu_blob {
            return Ok(());
        }
        let named: Vec<String> = self
            .icu_date_features()
            .iter()
            .map(|name| format!("`{name}`"))
            .collect();
        Err(crate::Error::IcuBlob {
            features: named.join(" and "),
        })
    }

    /// Whether a Leptos feature is on.
    fn leptos_on(&self) -> bool {
        ["leptos", "leptos-0-8", "ssr", "hydrate", "csr"]
            .iter()
            .any(|name| self.has(name))
    }

    /// The lines of [`DATE_LINES`] for the kinds of application whose
    /// framework is on (`plan/08` §3.5): Leptos's; a command-line tool's or
    /// terminal UI's (`native`, `ratatui`); an Axum server's (without
    /// Leptos, whose server writes its own); and, with none of those but a
    /// host, the framework-free one. Every line when nothing says which kind
    /// of application this is.
    #[doc(hidden)]
    pub fn date_lines(&self) -> Vec<(&'static str, &'static [&'static str])> {
        let leptos = self.leptos_on();
        let native = self.has("native") || self.has("ratatui");
        let axum = self.has("axum") && !leptos;
        let host = !(leptos || native || axum) && (self.has("host-std") || self.has("host-web"));
        let on = [leptos, native, axum, host];
        if !on.contains(&true) {
            return DATE_LINES.to_vec();
        }
        DATE_LINES
            .iter()
            .zip(on)
            .filter(|(_, on)| *on)
            .map(|(line, _)| *line)
            .collect()
    }

    /// The date features that make a side's formatter ICU4X, for the error
    /// that asks for `mf2-build`'s `icu-blob`.
    #[doc(hidden)]
    pub fn icu_date_features(&self) -> Vec<String> {
        Side::ALL
            .into_iter()
            .flat_map(|side| self.date_features_on(side))
            .filter(|feature| feature.ends_with("-icu"))
            .collect()
    }

    /// The `gated-function` message for the date function `function`
    /// (`plan/08` §3.3): the sides without a formatter, the features to
    /// write, the families of the frameworks that are on, and what each
    /// formatter costs.
    pub fn no_date_formatter(&self, function: &str) -> String {
        let sides = self.sides_without_formatter();
        let mut out = format!(":{function} formats a date, and this build has no date formatter");
        let costed: Vec<Side> = if sides.is_empty() {
            out.push_str(
                "; a message may never add formatting code by itself. Write the \
                 features for the builds of this crate on the `mf2` dependency: ",
            );
            let lines: Vec<String> = self
                .date_lines()
                .iter()
                .map(|(what, features)| {
                    let features: Vec<String> = features.iter().map(|&f| f.to_owned()).collect();
                    format!("for {what}, {}", quoted(&features))
                })
                .collect();
            out.push_str(&lines.join("; "));
            out.push('.');
            Side::ALL.to_vec()
        } else {
            let names: Vec<&str> = sides.iter().map(|side| side.name()).collect();
            let _ = write!(out, " for {}", names.join(" or "));
            let elsewhere: Vec<String> = Side::ALL
                .into_iter()
                .filter(|side| !sides.contains(side))
                .flat_map(|side| self.date_features_on(side))
                .collect();
            if !elsewhere.is_empty() {
                let _ = write!(
                    out,
                    " ({} formats on the other side only)",
                    quoted(&elsewhere)
                );
            }
            let _ = write!(
                out,
                "; a message may never add formatting code by itself. Write {} on \
                 the `mf2` dependency.",
                quoted(&self.missing_date_features())
            );
            let families: Vec<String> = self
                .date_families()
                .iter()
                .map(|family| format!("`{}*` ({})", family.prefix, family.side.name()))
                .collect();
            let _ = write!(
                out,
                " The date families of the frameworks on: {}.",
                families.join(", ")
            );
            sides
        };
        for side in costed {
            let formatters: Vec<String> = side
                .formatters()
                .iter()
                .map(|&formatter| {
                    let recommended = if formatter == side.recommended() {
                        ", recommended"
                    } else {
                        ""
                    };
                    format!(
                        "`{}` is {}: {}{recommended}",
                        formatter.name(),
                        formatter.what(),
                        formatter.cost(side)
                    )
                })
                .collect();
            let _ = write!(out, " In {}: {}.", side.name(), formatters.join("; "));
        }
        if self.has("datetime") && !self.formats_dates() {
            out.push_str(
                " `datetime` alone turns the date functions on with no formatter; \
                 a formatter's feature turns it on.",
            );
        }
        out
    }

    /// Numbers and plural selection through the browser's `Intl` on the
    /// client (`number-intl`, 2.0's `intl`). The server keeps the Rust path,
    /// so the number entries move to the server-only table
    /// ([`Features::number_place`]).
    pub fn number_intl(&self) -> bool {
        self.has("number-intl")
    }

    /// The number split (`plan/08` §6): `:currency` and `:unit` take their
    /// names from the browser's `Intl` on the client, the digits and plural
    /// selection staying in Rust (`mf2-fn-number`'s and `mf2-host-web`'s
    /// `intl-names`, which the i18n crate forwards under the same name). The
    /// currency and unit entries move to the server-only table
    /// ([`Features::names_place`]).
    pub fn intl_names(&self) -> bool {
        self.has("intl-names")
    }

    /// Whether this build provides the built-in function `identifier` (an
    /// MF2 identifier without its `:`), or `None` if it is not a built-in —
    /// a custom function, which `[functions]` in `mf2.toml` answers for.
    pub fn provides(&self, identifier: &str) -> Option<bool> {
        let (_, gate) = BUILTINS.iter().find(|(name, _)| *name == identifier)?;
        Some(match gate {
            None => true,
            Some(feature) => self.gate_on(feature),
        })
    }

    /// Whether a [`BUILTINS`] gate is on: `datetime` when a formatter
    /// formats on every side this build builds ([`Features::formats_dates`];
    /// `datetime` alone is not enough), any other by its own.
    fn gate_on(&self, feature: &str) -> bool {
        match feature {
            "datetime" => self.formats_dates(),
            _ => self.has(feature),
        }
    }

    /// The feature `identifier` needs and this build does not have.
    pub fn missing_feature(&self, identifier: &str) -> Option<&'static str> {
        let (_, gate) = BUILTINS.iter().find(|(name, _)| *name == identifier)?;
        gate.filter(|feature| !self.gate_on(feature))
    }
}

/// `` `a`, `b` ``.
fn quoted(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::Features;
    use super::{DateFormatter, Place, Side};

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
    fn a_formatter_alone_turns_the_date_functions_on() {
        for list in [
            "native-datetime-icu",
            "leptos-client-datetime-intl",
            "host-web-datetime-iso",
        ] {
            let features = Features::parse(list);
            assert!(features.fn_datetime(), "{list}");
            assert_eq!(features.provides("time"), Some(true), "{list}");
            assert_eq!(features.missing_feature("datetime"), None, "{list}");
        }
        assert_eq!(
            Features::default().missing_feature("date"),
            Some("datetime")
        );
        assert!(Features::parse("datetime").fn_datetime());
        assert_eq!(
            Features::parse("datetime").date_formatter(Side::Native),
            None
        );
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
        let features = Features::parse("fn-number, mf2/datetime native-datetime-icu");
        assert!(features.fn_number());
        assert!(features.fn_datetime());
        assert_eq!(
            features.date_formatter(Side::Native),
            Some(DateFormatter::Icu)
        );
    }

    #[test]
    fn only_some_features_change_a_catalog() {
        let features = Features::parse(
            "default,ssr,number-intl,datetime,fn-number,host-web-datetime-intl,leptos-client-datetime-intl",
        );
        assert_eq!(
            features.for_catalogs().names().collect::<Vec<_>>(),
            ["datetime", "fn-number", "number-intl"]
        );
        // Two spellings of one build compare equal; an `icu` formatter on
        // either side adds the slice.
        assert_eq!(
            Features::parse("native-datetime-icu").for_catalogs(),
            Features::parse("datetime,host-std-datetime-icu").for_catalogs()
        );
        assert_eq!(
            Features::parse("leptos-client-datetime-icu")
                .for_catalogs()
                .names()
                .collect::<Vec<_>>(),
            ["datetime", "icu-blob"]
        );
        // ICU4X on the server alone puts the slice in the server-only
        // table: the browser's catalog is the one built with no native
        // formatter.
        assert_eq!(
            Features::parse("ssr,leptos-client-datetime-intl,leptos-server-datetime-icu")
                .for_catalogs(),
            Features::parse("ssr,leptos-client-datetime-intl").for_catalogs()
        );
    }

    // `plan/08` §4.1: where the number entries go.

    #[test]
    fn the_number_entries_go_where_they_are_read() {
        let place = |list| Features::parse(list).number_place();
        // `number-intl` with a browser side: the browser reads none of them.
        for list in [
            "ssr,fn-number,number-intl",
            "hydrate,fn-number,number-intl",
            "csr,fn-number,number-intl",
            "host-web,fn-number,number-intl",
            "fn-number,number-intl,host-std-datetime-icu,host-web-datetime-intl",
        ] {
            assert_eq!(place(list), Place::Server, "{list}");
        }
        // Without it the browser formats them in Rust.
        for list in [
            "ssr,fn-number",
            "hydrate,fn-number",
            "csr,fn-number",
            "host-web,fn-number",
        ] {
            assert_eq!(place(list), Place::Catalog, "{list}");
        }
        // No browser side: one reader, one file.
        for list in [
            "native,fn-number,number-intl",
            "axum,fn-number,number-intl",
            "host-std,fn-number,number-intl",
        ] {
            assert_eq!(place(list), Place::Catalog, "{list}");
        }
        // The browser-side choice is one of the catalog's features, so a
        // site built with and without it does not compare equal.
        assert_ne!(
            Features::parse("csr,fn-number,number-intl").for_catalogs(),
            Features::parse("csr,fn-number").for_catalogs()
        );
        assert_eq!(
            Features::parse("native,fn-number,number-intl").for_catalogs(),
            Features::parse("native,fn-number").for_catalogs()
        );
    }

    // `plan/08` §6: the number split moves the currency and unit entries alone.

    #[test]
    fn the_names_go_where_they_are_read() {
        let place = |list: &str| {
            let f = Features::parse(list);
            (f.number_place(), f.names_place())
        };
        for list in [
            "ssr,fn-number,intl-names",
            "hydrate,fn-number,intl-names",
            "csr,fn-number,intl-names",
            "host-web,fn-number,intl-names",
        ] {
            assert_eq!(place(list), (Place::Catalog, Place::Server), "{list}");
        }
        // The whole option already moves them with the rest.
        assert_eq!(
            place("csr,fn-number,number-intl,intl-names"),
            (Place::Server, Place::Server)
        );
        // Without it, or with no browser side, they stay with the rest.
        for list in [
            "csr,fn-number",
            "native,fn-number,intl-names",
            "axum,fn-number,intl-names",
        ] {
            assert_eq!(place(list), (Place::Catalog, Place::Catalog), "{list}");
        }
        assert_ne!(
            Features::parse("csr,fn-number,intl-names").for_catalogs(),
            Features::parse("csr,fn-number").for_catalogs()
        );
        assert_eq!(
            Features::parse("native,fn-number,intl-names").for_catalogs(),
            Features::parse("native,fn-number").for_catalogs()
        );
    }

    // `plan/08` §4.1: where the date slice goes.

    #[test]
    fn the_date_slice_goes_where_it_is_read() {
        let place = |list| Features::parse(list).date_slice_place();
        assert_eq!(
            place("ssr,leptos-client-datetime-intl,leptos-server-datetime-icu"),
            Place::Server
        );
        assert_eq!(
            place("ssr,leptos-client-datetime-icu,leptos-server-datetime-icu"),
            Place::Catalog
        );
        assert_eq!(
            place("hydrate,leptos-client-datetime-icu,leptos-server-datetime-iso"),
            Place::Catalog
        );
        // No browser side: one reader, one file.
        assert_eq!(place("native,native-datetime-icu"), Place::Catalog);
        assert_eq!(place("axum,axum-datetime-icu"), Place::Catalog);
        assert_eq!(place("host-std,host-std-datetime-icu"), Place::Catalog);
        // A browser formatter says there is a browser side.
        assert_eq!(
            place("host-std-datetime-icu,host-web-datetime-intl"),
            Place::Server
        );
        assert_eq!(
            place("ssr,leptos-client-datetime-intl,leptos-server-datetime-iso"),
            Place::Nowhere
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

        let both = Features::parse("fn-number,native-datetime-iso");
        assert_eq!(both.provides("percent"), Some(true));
        assert_eq!(both.provides("time"), Some(true));
        assert_eq!(both.missing_feature("unit"), None);
    }

    // `plan/08` §3.3 and §3.5: a build with dates and no formatter.

    #[test]
    fn datetime_alone_formats_no_date() {
        let alone = Features::parse("fn-number,datetime");
        assert!(alone.fn_datetime(), "the date code still links");
        assert!(!alone.formats_dates());
        assert_eq!(alone.provides("date"), Some(false));
        assert_eq!(alone.missing_feature("time"), Some("datetime"));
        assert!(
            alone
                .no_date_formatter("date")
                .contains("`datetime` alone turns the date functions on")
        );
    }

    #[test]
    fn a_server_rendered_build_with_only_a_client_formatter_has_no_dates() {
        let ssr = Features::parse("leptos,ssr,leptos-client-datetime-intl");
        assert_eq!(ssr.sides_without_formatter(), [Side::Native]);
        assert!(!ssr.formats_dates());
        assert_eq!(ssr.provides("datetime"), Some(false));
        assert_eq!(ssr.missing_date_features(), ["leptos-server-datetime-icu"]);
        let message = ssr.no_date_formatter("datetime");
        assert!(message.starts_with(":datetime formats a date"), "{message}");
        assert!(message.contains("for native code"), "{message}");
        assert!(
            message.contains("Write `leptos-server-datetime-icu` on the `mf2` dependency"),
            "{message}"
        );
        assert!(
            message.contains("`leptos-server-datetime-*` (native code)"),
            "{message}"
        );
        assert!(message.contains("+298 KB"), "{message}");
        assert!(message.contains("`icu` is ICU4X"), "{message}");
        assert!(message.contains("`iso` is the ISO stand-in"), "{message}");

        // The browser's build of the same application has its formatter.
        let hydrate = Features::parse("leptos,hydrate,leptos-client-datetime-intl");
        assert!(hydrate.formats_dates());
        assert_eq!(
            hydrate.missing_date_features(),
            ["leptos-server-datetime-icu"]
        );

        let both =
            Features::parse("leptos,ssr,leptos-client-datetime-intl,leptos-server-datetime-icu");
        assert!(both.formats_dates());
        assert!(both.missing_date_features().is_empty());
    }

    #[test]
    fn each_framework_names_its_own_family() {
        let prefixes = |list: &str| -> Vec<(&'static str, Side, bool)> {
            Features::parse(list)
                .date_families()
                .into_iter()
                .map(|f| (f.prefix, f.side, f.builds_here))
                .collect()
        };
        assert_eq!(
            prefixes("leptos,ssr,axum,host-std"),
            [
                ("leptos-server-datetime-", Side::Native, true),
                ("leptos-client-datetime-", Side::Browser, false),
            ]
        );
        assert_eq!(
            prefixes("leptos,csr,host-web"),
            [("leptos-client-datetime-", Side::Browser, true)]
        );
        assert_eq!(
            prefixes("axum,host-std"),
            [("axum-datetime-", Side::Native, true)]
        );
        assert_eq!(
            prefixes("ratatui"),
            [("native-datetime-", Side::Native, true)]
        );
        assert_eq!(
            prefixes("host-std,host-web"),
            [
                ("host-std-datetime-", Side::Native, true),
                ("host-web-datetime-", Side::Browser, true),
            ]
        );
        assert!(prefixes("fn-number").is_empty());
        assert_eq!(
            Features::parse("native").missing_date_features(),
            ["native-datetime-icu"]
        );
        assert_eq!(
            Features::parse("axum").missing_date_features(),
            ["axum-datetime-icu"]
        );
    }

    #[test]
    fn with_no_framework_the_message_names_every_line() {
        let none = Features::default();
        assert!(none.sides_without_formatter().is_empty());
        assert!(!none.formats_dates());
        let message = none.no_date_formatter("time");
        for (_, features) in super::DATE_LINES {
            for feature in features {
                assert!(message.contains(&format!("`{feature}`")), "{message}");
            }
        }
        assert!(message.contains("In the browser:"), "{message}");
        assert!(message.contains("In native code:"), "{message}");
        assert!(message.contains("+239 B gzipped"), "{message}");
        // With no framework, a formatter of either side is enough.
        assert!(Features::parse("host-web-datetime-iso").formats_dates());
    }

    #[test]
    fn the_date_lines_are_those_of_the_frameworks_that_are_on() {
        let kinds = |features: &str| -> Vec<&str> {
            Features::parse(features)
                .date_lines()
                .iter()
                .map(|(what, _)| *what)
                .collect()
        };
        let all: Vec<&str> = super::DATE_LINES.iter().map(|(what, _)| *what).collect();
        assert_eq!(kinds(""), all);
        assert_eq!(kinds("fn-number"), all);
        assert_eq!(kinds("leptos"), ["a server-rendered Leptos application"]);
        assert_eq!(kinds("ssr,axum"), ["a server-rendered Leptos application"]);
        assert_eq!(kinds("ratatui"), ["a command-line tool or a terminal UI"]);
        assert_eq!(kinds("axum"), ["an Axum server"]);
        assert_eq!(kinds("host-std"), ["no framework"]);
        let message = Features::parse("leptos").no_date_formatter("datetime");
        assert!(
            message.contains("`leptos-server-datetime-icu`"),
            "{message}"
        );
        assert!(!message.contains("`native-datetime-icu`"), "{message}");
    }

    #[test]
    fn date_features_are_named_as_written() {
        let features = Features::parse(
            "datetime,host-std-datetime-icu,host-std-datetime-iso,axum-datetime-icu,\
             host-web-datetime-intl",
        );
        assert_eq!(
            features.date_features_on(Side::Native),
            ["axum-datetime-icu", "host-std-datetime-iso"]
        );
        assert_eq!(
            features.date_features_on(Side::Browser),
            ["host-web-datetime-intl"]
        );
        assert_eq!(features.icu_date_features(), ["axum-datetime-icu"]);
        assert_eq!(
            features.date_features_without_framework(),
            ["axum-datetime-icu"]
        );
        let with = Features::parse("axum,axum-datetime-icu,ssr,leptos-client-datetime-intl");
        assert!(with.date_features_without_framework().is_empty());
        assert_eq!(
            Features::parse("native-datetime-iso,leptos-server-datetime-icu")
                .date_features_without_framework(),
            ["leptos-server-datetime-icu", "native-datetime-iso"]
        );
    }

    #[test]
    fn each_side_recommends_its_formatter() {
        assert_eq!(Side::Browser.recommended(), DateFormatter::Intl);
        assert_eq!(Side::Native.recommended(), DateFormatter::Icu);
        for side in Side::ALL {
            assert!(side.formatters().contains(&side.recommended()));
        }
    }
}
