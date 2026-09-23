//! Phases, ledger columns, and the two tables of plans/01-conformance.md that
//! the ledger is generated and checked from: the layer → phase table (§3) and
//! the `n/a` matrix for error tests (§4).

use std::fmt;

use crate::key::TestKey;

/// A project phase (plans/00-master-plan.md §9), in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    P0,
    P1,
    P2,
    P3,
    P4,
    P5a,
    P5b,
    P6,
    P7,
    P8,
    P9,
}

impl Phase {
    /// Every phase, in order: P0 < P1 < P2 < P3 < P4 < P5a < P5b < P6 < P7 < P8 < P9.
    pub const ALL: [Self; 11] = [
        Self::P0,
        Self::P1,
        Self::P2,
        Self::P3,
        Self::P4,
        Self::P5a,
        Self::P5b,
        Self::P6,
        Self::P7,
        Self::P8,
        Self::P9,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::P0 => "P0",
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
            Self::P4 => "P4",
            Self::P5a => "P5a",
            Self::P5b => "P5b",
            Self::P6 => "P6",
            Self::P7 => "P7",
            Self::P8 => "P8",
            Self::P9 => "P9",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == s)
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A status column of the ledger: the layers with all features on, and the
/// default-features configuration of L4–L7. L7 has two delivery modes, each
/// with columns of its own (`plans/01-conformance.md` §3; owner,
/// 2026-09-23): `L7`/`L7d` an islands page, `L7c`/`L7cd` a client-only one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Column {
    L1,
    L2,
    L3,
    L4,
    L5,
    L6,
    L7,
    L7c,
    L4d,
    L5d,
    L6d,
    L7d,
    L7cd,
}

impl Column {
    /// Every column, in ledger order.
    pub const ALL: [Self; 13] = [
        Self::L1,
        Self::L2,
        Self::L3,
        Self::L4,
        Self::L5,
        Self::L6,
        Self::L7,
        Self::L7c,
        Self::L4d,
        Self::L5d,
        Self::L6d,
        Self::L7d,
        Self::L7cd,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
            Self::L4 => "L4",
            Self::L5 => "L5",
            Self::L6 => "L6",
            Self::L4d => "L4d",
            Self::L5d => "L5d",
            Self::L6d => "L6d",
            Self::L7 => "L7",
            Self::L7c => "L7c",
            Self::L7d => "L7d",
            Self::L7cd => "L7cd",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }

    /// The default-features columns, the only ones where `degraded` is allowed.
    pub fn is_default_features(self) -> bool {
        matches!(
            self,
            Self::L4d | Self::L5d | Self::L6d | Self::L7d | Self::L7cd
        )
    }

    /// The delivery-mode columns, which only a browser can run: `cargo xtask
    /// l7-web` judges them and holds the ledger to the result, as `cargo
    /// xtask l4-web` does for the `intl` entries ([`BROWSER_HARNESSED`]).
    pub fn is_delivery_mode(self) -> bool {
        matches!(self, Self::L7 | Self::L7c | Self::L7d | Self::L7cd)
    }

    /// The macro layer, the only one where `via = "dyn"` is allowed.
    pub fn is_macro_layer(self) -> bool {
        matches!(self, Self::L5 | Self::L5d)
    }

    /// The phase at whose exit this column must be green for the test `key`
    /// (plans/01-conformance.md §3). An `xfail` MUST NOT name a later phase.
    pub fn deadline(self, key: &TestKey) -> Phase {
        let file = key.file.as_str();
        match self {
            Self::L1 | Self::L2 => Phase::P1,
            Self::L3 => Phase::P2,
            Self::L4 if L4_FUNCTION_FILES_AT_P4.contains(&file) => Phase::P4,
            Self::L4
                if L4_TESTS_AT_P4
                    .iter()
                    .any(|&(f, h, n)| f == file && h == key.hash && n == key.nth) =>
            {
                Phase::P4
            }
            Self::L4 => Phase::P3,
            Self::L4d => Phase::P4,
            Self::L5 | Self::L5d => Phase::P5b,
            Self::L6 | Self::L6d => Phase::P6,
            Self::L7 | Self::L7c | Self::L7d | Self::L7cd => Phase::P7,
        }
    }
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Suite files whose L4 (all features) turns green at P4 rather than P3 —
/// and `extra/functions/unit.json`, our `:unit` tests (the suite has none),
/// written in Phase 4 (plans/11 A4).
pub const L4_FUNCTION_FILES_AT_P4: &[&str] = &[
    "functions/percent.json",
    "functions/currency.json",
    "functions/date.json",
    "functions/time.json",
    "functions/datetime.json",
    "extra/functions/unit.json",
];

/// Single tests (`file`, `hash`, `nth`) of the other files whose L4 turns
/// green at P4 too, because they need Phase 4's locale data
/// (plans/01-conformance.md §3; owner, 2026-09-21): `syntax.json` #90,
/// `{$one} et {$two}` in `fr`, formats unannotated floats with the French
/// decimal comma — the `number.symbols` entry and `fn-number`.
pub const L4_TESTS_AT_P4: &[(&str, &str, u32)] = &[("syntax.json", "8a3aac3e", 0)];

/// The error names of the six Data Model Errors (spec `errors.md`).
pub const DATA_MODEL_ERRORS: &[&str] = &[
    "variant-key-mismatch",
    "missing-fallback-variant",
    "missing-selector-annotation",
    "duplicate-declaration",
    "duplicate-option-name",
    "duplicate-variant",
];

/// The row of the `n/a` matrix a test falls in (plans/01-conformance.md §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TestKind {
    /// `expErrors` contains `syntax-error`.
    SyntaxError,
    /// `expErrors` contains a Data Model Error (and no `syntax-error`).
    DataModelError,
    /// Everything else.
    Other,
}

