//! Size mode (A8): B7 on the reference workload — every locale's catalog raw,
//! gzip (three implementations) and brotli, stripped and unstripped, split
//! into structure (everything before STRINGS) and pool (STRINGS), section by
//! section; the B7 checks; the deltas against P0.7's recommended layout; the
//! NAMES `str32`-vs-varint estimate.

use std::fmt::Write as _;
use std::path::Path;

use mf2_catalog::Catalog;
use mf2_catalog::format::section;
use serde::Serialize;

use crate::baseline::{self, P07};
use crate::compress::{Compressed, Compressors, Gz};
use crate::corpus::{self, Built, Kinds, MANIFEST_HASH};
use crate::error::{Error, Result};
use crate::names::{self, Encoding};
use crate::report::{Build, delta, fixed, n, signed, unix_time};

/// B7 is stated on brotli (quality 11, window 22: the `.br` file the build
/// writes and `mf2-axum` serves; owner, 2026-09-21). Its limits are the
/// former gzip limits scaled by `BR_SCALE` = 91/100: the worst brotli/gzip
/// ratio measured on the four locales (en-XA, 0.909 in P0.7 and in Phase 2),
/// rounded up, so no locale is held tighter than it was under gzip.
pub const BR_SCALE: (usize, usize) = (91, 100);

/// B7's absolute bound for the reference `en`: 0.91 × 25 KB = 23,296 B.
pub const EN_BR_MAX: usize = 25 * 1024 * BR_SCALE.0 / BR_SCALE.1;

/// B7's bound for any locale: 0.91 × (0.5 × source + 1 KB).
pub const fn br_limit(source_bytes: usize) -> usize {
    BR_SCALE.0 * (source_bytes + 2048) / (2 * BR_SCALE.1)
}

/// A delta against P0.7 beyond this many brotli bytes is flagged…
pub const FLAG_BYTES: i64 = 100;
/// …and so is one beyond this fraction of P0.7's figure.
pub const FLAG_FRACTION: f64 = 0.01;

/// One section's raw size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionSize {
    /// Section kind.
    pub kind: u16,
    /// Its name (`mf2_catalog::format::section::name`).
    pub name: String,
    /// Bytes.
    pub raw: usize,
}

/// One catalog (stripped or unstripped), measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantSizes {
    /// The whole file.
    pub whole: Compressed,
    /// Everything before STRINGS: header, section table and every other section.
    pub structure: Compressed,
    /// STRINGS.
    pub pool: Compressed,
    /// The fixed header and the section table.
    pub header: usize,
    /// Every section, in file order.
    pub sections: Vec<SectionSize>,
    /// Messages whose `decode` equals the parsed model.
    pub lossless: usize,
}

impl VariantSizes {
    /// Raw size of the section called `name`, if present.
    pub fn section(&self, name: &str) -> Option<usize> {
        self.sections.iter().find(|s| s.name == name).map(|s| s.raw)
    }
}

/// The NAMES estimate: the stripped catalog with varint string references in NAMES.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamesEstimate {
    /// NAMES entries (deduplicated).
    pub entries: usize,
    /// String references in them.
    pub names: usize,
    /// NAMES bytes, format v1 (`str32`).
    pub names_str32: usize,
    /// NAMES bytes with varint string references.
    pub names_varint: usize,
    /// MESSAGES bytes, format v1.
    pub messages_str32: usize,
    /// MESSAGES bytes with the heads re-pointed at the smaller NAMES.
    pub messages_varint: usize,
    /// The whole re-encoded catalog.
    pub whole_varint: Compressed,
    /// NAMES bytes with fixed 2-byte string references.
    pub names_str16: usize,
    /// MESSAGES bytes with the heads re-pointed at the 2-byte NAMES.
    pub messages_str16: usize,
    /// The whole catalog with 2-byte NAMES references.
    pub whole_str16: Compressed,
}

