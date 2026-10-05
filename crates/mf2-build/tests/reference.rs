//! The user guide's reference pages (Phase 10 F2): every lint, every
//! `mf2.toml` key and every feature of `mf2` has a section of its own, and
//! nothing documented there has gone. The idea
//! came from an audit of `leptos-fluent`: a test that every
//! configuration option has a section in the docs.
//!
//! The keys are read from the configuration's own deserializer (its "unknown
//! field" message names the fields it expects), the lints from
//! [`Lint::ALL`] and the features from `crates/mf2/Cargo.toml`, so a new one
//! fails here until the page has it. Run in the workspace only (the pages
//! are not in the package).
//!
//! The same manifest is held to the table the build and the tools read
//! ([`FAMILIES`] and each domain's [`Backend`]s): a feature of a family
//! written in one and not the other fails here too.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mf2_build::{
    Backend, Config, DateBackend, Error, FAMILIES, Framework, Lint, NumberBackend, domain_features,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn page(name: &str) -> String {
    let path = root().join("docs").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The words in backticks in `text`.
fn ticked(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// A page's headings below its title and the text under each, outside
/// fenced blocks: `(level, heading, body)`.
fn sections(text: &str) -> Vec<(usize, String, String)> {
    let mut out: Vec<(usize, String, String)> = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let level = line.bytes().take_while(|&b| b == b'#').count();
        if !fenced && level > 1 && line[level..].starts_with(' ') {
            out.push((level, line[level + 1..].to_owned(), String::new()));
        } else if let Some(last) = out.last_mut().filter(|_| fenced || level != 1) {
            last.2.push_str(line);
            last.2.push('\n');
        }
    }
    out
}

/// The fenced blocks of `lang` on a page.
fn blocks(text: &str, lang: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(info) = trimmed.strip_prefix("```") {
            match current.take() {
                Some(block) => out.push(block),
                None if info.trim() == lang => current = Some(String::new()),
                None => current = Some(String::from("\u{0}")),
            }
            continue;
        }
        if let Some(block) = &mut current {
            block.push_str(line);
            block.push('\n');
        }
    }
    out.retain(|b| !b.starts_with('\u{0}'));
    out
}

/// The fields the table at `prefix` (`""` for the top level) takes, as its
/// deserializer lists them when refusing a key it does not know; `None`
/// if it is not a table of fields (a map, a list or a value).
fn fields(prefix: &str) -> Option<Vec<String>> {
    let probe = "mf2_reference_probe = 1\n";
    let text = if prefix.is_empty() {
        probe.to_owned()
    } else {
        format!("[{prefix}]\n{probe}")
    };
    match Config::parse(&text, Path::new("mf2.toml")) {
        Err(Error::Config { message, .. }) if message.contains("unknown field") => {
            let (_, expected) = message.split_once("expected")?;
            Some(ticked(expected).into_iter().map(str::to_owned).collect())
        }
        _ => None,
    }
}

/// Every key of `mf2.toml`: `source_locale`, `catalog`, `catalog.strip`, ….
fn config_keys() -> BTreeSet<String> {
    let top = fields("").expect("the top level refuses an unknown key");
    assert!(top.len() > 3, "too few keys read: {top:?}");
    let mut keys = BTreeSet::new();
    for key in top {
        for field in fields(&key).unwrap_or_default() {
            keys.insert(format!("{key}.{field}"));
        }
        keys.insert(key);
    }
    keys
}

/// The keys `configuration.md` has a heading for: a key or a `[table]` in
/// backticks, and a field in backticks in a heading under its table.
fn documented_keys(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut table: Option<(usize, String)> = None;
    for (level, heading, _) in sections(text) {
        if table.as_ref().is_some_and(|(at, _)| level <= *at) {
            table = None;
        }
        for word in ticked(&heading) {
            if let Some(name) = word.strip_prefix('[').and_then(|w| w.strip_suffix(']')) {
                found.insert(name.to_owned());
                table = Some((level, name.to_owned()));
            } else if let Some((_, name)) = &table {
                found.insert(format!("{name}.{word}"));
            } else {
                found.insert(word.to_owned());
            }
        }
    }
    found
}

#[test]
fn every_config_key_has_a_section() {
    let keys = config_keys();
    let documented = documented_keys(&page("configuration.md"));
    let missing: Vec<&String> = keys.difference(&documented).collect();
    assert!(
        missing.is_empty(),
        "docs/configuration.md has no section for {missing:?}"
    );
    let stale: Vec<&String> = documented.difference(&keys).collect();
    assert!(
        stale.is_empty(),
        "docs/configuration.md documents keys mf2.toml does not have: {stale:?}"
    );
}

#[test]
fn every_config_sample_parses() {
    let samples = blocks(&page("configuration.md"), "toml");
    assert!(!samples.is_empty());
    for sample in samples {
        if let Err(e) = Config::parse(&sample, Path::new("mf2.toml")) {
            panic!("docs/configuration.md: {e}\n{sample}");
        }
    }
}

