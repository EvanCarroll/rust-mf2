//! `cargo xtask fuzz-seed`: writes the seed corpora of the fuzz targets
//! (git-ignored):
//!
//! * `fuzz/corpus/parse/` — every `src` of the vendored WG suite and every
//!   message of the committed reference workload, one file each;
//! * `fuzz/corpus/catalog/` — for every suite message that parses, its
//!   one-message catalog (`writer::single`, slots from `analyze`), unstripped
//!   and stripped, and its source — plain, and with an options byte and
//!   mutation instructions after a NUL (the target's source mode);
//!   and the reference workload as one catalog, its manifest built the way
//!   `mf2-build` will — unstripped with a plural entry and fallbacks (every
//!   section), and stripped.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mf2_catalog::writer::{self, Options};
use mf2_catalog::{CldrVersion, Dir, Manifest};
use mf2_conformance::{SUITE_DIR, Suite};

use crate::error::{Error, Result};

/// `en`'s `plural.cardinal` entry (`plans/02-catalog-format.md` §4.1).
const EN_CARDINAL: [u8; 5] = [0x21, 0x01, 0x05, 0x82, 0x01];

pub(crate) fn run(root: &Path) -> Result<()> {
    let suite = Suite::load(&root.join(SUITE_DIR))?;
    let workload_path = root.join("bench/corpora/workload-1600.json");
    let text = fs::read_to_string(&workload_path).map_err(|source| Error::IoAt {
        path: workload_path.clone(),
        source,
    })?;
    let workload: BTreeMap<String, String> =
        serde_json::from_str(&text).map_err(|e| Error::Json {
            path: workload_path.clone(),
            message: e.to_string(),
        })?;

    let dir = create(root, "fuzz/corpus/parse")?;
    let sources = suite
        .tests()
        .iter()
        .map(|t| t.src.as_str())
        .chain(workload.values().map(String::as_str));
    let mut n = 0usize;
    for (i, src) in sources.enumerate() {
        write(&dir, &format!("seed-{i:05}"), src.as_bytes())?;
        n += 1;
    }
    eprintln!("fuzz-seed: wrote {n} files to {}", dir.display());

    let dir = create(root, "fuzz/corpus/catalog")?;
    let mut n = 0usize;
    for (i, t) in suite.tests().iter().enumerate() {
        let Some(model) = mf2_syntax::parse_model(&t.src).message else {
            continue;
        };
        let analysis = mf2_syntax::analyze(&model);
        let slots: Vec<&str> = analysis.externals.iter().map(|s| &*s.nfc).collect();
        let options = Options::new("en", Dir::Ltr);
        let (bytes, _) = writer::single(&model, &slots, &options)?;
        write(&dir, &format!("suite-{i:04}.mf2b"), &bytes)?;
        let (bytes, _) = writer::single(&model, &slots, &options.stripped())?;
        write(&dir, &format!("suite-{i:04}-stripped.mf2b"), &bytes)?;
        write(&dir, &format!("suite-{i:04}.mf2"), t.src.as_bytes())?;
        write(
            &dir,
            &format!("suite-{i:04}-damaged.mf2"),
            &damaged(&t.src, i),
        )?;
        n += 4;
    }
    let (full, stripped) = workload_catalogs(&workload)?;
    write(&dir, "workload.mf2b", &full)?;
    write(&dir, "workload-stripped.mf2b", &stripped)?;
    n += 2;
    eprintln!(
        "fuzz-seed: wrote {n} files to {} (workload catalog: {} B, stripped {} B)",
        dir.display(),
        full.len(),
        stripped.len()
    );
    Ok(())
}

/// The reference workload as one catalog (ids ascending; slots and markup
/// from `analyze`; the functions the messages use), unstripped — with `en`'s
/// plural rule and every 97th message from a fallback locale, so that every
/// section is present — and stripped.
fn workload_catalogs(workload: &BTreeMap<String, String>) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut models = Vec::with_capacity(workload.len());
    let mut manifest = Manifest::default();
    let mut functions = BTreeSet::new();
    for (id, src) in workload {
        let Some(model) = mf2_syntax::parse_model(src).message else {
            return Err(Error::Json {
                path: PathBuf::from("bench/corpora/workload-1600.json"),
                message: format!("message {id:?} does not parse"),
            });
        };
        let a = mf2_syntax::analyze(&model);
        manifest.ids.push(id.clone());
        manifest
            .slots
            .push(a.externals.iter().map(|s| s.nfc.to_string()).collect());
        manifest
            .markup
            .push(a.markup.iter().map(|s| s.nfc.to_string()).collect());
        functions.extend(a.functions.iter().map(|s| s.nfc.to_string()));
        models.push(model);
    }
    manifest.functions = functions.into_iter().collect();
    let refs: Vec<Option<&_>> = models.iter().map(Some).collect();
    let mut options = Options::new("en", Dir::Ltr);
    options.cldr_version = Some(CldrVersion {
        major: 48,
        minor: 2,
        patch: 1,
    });
    options.locale_entries = vec![(1, EN_CARDINAL.to_vec())];
    let stripped = writer::catalog(&manifest, &refs, &options.clone().stripped())?;
    options.fallback = (0..refs.len())
        .step_by(97)
        .map(|i| (u32::try_from(i).unwrap_or(0), String::from("de")))
        .collect();
    let full = writer::catalog(&manifest, &refs, &options)?;
    Ok((full, stripped))
}

/// A source-mode input of the `catalog` target: `src`, NUL, an options byte
/// and four mutation instructions (`[op, pos_lo, pos_hi, val]`), all derived
/// from `seed` so that the corpus is reproducible.
fn damaged(src: &str, seed: usize) -> Vec<u8> {
    let mut x = u64::try_from(seed)
        .unwrap_or(0)
        .wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut out = Vec::with_capacity(src.len() + 18);
    out.extend_from_slice(src.as_bytes());
    out.push(0);
    for _ in 0..17 {
        // xorshift64*
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        out.push(x.wrapping_mul(0x2545_f491_4f6c_dd1d).to_le_bytes()[7]);
    }
    out
}

fn create(root: &Path, rel: &str) -> Result<PathBuf> {
    let dir = root.join(rel);
    fs::create_dir_all(&dir).map_err(|source| Error::IoAt {
        path: dir.clone(),
        source,
    })?;
    Ok(dir)
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = dir.join(name);
    fs::write(&path, bytes).map_err(|source| Error::IoAt { path, source })
}
