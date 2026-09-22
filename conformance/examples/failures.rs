//! Prints every failing cell of one layer, with what failed:
//! `cargo run -p mf2-conformance --example failures -- L4`.

use std::path::Path;
use std::process::ExitCode;

use mf2_conformance::{Column, Harness, SUITE_DIR, Suite};

fn main() -> ExitCode {
    let column = std::env::args().nth(1).unwrap_or_else(|| "L4".to_owned());
    let Some(column) = Column::parse(&column) else {
        eprintln!("unknown column {column}");
        return ExitCode::FAILURE;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let suite = match Suite::load(&root.join(SUITE_DIR)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let harness = match Harness::load(&root) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let (mut run, mut failed) = (0, 0);
    for test in suite.tests() {
        if !test.kind.applies(column) {
            continue;
        }
        let Some(outcome) = harness.run(column, test) else {
            continue;
        };
        run += 1;
        if let Err(e) = outcome {
            failed += 1;
            println!("{}#{} {:?}\n    {e}", test.key.file, test.index, test.src);
        }
    }
    println!("{column}: {} of {run} pass", run - failed);
    ExitCode::SUCCESS
}
