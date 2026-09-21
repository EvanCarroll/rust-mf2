//! `icu-data` — the ICU4X (2.3) side of the P0.4 comparison.
//!
//! * `sizes`: per-locale ICU4X plural data — the postcard payload (the audit's
//!   `plural-payload` method) and real `BlobDataProvider` blobs exported with
//!   `icu_provider_export` from the **same vendored CLDR 48.2.1 files** — next
//!   to our encoded entries.
//! * `oracle`: ICU4X's own plural implementation, fed with data generated from
//!   the same CLDR files, as a differential oracle: every CLDR sample plus a
//!   grid of extra numbers, every locale, cardinal and ordinal, against
//!   `plural_eval::select` over our encoding. Also compares ICU4X's compiled data.

mod error;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use clap::{Parser, Subcommand};
use fixed_decimal::{CompactDecimal, Decimal};
use icu_locale_core::Locale;
use icu_plurals::provider::{Baked, PluralsCardinalV1, PluralsOrdinalV1};
use icu_plurals::{PluralCategory, PluralOperands, PluralRules, PluralRulesPreferences};
use icu_provider::prelude::*;
use icu_provider_export::blob_exporter::BlobExporter;
use icu_provider_export::prelude::*;
use icu_provider_source::SourceDataProvider;
use plural_eval::{Category, Operands};
use plural_rules::cldr::{self, Kind};
use plural_rules::encode::encode;

use crate::error::Error;

const PANEL: [&str; 11] = ["en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy"];