/// Everything measured for one locale.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocaleSizes {
    /// BCP 47 tag.
    pub tag: String,
    /// `ltr` or `rtl`.
    pub dir: &'static str,
    /// Messages (manifest ids).
    pub messages: usize,
    /// INDEX kinds.
    pub kinds: Kinds,
    /// MF2 source bytes.
    pub source_bytes: usize,
    /// CLDR locale of the `plural.cardinal` entry.
    pub plural_locale: &'static str,
    /// Its bytes.
    pub plural_bytes: usize,
    /// Production catalog (COLD and IDS stripped).
    pub stripped: VariantSizes,
    /// With IDS (COLD is empty for this workload).
    pub unstripped: VariantSizes,
    /// NAMES with varint string references.
    pub names: NamesEstimate,
}

/// One threshold, evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    /// Locale.
    pub locale: String,
    /// The rule.
    pub rule: String,
    /// Measured.
    pub value: usize,
    /// Allowed maximum (or, for the round trip, the required count).
    pub limit: usize,
    /// Whether it holds.
    pub passed: bool,
}

/// The stripped catalog against P0.7's recommended layout.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct P07Delta {
    /// Locale.
    pub locale: String,
    /// The gzip implementation of the gz deltas (P0.7 used GNU gzip).
    pub gz: Gz,
    /// Δ raw bytes.
    pub raw: i64,
    /// Δ gz bytes.
    pub gz_bytes: i64,
    /// Δ brotli bytes.
    pub br: i64,
    /// Δ structure gz.
    pub structure_gz: i64,
    /// Δ pool gz.
    pub pool_gz: i64,
    /// Δ unstripped gz.
    pub unstripped_gz: i64,
    /// Δ brotli (the B7 metric) beyond 100 B or 1 % of P0.7's figure.
    pub beyond_noise: bool,
}

/// The whole size run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SizeReport {
    /// `catalog-bench <version>`.
    pub tool: String,
    /// How the harness was built.
    pub build: Build,
    /// Seconds since the Unix epoch.
    pub unix_time: u64,
    /// GNU gzip's `--version` line, if installed.
    pub gnu_gzip: Option<String>,
    /// The gzip implementation of the gz figures (reported, not gated: B7 is
    /// stated on brotli).
    pub gz: Gz,
    /// The manifest's hash (asserted to be P0.7's reference).
    pub manifest_hash: String,
    /// The committed source-locale corpus.
    pub corpus: &'static str,
    /// Per locale.
    pub locales: Vec<LocaleSizes>,
    /// B7 and the round trips.
    pub checks: Vec<Check>,
    /// Against P0.7.
    pub p07: Vec<P07Delta>,
    /// Every check passed.
    pub passed: bool,
}

fn measure(bytes: &[u8], lossless: usize, c: &Compressors) -> Result<VariantSizes> {
    let cat = Catalog::new(bytes.to_vec(), MANIFEST_HASH).map_err(|source| Error::Load {
        tag: "(measure)".to_owned(),
        source,
    })?;
    let table: Vec<(u16, u32, u32)> = cat.sections().collect();
    let header = table.first().map_or(bytes.len(), |s| s.1 as usize);
    let strings = table
        .iter()
        .find(|s| s.0 == section::STRINGS)
        .map_or(bytes.len(), |s| s.1 as usize);
    let (structure, pool) = bytes.split_at(strings);
    Ok(VariantSizes {
        whole: c.all(bytes)?,
        structure: c.all(structure)?,
        pool: c.all(pool)?,
        header,
        sections: table
            .iter()
            .map(|&(kind, _, len)| SectionSize {
                kind,
                name: section::name(kind).map_or_else(|| format!("kind {kind}"), str::to_owned),
                raw: len as usize,
            })
            .collect(),
        lossless,
    })
}

