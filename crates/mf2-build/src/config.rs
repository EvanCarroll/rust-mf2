//! `mf2.toml`: the one place a corpus is
//! configured, read by `build.rs` through [`crate::Build`] and by `mf2-cli`,
//! so the two always agree.
//!
//! ```toml
//! source_locale = "en"
//!
//! [fallback]                 # chains, flattened at build time (D5)
//! "es-MX" = ["es", "en"]
//!
//! [catalog]
//! strip = ["cold", "ids"]    # production client catalogs
//! missing = "fallback"       # fallback | id | empty
//!
//! [locale_data]
//! currencies = "used"        # "used" | "all" | ["USD", "EUR"]
//! units = "used"
//!
//! [dates]                    # the ICU4X date formatter's form (plan/08 §5.1)
//! calendars = "auto"         # "auto" | "gregorian" | "all"
//! zone-names = "auto"        # "auto" | true | false
//!
//! [lints]
//! neutral-numbers = "warn"
//!
//! [functions]                # custom functions for the generated registry
//! "app:emoji" = "my_app_i18n::functions::emoji"
//! ```
//!
//! The client feature set is *not* here: it is the i18n crate's own cargo
//! features, which [`Features`](crate::Features) reads.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mf2_locale_data::number::Selection;
use mf2_resource::LineIndex;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::lint::{Level, Lint};

/// The name of the file, beside the i18n crate's `Cargo.toml`.
pub const FILE_NAME: &str = "mf2.toml";

/// A corpus's configuration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[non_exhaustive]
pub struct Config {
    /// The locale the manifest and every lint compare against.
    pub source_locale: String,
    /// Per locale, the locales to take a missing message from, in order.
    /// Chains are flattened at build time (D5); the source locale is the
    /// implicit last resort.
    pub fallback: BTreeMap<String, Vec<String>>,
    /// What the per-locale catalogs carry.
    pub catalog: CatalogConfig,
    /// How much CLDR data a catalog carries.
    pub locale_data: LocaleDataConfig,
    /// The form of the ICU4X date formatter: which calendars and whether
    /// zone names.
    pub dates: DatesConfig,
    /// Lint levels, by lint name.
    pub lints: BTreeMap<Lint, Level>,
    /// Custom functions for the generated registry: MF2 identifier → the
    /// Rust path of a `&'static dyn Function`.
    pub functions: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            source_locale: "en".to_owned(),
            fallback: BTreeMap::new(),
            catalog: CatalogConfig::default(),
            locale_data: LocaleDataConfig::default(),
            dates: DatesConfig::default(),
            lints: BTreeMap::new(),
            functions: BTreeMap::new(),
        }
    }
}

/// `[catalog]`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[non_exhaustive]
pub struct CatalogConfig {
    /// Sections to leave out of the catalogs.
    pub strip: BTreeSet<Strip>,
    /// What a locale that lacks a message gets.
    pub missing: Missing,
}

impl Default for CatalogConfig {
    fn default() -> Self {
        CatalogConfig {
            strip: [Strip::Cold, Strip::Ids].into_iter().collect(),
            missing: Missing::Fallback,
        }
    }
}

/// A catalog section a production build leaves out.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Strip {
    /// Attributes, comments and the other cold data.
    Cold,
    /// The id table — the client formats by `MsgId`.
    Ids,
}

/// What a locale that lacks a message gets.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Missing {
    /// The fallback chain's text, flagged as a fallback (D5, F7).
    #[default]
    Fallback,
    /// The message's id, so a gap is visible in the page.
    Id,
    /// Nothing at all.
    Empty,
}

/// `[locale_data]`.
#[derive(Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[non_exhaustive]
pub struct LocaleDataConfig {
    /// Which currencies a catalog carries.
    pub currencies: DataSet,
    /// Which units a catalog carries.
    pub units: DataSet,
}

/// `[dates]`: the form of the ICU4X date formatter the build links and cuts
/// the date slice for (`plan/08` §5.1). By default the build works both
/// halves out from the corpus; `calendars = "all"` with `zone-names = true`
/// is the widest form, which formats whatever any message can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields, default, rename_all = "kebab-case")]
#[non_exhaustive]
pub struct DatesConfig {
    /// Which calendars the formatter shows.
    pub calendars: DateCalendars,
    /// Whether it shows zone names (`timeZoneStyle`).
    pub zone_names: ZoneNames,
}

/// `[dates] calendars`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DateCalendars {
    /// Gregorian only, unless a language prefers another calendar or a
    /// message names one, or takes `calendar` from a variable (`"auto"`).
    #[default]
    Auto,
    /// Gregorian only (`"gregorian"`): a date in another calendar is an
    /// *Unsupported Operation*, and a language that prefers another one is
    /// shown Gregorian dates.
    Gregorian,
    /// Every calendar ICU4X formats (`"all"`).
    All,
}

