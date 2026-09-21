//! `p04` — the P0.4 driver: correctness run, data sizes, entry dumps.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use plural_rules::check::{self, Counts};
use plural_rules::cldr::{self, Cldr, Kind};
use plural_rules::encode::encode;

const PANEL: [&str; 11] = ["en", "es", "de", "fr", "ar", "he", "ja", "hi", "ru", "pl", "cy"];

#[derive(Parser)]
#[command(about = "P0.4 plural probe driver")]
struct Cli {
    /// Directory holding plurals.json and ordinals.json.
    #[arg(long, default_value_os_t = cldr::default_dir())]
    cldr: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run every CLDR sample through parse → encode → evaluate (exit 1 on any failure).
    Check,
    /// Encoded entry sizes: distinct rule sets, min/median/max, the locale panel.
    Sizes {
        /// Write the panel's entries (<loc>.cardinal.bin, <loc>.ordinal.bin, <loc>.both.bin) here.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Print one line per locale.
        #[arg(long)]
        all: bool,
    },
    /// Show a locale's rules and encoded bytes.
    Dump { locale: String },
}

fn print_counts(name: &str, c: &Counts) {
    println!(
        "{name:<9} locales {:>4} | rules {:>4} (+{} other) | listed items {:>5} ({} ranges) | samples {:>6} = {} @integer + {} @decimal ({} with exponent)",
        c.locales,
        c.explicit_rules,
        c.other_rules,
        c.listed_items,
        c.ranges,
        c.samples(),
        c.integer_samples,
        c.decimal_samples,
        c.exponent_samples,
    );
}