fn locale_sizes(b: &Built, c: &Compressors) -> Result<LocaleSizes> {
    let v = names::reencode(&b.stripped, Encoding::Str32, Encoding::Varint)?;
    let h = names::reencode(&b.stripped, Encoding::Str32, Encoding::Str16)?;
    Ok(LocaleSizes {
        tag: b.tag.clone(),
        dir: match b.dir {
            mf2_catalog::Dir::Rtl => "rtl",
            _ => "ltr",
        },
        messages: b.messages,
        kinds: b.kinds,
        source_bytes: b.source_bytes,
        plural_locale: b.plural.0,
        plural_bytes: b.plural.1,
        stripped: measure(&b.stripped, b.lossless.0, c)?,
        unstripped: measure(&b.unstripped, b.lossless.1, c)?,
        names: NamesEstimate {
            entries: v.entries,
            names: v.names,
            names_str32: v.names_before,
            names_varint: v.names_after,
            messages_str32: v.messages_before,
            messages_varint: v.messages_after,
            whole_varint: c.all(&v.bytes)?,
            names_str16: h.names_after,
            messages_str16: h.messages_after,
            whole_str16: c.all(&h.bytes)?,
        },
    })
}

/// B7 (stripped: raw and brotli) and the round trips, for one locale.
fn checks(l: &LocaleSizes) -> Vec<Check> {
    let src = l.source_bytes;
    let s = &l.stripped;
    let check = |rule: String, value: usize, limit: usize| Check {
        locale: l.tag.clone(),
        passed: value <= limit,
        rule,
        value,
        limit,
    };
    // raw ≤ 1.25 × src + 8 × messages ⟺ raw ≤ ⌊(5 src + 32 messages) / 4⌋.
    let mut out = vec![
        check(
            "B7 raw ≤ 1.25 × source + 8 B/message".to_owned(),
            s.whole.raw,
            (5 * src + 32 * l.messages) / 4,
        ),
        check(
            "B7 brotli ≤ 0.91 × (0.5 × source + 1 KB)".to_owned(),
            s.whole.br,
            br_limit(src),
        ),
    ];
    if l.tag == "en" {
        out.push(check(
            "B7 reference en: brotli ≤ 0.91 × 25 KB".to_owned(),
            s.whole.br,
            EN_BR_MAX,
        ));
    }
    for (name, v) in [("stripped", s), ("unstripped", &l.unstripped)] {
        out.push(Check {
            locale: l.tag.clone(),
            rule: format!("lossless: decode = parse ({name})"),
            value: v.lossless,
            limit: l.messages,
            passed: v.lossless == l.messages,
        });
    }
    out
}

fn p07_delta(l: &LocaleSizes, p: &P07, gz: Gz) -> P07Delta {
    let g = |c: &Compressed| c.gz(gz).unwrap_or(0);
    let d_gz = delta(g(&l.stripped.whole), p.stripped[1]);
    let d_br = delta(l.stripped.whole.br, p.stripped[2]);
    // `as f64` of byte counts far below 2^52 is exact.
    #[allow(clippy::cast_precision_loss)]
    let beyond =
        d_br.abs() > FLAG_BYTES || d_br.abs() as f64 > FLAG_FRACTION * p.stripped[2] as f64;
    P07Delta {
        locale: l.tag.clone(),
        gz,
        raw: delta(l.stripped.whole.raw, p.stripped[0]),
        gz_bytes: d_gz,
        br: d_br,
        structure_gz: delta(g(&l.stripped.structure), p.structure[1]),
        pool_gz: delta(g(&l.stripped.pool), p.pool[1]),
        unstripped_gz: delta(g(&l.unstripped.whole), p.unstripped[1]),
        beyond_noise: beyond,
    }
}

