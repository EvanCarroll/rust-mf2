//! `mf2 convert`: a one-shot migration into MF2 (`plans/05-tooling.md`
//! §6.1).
//!
//! `--from fluent FTL_DIR` reads `FTL_DIR/<locale>/**.ftl` and writes
//! `locales/<tag>/<name>.mf2` under `--dir`, in `mf2 fmt`'s canonical form.
//! What it cannot map is reported under a stable code and the command exits
//! non-zero; everything else is still written, so the rest of a corpus can
//! be checked while the errors are fixed by hand. It never overwrites: an
//! existing `.mf2` with other text stops it before anything is written. One
//! that already holds what it would write is left alone, so a second run
//! writes nothing and exits as the first did.
//!
//! `--from leptos-fluent APP_DIR` does the same with the application's
//! `.ftl` files and rewrites its call sites (§6.2), showing a diff unless
//! `--write` is given.

pub(crate) mod fluent;
pub(crate) mod leptos_fluent;
pub(crate) mod report;

use std::path::{Path, PathBuf};

use clap::{Args as ClapArgs, ValueEnum};
use mf2_build::Layout;

use self::report::{Code, Finding, Report};
use crate::Format;
use crate::error::{Error, Result, read, write};

/// What `mf2 convert` reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Source {
    /// Fluent `.ftl` files, one directory per locale.
    Fluent,
    /// A `leptos-fluent` application: its `.ftl` files and its call sites.
    LeptosFluent,
}

/// `mf2 convert`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    /// The format to convert from.
    #[arg(long, value_enum, value_name = "FORMAT")]
    from: Source,
    /// `fluent`: the directory holding one subdirectory per locale
    /// (`<DIR>/<locale>/**.ftl`). `leptos-fluent`: the application's crate.
    #[arg(value_name = "DIR")]
    input: PathBuf,
    /// How to report.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// `leptos-fluent`: write the `.mf2` files and the rewritten Rust
    /// files; without it, show the diff and write nothing.
    #[arg(long)]
    write: bool,
    /// `leptos-fluent`: the `.ftl` directory, if not the initializer's
    /// `locales:` or `APP_DIR/locales`.
    #[arg(long, value_name = "DIR")]
    locales: Option<PathBuf>,
    /// `leptos-fluent`: the crate the rewritten `use` names, if not
    /// `crate` (the messages in the application's own crate) or the
    /// package in `--dir`'s Cargo.toml.
    #[arg(long, value_name = "NAME")]
    i18n_crate: Option<String>,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    if args.from == Source::LeptosFluent {
        return leptos_fluent::run(
            dir,
            &leptos_fluent::Request {
                app: &args.input,
                write: args.write,
                locales: args.locales.as_deref(),
                i18n_crate: args.i18n_crate.as_deref(),
                format: args.format,
            },
        );
    }
    if args.write || args.locales.is_some() || args.i18n_crate.is_some() {
        return Err(Error::Usage(
            "--write, --locales and --i18n-crate are for --from leptos-fluent; \
             --from fluent always writes"
                .to_owned(),
        ));
    }
    let mut report = Report::default();
    let outputs = convert_fluent(
        &args.input,
        &Layout::new(dir).locales,
        fluent::Options::default(),
        &mut report,
    )?;

    // Nothing is written if anything would be overwritten.
    let pending = pending(&outputs, "")?;
    for (path, text) in &pending {
        write(path, text)?;
    }

    match args.format {
        Format::Text => {
            print!("{}", report.to_text());
            let entries: usize = report.entries.values().sum();
            println!(
                "mf2 convert: {entries} entries in {} locale(s), {} file(s) written{}; {} error(s), {} warning(s)",
                report.entries.len(),
                pending.len(),
                unchanged(outputs.len() - pending.len()),
                report.errors(),
                report.warnings()
            );
        }
        Format::Json => println!("{}", report.to_json()),
    }
    if report.errors() > 0 {
        return Err(Error::Corpus);
    }
    Ok(())
}

/// The outputs a run writes: those not already on disk with the same text.
/// An existing file with other text is an error, before anything is
/// written; `hint` ends its message.
pub(crate) fn pending<'a>(
    outputs: &'a [(PathBuf, String)],
    hint: &str,
) -> Result<Vec<&'a (PathBuf, String)>> {
    let mut out = Vec::new();
    for output in outputs {
        let (path, text) = output;
        if !path.exists() {
            out.push(output);
        } else if std::fs::read(path).ok().as_deref() != Some(text.as_bytes()) {
            return Err(Error::Usage(format!(
                "{} already exists, with other text; mf2 convert never overwrites, \
                 so nothing was written{hint}",
                path.display()
            )));
        }
    }
    Ok(out)
}

/// The summary's note on outputs already as converted, if there are any.
pub(crate) fn unchanged(count: usize) -> String {
    if count == 0 {
        String::new()
    } else {
        format!(" ({count} unchanged)")
    }
}

