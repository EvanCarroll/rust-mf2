//! One catalog per locale: fallbacks flattened, locale data sliced, the
//! bytes written, compressed and content-hashed.

use std::borrow::Cow;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use mf2_catalog::writer::{self, Options};
use mf2_catalog::{Dir, Manifest};
use mf2_locale_data::{CLDR_VERSION, direction, locale_entries};
use mf2_model::{Message, Pattern, PatternMessage};

use crate::config::{Config, Missing, Strip};
use crate::error::{Error, Result};
use crate::features::Place;
use crate::slice::Slice;

/// Brotli as `plans/06-size-and-perf.md` §3 measures B7: quality 11, window
/// 22 — what a server has on disk and hands to a client that says
/// `Accept-Encoding: br`.
const BROTLI_QUALITY: u32 = 11;
const BROTLI_WINDOW: u32 = 22;
/// A debug build's brotli (plans/19 §11): on a 197 KB catalog, the brotli
/// CLI took 3 % of quality 11's time at quality 5, for 12 % more bytes;
/// quality 9 took 2.4 times quality 5's, for 3 % fewer.
const BROTLI_FAST_QUALITY: u32 = 5;

/// How [`write`] compresses a catalog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Compress {
    /// Not at all: nothing serves it (a module-only or native build).
    No,
    /// At a fast level: a debug build, served by a development server.
    Fast,
    /// As B7 is measured: what a release serves.
    Best,
}

/// One locale's catalog and what it cost.
#[derive(Clone)]
pub struct Catalog {
    /// The BCP 47 tag.
    pub tag: String,
    /// The `.mf2b` bytes.
    pub bytes: Vec<u8>,
    /// Brotli 11.
    pub br: Vec<u8>,
    /// gzip, best.
    pub gz: Vec<u8>,
    /// The content hash that names the file.
    pub hash: String,
    /// Messages this locale does not have of its own.
    pub missing: usize,
    /// Of those, how many took another locale's text (D5).
    pub fallbacks: usize,
    /// The LOCALE entries it carries: `(key, bytes)`, ascending by key —
    /// what `mf2 stats` breaks down.
    pub locale_entries: Vec<(u32, usize)>,
    /// The server-only table (`plan/08` §4.2): the LOCALE entries only
    /// native code reads, in the LOCALE section's encoding, written beside
    /// the catalog and embedded in the server. Empty when there is none.
    /// Neither the catalog's bytes nor its hash cover it.
    pub server: Vec<u8>,
    /// The table's entries, as [`Catalog::locale_entries`].
    pub server_entries: Vec<(u32, usize)>,
    /// What its locale data covers.
    pub slice: Slice,
}

impl Catalog {
    /// `<locale>.<content-hash>.mf2b`, the name the server publishes
    /// (`plans/02-catalog-format.md` §3).
    pub fn file_name(&self) -> String {
        format!("{}.{}.mf2b", self.tag, self.hash)
    }

    /// `<locale>.<content-hash>.mf2b.server`, where the build writes the
    /// server-only table beside the catalog, when there is one. A site
    /// (`Outcome::publish`) never holds it.
    pub fn server_file_name(&self) -> Option<String> {
        (!self.server.is_empty()).then(|| format!("{}.server", self.file_name()))
    }
}

impl core::fmt::Debug for Catalog {
    /// Sizes, not bytes: a catalog is tens of kilobytes, and a test that
    /// fails should print something a person can read.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Catalog")
            .field("tag", &self.tag)
            .field("hash", &self.hash)
            .field("raw", &self.bytes.len())
            .field("br", &self.br.len())
            .field("gz", &self.gz.len())
            .field("missing", &self.missing)
            .field("fallbacks", &self.fallbacks)
            .field("locale_entries", &self.locale_entries)
            .field("server", &self.server.len())
            .field("server_entries", &self.server_entries)
            .field("slice", &self.slice)
            .finish()
    }
}

/// Where a message's text came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The locale's own.
    Own,
    /// Another locale's, by the fallback chain.
    Fallback(usize),
    /// The id, because `missing = "id"`.
    Id,
    /// Nothing, because `missing = "empty"`.
    Empty,
}

/// The messages a locale ships, one per `MsgId` index.
///
/// `own[i]` is the locale's own model, `chain` the locales to fall back to
/// in order, each with its own models. The filler owns the messages the
/// `missing` policy invents, so that they outlive the borrowed slice.
pub struct Resolved<'a> {
    /// One message per id, in `MsgId` order.
    pub messages: Vec<Option<&'a Message<'a>>>,
    /// Where each came from.
    pub origins: Vec<Origin>,
}

