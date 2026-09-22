//! Budget B7 on the catalogs `mf2-build` writes (Phase 5a, A10).
//!
//! Phase 2 measured B7 on catalogs written straight from the committed
//! corpora, with no locale data in them. These are the ones an application
//! actually serves: the manifest built from the source locale, fallbacks
//! flattened, COLD and IDS stripped, and the LOCALE entries this corpus
//! needs.
//!
//! B7 (`plans/06-size-and-perf.md` §3) is stated on brotli 11 — the `.br`
//! file the build writes and `mf2-axum` serves:
//!
//! * the reference `en`: ≤ 0.91 × 25 KB = 23,296 B;
//! * every locale: br ≤ 0.91 × (0.5 × MF2 source bytes + 1 KB), and
//!   raw ≤ 1.25 × source bytes + 8 B per message.

mod common;

use common::{out_dir, workload};
use mf2_build::loader::{Loader, resource};
use mf2_build::{Build, Features};

/// The worst brotli/gzip ratio measured on the four locales, which is what
/// B7's brotli limits are the gzip ones scaled by.
const BR_SCALE: (usize, usize) = (91, 100);

/// B7's absolute bound for the reference `en`.
const EN_BR_MAX: usize = 25 * 1024 * BR_SCALE.0 / BR_SCALE.1;

/// B7's bound for any locale: 0.91 × (0.5 × source + 1 KB).
fn br_limit(source_bytes: usize) -> usize {
    BR_SCALE.0 * (source_bytes + 2048) / (2 * BR_SCALE.1)
}

/// B7's raw bound: 1.25 × source + 8 B per message.
fn raw_limit(source_bytes: usize, messages: usize) -> usize {
    source_bytes * 5 / 4 + messages * 8
}

/// What Phase 2 measured for the same locales, with no locale data in the
/// catalogs (`plans/phase-2-results.md` §A8, re-stated in
/// `plans/06-size-and-perf.md` §3).
const PHASE_2_BR: [(&str, usize); 4] = [
    ("en", 18_072),
    ("pl", 24_137),
    ("en-XA", 21_537),
    ("ar-XB", 18_423),
];

#[test]
fn b7_every_locale_of_the_reference_workload() {
    let root = workload();
    let outcome = Build::at(&root, out_dir("b7"))
        .features(Features::parse("fn-number"))
        .run()
        .expect("the workload builds");
    assert!(outcome.report.is_clean(), "{}", outcome.report.to_text());

    eprintln!(
        "\n{:<8} {:>9} {:>9} {:>9} {:>9} {:>9} {:>8}  locale data",
        "locale", "source", "raw", "raw max", "br", "br max", "P2 br"
    );
    for catalog in &outcome.catalogs {
        let loaded = resource::Resources
            .load(&root.join("locales").join(&catalog.tag))
            .expect("the locale's files");
        // "MF2 source bytes": the UTF-8 length of every message source,
        // summed — the same definition `bench/catalog-bench` uses.
        let source_bytes: usize = loaded.records.iter().map(|r| r.source.len()).sum();
        let messages = loaded.records.len();
        let br_max = br_limit(source_bytes);
        let raw_max = raw_limit(source_bytes, messages);
        let entries: usize = catalog.locale_entries.iter().map(|(_, n)| n).sum();
        let phase_2 = PHASE_2_BR
            .iter()
            .find(|(tag, _)| *tag == catalog.tag)
            .map_or(0, |(_, br)| *br);
        eprintln!(
            "{:<8} {source_bytes:>9} {:>9} {raw_max:>9} {:>9} {br_max:>9} {phase_2:>8}  {entries} B",
            catalog.tag,
            catalog.bytes.len(),
            catalog.br.len(),
        );

        assert!(
            catalog.br.len() <= br_max,
            "{}: {} B br over the {br_max} B limit (0.91 × (0.5 × {source_bytes} + 1 KB))",
            catalog.tag,
            catalog.br.len()
        );
        assert!(
            catalog.bytes.len() <= raw_max,
            "{}: {} B raw over the {raw_max} B limit",
            catalog.tag,
            catalog.bytes.len()
        );
        if catalog.tag == "en" {
            assert!(
                catalog.br.len() <= EN_BR_MAX,
                "the reference locale is {} B br, over {EN_BR_MAX}",
                catalog.br.len()
            );
        }
    }
}

/// The locale data a catalog now carries is what it costs on the wire —
/// which is the only difference from Phase 2's figures.
#[test]
fn b7_the_locale_data_costs_what_it_weighs() {
    let root = workload();
    let with = Build::at(&root, out_dir("b7-with"))
        .features(Features::parse("fn-number"))
        .run()
        .expect("builds");
    // Without `fn-number` the number entries are not carried at all, so the
    // difference is what the number data costs on the wire.
    let without = Build::at(&root, out_dir("b7-without"))
        .features(Features::default())
        .run()
        .expect("builds");

    for (a, b) in with.catalogs.iter().zip(&without.catalogs) {
        assert_eq!(a.tag, b.tag);
        let delta = i64::try_from(a.br.len()).unwrap_or(i64::MAX)
            - i64::try_from(b.br.len()).unwrap_or(i64::MAX);
        let entries: usize = a.locale_entries.iter().map(|(_, n)| n).sum();
        let other: usize = b.locale_entries.iter().map(|(_, n)| n).sum();
        eprintln!(
            "{:<8} {:>6} B br with fn-number, {:>6} without (Δ {delta:+}); \
             locale data {entries} B vs {other} B",
            a.tag,
            a.br.len(),
            b.br.len()
        );
        // Whatever it costs, it is tens of bytes, not kilobytes.
        assert!(
            delta.abs() < 200,
            "{}: the number data moved the catalog by {delta} B br",
            a.tag
        );
    }
}
