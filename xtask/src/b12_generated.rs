//! `cargo xtask b12-generated`: budgets **B1′** and **B13** on the module
//! `mf2-build` generates, and the rule that the other side's features cost
//! this side nothing.
//!
//! Phase 5a measured both by editing the fixture's corpus by hand and putting
//! it back: B1′ =
//! +0 B, B13 = 13,599 B avoided. This turns them into a gate — the corpora
//! are two cargo features of the fixture, so a regression fails a command
//! instead of waiting to be re-measured.
//!
//! | Build | Corpus | Features | What it shows |
//! |---|---|---|---|
//! | E | nothing a function crate could serve | `hydrate` | the floor |
//! | F | the same | `hydrate,host-web-number-builtin,host-web-datetime-iso` | **B1′** = F − E must be **+0**: two function crates linked, neither reachable from the generated registry |
//! | A | the fixture's own | `hydrate,host-web-number-builtin` | a corpus that uses `:integer` |
//! | B | the same plus `:currency`, `:unit`, `:percent` | `hydrate,host-web-number-builtin` | **B13** = B − A: what a corpus that does not use them does not pay |
//! | each pair of `OTHER_SIDE` | the fixture's own, or a `:datetime` message | a browser's features, and the same with the server's beside them | **the other side** must link **no symbol** the browser's own features did not |
//!
//! An application writes both sides' features on its one `mf2` line, so its
//! browser build sees the server's. A feature that belongs to the server —
//! its number formatter, its date formatter's cache, the load number that
//! cache keys on — must leave the browser alone, and the cost table cannot say
//! so: it builds the client with the client's features only.
//!
//! B1′ and B13 are bytes: `wasm32-unknown-unknown`, profile `wasm-release`,
//! raw size of the client binary — the same figures Phase 5a reported, so the
//! two are comparable.
//!
//! **The other side is symbols, not bytes** (owner, 2026-10-07). It was bytes,
//! and had to stop being: the server's features change the feature set of
//! `mf2-fn-datetime`, `mf2-runtime` and `mf2-catalog`, so those crates compile
//! under a different `-Cmetadata` and every symbol in them is renamed, even
//! where the server's code is `cfg`'d out of this target and the bodies are
//! identical. Asking two such compilations for byte-identical wasm asks rustc
//! for something it does not promise: run 22 (2026-10-06) read +113 B, and a
//! `twiggy diff` of 187 rows was renames — every row that was not hidden by
//! the listing paired off to the byte, same function, same size, opposite
//! sign.
//!
//! So each pair is built with the name section kept, `twiggy top` lists what
//! each build linked, the crate disambiguators are erased, and the two sets
//! must be equal. That is the rule itself rather than a proxy for it, and it
//! is the stronger test: a symbol the server's features drag in fails even if
//! it costs nothing, where the byte count would have let a size-neutral one
//! through. The bytes are still printed, because a jump worth looking at is
//! worth seeing; they decide nothing. A failing pair also prints its sections,
//! which says whether what moved was code, data or a custom section.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::cmd;
use crate::error::{Error, Result};
use crate::fsx;

/// The fixture's client binary.
const BIN: &str = "mf2-i18n-client";

/// What Phase 5a measured for B13, and how far this may drift before it is a
/// regression rather than a corpus difference.
const B13_EXPECTED: i64 = 13_599;
const B13_TOLERANCE: f64 = 0.10;

/// How many symbols a failing pair lists before it stops.
const SHOWN: usize = 40;

