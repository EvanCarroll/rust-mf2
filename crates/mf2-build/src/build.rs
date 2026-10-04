//! The entry point: everything a `build.rs` (or `mf2-cli`) does, in order.
//!
//! ```no_run
//! # fn main() -> Result<(), mf2_build::Error> {
//! // `emit_cargo` prints the report as cargo warnings and errors, and
//! // `into_result` turns an error count into a failed build: a corpus with
//! // errors in it writes nothing, so a build script that does not check
//! // would go on compiling against the *previous* build's output.
//! mf2_build::Build::new()?
//!     .source_locale("en")
//!     .emit_cargo(true)
//!     .run()?
//!     .into_result()?;
//! # Ok(()) }
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mf2_catalog::{Dir, Manifest};
use mf2_model::Message;

use crate::catalog::{self, Catalog, Filler};
use crate::codegen;
use crate::config::{Config, DataSet, Layout};
use crate::corpus::{self, LocaleSource};
use crate::error::{Error, Result};
use crate::features::{Features, Place};
use crate::lint::{Level, Lint};
use crate::manifest;
use crate::report::{Report, Sink};
use crate::slice;

/// The manifest's file name in `OUT_DIR` (`plans/02-catalog-format.md` §5).
pub(crate) const MANIFEST_FILE: &str = "manifest.mf2m";
/// The generated module's file name in `OUT_DIR`.
pub(crate) const GENERATED_FILE: &str = "mf2_generated.rs";
/// The catalog table's file name, when the two are emitted apart.
pub(crate) const CATALOGS_FILE: &str = "mf2_catalogs.rs";
/// The catalog index's file name in a published site ([`Outcome::publish`]):
/// what a client-only application reads to learn each locale's hashed URL
/// (`plans/04-leptos-integration.md` §8).
pub(crate) const INDEX_FILE: &str = "index.json";

/// What a build writes (`plans/05-tooling.md` §4; owner question 1 of
/// `plans/12-phase-5a-work-order.md`).
///
/// Every locale change rewrites a catalog, and a catalog is named by its
/// content hash, so a build that writes both puts a new name in the
/// generated module — and cargo recompiles the i18n crate and everything
/// that depends on it, in both of cargo-leptos' builds.
///
/// Splitting the two takes that cost away from the client: the i18n crate
/// emits [`Emit::Module`], which editing a translation's text never changes,
/// and a crate only the server binary depends on emits [`Emit::Catalogs`].
/// (A translation that introduces a *function* the source does not use still
/// moves the manifest hash, and must — see [`crate::manifest`].)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum Emit {
    /// The manifest, the catalogs and a module that names them (the
    /// default: one crate, one build script).
    #[default]
    Both,
    /// The manifest and a module with no catalog name, hash or byte in it.
    Module,
    /// The catalogs and the table that embeds them, for a server-only crate.
    Catalogs,
    /// For a native application (`mf2::native`): the manifest, the catalogs,
    /// and a module that formats through the native host, with nothing
    /// behind `ssr` and one `CORPUS` value that embeds the catalogs.
    Native,
    /// As [`Emit::Native`], but the catalogs are only written to the output
    /// directory — the application ships them and loads them from a
    /// directory — and `CORPUS` names their files without embedding them.
    NativeFiles,
}

/// A build, configured.
#[derive(Debug)]
pub struct Build {
    root: PathBuf,
    out_dir: PathBuf,
    config: Option<Config>,
    features: Option<Features>,
    source_locale: Option<String>,
    facade: String,
    emit: Emit,
    write: bool,
    emit_cargo: bool,
    inline_manifest: bool,
    /// How a web server's catalogs are compressed: at a fast level in a
    /// debug build's script (plans/19 §11), else at the best.
    compress: catalog::Compress,
}

/// One locale in the built corpus.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct LocaleInfo {
    /// The BCP 47 tag.
    pub tag: String,
    /// Its base direction.
    pub dir: Dir,
    /// The content hash of its catalog.
    pub hash: String,
    /// The file name the server publishes.
    pub file_name: String,
    /// The file name of its server-only table (`plan/08` §4.2), when the
    /// build wrote one: embedded in the server, never published.
    pub server_file_name: Option<String>,
}

