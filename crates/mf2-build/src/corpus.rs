//! Reading a corpus: every locale's files, parsed and validated, with each
//! error placed in the file it came from.
//!
//! A message's own diagnostics carry spans inside its MF2 source; the
//! record's [`ValueMap`](mf2_resource::ValueMap) takes those back to the
//! file, so a `}` missing in the middle of a wrapped continuation line is
//! reported where it stands.

use std::path::{Path, PathBuf};

use mf2_model::{ErrorClass, Message};

use crate::config::Config;
use crate::error::Result;
use crate::lint::{Level, Lint};
use crate::loader::{Loaded, Record, for_path};
use crate::report::{Report, Sink};

/// One locale, as it stands on disk.
#[derive(Debug)]
pub struct LocaleSource {
    /// The BCP 47 tag, from the directory or file name.
    pub tag: String,
    /// The directory or file it was read from.
    pub path: PathBuf,
    /// Its files and records.
    pub loaded: Loaded,
}

impl LocaleSource {
    /// The file a record came from.
    pub fn file(&self, record: &Record) -> &crate::loader::SourceFile {
        self.loaded.file(record)
    }

    /// The position in its file of `start..end` inside a record's source.
    pub fn position(&self, record: &Record, start: u32, end: u32) -> mf2_resource::Position {
        let span = record.source_span(start, end);
        self.file(record).position(span.start)
    }
}

/// Reads every locale of `tags` from `locales`, reporting what the container
/// could not read.
pub fn load(
    locales_dir: &Path,
    tags: &[String],
    config: &Config,
    report: &mut Report,
) -> Result<Vec<LocaleSource>> {
    let mut out = Vec::with_capacity(tags.len());
    for tag in tags {
        let dir = locales_dir.join(tag);
        let path = if dir.is_dir() {
            dir
        } else {
            locales_dir.join(format!("{tag}.json"))
        };
        let loader = for_path(&path);
        let loaded = loader.load(&path)?;
        let source = LocaleSource {
            tag: tag.clone(),
            path,
            loaded,
        };
        check_container(&source, config, report);
        out.push(source);
    }
    Ok(out)
}

/// The container's own errors, and whether each file admits to the locale it
/// sits in.
fn check_container(source: &LocaleSource, config: &Config, report: &mut Report) {
    let mut sink = Sink::new(report, &source.tag);
    for problem in &source.loaded.problems {
        let file = &source.loaded.files[problem.file];
        sink.add(
            Level::Error,
            None,
            &file.path,
            file.position(problem.span.start),
            None,
            problem.message,
        );
    }
    let level = config.level(Lint::LocaleMismatch);
    for (file, declared) in source.loaded.files.iter().zip(&source.loaded.declared) {
        if let Some(declared) = declared
            && *declared != source.tag
        {
            sink.add(
                level,
                Some(Lint::LocaleMismatch),
                &file.path,
                file.position(0),
                None,
                format!(
                    "this file declares @locale {declared:?} but sits under {:?}",
                    source.tag
                ),
            );
        }
    }
}

/// Parses every record of one locale.
///
/// `models[i]` is `None` when record `i` has a syntax or data-model error —
/// which is reported — so nothing downstream ever sees a message the spec
/// refuses.
pub fn parse<'a>(source: &'a LocaleSource, report: &mut Report) -> Vec<Option<Message<'a>>> {
    let mut sink = Sink::new(report, &source.tag);
    let mut models = Vec::with_capacity(source.loaded.records.len());
    for record in &source.loaded.records {
        let parsed = mf2_syntax::parse_model(&record.source);
        for d in &parsed.diagnostics {
            let (start, end) = d.span.map_or((0, 0), |s| (s.start, s.end));
            let at = source.position(record, start, end);
            sink.add_invalid(
                &source.file(record).path,
                at,
                Some(&record.id),
                d.kind,
                describe(d),
            );
        }
        models.push(if parsed.diagnostics.is_empty() {
            parsed.message
        } else {
            None
        });
    }
    models
}

/// What a frontend diagnostic says, in words.
fn describe(d: &mf2_model::Diagnostic) -> String {
    let what = match d.kind.class() {
        ErrorClass::Syntax => "syntax error",
        ErrorClass::DataModel => "data model error",
        ErrorClass::Resolution => "resolution error",
        ErrorClass::MessageFunction => "message function error",
    };
    match mf2_syntax::code::describe(d.code) {
        Some(detail) => format!("{what}: {detail}"),
        None => what.to_string(),
    }
}

/// The records of one locale by id, reporting an id that appears twice.
pub fn by_id<'a>(
    source: &'a LocaleSource,
    config: &Config,
    report: &mut Report,
) -> std::collections::BTreeMap<&'a str, usize> {
    let mut sink = Sink::new(report, &source.tag);
    let mut out = std::collections::BTreeMap::new();
    for (i, record) in source.loaded.records.iter().enumerate() {
        if let Some(first) = out.insert(record.id.as_str(), i) {
            let file = &source.loaded.files[record.file];
            let earlier = &source.loaded.files[source.loaded.records[first].file];
            sink.add(
                config.level(Lint::DuplicateId),
                Some(Lint::DuplicateId),
                &file.path,
                file.position(record.id_span.start),
                Some(&record.id),
                format!("this id is already defined in {}", earlier.path.display()),
            );
        }
    }
    out
}