/// The other side's features: a browser build, then the same with the
/// server's feature an application writes beside it. Each pair must link the
/// same symbols.
const OTHER_SIDE: [(&str, &str, &str); 6] = [
    (
        "a browser on `Intl` numbers beside a server on mf2's own number code",
        "hydrate,host-web-number-intl",
        "hydrate,host-web-number-intl,host-std-number-builtin",
    ),
    (
        "a browser in plain digits beside a server on mf2's own number code",
        "hydrate,host-web-number-plain",
        "hydrate,host-web-number-plain,host-std-number-builtin",
    ),
    (
        "a browser on mf2's own number code beside a server in plain digits",
        "hydrate,host-web-number-builtin",
        "hydrate,host-web-number-builtin,host-std-number-plain",
    ),
    (
        "an `Intl` browser beside an ICU4X server",
        "hydrate,host-web-number-plain,host-web-datetime-intl,corpus-dates",
        "hydrate,host-web-number-plain,host-web-datetime-intl,host-std-datetime-icu,corpus-dates",
    ),
    (
        "an ICU4X browser beside an ICU4X server",
        "hydrate,host-web-number-plain,host-web-datetime-icu,corpus-dates",
        "hydrate,host-web-number-plain,host-web-datetime-icu,host-std-datetime-icu,corpus-dates",
    ),
    (
        "a cached ICU4X browser beside an ICU4X server",
        "hydrate,host-web-number-plain,host-web-datetime-icu-cached,corpus-dates",
        "hydrate,host-web-number-plain,host-web-datetime-icu-cached,host-std-datetime-icu,corpus-dates",
    ),
];

/// One build of the fixture's client: its size, and where its artifact was
/// kept. Every build writes the same path, so the copy is what lets a pair be
/// compared after both have been built.
struct Built {
    size: u64,
    kept: PathBuf,
}

/// One pair of `OTHER_SIDE`, built and read.
struct Pair {
    what: &'static str,
    with: &'static str,
    alone: Built,
    beside: Built,
    /// Symbols the build with the server's features linked and the browser's
    /// own did not. Any at all is the leak this gate exists to catch.
    only_beside: Vec<String>,
    /// The other direction: a symbol the server's features took *away*. Not a
    /// leak, but not something to pass over in silence either.
    only_alone: Vec<String>,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let stripped = root.join("target").join("b12-generated");
    let named = root.join("target").join("b12-generated-names");
    let build = |features: &str, label: &str| -> Result<Built> {
        eprintln!("b12-generated: building --features {features}");
        build_one(root, &stripped, features, label, &[])
    };

    let e = build("hydrate,corpus-plain", "e")?.size;
    let f = build(
        "hydrate,host-web-number-builtin,host-web-datetime-iso,corpus-plain",
        "f",
    )?
    .size;
    let a = build("hydrate,host-web-number-builtin", "a")?.size;
    let b = build("hydrate,host-web-number-builtin,corpus-measures", "b")?.size;
    let others: Vec<Pair> = OTHER_SIDE
        .iter()
        .enumerate()
        .map(|(i, &(what, without, with))| pair(root, &named, i, what, without, with))
        .collect::<Result<_>>()?;

    #[allow(clippy::cast_possible_wrap)]
    let b1 = f as i64 - e as i64;
    #[allow(clippy::cast_possible_wrap)]
    let b13 = b as i64 - a as i64;

    println!("\n| build | corpus | features | .wasm |");
    println!("|---|---|---|---:|");
    println!("| E | nothing to serve | hydrate | {e} |");
    println!("| F | the same | hydrate,host-web-number-builtin,host-web-datetime-iso | {f} |");
    println!("| A | the fixture's | hydrate,host-web-number-builtin | {a} |");
    println!("| B | A plus the measures | hydrate,host-web-number-builtin | {b} |");
    for p in &others {
        println!(
            "| the other side | {} | {} | {} → {} |",
            p.what, p.with, p.alone.size, p.beside.size
        );
    }
    println!(
        "\nThe other side's four figures above are the names-kept build, which is \
         larger than the shipped one and is not what B1′ and B13 measure: this row \
         is judged on its symbols, not its bytes."
    );
    println!("\nB1′ = F − E = {b1:+} B (must be +0)");
    println!("B13 = B − A = {b13:+} B avoided (Phase 5a: {B13_EXPECTED:+})");

