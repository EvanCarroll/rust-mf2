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
use crate::features::{DateFormatter, Features, Place, Side};
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
/// the server-only table. `numbers` is where the plural and number entries
/// go, `names` where `currency.data` and `unit.data` go (the number split,
/// `plan/08` §6), and `date_slice` where ICU4X's date slice goes, when the
/// corpus needs them (`plan/08` §4.1).
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
    numbers: Place,
    names: Place,
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
    // `locale_entries` gives the plural and number entries alone.
    let mut server_entries: Vec<(u32, Vec<u8>)> = Vec::new();
    match numbers {
        Place::Catalog => options.locale_entries = locale_entries(tag, &slice.needs)?,
        Place::Server => server_entries = locale_entries(tag, &slice.needs)?,
        Place::Nowhere => {}
    }
    // The number split: the currency and unit entries alone go to the
    // server-only table.
    if numbers == Place::Catalog && names == Place::Server {
        let (moved, kept): (Vec<_>, Vec<_>) = core::mem::take(&mut options.locale_entries)
            .into_iter()
            .partition(|(key, _)| is_name_entry(*key));
        options.locale_entries = kept;
        server_entries.extend(moved);
    }
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

/// `currency.data` or `unit.data`: what the number split (`plan/08` §6)
/// takes from the browser instead.
fn is_name_entry(key: u32) -> bool {
    use mf2_catalog::format::locale_key;
    matches!(key, locale_key::CURRENCY_DATA | locale_key::UNIT_DATA)
}

/// Who reads a LOCALE entry (`plan/08` §4.1). The browser looks only in the
/// catalog it downloads; native code looks in the catalog and, beside a
/// catalog a browser downloads, in the server-only table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readers {
    /// The browser's client reads it, when a browser downloads the catalog.
    pub browser: bool,
    /// Native code reads it: a server, or a native application.
    pub native: bool,
}

impl Readers {
    /// The readers of the entry `key` under `features`, from the table of
    /// `plan/08` §4.1 and not from [`Features::number_place`] or
    /// [`Features::date_slice_place`], so that [`check_read`] holds the
    /// placement to the readers rather than to itself. A key the table does
    /// not name has no reader.
    pub fn of(key: u32, features: &Features) -> Readers {
        use mf2_catalog::format::locale_key;
        match key {
            locale_key::ICU_BLOB => {
                let icu = |side| features.date_formatter(side) == Some(DateFormatter::Icu);
                Readers {
                    browser: icu(Side::Browser),
                    native: icu(Side::Native),
                }
            }
            // A browser formats and selects through `Intl` under
            // `number-intl` and reads none of them; native code reads them
            // on every build whose corpus needs them.
            locale_key::PLURAL_CARDINAL
            | locale_key::PLURAL_ORDINAL
            | locale_key::NUMBER_SYMBOLS
            | locale_key::NUMBER_PATTERNS => Readers {
                browser: !features.number_intl(),
                native: true,
            },
            // The number split (`intl-names`, `plan/08` §6) takes the
            // currency and unit names from `Intl` too.
            locale_key::CURRENCY_DATA | locale_key::UNIT_DATA => Readers {
                browser: !features.number_intl() && !features.intl_names(),
                native: true,
            },
            _ => Readers {
                browser: false,
                native: false,
            },
        }
    }

    /// Whether one of them looks in `place`. `downloaded` is whether a
    /// browser downloads the catalog: then an entry belongs in it only if
    /// the browser reads it, and in the server-only table only if native
    /// code does; without one, native code is the catalog's one reader and
    /// there is no table.
    pub fn look_in(self, place: Place, downloaded: bool) -> bool {
        match place {
            Place::Catalog if downloaded => self.browser,
            Place::Catalog => self.native,
            Place::Server => downloaded && self.native,
            // Nothing is written there.
            Place::Nowhere => true,
        }
    }

    fn name(self) -> &'static str {
        match (self.browser, self.native) {
            (true, true) => "the browser and native code",
            (true, false) => "the browser alone",
            (false, true) => "native code alone",
            (false, false) => "none at all",
        }
    }
}

/// `unread-data` (`plan/08` §7), checked after slicing: fails if one of
/// the entries [`write`] put in the catalog (`catalog`, as
/// [`Catalog::locale_entries`]) or in the server-only table (`table`, as
/// [`Catalog::server_entries`]) went where none of its [`Readers`] looks.
/// `downloaded` is whether a browser downloads the catalog: a browser side
/// is on and the build is not a native application's.
pub fn check_read(
    tag: &str,
    catalog: &[(u32, usize)],
    table: &[(u32, usize)],
    features: &Features,
    downloaded: bool,
) -> Result<()> {
    let placed = catalog
        .iter()
        .map(|(key, _)| (*key, Place::Catalog))
        .chain(table.iter().map(|(key, _)| (*key, Place::Server)));
    for (key, place) in placed {
        let readers = Readers::of(key, features);
        if !readers.look_in(place, downloaded) {
            return Err(Error::UnreadData {
                locale: tag.to_owned(),
                entry: entry_name(key),
                place: match place {
                    Place::Catalog if downloaded => "into the catalog a browser downloads",
                    Place::Catalog => "into the catalog",
                    Place::Server => "into the server-only table",
                    Place::Nowhere => "nowhere",
                },
                readers: readers.name(),
            });
        }
    }
    Ok(())
}

