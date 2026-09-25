//! `cargo xtask conformance-report [--init]`: the ledger generator and checker
//! (logic in `mf2-conformance`; this is the command-line shell).

use std::fs;
use std::path::{Path, PathBuf};

use mf2_conformance::spec::spec_dir;
use mf2_conformance::{Harness, LEDGER_PATH, Ledger, REPORT_PATH, coverage, report};

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
    let suite = mf2_conformance::load_suite(root)?;
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

/// `--promote`: add entries for tests the ledger lacks (and cells for
/// columns it lacks), run the layer
/// harnesses and turn every `xfail` cell they pass into `pass` (and every
/// documented degradation into `degraded`) — the ratchet's way up —
/// rewriting the ledger.
pub(crate) fn promote(root: &Path, ledger: Option<&Path>) -> Result<()> {
    let ledger_path = resolve(root, ledger, LEDGER_PATH);
    let suite = mf2_conformance::load_suite(root)?;
    let text = fs::read_to_string(&ledger_path).map_err(|source| Error::IoAt {
        path: ledger_path.clone(),
        source,
    })?;
    let mut ledger = Ledger::parse(&text)?;
    // Tests the ledger lacks (a new file under conformance/extra/) get
    // entries first, then everything the harnesses pass is promoted.
    let added = ledger.add_missing(&suite);
    if added > 0 {
        eprintln!("conformance-report: added {added} ledger entries for new tests");
    }
    // …and columns the ledger lacks (a new layer) get their fresh cells.
    let cells = ledger.add_missing_columns(&suite);
    if cells > 0 {
        eprintln!("conformance-report: added {cells} cell(s) for new columns");
    }
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
    let suite = mf2_conformance::load_suite(root)?;
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
    let gaps = write_coverage(root, &suite, &report_path)?;
    if !violations.is_empty() {
        Err(Error::LedgerViolations(violations.len()))
    } else if gaps > 0 {
        Err(Error::CoverageGaps(gaps))
    } else {
        Ok(())
    }
}

/// The spec coverage matrix (plans/01-conformance.md §5): check
/// `conformance/coverage.toml` against the spec and the tree, and write
/// `COVERAGE.md` beside the report. Returns the number of gaps.
fn write_coverage(
    root: &Path,
    suite: &mf2_conformance::Suite,
    report_path: &Path,
) -> Result<usize> {
    let statements = coverage::statements(&spec_dir(root)?)?;
    let matrix = coverage::Coverage::load(root)?;
    let gaps = coverage::check(&statements, &matrix, suite, root);
    let path = report_path.with_file_name("COVERAGE.md");
    let rendered = coverage::render(&statements, &matrix, suite, coverage::pin(root).as_deref());
    fs::write(&path, rendered).map_err(|source| Error::IoAt {
        path: path.clone(),
        source,
    })?;
    for g in gaps.iter().take(50) {
        eprintln!("  coverage: {g}");
    }
    eprintln!(
        "conformance-report: {} normative statements, {} gap(s); wrote {}",
        statements.len(),
        gaps.len(),
        path.display()
    );
    Ok(gaps.len())
}
