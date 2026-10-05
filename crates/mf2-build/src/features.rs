//! The client feature set.
//!
//! Features belong to the **application**, written once on its `mf2`
//! dependency and seen by its server's build and its wasm build alike. A
//! build script therefore reads them from `mf2` itself, as cargo resolved
//! them (`run.rs`), and `mf2-cli` from `--features` or from cargo; neither
//! reads `mf2.toml`, which would be a second place for them to disagree.

use std::collections::BTreeSet;
use std::fmt::Write as _;

mod backend;
mod costs;
mod family;
mod gate;

pub use backend::{Backend, DateBackend, NumberBackend};
pub use family::{Active, FAMILIES, Family, Framework, KINDS, Side, domain_features, kind_lines};
use gate::Ask;
pub use gate::{BUILTINS, Gate};

/// Which functions the built catalogs may use, and which locale data they
/// need.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features {
    names: BTreeSet<String>,
}

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

/// What of a feature set changes a catalog — the file a browser downloads:
/// which functions a message may call, and which LOCALE entries the catalog
/// carries ([`Features::for_catalogs`]). The wasm and the catalogs must
/// agree on them. Two spellings of one build (a framework's family or the
/// host's, with or without the feature they imply) give the same, and so do
/// two builds that differ only in code: the `intl` and `iso` date
/// formatters, `tzdb-bundled`, the host features.
// Each flag is an independent fact about the catalog.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogContent {
    /// A message may call `:number`, `:integer` and `:offset`, and select
    /// by plural rules.
    pub numbers: bool,
    /// A message may call `:percent`, `:currency` and `:unit`.
    pub localized_numbers: bool,
    /// A message may call `:datetime`, `:date` and `:time`.
    pub dates: bool,
    /// The catalog carries the plural rules.
    pub plural_rules: bool,
    /// The catalog carries the number data: symbols, patterns, currencies
    /// and units.
    pub number_data: bool,
    /// The catalog carries ICU4X's date slice (`icu.blob`).
    pub date_slice: bool,
}

impl CatalogContent {
    /// What is on, in words, for a message: `number functions`, `plural
    /// rules`.
    pub fn names(&self) -> Vec<&'static str> {
        [
            (self.numbers, "number functions"),
            (self.localized_numbers, "percent/currency/unit"),
            (self.dates, "date functions"),
            (self.plural_rules, "plural rules"),
            (self.number_data, "number data"),
            (self.date_slice, "date slice"),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| name)
        .collect()
    }
}

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