    let mut failures = Vec::new();
    for p in &others {
        #[allow(clippy::cast_possible_wrap)]
        let delta = p.beside.size as i64 - p.alone.size as i64;
        let crossed = p.only_beside.len();
        let lost = p.only_alone.len();
        println!(
            "the other side, {}: {crossed} symbol(s) crossed, {lost} only without \
             (must be 0 and 0; bytes {delta:+})",
            p.what
        );
        if crossed > 0 || lost > 0 {
            failures.push(format!(
                "the other side's features changed what this one links ({} crossed, {} \
                 only without; {}; with `{}`): a feature of the server reached the \
                 browser's build",
                crossed, lost, p.what, p.with
            ));
        }
    }
    // After every verdict, so that the detail cannot be mistaken for one.
    for p in &others {
        if !p.only_beside.is_empty() || !p.only_alone.is_empty() {
            detail(p);
        }
    }
    if b1 != 0 {
        failures.push(format!(
            "B1′ is {b1:+} B: a function crate that the generated registry never names \
             reached the wasm"
        ));
    }
    #[allow(clippy::cast_precision_loss)]
    let drift = (b13 - B13_EXPECTED).abs() as f64 / B13_EXPECTED as f64;
    if drift > B13_TOLERANCE {
        failures.push(format!(
            "B13 is {b13:+} B, {:.0} % from Phase 5a's {B13_EXPECTED:+}: the measure \
             functions' share of the wasm moved",
            drift * 100.0
        ));
    }
    if failures.is_empty() {
        eprintln!("b12-generated: B1′, B13 and the other side's symbols hold");
        return Ok(());
    }
    Err(Error::CommandFailed {
        command: "b12-generated".to_owned(),
        status: format!("{} budget(s) failed", failures.len()),
        stderr: failures.join("\n"),
    })
}

/// Builds one pair with the name section kept and reads what each linked.
fn pair(
    root: &Path,
    target: &Path,
    i: usize,
    what: &'static str,
    without: &'static str,
    with: &'static str,
) -> Result<Pair> {
    let alone = build_named(root, target, without, &format!("other-{i}-without"))?;
    let beside = build_named(root, target, with, &format!("other-{i}-with"))?;
    let here = symbols(root, &alone.kept)?;
    let there = symbols(root, &beside.kept)?;
    Ok(Pair {
        what,
        with,
        only_beside: there.difference(&here).cloned().collect(),
        only_alone: here.difference(&there).cloned().collect(),
        alone,
        beside,
    })
}

/// What a failing pair linked that its partner did not, and which sections
/// moved. Printed, never fatal: the budget has already failed, and an
/// unreadable module must not change which error the gate reports.
fn detail(p: &Pair) {
    println!(
        "\n#### what the other side's features changed: {}\n",
        p.what
    );
    for (title, names) in [
        ("linked only with the server's features", &p.only_beside),
        ("linked only without them", &p.only_alone),
    ] {
        if names.is_empty() {
            continue;
        }
        println!("{} ({}):\n", title, names.len());
        for name in names.iter().take(SHOWN) {
            println!("* `{name}`");
        }
        if names.len() > SHOWN {
            println!("* … and {} more", names.len() - SHOWN);
        }
        println!();
    }
    match section_delta(&p.alone.kept, &p.beside.kept) {
        Ok(rows) => {
            println!("| section | browser alone | with the server's features | delta |");
            println!("|---|---:|---:|---:|");
            for (name, alone, beside) in &rows {
                #[allow(clippy::cast_possible_wrap)]
                let delta = *beside as i64 - *alone as i64;
                println!("| `{name}` | {alone} | {beside} | {delta:+} |");
            }
        }
        Err(err) => println!("the sections could not be read: {err}"),
    }
}

