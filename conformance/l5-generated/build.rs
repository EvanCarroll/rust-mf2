//! The corpus and the call sites, from `l4gen`'s generator.
//!
//! `MF2_L5_GENERATED` messages (default 200) from seed `MF2_L5_SEED`
//! (default 1). Only the messages the spec accepts go in — a corpus cannot
//! hold the others, and that they are refused is layer L5's own business
//! (the suite's error tests). Everything is compiled in one locale: a corpus
//! has one source locale, and what this layer adds over L4 is the manifest,
//! the slots and the macro, not the locale data.

use std::fmt::Write as _;

use mf2_conformance::abnf::Grammar;
use mf2_conformance::l4gen;
use mf2_conformance::spec::{ABNF, spec_path};
use mf2_l4_runner::ArgSpec;
use mf2_l5_gen::{Message, Param, rust_str};
use mf2_runtime::BidiStrategy;

/// The locale the corpus is built in.
const LOCALE: &str = "en";

fn main() {
    println!("cargo::rerun-if-env-changed=MF2_L5_GENERATED");
    println!("cargo::rerun-if-env-changed=MF2_L5_SEED");
    if let Err(e) = generate() {
        println!("cargo::error={e}");
        std::process::exit(1);
    }
}

fn number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn generate() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );
    // conformance/l5-generated → the repository root.
    let root = manifest
        .ancestors()
        .nth(2)
        .ok_or("this crate sits two directories below the root")?;
    let abnf = spec_path(root, ABNF)?;
    println!("cargo::rerun-if-changed={}", abnf.display());
    let grammar = Grammar::parse(&std::fs::read_to_string(&abnf)?)?;

    let wanted = number("MF2_L5_GENERATED", 200);
    let first = number("MF2_L5_SEED", 1);

    let mut messages = Vec::new();
    let mut meta = String::from(
        "/// Every generated case: its id, the message it was built from, and\n\
         /// whether the seed asked for bidi isolation to be off — everything\n\
         /// L4's runner needs to be given the same thing.\n\
         pub static META: &[(&str, &str, bool)] = &[\n",
    );
    let mut args = String::from(
        "/// The arguments of each case, as L4's runner takes them. The call\n\
         /// site passes the same values through `ArgValue`.\n\
         #[allow(clippy::type_complexity)]\n\
         pub static ARGS: &[(&str, fn() -> ::std::vec::Vec<(::std::string::String, \
         ::mf2_l4_runner::ArgSpec)>)] = &[\n",
    );

    let mut seed = first;
    let mut kept = 0u64;
    // A generous bound: the generator refuses nothing often, but a seed that
    // does not parse or is not valid is skipped, and the loop must end.
    let limit = first + wanted.saturating_mul(100) + 1_000;
    while kept < wanted && seed < limit {
        let generated = l4gen::message(&grammar, seed);
        seed += 1;
        let Ok(generated) = generated else { continue };
        if !generated.valid {
            continue;
        }
        let id = format!("gen.t{kept:06}");
        let params: Vec<(String, Param)> = generated
            .args
            .iter()
            .map(|(name, spec)| (name.clone(), param(spec)))
            .collect();
        let _ = writeln!(
            meta,
            "    ({}, {}, {}),",
            rust_str(&id),
            rust_str(&generated.source),
            matches!(generated.bidi, BidiStrategy::None)
        );
        let _ = write!(args, "    ({}, || ::std::vec![", rust_str(&id));
        for (name, spec) in &generated.args {
            let _ = write!(
                args,
                "({}.to_owned(), {}), ",
                rust_str(name),
                arg_spec(spec)
            );
        }
        let _ = writeln!(args, "]),");
        messages.push(Message {
            id,
            src: generated.source,
            params,
            invalid: false,
        });
        kept += 1;
    }
    if kept < wanted {
        return Err(format!("only {kept} of {wanted} messages were generated").into());
    }
    meta.push_str("];\n\n");
    args.push_str("];\n");
    meta.push_str(&args);
    let _ = writeln!(
        meta,
        "\n/// The locale the corpus was built in.\npub const LOCALE: &str = {LOCALE:?};"
    );

    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    mf2_l5_gen::build(&out, LOCALE, &messages, &meta)?;
    Ok(())
}

/// The call site's form of one generated argument.
fn param(spec: &ArgSpec) -> Param {
    match spec {
        ArgSpec::Str(s) => Param::Str(s.clone()),
        ArgSpec::Int(n) => Param::Int(*n),
        ArgSpec::Float(x) => Param::Float(*x),
        ArgSpec::Decimal(d) => Param::Decimal(d.clone()),
        ArgSpec::DateTime(d) => Param::DateTime(iso(d)),
        ArgSpec::Other => Param::Opaque,
    }
}

/// The same argument as L4's runner takes it, as Rust source. A date/time
/// goes through the *same text* the call site parses, so that the two sides
/// are given the same value and not two readings of one.
fn arg_spec(spec: &ArgSpec) -> String {
    match spec {
        ArgSpec::Str(s) => format!("::mf2_l4_runner::ArgSpec::Str({}.to_owned())", rust_str(s)),
        ArgSpec::Int(n) => format!("::mf2_l4_runner::ArgSpec::Int({n}i64)"),
        ArgSpec::Float(x) => format!("::mf2_l4_runner::ArgSpec::Float({})", rust_f64(*x)),
        ArgSpec::Decimal(d) => format!(
            "::mf2_l4_runner::ArgSpec::Decimal({}.to_owned())",
            rust_str(d)
        ),
        ArgSpec::DateTime(d) => {
            format!("::mf2_l4_runner::ArgSpec::date_time({})", rust_str(&iso(d)))
        }
        ArgSpec::Other => "::mf2_l4_runner::ArgSpec::Other".to_owned(),
    }
}

/// A Rust expression for `x`: `{x:?}` writes `inf` and `NaN`, which are not
/// literals. The generator produces both on purpose.
fn rust_f64(x: f64) -> String {
    if x.is_nan() {
        "f64::NAN".to_owned()
    } else if x == f64::INFINITY {
        "f64::INFINITY".to_owned()
    } else if x == f64::NEG_INFINITY {
        "f64::NEG_INFINITY".to_owned()
    } else {
        format!("{x:?}f64")
    }
}

/// A date/time as the literal text both sides parse.
fn iso(value: &mf2_runtime::DateTime<'_>) -> String {
    let mut out = String::new();
    value.write_iso(&mut out);
    out
}
