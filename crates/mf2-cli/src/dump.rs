//! `mf2 dump`: a catalog back to MF2 source, or to the data model as JSON.

use std::path::PathBuf;

use clap::{Args as ClapArgs, ValueEnum};
use mf2_catalog::{Catalog, Manifest, MsgId};

use crate::error::{Error, Result, read_bytes};

/// What `dump` writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, ValueEnum)]
pub(crate) enum Shape {
    /// MF2 source, one message per line (`id = source`).
    #[default]
    Mf2,
    /// The data model, as the JSON of `spec/data-model/message.json`.
    Json,
}

/// `mf2 dump`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    /// The catalog.
    #[arg(value_name = "FILE.mf2b")]
    catalog: PathBuf,
    /// What to write.
    #[arg(long, value_enum, default_value_t = Shape::Mf2)]
    format: Shape,
    /// The manifest, for the ids of a catalog whose id table was stripped.
    #[arg(long, value_name = "FILE.mf2m")]
    manifest: Option<PathBuf>,
    /// Only this message.
    #[arg(long, value_name = "ID")]
    id: Option<String>,
}

pub(crate) fn run(args: &Args) -> Result<()> {
    let bytes = read_bytes(&args.catalog)?;
    let manifest = match &args.manifest {
        Some(path) => Some(Manifest::read(&read_bytes(path)?).map_err(mf2_build::Error::from)?),
        None => None,
    };
    // A catalog carries the hash it was written against; dumping is a
    // build-side operation, so it is read on its own terms.
    let hash = header_hash(&bytes).ok_or_else(|| {
        Error::Usage(format!(
            "{}: not a catalog (no MF2B header)",
            args.catalog.display()
        ))
    })?;
    let catalog = Catalog::new(bytes, hash).map_err(|source| Error::Catalog {
        path: args.catalog.clone(),
        source,
    })?;

    let count = catalog.message_count();
    let mut out = String::new();
    for index in 0..count {
        let id = name_of(index, &catalog, manifest.as_ref());
        if let Some(wanted) = &args.id
            && id.as_deref() != Some(wanted.as_str())
        {
            continue;
        }
        let message = mf2_catalog::decode(&catalog, MsgId::from_raw(index)).map_err(|source| {
            Error::Decode {
                path: args.catalog.clone(),
                id: index,
                source,
            }
        })?;
        let shown = id.unwrap_or_else(|| format!("#{index}"));
        match args.format {
            Shape::Mf2 => {
                let source = mf2_syntax::serialize(&message).unwrap_or_default();
                out.push_str(&shown);
                out.push_str(" = ");
                out.push_str(&source.replace('\n', "\\n"));
                out.push('\n');
            }
            Shape::Json => {
                let value = serde_json::json!({
                    "id": shown,
                    "message": message,
                });
                out.push_str(&serde_json::to_string(&value).unwrap_or_default());
                out.push('\n');
            }
        }
    }
    if out.is_empty() {
        return Err(Error::Usage(format!(
            "{}: no message matched",
            args.catalog.display()
        )));
    }
    print!("{out}");
    Ok(())
}

/// The id of message `index`: the catalog's own id table, or the manifest's.
fn name_of(index: u32, catalog: &Catalog, manifest: Option<&Manifest>) -> Option<String> {
    if let Some(manifest) = manifest {
        return manifest.ids.get(index as usize).cloned();
    }
    catalog.id_of(MsgId::from_raw(index))
}

/// `manifest_hash` from a catalog's header, so that a catalog can be read
/// without being told what to expect.
fn header_hash(bytes: &[u8]) -> Option<u64> {
    if bytes.get(..4)? != b"MF2B" {
        return None;
    }
    let field = bytes.get(8..16)?;
    Some(u64::from_le_bytes(field.try_into().ok()?))
}