/// Runs the size measurement. `gz` picks the gzip implementation of the
/// reported gz figures (B7 itself is brotli); without GNU gzip installed,
/// `Gz::Gnu` falls back to flate2. With `emit`, the catalogs are also written
/// there: `<tag>.mf2b` (stripped) and `<tag>.full.mf2b`.
pub fn run(
    repo: &Path,
    gz: Gz,
    emit: Option<&Path>,
    mut progress: impl FnMut(&str),
) -> Result<SizeReport> {
    let c = Compressors::detect();
    let gz = if gz == Gz::Gnu && c.gnu_version.is_none() {
        progress("GNU gzip is not installed: the gz figures use flate2 (B7 is brotli)");
        Gz::Flate2
    } else {
        gz
    };
    progress("generating the workload locales");
    let sources = corpus::generate(repo)?;
    progress("parsing, writing, loading and decoding the catalogs");
    let (manifest, built) = corpus::build(&sources, true)?;
    if let Some(dir) = emit {
        std::fs::create_dir_all(dir)?;
        for b in &built {
            std::fs::write(dir.join(format!("{}.mf2b", b.tag)), &b.stripped)?;
            std::fs::write(dir.join(format!("{}.full.mf2b", b.tag)), &b.unstripped)?;
        }
        std::fs::write(dir.join("manifest.mf2m"), manifest.write())?;
        progress(&format!(
            "wrote the catalogs and manifest.mf2m to {}",
            dir.display()
        ));
    }
    let mut locales = Vec::with_capacity(built.len());
    for b in &built {
        progress(&format!("compressing {}", b.tag));
        locales.push(locale_sizes(b, &c)?);
    }
    // P0.7's gz figures are GNU gzip's; compare like with like when possible.
    let p07_gz = if c.gnu_version.is_some() { Gz::Gnu } else { gz };
    let checks: Vec<Check> = locales.iter().flat_map(checks).collect();
    let p07 = locales
        .iter()
        .filter_map(|l| baseline::p07(&l.tag).map(|p| p07_delta(l, p, p07_gz)))
        .collect();
    Ok(SizeReport {
        tool: format!("catalog-bench {}", env!("CARGO_PKG_VERSION")),
        build: Build::current(),
        unix_time: unix_time(),
        gnu_gzip: c.gnu_version,
        gz,
        manifest_hash: format!("{:016x}", manifest.hash()),
        corpus: corpus::COMMITTED_CORPUS,
        passed: checks.iter().all(|c| c.passed),
        locales,
        checks,
        p07,
    })
}

fn gz_cell(c: &Compressed, gz: Gz) -> String {
    c.gz(gz).map_or_else(|| "n/a".to_owned(), n)
}

fn triple(c: &Compressed, gz: Gz) -> String {
    format!("{} / {} / {}", n(c.raw), gz_cell(c, gz), n(c.br))
}