/// Where the build writes each LOCALE entry, by who reads it
/// ([`Features::placement`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The plural rules (`plural.cardinal`, `plural.ordinal`): read by a
    /// side whose number formatter is `builtin` or `plain`.
    pub plural: Place,
    /// The number data (`number.symbols`, `number.patterns`,
    /// `currency.data`, `unit.data`): read by a side whose number formatter
    /// is `builtin`.
    pub numbers: Place,
    /// ICU4X's date slice (`icu.blob`): read by a side whose date formatter
    /// is `icu`.
    pub dates: Place,
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

    /// What of this set changes a catalog a browser downloads
    /// ([`CatalogContent`]). What `mf2 compile --site` compares with the
    /// i18n crate's.
    #[must_use]
    pub fn for_catalogs(&self) -> CatalogContent {
        let placement = self.placement(false);
        CatalogContent {
            numbers: self.formats::<NumberBackend>(),
            localized_numbers: self.formats_with(Ask::LOCALIZED),
            dates: self.formats::<DateBackend>(),
            plural_rules: placement.plural == Place::Catalog,
            number_data: placement.numbers == Place::Catalog,
            date_slice: placement.dates == Place::Catalog,
        }
    }

    /// Whether `name` is on.
    pub fn has(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// The names, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
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
    /// `several-formatters` case. A backend whose feature a stronger
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
    /// of its own side's on (for numbers `builtin`, then `intl`, then
    /// `plain`; for dates ICU4X, then `Intl`, then ISO), whatever the other
    /// side's say; `None` with none.
    pub fn backend<B: Backend>(&self, side: Side) -> Option<B> {
        self.on(side).into_iter().max()
    }

    /// Whether the build cuts ICU4X's date slice (`icu.blob`): when either
    /// side's formatter is `icu`. A build script cuts it for both builds, so
    /// it needs `mf2-build`'s `icu-blob` then; [`Features::placement`] says
    /// whether it goes into the catalog or the server-only table.
    pub fn cuts_date_slice(&self) -> bool {
        self.read_by(DateBackend::reads_slice)
    }

    /// Whether the build cuts the plural rules: when either side's number
    /// formatter selects from them (`builtin`, `plain`).
    #[doc(hidden)]
    pub fn cuts_plural_rules(&self) -> bool {
        self.read_by(NumberBackend::reads_plural_rules)
    }

    /// Whether the build cuts the number data: when either side's number
    /// formatter is `builtin`.
    #[doc(hidden)]
    pub fn cuts_number_data(&self) -> bool {
        self.read_by(NumberBackend::reads_number_data)
    }

    /// Whether `side`'s backend of `B`'s domain reads what `reads` names.
    fn reads<B: Backend>(&self, side: Side, reads: fn(B) -> bool) -> bool {
        self.backend::<B>(side).is_some_and(reads)
    }

    /// Whether either side's does.
    fn read_by<B: Backend>(&self, reads: fn(B) -> bool) -> bool {
        Side::ALL.into_iter().any(|side| self.reads(side, reads))
    }

    /// Whether a browser downloads this build's catalogs: a framework or
    /// host of the browser's side is on, or a browser's number or date
    /// formatter is — each of those names a browser build. Without one the
    /// catalogs have native readers alone, and keep every entry.
    #[doc(hidden)]
    pub fn has_browser_side(&self) -> bool {
        self.families()
            .iter()
            .any(|active| active.family.side == Side::Browser)
            || self.backend::<NumberBackend>(Side::Browser).is_some()
            || self.backend::<DateBackend>(Side::Browser).is_some()
    }

    /// Where each LOCALE entry goes: into the catalog when the browser's
    /// formatter reads it, or when no browser downloads the catalog and
    /// native code's does; into the server-only table when native code's
    /// reads it and the browser's does not; nowhere when neither side's
    /// does. `native_application` is a build no browser downloads from,
    /// whatever its features say: what native code reads stays in its
    /// catalog, one reader and one file, and what only a browser would read
    /// is not written (`plan/08` §4.2).
    pub fn placement(&self, native_application: bool) -> Placement {
        Placement {
            plural: self.place(native_application, NumberBackend::reads_plural_rules),
            numbers: self.place(native_application, NumberBackend::reads_number_data),
            dates: self.place(native_application, DateBackend::reads_slice),
        }
    }

    /// Where an entry goes that the backends `reads` names read.
    fn place<B: Backend>(&self, native_application: bool, reads: fn(B) -> bool) -> Place {
        let browser = self.reads(Side::Browser, reads);
        let native = self.reads(Side::Native, reads);
        if native_application {
            if native {
                Place::Catalog
            } else {
                Place::Nowhere
            }
        } else if browser || (native && !self.has_browser_side()) {
            Place::Catalog
        } else if native {
            Place::Server
        } else {
            Place::Nowhere
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
    /// compile`, a library with no host).
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
        self.sides_unable(Ask::<B>::ANY)
    }

    /// The sides this build formats on whose backend of `B`'s domain cannot
    /// do what `ask` asks — or that have none.
    fn sides_unable<B: Backend>(&self, ask: Ask<B>) -> Vec<Side> {
        let families = self.families();
        Side::ALL
            .into_iter()
            .filter(|&side| {
                families
                    .iter()
                    .any(|active| active.builds_here && active.family.side == side)
                    && !self.reads(side, ask.able)
            })
            .collect()
    }

    /// Whether a function of `B`'s domain can format in this build: every
    /// side it builds has a backend; with no framework or host on, some side
    /// has one. The domain's own feature alone has none.
    pub fn formats<B: Backend>(&self) -> bool {
        self.formats_with(Ask::<B>::ANY)
    }

    /// [`Features::formats`] for a function that asks more of its backend.
    fn formats_with<B: Backend>(&self, ask: Ask<B>) -> bool {
        if self.families().iter().any(|active| active.builds_here) {
            self.sides_unable(ask).is_empty()
        } else {
            self.read_by(ask.able)
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
        self.missing_for(Ask::<B>::ANY)
    }

    /// [`Features::missing`] for a function that asks more of its backend:
    /// a side whose backend cannot do it is told the recommended one too.
    fn missing_for<B: Backend>(&self, ask: Ask<B>) -> Vec<String> {
        let mut missing: Vec<String> = Vec::new();
        for active in self.families() {
            let feature = active.family.recommended::<B>();
            if !self.reads(active.family.side, ask.able) && !missing.contains(&feature) {
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
    /// domain that any of its backends formats: the sides without one, the
    /// features to write, the families of the frameworks that are on, and
    /// what each backend costs.
    pub fn refusal<B: Backend>(&self, function: &str) -> String {
        self.refusal_for(function, Ask::<B>::ANY)
    }

    /// The `gated-function` message for `function`, which asks `ask` of its
    /// domain's backend: the sides whose backend cannot, the features to
    /// write, the families of the frameworks that are on, and what each
    /// backend that can costs.
    fn refusal_for<B: Backend>(&self, function: &str, ask: Ask<B>) -> String {
        let sides = self.sides_unable(ask);
        let mut out = format!(
            ":{function} formats {}, and this build has no {}",
            ask.thing, ask.lacks
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
            // A side with a backend that cannot: what it is, so that the
            // message says why the feature that is on is not enough.
            let unable: Vec<String> = sides
                .iter()
                .filter_map(|&side| {
                    let backend = self.backend::<B>(side)?;
                    Some(format!(
                        "{} is {}",
                        quoted(&self.features_of(side, backend)),
                        backend.what()
                    ))
                })
                .collect();
            if !unable.is_empty() {
                let _ = write!(out, " ({})", unable.join("; "));
            }
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
                quoted(&self.missing_for(ask))
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
                .filter(|&&backend| (ask.able)(backend))
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

    /// Whether this build provides the built-in function `identifier` (an
    /// MF2 identifier without its `:`), or `None` if it is not a built-in —
    /// a custom function, which `[functions]` in `mf2.toml` answers for.
    pub fn provides(&self, identifier: &str) -> Option<bool> {
        let (_, gate) = BUILTINS.iter().find(|(name, _)| *name == identifier)?;
        Some(match gate {
            Gate::None => true,
            Gate::Number => self.formats::<NumberBackend>(),
            Gate::LocalizedNumber => self.formats_with(Ask::LOCALIZED),
            Gate::Date => self.formats::<DateBackend>(),
        })
    }

    /// The `gated-function` message for the built-in function `identifier`,
    /// which this build does not provide: the sides that cannot format it,
    /// and the features to write. `None` if it is not a built-in, or needs
    /// nothing of the build.
    pub fn gated(&self, identifier: &str) -> Option<String> {
        let (_, gate) = BUILTINS.iter().find(|(name, _)| *name == identifier)?;
        match gate {
            Gate::None => None,
            Gate::Number => Some(self.refusal::<NumberBackend>(identifier)),
            Gate::LocalizedNumber => Some(self.refusal_for(identifier, Ask::LOCALIZED)),
            Gate::Date => Some(self.refusal::<DateBackend>(identifier)),
        }
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
    use super::{Backend, DateBackend, NumberBackend, Place, Side};

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
            assert!(features.domain_on::<DateBackend>(), "{list}");
            assert_eq!(features.provides("time"), Some(true), "{list}");
        }
        assert_eq!(Features::default().provides("date"), Some(false));
        assert!(Features::parse("datetime").domain_on::<DateBackend>());
        assert_eq!(
            Features::parse("datetime").backend::<DateBackend>(Side::Native),
            None
        );
    }

    #[test]
    fn date_formatters_are_ordered_weakest_first() {
        assert!(DateBackend::IcuCached > DateBackend::Icu);
        assert!(DateBackend::Icu > DateBackend::Intl);
        assert!(DateBackend::Intl > DateBackend::Iso);
        for (side, strongest) in [(Side::Browser, "icu-cached"), (Side::Native, "icu")] {
            let names: Vec<_> = DateBackend::offered(side)
                .iter()
                .map(|f| f.name())
                .collect();
            assert_eq!(names.first(), Some(&strongest));
            assert_eq!(names.last(), Some(&"iso"));
        }
    }

    #[test]
    fn the_cached_formatter_is_icu_and_one_formatter() {
        // As cargo resolves it: the feature turns `icu`'s on, and the host's.
        let cargo = Features::parse(
            "datetime,host-web-datetime-icu,host-web-datetime-icu-cached,\
             leptos-client-datetime-icu-cached",
        );
        assert_eq!(
            cargo.on::<DateBackend>(Side::Browser),
            [DateBackend::IcuCached]
        );
        assert_eq!(
            cargo.features_on::<DateBackend>(Side::Browser),
            ["leptos-client-datetime-icu-cached"]
        );
        assert!(cargo.cuts_date_slice());
        assert_eq!(cargo.placement(false).dates, Place::Catalog);
        assert_eq!(
            cargo.icu_date_features(),
            ["leptos-client-datetime-icu-cached"]
        );
        // Written by hand, alone: it formats, and reads the slice.
        let alone = Features::parse("csr,leptos-client-datetime-icu-cached");
        assert_eq!(
            alone.backend::<DateBackend>(Side::Browser),
            Some(DateBackend::IcuCached)
        );
        assert!(alone.formats::<DateBackend>() && alone.cuts_date_slice());
        // `icu` written beside it is the same formatter, not a second one.
        let both = Features::parse("leptos-client-datetime-icu,leptos-client-datetime-icu-cached");
        assert_eq!(
            both.on::<DateBackend>(Side::Browser),
            [DateBackend::IcuCached]
        );
        // Another formatter beside it is a second one, and it is the stronger.
        let with_intl =
            Features::parse("leptos-client-datetime-intl,leptos-client-datetime-icu-cached");
        assert_eq!(
            with_intl.on::<DateBackend>(Side::Browser),
            [DateBackend::IcuCached, DateBackend::Intl]
        );
        // Native code has no such feature: its `icu` always has the cache.
        assert_eq!(
            Features::parse("native-datetime-icu-cached").backend::<DateBackend>(Side::Native),
            None
        );
    }

    #[test]
    fn number_formatters_are_ordered_weakest_first() {
        assert!(NumberBackend::Builtin > NumberBackend::Intl);
        assert!(NumberBackend::Intl > NumberBackend::Plain);
        for side in Side::ALL {
            let names: Vec<_> = NumberBackend::offered(side)
                .iter()
                .map(|f| f.name())
                .collect();
            assert_eq!(names.first(), Some(&"builtin"));
            assert_eq!(names.last(), Some(&"plain"));
            // Only a browser has `Intl`.
            assert_eq!(names.contains(&"intl"), side == Side::Browser);
            // What the tools recommend writes the language's own form, so
            // it is the answer to every number function.
            assert!(NumberBackend::recommended(side).localizes());
        }
    }

    #[test]
    fn each_side_formats_numbers_with_its_own_strongest() {
        let number = |list: &str, side| Features::parse(list).backend::<NumberBackend>(side);
        let split = "leptos-client-number-intl,leptos-server-number-builtin";
        assert_eq!(number(split, Side::Browser), Some(NumberBackend::Intl));
        assert_eq!(number(split, Side::Native), Some(NumberBackend::Builtin));
        // `builtin`, then `intl`, then `plain`.
        assert_eq!(
            number(
                "host-web-number-intl,leptos-client-number-builtin",
                Side::Browser
            ),
            Some(NumberBackend::Builtin)
        );
        assert_eq!(
            number("host-web-number-plain,host-web-number-intl", Side::Browser),
            Some(NumberBackend::Intl)
        );
        assert_eq!(
            number("axum-number-plain,native-number-builtin", Side::Native),
            Some(NumberBackend::Builtin)
        );
        // One side's features say nothing of the other's.
        assert_eq!(number("native-number-builtin", Side::Browser), None);
        assert_eq!(number("host-web-number-builtin", Side::Native), None);
        // `intl` is no formatter of the native families, and `number`
        // alone is none at all.
        assert_eq!(
            number("native-number-intl,host-std-number-intl", Side::Native),
            None
        );
        let alone = Features::parse("number");
        assert!(alone.domain_on::<NumberBackend>());
        assert!(!alone.formats::<NumberBackend>());
    }

    #[test]
    fn cargo_feature_variables_become_feature_names() {
        let features = Features::from_vars([
            (
                "CARGO_FEATURE_NATIVE_NUMBER_BUILTIN".to_owned(),
                "1".to_owned(),
            ),
            ("CARGO_FEATURE_SSR".to_owned(), "1".to_owned()),
            ("PATH".to_owned(), "/usr/bin".to_owned()),
        ]);
        assert_eq!(
            features.backend::<NumberBackend>(Side::Native),
            Some(NumberBackend::Builtin)
        );
        assert!(features.has("ssr"));
        assert!(!features.domain_on::<DateBackend>());
        assert_eq!(
            features.names().collect::<Vec<_>>(),
            ["native-number-builtin", "ssr"]
        );
    }

    #[test]
    fn a_features_argument_reads_like_cargos() {
        let features = Features::parse("native-number-plain, mf2/datetime native-datetime-icu");
        assert_eq!(
            features.backend::<NumberBackend>(Side::Native),
            Some(NumberBackend::Plain)
        );
        assert!(features.domain_on::<DateBackend>());
        assert_eq!(
            features.backend::<DateBackend>(Side::Native),
            Some(DateBackend::Icu)
        );
    }

    #[test]
    fn only_some_features_change_a_catalog() {
        let content = |list: &str| Features::parse(list).for_catalogs();
        // A Leptos server: its own formatters say which functions a message
        // may call, and what only it reads is in no catalog a browser
        // downloads.
        assert_eq!(
            content(
                "default,ssr,number,datetime,leptos-client-number-intl,\
                 leptos-server-number-builtin,leptos-client-datetime-intl"
            )
            .names(),
            ["number functions", "percent/currency/unit"]
        );
        // Two spellings of one build compare equal.
        assert_eq!(
            content("native-datetime-icu"),
            content("datetime,host-std-datetime-icu")
        );
        assert_eq!(
            content("native-number-builtin"),
            content("number,host-std-number-builtin")
        );
        // What a browser's formatter reads is in its catalog.
        assert_eq!(
            content("leptos-client-datetime-icu").names(),
            ["date functions", "date slice"]
        );
        assert_eq!(
            content("leptos-client-number-builtin").names(),
            [
                "number functions",
                "percent/currency/unit",
                "plural rules",
                "number data"
            ]
        );
        assert_eq!(
            content("leptos-client-number-plain").names(),
            ["number functions", "plural rules"]
        );
        assert_eq!(
            content("leptos-client-number-intl").names(),
            ["number functions", "percent/currency/unit"]
        );
        // The `intl` and `iso` date formatters differ only in code.
        assert_eq!(
            content("csr,leptos-client-datetime-intl"),
            content("csr,leptos-client-datetime-iso")
        );
        // What the server alone reads goes to the server-only table: the
        // browser's catalog is the one built with no native formatter.
        assert_eq!(
            content("hydrate,leptos-client-datetime-intl,leptos-server-datetime-icu"),
            content("hydrate,leptos-client-datetime-intl")
        );
        assert_eq!(
            content("hydrate,leptos-client-number-intl,leptos-server-number-builtin"),
            content("hydrate,leptos-client-number-intl")
        );
        assert!(content("").names().is_empty());
    }

    // `plan/08` §4.1: where the plural rules and the number data go.

    #[test]
    fn the_number_entries_go_where_they_are_read() {
        let place = |list: &str| {
            let placement = Features::parse(list).placement(false);
            (placement.plural, placement.numbers)
        };
        // `intl` in the browser reads neither; the server's `builtin` reads
        // both.
        for list in [
            "ssr,leptos-client-number-intl,leptos-server-number-builtin",
            "hydrate,leptos-client-number-intl,leptos-server-number-builtin",
            "host-web-number-intl,host-std-number-builtin",
        ] {
            assert_eq!(place(list), (Place::Server, Place::Server), "{list}");
        }
        // `builtin` in the browser reads both.
        for list in [
            "ssr,leptos-client-number-builtin,leptos-server-number-builtin",
            "csr,leptos-client-number-builtin",
            "host-web,host-web-number-builtin",
        ] {
            assert_eq!(place(list), (Place::Catalog, Place::Catalog), "{list}");
        }
        // `plain` in the browser selects from the plural rules and reads no
        // number data.
        assert_eq!(
            place("hydrate,leptos-client-number-plain,leptos-server-number-builtin"),
            (Place::Catalog, Place::Server)
        );
        assert_eq!(
            place("csr,leptos-client-number-plain"),
            (Place::Catalog, Place::Nowhere)
        );
        // A browser on `intl` with no server: no side reads either.
        assert_eq!(
            place("csr,leptos-client-number-intl"),
            (Place::Nowhere, Place::Nowhere)
        );
        // No browser side: one reader, one file.
        for list in [
            "native,native-number-builtin",
            "axum,axum-number-builtin",
            "host-std,host-std-number-builtin",
        ] {
            assert_eq!(place(list), (Place::Catalog, Place::Catalog), "{list}");
        }
        assert_eq!(
            place("native,native-number-plain"),
            (Place::Catalog, Place::Nowhere)
        );
        // No number formatter: no entry.
        assert_eq!(place("ssr"), (Place::Nowhere, Place::Nowhere));
        // A native application keeps what native code reads in its catalog,
        // whatever its features say of a browser, and writes nothing a
        // browser alone would read.
        let native = |list: &str| {
            let placement = Features::parse(list).placement(true);
            (placement.plural, placement.numbers, placement.dates)
        };
        assert_eq!(
            native("native,hydrate,leptos-client-number-intl,native-number-builtin"),
            (Place::Catalog, Place::Catalog, Place::Nowhere)
        );
        assert_eq!(
            native(
                "native,native-number-plain,leptos-client-number-builtin,\
                 leptos-client-datetime-icu"
            ),
            (Place::Catalog, Place::Nowhere, Place::Nowhere)
        );
        assert_eq!(
            native("native,native-datetime-icu"),
            (Place::Nowhere, Place::Nowhere, Place::Catalog)
        );
        // The browser's choice changes its catalog, so a site built with
        // one and with another does not compare equal.
        assert_ne!(
            Features::parse("csr,leptos-client-number-intl").for_catalogs(),
            Features::parse("csr,leptos-client-number-builtin").for_catalogs()
        );
    }

    #[test]
    fn the_number_data_is_cut_when_a_side_reads_it() {
        let cuts = |list: &str| {
            let features = Features::parse(list);
            (features.cuts_plural_rules(), features.cuts_number_data())
        };
        assert_eq!(cuts(""), (false, false));
        assert_eq!(cuts("leptos-client-number-intl"), (false, false));
        assert_eq!(cuts("leptos-client-number-plain"), (true, false));
        assert_eq!(
            cuts("leptos-client-number-intl,leptos-server-number-plain"),
            (true, false)
        );
        assert_eq!(
            cuts("leptos-client-number-intl,leptos-server-number-builtin"),
            (true, true)
        );
    }

    // `plan/08` §4.1: where the date slice goes.

    #[test]
    fn the_date_slice_goes_where_it_is_read() {
        let place = |list: &str| Features::parse(list).placement(false).dates;
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
    fn gated_functions_need_their_formatter() {
        let none = Features::default();
        assert_eq!(none.provides("string"), Some(true));
        assert_eq!(none.provides("number"), Some(false));
        assert_eq!(none.provides("percent"), Some(false));
        assert_eq!(none.provides("date"), Some(false));
        assert_eq!(none.provides("app:emoji"), None);
        assert_eq!(none.gated("string"), None);
        assert_eq!(none.gated("app:emoji"), None);

        let plain = Features::parse("native,native-number-plain");
        assert_eq!(plain.provides("integer"), Some(true));
        assert_eq!(plain.provides("currency"), Some(false));

        let both = Features::parse("native-number-builtin,native-datetime-iso");
        assert_eq!(both.provides("percent"), Some(true));
        assert_eq!(both.provides("time"), Some(true));

        // A browser's `intl` writes the language's own form too.
        let intl = Features::parse("csr,leptos-client-number-intl");
        assert_eq!(intl.provides("unit"), Some(true));
    }

    #[test]
    fn a_number_function_with_no_formatter_names_the_features_to_write() {
        let ssr = Features::parse("leptos,ssr,leptos-client-number-intl");
        assert_eq!(ssr.sides_without::<NumberBackend>(), [Side::Native]);
        assert_eq!(ssr.provides("number"), Some(false));
        let message = ssr.gated("number").unwrap_or_default();
        assert!(
            message.starts_with(
                ":number formats a number, and this build has no number formatter for native \
                 code (`leptos-client-number-intl` formats on the other side only)"
            ),
            "{message}"
        );
        assert!(
            message.contains("Write `leptos-server-number-builtin` on the `mf2` dependency"),
            "{message}"
        );
        assert!(
            message.contains("`leptos-server-number-*` (native code)"),
            "{message}"
        );
        assert!(message.contains("`builtin` is mf2's own code"), "{message}");
        assert!(message.contains("`plain` is plain digits"), "{message}");
        assert!(!message.contains("`intl` is"), "{message}");

        // With nothing that says which builds the crate has: every line.
        let message = Features::default().gated("integer").unwrap_or_default();
        for (_, features) in super::kind_lines::<NumberBackend>() {
            for feature in features {
                assert!(message.contains(&format!("`{feature}`")), "{message}");
            }
        }
        assert!(
            message.contains("`intl` is the browser's Intl.NumberFormat"),
            "{message}"
        );
    }

    #[test]
    fn plain_digits_cannot_show_a_currency() {
        let plain = Features::parse("native,native-number-plain");
        assert!(plain.formats::<NumberBackend>());
        assert_eq!(plain.provides("currency"), Some(false));
        let message = plain.gated("currency").unwrap_or_default();
        assert!(
            message.starts_with(
                ":currency formats a number in the language's own form, and this build has no \
                 number formatter that writes one for native code (`native-number-plain` is \
                 plain digits)"
            ),
            "{message}"
        );
        assert!(
            message.contains("Write `native-number-builtin` on the `mf2` dependency"),
            "{message}"
        );
        assert!(message.contains("`builtin` is mf2's own code"), "{message}");
        // `plain` is not offered as an answer.
        assert!(!message.contains("`plain` is"), "{message}");
        assert!(!message.contains("alone turns"), "{message}");

        // A browser on `plain` beside a server on `builtin`: the browser's
        // build is the one refused.
        let split = "leptos,leptos-client-number-plain,leptos-server-number-builtin";
        assert_eq!(
            Features::parse(&format!("ssr,{split}")).provides("percent"),
            Some(true)
        );
        let hydrate = Features::parse(&format!("hydrate,{split}"));
        assert_eq!(hydrate.provides("percent"), Some(false));
        let message = hydrate.gated("percent").unwrap_or_default();
        assert!(message.contains("for the browser"), "{message}");
        assert!(
            message.contains("Write `leptos-client-number-intl` on the `mf2` dependency"),
            "{message}"
        );
    }

    // `plan/08` §3.3 and §3.5: a build with dates and no formatter.

    #[test]
    fn datetime_alone_formats_no_date() {
        let alone = Features::parse("datetime");
        assert!(
            alone.domain_on::<DateBackend>(),
            "the date code still links"
        );
        assert!(!alone.formats::<DateBackend>());
        assert_eq!(alone.provides("date"), Some(false));
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
        assert!(
            message.contains(&DateBackend::Icu.cost(Side::Native)),
            "{message}"
        );
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
        assert!(prefixes("compile").is_empty());
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
        assert!(
            message.contains(&DateBackend::Intl.cost(Side::Browser)),
            "{message}"
        );
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
        assert_eq!(kinds("compile"), all);
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
                "host-web-datetime-icu-cached",
                "leptos-client-datetime-iso",
                "leptos-client-datetime-intl",
                "leptos-client-datetime-icu",
                "leptos-client-datetime-icu-cached",
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