/// Converts every locale under `input`, returning each output file's path
/// under `locales` and its text.
pub(crate) fn convert_fluent(
    input: &Path,
    locales: &Path,
    options: fluent::Options,
    report: &mut Report,
) -> Result<Vec<(PathBuf, String)>> {
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(input).map_err(|source| Error::io(input, source))? {
        let entry = entry.map_err(|source| Error::io(input, source))?;
        if entry.path().is_dir() {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    if dirs.is_empty() {
        return Err(Error::Usage(format!(
            "{}: no locale directories (expected {}/<locale>/*.ftl)",
            input.display(),
            input.display()
        )));
    }

    let mut outputs = Vec::new();
    for dir in dirs {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tag = name.replace('_', "-");
        if icu_locale_core::Locale::try_from_str(&tag).is_err() {
            report.push(Finding {
                code: Code::Locale,
                locale: name.clone(),
                file: dir.clone(),
                line: 1,
                column: 1,
                id: None,
                message: format!("{name} is not a well-formed BCP 47 language tag; skipped"),
            });
            continue;
        }

        let mut paths = Vec::new();
        collect_ftl(&dir, &mut paths)?;
        paths.sort();
        let mut files: Vec<fluent::File> = Vec::new();
        for path in paths {
            let relative = path.strip_prefix(&dir).unwrap_or(&path);
            let stem = relative.with_extension("");
            let flat = stem
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(".");
            if let Some(first) = files.iter().find(|f| f.name == flat) {
                report.push(Finding {
                    code: Code::FileCollision,
                    locale: tag.clone(),
                    file: path.clone(),
                    line: 1,
                    column: 1,
                    id: None,
                    message: format!(
                        "becomes {flat}.mf2, as {} does; the first, in path order, is kept",
                        first.path.display()
                    ),
                });
                continue;
            }
            files.push(fluent::File {
                text: read(&path)?,
                path,
                name: flat,
            });
        }
        for out in fluent::convert_locale(&tag, &files, options, report) {
            outputs.push((
                locales.join(&tag).join(format!("{}.mf2", out.name)),
                out.text,
            ));
        }
    }
    Ok(outputs)
}

/// Every `.ftl` file under `dir`.
fn collect_ftl(dir: &Path, into: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).map_err(|source| Error::io(dir, source))? {
        let entry = entry.map_err(|source| Error::io(dir, source))?;
        let path = entry.path();
        if path.is_dir() {
            collect_ftl(&path, into)?;
        } else if path.extension().is_some_and(|e| e == "ftl") {
            into.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::report::{Code, Report};
    use super::{convert_fluent, fluent};

    fn corpus() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fluent/constructs")
    }

    /// The construct corpus's expected output: `<tag>/<name>.mf2` → text.
    fn expected() -> BTreeMap<PathBuf, String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fluent/constructs.expected");
        let mut out = BTreeMap::new();
        for locale in std::fs::read_dir(&root)
            .expect("the expected output")
            .flatten()
        {
            for file in std::fs::read_dir(locale.path())
                .expect("a locale")
                .flatten()
            {
                let path = file.path();
                let rel = path.strip_prefix(&root).expect("under root").to_path_buf();
                out.insert(rel, std::fs::read_to_string(&path).expect("readable"));
            }
        }
        out
    }

    fn convert(options: fluent::Options) -> (BTreeMap<PathBuf, String>, Report) {
        let mut report = Report::default();
        let outputs = convert_fluent(&corpus(), Path::new(""), options, &mut report)
            .expect("the corpus converts");
        (outputs.into_iter().collect(), report)
    }

    #[test]
    fn the_construct_corpus_converts_as_expected() {
        let (outputs, report) = convert(fluent::Options::default());
        for (path, text) in &expected() {
            assert_eq!(outputs.get(path), Some(text), "{}", path.display());
        }
        assert_eq!(outputs.len(), expected().len());
        // Every DATETIME is approximate by design, and nothing else is
        // reported: the mapped part of the corpus maps.
        let codes: Vec<Code> = report.findings.iter().map(|f| f.code).collect();
        assert_eq!(
            codes,
            vec![Code::DatetimeApproximate; 7],
            "{}",
            report.to_text()
        );
        assert_eq!(
            report.features().into_iter().collect::<Vec<_>>(),
            ["fn-datetime", "fn-number"]
        );
    }

    /// Negative control: with `NUMBER` removed from the mapping, its
    /// constructs are reported and the comparison above fails.
    #[test]
    fn a_row_removed_from_the_mapping_is_reported() {
        let (outputs, report) = convert(fluent::Options {
            without_function: Some("NUMBER"),
        });
        let unknown: Vec<&str> = report
            .findings
            .iter()
            .filter(|f| f.code == Code::UnknownFunction)
            .filter_map(|f| f.id.as_deref())
            .collect();
        assert!(unknown.contains(&"number-plain"), "{}", report.to_text());
        assert!(unknown.contains(&"select-ordinal"), "{}", report.to_text());
        assert!(report.errors() > 0);
        let expected = expected();
        let path = Path::new("en/functions.mf2");
        assert_ne!(outputs.get(path), expected.get(path));
    }
}