#[derive(Parser)]
#[command(about = "P0.4: ICU4X plural data sizes and differential oracle")]
struct Cli {
    /// Root of the vendored cldr-json tree (holds cldr-core/).
    #[arg(long, default_value_os_t = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../third_party/cldr-json"))]
    cldr_root: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// ICU4X payload and blob sizes for the panel, next to ours.
    Sizes {
        /// Write the blobs here (for gzip -9 by scripts/icu.sh).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Differential run against ICU4X over every sample + an extra grid.
    Oracle,
}

fn payload_size<M, P>(provider: &P, locale: &Locale) -> Result<Option<usize>, Error>
where
    M: DataMarker,
    P: DataProvider<M>,
    for<'a> <M::DataStruct as icu_provider::prelude::yoke::Yokeable<'a>>::Output: serde::Serialize,
{
    let dl = DataLocale::from(locale);
    let req = DataRequest { id: DataIdentifierBorrowed::for_locale(&dl), ..Default::default() };
    match DataProvider::<M>::load(provider, req) {
        Ok(resp) => Ok(Some(postcard::to_allocvec(resp.payload.get())?.len())),
        Err(e) if e.kind == DataErrorKind::IdentifierNotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn blob(provider: &SourceDataProvider, locales: &[&str], markers: &[DataMarkerInfo]) -> Result<Vec<u8>, Error> {
    let mut families = Vec::new();
    for l in locales {
        families.push(DataLocaleFamily::single(DataLocale::from(&Locale::try_from_str(l)?)));
    }
    let mut buf = Vec::new();
    ExportDriver::new(
        families,
        DeduplicationStrategy::None.into(),
        icu_locale::LocaleFallbacker::new().static_to_owned(),
    )
    .with_markers(markers.iter().copied())
    .export(provider, BlobExporter::new_with_sink(Box::new(&mut buf)))?;
    Ok(buf)
}

fn fmt_opt(x: Option<usize>) -> String {
    x.map_or_else(|| "—".to_owned(), |v| v.to_string())
}

fn run_sizes(root: &Path, out: Option<PathBuf>) -> Result<(), Error> {
    let source = SourceDataProvider::new_custom().with_cldr(root)?;
    let ours = cldr::load(&root.join("cldr-core/supplemental"))?;
    if let Some(d) = &out {
        std::fs::create_dir_all(d)?;
    }
    println!("per-locale plural data, bytes: ours (encoded entry) vs ICU4X 2.3 postcard payload (compiled data | from CLDR 48.2.1) vs ICU4X blob");
    println!("locale | ours card ord | payload card (baked|source) | payload ord (baked|source) | blob card | blob card+ord");
    for l in PANEL {
        let loc = Locale::try_from_str(l)?;
        let oc = ours.resolve(Kind::Cardinal, l).map_or(0, |(_, r)| encode(&r.rules).len());
        let oo = ours.resolve(Kind::Ordinal, l).map_or(0, |(_, r)| encode(&r.rules).len());
        let pcb = payload_size::<PluralsCardinalV1, _>(&Baked, &loc)?;
        let pcs = payload_size::<PluralsCardinalV1, _>(&source, &loc)?;
        let pob = payload_size::<PluralsOrdinalV1, _>(&Baked, &loc)?;
        let pos = payload_size::<PluralsOrdinalV1, _>(&source, &loc)?;
        let bc = blob(&source, &[l], &[PluralsCardinalV1::INFO])?;
        let bco = blob(&source, &[l], &[PluralsCardinalV1::INFO, PluralsOrdinalV1::INFO])?;
        println!(
            "{l:<6} | {oc:>4} {oo:>4} | {:>4} | {:>4} | {:>4} | {:>4} | {:>5} | {:>5}",
            fmt_opt(pcb),
            fmt_opt(pcs),
            fmt_opt(pob),
            fmt_opt(pos),
            bc.len(),
            bco.len()
        );
        if let Some(d) = &out {
            std::fs::write(d.join(format!("{l}.icu-card.blob")), &bc)?;
            std::fs::write(d.join(format!("{l}.icu-card-ord.blob")), &bco)?;
        }
    }
    let trio = blob(&source, &["en", "ar", "ru"], &[PluralsCardinalV1::INFO])?;
    println!("blob en+ar+ru cardinal: {} B", trio.len());
    if let Some(d) = &out {
        std::fs::write(d.join("en-ar-ru.icu-card.blob"), &trio)?;
    }
    Ok(())
}

fn icu_category(c: PluralCategory) -> Category {
    match c {
        PluralCategory::Zero => Category::Zero,
        PluralCategory::One => Category::One,
        PluralCategory::Two => Category::Two,
        PluralCategory::Few => Category::Few,
        PluralCategory::Many => Category::Many,
        PluralCategory::Other => Category::Other,
    }
}

fn icu_operands(s: &str) -> Option<PluralOperands> {
    if s.contains(['c', 'e']) {
        // ICU4X's CompactDecimal only accepts the `c` separator.
        CompactDecimal::from_str(&s.replace('e', "c")).ok().map(|d| PluralOperands::from(&d))
    } else {
        Decimal::try_from_str(s).ok().map(|d| PluralOperands::from(&d))
    }
}

/// Numbers beyond the CLDR samples: integers 0..=2000 and selected large
/// values, decimals with 1–3 fraction digits, and compact-exponent forms.
fn grid() -> Vec<String> {
    let mut g: Vec<String> = (0..=2000).map(|i| i.to_string()).collect();
    for base in [10_000u64, 100_000, 1_000_000, 10_000_000, 1_000_000_000_000] {
        for d in [0, 1, 2, 5, 11, 21, 101] {
            g.push((base + d).to_string());
            g.push((base * 3 + d).to_string());
        }
    }
    for x in 0..=300 {
        g.push(format!("{}.{}", x / 10, x % 10));
    }
    for x in 0..=300 {
        g.push(format!("{}.{:02}", x / 100, x % 100));
    }
    for x in [0, 1, 5, 10, 21, 100, 1000, 1001, 1010, 1100, 2000] {
        g.push(format!("{}.{:03}", x / 1000, x % 1000));
        g.push(format!("{x}.000"));
        g.push(format!("{x}.500"));
    }
    for s in [
        "1c3", "1.5c3", "2c3", "1c5", "12c5", "1c6", "1.5c6", "2c6", "7c6", "1.0000001c6", "1.1c6", "1c7",
        "1.1c9", "1.20050c3", "3.0001c3", "1e6", "1.1e6", "0c6",
    ] {
        g.push(s.to_owned());
    }
    g
}

fn run_oracle(root: &Path) -> Result<bool, Error> {
    let source = SourceDataProvider::new_custom().with_cldr(root)?;
    let ours = cldr::load(&root.join("cldr-core/supplemental"))?;
    let grid = grid();
    let mut ok = true;
    for kind in Kind::ALL {
        let (mut n_samples, mut n_grid, mut mismatch_source, mut mismatch_baked) = (0usize, 0usize, 0usize, 0usize);
        let mut baked_locales = std::collections::BTreeSet::new();
        // Differing locales where compiled data answered `other` for every input
        // (i.e. the locale is absent from ICU4X's compiled set and fell back to root).
        let mut baked_all_other = std::collections::BTreeSet::new();
        for (locale, lr) in ours.get(kind) {
            let mut baked_only_other = true;
            let loc = Locale::try_from_str(locale)?;
            let prefs = PluralRulesPreferences::from(&loc);
            let (icu_source, icu_baked) = match kind {
                Kind::Cardinal => (
                    PluralRules::try_new_cardinal_unstable(&source, prefs)?,
                    PluralRules::try_new_cardinal(prefs)?,
                ),
                Kind::Ordinal => (
                    PluralRules::try_new_ordinal_unstable(&source, prefs)?,
                    PluralRules::try_new_ordinal(prefs)?,
                ),
            };
            let entry = encode(&lr.rules);
            let mut samples = Vec::new();
            for rule in &lr.rules {
                for list in [&rule.integer, &rule.decimal].into_iter().flatten() {
                    samples.extend(list.expand()?);
                }
            }
            n_samples += samples.len();
            n_grid += grid.len();
            for s in samples.iter().chain(grid.iter()) {
                let (Some(ops), Some(icu_ops)) = (Operands::parse(s.as_bytes()), icu_operands(s)) else {
                    println!("unparsable {s:?}");
                    ok = false;
                    continue;
                };
                let mine = plural_eval::select(&entry, &ops);
                let theirs = icu_category(icu_source.category_for(icu_ops));
                if mine != theirs {
                    mismatch_source += 1;
                    ok = false;
                    println!("MISMATCH {} {locale} {s:?}: ours {} ICU4X {}", kind.name(), mine.as_str(), theirs.as_str());
                }
                let baked = icu_category(icu_baked.category_for(icu_ops));
                baked_only_other &= baked == Category::Other;
                if baked != theirs {
                    mismatch_baked += 1;
                    baked_locales.insert(locale.clone());
                }
            }
            if baked_only_other && baked_locales.contains(locale) {
                baked_all_other.insert(locale.clone());
            }
        }
        println!(
            "{:<8} {} locales: {} samples + {} grid numbers compared; ours vs ICU4X(CLDR 48.2.1 source) mismatches: {}; ICU4X compiled data vs source: {} differences in {} locales {:?}, of which {} answer `other` for everything",
            kind.name(),
            ours.get(kind).len(),
            n_samples,
            n_grid,
            mismatch_source,
            mismatch_baked,
            baked_locales.len(),
            baked_locales,
            baked_all_other.len()
        );
    }
    Ok(ok)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::Sizes { out } => run_sizes(&cli.cldr_root, out).map(|()| true),
        Cmd::Oracle => run_oracle(&cli.cldr_root),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