/// The name of the LOCALE entry `key`, or the key itself.
fn entry_name(key: u32) -> String {
    use mf2_catalog::format::locale_key;
    match key {
        locale_key::PLURAL_CARDINAL => "plural.cardinal".to_owned(),
        locale_key::PLURAL_ORDINAL => "plural.ordinal".to_owned(),
        locale_key::NUMBER_SYMBOLS => "number.symbols".to_owned(),
        locale_key::NUMBER_PATTERNS => "number.patterns".to_owned(),
        locale_key::CURRENCY_DATA => "currency.data".to_owned(),
        locale_key::UNIT_DATA => "unit.data".to_owned(),
        locale_key::ICU_BLOB => "icu.blob".to_owned(),
        other => format!("key {other}"),
    }
}

#[cfg(test)]
mod tests {
    use mf2_catalog::format::locale_key;

    use super::check_read;
    use crate::error::Error;
    use crate::features::Features;

    const NUMBERS: [u32; 6] = [
        locale_key::PLURAL_CARDINAL,
        locale_key::PLURAL_ORDINAL,
        locale_key::NUMBER_SYMBOLS,
        locale_key::NUMBER_PATTERNS,
        locale_key::CURRENCY_DATA,
        locale_key::UNIT_DATA,
    ];

    fn entries(keys: &[u32]) -> Vec<(u32, usize)> {
        keys.iter().map(|key| (*key, 8)).collect()
    }

    /// The check on `features`, with the catalog downloaded when a browser
    /// side is on, as `Build` calls it outside a native application.
    fn check(features: &str, catalog: &[u32], table: &[u32]) -> crate::Result<()> {
        let features = Features::parse(features);
        let downloaded = features.has_browser_side();
        check_read(
            "en",
            &entries(catalog),
            &entries(table),
            &features,
            downloaded,
        )
    }

    fn unread(result: crate::Result<()>, entry: &str) {
        match result {
            Err(Error::UnreadData { entry: found, .. }) => assert_eq!(found, entry),
            other => panic!("expected unread-data for {entry}, got {other:?}"),
        }
    }

    // The placements `catalog::write` makes pass.

    #[test]
    fn every_entry_where_its_readers_look_passes() {
        check("hydrate,fn-number", &NUMBERS, &[]).expect("the browser reads them");
        check("hydrate,fn-number,number-intl", &[], &NUMBERS).expect("the server reads them");
        check("native,fn-number,number-intl", &NUMBERS, &[]).expect("one reader, one file");
        check(
            "hydrate,fn-number,intl-names",
            &[
                locale_key::PLURAL_CARDINAL,
                locale_key::NUMBER_SYMBOLS,
                locale_key::NUMBER_PATTERNS,
            ],
            &[locale_key::CURRENCY_DATA, locale_key::UNIT_DATA],
        )
        .expect("the split: the server alone reads the names");
        check(
            "ssr,leptos-client-datetime-intl,leptos-server-datetime-icu",
            &[],
            &[locale_key::ICU_BLOB],
        )
        .expect("only the server formats with ICU4X");
        check(
            "ssr,leptos-client-datetime-icu,leptos-server-datetime-iso",
            &[locale_key::ICU_BLOB],
            &[],
        )
        .expect("the browser formats with ICU4X");
        check("native,native-datetime-icu", &[locale_key::ICU_BLOB], &[])
            .expect("one reader, one file");
    }

    // Deliberately wrong placements: each must fail.

    #[test]
    fn number_entries_in_a_catalog_the_browser_never_reads_fail() {
        unread(
            check("hydrate,fn-number,number-intl", &NUMBERS, &[]),
            "plural.cardinal",
        );
    }

    #[test]
    fn number_entries_in_the_table_while_the_browser_reads_them_fail() {
        unread(
            check("hydrate,fn-number", &[], &[locale_key::NUMBER_SYMBOLS]),
            "number.symbols",
        );
    }

    #[test]
    fn names_in_the_catalog_under_the_split_fail() {
        unread(
            check(
                "hydrate,fn-number,intl-names",
                &[locale_key::CURRENCY_DATA],
                &[],
            ),
            "currency.data",
        );
    }

    #[test]
    fn a_table_with_no_browser_side_fails() {
        unread(
            check("native,fn-number", &[], &[locale_key::UNIT_DATA]),
            "unit.data",
        );
    }

    #[test]
    fn the_date_slice_where_its_one_reader_does_not_look_fails() {
        // Only the server formats with ICU4X: not in the browser's catalog.
        unread(
            check(
                "ssr,leptos-client-datetime-intl,leptos-server-datetime-icu",
                &[locale_key::ICU_BLOB],
                &[],
            ),
            "icu.blob",
        );
        // Only the browser does: not in the server's table.
        unread(
            check(
                "ssr,leptos-client-datetime-icu,leptos-server-datetime-iso",
                &[],
                &[locale_key::ICU_BLOB],
            ),
            "icu.blob",
        );
        // Neither does: nowhere at all.
        unread(
            check("native,native-datetime-iso", &[locale_key::ICU_BLOB], &[]),
            "icu.blob",
        );
    }

    #[test]
    fn an_entry_no_one_knows_fails() {
        unread(check("native,fn-number", &[5], &[]), "key 5");
    }
}