/// `[dates] zone-names`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ZoneNames {
    /// Only if a message has `timeZoneStyle` (`"auto"`).
    #[default]
    Auto,
    /// Always (`true`).
    Yes,
    /// Never (`false`): `timeZoneStyle` is an *Unsupported Operation*.
    No,
}

impl<'de> Deserialize<'de> for ZoneNames {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Flag(bool),
            Word(String),
        }
        match Raw::deserialize(d)? {
            Raw::Flag(true) => Ok(ZoneNames::Yes),
            Raw::Flag(false) => Ok(ZoneNames::No),
            Raw::Word(w) if w == "auto" => Ok(ZoneNames::Auto),
            Raw::Word(w) => Err(serde::de::Error::custom(format!(
                "expected \"auto\", true or false, not {w:?}"
            ))),
        }
    }
}

/// How much of one CLDR table a catalog carries.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum DataSet {
    /// Only what the corpus names in a literal option (`"used"`, the
    /// default). A non-literal option value makes it [`DataSet::All`]
    /// anyway, with a `dynamic-currency` / `dynamic-unit` warning.
    #[default]
    Used,
    /// Every one CLDR has (`"all"`).
    All,
    /// What the corpus names in a literal option, plus these. When an
    /// option is a variable, these are the codes it can hold: the catalog
    /// carries the list, not every code, and there is no warning.
    Listed(BTreeSet<String>),
}

impl DataSet {
    /// This set together with the codes the corpus was found to use.
    ///
    /// An explicit list wins over a variable's every-code: the developer's
    /// list says which codes the variable can hold.
    pub fn with_used(&self, used: &Selection) -> Selection {
        match (self, used) {
            (DataSet::All, _) => Selection::All,
            (DataSet::Used, used) => used.clone(),
            (DataSet::Listed(listed), Selection::All) => Selection::Listed(listed.clone()),
            (DataSet::Listed(listed), Selection::Listed(used)) => {
                Selection::Listed(listed.iter().chain(used).cloned().collect())
            }
        }
    }
}

impl<'de> Deserialize<'de> for DataSet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Word(String),
            List(BTreeSet<String>),
        }
        match Raw::deserialize(d)? {
            Raw::Word(w) if w == "used" => Ok(DataSet::Used),
            Raw::Word(w) if w == "all" => Ok(DataSet::All),
            Raw::Word(w) => Err(serde::de::Error::custom(format!(
                "expected \"used\", \"all\" or a list of codes, not {w:?}"
            ))),
            Raw::List(list) => Ok(DataSet::Listed(list)),
        }
    }
}