/// The messages the `missing` policy invents: the id as text, or nothing.
#[derive(Debug, Default)]
pub struct Filler {
    messages: Vec<Message<'static>>,
}

impl Filler {
    /// A filler for `ids` under `missing`.
    pub fn new(ids: &[String], missing: Missing) -> Filler {
        let messages = match missing {
            Missing::Id => ids
                .iter()
                .map(|id| {
                    Message::Pattern(PatternMessage {
                        declarations: Vec::new(),
                        pattern: Pattern::from_text(Cow::Owned(id.clone())),
                    })
                })
                .collect(),
            Missing::Empty => ids
                .iter()
                .map(|_| {
                    Message::Pattern(PatternMessage {
                        declarations: Vec::new(),
                        pattern: Pattern::from_text(Cow::Borrowed("")),
                    })
                })
                .collect(),
            Missing::Fallback => Vec::new(),
        };
        Filler { messages }
    }

    fn get(&self, index: usize) -> Option<&Message<'static>> {
        self.messages.get(index)
    }
}

/// Flattens the fallback chain for one locale (D5).
pub fn resolve<'a>(
    ids: &[String],
    own: &[Option<&'a Message<'a>>],
    chain: &[&[Option<&'a Message<'a>>]],
    missing: Missing,
    filler: &'a Filler,
) -> Resolved<'a> {
    let mut messages = Vec::with_capacity(ids.len());
    let mut origins = Vec::with_capacity(ids.len());
    for i in 0..ids.len() {
        if let Some(message) = own.get(i).copied().flatten() {
            messages.push(Some(message));
            origins.push(Origin::Own);
            continue;
        }
        match missing {
            Missing::Fallback => {
                let found = chain
                    .iter()
                    .enumerate()
                    .find_map(|(step, models)| models.get(i).copied().flatten().map(|m| (step, m)));
                if let Some((step, message)) = found {
                    messages.push(Some(message));
                    origins.push(Origin::Fallback(step));
                } else {
                    messages.push(None);
                    origins.push(Origin::Empty);
                }
            }
            Missing::Id => {
                messages.push(filler.get(i));
                origins.push(Origin::Id);
            }
            Missing::Empty => {
                messages.push(filler.get(i));
                origins.push(Origin::Empty);
            }
        }
    }
    Resolved { messages, origins }
}

/// Writes one locale's catalog: the bytes, `.br`, `.gz` and the hash, and
/// the server-only table. `date_slice` is where ICU4X's date slice goes
/// (`plan/08` §4.1), when the corpus needs one.
// Each argument is one independent input of the catalog.
#[allow(clippy::too_many_arguments)]
pub fn write(
    tag: &str,
    manifest: &Manifest,
    resolved: &Resolved<'_>,
    chain_tags: &[String],
    slice: Slice,
    config: &Config,
    compress: Compress,
    date_slice: Place,
) -> Result<Catalog> {
    let dir = direction(tag).map_err(|source| Error::Locale {
        locale: tag.to_owned(),
        source,
    })?;
    let mut options = Options::new(tag, dir);
    options.strip_cold = config.catalog.strip.contains(&Strip::Cold);
    options.strip_ids = config.catalog.strip.contains(&Strip::Ids);
    options.cldr_version = Some(CLDR_VERSION);
    options.locale_entries = locale_entries(tag, &slice.needs)?;
    let mut server_entries: Vec<(u32, Vec<u8>)> = Vec::new();
    #[cfg(feature = "icu-blob")]
    if let Some(entry) = crate::slice::icu_entry(tag, &slice)? {
        match date_slice {
            Place::Catalog => options.locale_entries.push(entry),
            Place::Server => server_entries.push(entry),
            // No side formats with ICU4X, so the slice was not cut.
            Place::Nowhere => {}
        }
    }
    #[cfg(not(feature = "icu-blob"))]
    let _ = date_slice;
    let mut missing = 0;
    let mut fallbacks = 0;
    for (i, origin) in resolved.origins.iter().enumerate() {
        match origin {
            Origin::Own => {}
            Origin::Fallback(step) => {
                missing += 1;
                fallbacks += 1;
                let from = chain_tags
                    .get(*step)
                    .cloned()
                    .unwrap_or_else(|| tag.to_owned());
                options.fallback.push((
                    u32::try_from(i).map_err(|_| {
                        Error::Layout("more messages than a catalog can hold".to_owned())
                    })?,
                    from,
                ));
            }
            Origin::Id | Origin::Empty => missing += 1,
        }
    }
    let bytes =
        writer::catalog(manifest, &resolved.messages, &options).map_err(|source| Error::Write {
            locale: tag.to_owned(),
            source,
        })?;
    // Under `Emit::Module` nothing writes a catalog file, and brotli 11 at a
    // 22-bit window is the most expensive thing in the pass — so the split
    // that was meant to take work off the i18n crate does not pay for output
    // it then discards (`Build::emit`).
    let (br, gz) = match compress {
        Compress::No => (Vec::new(), Vec::new()),
        Compress::Fast => (
            brotli_at(tag, &bytes, BROTLI_FAST_QUALITY)?,
            gzip_at(tag, &bytes, flate2::Compression::fast())?,
        ),
        Compress::Best => (brotli(tag, &bytes)?, gzip(tag, &bytes)?),
    };
    let hash = content_hash(&bytes);
    let server = writer::server_table(&server_entries).map_err(|source| Error::Write {
        locale: tag.to_owned(),
        source,
    })?;
    let sizes = |entries: &[(u32, Vec<u8>)]| {
        let mut sizes: Vec<(u32, usize)> = entries
            .iter()
            .map(|(key, payload)| (*key, payload.len()))
            .collect();
        sizes.sort_unstable();
        sizes
    };
    let locale_entries = sizes(&options.locale_entries);
    let server_entries = sizes(&server_entries);
    Ok(Catalog {
        tag: tag.to_owned(),
        bytes,
        br,
        gz,
        hash,
        missing,
        fallbacks,
        locale_entries,
        server,
        server_entries,
        slice,
    })
}

