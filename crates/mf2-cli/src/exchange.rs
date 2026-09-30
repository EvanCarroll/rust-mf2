//! `mf2 export` / `mf2 import`: flat JSON, the shape every
//! translation-management system speaks, and XLIFF 2, the standard a
//! translation tool protects placeholders in ([`xliff`];
//! `plans/05-tooling.md` §6.3).
//!
//! Export writes `{id: source}` sorted by id. Import reads one back into the
//! container the locale already uses: a `.mf2` resource keeps its sections,
//! comments and properties, and only the values change — a translator's
//! round trip must not throw away the context the source carries. Before it
//! writes anything, import runs every check `mf2 check` makes on the files
//! as they would be, and writes nothing if that brings an error (Phase 10
//! E3).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(crate) mod xliff;

use clap::{Args as ClapArgs, ValueEnum};
use mf2_build::loader::{Loader, json, resource};
use mf2_build::{Build, Config, Diagnostic, Emit, Features, Layout, Level, Lint, Report};
use mf2_resource::{parse, serialize_with};

use crate::FeatureArgs;
use crate::error::{Error, Result, read, write};

/// What `mf2 export` writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum ExportFormat {
    /// `{id: source}`, the locale's own messages.
    #[default]
    Json,
    /// An XLIFF 2 document: the source locale's messages, with the locale's
    /// translations as targets.
    Xliff,
}

/// `mf2 export`.
#[derive(Debug, ClapArgs)]
pub(crate) struct ExportArgs {
    /// The locale to write.
    #[arg(value_name = "LOCALE")]
    locale: String,
    /// Where to write it; standard output by default.
    #[arg(long, short, value_name = "FILE")]
    pub(crate) out: Option<PathBuf>,
    /// The format.
    #[arg(long, value_enum, default_value_t)]
    format: ExportFormat,
}

/// `mf2 import`.
#[derive(Debug, ClapArgs)]
pub(crate) struct ImportArgs {
    /// The locale to read into.
    #[arg(value_name = "LOCALE")]
    locale: String,
    /// The flat JSON or XLIFF 2 document to read (told apart by content).
    #[arg(value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// Say what would change and write nothing.
    #[arg(long)]
    dry_run: bool,
    // What the result is checked with, as `mf2 check`: without `--features`,
    // the i18n crate's, as cargo resolves them.
    #[command(flatten)]
    features: FeatureArgs,
}

pub(crate) fn export(dir: &Path, args: &ExportArgs) -> Result<()> {
    let layout = Layout::new(dir);
    let path = locale_path(&layout, &args.locale)?;
    if args.format == ExportFormat::Xliff {
        let source = Config::load(dir)?.source_locale;
        if source == args.locale {
            return Err(Error::Usage(format!(
                "{source} is the source locale: an XLIFF document is a translation into another"
            )));
        }
        let source_path = locale_path(&layout, &source)?;
        let (text, count) = xliff::export(&source_path, &source, &path, &args.locale)?;
        match &args.out {
            Some(file) => {
                write(file, &text)?;
                eprintln!(
                    "mf2 export: {count} messages of {source} with {}'s translations to {}",
                    args.locale,
                    file.display()
                );
            }
            None => print!("{text}"),
        }
        return Ok(());
    }
    let loaded = mf2_build::loader::for_path(&path).load(&path)?;
    let text = json::write(
        loaded
            .records
            .iter()
            .map(|r| (r.id.as_str(), r.source.as_str())),
    );
    match &args.out {
        Some(file) => {
            write(file, &text)?;
            eprintln!(
                "mf2 export: {} messages of {} to {}",
                loaded.records.len(),
                args.locale,
                file.display()
            );
        }
        None => print!("{text}"),
    }
    Ok(())
}