#[test]
fn every_lint_has_a_section_with_its_levels() {
    let text = page("lints.md");
    let mut documented = BTreeSet::new();
    for (_, heading, body) in sections(&text) {
        for word in ticked(&heading) {
            let lint = Lint::from_name(word)
                .unwrap_or_else(|| panic!("docs/lints.md: `{word}` is not a lint"));
            documented.insert(lint);
            let levels = format!(
                "Default `{}`; the lowest `mf2.toml` may set it is `{}`.",
                lint.default_level(),
                lint.floor()
            );
            assert!(
                body.contains(&levels),
                "docs/lints.md: `{word}` should say: {levels}"
            );
            assert!(body.contains("Fix:"), "docs/lints.md: `{word}` has no fix");
        }
    }
    let missing: Vec<&str> = Lint::ALL
        .iter()
        .filter(|l| !documented.contains(l))
        .map(|l| l.name())
        .collect();
    assert!(
        missing.is_empty(),
        "docs/lints.md has no section for {missing:?}"
    );
}

/// The features of `mf2`, each with what it turns on.
fn mf2_features() -> BTreeMap<String, Vec<String>> {
    let path = root().join("crates/mf2/Cargo.toml");
    let manifest: toml::Table = std::fs::read_to_string(&path)
        .expect("crates/mf2/Cargo.toml")
        .parse()
        .expect("a manifest");
    manifest["features"]
        .as_table()
        .expect("[features]")
        .iter()
        .filter(|(name, _)| *name != "default")
        .map(|(name, on)| {
            let on = on
                .as_array()
                .expect("a feature is a list")
                .iter()
                .map(|f| f.as_str().expect("of names").to_owned())
                .collect();
            (name.clone(), on)
        })
        .collect()
}

/// Holds `mf2`'s features of `B`'s domain to the table:
///
/// * every family has a feature for each backend its side offers, and the
///   domain has its own;
/// * a side's framework-free feature turns the domain on, itself or through
///   the weaker backend it implies;
/// * a framework's feature is that framework-free feature under another
///   name, and turns on nothing else;
/// * the manifest has no feature of the domain the table does not offer.
fn manifest_matches_table<B: Backend>(features: &BTreeMap<String, Vec<String>>) {
    let offered = domain_features::<B>();
    for feature in &offered {
        assert!(
            features.contains_key(feature),
            "crates/mf2/Cargo.toml has no `{feature}`, which the table offers"
        );
    }
    for family in &FAMILIES {
        let host = FAMILIES
            .iter()
            .find(|host| host.side == family.side && host.framework == Framework::Host)
            .expect("each side has a framework-free family");
        for &backend in B::offered(family.side) {
            let feature = family.feature(backend);
            let on = &features[&feature];
            if family.framework == Framework::Host {
                let implied = backend.implies().map(|weaker| family.feature(weaker));
                assert!(
                    on.iter()
                        .any(|name| name == B::DOMAIN || Some(name) == implied.as_ref()),
                    "`{feature}` does not turn `{}` on: {on:?}",
                    B::DOMAIN
                );
            } else {
                assert_eq!(
                    on,
                    &[host.feature(backend)],
                    "`{feature}` is `{}` under its framework's name, and nothing else",
                    host.feature(backend)
                );
            }
        }
    }
    let of_domain = format!("-{}-", B::DOMAIN);
    let unknown: Vec<&String> = features
        .keys()
        .filter(|name| name.contains(&of_domain))
        .filter(|name| !offered.contains(name))
        .collect();
    assert!(
        unknown.is_empty(),
        "crates/mf2/Cargo.toml has `{}` features the table does not offer: {unknown:?}",
        B::DOMAIN
    );
}

#[test]
fn every_date_feature_of_mf2_is_in_the_table() {
    manifest_matches_table::<DateBackend>(&mf2_features());
}

#[test]
fn every_number_feature_of_mf2_is_in_the_table() {
    manifest_matches_table::<NumberBackend>(&mf2_features());
}

#[test]
fn every_feature_of_mf2_has_a_section() {
    let manifest = mf2_features();
    let features: BTreeSet<&str> = manifest.keys().map(String::as_str).collect();
    let text = page("features.md");
    let documented: BTreeSet<String> = sections(&text)
        .into_iter()
        .flat_map(|(_, heading, _)| {
            ticked(&heading)
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect();
    let missing: Vec<&&str> = features
        .iter()
        .filter(|f| !documented.contains(**f))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/features.md has no section for {missing:?}"
    );
    let stale: Vec<&String> = documented
        .iter()
        .filter(|d| !features.contains(d.as_str()))
        .collect();
    assert!(
        stale.is_empty(),
        "docs/features.md documents features mf2 does not have: {stale:?}"
    );
}
