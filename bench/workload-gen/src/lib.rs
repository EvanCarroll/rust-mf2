//! Deterministic generator of the reference workload: locales in the working `.mf2` grammar,
//! the same messages as flat JSON, a cargo-leptos app
//! with call sites in the seven measured positions behind a pluggable
//! template, and the committed corpora under `bench/corpora/`; the same
//! locales as Fluent `.ftl` files on request.
//!
//! Same knobs and seed ⇒ byte-identical output. See `README.md`.

pub mod appgen;
pub mod error;
pub mod fluent;
pub mod json;
pub mod knobs;
pub mod locale;
pub mod model;
pub mod output;
pub mod resource;
pub mod rng;
pub mod shape;
pub mod sites;
pub mod stats;
pub mod suite;
pub mod template;
pub mod text;
pub mod vocab;

use std::path::PathBuf;

pub use error::Error;
pub use knobs::Knobs;
pub use model::Workload;
pub use output::Files;
pub use sites::SitePlan;
pub use template::Template;

/// The repository root (this crate lives in `bench/workload-gen`).
pub fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(std::path::Path::parent)
        .map_or(manifest.clone(), std::path::Path::to_path_buf)
}

/// Directory name of a template's app crate inside an output directory.
pub fn app_dir(template: &Template) -> String {
    format!("app-{}", template.name)
}

/// The resource files a run writes besides the flat JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// `locales/<tag>/<namespace>.mf2`.
    Mf2,
    /// `ftl/<tag>/<namespace>.ftl`: the same messages as Fluent
    /// (`src/fluent.rs`).
    Ftl,
}

/// `locales/<tag>/<namespace>.mf2` and `json/<tag>.json` for every locale.
pub fn locale_files(wl: &Workload) -> Result<Files, Error> {
    resource_files(wl, &[Format::Mf2])
}

/// `json/<tag>.json` for every locale, and its resource files in each of
/// `formats`.
pub fn resource_files(wl: &Workload, formats: &[Format]) -> Result<Files, Error> {
    let locales = locale::locales(&wl.knobs)?;
    let en_sources = locale::sources(wl, &locales[0]);
    let comments: Vec<resource::Comments> = (0..wl.files.len())
        .map(|fi| resource::plan_comments(wl, fi, &en_sources))
        .collect();
    let (fluent_ids, fluent_comments) = if formats.contains(&Format::Ftl) {
        let ids = fluent::ids(wl)?;
        let en_bodies = locale::bodies(wl, &locales[0]);
        let comments = (0..wl.files.len())
            .map(|fi| fluent::plan_comments(wl, fi, &ids, &en_bodies))
            .collect();
        (ids, comments)
    } else {
        (Vec::new(), Vec::new())
    };
    let mut files = Files::new();
    for loc in &locales {
        let bodies = locale::bodies(wl, loc);
        let sources: Vec<String> = bodies
            .iter()
            .zip(&wl.messages)
            .map(|(body, m)| model::render_body(body, &m.vars))
            .collect();
        for (fi, file) in wl.files.iter().enumerate() {
            if formats.contains(&Format::Mf2) {
                files.insert(
                    format!("locales/{}/{}.mf2", loc.tag, file.namespace),
                    resource::write_file(wl, fi, loc.tag, &sources, &comments[fi]),
                );
            }
            if formats.contains(&Format::Ftl) {
                files.insert(
                    format!("ftl/{}/{}.ftl", loc.tag, file.namespace),
                    fluent::write_file(wl, fi, &fluent_ids, &bodies, &fluent_comments[fi]),
                );
            }
        }
        files.insert(format!("json/{}.json", loc.tag), flat_json(wl, &sources));
    }
    Ok(files)
}

/// Flat JSON (`id → source`) in `MsgId` (sorted-id) order.
pub fn flat_json(wl: &Workload, sources: &[String]) -> String {
    json::flat_object(
        wl.order
            .iter()
            .map(|&j| (wl.messages[j].id.as_str(), sources[j].as_str())),
    )
}

/// The source-locale corpus as committed to `bench/corpora/workload-<N>.json`.
pub fn corpus_json(wl: &Workload) -> Result<String, Error> {
    let locales = locale::locales(&wl.knobs)?;
    Ok(flat_json(wl, &locale::sources(wl, &locales[0])))
}

