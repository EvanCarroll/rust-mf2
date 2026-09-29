//! The ambient forms' cost per call, natively, with the native store
//! installed and nothing in a request: `to_cow` and `to_string` of a simple
//! message, `to_string` and `{}` of a 1-argument pattern. Built with and
//! without `ssr` (`run.sh`), the difference is the lookup's first step, which
//! a build that unifies `ssr` and `native` pays on every native format
//! (plans/19-native-and-terminal.md §5; A4 left it to C2).
//!
//! One run: the four rows interleaved round by round, each round a batch of
//! calls per row; prints each row's median ns per call over the rounds, as
//! one JSON line.

use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;

use mf2::{ArgValue, CatalogFile, Corpus, Dir, Function, MsgId, Registry, functions, tr, tr_args1};
use mf2_catalog::Manifest;
use mf2_catalog::writer::{self, Options};

static FUNCTIONS: [(&str, &dyn Function); 1] = [("string", &functions::STRING)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

fn corpus() -> &'static Corpus {
    let manifest = Manifest {
        ids: vec!["m0".to_owned(), "m1".to_owned()],
        slots: vec![Vec::new(), vec!["name".to_owned()]],
        markup: vec![Vec::new(), Vec::new()],
        functions: Vec::new(),
    };
    let sources = ["Welcome to the trace", "Hello, {$name}!"];
    let parsed: Vec<_> = sources
        .iter()
        .map(|s| mf2_syntax::parse_model(s).message.expect("parses"))
        .collect();
    let messages: Vec<_> = parsed.iter().map(Some).collect();
    let bytes =
        writer::catalog(&manifest, &messages, &Options::new("en", Dir::Ltr)).expect("writes");
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let files: &'static [CatalogFile] =
        Box::leak(Box::new([CatalogFile::new("en", "en.mf2b", Some(bytes))]));
    Box::leak(Box::new(Corpus::new(
        "en",
        manifest.hash(),
        &[("en", Dir::Ltr)],
        &REGISTRY,
        files,
    )))
}

fn main() {
    let rounds: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(101);
    mf2::native::install(corpus());
    let simple = tr(MsgId::from_raw(0));
    let pattern = tr_args1(MsgId::from_raw(1), ArgValue::str_static("Ada"));
    let mut out = String::with_capacity(256);
    let rows: [(&str, &dyn Fn(&mut String) -> usize); 4] = [
        ("simple to_cow", &|_| black_box(simple).to_cow().len()),
        ("simple to_string", &|_| black_box(simple).to_string().len()),
        ("pattern to_string", &|_| {
            black_box(&pattern).to_string().len()
        }),
        ("pattern {}", &|out: &mut String| {
            out.clear();
            let _ = write!(out, "{}", black_box(&pattern));
            out.len()
        }),
    ];
    const BATCH: usize = 20_000;
    let mut samples = vec![Vec::with_capacity(rounds); rows.len()];
    let mut check = 0usize;
    for round in 0..rounds {
        for k in 0..rows.len() {
            let i = (round + k) % rows.len();
            let (_, row) = rows[i];
            let start = Instant::now();
            for _ in 0..BATCH {
                check = check.wrapping_add(row(&mut out));
            }
            samples[i].push(start.elapsed().as_nanos() as f64 / BATCH as f64);
        }
    }
    let medians: Vec<String> = rows
        .iter()
        .zip(&mut samples)
        .map(|((name, _), s)| {
            s.sort_by(f64::total_cmp);
            format!("\"{name}\": {:.2}", s[s.len() / 2])
        })
        .collect();
    let build = if cfg!(feature = "ssr") {
        "ssr+native"
    } else {
        "native"
    };
    println!(
        "{{\"build\": \"{build}\", {}, \"check\": {check}}}",
        medians.join(", ")
    );
}
