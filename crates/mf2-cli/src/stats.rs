//! `mf2 stats`: what the corpus costs, locale by locale, and what ships
//! where, bundle by bundle.

use std::path::Path;

use clap::Args as ClapArgs;
use mf2_build::catalog::Bundle;
use mf2_build::{Build, Config, Place};
use mf2_catalog::format::{HEADER_LEN, SECTION_ENTRY_LEN, header, locale_key, section};
use mf2_catalog::nfc_map::NfcMap;

use crate::error::Result;
use crate::{FeatureArgs, Format};

/// The CLDR release the shipped tables come from.
const CLDR: mf2_catalog::CldrVersion = mf2_locale_data::CLDR_VERSION;

/// The `unicode-org/message-format-wg` commit this implementation was built
/// against (`third_party/message-format-wg/PIN`; a test in this crate holds
/// the two together).
pub(crate) const SPEC_COMMIT: &str = "5c4ddb27e726fd7881c1787a632efba83ab0d850";

/// `mf2 stats`.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    // Without `--features`, mf2's, as cargo resolves them for this crate.
    #[command(flatten)]
    features: FeatureArgs,
    /// How to report.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let config = Config::load(dir)?;
    let features = crate::check::features(dir, &args.features);
    // `Build::check` emits for a web build: a browser downloads the
    // catalogs when a browser side is on (`plan/08` §4.1).
    let downloaded = features.has_browser_side();
    let outcome = Build::at(dir, std::env::temp_dir().join("mf2-stats"))
        .config(config)
        .features(features.clone())
        .check()?;
    // What ships where (`plan/08` §7), locale by locale, and the bytes a
    // browser downloads and never reads.
    let bundles: Vec<Vec<Bundle>> = outcome
        .catalogs
        .iter()
        .map(|catalog| catalog.bundles(&features, downloaded))
        .collect();
    let unread = |bundles: &[Bundle]| -> usize {
        bundles
            .iter()
            .filter(|b| b.unread_by_browser(downloaded))
            .map(|b| b.bytes)
            .sum()
    };
    let unread_total: usize = bundles.iter().map(|b| unread(b)).sum();
    // The pseudo-locales' names are the build's, not the corpus's.
    let total = outcome.manifest.ids.len() - outcome.added.len();
    // Coverage counts only the messages that need translating: one marked
    // `@do-not-translate` is neither missing where it is absent nor
    // translated where it is copied.
    let coverage = |tag: &str| outcome.coverage.iter().find(|c| c.tag == tag);
    let do_not_translate = total - outcome.coverage.first().map_or(total, |c| c.translatable);

    match args.format {
        Format::Text => {
            println!(
                "corpus {} — {total} messages{}, source locale {}, manifest {:#018x}",
                dir.display(),
                if do_not_translate > 0 {
                    format!(" ({do_not_translate} marked @do-not-translate)")
                } else {
                    String::new()
                },
                outcome.source_locale,
                outcome.manifest_hash
            );
            println!(
                "CLDR {}.{}.{} · MF2 spec {} · catalog format v{}",
                CLDR.major,
                CLDR.minor,
                CLDR.patch,
                &SPEC_COMMIT[..8],
                mf2_catalog::format::VERSION_MAJOR
            );
            println!(
                "\n{:<8} {:>9} {:>8} {:>9} {:>9} {:>9}  catalog",
                "locale", "coverage", "missing", "raw", "gz", "br"
            );
            for catalog in &outcome.catalogs {
                let (translated, translatable, missing) = coverage(&catalog.tag)
                    .map_or((0, 0, 0), |c| {
                        (c.translated(), c.translatable, c.missing.len())
                    });
                println!(
                    "{:<8} {:>8.1}% {:>8} {:>9} {:>9} {:>9}  {}",
                    catalog.tag,
                    percent(translated, translatable),
                    missing,
                    catalog.bytes.len(),
                    catalog.gz.len(),
                    catalog.br.len(),
                    catalog.file_name()
                );
            }
            println!("\nwhat ships where (raw bytes, who reads it, where it ships):");
            for (catalog, bundles) in outcome.catalogs.iter().zip(&bundles) {
                println!("  {}", catalog.tag);
                for bundle in bundles {
                    println!(
                        "    {:<18} {:>7} B  {:<27}  {}",
                        bundle.name,
                        bundle.bytes,
                        bundle.readers.name(),
                        place_name(bundle.place)
                    );
                }
            }
            if downloaded {
                println!("bytes a browser downloads and never reads: {unread_total} B");
            } else {
                println!(
                    "bytes a browser downloads and never reads: {unread_total} B \
                     (no browser side: native code alone reads these catalogs)"
                );
            }
            println!("\ncanonical equivalence, the keys a decomposed value can reach:");
            for catalog in &outcome.catalogs {
                let (code_points, bytes) = nfc_map(&catalog.bytes);
                let what = if code_points == 0 {
                    "(nothing: only an identical string matches a key)".to_owned()
                } else {
                    format!("{code_points} code points")
                };
                println!("  {:<8} {bytes:>6} B  {what}", catalog.tag);
            }
            let warnings = outcome.report.warnings();
            let errors = outcome.report.errors();
            if warnings > 0 || errors > 0 {
                println!("\n{errors} error(s), {warnings} warning(s) — run `mf2 check`");
            }
        }
        Format::Json => {
            let locales: Vec<serde_json::Value> = outcome
                .catalogs
                .iter()
                .zip(&bundles)
                .map(|(catalog, bundles)| {
                    let entries = entries(catalog);
                    let server = server_entries(catalog);
                    let (translated, missing) = coverage(&catalog.tag)
                        .map_or((0, 0), |c| (c.translated(), c.missing.len()));
                    serde_json::json!({
                        "locale": catalog.tag,
                        "file": catalog.file_name(),
                        "hash": catalog.hash,
                        "messages": translated,
                        "missing": missing,
                        "fallbacks": catalog.fallbacks,
                        "raw": catalog.bytes.len(),
                        "gz": catalog.gz.len(),
                        "br": catalog.br.len(),
                        "nfc_map": {
                            "code_points": nfc_map(&catalog.bytes).0,
                            "bytes": nfc_map(&catalog.bytes).1,
                        },
                        "locale_data": entries
                            .iter()
                            .map(|(name, n)| serde_json::json!({ "entry": name, "bytes": n }))
                            .collect::<Vec<_>>(),
                        "server_file": catalog.server_file_name(),
                        "server_data": server
                            .iter()
                            .map(|(name, n)| serde_json::json!({ "entry": name, "bytes": n }))
                            .collect::<Vec<_>>(),
                        "bundles": bundles
                            .iter()
                            .map(|b| {
                                serde_json::json!({
                                    "bundle": b.name,
                                    "bytes": b.bytes,
                                    "browser": b.readers.browser,
                                    "native": b.readers.native,
                                    "ships": place_name(b.place),
                                })
                            })
                            .collect::<Vec<_>>(),
                        "unread_by_browser": unread(bundles),
                    })
                })
                .collect();
            let report = serde_json::json!({
                "messages": total,
                "do_not_translate": do_not_translate,
                "source_locale": outcome.source_locale,
                "manifest_hash": format!("{:#018x}", outcome.manifest_hash),
                "cldr": format!("{}.{}.{}", CLDR.major, CLDR.minor, CLDR.patch),
                "spec": SPEC_COMMIT,
                "catalog_format": mf2_catalog::format::VERSION_MAJOR,
                "browser_downloads": downloaded,
                "locales": locales,
                "unread_by_browser": unread_total,
                "errors": outcome.report.errors(),
                "warnings": outcome.report.warnings(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            );
        }
    }
    Ok(())
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 100.0;
    }
    #[allow(clippy::cast_precision_loss)]
    {
        part as f64 * 100.0 / whole as f64
    }
}

