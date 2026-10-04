//! Correctness on the WG suite — the baseline of the "strictly more correct"
//! criterion (rule 2).
//!
//! As in `probes/audit/ox-conformance`: for every suite test, the expected
//! errors are its `expErrors` types restricted to `syntax-error` and the six
//! Data Model Errors; a parser is *exact* on a test when the set it reports
//! equals that set. Expectations come from the vendored suite through
//! `mf2-conformance` (defaults applied); `bench/corpora/suite.json` must match
//! the vendored suite test for test, or the run stops (a stale corpus would
//! make the timings and the correctness figures disagree).
//!
//! Also recorded: how many workload messages the parser reports any error for
//! (the generator emits valid MF2 only, so this should be zero).

use std::path::{Path, PathBuf};

use mf2_conformance::Suite;
use mf2_conformance::matrix::DATA_MODEL_ERRORS;
use serde::Serialize;

use crate::adapter::{Contender, ErrorSet};
use crate::corpus::{Corpus, SUITE_FILE, SuiteEntry};
use crate::error::{Error, Result};

/// One suite test's source and the errors it must produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    /// Suite file, relative to `test/tests`.
    pub file: String,
    /// Index in that file.
    pub index: usize,
    /// The message source.
    pub src: String,
    /// `syntax-error` and Data Model Error names from `expErrors`.
    pub want: ErrorSet,
}

/// The vendored suite's test directory, `<repo>/third_party/message-format-wg/test/tests`.
pub fn default_suite_dir() -> PathBuf {
    crate::repo_root().join(mf2_conformance::SUITE_DIR)
}

/// Loads the vendored suite and checks `suite.json` against it.
pub fn expected(
    suite_dir: &Path,
    entries: &[SuiteEntry],
    corpora_dir: &Path,
) -> Result<Vec<Expected>> {
    let suite = Suite::load(suite_dir)?;
    let stale = |message: String| Error::Corpus {
        path: corpora_dir.join(SUITE_FILE),
        message: format!("{message}; regenerate it with `cargo xtask gen-workload corpora`"),
    };
    if suite.tests().len() != entries.len() {
        return Err(stale(format!(
            "{} entries, but the vendored suite has {} tests",
            entries.len(),
            suite.tests().len()
        )));
    }
    let mut out = Vec::with_capacity(entries.len());
    for (test, entry) in suite.tests().iter().zip(entries) {
        if test.key.file != entry.file || test.index != entry.index || test.src != entry.src {
            return Err(stale(format!(
                "entry {}#{} differs from the vendored suite",
                entry.file, entry.index
            )));
        }
        let want = test
            .exp_errors
            .iter()
            .filter(|e| *e == "syntax-error" || DATA_MODEL_ERRORS.contains(&e.as_str()))
            .cloned()
            .collect();
        out.push(Expected {
            file: entry.file.clone(),
            index: entry.index,
            src: entry.src.clone(),
            want,
        });
    }
    Ok(out)
}

/// A test on which a parser is not exact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mismatch {
    /// Suite file.
    pub file: String,
    /// Index in that file.
    pub index: usize,
    /// The message source.
    pub src: String,
    /// Expected error names.
    pub want: Vec<String>,
    /// Reported error names.
    pub got: Vec<String>,
}

/// One parser's correctness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Correctness {
    /// Contender key.
    pub parser: &'static str,
    /// Suite tests.
    pub suite_total: usize,
    /// Tests on which the reported set equals the expected set.
    pub suite_exact: usize,
    /// The others.
    pub mismatches: Vec<Mismatch>,
    /// Workload messages.
    pub workload_total: usize,
    /// Workload messages for which the parser reports any error.
    pub workload_with_errors: usize,
}

impl Correctness {
    /// Exact on every suite test.
    pub fn is_complete(&self) -> bool {
        self.suite_exact == self.suite_total && self.mismatches.is_empty()
    }
}

/// Runs `contender` over the suite expectations and the workload.
pub fn check(contender: &dyn Contender, expected: &[Expected], workload: &Corpus) -> Correctness {
    let mut mismatches = Vec::new();
    for e in expected {
        let got = contender.classify(&e.src);
        if got != e.want {
            mismatches.push(Mismatch {
                file: e.file.clone(),
                index: e.index,
                src: e.src.clone(),
                want: e.want.iter().cloned().collect(),
                got: got.into_iter().collect(),
            });
        }
    }
    let workload_with_errors = workload
        .messages
        .iter()
        .filter(|m| !contender.classify(m).is_empty())
        .count();
    Correctness {
        parser: contender.key(),
        suite_total: expected.len(),
        suite_exact: expected.len() - mismatches.len(),
        mismatches,
        workload_total: workload.messages.len(),
        workload_with_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::{check, default_suite_dir, expected};
    use crate::corpus::{Corpora, default_dir};
    use crate::ox::Ox;

    /// The audit's figure, reproduced in-tree: 460/462, the two misses being
    /// over-reporting (a superset of the expected errors). Runs the whole
    /// suite once (a few ms in a debug build).
    #[test]
    fn ox_baseline_is_460_of_462_and_the_workload_is_clean() {
        let corpora = Corpora::load(&default_dir()).expect("corpora");
        let exp = expected(&default_suite_dir(), &corpora.suite_entries, &default_dir())
            .expect("suite.json matches the vendored suite");
        assert_eq!(exp.len(), 462);
        let c = check(&Ox, &exp, &corpora.workload);
        assert_eq!(
            (c.suite_exact, c.suite_total),
            (460, 462),
            "{:#?}",
            c.mismatches
        );
        for m in &c.mismatches {
            assert!(m.want.iter().all(|w| m.got.contains(w)), "{m:?}");
        }
        assert_eq!(c.workload_with_errors, 0);
        assert!(!c.is_complete());
    }

    #[test]
    fn a_stale_suite_corpus_is_refused() {
        let corpora = Corpora::load(&default_dir()).expect("corpora");
        let mut entries = corpora.suite_entries.clone();
        entries[0].src.push('x');
        assert!(expected(&default_suite_dir(), &entries, &default_dir()).is_err());
        entries.pop();
        assert!(expected(&default_suite_dir(), &entries, &default_dir()).is_err());
    }
}
