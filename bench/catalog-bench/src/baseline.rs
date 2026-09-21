//! Phase 0's figures that Phase 2 re-measures and must not lose
//! (`plans/09-phase-2-work-order.md`, "State at the start";
//! `plans/phase-0-results.md` §P0.7, §P0.8). The per-locale P0.7 section and
//! structure/pool figures are from P0.7's `out/tables.md`
//! (`probes/p0-07-catalog-encoding`, deleted at the end of Phase 2; the `en`
//! row is also in phase-0-results), all for P0.7's recommended layout
//! (`nul-planes-split-stripped-sorted`), GNU gzip -9 -n, brotli 11/22.

/// P0.7's figures for one locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P07 {
    /// BCP 47 tag.
    pub tag: &'static str,
    /// MF2 source bytes.
    pub source_bytes: usize,
    /// Stripped (production) catalog: raw, gzip -9, brotli 11.
    pub stripped: [usize; 3],
    /// Unstripped (+ IDS): raw, gzip -9, brotli 11.
    pub unstripped: [usize; 3],
    /// Raw section sizes of the stripped catalog, in P0.7's file order
    /// (`header` = fixed header + section table).
    pub sections: [(&'static str, usize); 7],
    /// Everything before STRINGS: raw, gzip -9, brotli 11.
    pub structure: [usize; 3],
    /// STRINGS: raw, gzip -9, brotli 11.
    pub pool: [usize; 3],
}

/// P0.7, per locale.
pub const P07_FIGURES: [P07; 4] = [
    P07 {
        tag: "en",
        source_bytes: 43_250,
        stripped: [50_867, 20_536, 17_929],
        unstripped: [70_790, 29_091, 25_741],
        sections: [
            ("header", 90),
            ("INDEX", 6_400),
            ("MESSAGES", 4_568),
            ("NAMES", 701),
            ("FUNCS", 2),
            ("LOCALE", 8),
            ("STRINGS", 39_098),
        ],
        structure: [11_769, 6_856, 6_306],
        pool: [39_098, 12_804, 11_329],
    },
    P07 {
        tag: "pl",
        source_bytes: 58_518,
        stripped: [67_302, 26_647, 24_068],
        unstripped: [87_225, 36_040, 31_927],
        sections: [
            ("header", 90),
            ("INDEX", 6_400),
            ("MESSAGES", 4_966),
            ("NAMES", 708),
            ("FUNCS", 2),
            ("LOCALE", 35),
            ("STRINGS", 55_101),
        ],
        structure: [12_201, 7_151, 6_641],
        pool: [55_101, 18_567, 17_307],
    },
    P07 {
        tag: "en-XA",
        source_bytes: 95_078,
        stripped: [100_528, 23_620, 21_477],
        unstripped: [120_451, 33_377, 29_284],
        sections: [
            ("header", 90),
            ("INDEX", 6_400),
            ("MESSAGES", 5_178),
            ("NAMES", 708),
            ("FUNCS", 2),
            ("LOCALE", 8),
            ("STRINGS", 88_142),
        ],
        structure: [12_386, 7_268, 6_755],
        pool: [88_142, 16_255, 14_504],
    },
    P07 {
        tag: "ar-XB",
        source_bytes: 54_491,
        stripped: [60_790, 20_518, 18_491],
        unstripped: [80_713, 29_819, 26_314],
        sections: [
            ("header", 90),
            ("INDEX", 6_400),
            ("MESSAGES", 4_578),
            ("NAMES", 708),
            ("FUNCS", 2),
            ("LOCALE", 20),
            ("STRINGS", 48_992),
        ],
        structure: [11_798, 6_877, 6_541],
        pool: [48_992, 13_401, 11_734],
    },
];

/// P0.7's figures for `tag`.
pub fn p07(tag: &str) -> Option<&'static P07> {
    P07_FIGURES.iter().find(|p| p.tag == tag)
}

/// P0.8's native figures for one locale (per-access UTF-8, the chosen
/// strategy; criterion 0.8.2, release + fat LTO, taken under load 2–5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct P08 {
    /// BCP 47 tag.
    pub tag: &'static str,
    /// `Catalog::new` over the stripped catalog, µs.
    pub new_us: f64,
    /// Simple lookup (`get` + `text`, P0.3's `Formatter::simple`) over every
    /// simple id, ns; measured for `en` and `ar-XB` only.
    pub simple_ns: Option<f64>,
}

/// P0.8, per locale (phase-0-results §P0.8).
pub const P08_FIGURES: [P08; 4] = [
    P08 {
        tag: "en",
        new_us: 5.7,
        simple_ns: Some(20.7),
    },
    P08 {
        tag: "pl",
        new_us: 6.7,
        simple_ns: None,
    },
    P08 {
        tag: "en-XA",
        new_us: 6.9,
        simple_ns: None,
    },
    P08 {
        tag: "ar-XB",
        new_us: 6.1,
        simple_ns: Some(28.4),
    },
];

/// P0.8's figures for `tag`.
pub fn p08(tag: &str) -> Option<&'static P08> {
    P08_FIGURES.iter().find(|p| p.tag == tag)
}

#[cfg(test)]
mod tests {
    use super::P07_FIGURES;

    #[test]
    fn p07_rows_add_up() {
        for p in &P07_FIGURES {
            let sum: usize = p.sections.iter().map(|s| s.1).sum();
            assert_eq!(sum, p.stripped[0], "{}", p.tag);
            assert_eq!(p.structure[0] + p.pool[0], p.stripped[0], "{}", p.tag);
            assert_eq!(p.pool[0], p.sections[6].1, "{}", p.tag);
        }
    }
}