/// A catalog's NFC section, as code points listed and raw bytes: the
/// canonical-equivalence map (`plan/01` §4.3). Read from the section table
/// directly, so that `stats` needs no manifest hash.
fn nfc_map(bytes: &[u8]) -> (usize, usize) {
    let count = bytes
        .get(header::SECTION_COUNT..header::SECTION_COUNT + 2)
        .and_then(|b| <[u8; 2]>::try_from(b).ok())
        .map_or(0, |b| usize::from(u16::from_le_bytes(b)));
    for i in 0..count {
        let at = HEADER_LEN + i * SECTION_ENTRY_LEN;
        let Some(entry) = bytes.get(at..at + SECTION_ENTRY_LEN) else {
            break;
        };
        let field = |lo: usize| {
            u32::from_le_bytes([entry[lo], entry[lo + 1], entry[lo + 2], entry[lo + 3]]) as usize
        };
        if u16::from_le_bytes([entry[0], entry[1]]) != section::NFC {
            continue;
        }
        let (off, len) = (field(2), field(6));
        let Some(payload) = bytes.get(off..off.saturating_add(len)) else {
            break;
        };
        if let Some(map) = NfcMap::from_bytes(payload) {
            return (map.code_points(), map.len_bytes());
        }
    }
    (0, 0)
}

/// The LOCALE entries of a catalog, named and measured.
fn entries(catalog: &mf2_build::catalog::Catalog) -> Vec<(&'static str, usize)> {
    named(&catalog.locale_entries)
}

/// The entries of a catalog's server-only table, named and measured.
fn server_entries(catalog: &mf2_build::catalog::Catalog) -> Vec<(&'static str, usize)> {
    named(&catalog.server_entries)
}

fn named(entries: &[(u32, usize)]) -> Vec<(&'static str, usize)> {
    entries
        .iter()
        .map(|(key, bytes)| (entry_name(*key), *bytes))
        .collect()
}

/// Where a bundle ships, in words.
fn place_name(place: Place) -> &'static str {
    match place {
        Place::Catalog => "catalog",
        Place::Server => "server-only table",
        Place::Nowhere => "nowhere",
    }
}

/// What a LOCALE key is called (the catalog-format design §4).
fn entry_name(key: u32) -> &'static str {
    match key {
        locale_key::PLURAL_CARDINAL => "plural.cardinal",
        locale_key::PLURAL_ORDINAL => "plural.ordinal",
        locale_key::NUMBER_SYMBOLS => "number.symbols",
        locale_key::NUMBER_PATTERNS => "number.patterns",
        locale_key::CURRENCY_DATA => "currency.data",
        locale_key::UNIT_DATA => "unit.data",
        locale_key::ICU_BLOB => "icu.blob",
        _ => "(unknown)",
    }
}
