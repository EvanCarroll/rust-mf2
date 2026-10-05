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

mod backend;
mod family;

pub use backend::{Backend, DateBackend};
pub use family::{Active, FAMILIES, Family, Framework, KINDS, Side, domain_features, kind_lines};

/// Which functions the built catalogs may use, and which locale data they
/// need.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features {
    names: BTreeSet<String>,
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
/// [`Features::number_place`] being [`Place::Server`]). These are the
/// browser-side choices, in the one list both builds see, so that a
/// server's build script knows what the browser reads (`plan/08` §4.1). The
/// wasm and the catalogs must agree on them; the other features (the `intl`
/// and `iso` formatters, `tzdb-bundled`, the host features) change only the
/// code a build compiles — `tzdb-bundled` only which IANA database a named
/// time zone is looked up in.
pub const CATALOG_FEATURES: [&str; 4] = ["fn-number", "datetime", "icu-blob", "number-intl"];

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
    /// the number entries go to the server-only table. What
    /// `mf2 compile --site` compares with the i18n crate's, so that two
    /// spellings of one build (a framework's family or the host's, with or
    /// without the `datetime` they imply) compare equal.
    #[must_use]
    pub fn for_catalogs(&self) -> Features {
        let [numbers, dates, slice, intl] = CATALOG_FEATURES;
        let on = [
            (numbers, self.fn_number()),
            (dates, self.fn_datetime()),
            (slice, self.date_slice_place() == Place::Catalog),
            (intl, self.number_place() == Place::Server),
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
        self.domain_on::<DateBackend>()
    }

    /// Whether `B`'s domain is on: the feature every backend of it turns
    /// on, or a backend's own — for a list written by hand, which does not
    /// spell out what a backend's feature implies.
    #[doc(hidden)]
    pub fn domain_on<B: Backend>(&self) -> bool {
        self.has(B::DOMAIN)
            || Side::ALL
                .into_iter()
                .any(|side| !self.on::<B>(side).is_empty())
    }

    /// Every backend of `B`'s domain on for `side`, the strongest first: one
    /// of its families' features. More than one is the
    /// `several-date-formatters` case. A backend whose feature a stronger
    /// one's turns on with it is that stronger one, not a second.
    #[doc(hidden)]
    pub fn on<B: Backend>(&self, side: Side) -> Vec<B> {
        let on: Vec<B> = B::offered(side)
            .iter()
            .copied()
            .filter(|backend| {
                side.families()
                    .any(|family| self.has(&family.feature(*backend)))
            })
            .collect();
        on.iter()
            .copied()
            .filter(|backend| {
                !on.iter()
                    .any(|stronger| stronger.implies() == Some(*backend))
            })
            .collect()
    }

    /// The backend `side`'s build formats `B`'s domain with: the strongest
    /// of its own side's on (for dates ICU4X, then `Intl`, then ISO),
    /// whatever the other side's say; `None` with none.
    pub fn backend<B: Backend>(&self, side: Side) -> Option<B> {
        self.on(side).into_iter().max()
    }

    /// Whether the build cuts ICU4X's date slice (`icu.blob`): when either
    /// side's formatter is `icu`. A build script cuts it for both builds, so
    /// it needs `mf2-build`'s `icu-blob` then; [`Features::date_slice_place`]
    /// says whether it goes into the catalog or the server-only table.
    pub fn cuts_date_slice(&self) -> bool {
        Side::ALL
            .into_iter()
            .any(|side| self.reads_date_slice(side))
    }

    /// Whether `side`'s date formatter formats from the date slice.
    fn reads_date_slice(&self, side: Side) -> bool {
        self.backend::<DateBackend>(side)
            .is_some_and(DateBackend::reads_slice)
    }

    /// Whether a browser downloads this build's catalogs: a framework or
    /// host of the browser's side is on, or a browser date formatter is —
    /// each of those names a browser build. Without one the catalogs have
    /// native readers alone, and keep every entry.
    #[doc(hidden)]
    pub fn has_browser_side(&self) -> bool {
        self.families()
            .iter()
            .any(|active| active.family.side == Side::Browser)
            || self.backend::<DateBackend>(Side::Browser).is_some()
    }

    /// Where ICU4X's date slice (`icu.blob`) goes: into the catalog when the
    /// browser's formatter is `icu` or no browser downloads the catalog and
    /// native code's is; into the server-only table when native code's is
    /// `icu` and the browser's is not; nowhere when neither side's is. A
    /// native application's build keeps it in its catalog whatever this says
    /// (`Build`).
    pub fn date_slice_place(&self) -> Place {
        let icu = |side| self.reads_date_slice(side);
        if icu(Side::Browser) || (icu(Side::Native) && !self.has_browser_side()) {
            Place::Catalog
        } else if icu(Side::Native) {
            Place::Server
        } else {
            Place::Nowhere
        }
    }

    /// Where the number entries go: `plural.cardinal`, `plural.ordinal`,
    /// `number.symbols`, `number.patterns`, `currency.data` and `unit.data`.
    /// With `number-intl` the browser formats and selects through `Intl` and
    /// reads none of them, so when a browser downloads the catalog they go
    /// to the server-only table; otherwise, and when no browser does, into
    /// the catalog. Native code reads them on every build that needs them,
    /// so never nowhere. A native application's build keeps them in its
    /// catalog whatever this says (`Build`).
    pub fn number_place(&self) -> Place {
        if self.number_intl() && self.has_browser_side() {
            Place::Server
        } else {
            Place::Catalog
        }
    }

    /// The families of the frameworks that are on, each with whether this
    /// build is one of its builds:
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
    pub fn families(&self) -> Vec<Active> {
        let active = |prefix: &str, builds_here| {
            FAMILIES
                .iter()
                .find(|family| family.prefix == prefix)
                .map(|family| Active {
                    family,
                    builds_here,
                })
        };
        let (ssr, hydrate, csr) = (self.has("ssr"), self.has("hydrate"), self.has("csr"));
        let mut families = Vec::new();
        if ssr || hydrate {
            families.extend(active("leptos-server-", ssr));
        }
        if ssr || hydrate || csr {
            families.extend(active("leptos-client-", hydrate || csr));
        }
        if self.has("axum") && !ssr {
            families.extend(active("axum-", true));
        }
        if self.has("native") || self.has("ratatui") {
            families.extend(active("native-", true));
        }
        if families.is_empty() {
            if self.has("host-std") {
                families.extend(active("host-std-", true));
            }
            if self.has("host-web") {
                families.extend(active("host-web-", true));
            }
        }
        families
    }

    /// The sides this build formats on and has no backend of `B`'s domain
    /// for, in [`Side::ALL`]'s order. Empty when every side it builds has
    /// one, and when no framework or host says which sides it builds.
    #[doc(hidden)]
    pub fn sides_without<B: Backend>(&self) -> Vec<Side> {
        let families = self.families();
        Side::ALL
            .into_iter()
            .filter(|&side| {
                families
                    .iter()
                    .any(|active| active.builds_here && active.family.side == side)
                    && self.backend::<B>(side).is_none()
            })
            .collect()
    }

    /// Whether a function of `B`'s domain can format in this build: every
    /// side it builds has a backend; with no framework or host on, some side
    /// has one. The domain's own feature alone has none.
    pub fn formats<B: Backend>(&self) -> bool {
        if self.families().iter().any(|active| active.builds_here) {
            self.sides_without::<B>().is_empty()
        } else {
            Side::ALL
                .into_iter()
                .any(|side| self.backend::<B>(side).is_some())
        }
    }

    /// The features that turn `backend` on for `side`, as a crate writes
    /// them: the frameworks' features, and the host family's only when no
    /// framework's names it (the frameworks' families are written as the
    /// host's, so both are on).
    fn features_of<B: Backend>(&self, side: Side, backend: B) -> Vec<String> {
        let mut families = side.families();
        let Some(host) = families.next() else {
            return Vec::new();
        };
        let named: Vec<String> = families
            .map(|family| family.feature(backend))
            .filter(|feature| self.has(feature))
            .collect();
        if named.is_empty() {
            let feature = host.feature(backend);
            if self.has(&feature) {
                return vec![feature];
            }
        }
        named
    }

    /// The features of `B`'s domain on for `side`, as a crate writes them
    /// ([`Features::features_of`]), the strongest backend's first.
    #[doc(hidden)]
    pub fn features_on<B: Backend>(&self, side: Side) -> Vec<String> {
        self.on::<B>(side)
            .into_iter()
            .flat_map(|backend| self.features_of(side, backend))
            .collect()
    }

    /// The features of `B`'s domain to add: for each family of the
    /// frameworks that are on whose side has no backend, the side's
    /// recommended one.
    #[doc(hidden)]
    pub fn missing<B: Backend>(&self) -> Vec<String> {
        let mut missing: Vec<String> = Vec::new();
        for active in self.families() {
            let feature = active.family.recommended::<B>();
            if self.backend::<B>(active.family.side).is_none() && !missing.contains(&feature) {
                missing.push(feature);
            }
        }
        missing
    }

    /// The features of `B`'s domain on whose framework is not: a Leptos
    /// family without Leptos, `axum-` without `axum`, `native-` without
    /// `native`. The host families need none.
    #[doc(hidden)]
    pub fn without_framework<B: Backend>(&self) -> Vec<String> {
        let leptos = self.leptos_on();
        let mut off = Vec::new();
        for family in &FAMILIES {
            let on = match family.framework {
                Framework::Host => true,
                Framework::Leptos => leptos,
                Framework::Axum => self.has("axum"),
                Framework::Native => self.has("native") || self.has("ratatui"),
            };
            if on {
                continue;
            }
            for &backend in B::offered(family.side) {
                let feature = family.feature(backend);
                if self.has(&feature) {
                    off.push(feature);
                }
            }
        }
        off
    }

    /// The build's refusal when it cuts ICU4X's date slice and `mf2-build`
    /// lacks `icu-blob`: [`Error::IcuBlob`], naming the `icu` features that
    /// are on. The build script asks it with its own features; `mf2 check`
    /// with those the application's manifest gives its `mf2-build`
    /// build-dependency, so both say the same.
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

    /// What to write for `B`'s domain, for the kinds of application whose
    /// framework is on ([`kind_lines`]): Leptos's; a command-line tool's or
    /// terminal UI's (`native`, `ratatui`); an Axum server's (without
    /// Leptos, whose server writes its own); and, with none of those but a
    /// host, the framework-free one. Every line when nothing says which kind
    /// of application this is.
    #[doc(hidden)]
    pub fn lines<B: Backend>(&self) -> Vec<(&'static str, Vec<String>)> {
        let leptos = self.leptos_on();
        let native = self.has("native") || self.has("ratatui");
        let axum = self.has("axum") && !leptos;
        let host = !(leptos || native || axum) && (self.has("host-std") || self.has("host-web"));
        let on = [leptos, native, axum, host];
        let lines = kind_lines::<B>();
        if !on.contains(&true) {
            return lines;
        }
        lines
            .into_iter()
            .zip(on)
            .filter(|(_, on)| *on)
            .map(|(line, _)| line)
            .collect()
    }

    /// The date features that make a side's formatter ICU4X, for the error
    /// that asks for `mf2-build`'s `icu-blob`.
    #[doc(hidden)]
    pub fn icu_date_features(&self) -> Vec<String> {
        Side::ALL
            .into_iter()
            .flat_map(|side| {
                self.on::<DateBackend>(side)
                    .into_iter()
                    .filter(|backend| backend.reads_slice())
                    .flat_map(move |backend| self.features_of(side, backend))
            })
            .collect()
    }

    /// The `gated-function` message for `function`, a function of `B`'s
    /// domain: the sides without a backend, the features to write, the
    /// families of the frameworks that are on, and what each backend costs.
    pub fn refusal<B: Backend>(&self, function: &str) -> String {
        let sides = self.sides_without::<B>();
        let mut out = format!(
            ":{function} formats {}, and this build has no {}",
            B::THING,
            B::NOUN
        );
        let costed: Vec<Side> = if sides.is_empty() {
            out.push_str(
                "; a message may never add formatting code by itself. Write the \
                 features for the builds of this crate on the `mf2` dependency: ",
            );
            let lines: Vec<String> = self
                .lines::<B>()
                .iter()
                .map(|(what, features)| format!("for {what}, {}", quoted(features)))
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
                .flat_map(|side| self.features_on::<B>(side))
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
                quoted(&self.missing::<B>())
            );
            let families: Vec<String> = self
                .families()
                .iter()
                .map(|active| {
                    format!(
                        "`{}*` ({})",
                        active.family.stem::<B>(),
                        active.family.side.name()
                    )
                })
                .collect();
            let _ = write!(
                out,
                " The {} families of the frameworks on: {}.",
                B::ADJECTIVE,
                families.join(", ")
            );
            sides
        };
        for side in costed {
            let backends: Vec<String> = B::offered(side)
                .iter()
                .map(|&backend| {
                    let recommended = if backend == B::recommended(side) {
                        ", recommended"
                    } else {
                        ""
                    };
                    format!(
                        "`{}` is {}: {}{recommended}",
                        backend.name(),
                        backend.what(),
                        backend.cost(side)
                    )
                })
                .collect();
            let _ = write!(out, " In {}: {}.", side.name(), backends.join("; "));
        }
        if self.has(B::DOMAIN) && !self.formats::<B>() {
            let _ = write!(
                out,
                " `{}` alone turns the {} functions on with no formatter; a \
                 formatter's feature turns it on.",
                B::DOMAIN,
                B::ADJECTIVE
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
    /// formats on every side this build builds ([`Features::formats`];
    /// `datetime` alone is not enough), any other by its own.
    fn gate_on(&self, feature: &str) -> bool {
        match feature {
            "datetime" => self.formats::<DateBackend>(),
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
    use super::{Backend, DateBackend, Place, Side};

    // `plan/08` §3.2, on each side: the formatter is chosen from the
    // features of its own side only; with more than one of a family on, the
    // strongest formats; with two frameworks that disagree, the strongest
    // formats; and a feature only adds.

    #[test]
    fn each_side_reads_only_its_own_features() {
        let split = Features::parse("leptos-client-datetime-intl,leptos-server-datetime-icu");
        assert_eq!(
            split.backend::<DateBackend>(Side::Browser),
            Some(DateBackend::Intl)
        );
        assert_eq!(
            split.backend::<DateBackend>(Side::Native),
            Some(DateBackend::Icu)
        );
        assert_eq!(split.on::<DateBackend>(Side::Browser), [DateBackend::Intl]);
        assert_eq!(split.on::<DateBackend>(Side::Native), [DateBackend::Icu]);

        let browser_only = Features::parse("host-web-datetime-icu");
        assert_eq!(
            browser_only.backend::<DateBackend>(Side::Browser),
            Some(DateBackend::Icu)
        );
        assert_eq!(browser_only.backend::<DateBackend>(Side::Native), None);

        let native_only = Features::parse("native-datetime-icu");
        assert_eq!(
            native_only.backend::<DateBackend>(Side::Native),
            Some(DateBackend::Icu)
        );
        assert_eq!(native_only.backend::<DateBackend>(Side::Browser), None);

        // `intl` is no formatter of the native families.
        let no_native_intl = Features::parse("native-datetime-intl,host-std-datetime-intl");
        assert_eq!(no_native_intl.backend::<DateBackend>(Side::Native), None);
        assert_eq!(no_native_intl.backend::<DateBackend>(Side::Browser), None);
    }

    #[test]
    fn two_of_one_family_the_strongest_formats_in_the_browser() {
        let icu_intl = Features::parse("leptos-client-datetime-intl,leptos-client-datetime-icu");
        assert_eq!(
            icu_intl.backend::<DateBackend>(Side::Browser),
            Some(DateBackend::Icu)
        );
        assert_eq!(
            icu_intl.on::<DateBackend>(Side::Browser),
            [DateBackend::Icu, DateBackend::Intl]
        );
        let intl_iso = Features::parse("host-web-datetime-iso,host-web-datetime-intl");
        assert_eq!(
            intl_iso.backend::<DateBackend>(Side::Browser),
            Some(DateBackend::Intl)
        );
        let all =
            Features::parse("host-web-datetime-iso,host-web-datetime-intl,host-web-datetime-icu");
        assert_eq!(
            all.backend::<DateBackend>(Side::Browser),
            Some(DateBackend::Icu)
        );
        assert_eq!(all.on::<DateBackend>(Side::Browser).len(), 3);
    }

    #[test]
    fn two_of_one_family_the_strongest_formats_natively() {
        let both = Features::parse("axum-datetime-iso,axum-datetime-icu");
        assert_eq!(
            both.backend::<DateBackend>(Side::Native),
            Some(DateBackend::Icu)
        );
        assert_eq!(
            both.on::<DateBackend>(Side::Native),
            [DateBackend::Icu, DateBackend::Iso]
        );
        assert_eq!(both.backend::<DateBackend>(Side::Browser), None);
    }

    #[test]
    fn two_frameworks_that_disagree_the_strongest_formats_natively() {
        // A command-line tool on ISO, with an optional web mode on ICU4X.
        let cli = Features::parse("native-datetime-iso");
        assert_eq!(
            cli.backend::<DateBackend>(Side::Native),
            Some(DateBackend::Iso)
        );
        assert!(!cli.cuts_date_slice());
        let with_web = Features::parse("native-datetime-iso,axum-datetime-icu");
        assert_eq!(
            with_web.backend::<DateBackend>(Side::Native),
            Some(DateBackend::Icu)
        );
        assert!(with_web.cuts_date_slice());
        let leptos = Features::parse("leptos-server-datetime-iso,native-datetime-icu");
        assert_eq!(
            leptos.backend::<DateBackend>(Side::Native),
            Some(DateBackend::Icu)
        );
    }

    #[test]
    fn two_frameworks_that_disagree_the_strongest_formats_in_the_browser() {
        let mixed = Features::parse("host-web-datetime-intl,leptos-client-datetime-iso");
        assert_eq!(
            mixed.backend::<DateBackend>(Side::Browser),
            Some(DateBackend::Intl)
        );
        let icu = Features::parse("host-web-datetime-icu,leptos-client-datetime-intl");
        assert_eq!(
            icu.backend::<DateBackend>(Side::Browser),
            Some(DateBackend::Icu)
        );
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
            Features::parse("datetime").backend::<DateBackend>(Side::Native),
            None
        );
    }

    #[test]
    fn date_formatters_are_ordered_weakest_first() {
        assert!(DateBackend::Icu > DateBackend::Intl);
        assert!(DateBackend::Intl > DateBackend::Iso);
        for side in Side::ALL {
            let names: Vec<_> = DateBackend::offered(side)
                .iter()
                .map(|f| f.name())
                .collect();
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
            features.backend::<DateBackend>(Side::Native),
            Some(DateBackend::Icu)
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
        assert!(!alone.formats::<DateBackend>());
        assert_eq!(alone.provides("date"), Some(false));
        assert_eq!(alone.missing_feature("time"), Some("datetime"));
        assert!(
            alone
                .refusal::<DateBackend>("date")
                .contains("`datetime` alone turns the date functions on")
        );
    }

    #[test]
    fn a_server_rendered_build_with_only_a_client_formatter_has_no_dates() {
        let ssr = Features::parse("leptos,ssr,leptos-client-datetime-intl");
        assert_eq!(ssr.sides_without::<DateBackend>(), [Side::Native]);
        assert!(!ssr.formats::<DateBackend>());
        assert_eq!(ssr.provides("datetime"), Some(false));
        assert_eq!(ssr.missing::<DateBackend>(), ["leptos-server-datetime-icu"]);
        let message = ssr.refusal::<DateBackend>("datetime");
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
        assert!(hydrate.formats::<DateBackend>());
        assert_eq!(
            hydrate.missing::<DateBackend>(),
            ["leptos-server-datetime-icu"]
        );

        let both =
            Features::parse("leptos,ssr,leptos-client-datetime-intl,leptos-server-datetime-icu");
        assert!(both.formats::<DateBackend>());
        assert!(both.missing::<DateBackend>().is_empty());
    }

    #[test]
    fn each_framework_names_its_own_family() {
        let prefixes = |list: &str| -> Vec<(&'static str, Side, bool)> {
            Features::parse(list)
                .families()
                .into_iter()
                .map(|f| (f.family.prefix, f.family.side, f.builds_here))
                .collect()
        };
        assert_eq!(
            prefixes("leptos,ssr,axum,host-std"),
            [
                ("leptos-server-", Side::Native, true),
                ("leptos-client-", Side::Browser, false),
            ]
        );
        assert_eq!(
            prefixes("leptos,csr,host-web"),
            [("leptos-client-", Side::Browser, true)]
        );
        assert_eq!(prefixes("axum,host-std"), [("axum-", Side::Native, true)]);
        assert_eq!(prefixes("ratatui"), [("native-", Side::Native, true)]);
        assert_eq!(
            prefixes("host-std,host-web"),
            [
                ("host-std-", Side::Native, true),
                ("host-web-", Side::Browser, true),
            ]
        );
        assert!(prefixes("fn-number").is_empty());
        assert_eq!(
            Features::parse("native").missing::<DateBackend>(),
            ["native-datetime-icu"]
        );
        assert_eq!(
            Features::parse("axum").missing::<DateBackend>(),
            ["axum-datetime-icu"]
        );
    }

    #[test]
    fn with_no_framework_the_message_names_every_line() {
        let none = Features::default();
        assert!(none.sides_without::<DateBackend>().is_empty());
        assert!(!none.formats::<DateBackend>());
        let message = none.refusal::<DateBackend>("time");
        for (_, features) in super::kind_lines::<DateBackend>() {
            for feature in features {
                assert!(message.contains(&format!("`{feature}`")), "{message}");
            }
        }
        assert!(message.contains("In the browser:"), "{message}");
        assert!(message.contains("In native code:"), "{message}");
        assert!(message.contains("+239 B gzipped"), "{message}");
        // With no framework, a formatter of either side is enough.
        assert!(Features::parse("host-web-datetime-iso").formats::<DateBackend>());
    }

    #[test]
    fn the_date_lines_are_those_of_the_frameworks_that_are_on() {
        let kinds = |features: &str| -> Vec<&str> {
            Features::parse(features)
                .lines::<DateBackend>()
                .iter()
                .map(|(what, _)| *what)
                .collect()
        };
        let all: Vec<&str> = super::KINDS.iter().map(|(what, _)| *what).collect();
        assert_eq!(kinds(""), all);
        assert_eq!(kinds("fn-number"), all);
        assert_eq!(kinds("leptos"), ["a server-rendered Leptos application"]);
        assert_eq!(kinds("ssr,axum"), ["a server-rendered Leptos application"]);
        assert_eq!(kinds("ratatui"), ["a command-line tool or a terminal UI"]);
        assert_eq!(kinds("axum"), ["an Axum server"]);
        assert_eq!(kinds("host-std"), ["no framework"]);
        let message = Features::parse("leptos").refusal::<DateBackend>("datetime");
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
            features.features_on::<DateBackend>(Side::Native),
            ["axum-datetime-icu", "host-std-datetime-iso"]
        );
        assert_eq!(
            features.features_on::<DateBackend>(Side::Browser),
            ["host-web-datetime-intl"]
        );
        assert_eq!(features.icu_date_features(), ["axum-datetime-icu"]);
        assert_eq!(
            features.without_framework::<DateBackend>(),
            ["axum-datetime-icu"]
        );
        let with = Features::parse("axum,axum-datetime-icu,ssr,leptos-client-datetime-intl");
        assert!(with.without_framework::<DateBackend>().is_empty());
        assert_eq!(
            Features::parse("native-datetime-iso,leptos-server-datetime-icu")
                .without_framework::<DateBackend>(),
            ["leptos-server-datetime-icu", "native-datetime-iso"]
        );
    }

    #[test]
    fn the_table_gives_the_date_features_and_the_line_of_each_kind() {
        // Family by family, the weakest formatter first: the order `mf2
        // check` lists what is on in.
        assert_eq!(
            super::domain_features::<DateBackend>(),
            [
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
            ]
        );
        let lines = super::kind_lines::<DateBackend>();
        let lines: Vec<(&str, Vec<&str>)> = lines
            .iter()
            .map(|(what, features)| (*what, features.iter().map(String::as_str).collect()))
            .collect();
        assert_eq!(
            lines,
            [
                (
                    "a server-rendered Leptos application",
                    vec!["leptos-client-datetime-intl", "leptos-server-datetime-icu"]
                ),
                (
                    "a command-line tool or a terminal UI",
                    vec!["native-datetime-icu"]
                ),
                ("an Axum server", vec!["axum-datetime-icu"]),
                (
                    "no framework",
                    vec!["host-std-datetime-icu", "host-web-datetime-intl"]
                ),
            ]
        );
    }

    #[test]
    fn each_side_recommends_its_formatter() {
        assert_eq!(DateBackend::recommended(Side::Browser), DateBackend::Intl);
        assert_eq!(DateBackend::recommended(Side::Native), DateBackend::Icu);
        for side in Side::ALL {
            assert!(DateBackend::offered(side).contains(&DateBackend::recommended(side)));
        }
    }
}