/// Every symbol a module links, as `twiggy top` lists it, with the crate
/// disambiguators erased so that the same function compiled under a different
/// `-Cmetadata` is the same name here. twiggy is in the image at the version
/// `tools/ci/setup.sh` pins, for `bench/b12/check.sh` and
/// `tools/fmt-check.sh`.
fn symbols(root: &Path, wasm: &Path) -> Result<BTreeSet<String>> {
    let out = cmd::run_capture(
        OsStr::new("twiggy"),
        &[
            OsStr::new("top"),
            OsStr::new("-n"),
            OsStr::new("1000000"),
            wasm.as_os_str(),
        ],
        root,
        &[],
    )?;
    let text = String::from_utf8_lossy(&out);
    let mut names = BTreeSet::new();
    for line in text.lines() {
        // ` 3584 ┊ 17.48% ┊ <item>`, as tools/fmt-check.sh reads it. The
        // header and the rule above the rows separate with `│`, not `┊`, so
        // they fall out here; twiggy's closing rows do have three fields and
        // `is_summary` is what drops those.
        let mut fields = line.split('┊');
        let (Some(_), Some(_), Some(item)) = (fields.next(), fields.next(), fields.next()) else {
            continue;
        };
        let item = item.trim();
        if !item.is_empty() && !is_summary(item) {
            names.insert(normalize(item));
        }
    }
    Ok(names)
}

/// twiggy's own closing rows, which are not symbols. `Σ [n Total Rows]`
/// counts the rows, and the two builds of a pair do not have the same number
/// of them, so leaving it in would put a difference in every set.
fn is_summary(item: &str) -> bool {
    item.starts_with('Σ') || item.starts_with("... and ") || item.starts_with("… and ")
}

/// A symbol as this gate compares it. Two builds of a pair name the same
/// function differently in two ways, neither of which says anything about what
/// was linked, and run 23 found both: the crate disambiguator, and LLVM's
/// numeric suffix on a local symbol.
fn normalize(item: &str) -> String {
    erase_hashes(strip_local_suffix(item))
}

/// `…::write_char.190` without the `.190`. LLVM numbers local symbols by
/// where they fall in the module, so the same function carries a different
/// number in two builds that link a different number of things --- which is
/// every pair here.
fn strip_local_suffix(item: &str) -> &str {
    match item.rsplit_once('.') {
        Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => item,
    }
}

