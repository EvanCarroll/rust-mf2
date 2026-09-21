//! `cargo xtask conformance-report [--init]`: the ledger generator and checker
//! (logic in `mf2-conformance`; this is the command-line shell).

use std::fs;
use std::path::{Path, PathBuf};

use mf2_conformance::{Harness, LEDGER_PATH, Ledger, REPORT_PATH, SUITE_DIR, Suite, report};

use crate::error::{Error, Result};
use crate::pin::Pin;

fn resolve(root: &Path, given: Option<&Path>, default: &str) -> PathBuf {
    given.map_or_else(|| root.join(default), Path::to_path_buf)
}

/// `--init`: write a fresh ledger for the vendored suite.
pub(crate) fn init(root: &Path, ledger: Option<&Path>, force: bool) -> Result<()> {
    let path = resolve(root, ledger, LEDGER_PATH);
    if path.exists() && !force {
        return Err(Error::LedgerExists(path));
    }
    let suite = Suite::load(&root.join(SUITE_DIR))?;
    let ledger = Ledger::init(&suite);
    fs::write(&path, ledger.to_toml()).map_err(|source| Error::IoAt {
        path: path.clone(),
        source,
    })?;
    eprintln!(
        "conformance-report: wrote {} ({} entries, current_phase = {})",
        path.display(),
        ledger.entries.len(),
        ledger.current_phase
    );
    Ok(())
}

/// `--promote`: run the layer harnesses and turn every `xfail` cell they pass
/// into `pass` (the ratchet's way up), rewriting the ledger.
pub(crate) fn promote(root: &Path, ledger: Option<&Path>) -> Result<()> {
    let ledger_path = resolve(root, ledger, LEDGER_PATH);
    let suite = Suite::load(&root.join(SUITE_DIR))?;
    let text = fs::read_to_string(&ledger_path).map_err(|source| Error::IoAt {
        path: ledger_path.clone(),
        source,
    })?;
    let mut ledger = Ledger::parse(&text)?;
    let results = Harness::load(root)?.run_all(&suite);
    let changed = mf2_conformance::promote(&mut ledger, &results);
    fs::write(&ledger_path, ledger.to_toml()).map_err(|source| Error::IoAt {
        path: ledger_path.clone(),
        source,
    })?;
    eprintln!(
        "conformance-report: promoted {changed} cell(s) from xfail to pass in {}",
        ledger_path.display()
    );
    Ok(())
}

/// Check the ledger against the vendored suite and the layer harnesses, and
/// write the report. The report is written even when the harness is red (it
/// lists the violations).
pub(crate) fn check(root: &Path, ledger: Option<&Path>, report_path: Option<&Path>) -> Result<()> {
    let ledger_path = resolve(root, ledger, LEDGER_PATH);
    let report_path = resolve(root, report_path, REPORT_PATH);
    let suite = Suite::load(&root.join(SUITE_DIR))?;
    let text = fs::read_to_string(&ledger_path).map_err(|source| Error::IoAt {
        path: ledger_path.clone(),
        source,
    })?;
    let ledger = Ledger::parse(&text)?;
    let mut violations = mf2_conformance::check(&suite, &ledger);
    let results = Harness::load(root)?.run_all(&suite);
    violations.extend(mf2_conformance::verify(&ledger, &results));

    let upstream = Pin::load(&root.join("third_party/message-format-wg/PIN"))
        .ok()
        .and_then(|pin| {
            let commit = pin.get("commit").ok()?.to_owned();
            let date = pin.get("date").map(str::to_owned).unwrap_or_default();
            Some(format!(
                "`unicode-org/message-format-wg` @ `{commit}` ({date})"
            ))
        });
    let rendered = report::render(
        &suite,
        &ledger,
        &violations,
        upstream.as_deref(),
        Some(&results),
    );
    fs::write(&report_path, rendered).map_err(|source| Error::IoAt {
        path: report_path.clone(),
        source,
    })?;

    for v in violations.iter().take(50) {
        eprintln!("  violation: {v}");
    }
    if violations.len() > 50 {
        eprintln!(
            "  … and {} more (see {})",
            violations.len() - 50,
            report_path.display()
        );
    }
    eprintln!(
        "conformance-report: {} tests, {} ledger entries, current_phase = {}: {}; wrote {}",
        suite.tests().len(),
        ledger.entries.len(),
        ledger.current_phase,
        if violations.is_empty() {
            "green"
        } else {
            "RED"
        },
        report_path.display()
    );
    if violations.is_empty() {
        Ok(())
    } else {
        Err(Error::LedgerViolations(violations.len()))
    }
}