impl TestKind {
    /// Classifies a test by the `type`s of its (resolved) `expErrors`.
    pub fn classify<'a>(exp_errors: impl IntoIterator<Item = &'a str> + Clone) -> Self {
        if exp_errors.clone().into_iter().any(|e| e == "syntax-error") {
            Self::SyntaxError
        } else if exp_errors
            .into_iter()
            .any(|e| DATA_MODEL_ERRORS.contains(&e))
        {
            Self::DataModelError
        } else {
            Self::Other
        }
    }

    /// Whether `column` applies to a test of this kind; `false` means the cell
    /// MUST be `n/a`. The default-features columns follow their layer; L7
    /// follows L6, because a page holds only what the build accepted.
    pub fn applies(self, column: Column) -> bool {
        let rendered = matches!(column, Column::L6 | Column::L6d) || column.is_delivery_mode();
        match self {
            Self::Other => true,
            Self::SyntaxError => !(rendered || matches!(column, Column::L2 | Column::L3)),
            Self::DataModelError => !(rendered || column == Column::L3),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SyntaxError => "syntax-error",
            Self::DataModelError => "data-model-error",
            Self::Other => "other",
        }
    }
}

/// Columns whose harness exists. Nothing in a column without a harness can be
/// verified, so `pass` and `degraded` are refused there (a pass that nothing
/// checks is a silent skip). L1/L2 joined in Phase 1, L3 in Phase 2, L4 in
/// Phase 3, L4d (the default configuration) in Phase 4, and so on.
pub const HARNESSED: &[Column] = &[
    Column::L1,
    Column::L2,
    Column::L3,
    Column::L4,
    Column::L4d,
    Column::L5,
    Column::L5d,
    Column::L6,
    Column::L6d,
];

/// Columns whose harness runs in browser engines rather than in `cargo
/// test`: `cargo xtask l7-web` builds the pages, drives them, judges every
/// cell and holds the ledger to it. `pass` and `degraded` are allowed here
/// for that reason; `cargo xtask conformance-report` checks their shape
/// only.
pub const BROWSER_HARNESSED: &[Column] = &[Column::L7, Column::L7c, Column::L7d, Column::L7cd];

#[cfg(test)]
mod tests {
    use super::{Column, Phase, TestKind};
    use crate::key::TestKey;

    #[test]
    fn phase_order() {
        let mut sorted = Phase::ALL;
        sorted.sort();
        assert_eq!(sorted, Phase::ALL);
        assert!(Phase::P4 < Phase::P5a && Phase::P5a < Phase::P5b && Phase::P5b < Phase::P6);
        assert_eq!(Phase::parse("P5b"), Some(Phase::P5b));
        assert_eq!(Phase::parse("P5"), None);
    }

    #[test]
    fn na_matrix() {
        use Column::{L1, L2, L3, L4, L4d, L5, L5d, L6, L6d, L7, L7c, L7cd, L7d};
        let syn: Vec<_> = Column::ALL
            .into_iter()
            .filter(|c| !TestKind::SyntaxError.applies(*c))
            .collect();
        assert_eq!(syn, [L2, L3, L6, L7, L7c, L6d, L7d, L7cd]);
        let dm: Vec<_> = Column::ALL
            .into_iter()
            .filter(|c| !TestKind::DataModelError.applies(*c))
            .collect();
        assert_eq!(dm, [L3, L6, L7, L7c, L6d, L7d, L7cd]);
        assert!(
            [L1, L2, L3, L4, L5, L6, L7, L7c, L4d, L5d, L6d, L7d, L7cd]
                .into_iter()
                .all(|c| TestKind::Other.applies(c))
        );
    }

    #[test]
    fn deadlines() {
        let key = |file: &str, hash: &str| TestKey {
            file: file.to_owned(),
            hash: hash.to_owned(),
            nth: 0,
        };
        let k = |file: &str| key(file, "00000000");
        assert_eq!(Column::L4.deadline(&k("functions/number.json")), Phase::P3);
        assert_eq!(Column::L4.deadline(&k("bidi.json")), Phase::P3);
        assert_eq!(
            Column::L4.deadline(&k("functions/currency.json")),
            Phase::P4
        );
        assert_eq!(Column::L4.deadline(&k("syntax.json")), Phase::P3);
        assert_eq!(
            Column::L4.deadline(&key("syntax.json", "8a3aac3e")),
            Phase::P4
        );
        assert_eq!(Column::L4d.deadline(&k("syntax.json")), Phase::P4);
        assert_eq!(Column::L5d.deadline(&k("syntax.json")), Phase::P5b);
        assert_eq!(Column::L6d.deadline(&k("syntax.json")), Phase::P6);
        assert_eq!(Column::L7cd.deadline(&k("syntax.json")), Phase::P7);
    }
}