fn run_check(cldr: &Cldr) -> ExitCode {
    let r = check::run(cldr);
    println!("CLDR {} — every sample, every category, every locale", cldr.version);
    print_counts("cardinal", &r.cardinal);
    print_counts("ordinal", &r.ordinal);
    let total = r.cardinal.samples() + r.ordinal.samples();
    println!("total samples {total}; assertions {}; failures {}", r.assertions, r.failures.len());
    let failed: BTreeSet<(Kind, &str, &str)> =
        r.failures.iter().map(|f| (f.kind, f.locale.as_str(), f.sample.as_str())).collect();
    println!(
        "samples passing: {} / {} ({:.3} %)",
        total - failed.len(),
        total,
        100.0 * (total - failed.len()) as f64 / total as f64
    );
    for f in &r.failures {
        println!(
            "FAIL {} {} {} {:?}: {}",
            f.kind.name(),
            f.locale,
            f.category.as_str(),
            f.sample,
            f.cause
        );
    }
    for (kind, locale, cat) in &r.unsampled {
        println!("note: {} {locale} `{cat}` has no samples", kind.name());
    }
    let mut hand: BTreeMap<(Kind, &str), usize> = BTreeMap::new();
    for (k, l, _) in &r.hand_semantics_failures {
        *hand.entry((*k, l.as_str())).or_default() += 1;
    }
    println!(
        "audit `hand` semantics (c/e treated as 0) would fail {} samples in {} locales: {}",
        r.hand_semantics_failures.len(),
        hand.len(),
        hand.iter().map(|((k, l), n)| format!("{l}/{}:{n}", k.name())).collect::<Vec<_>>().join(" ")
    );
    if r.failures.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn stats(mut v: Vec<usize>) -> String {
    v.sort_unstable();
    let median = if v.is_empty() {
        0.0
    } else if v.len() % 2 == 1 {
        v[v.len() / 2] as f64
    } else {
        (v[v.len() / 2 - 1] + v[v.len() / 2]) as f64 / 2.0
    };
    format!(
        "min {} / median {} / max {} / mean {:.1} (n={})",
        v.first().unwrap_or(&0),
        median,
        v.last().unwrap_or(&0),
        v.iter().sum::<usize>() as f64 / v.len().max(1) as f64,
        v.len()
    )
}

fn entry(cldr: &Cldr, kind: Kind, locale: &str) -> (Vec<u8>, String) {
    match cldr.resolve(kind, locale) {
        Some((used, lr)) => (encode(&lr.rules), used.to_owned()),
        None => (Vec::new(), "root".to_owned()),
    }
}

fn run_sizes(cldr: &Cldr, out: Option<PathBuf>, all: bool) -> std::io::Result<ExitCode> {
    println!("CLDR {}", cldr.version);
    for kind in Kind::ALL {
        let map = cldr.get(kind);
        let encoded: BTreeSet<Vec<u8>> = map.values().map(|lr| encode(&lr.rules)).collect();
        let texts: BTreeSet<Vec<(String, String)>> = map
            .values()
            .map(|lr| {
                let mut v: Vec<(String, String)> = lr
                    .raw
                    .iter()
                    .map(|(c, t)| (c.clone(), t.split('@').next().unwrap_or("").trim().to_owned()))
                    .collect();
                v.sort();
                v
            })
            .collect();
        let sizes: Vec<usize> = map.values().map(|lr| encode(&lr.rules).len()).collect();
        println!(
            "{:<9} {} locales; distinct rule sets: {} by condition text, {} by encoded bytes; entry bytes {}",
            kind.name(),
            map.len(),
            texts.len(),
            encoded.len(),
            stats(sizes)
        );
        println!(
            "          all distinct entries concatenated: {} B",
            encoded.iter().map(Vec::len).sum::<usize>()
        );
    }
    let only_ordinal: Vec<&String> = cldr.ordinal.keys().filter(|l| !cldr.cardinal.contains_key(*l)).collect();
    println!("locales in ordinals.json but not plurals.json: {only_ordinal:?}");

    // Per locale = every locale of plurals.json; ordinal resolved by subtag
    // truncation, else root (empty entry).
    let mut both = Vec::new();
    let mut fallbacks = 0;
    for locale in cldr.cardinal.keys() {
        let (c, _) = entry(cldr, Kind::Cardinal, locale);
        let (o, used) = entry(cldr, Kind::Ordinal, locale);
        if used != *locale {
            fallbacks += 1;
        }
        both.push(c.len() + o.len());
        if all {
            println!("  {locale:<8} cardinal {:>3} B  ordinal {:>3} B ({used})  sum {:>3} B", c.len(), o.len(), c.len() + o.len());
        }
    }
    println!("per locale (cardinal + ordinal entry, {fallbacks} ordinals by fallback): {}", stats(both.clone()));
    let worst = cldr
        .cardinal
        .keys()
        .map(|l| (entry(cldr, Kind::Cardinal, l).0.len() + entry(cldr, Kind::Ordinal, l).0.len(), l))
        .max();
    println!("largest: {worst:?}");

    println!("panel (raw entry bytes; container framing not included):");
    println!("  locale  cardinal  ordinal   sum");
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir)?;
    }
    for locale in PANEL {
        let (c, _) = entry(cldr, Kind::Cardinal, locale);
        let (o, used) = entry(cldr, Kind::Ordinal, locale);
        println!("  {locale:<7} {:>8}  {:>7}  {:>4}   (ordinal from {used})", c.len(), o.len(), c.len() + o.len());
        if let Some(dir) = &out {
            std::fs::write(dir.join(format!("{locale}.cardinal.bin")), &c)?;
            std::fs::write(dir.join(format!("{locale}.ordinal.bin")), &o)?;
            std::fs::write(dir.join(format!("{locale}.both.bin")), [c, o].concat())?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn run_dump(cldr: &Cldr, locale: &str) -> ExitCode {
    for kind in Kind::ALL {
        match cldr.resolve(kind, locale) {
            Some((used, lr)) => {
                println!("{} ({used}):", kind.name());
                for (cat, text) in &lr.raw {
                    println!("  {cat:<5} {text}");
                }
                let bytes = encode(&lr.rules);
                let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
                println!("  encoded {} B: {}", bytes.len(), hex.join(" "));
            }
            None => println!("{}: no data (root: everything is `other`, empty entry)", kind.name()),
        }
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let cldr = match cldr::load(&cli.cldr) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    match cli.cmd {
        Cmd::Check => run_check(&cldr),
        Cmd::Sizes { out, all } => match run_sizes(&cldr, out, all) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Cmd::Dump { locale } => run_dump(&cldr, &locale),
    }
}
