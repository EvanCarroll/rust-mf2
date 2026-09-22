//! `cargo xtask locale-data`: regenerate `crates/mf2-locale-data/data/`.
//! Offline: the plural and direction tables come from the vendored CLDR JSON
//! (`third_party/cldr-json`); the all-locale number table from every
//! locale's `numbers.json` in the `cargo xtask cldr-sync` cache
//! (`target/xtask-cache/cldr-json`, checked to be at the PIN's commit) plus
//! the vendored supplemental files. `cargo xtask cldr-sync` is what fetches
//! and refreshes both.

use std::fs;
use std::path::{Path, PathBuf};

use crate::cldr_sync::cache_dir;
use crate::error::{Error, Result};
use crate::git::Repo;
use crate::pin::Pin;

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let cldr = root.join("third_party").join("cldr-json");
    let pin = Pin::load(&cldr.join("PIN"))?;
    let tag = pin.get("tag")?.to_owned();
    let commit = pin.get("commit")?.to_owned();
    let supplemental = cldr.join("cldr-core").join("supplemental");
    let table = mf2_locale_data::extract::plurals_table(
        &read(&supplemental.join("plurals.json"))?,
        &read(&supplemental.join("ordinals.json"))?,
        &tag,
    )?;
    let data = root.join("crates").join("mf2-locale-data").join("data");
    let write = |name: &str, text: &str| {
        let path = data.join(name);
        fs::write(&path, text).map_err(|source| Error::IoAt {
            path: path.clone(),
            source,
        })?;
        eprintln!(
            "locale-data: {} ({} bytes, CLDR {tag})",
            path.display(),
            text.len()
        );
        Ok::<(), Error>(())
    };
    write("plurals.txt", &table)?;
    let likely = read(&supplemental.join("likelySubtags.json"))?;
    let scripts = read(&cldr.join("cldr-core").join("scriptMetadata.json"))?;
    write(
        "directions.txt",
        &mf2_locale_data::extract::directions_table(&likely, &scripts, &tag)?,
    )?;

    let full = cache_root(root, &commit)?;
    let main = full.join("cldr-numbers-full").join("main");
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(&main).map_err(|source| Error::IoAt {
        path: main.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| Error::IoAt {
            path: main.clone(),
            source,
        })?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    let mut locales = Vec::with_capacity(names.len());
    for name in names {
        let json = read(&main.join(&name).join("numbers.json"))?;
        locales.push((name, json));
    }
    let core = full.join("cldr-core");
    let parent_locales = read(&core.join("supplemental").join("parentLocales.json"))?;
    let numbering_systems = read(&supplemental.join("numberingSystems.json"))?;
    let available_locales = read(&core.join("availableLocales.json"))?;
    let default_content = read(&core.join("defaultContent.json"))?;
    let inputs = mf2_locale_data::extract::NumberInputs {
        locales: &locales,
        parent_locales: &parent_locales,
        likely_subtags: &likely,
        numbering_systems: &numbering_systems,
        available_locales: &available_locales,
        default_content: &default_content,
    };
    eprintln!(
        "locale-data: numbers of {} locales from {}",
        locales.len(),
        main.display()
    );
    let numbers = mf2_locale_data::extract::numbers_table(&inputs, &tag)?;
    write("numbers.txt", &numbers)?;

    // Currencies and units: one file at a time (units.json is ~143 KB a
    // locale), deduplicated through the parents of numbers.txt.
    let names: Vec<String> = locales.into_iter().map(|(name, _)| name).collect();
    let currency_data = read(&supplemental.join("currencyData.json"))?;
    eprintln!("locale-data: currencies of {} locales", names.len());
    write(
        "currencies.txt",
        &mf2_locale_data::extract::currencies_table(&main, &names, &currency_data, &numbers, &tag)?,
    )?;
    let units_main = full.join("cldr-units-full").join("main");
    eprintln!("locale-data: units of {} locales", names.len());
    write(
        "units.txt",
        &mf2_locale_data::extract::units_table(&units_main, &names, &numbers, &tag)?,
    )
}

/// The cache's package root (`…/cldr-json` or the repository root,
/// depending on upstream's layout), after checking that the cache is a
/// checkout of the PIN's commit.
fn cache_root(root: &Path, commit: &str) -> Result<PathBuf> {
    let dir = cache_dir(root);
    let repo = Repo::open(&dir).ok_or_else(|| Error::CacheMissing("absent".to_owned()))?;
    let head = repo
        .commit_of("HEAD")
        .map_err(|_| Error::CacheMissing("not checked out".to_owned()))?;
    if head != commit {
        return Err(Error::CacheMismatch(format!(
            "checked out at {head}, the PIN says {commit}"
        )));
    }
    for base in [dir.join("cldr-json"), dir.clone()] {
        if base
            .join("cldr-numbers-full")
            .join("main")
            .join("und")
            .join("numbers.json")
            .is_file()
        {
            return Ok(base);
        }
    }
    Err(Error::CacheMissing(
        "without the all-locale number files".to_owned(),
    ))
}