/// What a build produced.
#[derive(Debug)]
#[non_exhaustive]
pub struct Outcome {
    /// The manifest, from the source locale.
    #[doc(hidden)]
    pub manifest: Manifest,
    /// The ids of `manifest` that the build added and no file holds: the
    /// pseudo-locales' `language.<tag>` names.
    #[doc(hidden)]
    pub added: Vec<String>,
    /// The ids the generated module names itself (the languages' names),
    /// which `unused-id` counts as used.
    #[doc(hidden)]
    pub used: Vec<String>,
    /// Where the source locale defines each of its ids, for `unused-id`.
    #[doc(hidden)]
    pub defined: BTreeMap<String, (PathBuf, mf2_resource::Position)>,
    /// Its hash — what the wasm and every catalog agree on.
    pub manifest_hash: u64,
    /// Everything the build has to say.
    pub report: Report,
    /// Per locale, in tag order: how many of the messages that need
    /// translating it has, which `mf2 stats` reports.
    #[doc(hidden)]
    pub coverage: Vec<crate::check::Coverage>,
    /// Which gated function families the corpus can use, for the feature
    /// list `mf2 check` prints.
    #[doc(hidden)]
    pub needs: crate::check::Needs,
    /// One catalog per locale, in tag order.
    #[doc(hidden)]
    pub catalogs: Vec<Catalog>,
    /// The locale table the generated module carries.
    pub locales: Vec<LocaleInfo>,
    /// The source locale.
    pub source_locale: String,
    /// The files whose bytes changed (and were written).
    pub written: Vec<PathBuf>,
    /// Old catalogs removed.
    pub removed: Vec<PathBuf>,
    /// The generated Rust module's source.
    pub generated: String,
    /// The catalog table's source, when the build emits it apart
    /// ([`Emit::Catalogs`]); empty otherwise.
    pub catalogs_module: String,
    /// The ICU4X date formatter's form (`plan/08` §5.1), which `mf2 check`
    /// prints: `None` when no side formats with ICU4X or no date reaches
    /// the corpus.
    #[doc(hidden)]
    pub dates: Option<slice::DateForm>,
    /// Where the outputs went.
    pub out_dir: PathBuf,
}

impl Outcome {
    /// The catalog of `tag`.
    #[doc(hidden)]
    pub fn catalog(&self, tag: &str) -> Option<&Catalog> {
        self.catalogs.iter().find(|c| c.tag == tag)
    }

    /// Whether the corpus had no errors.
    pub fn is_clean(&self) -> bool {
        self.report.is_clean()
    }

    /// The outcome if the corpus was clean, else the error a build script
    /// fails with — the report is printed before it, as warnings and errors.
    pub fn into_result(self) -> Result<Outcome> {
        if self.report.is_clean() {
            return Ok(self);
        }
        Err(self.corpus_error())
    }

    fn corpus_error(&self) -> Error {
        Error::Corpus {
            errors: self.report.errors(),
            locales: self.report.failing_locales().len(),
        }
    }

    /// Writes what a static host serves into `dir`: every catalog under its
    /// content-hashed name (with its `.br` and `.gz`, when the build
    /// compressed them) and `index.json`, `{"<tag>": "<file name>", …}`,
    /// which a client-only application reads to find them. Nothing else —
    /// not the manifest, not the generated module — so the whole directory
    /// can be published as it is. Catalogs an earlier publish left in `dir`
    /// are removed; each file is written only when its bytes change.
    ///
    /// The catalogs are immutable and may be cached forever; the index is
    /// not, and a host should serve it `no-cache`.
    pub fn publish(&self, dir: &Path) -> Result<Published> {
        if !self.report.is_clean() {
            return Err(self.corpus_error());
        }
        std::fs::create_dir_all(dir).map_err(|source| Error::io(dir.to_path_buf(), source))?;
        let mut published = Published::default();
        // What a browser reads and nothing else: no server-only table.
        let keep = write_catalogs(dir, &self.catalogs, false, &mut published.written)?;
        let index: serde_json::Map<String, serde_json::Value> = self
            .catalogs
            .iter()
            .map(|c| (c.tag.clone(), serde_json::Value::String(c.file_name())))
            .collect();
        let mut json = serde_json::Value::Object(index).to_string();
        json.push('\n');
        let index_path = dir.join(INDEX_FILE);
        if catalog::write_if_changed(&index_path, json.as_bytes())? {
            published.written.push(index_path);
        }
        published.removed = catalog::remove_stale(dir, &keep)?;
        Ok(published)
    }
}

