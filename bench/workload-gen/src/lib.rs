//! Deterministic generator of the reference workload
//! (plans/06-size-and-perf.md §2): locales in the working `.mf2` grammar of
//! plans/05-tooling.md §2, the same messages as flat JSON, a cargo-leptos app
//! with call sites in the seven measured positions behind a pluggable
//! template, and the committed corpora under `bench/corpora/`.
//!
//! Same knobs and seed ⇒ byte-identical output. See `README.md`.

pub mod appgen;
pub mod error;
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

/// `locales/<tag>/<namespace>.mf2` and `json/<tag>.json` for every locale.
pub fn locale_files(wl: &Workload) -> Result<Files, Error> {
    let locales = locale::locales(&wl.knobs)?;
    let en_sources = locale::sources(wl, &locales[0]);
    let comments: Vec<resource::Comments> = (0..wl.files.len())
        .map(|fi| resource::plan_comments(wl, fi, &en_sources))
        .collect();
    let mut files = Files::new();
    for loc in &locales {
        let sources = if matches!(loc.kind, locale::Kind::Source) {
            en_sources.clone()
        } else {
            locale::sources(wl, loc)
        };
        for (fi, file) in wl.files.iter().enumerate() {
            files.insert(
                format!("locales/{}/{}.mf2", loc.tag, file.namespace),
                resource::write_file(wl, fi, loc.tag, &sources, &comments[fi]),
            );
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
    let wl = Workload::generate(knobs)?;
    let mut files = locale_files(&wl)?;
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
    stats::resources(&mut report, &files, &tags, wl.files.len());
    let plan = SitePlan::generate(&wl)?;
    stats::sites(&mut report, &wl, &plan);
    Ok(report)
}