impl SizeReport {
    /// The JSON form.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self).map(|mut s| {
            s.push('\n');
            s
        })
    }

    /// The gzip implementation the comparison with P0.7 uses.
    fn p07_gz(&self) -> Gz {
        self.p07.first().map_or(self.gz, |d| d.gz)
    }

    /// The Markdown form.
    pub fn to_markdown(&self) -> String {
        let mut o = String::new();
        let gz = self.gz;
        let _ = writeln!(o, "# Catalog size report — B7 (A8)\n");
        let _ = writeln!(
            o,
            "Generated by `{}` ({}). Inputs: the four locales of the reference workload \
             (workload-gen, default knobs, seed 1; `en` is `{}` byte for byte), parsed with \
             `mf2-syntax`; manifest from `en` (ids sorted, slots and markup from `analyze`, \
             functions of every locale), `manifest_hash` **`{}`** (P0.7's reference); one \
             `plural.cardinal` LOCALE entry per catalog (P0.4's bytes, CLDR 48.2.1). \
             Compressors: {}; {} (the gzip figures include the 18 B gzip framing); brotli \
             quality 11, window 22. **B7 is stated on brotli** (quality 11, window 22 — the \
             `.br` file the build writes and serves); the gz figures ({}) are reported, not \
             gated. KB = 1,024 B. Sizes are deterministic: the same inputs and compressor \
             versions give the same bytes.\n",
            self.tool,
            self.build.line(),
            self.corpus,
            self.manifest_hash,
            self.gnu_gzip.as_deref().map_or_else(
                || "GNU gzip not installed".to_owned(),
                |v| format!("`{v}` as `gzip -9 -n`")
            ),
            self.build.crates,
            gz.label(),
        );

        // --- B7 ---
        let _ = writeln!(o, "## B7 — production catalogs (COLD and IDS stripped)\n");
        let _ = writeln!(
            o,
            "| locale | messages | MF2 source B | raw | raw limit (1.25 × src + 8 × msgs) | brotli | brotli limit (0.91 × (0.5 × src + 1,024)) | br / src | gz ({}) | verdict |",
            gz.label()
        );
        let _ = writeln!(o, "|---|---:|---:|---:|---:|---:|---:|---:|---:|---|");
        for l in &self.locales {
            let mine: Vec<&Check> = self.checks.iter().filter(|c| c.locale == l.tag).collect();
            let raw_limit = mine.first().map_or(0, |c| c.limit);
            let br_limit = mine.get(1).map_or(0, |c| c.limit);
            let b7_ok = mine
                .iter()
                .filter(|c| c.rule.starts_with("B7"))
                .all(|c| c.passed);
            let br = l.stripped.whole.br;
            #[allow(clippy::cast_precision_loss)]
            let ratio = br as f64 / l.source_bytes.max(1) as f64;
            let _ = writeln!(
                o,
                "| {} | {} | {} | {} | {} | **{}** | {} | {} | {} | {} |",
                l.tag,
                n(l.messages),
                n(l.source_bytes),
                n(l.stripped.whole.raw),
                n(raw_limit),
                n(br),
                n(br_limit),
                fixed(ratio, 3),
                gz_cell(&l.stripped.whole, gz),
                if b7_ok { "met" } else { "**NOT MET**" },
            );
        }
        let _ = writeln!(o, "\n| locale | check | value | limit | result |");
        let _ = writeln!(o, "|---|---|---:|---:|---|");
        for c in &self.checks {
            let _ = writeln!(
                o,
                "| {} | {} | {} | {} | {} |",
                c.locale,
                c.rule,
                n(c.value),
                n(c.limit),
                if c.passed { "pass" } else { "**FAIL**" }
            );
        }
        let _ = writeln!(
            o,
            "\n**{}** — B7 (plans/06 §3), on brotli 11: `en` ≤ 0.91 × 25 KB (23,296 B); every \
             locale ≤ 0.91 × (0.5 × source + 1 KB), and raw ≤ 1.25 × source + 8 B/message; every \
             catalog decodes back to the parsed model. The factor 0.91 is the worst brotli/gzip \
             ratio measured (en-XA, 0.909 in P0.7 and in Phase 2), rounded up: no locale is \
             held tighter than under the former gzip limits.\n",
            if self.passed { "PASS" } else { "FAIL" }
        );

        // --- stripped vs unstripped, structure vs pool ---
        let _ = writeln!(o, "## Stripped and unstripped; structure vs pool\n");
        let _ = writeln!(
            o,
            "Structure = everything before STRINGS (header, section table, INDEX, MESSAGES, \
             NAMES, LOCALE, FUNCS, IDS); pool = STRINGS. Each part compressed on its own \
             (raw / gz ({}) / brotli).\n",
            gz.label()
        );
        let _ = writeln!(
            o,
            "| locale | catalog | raw | gz | brotli | structure raw / gz / br | pool raw / gz / br |"
        );
        let _ = writeln!(o, "|---|---|---:|---:|---:|---:|---:|");
        for l in &self.locales {
            for (name, v) in [("stripped", &l.stripped), ("unstripped", &l.unstripped)] {
                let _ = writeln!(
                    o,
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    l.tag,
                    name,
                    n(v.whole.raw),
                    gz_cell(&v.whole, gz),
                    n(v.whole.br),
                    triple(&v.structure, gz),
                    triple(&v.pool, gz),
                );
            }
        }

        // --- sections ---
        let _ = writeln!(o, "\n## Sections (raw bytes)\n");
        let names = [
            "INDEX", "MESSAGES", "COLD", "NAMES", "LOCALE", "FUNCS", "IDS", "STRINGS",
        ];
        let _ = writeln!(
            o,
            "| locale | catalog | header + table | {} | total |",
            names.join(" | ")
        );
        let _ = writeln!(o, "|---|---|---:|{}---:|", "---:|".repeat(names.len()));
        for l in &self.locales {
            for (name, v) in [("stripped", &l.stripped), ("unstripped", &l.unstripped)] {
                let cells: Vec<String> = names
                    .iter()
                    .map(|s| v.section(s).map_or_else(|| "—".to_owned(), n))
                    .collect();
                let _ = writeln!(
                    o,
                    "| {} | {} | {} | {} | {} |",
                    l.tag,
                    name,
                    n(v.header),
                    cells.join(" | "),
                    n(v.whole.raw)
                );
            }
        }
        let _ = writeln!(
            o,
            "\nINDEX kinds (every locale): {}. Plural data (LOCALE key 1): {}.",
            self.locales.first().map_or_else(String::new, |l| format!(
                "{} simple, {} pattern, {} select, {} absent",
                l.kinds.simple, l.kinds.pattern, l.kinds.select, l.kinds.absent
            )),
            self.locales
                .iter()
                .map(|l| format!(
                    "{} → `{}` rules, {} B",
                    l.tag, l.plural_locale, l.plural_bytes
                ))
                .collect::<Vec<_>>()
                .join("; ")
        );

        self.p07_markdown(&mut o);
        self.compressor_markdown(&mut o);
        self.names_markdown(&mut o);
        o
    }

    fn p07_markdown(&self, o: &mut String) {
        let pg = self.p07_gz();
        let _ = writeln!(
            o,
            "\n## Against P0.7's recommended layout\n\nP0.7 (plans/phase-0-results.md §P0.7) \
             measured the prototype of this format on the same four locales with GNU gzip and \
             brotli 11; gz deltas below use **{}**. A brotli delta (the B7 metric) beyond {} B \
             or {} % of P0.7's figure is flagged.\n",
            pg.label(),
            FLAG_BYTES,
            fixed(FLAG_FRACTION * 100.0, 0)
        );
        let _ = writeln!(
            o,
            "| locale | raw (Δ) | gz (Δ) | gz Δ % | brotli (Δ) | structure gz (Δ) | pool gz (Δ) | unstripped gz (Δ) | flag |"
        );
        let _ = writeln!(o, "|---|---:|---:|---:|---:|---:|---:|---:|---|");
        for d in &self.p07 {
            let Some(l) = self.locales.iter().find(|l| l.tag == d.locale) else {
                continue;
            };
            let g = |c: &Compressed| c.gz(pg).unwrap_or(0);
            let before = baseline::p07(&d.locale).map_or(1, |p| p.stripped[1]);
            #[allow(clippy::cast_precision_loss)]
            let pct = d.gz_bytes as f64 * 100.0 / before.max(1) as f64;
            let _ = writeln!(
                o,
                "| {} | {} ({}) | {} ({}) | {}{} % | {} ({}) | {} ({}) | {} ({}) | {} ({}) | {} |",
                d.locale,
                n(l.stripped.whole.raw),
                signed(d.raw),
                n(g(&l.stripped.whole)),
                signed(d.gz_bytes),
                if pct > 0.0 { "+" } else { "" },
                fixed(pct, 2),
                n(l.stripped.whole.br),
                signed(d.br),
                n(g(&l.stripped.structure)),
                signed(d.structure_gz),
                n(g(&l.stripped.pool)),
                signed(d.pool_gz),
                n(g(&l.unstripped.whole)),
                signed(d.unstripped_gz),
                match (d.beyond_noise, d.br > 0) {
                    (false, _) => "within noise",
                    (true, true) => "**beyond noise**",
                    (true, false) => "improvement",
                }
            );
        }
        let _ = writeln!(
            o,
            "\nSection by section, stripped, raw bytes — now (Δ against P0.7). P0.7's header \
             was 30 B + 10 B per section; format v1's is 32 B + 10 B per section.\n"
        );
        let names = [
            "header", "INDEX", "MESSAGES", "NAMES", "FUNCS", "LOCALE", "STRINGS",
        ];
        let _ = writeln!(o, "| locale | {} |", names.join(" | "));
        let _ = writeln!(o, "|---|{}", "---:|".repeat(names.len()));
        for l in &self.locales {
            let Some(p) = baseline::p07(&l.tag) else {
                continue;
            };
            let cells: Vec<String> = p
                .sections
                .iter()
                .map(|&(name, before)| {
                    let now = if name == "header" {
                        l.stripped.header
                    } else {
                        l.stripped.section(name).unwrap_or(0)
                    };
                    format!("{} ({})", n(now), signed(delta(now, before)))
                })
                .collect();
            let _ = writeln!(o, "| {} | {} |", l.tag, cells.join(" | "));
        }
        let _ = writeln!(o, "\nWhere the bytes went (stripped):\n");
        for l in &self.locales {
            let (Some(p), Some(d)) = (
                baseline::p07(&l.tag),
                self.p07.iter().find(|d| d.locale == l.tag),
            ) else {
                continue;
            };
            let moved: Vec<String> = p
                .sections
                .iter()
                .filter_map(|&(name, before)| {
                    let now = if name == "header" {
                        l.stripped.header
                    } else {
                        l.stripped.section(name).unwrap_or(0)
                    };
                    (now != before).then(|| format!("{name} {}", signed(delta(now, before))))
                })
                .collect();
            let _ = writeln!(
                o,
                "* {}: raw {} = {}; gz {} (structure alone {}, pool alone {}).",
                l.tag,
                signed(d.raw),
                if moved.is_empty() {
                    "no section changed".to_owned()
                } else {
                    moved.join(", ")
                },
                signed(d.gz_bytes),
                signed(d.structure_gz),
                signed(d.pool_gz),
            );
        }
        let _ = writeln!(
            o,
            "\nThe grammar differences behind these deltas (format v1, plans/02 §2, against \
             P0.7's prototype, phase-0-results §P0.7): the fixed header is 32 B (+2: a 4-byte \
             `cldr_version` in place of the 2-byte `locale_len`); NAMES and FUNCS hold `str32` string \
             references instead of varints (O(1) access); a MESSAGES record starts `varint names \
             · varint (decl_count << 1 | c)` (the COLD bit rides on the small count) and the \
             writer puts the most-referenced NAMES entries first, so most `names` values stay one \
             byte; every variant carries its own `varint nkeys` (a key-count mismatch is \
             representable) while a key is one `varint` (`*` = 0, else StrRef + 1) instead of a \
             kind byte and a StrRef; IDS (unstripped only) has a restart table and a full id \
             every 16 ids for O(log n) `lookup`."
        );
    }

    fn compressor_markdown(&self, o: &mut String) {
        let _ = writeln!(
            o,
            "\n## Compressors (owner decision: what CI measures B7 with)\n\nWhole catalogs; Δ \
             against GNU `gzip -9 -n`. flate2 uses its default backend (miniz_oxide) at level \
             9, its maximum (`Compression::best()`); zlib-rs is the pure-Rust zlib port at \
             level 9 (`DeflateConfig::best_compression()`), gzip wrapper.\n"
        );
        let _ = writeln!(
            o,
            "| locale | catalog | GNU gzip -9 -n | flate2 -9 (Δ) | zlib-rs -9 (Δ) | brotli 11 |"
        );
        let _ = writeln!(o, "|---|---|---:|---:|---:|---:|");
        for l in &self.locales {
            for (name, v) in [("stripped", &l.stripped), ("unstripped", &l.unstripped)] {
                let gnu = v.whole.gz_gnu;
                let d = |x: usize| {
                    gnu.map_or_else(String::new, |g| format!(" ({})", signed(delta(x, g))))
                };
                let _ = writeln!(
                    o,
                    "| {} | {} | {} | {}{} | {}{} | {} |",
                    l.tag,
                    name,
                    gnu.map_or_else(|| "n/a".to_owned(), n),
                    n(v.whole.gz_flate2),
                    d(v.whole.gz_flate2),
                    n(v.whole.gz_zlib_rs),
                    d(v.whole.gz_zlib_rs),
                    n(v.whole.br),
                );
            }
        }
        let _ = writeln!(
            o,
            "\nThe same Δ for each part compressed alone (production catalogs). Where the parts \
             agree and the whole does not, the difference comes from compressing the two \
             kinds of data in one stream — most likely where each implementation ends its \
             deflate blocks around the structure → text boundary (not verified) — not from \
             how it compresses either kind alone.\n"
        );
        let _ = writeln!(
            o,
            "| locale | flate2 Δ whole / structure / pool | zlib-rs Δ whole / structure / pool |"
        );
        let _ = writeln!(o, "|---|---:|---:|");
        for l in &self.locales {
            let v = &l.stripped;
            let d = |part: &Compressed, x: usize| {
                part.gz_gnu
                    .map_or_else(|| "n/a".to_owned(), |g| signed(delta(x, g)))
            };
            let _ = writeln!(
                o,
                "| {} | {} / {} / {} | {} / {} / {} |",
                l.tag,
                d(&v.whole, v.whole.gz_flate2),
                d(&v.structure, v.structure.gz_flate2),
                d(&v.pool, v.pool.gz_flate2),
                d(&v.whole, v.whole.gz_zlib_rs),
                d(&v.structure, v.structure.gz_zlib_rs),
                d(&v.pool, v.pool.gz_zlib_rs),
            );
        }
    }

    fn names_markdown(&self, o: &mut String) {
        let pg = self.p07_gz();
        let _ = writeln!(
            o,
            "\n## NAMES: `str32` (format v1) vs `str16` and varint string references\n\n\
             Format v1 writes a NAMES entry as `varint n_ext · varint n_local · str32*` so a \
             name is one O(1) read. Two alternatives, each measured by re-encoding the stripped \
             catalog's NAMES and re-pointing every MESSAGES head (INDEX follows), everything \
             else byte for byte: fixed 2-byte references (`str16`, still O(1); needs every name \
             in the pool's first 64 KB and, in a real format, a 1-byte width flag not counted \
             here), and P0.7's varint references (a name found by skipping the ones before \
             it). Δ against format v1; gz = {}.\n",
            pg.label()
        );
        let _ = writeln!(
            o,
            "| locale | entries | names | encoding | NAMES bytes | MESSAGES Δ | catalog raw Δ | gz Δ | brotli Δ |"
        );
        let _ = writeln!(o, "|---|---:|---:|---|---:|---:|---:|---:|---:|");
        for l in &self.locales {
            let e = &l.names;
            let g = |c: &Compressed| c.gz(pg).unwrap_or(0);
            for (name, names, messages, whole) in [
                ("str16", e.names_str16, e.messages_str16, &e.whole_str16),
                ("varint", e.names_varint, e.messages_varint, &e.whole_varint),
            ] {
                let _ = writeln!(
                    o,
                    "| {} | {} | {} | {} | {} → {} | {} | {} | {} | {} |",
                    l.tag,
                    n(e.entries),
                    n(e.names),
                    name,
                    n(e.names_str32),
                    n(names),
                    signed(delta(messages, e.messages_str32)),
                    signed(delta(whole.raw, l.stripped.whole.raw)),
                    signed(delta(g(whole), g(&l.stripped.whole))),
                    signed(delta(whole.br, l.stripped.whole.br)),
                );
            }
        }
    }
}
