//! Phase 10 C3: the matcher's answers for `oracle.sh` — copied into
//! `crates/mf2/tests/` for one run and removed after it. Reads the cases
//! from `$MF2_ORACLE/in.txt`, writes the answers to `$MF2_ORACLE/rust.txt`.
use mf2::{Dir, LanguageMatching};
use std::io::{BufRead, Write};
#[test]
fn oracle() {
    let m = LanguageMatching::cldr();
    let dir = std::path::PathBuf::from(std::env::var("MF2_ORACLE").expect("MF2_ORACLE"));
    let input = std::fs::File::open(dir.join("in.txt")).expect("in");
    let mut out = std::fs::File::create(dir.join("rust.txt")).expect("out");
    for line in std::io::BufReader::new(input).lines() {
        let line = line.expect("line");
        let (kind, rest) = line.split_once(' ').expect("kind");
        if kind == "d" {
            let (a, b) = rest.split_once(' ').expect("pair");
            writeln!(out, "d {a} {b} {}", m.distance_of(a, b).map_or(-1, i64::from)).expect("w");
        } else {
            let (ds, ss) = rest.split_once(" | ").expect("lists");
            let desired: Vec<&str> = ds.split(' ').collect();
            let supported: Vec<(&str, Dir)> = ss.split(' ').map(|t| (t, Dir::Ltr)).collect();
            let got = m.best_match(desired.iter().copied(), &supported).map_or("-", |i| supported[i].0);
            writeln!(out, "b {rest} => {got}").expect("w");
        }
    }
}
