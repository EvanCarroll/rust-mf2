//! Differential run: `mf2-syntax` vs `ox_mf2_parser` on messages generated
//! from the vendored ABNF and on random mutations of them (mostly malformed).
//! Compares syntax-error presence, and the Data Model errors where both
//! parsers accept the message.
//!
//! ox hangs on some malformed inputs (`{:` + a noncharacter never returns),
//! so it runs in a **child process** that is killed after a timeout and
//! restarted; run the whole thing under a memory limit:
//!
//! ```sh
//! cargo build --release -p mf2-conformance --example differential
//! (ulimit -v 4000000; nice ./target/release/examples/differential 20000)
//! ```

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use mf2_conformance::abnf::{Generator, Grammar, Rng};
use ox_mf2_parser::{
    ParseOptions, SourceFileInput, SourceStore, build_semantic_model, parse_source,
    validate_semantics,
};

type Set = BTreeSet<String>;

fn ox_report(src: &str) -> Set {
    let mut sources = SourceStore::new();
    let id = sources.add(SourceFileInput {
        source: src,
        ..SourceFileInput::default()
    });
    let mut got = Set::new();
    match parse_source(&sources, id, ParseOptions::default()) {
        Err(e) => {
            got.insert(format!("internal:{e}"));
        }
        Ok(p) if !p.diagnostics.is_empty() => {
            got.insert("syntax-error".into());
        }
        Ok(p) => match build_semantic_model(&sources, &p).and_then(|m| validate_semantics(&m)) {
            Ok(errs) => {
                for e in errs {
                    let name = match e.code().json_code() {
                        "variant-key-arity-mismatch" => "variant-key-mismatch",
                        "invalid-declaration-dependency" => "duplicate-declaration",
                        o => o,
                    };
                    got.insert(name.to_owned());
                }
            }
            Err(e) => {
                got.insert(format!("internal:{e}"));
            }
        },
    }
    got
}

/// mf2-syntax's report in the same shape, and its diagnostic codes.
fn ours_report(src: &str) -> (Set, Vec<u16>) {
    let p = mf2_syntax::parse_model(src);
    let codes = p.diagnostics.iter().map(|d| d.code).collect();
    if p.diagnostics.has_class(mf2_model::ErrorClass::Syntax) {
        return (["syntax-error".to_owned()].into(), codes);
    }
    let set = p
        .diagnostics
        .iter()
        .map(|d| d.kind.suite_name().to_owned())
        .collect();
    (set, codes)
}

/// The child: one JSON string per line in, one JSON array per line out.
fn child() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let src: String = serde_json::from_str(&line).expect("JSON string");
        let report: Vec<String> = ox_report(&src).into_iter().collect();
        writeln!(out, "{}", serde_json::to_string(&report).expect("JSON")).expect("stdout");
        out.flush().expect("flush");
    }
}

struct Ox {
    child: Child,
    stdin: ChildStdin,
    lines: mpsc::Receiver<String>,
}

impl Ox {
    fn spawn() -> Self {
        let mut child = Command::new(std::env::current_exe().expect("exe"))
            .arg("--child")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn child");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ox {
            child,
            stdin,
            lines,
        }
    }

    /// ox's report, or `None` if it did not answer within the timeout (the
    /// child is then killed and replaced).
    fn report(&mut self, src: &str) -> Option<Set> {
        writeln!(self.stdin, "{}", serde_json::to_string(src).expect("JSON")).ok()?;
        self.stdin.flush().ok()?;
        if let Ok(line) = self.lines.recv_timeout(Duration::from_secs(2)) {
            Some(
                serde_json::from_str::<Vec<String>>(&line)
                    .ok()?
                    .into_iter()
                    .collect(),
            )
        } else {
            let _ = self.child.kill();
            let _ = self.child.wait();
            *self = Ox::spawn();
            None
        }
    }
}

impl Drop for Ox {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn mutate(rng: &mut Rng, s: &str) -> String {
    const SPECIAL: &[char] = &[
        '{', '}', '|', '\\', '$', ':', '#', '/', '@', '=', '.', '*', ' ', '\u{200e}', 'a', '1', '-',
    ];
    let mut chars: Vec<char> = s.chars().collect();
    let mut pick = |n: usize| usize::try_from(rng.below(n as u64)).unwrap_or(0);
    for _ in 0..=pick(3) {
        let i = pick(chars.len() + 1);
        let c = SPECIAL[pick(SPECIAL.len())];
        match pick(3) {
            0 if i < chars.len() => {
                chars.remove(i);
            }
            1 => chars.insert(i.min(chars.len()), c),
            _ if i < chars.len() => chars[i] = c,
            _ => {}
        }
    }
    chars.into_iter().collect()
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--child") {
        return child();
    }
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("root");
    let text = mf2_conformance::spec::read_spec(root, mf2_conformance::spec::ABNF)
        .unwrap_or_else(|e| panic!("{e}"));
    let g = Grammar::parse(&text).expect("ABNF");
    let mut rng = Rng::new(42);
    let mut ox = Ox::spawn();
    let mut tally = std::collections::BTreeMap::<&str, u64>::new();
    let mut shown = std::collections::BTreeMap::<&str, u64>::new();
    for seed in 0..n {
        let base = Generator::new(&g, seed).generate("message");
        for variant in 0..3 {
            let src = if variant == 0 {
                base.clone()
            } else {
                mutate(&mut rng, &base)
            };
            let (ours, codes) = ours_report(&src);
            let theirs_report = ox.report(&src);
            let theirs_text = format!("{theirs_report:?}");
            let class = match theirs_report {
                None => "ox hangs",
                Some(theirs) if theirs == ours => "agree",
                Some(theirs)
                    if theirs.contains("syntax-error") != ours.contains("syntax-error") =>
                {
                    // ox accepts markup as a `.local` value; the ABNF allows
                    // only an expression there (mf2-syntax: code 24).
                    if codes.contains(&mf2_syntax::code::MARKUP_NOT_ALLOWED) {
                        "ox accepts markup as a declaration value (a syntax error)"
                    } else {
                        "SYNTAX DISAGREEMENT"
                    }
                }
                Some(theirs) if theirs.iter().any(|e| e.starts_with("internal:")) => {
                    "ox internal error on a message mf2-syntax parses"
                }
                // ox checks for a fallback and for duplicates only among
                // variants with the right key count; the spec (and the suite's
                // data-model-errors.json #0, #1) does not restrict them.
                Some(theirs)
                    if ours.contains("variant-key-mismatch")
                        && theirs
                            .difference(&ours)
                            .all(|e| e == "missing-fallback-variant")
                        && ours.difference(&theirs).all(|e| e == "duplicate-variant") =>
                {
                    "ox restricts fallback/duplicate checks to variants of the right arity"
                }
                Some(_) => "DATA-MODEL DISAGREEMENT",
            };
            *tally.entry(class).or_default() += 1;
            let seen = shown.entry(class).or_default();
            if class != "agree" && *seen < 3 {
                *seen += 1;
                println!("{class}: seed {seed} v{variant} ours={ours:?} ox={theirs_text} {src:?}");
            }
        }
    }
    println!("{n} seeds × 3 (generated + 2 mutations):");
    for (class, count) in tally {
        println!("  {class}: {count}");
    }
}
