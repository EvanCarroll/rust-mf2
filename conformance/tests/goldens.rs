//! The committed locale-output goldens equal a fresh render. After a deliberate
//! change of output, `cargo xtask goldens` rewrites them — review the diff.

use std::fs;
use std::path::PathBuf;

use mf2_conformance::goldens::{FAMILIES, path, render};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance/ has a parent")
        .to_path_buf()
}

#[test]
fn goldens_are_current() {
    for family in FAMILIES {
        let file = root().join(path(family));
        let committed = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{}: {e} (run `cargo xtask goldens`)", file.display()));
        let fresh = render(family).unwrap_or_else(|e| panic!("{}: {e}", family.name));
        if committed != fresh {
            let first = committed
                .lines()
                .zip(fresh.lines())
                .enumerate()
                .find(|(_, (a, b))| a != b)
                .map_or_else(
                    || "the files differ in length".to_owned(),
                    |(i, (a, b))| format!("line {}:\n  committed {a}\n  now       {b}", i + 1),
                );
            panic!(
                "{} is out of date ({first}); `cargo xtask goldens` rewrites it",
                file.display()
            );
        }
    }
}
