//! `workload-gen`: command-line interface (also reached through
//! `cargo xtask gen-workload …`).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use workload_gen::model::canary;
use workload_gen::{Error, Knobs, Template, Workload, locale, repo_root, stats, suite};

/// Deterministic generator of the mf2-two reference workload
/// (plans/06-size-and-perf.md §2).
#[derive(Debug, Parser)]
#[command(name = "workload-gen", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Write locales, flat JSON, sites.json and one app crate per template.
    All(AllArgs),
    /// Write locales/<tag>/*.mf2 and json/<tag>.json only.
    Locales(OutArgs),
    /// Write bench/corpora/workload-<N>.json and bench/corpora/suite.json.
    Corpora(CorporaArgs),
    /// Print the shape table; exit 1 if outside the tolerances of plans/06 §2.
    Stats(StatsArgs),
    /// Print the canary strings CI greps the client wasm for (B6).
    Canaries(CanaryArgs),
    /// List the built-in templates, or copy one into a directory.
    Templates(TemplatesArgs),
}

#[derive(Debug, Args)]
struct OutArgs {
    #[command(flatten)]
    knobs: Knobs,
    /// Output directory [default: <repo>/target/workload].
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct AllArgs {
    #[command(flatten)]
    out: OutArgs,
    /// Call-site template: a built-in name or a directory holding
    /// template.toml. Repeat for several apps side by side.
    #[arg(short, long = "template", default_values_t = vec!["literal".to_owned(), "closure".to_owned()])]
    templates: Vec<String>,
}

#[derive(Debug, Args)]
struct CorporaArgs {
    #[command(flatten)]
    knobs: Knobs,
    /// Output directory [default: <repo>/bench/corpora].
    #[arg(long)]
    out: Option<PathBuf>,
    /// The WG suite's tests directory [default: <repo>/third_party/message-format-wg/test/tests].
    #[arg(long)]
    suite: Option<PathBuf>,
    /// Compare with the files on disk instead of writing; exit 1 if stale.
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Args)]
struct StatsArgs {
    #[command(flatten)]
    knobs: Knobs,
    /// Measure an existing flat JSON corpus instead of generating one.
    #[arg(long)]
    json: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CanaryArgs {
    #[command(flatten)]
    knobs: Knobs,
    /// Print only the three grep patterns, one per line.
    #[arg(long)]
    grep: bool,
}

#[derive(Debug, Args)]
struct TemplatesArgs {
    /// Copy this built-in template's files into --out.
    #[arg(long)]
    dump: Option<String>,
    /// Destination directory for --dump.
    #[arg(long)]
    out: Option<PathBuf>,
}

fn default_out() -> PathBuf {
    repo_root().join("target").join("workload")
}

fn run(cli: Cli) -> Result<(), Error> {
    match cli.command {
        Command::All(args) => {
            let templates = args
                .templates
                .iter()
                .map(|t| Template::resolve(t))
                .collect::<Result<Vec<_>, _>>()?;
            let out = args.out.out.unwrap_or_else(default_out);
            let files = workload_gen::generate(&args.out.knobs, &templates)?;
            files.write_to(&out, &args.out.knobs.summary())?;
            println!("wrote {} files to {}", files.len(), out.display());
            for t in &templates {
                println!(
                    "  app ({}): {}",
                    t.name,
                    out.join(workload_gen::app_dir(t)).display()
                );
            }
        }
        Command::Locales(args) => {
            let out = args.out.unwrap_or_else(default_out);
            let files = workload_gen::generate(&args.knobs, &[])?;
            files.write_to(&out, &args.knobs.summary())?;
            println!("wrote {} files to {}", files.len(), out.display());
        }
        Command::Corpora(args) => corpora(&args)?,
        Command::Stats(args) => {
            let report = match &args.json {
                Some(path) => {
                    let text = std::fs::read_to_string(path)?;
                    let map: serde_json::Map<String, serde_json::Value> =
                        serde_json::from_str(&text)?;
                    let corpus: Vec<(String, String)> = map
                        .into_iter()
                        .map(|(k, v)| (k, v.as_str().unwrap_or_default().to_owned()))
                        .collect();
                    let mut report = stats::Report::default();
                    stats::corpus(&mut report, &corpus);
                    report
                }
                None => workload_gen::report(&args.knobs)?,
            };
            print!("{}", report.render());
            if !report.passed() {
                return Err(Error::Shape(report.failures().join("; ")));
            }
        }
        Command::Canaries(args) => {
            if args.grep {
                println!("{}", canary::MESSAGE_ID);
                println!("{}", canary::VARIABLE);
                println!("{}", canary::TEXT_PREFIX);
            } else {
                println!("message-id\t{}", canary::MESSAGE_ID);
                println!("variable\t{}", canary::VARIABLE);
                println!("text-prefix\t{}", canary::TEXT_PREFIX);
                for loc in locale::locales(&args.knobs)? {
                    println!("text\t{}\t{}", loc.tag, canary::text(loc.tag));
                }
            }
        }
        Command::Templates(args) => match args.dump {
            Some(name) => {
                let (toml, support) =
                    workload_gen::template::builtin_files(&name).ok_or_else(|| {
                        Error::Template {
                            template: name.clone(),
                            message: "no such built-in template".into(),
                        }
                    })?;
                let out = args
                    .out
                    .ok_or_else(|| Error::Knobs("--dump needs --out <dir>".into()))?;
                std::fs::create_dir_all(&out)?;
                std::fs::write(out.join("template.toml"), toml)?;
                std::fs::write(out.join("support.rs"), support)?;
                println!(
                    "wrote {} and support.rs",
                    out.join("template.toml").display()
                );
            }
            None => {
                for name in workload_gen::template::builtin_names() {
                    let t = Template::builtin(name)?;
                    println!("{name}\t{}", t.description);
                }
            }
        },
    }
    Ok(())
}

fn corpora(args: &CorporaArgs) -> Result<(), Error> {
    let root = repo_root();
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| root.join("bench").join("corpora"));
    let suite_dir = args.suite.clone().unwrap_or_else(|| {
        root.join("third_party")
            .join("message-format-wg")
            .join("test")
            .join("tests")
    });
    let wl = Workload::generate(&args.knobs)?;
    let corpus_name = format!("workload-{}.json", args.knobs.messages);
    let entries = suite::entries(&suite_dir)?;
    let outputs = [
        (corpus_name, workload_gen::corpus_json(&wl)?),
        ("suite.json".to_owned(), suite::to_json(&entries)),
    ];
    if args.check {
        let mut stale = Vec::new();
        for (name, content) in &outputs {
            let path = out.join(name);
            if std::fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
                stale.push(path.display().to_string());
            }
        }
        if !stale.is_empty() {
            return Err(Error::Stale(stale.join(", ")));
        }
        println!("corpora up to date ({} suite tests)", entries.len());
    } else {
        std::fs::create_dir_all(&out)?;
        for (name, content) in &outputs {
            std::fs::write(out.join(name), content)?;
            println!("wrote {}", out.join(name).display());
        }
        println!(
            "{} suite tests, {} messages",
            entries.len(),
            wl.messages.len()
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("workload-gen: {e}");
            ExitCode::FAILURE
        }
    }
}