/// Everything for one output directory: locales, JSON, `sites.json`, and one
/// app crate per template (`app-<name>/`).
pub fn generate(knobs: &Knobs, templates: &[Template]) -> Result<Files, Error> {
    generate_as(knobs, &[Format::Mf2], templates)
}

/// [`generate`], with the resource files in each of `formats`.
pub fn generate_as(
    knobs: &Knobs,
    formats: &[Format],
    templates: &[Template],
) -> Result<Files, Error> {
    let wl = Workload::generate(knobs)?;
    let mut files = resource_files(&wl, formats)?;
    if !templates.is_empty() {
        let plan = SitePlan::generate(&wl)?;
        files.insert("sites.json", appgen::sites_json(&wl, &plan));
        for template in templates {
            files.extend_under(&app_dir(template), appgen::app_files(&wl, &plan, template)?);
        }
    }
    Ok(files)
}

/// The full statistics report for `knobs` (locales, files and call sites).
pub fn report(knobs: &Knobs) -> Result<stats::Report, Error> {
    let wl = Workload::generate(knobs)?;
    let locales = locale::locales(knobs)?;
    let files = locale_files(&wl)?;
    let en = locale::sources(&wl, &locales[0]);
    let corpus: Vec<(String, String)> = wl
        .order
        .iter()
        .map(|&j| (wl.messages[j].id.clone(), en[j].clone()))
        .collect();
    let mut report = stats::Report::default();
    stats::corpus(&mut report, &corpus);
    let tags: Vec<&str> = locales.iter().map(|l| l.tag).collect();
    stats::resources(&mut report, &files, "locales", &tags, wl.files.len());
    let plan = SitePlan::generate(&wl)?;
    stats::sites(&mut report, &wl, &plan);
    Ok(report)
}

/// [`report`], measuring the Fluent files (`ftl/<tag>/*.ftl`) in place of the
/// `.mf2` corpus: the source locale's files parsed with `fluent-syntax`.
pub fn report_ftl(knobs: &Knobs) -> Result<stats::Report, Error> {
    let wl = Workload::generate(knobs)?;
    let locales = locale::locales(knobs)?;
    let files = resource_files(&wl, &[Format::Ftl])?;
    let tags: Vec<&str> = locales.iter().map(|l| l.tag).collect();
    let mut report = stats::Report::default();
    ftl_rows(&mut report, &files, &tags, knobs.files)?;
    let plan = SitePlan::generate(&wl)?;
    stats::sites(&mut report, &wl, &plan);
    Ok(report)
}

/// The Fluent rows for files already on disk, `dir/<tag>/*.ftl`, every
/// locale directory that is there (the source locale `en` first).
pub fn report_ftl_dir(knobs: &Knobs, dir: &std::path::Path) -> Result<stats::Report, Error> {
    let mut files = Files::new();
    let mut tags = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(tag) = path.file_name().and_then(|t| t.to_str()) else {
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        tags.push(tag.to_owned());
        for file in std::fs::read_dir(&path)? {
            let file = file?.path();
            if file.extension().is_some_and(|e| e == "ftl") {
                let name = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                files.insert(format!("ftl/{tag}/{name}"), std::fs::read(&file)?);
            }
        }
    }
    tags.sort_by_key(|t| (t != "en", t.clone()));
    if tags.first().map(String::as_str) != Some("en") {
        return Err(Error::NotFound(dir.join("en")));
    }
    let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
    let mut report = stats::Report::default();
    ftl_rows(&mut report, &files, &tags, knobs.files)?;
    Ok(report)
}

fn ftl_rows(
    report: &mut stats::Report,
    files: &Files,
    tags: &[&str],
    expected_files: usize,
) -> Result<(), Error> {
    let prefix = format!("ftl/{}/", tags[0]);
    let mut texts = Vec::new();
    for (path, bytes) in files.iter() {
        if path.starts_with(&prefix) {
            let text = std::str::from_utf8(bytes).map_err(|e| Error::Fluent {
                file: path.to_owned(),
                message: e.to_string(),
            })?;
            texts.push((path, text));
        }
    }
    stats::ftl_corpus(report, texts)?;
    // Every other locale must parse cleanly too.
    for (path, bytes) in files.iter() {
        if path.starts_with("ftl/") && !path.starts_with(&prefix) {
            stats::parse_ftl(path, &String::from_utf8_lossy(bytes))?;
        }
    }
    stats::resources(report, files, "ftl", tags, expected_files);
    Ok(())
}