impl Config {
    /// Reads `<dir>/mf2.toml`, or the defaults if there is none.
    pub fn load(dir: &Path) -> Result<Config> {
        let path = dir.join(FILE_NAME);
        match std::fs::read_to_string(&path) {
            Ok(text) => Config::parse(&text, &path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(source) => Err(Error::io(path, source)),
        }
    }

    /// Reads a configuration from TOML, naming `path` in any error.
    pub fn parse(text: &str, path: &Path) -> Result<Config> {
        let config: Config = toml::from_str(text).map_err(|e| config_error(text, path, &e))?;
        config.validate(path)?;
        Ok(config)
    }

    /// Writes the configuration back as TOML (`mf2 init`).
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// What `lint` does in this configuration.
    pub fn level(&self, lint: Lint) -> Level {
        self.lints
            .get(&lint)
            .copied()
            .unwrap_or_else(|| lint.default_level())
    }

    /// The fallback chain of `locale`: the locales to look in after it, in
    /// order, ending at the source locale. A locale with no chain of its own
    /// falls back to its parent tags (`es-MX` → `es`) and then to the source.
    pub fn chain(&self, locale: &str) -> Vec<String> {
        let mut chain: Vec<String> = match self.fallback.get(locale) {
            Some(listed) => listed.clone(),
            None => truncations(locale),
        };
        if locale != self.source_locale && !chain.contains(&self.source_locale) {
            chain.push(self.source_locale.clone());
        }
        chain.retain(|l| l != locale);
        let mut seen = BTreeSet::new();
        chain.retain(|l| seen.insert(l.clone()));
        chain
    }

    fn validate(&self, path: &Path) -> Result<()> {
        let bad = |message: String| Error::Config {
            path: path.to_path_buf(),
            message,
        };
        if self.source_locale.is_empty() {
            return Err(bad("source_locale must be a BCP 47 tag".to_owned()));
        }
        for (&lint, &level) in &self.lints {
            if level < lint.floor() {
                return Err(bad(format!(
                    "[lints] {lint} = {level:?}: this lint cannot be set below \
                     \"{}\" — the rest of the build relies on it",
                    lint.floor()
                )));
            }
        }
        for (locale, chain) in &self.fallback {
            if chain.iter().any(|l| l == locale) {
                return Err(bad(format!(
                    "[fallback] {locale:?}: a locale may not fall back to itself"
                )));
            }
        }
        for (identifier, path_to_fn) in &self.functions {
            if identifier.is_empty() || path_to_fn.is_empty() {
                return Err(bad(format!(
                    "[functions] {identifier:?}: needs a Rust path to a \
                     `&'static dyn Function`"
                )));
            }
        }
        Ok(())
    }
}

/// `es-MX` → `["es"]`: the tag's parents, longest first.
fn truncations(locale: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = locale;
    while let Some(cut) = rest.rfind('-') {
        rest = &rest[..cut];
        if !rest.is_empty() {
            out.push(rest.to_owned());
        }
    }
    out
}

/// A TOML error with the line and column it happened at.
///
/// `toml`'s `Display` renders a snippet of the file; `message` is the
/// sentence alone, which is what a report wants beside its own position.
fn config_error(text: &str, path: &Path, e: &toml::de::Error) -> Error {
    let message = match e.span() {
        Some(span) => {
            let index = LineIndex::new(text);
            let at = index.position(text, u32::try_from(span.start).unwrap_or(u32::MAX));
            format!("{}:{}: {}", at.line, at.column, e.message())
        }
        None => e.message().to_owned(),
    };
    Error::Config {
        path: path.to_path_buf(),
        message,
    }
}

/// `Serialize` for `mf2 init`, which writes a file that reads back as itself.
mod serialize {
    use super::{
        CatalogConfig, Config, DataSet, DateCalendars, DatesConfig, LocaleDataConfig, Missing,
        Strip, ZoneNames,
    };
    use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

    impl Serialize for Config {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut m = s.serialize_map(None)?;
            m.serialize_entry("source_locale", &self.source_locale)?;
            if !self.fallback.is_empty() {
                m.serialize_entry("fallback", &self.fallback)?;
            }
            m.serialize_entry("catalog", &self.catalog)?;
            m.serialize_entry("locale_data", &self.locale_data)?;
            if self.dates != DatesConfig::default() {
                m.serialize_entry("dates", &self.dates)?;
            }
            if !self.lints.is_empty() {
                let named: std::collections::BTreeMap<&str, String> = self
                    .lints
                    .iter()
                    .map(|(l, v)| (l.name(), v.to_string()))
                    .collect();
                m.serialize_entry("lints", &named)?;
            }
            if !self.functions.is_empty() {
                m.serialize_entry("functions", &self.functions)?;
            }
            m.end()
        }
    }

    impl Serialize for CatalogConfig {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut m = s.serialize_map(Some(2))?;
            m.serialize_entry("strip", &self.strip)?;
            m.serialize_entry("missing", &self.missing)?;
            m.end()
        }
    }

    impl Serialize for LocaleDataConfig {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut m = s.serialize_map(Some(2))?;
            m.serialize_entry("currencies", &self.currencies)?;
            m.serialize_entry("units", &self.units)?;
            m.end()
        }
    }

    impl Serialize for DatesConfig {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut m = s.serialize_map(Some(2))?;
            m.serialize_entry("calendars", &self.calendars)?;
            m.serialize_entry("zone-names", &self.zone_names)?;
            m.end()
        }
    }

    impl Serialize for DateCalendars {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_str(match self {
                DateCalendars::Auto => "auto",
                DateCalendars::Gregorian => "gregorian",
                DateCalendars::All => "all",
            })
        }
    }

    impl Serialize for ZoneNames {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            match self {
                ZoneNames::Auto => s.serialize_str("auto"),
                ZoneNames::Yes => s.serialize_bool(true),
                ZoneNames::No => s.serialize_bool(false),
            }
        }
    }

    impl Serialize for Strip {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_str(match self {
                Strip::Cold => "cold",
                Strip::Ids => "ids",
            })
        }
    }

    impl Serialize for Missing {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_str(match self {
                Missing::Fallback => "fallback",
                Missing::Id => "id",
                Missing::Empty => "empty",
            })
        }
    }

    impl Serialize for DataSet {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            match self {
                DataSet::Used => s.serialize_str("used"),
                DataSet::All => s.serialize_str("all"),
                DataSet::Listed(list) => {
                    let mut seq = s.serialize_seq(Some(list.len()))?;
                    for code in list {
                        seq.serialize_element(code)?;
                    }
                    seq.end()
                }
            }
        }
    }
}

