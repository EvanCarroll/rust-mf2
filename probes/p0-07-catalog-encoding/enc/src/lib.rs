//! P0.7 build side (std): throwaway MF2 parser, data model, manifest, `.mf2b`
//! writer (all layout variants), model-rebuilding decoder, and the corpus
//! loader shared with probes P0.3 and P0.8. Throwaway probe code.

pub mod decode;
pub mod error;
pub mod manifest;
pub mod model;
pub mod parse;
pub mod write;

use std::path::{Path, PathBuf};

pub use error::{Error, ParseError};
use manifest::Manifest;
use model::Message;
use write::{Input, Layout, Sizes};

/// The four generated locales (`cargo xtask gen-workload locales`).
pub const LOCALES: [&str; 4] = ["en", "pl", "en-XA", "ar-XB"];

pub struct LocaleData {
    pub tag: String,
    pub rtl: bool,
    /// UTF-8 bytes of all MF2 source values (plans/06 §2 "text length").
    pub source_bytes: usize,
    /// Sources and parsed messages in MsgId order (`None` = absent).
    pub sources: Vec<Option<String>>,
    pub messages: Vec<Option<Message>>,
    /// LOCALE entries (key, payload): `plural.cardinal` from P0.4's encoder.
    pub locale_entries: Vec<(u32, Vec<u8>)>,
    /// CLDR locale whose plural rules were used.
    pub plural_locale: String,
}

pub struct Corpus {
    pub manifest: Manifest,
    pub locales: Vec<LocaleData>,
}

/// `probes/p0-07-catalog-encoding/corpus/json`.
pub fn default_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/json")
}

fn read_json(path: &Path) -> Result<Vec<(String, String)>, Error> {
    let text = std::fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let obj = v.as_object().ok_or_else(|| Error::Other(format!("{} is not an object", path.display())))?;
    let mut out: Vec<(String, String)> = obj
        .iter()
        .map(|(k, v)| Ok((k.clone(), v.as_str().ok_or_else(|| Error::Other(format!("{k}: not a string")))?.to_owned())))
        .collect::<Result<_, Error>>()?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn parse_all(src: &[(String, String)]) -> Result<Vec<(String, Message)>, Error> {
    src.iter()
        .map(|(id, s)| parse::parse(s).map(|m| (id.clone(), m)).map_err(|source| Error::Parse { id: id.clone(), source }))
        .collect()
}

/// Loads `<dir>/<tag>.json` for each tag; the first tag is the source locale.
pub fn load_corpus(dir: &Path, tags: &[&str]) -> Result<Corpus, Error> {
    let raw: Vec<Vec<(String, String)>> = tags.iter().map(|t| read_json(&dir.join(format!("{t}.json")))).collect::<Result<_, _>>()?;
    let parsed: Vec<Vec<(String, Message)>> = raw.iter().map(|r| parse_all(r)).collect::<Result<_, _>>()?;
    let all: Vec<&[(String, Message)]> = parsed.iter().map(Vec::as_slice).collect();
    let manifest = Manifest::build(&parsed[0], &all)?;
    let cldr = plural_rules::cldr::load(&plural_rules::cldr::default_dir())?;
    let uses_plural = manifest.functions.iter().any(|f| f == "integer" || f == "number" || f == "offset")
        && parsed.iter().flatten().any(|(_, m)| matches!(m, Message::Select { .. }));
    let mut locales = Vec::new();
    for ((tag, r), p) in tags.iter().zip(&raw).zip(parsed) {
        let mut sources = vec![None; manifest.ids.len()];
        let mut messages = vec![None; manifest.ids.len()];
        for ((id, s), (_, m)) in r.iter().zip(p) {
            let i = manifest.msg_id(id).ok_or_else(|| Error::UnknownId { id: id.clone() })?;
            sources[i] = Some(s.clone());
            messages[i] = Some(m);
        }
        let mut locale_entries = Vec::new();
        let mut plural_locale = String::from("root");
        if uses_plural {
            let (used, rules) = match cldr.resolve(plural_rules::cldr::Kind::Cardinal, tag) {
                Some((u, lr)) => (u.to_owned(), plural_rules::encode::encode(&lr.rules)),
                None => ("root".to_owned(), Vec::new()),
            };
            plural_locale = used;
            locale_entries.push((mf2b_format::locale_key::PLURAL_CARDINAL, rules));
        }
        locales.push(LocaleData {
            tag: (*tag).to_owned(),
            rtl: tag.starts_with("ar") || tag.starts_with("he"),
            source_bytes: r.iter().map(|(_, s)| s.len()).sum(),
            sources,
            messages,
            locale_entries,
            plural_locale,
        });
    }
    Ok(Corpus { manifest, locales })
}

impl LocaleData {
    pub fn write(&self, manifest: &Manifest, layout: Layout) -> Result<(Vec<u8>, Sizes), Error> {
        let refs: Vec<Option<&Message>> = self.messages.iter().map(Option::as_ref).collect();
        let input = Input { locale: &self.tag, rtl: self.rtl, messages: &refs, locale_entries: &self.locale_entries };
        write::write(manifest, &input, layout)
    }

    /// Decodes `bytes` and compares everything with this locale's model.
    pub fn verify(&self, manifest: &Manifest, layout: Layout, bytes: &[u8]) -> Result<(), Error> {
        let d = decode::decode(bytes)?;
        let fail = |id: &str| Error::RoundTrip { id: id.to_owned(), locale: self.tag.clone(), variant: layout.name() };
        if d.locale != self.tag || d.rtl != self.rtl || d.manifest_hash != manifest.hash || d.locale_entries != self.locale_entries {
            return Err(fail("<header>"));
        }
        if d.messages.len() != self.messages.len() {
            return Err(fail("<count>"));
        }
        for (i, (a, b)) in d.messages.iter().zip(&self.messages).enumerate() {
            if a != b {
                return Err(fail(&manifest.ids[i]));
            }
        }
        match (&d.ids, layout.strip) {
            (Some(ids), false) if *ids == manifest.ids => Ok(()),
            (None, true) => Ok(()),
            _ => Err(fail("<ids>")),
        }
    }
}
