//! `mf2 pseudo`: the pseudo-locales, written from the source locale.

use std::path::Path;

use clap::Args as ClapArgs;
use mf2_build::Layout;
use mf2_build::loader::{Loader, resource};
use mf2_build::{Config, pseudo};
use mf2_resource::{parse, serialize_with};

use crate::error::{Error, Result, write};

/// `mf2 pseudo`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    /// Which to write; both by default.
    #[arg(long, value_name = "TAG")]
    locale: Vec<String>,
    /// Say what would be written and write nothing.
    #[arg(long)]
    dry_run: bool,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let config = Config::load(dir)?;
    let layout = Layout::new(dir);
    let kinds: Vec<pseudo::Kind> = if args.locale.is_empty() {
        pseudo::Kind::ALL.to_vec()
    } else {
        args.locale
            .iter()
            .map(|tag| {
                pseudo::Kind::from_tag(tag).ok_or_else(|| {
                    Error::Usage(format!("{tag}: not a pseudo-locale (en-XA, ar-XB)"))
                })
            })
            .collect::<Result<Vec<_>>>()?
    };

    let source = layout.locales.join(&config.source_locale);
    if !source.is_dir() {
        return Err(Error::Usage(format!(
            "{}: the source locale is not a directory of .mf2 files; `mf2 pseudo` \
             writes resources, so import it as one first",
            source.display()
        )));
    }
    let loaded = resource::Resources.load(&source)?;

    for kind in kinds {
        let mut written = 0usize;
        let mut messages = 0usize;
        for file in &loaded.files {
            let (resource, diagnostics) = parse(&file.text);
            if !diagnostics.is_empty() {
                return Err(Error::Usage(format!(
                    "{}: has a syntax error; run `mf2 check` first",
                    file.path.display()
                )));
            }
            let mut refused = Vec::new();
            // A message marked `@do-not-translate` (itself, or its section or
            // file) is copied as it stands: the check holds every language to
            // the source's text for it, and it needs no translating.
            let file_dnt = has_dnt(&resource.meta);
            let out = resource.map_values(|info, value| {
                if file_dnt
                    || has_dnt(info.meta)
                    || info.section.is_some_and(|head| has_dnt(&head.meta))
                {
                    return value;
                }
                let Some(model) = mf2_syntax::parse_model(&value).message else {
                    refused.push(info.full_id().to_string());
                    return value;
                };
                if let Ok(source) = mf2_syntax::serialize(&pseudo::message(&model, kind)) {
                    messages += 1;
                    std::borrow::Cow::Owned(source)
                } else {
                    refused.push(info.full_id().to_string());
                    value
                }
            });
            for id in &refused {
                eprintln!("mf2 pseudo: {id}: left as it stands (it does not parse)");
            }
            let mut out = out;
            set_locale(&mut out, kind.tag());
            let text = serialize_with(&out, &resource::FMT_STYLE)
                .map_err(|e| Error::Usage(format!("{}: {e}", file.path.display())))?;
            let name = file
                .path
                .file_name()
                .map(std::ffi::OsStr::to_os_string)
                .unwrap_or_default();
            let target = layout.locales.join(kind.tag()).join(&name);
            if args.dry_run {
                println!("would write {}", target.display());
            } else {
                write(&target, &text)?;
            }
            written += 1;
        }
        eprintln!(
            "mf2 pseudo: {} — {messages} message(s) in {written} file(s){}",
            kind.tag(),
            if args.dry_run { " (dry run)" } else { "" }
        );
    }
    Ok(())
}

fn has_dnt(meta: &[mf2_resource::Meta<'_>]) -> bool {
    meta.iter().any(|m| m.name == "do-not-translate")
}

/// The written resource says which locale it is.
fn set_locale(resource: &mut mf2_resource::Resource<'_, std::borrow::Cow<'_, str>>, tag: &str) {
    let tag = std::borrow::Cow::Owned(tag.to_owned());
    match resource.meta.iter_mut().find(|m| m.name == "locale") {
        Some(meta) => meta.value = Some(tag),
        None => resource.meta.insert(
            0,
            mf2_resource::Meta {
                name: std::borrow::Cow::Borrowed("locale"),
                value: Some(tag),
                span: mf2_resource::Span { start: 0, end: 0 },
                value_span: None,
            },
        ),
    }
}