/// What [`Outcome::publish`] did.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct Published {
    /// The files whose bytes changed (and were written).
    pub written: Vec<PathBuf>,
    /// Old catalogs removed.
    pub removed: Vec<PathBuf>,
}

/// Writes each catalog under its content-hashed name, with the compressed
/// variants the build made, into `dir`; returns every path that belongs
/// there, for [`catalog::remove_stale`].
fn write_catalogs(
    dir: &Path,
    catalogs: &[Catalog],
    server: bool,
    written: &mut Vec<PathBuf>,
) -> Result<Vec<PathBuf>> {
    let mut keep = Vec::new();
    for catalog in catalogs {
        let base = dir.join(catalog.file_name());
        // The server-only table, beside the catalog it completes; its name
        // holds `.mf2b`, so `remove_stale` prunes an old one too.
        if server && let Some(name) = catalog.server_file_name() {
            let path = dir.join(name);
            if catalog::write_if_changed(&path, &catalog.server)? {
                written.push(path.clone());
            }
            keep.push(path);
        }
        for (path, bytes) in [
            (base.clone(), &catalog.bytes),
            (with_suffix(&base, ".br"), &catalog.br),
            (with_suffix(&base, ".gz"), &catalog.gz),
        ] {
            // An `Emit::Module` build does not compress.
            if bytes.is_empty() {
                continue;
            }
            if catalog::write_if_changed(&path, bytes)? {
                written.push(path.clone());
            }
            keep.push(path);
        }
    }
    Ok(keep)
}

