//! Prints every normative statement of the vendored spec — id, line, key
//! words and text — for writing `conformance/coverage.toml` entries (after a
//! `spec-sync`, the ids `cargo test -p mf2-conformance --test coverage`
//! reports as missing). `--missing` prints only those without an entry.
//!
//! `cargo run -p mf2-conformance --example statements [-- --missing]`

use std::collections::BTreeSet;
use std::path::Path;

use mf2_conformance::coverage::{self, Coverage};
use mf2_conformance::spec::SPEC_DIR;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root");
    let missing_only = std::env::args().any(|a| a == "--missing");
    let statements = coverage::statements(&root.join(SPEC_DIR)).expect("the spec reads");
    let have: BTreeSet<String> = if missing_only {
        Coverage::load(root)
            .map(|c| c.entries.into_iter().map(|e| e.id).collect())
            .unwrap_or_default()
    } else {
        BTreeSet::new()
    };
    for s in statements.iter().filter(|s| !have.contains(&s.id)) {
        println!(
            "{}\tline {}\t[{}]\t§ {}\n    {}\n",
            s.id,
            s.line,
            s.keywords.join(", "),
            s.section,
            s.text
        );
    }
    eprintln!("{} statements", statements.len());
}
