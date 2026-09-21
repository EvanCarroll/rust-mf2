//! `p07` — P0.7 measurements. See RESULT.md for the commands.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};
use p07_enc::write::{IndexLayout, Layout, StrLayout, section_name};
use p07_enc::{Corpus, Error, LOCALES, default_corpus_dir, load_corpus};

#[derive(Parser)]
struct Cli {
    /// Directory holding `<tag>.json` (default: ../corpus/json).
    #[arg(long)]
    corpus: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Corpus shape: kinds, text bytes, plural data.
    Stats,
    /// Every layout variant × locale: raw / gzip -9 / brotli; verifies each round trip.
    Measure {
        #[arg(long, default_value = "out")]
        out: PathBuf,
    },
    /// Writes the recommended catalogs (stripped and unstripped) for P0.3 / P0.8.
    Emit {
        #[arg(long, default_value = "out/catalogs")]
        out: PathBuf,
    },
}

fn gzip9(data: &[u8]) -> Result<usize, Error> {
    let mut child = Command::new("gzip").args(["-9", "-n", "-c"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    child.stdin.take().ok_or(Error::Other("stdin".into()))?.write_all(data)?;
    let out = child.wait_with_output()?;
    Ok(out.stdout.len())
}

fn brotli11(data: &[u8]) -> Result<usize, Error> {
    let mut out = Vec::new();
    {
        let mut w = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
        w.write_all(data)?;
    }
    Ok(out.len())
}

fn corpus(cli: &Cli) -> Result<Corpus, Error> {
    load_corpus(&cli.corpus.clone().unwrap_or_else(default_corpus_dir), &LOCALES)
}

fn stats(c: &Corpus) {
    println!("manifest: {} ids, {} functions {:?}, hash {:016x}", c.manifest.ids.len(), c.manifest.functions.len(), c.manifest.functions, c.manifest.hash);
    println!("| locale | messages | simple | pattern | select | with markup | source bytes | text-part bytes | plural rules (CLDR locale, bytes) |");
    println!("|---|---|---|---|---|---|---|---|---|");
    for l in &c.locales {
        let (mut s, mut p, mut sel, mut mk, mut tb) = (0, 0, 0, 0, 0);
        for m in l.messages.iter().flatten() {
            tb += p07_enc::model::text_bytes(m);
            if !p07_enc::model::markup_names(m).is_empty() {
                mk += 1;
            }
            if p07_enc::write::simple_text(m).is_some() {
                s += 1;
            } else if matches!(m, p07_enc::model::Message::Select { .. }) {
                sel += 1;
            } else {
                p += 1;
            }
        }
        let pl = l.locale_entries.first().map_or(0, |e| e.1.len());
        println!(
            "| {} | {} | {s} | {p} | {sel} | {mk} | {} | {tb} | {} ({pl} B) |",
            l.tag,
            l.messages.iter().flatten().count(),
            l.source_bytes,
            l.plural_locale
        );
    }
}

fn all_layouts() -> Vec<Layout> {
    let mut v = Vec::new();
    for strs in [StrLayout::PrefixChar, StrLayout::RefLen, StrLayout::Offsets, StrLayout::Nul] {
        for index in [IndexLayout::Fixed, IndexLayout::VarintDelta, IndexLayout::Blocked, IndexLayout::Planes] {
            for split in [false, true] {
                for strip in [true, false] {
                    for rel in [false, true] {
                        v.push(Layout { strs, index, split, strip, dedup: true, rel, sorted: false });
                    }
                }
            }
        }
    }
    for strs in [StrLayout::PrefixChar, StrLayout::Nul] {
        for split in [false, true] {
            for index in [IndexLayout::Fixed, IndexLayout::Planes] {
                v.push(Layout { strs, index, split, strip: true, dedup: false, rel: false, sorted: false });
                v.push(Layout { strs, index, split, strip: true, dedup: true, rel: false, sorted: true });
            }
        }
    }
    v
}

struct Row {
    layout: Layout,
    locale: String,
    raw: usize,
    gz: usize,
    br: usize,
}

fn measure(c: &Corpus, out: &PathBuf) -> Result<(), Error> {
    std::fs::create_dir_all(out)?;
    let mut rows = Vec::new();
    let mut tsv = String::from("variant\tlocale\traw\tgz\tbr\n");
    // Every single-axis deviation from the recommended layout (the tables use them).
    let mut layouts = all_layouts();
    let b = Layout::RECOMMENDED;
    let mut extra = vec![Layout { strip: false, ..b }, Layout { rel: true, ..b }, Layout { dedup: false, sorted: false, ..b }];
    extra.extend([StrLayout::PrefixChar, StrLayout::RefLen, StrLayout::Offsets, StrLayout::Nul].map(|strs| Layout { strs, ..b }));
    extra.extend([IndexLayout::Fixed, IndexLayout::VarintDelta, IndexLayout::Blocked, IndexLayout::Planes].map(|index| Layout { index, ..b }));
    extra.extend([(false, false), (true, false), (false, true), (true, true)].map(|(split, sorted)| Layout { split, sorted, ..b }));
    for l in extra {
        if !layouts.contains(&l) {
            layouts.push(l);
        }
    }
    for layout in layouts {
        for l in &c.locales {
            let (bytes, _) = l.write(&c.manifest, layout)?;
            l.verify(&c.manifest, layout, &bytes)?;
            let (gz, br) = (gzip9(&bytes)?, brotli11(&bytes)?);
            tsv.push_str(&format!("{}\t{}\t{}\t{gz}\t{br}\n", layout.name(), l.tag, bytes.len()));
            rows.push(Row { layout, locale: l.tag.clone(), raw: bytes.len(), gz, br });
        }
    }
    std::fs::write(out.join("sizes.tsv"), &tsv)?;
    eprintln!("{} catalogs written, decoded and compared: all lossless", rows.len());

    let find = |lay: Layout, loc: &str| rows.iter().find(|r| r.layout == lay && r.locale == loc).ok_or(Error::Other(format!("no row {}", lay.name())));
    let base = Layout::RECOMMENDED;
    let plan = Layout::PLAN_BASELINE;
    let mut md = String::new();
    md.push_str(&format!("Recommended layout: `{}`. Plan baseline: `{}`.\n\n", base.name(), plan.name()));
    md.push_str("### B7 per locale\n\n| locale | source bytes | raw limit (1.25 × src + 8 × 1600) | **raw** | **gzip -9** | brotli 11 | unstripped raw / gz / br | plan baseline raw / gz / br |\n|---|---|---|---|---|---|---|---|\n");
    for l in &c.locales {
        let r = find(base, &l.tag)?;
        let u = find(Layout { strip: false, ..base }, &l.tag)?;
        let p = find(plan, &l.tag)?;
        let limit = l.source_bytes * 5 / 4 + 8 * c.manifest.ids.len();
        md.push_str(&format!(
            "| {} | {} | {limit} | {} | {} | {} | {} / {} / {} | {} / {} / {} |\n",
            l.tag, l.source_bytes, r.raw, r.gz, r.br, u.raw, u.gz, u.br, p.raw, p.gz, p.br
        ));
    }
    let table = |md: &mut String, title: &str, variants: Vec<(String, Layout)>, reference: Layout| -> Result<(), Error> {
        md.push_str(&format!("\n### {title}\n\n| variant |"));
        for l in &c.locales {
            md.push_str(&format!(" {} raw / gz / br (Δgz) |", l.tag));
        }
        md.push_str("\n|---|");
        for _ in &c.locales {
            md.push_str("---|");
        }
        md.push('\n');
        for (name, lay) in variants {
            md.push_str(&format!("| {name} |"));
            for l in &c.locales {
                let r = find(lay, &l.tag)?;
                let b = find(reference, &l.tag)?;
                md.push_str(&format!(" {} / {} / {} ({:+}) |", r.raw, r.gz, r.br, r.gz as i64 - b.gz as i64));
            }
            md.push('\n');
        }
        Ok(())
    };
    let nul_plan = Layout { strs: StrLayout::Nul, ..plan };
    let planes = Layout { index: IndexLayout::Planes, ..nul_plan };
    let grouped = Layout { split: true, ..planes };
    table(
        &mut md,
        "From the plan baseline to the recommended layout (cumulative; Δ against the baseline)",
        vec![
            ("plan baseline (prefix-char, fixed INDEX, first-use pool)".into(), plan),
            ("+ NUL-terminated strings".into(), nul_plan),
            ("+ byte-plane INDEX".into(), planes),
            ("+ identifiers grouped first".into(), grouped),
            ("+ pool sorted (= recommended)".into(), base),
        ],
        plan,
    )?;
    table(
        &mut md,
        "String lengths — the F4 conflict (pool must stay one valid `str`); other axes as recommended",
        [StrLayout::PrefixChar, StrLayout::RefLen, StrLayout::Offsets, StrLayout::Nul]
            .into_iter()
            .map(|st| (format!("{st:?}"), Layout { strs: st, ..base }))
            .collect(),
        base,
    )?;
    table(
        &mut md,
        "INDEX; other axes as recommended",
        [IndexLayout::Fixed, IndexLayout::Planes, IndexLayout::Blocked, IndexLayout::VarintDelta]
            .into_iter()
            .map(|i| (format!("{i:?}"), Layout { index: i, ..base }))
            .collect(),
        base,
    )?;
    table(
        &mut md,
        "Pool order (single vs split pools); other axes as recommended",
        vec![
            ("one pool, first-use order".into(), Layout { split: false, sorted: false, ..base }),
            ("identifiers grouped first (= split pools)".into(), Layout { split: true, sorted: false, ..base }),
            ("one pool, sorted".into(), Layout { split: false, sorted: true, ..base }),
            ("grouped + each group sorted (recommended)".into(), base),
        ],
        base,
    )?;
    table(
        &mut md,
        "Stripping, dedup, relative StrRefs; other axes as recommended",
        vec![
            ("recommended (stripped, dedup, absolute refs)".into(), base),
            ("unstripped (+ IDS, front-coded)".into(), Layout { strip: false, ..base }),
            ("relative StrRefs in MESSAGES".into(), Layout { rel: true, ..base }),
            ("no dedup (sorted off)".into(), Layout { dedup: false, sorted: false, ..base }),
            ("dedup (sorted off)".into(), Layout { sorted: false, ..base }),
        ],
        base,
    )?;
    // Section breakdown of the recommended layout.
    md.push_str("\n### Sections, recommended layout (raw bytes; structure = everything before STRINGS)\n\n| locale | header |");
    let (_, sz0) = c.locales[0].write(&c.manifest, base)?;
    for (k, _) in &sz0.sections {
        md.push_str(&format!(" {} |", section_name(*k)));
    }
    md.push_str(" structure raw / gz / br | pool raw / gz / br |\n|---|---|");
    for _ in &sz0.sections {
        md.push_str("---|");
    }
    md.push_str("---|---|\n");
    for l in &c.locales {
        let (bytes, sz) = l.write(&c.manifest, base)?;
        md.push_str(&format!("| {} | {} |", l.tag, sz.header));
        for (_, n) in &sz.sections {
            md.push_str(&format!(" {n} |"));
        }
        let pool = sz.sections.last().map_or(0, |x| x.1);
        let (st, pl) = bytes.split_at(bytes.len() - pool);
        md.push_str(&format!(
            " {} / {} / {} | {} / {} / {} |\n",
            st.len(),
            gzip9(st)?,
            brotli11(st)?,
            pl.len(),
            gzip9(pl)?,
            brotli11(pl)?
        ));
    }
    std::fs::write(out.join("tables.md"), &md)?;
    println!("{md}");
    Ok(())
}

fn emit(c: &Corpus, out: &PathBuf) -> Result<(), Error> {
    std::fs::create_dir_all(out)?;
    for l in &c.locales {
        for (suffix, layout) in [("", Layout::RECOMMENDED), (".full", Layout { strip: false, ..Layout::RECOMMENDED })] {
            let (bytes, _) = l.write(&c.manifest, layout)?;
            l.verify(&c.manifest, layout, &bytes)?;
            let p = out.join(format!("{}{suffix}.mf2b", l.tag));
            std::fs::write(&p, &bytes)?;
            println!("{} {} B", p.display(), bytes.len());
        }
    }
    println!("manifest_hash {:016x}", c.manifest.hash);
    Ok(())
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();
    let c = corpus(&cli)?;
    match &cli.cmd {
        Cmd::Stats => stats(&c),
        Cmd::Measure { out } => measure(&c, out)?,
        Cmd::Emit { out } => emit(&c, out)?,
    }
    Ok(())
}