pub(crate) fn import(dir: &Path, args: &ImportArgs) -> Result<()> {
    let layout = Layout::new(dir);
    let config = Config::load(dir)?;
    let text = read(&args.file)?;
    let verb = if args.dry_run {
        "would change"
    } else {
        "changed"
    };
    let plan = if xliff::xml::is_xliff(&text) {
        let source = &config.source_locale;
        let source_path = locale_path(&layout, source)?;
        let path = locale_path(&layout, &args.locale)?;
        let imported = xliff::import(
            &source_path,
            source,
            &path,
            &args.locale,
            &text,
            xliff::Checks::default(),
        )?;
        Plan {
            summary: if imported.refused.is_empty() {
                format!(
                    "mf2 import: {} message(s) {verb}, {} added, in {}",
                    imported.changed, imported.added, args.locale
                )
            } else {
                format!(
                    "mf2 import: {} unit(s) refused; {} message(s) {verb}",
                    imported.refused.len(),
                    imported.changed + imported.added
                )
            },
            rewrites: imported.rewrites,
            refused: imported.refused,
        }
    } else {
        json_plan(&layout, &config, args, &text, verb)?
    };

    for line in &plan.refused {
        eprintln!("{line}");
    }
    let findings = if plan.rewrites.is_empty() {
        Vec::new()
    } else {
        let features = crate::check::features(dir, &args.features);
        checked(&layout, &config, &features, &plan.rewrites)?
    };
    let mut report = String::new();
    for finding in &findings {
        finding.write_text(&mut report);
    }
    eprint!("{report}");
    let errors = findings.iter().filter(|d| d.level == Level::Error).count();
    if errors > 0 {
        eprintln!(
            "mf2 import: {errors} error(s) that {} does not have now; nothing {}",
            args.locale,
            if args.dry_run {
                "would be written"
            } else {
                "was written"
            }
        );
        return Err(Error::Corpus);
    }
    for rewrite in &plan.rewrites {
        if args.dry_run {
            println!("would rewrite {}", rewrite.path.display());
        } else {
            write(&rewrite.path, &rewrite.text)?;
        }
    }
    eprintln!("{}", plan.summary);
    if plan.refused.is_empty() {
        Ok(())
    } else {
        Err(Error::Corpus)
    }
}

/// A file an import would write, with its new text.
pub(crate) struct Rewrite {
    pub(crate) path: PathBuf,
    pub(crate) text: String,
}

/// What an import would do, before it is checked: the files it would
/// rewrite; what of the document it could not take, one line each (the
/// command then exits 1, after writing the rest); and the line that sums it
/// up.
struct Plan {
    rewrites: Vec<Rewrite>,
    refused: Vec<String>,
    summary: String,
}

/// A flat JSON document into the locale: new values for the ids it has.
fn json_plan(
    layout: &Layout,
    config: &Config,
    args: &ImportArgs,
    text: &str,
    verb: &str,
) -> Result<Plan> {
    let incoming: BTreeMap<String, String> = json::read(text)
        .map_err(|e| Error::Usage(format!("{}: {}", args.file.display(), e.message)))?
        .into_iter()
        .map(|pair| (pair.id, pair.source))
        .collect();

    let path = locale_path(layout, &args.locale)?;
    if path.is_file() {
        // A flat JSON locale: the file *is* the values.
        let text = json::write(
            incoming
                .iter()
                .map(|(id, source)| (id.as_str(), source.as_str())),
        );
        let now = read(&path)?;
        return Ok(Plan {
            summary: format!(
                "mf2 import: {} message(s) into {}",
                incoming.len(),
                path.display()
            ),
            rewrites: if text == now {
                Vec::new()
            } else {
                vec![Rewrite { path, text }]
            },
            refused: Vec::new(),
        });
    }

    // A resource locale: rewrite the values in place, leaving every section,
    // comment and property as it stands.
    let loaded = resource::Resources.load(&path)?;
    let known: BTreeSet<&str> = loaded.records.iter().map(|r| r.id.as_str()).collect();
    let mut changed = 0usize;
    let mut rewrites = Vec::new();
    for file in &loaded.files {
        let (resource, diagnostics) = parse(&file.text);
        if !diagnostics.is_empty() {
            return Err(Error::Usage(format!(
                "{}: has a syntax error; run `mf2 check` first",
                file.path.display()
            )));
        }
        let out = resource.map_values(|info, value| {
            let id = info.full_id().to_string();
            match incoming.get(&id) {
                Some(source) if *source != value => {
                    changed += 1;
                    std::borrow::Cow::Owned(source.clone())
                }
                _ => value,
            }
        });
        let text = serialize_with(&out, &resource::FMT_STYLE)
            .map_err(|e| Error::Usage(format!("{}: {e}", file.path.display())))?;
        if text != file.text {
            rewrites.push(Rewrite {
                path: file.path.clone(),
                text,
            });
        }
    }

    // An id the locale does not have needs a file and a section to go in,
    // which flat JSON does not say: it is refused, and XLIFF — which carries
    // where each message goes — is named as the way to add it. An id the
    // source does not have is no message at all.
    let source_path = locale_path(layout, &config.source_locale)?;
    let source = mf2_build::loader::for_path(&source_path).load(&source_path)?;
    let in_source: BTreeSet<&str> = source.records.iter().map(|r| r.id.as_str()).collect();
    let (new, strange): (Vec<&str>, Vec<&str>) = incoming
        .keys()
        .map(String::as_str)
        .filter(|id| !known.contains(id))
        .partition(|id| in_source.contains(id));
    let mut refused = Vec::new();
    if !new.is_empty() {
        refused.push(format!(
            "mf2 import: {} message(s) {} does not have yet were left out: {}; JSON import \
             changes the messages a language has, and XLIFF adds the others where the source \
             has them (`mf2 export {} --format xliff`)",
            new.len(),
            args.locale,
            first_ids(&new),
            args.locale
        ));
    }
    if !strange.is_empty() {
        refused.push(format!(
            "mf2 import: {} id(s) are not messages of {} and were left out: {}",
            strange.len(),
            config.source_locale,
            first_ids(&strange)
        ));
    }
    Ok(Plan {
        rewrites,
        refused,
        summary: format!("mf2 import: {changed} message(s) {verb} in {}", args.locale),
    })
}