impl Build {
    /// A build of the crate cargo is compiling: `CARGO_MANIFEST_DIR` for the
    /// corpus, `OUT_DIR` for the outputs.
    pub fn new() -> Result<Build> {
        let root = std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
            Error::Layout(
                "CARGO_MANIFEST_DIR is not set: outside a build script, use \
                 Build::at(root, out_dir)"
                    .to_owned(),
            )
        })?;
        let out_dir = std::env::var_os("OUT_DIR").ok_or_else(|| {
            Error::Layout(
                "OUT_DIR is not set: outside a build script, use Build::at(root, out_dir)"
                    .to_owned(),
            )
        })?;
        let mut build = Build::at(root, out_dir);
        // A debug build compresses at a fast level: maximum-quality brotli
        // cost seconds per translation edit in an unoptimized build script.
        // Cargo says `release` for `release` and the profiles that inherit it.
        if std::env::var_os("PROFILE").is_some_and(|p| p != "release") {
            build.compress = catalog::Compress::Fast;
        }
        Ok(build)
    }

    /// A build of the corpus at `root`, writing to `out_dir`.
    pub fn at(root: impl Into<PathBuf>, out_dir: impl Into<PathBuf>) -> Build {
        Build {
            root: root.into(),
            out_dir: out_dir.into(),
            config: None,
            features: None,
            source_locale: None,
            facade: "::mf2".to_owned(),
            emit: Emit::Both,
            write: true,
            emit_cargo: false,
            inline_manifest: false,
            compress: catalog::Compress::Best,
        }
    }

    /// Overrides `mf2.toml`'s `source_locale`.
    #[must_use]
    pub fn source_locale(mut self, locale: impl Into<String>) -> Build {
        self.source_locale = Some(locale.into());
        self
    }

    /// Uses this configuration instead of reading `mf2.toml`.
    #[must_use]
    pub fn config(mut self, config: Config) -> Build {
        self.config = Some(config);
        self
    }

    /// Uses this feature set instead of reading `CARGO_FEATURE_*`.
    #[must_use]
    pub fn features(mut self, features: Features) -> Build {
        self.features = Some(features);
        self
    }

    /// The crate path the generated module re-exports as `__mf2`
    /// (`::mf2`). A test with a stub facade names its own.
    #[must_use]
    pub fn facade(mut self, path: impl Into<String>) -> Build {
        self.facade = path.into();
        self
    }

    /// What to write (the default is [`Emit::Both`]).
    #[must_use]
    pub fn emit(mut self, emit: Emit) -> Build {
        self.emit = emit;
        self
    }

    /// Prints `cargo::rerun-if-changed` and `cargo::warning` lines.
    #[must_use]
    pub fn emit_cargo(mut self, emit: bool) -> Build {
        self.emit_cargo = emit;
        self
    }

    /// Bakes the manifest's **bytes** into the `tr!` wrapper instead of its
    /// path (`plans/05-tooling.md` §4): every expansion is then independent
    /// of where the target directory lives, which is what remote execution
    /// and a relocated CI cache need. It costs macro time and manifest size
    /// × call sites in the generated module, so it is opt-in (P0.9:
    /// +0.5 s per 2,000 sites).
    #[must_use]
    pub fn manifest_inline(mut self, inline: bool) -> Build {
        self.inline_manifest = inline;
        self
    }

    /// Runs the whole pipeline and writes the outputs.
    pub fn run(self) -> Result<Outcome> {
        self.go(true)
    }

    /// Runs everything but writes nothing — `mf2 check`.
    pub fn check(self) -> Result<Outcome> {
        self.go(false)
    }

    fn go(mut self, write: bool) -> Result<Outcome> {
        self.write = write;
        let layout = Layout::new(&self.root);
        let mut config = match self.config.take() {
            Some(config) => config,
            None => Config::load(&self.root)?,
        };
        if let Some(locale) = &self.source_locale {
            config.source_locale.clone_from(locale);
        }
        let features = match self.features.take() {
            Some(features) => features,
            None => Features::from_env(),
        };
        if self.emit_cargo {
            println!("cargo::rerun-if-changed={}", layout.locales.display());
            // Only a file that exists: cargo reruns a script whose watched
            // file is missing on every build, and `mf2.toml` is optional.
            let config_file = self.root.join(crate::config::FILE_NAME);
            if config_file.is_file() {
                println!("cargo::rerun-if-changed={}", config_file.display());
            }
        }
        let outcome = self.corpus(&layout, &config, &features)?;
        if self.emit_cargo {
            print!("{}", outcome.report.to_cargo_lines());
        }
        Ok(outcome)
    }

    fn corpus(&self, layout: &Layout, config: &Config, features: &Features) -> Result<Outcome> {
        let mut report = Report::new();
        let tags = layout.locales()?;
        if !tags.contains(&config.source_locale) {
            return Err(Error::Layout(format!(
                "the source locale {:?} is not among the locales in {} ({})",
                config.source_locale,
                layout.locales.display(),
                tags.join(", ")
            )));
        }
        let sources = corpus::load(&layout.locales, &tags, config, &mut report)?;
        let models: Vec<Vec<Option<Message<'_>>>> = sources
            .iter()
            .map(|source| corpus::parse(source, &mut report))
            .collect();
        let indexes: Vec<BTreeMap<&str, usize>> = sources
            .iter()
            .map(|source| corpus::by_id(source, config, &mut report))
            .collect();

        let source_index = tags
            .iter()
            .position(|t| *t == config.source_locale)
            .expect("checked above");
        let mut functions = BTreeSet::new();
        for locale_models in &models {
            manifest::functions_of(locale_models, &mut functions);
        }
        let mut built = manifest::build(
            &sources[source_index],
            &models[source_index],
            &indexes[source_index],
            &functions,
        );
        ids_of_translations(
            &sources,
            &indexes,
            source_index,
            &built.manifest.ids,
            config,
            &mut report,
        );
        let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
        let defined = definitions(&sources[source_index], &indexes[source_index]);
        let checked = crate::check::Corpus {
            sources: &sources,
            models: &models,
            indexes: &indexes,
            source_index,
            manifest: &built,
        };
        let needs = crate::check::corpus(&checked, config, features, &mut report);
        let coverage: Vec<crate::check::Coverage> = (0..tags.len())
            .map(|locale| crate::check::coverage_of(&checked, locale))
            .collect();

        // A corpus with errors is not written: the catalog writer would
        // refuse half of what the lints just reported (a translation using a
        // variable with no slot, say) and the reader would see the writer's
        // words instead of the lint's.
        if !report.is_clean() {
            let used = codegen::uses(&tag_refs, &built.manifest);
            return Ok(Outcome {
                manifest_hash: built.manifest.hash(),
                manifest: built.manifest,
                added: Vec::new(),
                used,
                defined,
                report,
                coverage,
                needs,
                catalogs: Vec::new(),
                locales: Vec::new(),
                source_locale: config.source_locale.clone(),
                written: Vec::new(),
                removed: Vec::new(),
                generated: String::new(),
                catalogs_module: String::new(),
                dates: None,
                out_dir: self.out_dir.clone(),
            });
        }

        // The pseudo-locales' names, once the lints have run: they are the
        // build's, not a translator's.
        let added = crate::pseudo::names(&tags, &built.manifest);
        for (id, _) in &added {
            if let Err(at) = built.manifest.ids.binary_search(id) {
                built.manifest.ids.insert(at, id.clone());
                built.manifest.slots.insert(at, Vec::new());
                built.manifest.markup.insert(at, Vec::new());
                // No source record holds it.
                built.source_records.insert(at, usize::MAX);
            }
        }

        // Per locale, the models in `MsgId` order, then the fallback chain.
        let by_msg_id: Vec<Vec<Option<&Message<'_>>>> = (0..tags.len())
            .map(|locale| {
                built
                    .manifest
                    .ids
                    .iter()
                    .map(|id| {
                        indexes[locale]
                            .get(id.as_str())
                            .and_then(|&record| models[locale].get(record))
                            .and_then(Option::as_ref)
                            .or_else(|| added.iter().find(|(a, _)| a == id).map(|(_, m)| m))
                    })
                    .collect()
            })
            .collect();

        let filler = Filler::new(&built.manifest.ids, config.catalog.missing);
        // A native application keeps every entry in its catalog (`plan/08`
        // §4.2): no browser downloads it.
        let (numbers, names, date_slice) = if codegen::is_native(self.emit) {
            (Place::Catalog, Place::Catalog, Place::Catalog)
        } else {
            (
                features.number_place(),
                features.names_place(),
                features.date_slice_place(),
            )
        };
        // Whether a browser downloads these catalogs, for `unread-data`.
        let downloaded = !codegen::is_native(self.emit) && features.has_browser_side();
        // Every locale is sliced before any catalog is written: the ICU4X
        // form is the corpus's, and each locale's date slice is cut for it
        // (`plan/08` §5.1).
        let mut sliced = Vec::with_capacity(tags.len());
        for (i, tag) in tags.iter().enumerate() {
            let chain_tags = config.chain(tag);
            let chain: Vec<&[Option<&Message<'_>>]> = chain_tags
                .iter()
                .filter_map(|t| tags.iter().position(|x| x == t))
                .map(|j| by_msg_id[j].as_slice())
                .collect();
            let chain_tags: Vec<String> = chain_tags
                .into_iter()
                .filter(|t| tags.iter().any(|x| x == t))
                .collect();
            let resolved = catalog::resolve(
                &built.manifest.ids,
                &by_msg_id[i],
                &chain,
                config.catalog.missing,
                &filler,
            );
            let flattened: Vec<&Message<'_>> =
                resolved.messages.iter().flatten().copied().collect();
            let slice = slice::of(&flattened, &config.locale_data, features);
            report_slicing(tag, &slice, features, config, &sources[i], &mut report);
            sliced.push((tag, chain_tags, resolved, slice));
        }
        // The form, when a side formats with ICU4X and a date can reach the
        // corpus; otherwise there is no slice to cut, and `None`.
        #[cfg(feature = "icu-blob")]
        let dates = if features.cuts_date_slice() && sliced.iter().any(|s| !s.3.dates.is_empty()) {
            let all: Vec<(&str, &slice::Slice)> =
                sliced.iter().map(|s| (s.0.as_str(), &s.3)).collect();
            let form = slice::date_form(&all, &config.dates)?;
            for s in &mut sliced {
                s.3.form = form.clone();
            }
            Some(form)
        } else {
            None
        };
        #[cfg(not(feature = "icu-blob"))]
        let dates: Option<slice::DateForm> = None;
        let mut catalogs = Vec::with_capacity(tags.len());
        let mut locales = Vec::with_capacity(tags.len());
        for (tag, chain_tags, resolved, slice) in sliced {
            let catalog = catalog::write(
                tag,
                &built.manifest,
                &resolved,
                &chain_tags,
                slice,
                config,
                // Only a web server serves them compressed.
                match self.emit {
                    Emit::Both | Emit::Catalogs => self.compress,
                    _ => catalog::Compress::No,
                },
                numbers,
                names,
                date_slice,
            )?;
            // `unread-data` (`plan/08` §7): nothing is written where none of
            // its readers looks.
            catalog::check_read(
                &catalog.tag,
                &catalog.locale_entries,
                &catalog.server_entries,
                features,
                downloaded,
            )?;
            locales.push(LocaleInfo {
                tag: tag.clone(),
                dir: catalog::dir_of(tag)?,
                hash: catalog.hash.clone(),
                file_name: catalog.file_name(),
                server_file_name: catalog.server_file_name(),
            });
            catalogs.push(catalog);
        }

        let unannotated = catalogs.iter().any(|c| c.slice.unannotated);
        let manifest_bytes = built.manifest.write();
        // The part of CLDR's language-matching data these locales need
        // (plans/19-native-and-terminal.md §9): all a browser's client
        // carries of it. It depends on the tags alone, so a translation
        // edit leaves it, and the module, as they are.
        let tags: Vec<&str> = locales.iter().map(|l| l.tag.as_str()).collect();
        let language_matching = mf2_locale_data::matching::Matching::shipped()?
            .cut(&tags)
            .encode()?
            .rust("__mf2::LanguageMatching");
        // What `markup::*` and `Locale::name()` are made of: the source's
        // markup names, and whether every locale has an argument-free
        // `language.<tag>` message. Both come from the manifest, which a
        // translation edit leaves as it is.
        let markup: Vec<String> = built
            .manifest
            .markup
            .iter()
            .flatten()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let names = codegen::named(&tags, &built.manifest);
        let used = codegen::uses(&tags, &built.manifest);
        // Some languages named and not all: the switcher shows the others'
        // tags, and `Locale::name()` is not generated.
        if names.contains(&true) && names.contains(&false) {
            let missing: Vec<String> = locales
                .iter()
                .zip(&names)
                .filter(|(_, named)| !**named)
                .map(|(locale, _)| format!("`language.{}`", locale.tag))
                .collect();
            let source = &sources[source_index];
            let file = source
                .loaded
                .files
                .first()
                .map_or_else(|| source.path.clone(), |f| f.path.clone());
            Sink::new(&mut report, &config.source_locale).add(
                Level::Warn,
                None,
                &file,
                mf2_resource::Position { line: 1, column: 1 },
                None,
                format!(
                    "some languages have a name message and some do not ({} missing, each \
                     with no argument): the locale switcher shows those languages' tags, \
                     and `Locale::name()` is not generated",
                    missing.join(", ")
                ),
            );
        }
        let module = codegen::Module {
            facade: &self.facade,
            manifest_path: &self.out_dir.join(MANIFEST_FILE),
            manifest_hash: built.manifest.hash(),
            source_locale: &config.source_locale,
            locales: &locales,
            language_matching: &language_matching,
            functions: &built.manifest.functions,
            custom: &config.functions,
            features,
            unannotated,
            dates: dates.as_ref(),
            messages: built.manifest.ids.len(),
            emit: self.emit,
            manifest_bytes: if self.inline_manifest {
                Some(manifest_bytes.as_slice())
            } else {
                None
            },
            markup: &markup,
            names: &names,
        };
        codegen::check(&module)?;
        let generated = codegen::write(&module);
        let catalogs_module = if self.emit == Emit::Catalogs {
            codegen::write_catalogs(&module)
        } else {
            String::new()
        };
        let mut outcome = Outcome {
            manifest_hash: built.manifest.hash(),
            manifest: built.manifest,
            added: added.into_iter().map(|(id, _)| id).collect(),
            used,
            defined,
            report,
            coverage,
            needs,
            catalogs,
            locales,
            source_locale: config.source_locale.clone(),
            written: Vec::new(),
            removed: Vec::new(),
            generated,
            catalogs_module,
            dates,
            out_dir: self.out_dir.clone(),
        };
        // A corpus with errors comes back with its report, not as an
        // `Err`: the caller prints it.
        if self.write && outcome.report.is_clean() {
            self.write_outputs(&mut outcome)?;
        }
        Ok(outcome)
    }

    /// Writes the manifest and the catalogs, each only when its bytes change.
    fn write_outputs(&self, outcome: &mut Outcome) -> Result<()> {
        std::fs::create_dir_all(&self.out_dir)
            .map_err(|source| Error::io(self.out_dir.clone(), source))?;
        if self.emit != Emit::Catalogs {
            let manifest_path = self.out_dir.join(MANIFEST_FILE);
            if catalog::write_if_changed(&manifest_path, &outcome.manifest.write())? {
                outcome.written.push(manifest_path);
            }
            let generated_path = self.out_dir.join(GENERATED_FILE);
            if catalog::write_if_changed(&generated_path, outcome.generated.as_bytes())? {
                outcome.written.push(generated_path);
            }
        }
        if self.emit == Emit::Module {
            // The catalogs are another crate's; nothing here writes one, so
            // a translation never touches this crate's outputs. Any that an
            // earlier `Emit::Both` or `Emit::Catalogs` build left behind are
            // still this directory's to prune — `remove_stale` only ever
            // touches `*.mf2b*`, never the manifest or the module.
            outcome.removed = catalog::remove_stale(&self.out_dir, &[])?;
            return Ok(());
        }
        if self.emit == Emit::Catalogs {
            let path = self.out_dir.join(CATALOGS_FILE);
            if catalog::write_if_changed(&path, outcome.catalogs_module.as_bytes())? {
                outcome.written.push(path);
            }
        }
        let keep = write_catalogs(&self.out_dir, &outcome.catalogs, true, &mut outcome.written)?;
        outcome.removed = catalog::remove_stale(&self.out_dir, &keep)?;
        Ok(())
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Where `source` defines each of its ids: the file and the id's position.
fn definitions(
    source: &LocaleSource,
    index: &BTreeMap<&str, usize>,
) -> BTreeMap<String, (PathBuf, mf2_resource::Position)> {
    index
        .iter()
        .filter_map(|(id, &record)| {
            let record = source.loaded.records.get(record)?;
            let file = source.loaded.files.get(record.file)?;
            Some((
                (*id).to_owned(),
                (file.path.clone(), file.position(record.id_span.start)),
            ))
        })
        .collect()
}

/// An id a translation has and the source locale does not would be dropped
/// without a word, so it is an error (`plans/05-tooling.md` §5).
fn ids_of_translations(
    sources: &[LocaleSource],
    indexes: &[BTreeMap<&str, usize>],
    source_index: usize,
    ids: &[String],
    config: &Config,
    report: &mut Report,
) {
    let known: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    for (i, source) in sources.iter().enumerate() {
        if i == source_index {
            continue;
        }
        let mut sink = Sink::new(report, &source.tag);
        for (id, &record) in &indexes[i] {
            if known.contains(id) {
                continue;
            }
            let record = &source.loaded.records[record];
            let file = &source.loaded.files[record.file];
            sink.add(
                config.level(Lint::ExtraId),
                Some(Lint::ExtraId),
                &file.path,
                file.position(record.id_span.start),
                Some(id),
                format!(
                    "the source locale {:?} has no message with this id",
                    sources[source_index].tag
                ),
            );
        }
    }
}

/// What slicing noticed: numbers without `fn-number`, and a currency or unit
/// set the build could not narrow.
fn report_slicing(
    tag: &str,
    slice: &slice::Slice,
    features: &Features,
    config: &Config,
    source: &LocaleSource,
    report: &mut Report,
) {
    let file = source
        .loaded
        .files
        .first()
        .map_or_else(|| source.path.clone(), |f| f.path.clone());
    let at = mf2_resource::Position { line: 1, column: 1 };
    let mut sink = Sink::new(report, tag);
    if slice.formats_numbers && !features.fn_number() {
        sink.add(
            config.level(Lint::NeutralNumbers),
            Some(Lint::NeutralNumbers),
            &file,
            at,
            None,
            "a placeholder in this locale's messages can receive a number, and \
             `fn-number` is off: a number it receives renders without the locale's \
             symbols, grouping or numbering system (if such placeholders only ever \
             receive text, set `neutral-numbers = \"allow\"` in mf2.toml)",
        );
    }
    // An explicit list says which codes the variable can hold, so the
    // catalog carries only those and there is nothing to warn about.
    let listed = |set: &DataSet| matches!(set, DataSet::Listed(_));
    if slice.dynamic_currency && !listed(&config.locale_data.currencies) {
        sink.add(
            config.level(Lint::DynamicCurrency),
            Some(Lint::DynamicCurrency),
            &file,
            at,
            None,
            "a `:currency` takes its currency from a variable, so the catalog \
             carries every currency CLDR has; listing the codes it can hold \
             under [locale_data] currencies carries only those",
        );
    }
    if slice.dynamic_unit && !listed(&config.locale_data.units) {
        sink.add(
            config.level(Lint::DynamicUnit),
            Some(Lint::DynamicUnit),
            &file,
            at,
            None,
            "a `:unit` takes its unit from a variable, so the catalog carries \
             every unit CLDR has; listing the ids it can hold under \
             [locale_data] units carries only those",
        );
    }
    let _ = Level::Warn;
}