/// Where `mf2.toml` and `locales/` live.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The i18n crate's directory.
    pub root: PathBuf,
    /// `<root>/locales`.
    pub locales: PathBuf,
}

impl Layout {
    /// The layout of the i18n crate rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Layout {
        let root = root.into();
        let locales = root.join("locales");
        Layout { root, locales }
    }

    /// The locale tags `locales/` holds, sorted: one directory per tag
    /// (`locales/en/…`), or one flat JSON file per tag (`locales/en.json`).
    pub fn locales(&self) -> Result<Vec<String>> {
        let mut tags = BTreeSet::new();
        let dir = std::fs::read_dir(&self.locales)
            .map_err(|source| Error::io(self.locales.clone(), source))?;
        for entry in dir {
            let entry = entry.map_err(|source| Error::io(self.locales.clone(), source))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|source| Error::io(path.clone(), source))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let tag = if kind.is_dir() {
                name.as_ref()
            } else if let Some(tag) = name.strip_suffix(".json") {
                tag
            } else {
                continue;
            };
            if !is_tag(tag) {
                return Err(Error::Layout(format!(
                    "{}: {tag:?} is not a locale tag — `locales/` holds one \
                     directory or one .json file per tag, and nothing else",
                    self.locales.display()
                )));
            }
            tags.insert(tag.to_owned());
        }
        if tags.is_empty() {
            return Err(Error::Layout(format!(
                "{}: no locales — expected a directory or a .json file per tag",
                self.locales.display()
            )));
        }
        Ok(tags.into_iter().collect())
    }
}

/// Whether `name` can be a locale tag: BCP 47's shape, loosely — subtags of
/// ASCII letters and digits joined by `-`.
///
/// Loose because the tag is only ever matched against CLDR data, which
/// decides what it means; strict because it becomes a file name and is
/// interpolated into generated Rust, and because an editor's scratch
/// directory under `locales/` should say so rather than become a locale.
fn is_tag(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .split('-')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(codes: &[&str]) -> BTreeSet<String> {
        codes.iter().map(|c| (*c).to_owned()).collect()
    }

    #[test]
    fn an_explicit_list_wins_over_a_variables_every_code() {
        let listed = DataSet::Listed(set(&["EUR", "USD"]));
        assert_eq!(
            listed.with_used(&Selection::All),
            Selection::Listed(set(&["EUR", "USD"]))
        );
        assert_eq!(
            listed.with_used(&Selection::Listed(set(&["JPY"]))),
            Selection::Listed(set(&["EUR", "JPY", "USD"]))
        );
        assert_eq!(DataSet::Used.with_used(&Selection::All), Selection::All);
        assert_eq!(
            DataSet::All.with_used(&Selection::Listed(set(&["JPY"]))),
            Selection::All
        );
    }

    #[test]
    fn dates_reads_the_form_and_writes_it_back() {
        let path = Path::new("mf2.toml");
        let auto = Config::parse("", path).expect("no [dates]");
        assert_eq!(auto.dates, DatesConfig::default());
        assert_eq!(auto.dates.calendars, DateCalendars::Auto);
        assert_eq!(auto.dates.zone_names, ZoneNames::Auto);
        let widest = Config::parse("[dates]\ncalendars = \"all\"\nzone-names = true\n", path)
            .expect("the widest form");
        assert_eq!(widest.dates.calendars, DateCalendars::All);
        assert_eq!(widest.dates.zone_names, ZoneNames::Yes);
        let narrow = Config::parse(
            "[dates]\ncalendars = \"gregorian\"\nzone-names = false\n",
            path,
        )
        .expect("the narrowest form");
        assert_eq!(narrow.dates.calendars, DateCalendars::Gregorian);
        assert_eq!(narrow.dates.zone_names, ZoneNames::No);
        let words = Config::parse("[dates]\nzone-names = \"auto\"\n", path).expect("auto");
        assert_eq!(words.dates.zone_names, ZoneNames::Auto);
        assert!(Config::parse("[dates]\ncalendars = \"buddhist\"\n", path).is_err());
        assert!(Config::parse("[dates]\nzone-names = \"yes\"\n", path).is_err());
        // What `mf2 init` writes reads back as itself.
        let written = widest.to_toml();
        assert_eq!(Config::parse(&written, path).expect("reads back"), widest);
        let plain = auto.to_toml();
        assert!(!plain.contains("[dates]"), "{plain}");
    }
}