/// How many ids a refusal names before it counts the rest.
const IDS_SHOWN: usize = 10;

/// The first [`IDS_SHOWN`] of `ids`, then how many more there are.
fn first_ids(ids: &[&str]) -> String {
    let shown = ids.len().min(IDS_SHOWN);
    let list = ids[..shown].join(", ");
    match ids.len() - shown {
        0 => list,
        more => format!("{list}, and {more} more"),
    }
}

/// Every check `mf2 check` makes, on the corpus as `rewrites` would leave it
/// — a copy of `locales/`, so that nothing is written — less what the corpus
/// reports as it stands. An import is refused for an error it brings, not
/// for one that is already there: in another language, or one it would
/// raise only because the features were not the build's.
fn checked(
    layout: &Layout,
    config: &Config,
    features: &Features,
    rewrites: &[Rewrite],
) -> Result<Vec<Diagnostic>> {
    let scratch = std::env::temp_dir().join(format!("mf2-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let copy = scratch.join("locales");
    let result = check_copy(layout, config, features, rewrites, &scratch, &copy);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn check_copy(
    layout: &Layout,
    config: &Config,
    features: &Features,
    rewrites: &[Rewrite],
    scratch: &Path,
    copy: &Path,
) -> Result<Vec<Diagnostic>> {
    copy_dir(&layout.locales, copy)?;
    for rewrite in rewrites {
        let within = rewrite.path.strip_prefix(&layout.locales).map_err(|_| {
            Error::Usage(format!(
                "{}: not under {}",
                rewrite.path.display(),
                layout.locales.display()
            ))
        })?;
        write(&copy.join(within), &rewrite.text)?;
    }
    let check = |root: &Path| {
        Build::at(root, scratch.join("out"))
            .config(config.clone())
            .features(features.clone())
            // Nothing is written or compressed: only the report counts.
            .emit(Emit::Module)
            .check()
    };
    let before = check(&layout.root)?.report;
    let after = check(scratch)?.report;
    Ok(brought(&before, after, copy, &layout.locales))
}

/// What `after` reports that `before` does not. A finding is the same one
/// wherever the file moved it, so lines and columns are not compared; the
/// copy's paths are put back to the corpus's first.
fn brought(before: &Report, after: Report, copy: &Path, real: &Path) -> Vec<Diagnostic> {
    type Key = (
        Level,
        String,
        Option<String>,
        Option<Lint>,
        Option<&'static str>,
        String,
    );
    let key = |d: &Diagnostic| -> Key {
        (
            d.level,
            d.locale.clone(),
            d.id.clone(),
            d.lint,
            d.kind.map(mf2_model::ErrorKind::suite_name),
            d.message.clone(),
        )
    };
    let mut there: BTreeMap<Key, usize> = BTreeMap::new();
    for d in &before.diagnostics {
        *there.entry(key(d)).or_default() += 1;
    }
    let (copy_text, real_text) = (copy.display().to_string(), real.display().to_string());
    let mut out = Vec::new();
    for mut d in after.diagnostics {
        if let Ok(within) = d.file.strip_prefix(copy) {
            d.file = real.join(within);
        }
        d.message = d.message.replace(&copy_text, &real_text);
        match there.get_mut(&key(&d)) {
            Some(n) if *n > 0 => *n -= 1,
            _ => out.push(d),
        }
    }
    out
}

/// Copies the directory `from` to `to`.
fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).map_err(|source| Error::io(to, source))?;
    let entries = std::fs::read_dir(from).map_err(|source| Error::io(from, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io(from, source))?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).map_err(|source| Error::io(&path, source))?;
        }
    }
    Ok(())
}

/// Where a locale lives: `locales/<tag>/` or `locales/<tag>.json`.
fn locale_path(layout: &Layout, tag: &str) -> Result<PathBuf> {
    let dir = layout.locales.join(tag);
    if dir.is_dir() {
        return Ok(dir);
    }
    let file = layout.locales.join(format!("{tag}.json"));
    if file.is_file() {
        return Ok(file);
    }
    Err(Error::Usage(format!(
        "{}: no locale {tag} (expected {} or {})",
        layout.locales.display(),
        dir.display(),
        file.display()
    )))
}
