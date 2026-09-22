//! The `.mf2` loader: a directory of W3C Message Resource files.

use std::path::Path;

use mf2_resource::{Style, code, parse, serialize_with};

use crate::error::{Error, Result};
use crate::loader::{Loaded, Loader, Problem, Property, Record, SourceFile};

/// `locales/<tag>/*.mf2`, read with `mf2-resource`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Resources;

/// How `mf2 fmt` writes a resource: values wrapped at 76 bytes, which is
/// what `bench/workload-gen` emits and what keeps a long message reviewable.
pub const FMT_STYLE: Style = Style {
    wrap: Some(76),
    indent: "  ",
};

impl Loader for Resources {
    fn name(&self) -> &'static str {
        "W3C Message Resource"
    }

    fn load(&self, path: &Path) -> Result<Loaded> {
        let mut loaded = Loaded::default();
        for file in files_in(path)? {
            let file = SourceFile::read(file)?;
            let index = loaded.files.len();
            read_into(&mut loaded, index, &file);
            loaded.files.push(file);
        }
        Ok(loaded)
    }
}

/// The `.mf2` files of `dir`, sorted by name so that a build reads them in
/// the same order everywhere (F8).
fn files_in(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|source| Error::io(dir.to_path_buf(), source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io(dir.to_path_buf(), source))?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "mf2") {
            files.push(path);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(Error::Layout(format!("{}: no .mf2 files", dir.display())));
    }
    Ok(files)
}

fn read_into(loaded: &mut Loaded, index: usize, file: &SourceFile) {
    let (resource, diagnostics) = parse(&file.text);
    for d in diagnostics {
        loaded.problems.push(Problem {
            file: index,
            span: d.span,
            code: d.code,
            message: code::message(d.code),
        });
    }
    loaded.declared.push(resource.locale().map(str::to_owned));
    for entry in resource.iter() {
        let id = entry.id().to_string();
        let e = entry.entry;
        loaded.records.push(Record {
            id,
            source: e.value.to_string(),
            file: index,
            span: e.span,
            id_span: e.id_span,
            value_span: e.value_span,
            map: e.map.clone(),
            comment: e.comment.as_ref().map(|c| c.text.to_string()),
            meta: e
                .meta
                .iter()
                .map(|m| Property {
                    name: m.name.to_string(),
                    value: m.value.as_ref().map(ToString::to_string),
                })
                .collect(),
        });
    }
}

/// One file, formatted canonically (`mf2 fmt`): `None` when it has a syntax
/// error, since nothing may rewrite a file it did not fully understand.
pub fn format(text: &str) -> Option<String> {
    let (resource, diagnostics) = parse(text);
    if !diagnostics.is_empty() {
        return None;
    }
    serialize_with(&resource, &FMT_STYLE).ok()
}