/// The first 16 hex digits of SHA-256 over the catalog's bytes: shared with
/// `mf2::native`, which checks a catalog file against the name it was loaded
/// under.
pub use mf2_catalog::content_hash;

/// Brotli at the quality and window B7 is measured with.
pub fn brotli(locale: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    brotli_at(locale, bytes, BROTLI_QUALITY)
}

fn brotli_at(locale: &str, bytes: &[u8], quality: u32) -> Result<Vec<u8>> {
    let fail = |source| Error::Compress {
        locale: locale.to_owned(),
        format: "brotli",
        source,
    };
    let mut out = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::new(&mut out, 4096, quality, BROTLI_WINDOW);
        writer.write_all(bytes).map_err(fail)?;
        // `CompressorWriter`'s `Drop` flushes too, but it cannot report; this
        // is where a truncated stream would otherwise pass silently.
        writer.flush().map_err(fail)?;
    }
    Ok(out)
}

/// gzip at its best setting, for clients without brotli.
pub fn gzip(locale: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    gzip_at(locale, bytes, flate2::Compression::best())
}

fn gzip_at(locale: &str, bytes: &[u8], level: flate2::Compression) -> Result<Vec<u8>> {
    let fail = |source| Error::Compress {
        locale: locale.to_owned(),
        format: "gzip",
        source,
    };
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), level);
    encoder.write_all(bytes).map_err(fail)?;
    encoder.finish().map_err(fail)
}

/// Writes `bytes` to `path`, but only if they differ from what is there.
///
/// `build.rs` runs on every compile of the i18n crate; rewriting a file whose
/// bytes did not change would touch its mtime and make cargo rebuild
/// everything downstream for nothing (P0.9).
pub fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool> {
    if let Ok(existing) = std::fs::read(path)
        && existing == bytes
    {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|source| Error::io(parent.to_path_buf(), source))?;
    }
    std::fs::write(path, bytes).map_err(|source| Error::io(path.to_path_buf(), source))?;
    Ok(true)
}

/// Removes the files of `dir` that a build no longer writes, so that an old
/// catalog never lingers under a name the server might still publish.
pub fn remove_stale(dir: &Path, keep: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(removed);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_catalog = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(".mf2b"));
        if is_catalog && !keep.contains(&path) {
            std::fs::remove_file(&path).map_err(|source| Error::io(path.clone(), source))?;
            removed.push(path);
        }
    }
    removed.sort();
    Ok(removed)
}

/// The direction the catalog header carries, for the generated locale table.
pub fn dir_of(tag: &str) -> Result<Dir> {
    direction(tag).map_err(|source| Error::Locale {
        locale: tag.to_owned(),
        source,
    })
}
