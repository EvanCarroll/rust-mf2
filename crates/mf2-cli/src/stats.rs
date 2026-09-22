//! `mf2 stats`: what the corpus costs, locale by locale and entry by entry.

use std::path::Path;

use clap::Args as ClapArgs;
use mf2_build::{Build, Config};
use mf2_catalog::format::locale_key;

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
    #[command(flatten)]
    features: FeatureArgs,
    /// How to report.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

pub(crate) fn run(dir: &Path, args: &Args) -> Result<()> {
    let config = Config::load(dir)?;
    let outcome = Build::at(dir, std::env::temp_dir().join("mf2-stats"))
        .config(config)
        .features(args.features.features())
        .check()?;
    let total = outcome.manifest.ids.len();

    match args.format {
        Format::Text => {
            println!(
                "corpus {} — {total} messages, source locale {}, manifest {:#018x}",
                dir.display(),
                outcome.source_locale,
                outcome.manifest_hash
            );
            println!(
                "CLDR {}.{}.{} · MF2 spec {} · catalog format v1",
                CLDR.major,
                CLDR.minor,
                CLDR.patch,
                &SPEC_COMMIT[..8]
            );
            println!(
                "\n{:<8} {:>9} {:>8} {:>9} {:>9} {:>9}  catalog",
                "locale", "coverage", "missing", "raw", "gz", "br"
            );
            for catalog in &outcome.catalogs {
                let own = total - catalog.missing;
                println!(
                    "{:<8} {:>8.1}% {:>8} {:>9} {:>9} {:>9}  {}",
                    catalog.tag,
                    percent(own, total),
                    catalog.missing,
                    catalog.bytes.len(),
                    catalog.gz.len(),
                    catalog.br.len(),
                    catalog.file_name()
                );
            }
            println!("\nlocale data, entry by entry (raw bytes in the catalog):");
            for catalog in &outcome.catalogs {
                let entries = entries(catalog);
                let total_bytes: usize = entries.iter().map(|(_, n)| n).sum();
                let list = if entries.is_empty() {
                    "(none)".to_owned()
                } else {
                    entries
                        .iter()
                        .map(|(name, n)| format!("{name} {n} B"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                println!("  {:<8} {total_bytes:>6} B  {list}", catalog.tag);
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
                .map(|catalog| {
                    let entries = entries(catalog);
                    serde_json::json!({
                        "locale": catalog.tag,
                        "file": catalog.file_name(),
                        "hash": catalog.hash,
                        "messages": total - catalog.missing,
                        "missing": catalog.missing,
                        "fallbacks": catalog.fallbacks,
                        "raw": catalog.bytes.len(),
                        "gz": catalog.gz.len(),
                        "br": catalog.br.len(),
                        "locale_data": entries
                            .iter()
                            .map(|(name, n)| serde_json::json!({ "entry": name, "bytes": n }))
                            .collect::<Vec<_>>(),
                    })
                })
                .collect();
            let report = serde_json::json!({
                "messages": total,
                "source_locale": outcome.source_locale,
                "manifest_hash": format!("{:#018x}", outcome.manifest_hash),
                "cldr": format!("{}.{}.{}", CLDR.major, CLDR.minor, CLDR.patch),
                "spec": SPEC_COMMIT,
                "catalog_format": 1,
                "locales": locales,
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

/// The LOCALE entries of a catalog, named and measured.
fn entries(catalog: &mf2_build::catalog::Catalog) -> Vec<(&'static str, usize)> {
    catalog
        .locale_entries
        .iter()
        .map(|(key, bytes)| (entry_name(*key), *bytes))
        .collect()
}

/// What a LOCALE key is called (`plans/02-catalog-format.md` §4).
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

#[cfg(test)]
mod tests {
    use super::SPEC_COMMIT;

    /// The pin this crate reports is the one `third_party/` is at.
    #[test]
    fn the_spec_commit_is_the_pinned_one() {
        let pin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../third_party/message-format-wg/PIN");
        let Ok(text) = std::fs::read_to_string(&pin) else {
            // A published crate has no `third_party/`; nothing to compare.
            return;
        };
        let commit = text
            .lines()
            .find_map(|l| l.strip_prefix("commit"))
            .and_then(|l| l.split('=').nth(1))
            .map(str::trim)
            .expect("the PIN names a commit");
        assert_eq!(
            commit,
            SPEC_COMMIT,
            "{} is not what stats reports",
            pin.display()
        );
    }
}
