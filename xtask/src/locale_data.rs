//! `cargo xtask locale-data`: regenerate `crates/mf2-locale-data/data/` from
//! the vendored CLDR JSON (`third_party/cldr-json`). Offline: the inputs are
//! in the tree; `cargo xtask cldr-sync` is what refreshes them.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::pin::Pin;

pub(crate) fn run(root: &Path) -> Result<()> {
    let cldr = root.join("third_party").join("cldr-json");
    let tag = Pin::load(&cldr.join("PIN"))?.get("tag")?.to_owned();
    let supplemental = cldr.join("cldr-core").join("supplemental");
    let read = |name: &str| {
        let path = supplemental.join(name);
        fs::read_to_string(&path).map_err(|source| Error::IoAt { path, source })
    };
    let table = mf2_locale_data::extract::plurals_table(
        &read("plurals.json")?,
        &read("ordinals.json")?,
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
    let likely = fs::read_to_string(supplemental.join("likelySubtags.json")).map_err(|source| {
        Error::IoAt {
            path: supplemental.join("likelySubtags.json"),
            source,
        }
    })?;
    let scripts_path = cldr.join("cldr-core").join("scriptMetadata.json");
    let scripts = fs::read_to_string(&scripts_path).map_err(|source| Error::IoAt {
        path: scripts_path.clone(),
        source,
    })?;
    write(
        "directions.txt",
        &mf2_locale_data::extract::directions_table(&likely, &scripts, &tag)?,
    )
}