/// `foo[0123456789abcdef]::bar` without the `[…]`: the crate disambiguator,
/// which changes with a crate's feature set and so differs between the two
/// builds of a pair for every symbol, saying nothing about what was linked.
///
/// It is a hexadecimal run of no fixed width --- `mf2_catalog[cd525ef543e6c17]`
/// in run 23 is fifteen characters --- so the width is a range. Eight at the
/// least, which is what keeps a type's own brackets (`[u8; 32]`) out of it,
/// the hexadecimal test being the other half.
fn erase_hashes(item: &str) -> String {
    let mut out = String::with_capacity(item.len());
    let mut rest = item;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let hash = after
            .find(']')
            .filter(|end| (8..=16).contains(end))
            .filter(|end| after[..*end].bytes().all(|b| b.is_ascii_hexdigit()));
        match hash {
            Some(end) => {
                out.push_str(&rest[..open]);
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Every section of both modules, by name, in the order the first carries
/// them. A module may hold more than one custom section of a name; the first
/// of each is what this compares, which is enough to say where bytes went.
fn section_delta(alone: &Path, beside: &Path) -> Result<Vec<(String, u64, u64)>> {
    let mut rows: Vec<(String, u64, u64)> = sections(alone)?
        .into_iter()
        .map(|(name, size)| (name, size, 0))
        .collect();
    for (name, size) in sections(beside)? {
        if let Some(row) = rows.iter_mut().find(|row| row.0 == name) {
            row.2 = size;
        } else {
            rows.push((name, 0, size));
        }
    }
    Ok(rows)
}

/// A wasm module's sections and the size of each payload: an eight-byte
/// header, then `(id, size, payload)` for each. A custom section is named by
/// the name its payload begins with.
fn sections(path: &Path) -> Result<Vec<(String, u64)>> {
    let bytes = read(path)?;
    let malformed = |detail: &str| Error::CommandFailed {
        command: format!("reading {}", path.display()),
        status: "malformed wasm".to_owned(),
        stderr: format!(": {detail}"),
    };
    if bytes.get(..4) != Some(&b"\0asm"[..]) {
        return Err(malformed("not a wasm module"));
    }
    let mut out = Vec::new();
    let mut at = 8usize;
    while at < bytes.len() {
        let id = *bytes.get(at).ok_or_else(|| malformed("a section id"))?;
        at += 1;
        let rest = bytes.get(at..).ok_or_else(|| malformed("a section size"))?;
        let (size, used) = uleb(rest).ok_or_else(|| malformed("a section size"))?;
        at += used;
        let end = at
            .checked_add(size)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| malformed("a section past the end"))?;
        let name = if id == 0 {
            custom_name(bytes.get(at..end).unwrap_or_default())
        } else {
            section_name(id).map_or_else(|| format!("section {id}"), str::to_owned)
        };
        out.push((name, u64::try_from(size).unwrap_or(u64::MAX)));
        at = end;
    }
    Ok(out)
}

/// A custom section's name, which its payload begins with.
fn custom_name(payload: &[u8]) -> String {
    match uleb(payload) {
        Some((len, used)) => match used.checked_add(len).and_then(|end| payload.get(used..end)) {
            Some(text) => format!("custom {}", String::from_utf8_lossy(text)),
            None => "custom (a name past the end)".to_owned(),
        },
        None => "custom (unnamed)".to_owned(),
    }
}

/// The name the specification gives a section id, for the ids that have one.
fn section_name(id: u8) -> Option<&'static str> {
    Some(match id {
        1 => "type",
        2 => "import",
        3 => "function",
        4 => "table",
        5 => "memory",
        6 => "global",
        7 => "export",
        8 => "start",
        9 => "element",
        10 => "code",
        11 => "data",
        12 => "data count",
        13 => "tag",
        _ => return None,
    })
}

/// A LEB128 unsigned integer: its value, and how many bytes it took.
fn uleb(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    let mut shift = 0u32;
    for (i, byte) in bytes.iter().enumerate().take(10) {
        value |= usize::from(byte & 0x7f).checked_shl(shift)?;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
    }
    None
}

/// Builds the fixture's client binary with `features`, keeps the artifact as
/// `label`, and returns what it measured.
fn build_one(
    root: &Path,
    target: &Path,
    features: &str,
    label: &str,
    extra: &[(&str, &OsStr)],
) -> Result<Built> {
    let wasm = compile(root, target, features, extra)?;
    let bytes = read(&wasm)?;
    let kept = target.join("kept").join(format!("{label}.wasm"));
    fsx::write(&kept, &bytes)?;
    Ok(Built {
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        kept,
    })
}

/// The same build with the name section kept, which the `wasm-release`
/// profile strips. Only the name section makes a symbol readable, and the
/// other side is judged on symbols.
fn build_named(root: &Path, target: &Path, features: &str, label: &str) -> Result<Built> {
    eprintln!("b12-generated: building --features {features} with the names kept");
    build_one(
        root,
        target,
        features,
        label,
        &[("CARGO_PROFILE_WASM_RELEASE_STRIP", OsStr::new("none"))],
    )
}

/// Builds the fixture's client binary with `features` and returns the path it
/// wrote. Every feature set writes the same path.
fn compile(
    root: &Path,
    target: &Path,
    features: &str,
    extra: &[(&str, &OsStr)],
) -> Result<PathBuf> {
    let cargo = cmd::cargo();
    let args: Vec<&OsStr> = vec![
        OsStr::new("build"),
        OsStr::new("--quiet"),
        OsStr::new("-p"),
        OsStr::new("mf2-i18n-fixture"),
        OsStr::new("--bin"),
        OsStr::new(BIN),
        OsStr::new("--no-default-features"),
        OsStr::new("--features"),
        OsStr::new(features),
        OsStr::new("--target"),
        OsStr::new("wasm32-unknown-unknown"),
        OsStr::new("--profile"),
        OsStr::new("wasm-release"),
    ];
    let jobs = std::env::var_os("CARGO_BUILD_JOBS").unwrap_or_else(|| OsString::from("3"));
    let mut envs: Vec<(&str, &OsStr)> = vec![
        ("CARGO_TARGET_DIR", target.as_os_str()),
        ("CARGO_BUILD_JOBS", jobs.as_os_str()),
    ];
    envs.extend_from_slice(extra);
    cmd::run_inherit_env(&cargo, &args, root, &envs)?;
    Ok(target
        .join("wasm32-unknown-unknown")
        .join("wasm-release")
        .join(format!("{BIN}.wasm")))
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{erase_hashes, is_summary, normalize, strip_local_suffix};

    #[test]
    fn run_23s_pairs_normalise_to_the_same_name() {
        // LLVM's suffix: the same function, numbered by where it falls in a
        // module that links a different number of things.
        assert_eq!(
            normalize("<alloc::string::String as core::fmt::Write>::write_char.190"),
            normalize("<alloc::string::String as core::fmt::Write>::write_char.184")
        );
        // A fifteen-character disambiguator, and a suffix, in one symbol.
        assert_eq!(
            normalize("<mf2_catalog[cd525ef543e6c17]::reader::Catalog>::text.141"),
            normalize("<mf2_catalog[a0b1c2d3e4f5061]::reader::Catalog>::text.135")
        );
        assert_eq!(
            normalize("<mf2_catalog[cd525ef543e6c17]::reader::Catalog>::text.141"),
            "<mf2_catalog::reader::Catalog>::text"
        );
        // What must NOT collapse: these are the leak run 23 found, and they
        // have no suffix and no disambiguator to lose.
        assert_ne!(
            normalize("<u8 as core::fmt::Display>::fmt"),
            normalize("<u8 as core::fmt::LowerHex>::fmt")
        );
        assert_ne!(
            normalize("<alloc::sync::Arc<dyn mf2_runtime::value::CustomValue>>::drop_slow"),
            normalize("<alloc::sync::Arc<dyn mf2::arg::ArgSource>>::drop_slow")
        );
        // A trailing dot-digits is only a suffix when it is all digits.
        assert_eq!(
            strip_local_suffix("custom section '.debug_info'"),
            "custom section '.debug_info'"
        );
        assert_eq!(
            strip_local_suffix("<u8 as core::fmt::Display>::fmt"),
            "<u8 as core::fmt::Display>::fmt"
        );
    }

    #[test]
    fn a_disambiguator_is_erased_and_nothing_else_is() {
        assert_eq!(
            erase_hashes("mf2_runtime[bf3a5801a21e5773]::number::decimal::split_literal"),
            "mf2_runtime::number::decimal::split_literal"
        );
        // Both sides of a rename pair become the same name, which is the whole
        // point: run 22's diff was 187 rows of exactly this.
        assert_eq!(
            erase_hashes("mf2_fn_datetime[4ac354dc10a0fb5e]::literal::parse_literal"),
            erase_hashes("mf2_fn_datetime[a2a5270a0ad98b97]::literal::parse_literal")
        );
        // A bracket that is not a disambiguator stays: a slice type, a length,
        // twiggy's own `Σ [187 Total Rows]`.
        assert_eq!(
            erase_hashes("<[u8; 32] as Foo>::bar"),
            "<[u8; 32] as Foo>::bar"
        );
        assert_eq!(erase_hashes("Σ [187 Total Rows]"), "Σ [187 Total Rows]");
        // twiggy's closing rows are not symbols, and the row count differs
        // between the two builds of a pair.
        assert!(is_summary("Σ [187 Total Rows]"));
        assert!(is_summary("... and 147 more."));
        assert!(!is_summary("mf2_runtime::number::decimal::split_literal"));
        // Sixteen characters, but not hex.
        assert_eq!(
            erase_hashes("a[zzzzzzzzzzzzzzzz]::b"),
            "a[zzzzzzzzzzzzzzzz]::b"
        );
        // Two of them, and a tail after the last.
        assert_eq!(
            erase_hashes("<a[0123456789abcdef]::T as b[fedcba9876543210]::U>::run"),
            "<a::T as b::U>::run"
        );
    }
}
